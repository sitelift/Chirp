//! One Windows message thread owns BOTH hooks and all detection/suppression state.
//! Callbacks never call Tauri, acquire locks, perform I/O, or wait for a consumer.
use crate::hotkey::{conflict, matches, Binding, Key};
use crossbeam_channel::{Receiver, Sender};
use std::{
    cell::RefCell,
    sync::{Mutex, OnceLock},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

type CaptureReply = tokio::sync::oneshot::Sender<Result<Vec<Key>, String>>;
enum Control {
    /// `[hold, toggle, cancel]`.
    Configure([Option<Binding>; 3]),
    /// `true` when Esc ends the capture; `false` records Esc as a key (the
    /// cancel target, whose default is Esc).
    Capture(CaptureReply, bool),
    Cancel,
    Stop,
}
enum Output {
    Press(&'static str, Instant),
    Release(Instant),
    Capture(CaptureReply, Result<Vec<Key>, String>),
    /// Keys held so far during a binding capture, for live display.
    CaptureKeys(Vec<Key>),
    /// The cancel binding while a dictation is running: cancel it.
    Cancel,
}

/// Set while a dictation records, so the cancel binding (Esc by default)
/// cancels it from any app. The completing press is swallowed, with its
/// release, so it does not also reach the focused app. Unarmed, the cancel
/// binding does nothing and its keys pass through.
pub static ESCAPE_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct Capture {
    reply: CaptureReply,
    keys: Vec<Key>,
    deadline: Instant,
    escape_cancels: bool,
}
struct Input {
    bindings: [Option<Binding>; 2],
    cancel: Option<Binding>,
    held: Vec<Key>,
    swallowed: Vec<Key>,
    active: [bool; 2],
    cancel_active: bool,
    resync_requested: bool,
    inhibit: bool,
    win_mask_pending: bool,
    capture: Option<Capture>,
    output: Sender<Output>,
    foreground: isize,
}
thread_local! { static INPUT: RefCell<Option<Input>> = const { RefCell::new(None) }; }
struct Service {
    tx: Sender<Control>,
    thread: JoinHandle<()>,
}
static SERVICE: OnceLock<Mutex<Option<Service>>> = OnceLock::new();
fn service() -> &'static Mutex<Option<Service>> {
    SERVICE.get_or_init(|| Mutex::new(None))
}
fn send(command: Control) -> Result<(), String> {
    service()
        .lock()
        .map_err(|_| "Input service poisoned")?
        .as_ref()
        .ok_or("Input service unavailable")?
        .tx
        .send(command)
        .map_err(|_| "Input service stopped".into())
}

impl Input {
    fn edges(&mut self, allow_press: bool) {
        for i in 0..2 {
            let next = self.bindings[i]
                .as_ref()
                .is_some_and(|b| matches(b, &self.held));
            if next != self.active[i] {
                if next && allow_press {
                    let _ = self.output.send(Output::Press(
                        if i == 0 { "hold" } else { "toggle" },
                        Instant::now(),
                    ));
                }
                if !next && self.active[i] && i == 0 {
                    let _ = self.output.send(Output::Release(Instant::now()));
                }
                self.active[i] = next;
            }
        }
    }
    /// Tracks the cancel binding's match state and reports a rising edge while
    /// a dictation is armed. While hold-to-talk is held, its keys do not count
    /// as extra modifiers, so Esc cancels with Right Ctrl still down.
    fn cancel_edge(&mut self, allow_press: bool) -> bool {
        let Some(cancel) = &self.cancel else {
            self.cancel_active = false;
            return false;
        };
        let next = matches(cancel, &self.held)
            || (0..2).any(|i| {
                self.active[i]
                    && self.bindings[i].as_ref().is_some_and(|b| {
                        let rest: Vec<Key> = self
                            .held
                            .iter()
                            .filter(|k| !b.keys.contains(k))
                            .copied()
                            .collect();
                        matches(cancel, &rest)
                    })
            });
        let fired = next
            && !self.cancel_active
            && allow_press
            && ESCAPE_ARMED.load(std::sync::atomic::Ordering::Relaxed);
        self.cancel_active = next;
        if fired {
            let _ = self.output.send(Output::Cancel);
        }
        fired
    }
    fn cancel(&mut self, reason: &str) {
        if let Some(c) = self.capture.take() {
            self.inhibit = !self.held.is_empty();
            self.active = [false; 2];
            self.cancel_active = false;
            let _ = self
                .output
                .send(Output::Capture(c.reply, Err(reason.into())));
        }
    }
    // Called only from the message loop, AFTER the hook has returned. Suppressed
    // downs never enter Windows async state; retain those until their matching up,
    // unless a focus/session reset tells us we may have missed the release.
    fn resync(&mut self, reset: bool) {
        let mut physical = self.held.clone();
        for vk in [
            VK_LCONTROL,
            VK_RCONTROL,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_LMENU,
            VK_RMENU,
            VK_LWIN,
            VK_RWIN,
        ] {
            let key = Key::from_vk(vk);
            if !physical.contains(&key) {
                physical.push(key);
            }
        }
        physical.retain(|k| unsafe { GetAsyncKeyState(k.vk as i32) } < 0);
        self.reconcile(&physical, reset);
    }
    fn reconcile(&mut self, physical: &[Key], reset: bool) {
        self.held
            .retain(|k| physical.contains(k) || (!reset && self.swallowed.contains(k)));
        for key in physical.iter().filter(|k| k.modifier()) {
            if !self.held.contains(key) {
                self.held.push(*key);
            }
        }
        if self.inhibit && self.held.is_empty() {
            self.inhibit = false;
        }
        if !self.inhibit && self.capture.is_none() {
            self.cancel_edge(false);
            self.edges(false);
        }
    }
    fn event(&mut self, key: Key, down: bool) -> (bool, bool) {
        let (suppress, mask) = self.transition(key, down);
        if mask {
            self.win_mask_pending = true;
        }
        // Also mask immediately before Win-up. This covers chords where Win was
        // pressed LAST: injecting before forwarding Win-down would be too early.
        let release_mask = !down && matches!(key.vk, VK_LWIN | VK_RWIN) && self.win_mask_pending;
        if release_mask {
            self.win_mask_pending = false;
        }
        (suppress, mask || release_mask)
    }
    fn transition(&mut self, key: Key, down: bool) -> (bool, bool) {
        let repeated = down && self.held.contains(&key);
        if repeated {
            self.resync_requested = true;
        }
        if down {
            if !repeated && self.held.len() < 256 {
                self.held.push(key);
            }
        } else {
            self.held.retain(|k| *k != key);
        }
        let was_swallowed = self.swallowed.contains(&key);
        if !down {
            self.swallowed.retain(|k| *k != key);
        }
        if let Some(c) = &mut self.capture {
            if down && key.vk == VK_ESCAPE && c.escape_cancels {
                self.cancel("Capture cancelled");
            } else {
                if down && !c.keys.contains(&key) && c.keys.len() < 17 {
                    c.keys.push(key);
                    let _ = self.output.send(Output::CaptureKeys(c.keys.clone()));
                }
                if !down && !c.keys.is_empty() && c.keys.iter().all(|k| !self.held.contains(k)) {
                    let c = self.capture.take().unwrap();
                    let _ = self.output.send(Output::Capture(c.reply, Ok(c.keys)));
                }
            }
            // Pass modifiers through so Windows never receives an unmatched modifier up.
            let suppress = !key.modifier();
            if down && suppress && !was_swallowed {
                self.swallowed.push(key);
            }
            return (
                suppress || was_swallowed,
                down && suppress && self.held.iter().any(|k| matches!(k.vk, VK_LWIN | VK_RWIN)),
            );
        }
        if self.inhibit {
            if self.held.is_empty() {
                self.inhibit = false;
                self.active = [false; 2];
                self.cancel_active = false;
            }
            return (was_swallowed, false);
        }
        let previous = self.active;
        // An armed cancel wins this event: it does not also start hold or toggle.
        let cancelled = self.cancel_edge(true);
        self.edges(!cancelled);
        // An armed cancel always swallows the non-modifier down that completes it
        // (regardless of its suppress flag); repeats and the up pair through `swallowed`.
        let cancel_suppress = cancelled
            && down
            && !key.modifier()
            && self.cancel.as_ref().is_some_and(|b| b.keys.contains(&key));
        // Only swallow non-modifiers whose down completes an enabled combo. Remember
        // every swallowed down until its up, even after other combo keys are released.
        let suppress = cancel_suppress
            || (down
                && !key.modifier()
                && self.bindings.iter().flatten().any(|b| {
                    b.suppresses() && b.keys.contains(&key) && matches(b, &self.held)
                }));
        if suppress && !was_swallowed {
            self.swallowed.push(key);
        }
        let win_held = self.held.iter().any(|k| matches!(k.vk, VK_LWIN | VK_RWIN));
        (
            suppress || was_swallowed,
            win_held
                && (cancel_suppress
                    || (0..2).any(|i| {
                        self.active[i]
                            && !previous[i]
                            && self.bindings[i].as_ref().is_some_and(|b| b.suppresses())
                    })),
        )
    }
}

fn input_event(key: Key, down: bool) -> bool {
    let (suppress, mask_win) = INPUT.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map(|s| s.event(key, down))
            .unwrap_or_default()
    });
    if mask_win {
        // Outside the RefCell borrow: SendInput may re-enter our hooks. Injected events
        // are ignored. A harmless key pair marks Win as used without swallowing Win-up.
        unsafe {
            let mut inputs: [INPUT; 2] = std::mem::zeroed();
            for (i, input) in inputs.iter_mut().enumerate() {
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki.wVk = 0xff;
                input.Anonymous.ki.dwFlags = if i == 1 { KEYEVENTF_KEYUP } else { 0 };
            }
            SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        }
    }
    suppress
}
unsafe extern "system" fn keyboard(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = &*(l as *const KBDLLHOOKSTRUCT);
        if k.flags & LLKHF_INJECTED == 0 {
            let down = w as u32 == WM_KEYDOWN || w as u32 == WM_SYSKEYDOWN;
            let mut key = Key {
                scan: k.scanCode as u16,
                extended: k.flags & LLKHF_EXTENDED != 0,
                vk: k.vkCode as u16,
            };
            key.vk = match key.vk {
                VK_CONTROL => {
                    if key.extended {
                        VK_RCONTROL
                    } else {
                        VK_LCONTROL
                    }
                }
                VK_MENU => {
                    if key.extended {
                        VK_RMENU
                    } else {
                        VK_LMENU
                    }
                }
                VK_SHIFT => {
                    if key.scan == 0x36 {
                        VK_RSHIFT
                    } else {
                        VK_LSHIFT
                    }
                }
                other => other,
            };
            if input_event(key, down) {
                return 1;
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, w, l)
}
unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && matches!(w as u32, WM_XBUTTONDOWN | WM_XBUTTONUP) {
        let m = &*(l as *const MSLLHOOKSTRUCT);
        if m.flags & LLMHF_INJECTED == 0 {
            let vk = if m.mouseData >> 16 == XBUTTON1 as u32 {
                VK_XBUTTON1
            } else {
                VK_XBUTTON2
            };
            if input_event(
                Key {
                    scan: 0,
                    extended: false,
                    vk,
                },
                w as u32 == WM_XBUTTONDOWN,
            ) {
                return 1;
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, w, l)
}
unsafe extern "system" fn window(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE || msg == WM_POWERBROADCAST || msg == WM_ACTIVATEAPP {
        INPUT.with(|s| {
            if let Some(s) = s.borrow_mut().as_mut() {
                s.cancel("Capture interrupted by session change");
                s.resync(true);
            }
        });
    }
    DefWindowProcW(hwnd, msg, w, l)
}

pub fn start(settings: &crate::state::Settings, app: AppHandle) -> Result<(), String> {
    let mut guard = service().lock().map_err(|_| "Input service poisoned")?;
    let bindings = [
        settings.hold_binding.clone(),
        settings.toggle_binding.clone(),
        settings.cancel_binding.clone(),
    ];
    if let Some(s) = guard.as_ref() {
        return s
            .tx
            .send(Control::Configure(bindings))
            .map_err(|_| "Input service stopped".into());
    }
    let (tx, rx) = crossbeam_channel::unbounded();
    let (output, events) = crossbeam_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("chirp-input".into())
        .spawn(move || unsafe {
            let result = run(bindings, output, rx, ready_tx.clone());
            if let Err(e) = result {
                let _ = ready_tx.send(Err(e));
            }
        })
        .map_err(|e| e.to_string())?;
    if let Err(e) = ready_rx.recv().map_err(|e| e.to_string())? {
        let _ = thread.join();
        return Err(e);
    }
    let dispatch = std::thread::Builder::new()
        .name("chirp-input-events".into())
        .spawn(move || {
            app.state::<crate::state::SharedState>()
                .blocking_lock()
                .hotkey_status = crate::state::HotkeyStatus::Active;
            let _ = app.emit("hotkey-status", "active");
            while let Ok(event) = events.recv() {
                match event {
                    Output::Press(mode, at) => {
                        if crate::tray::is_paused() {
                            // Paused: say so instead of failing silently.
                            let _ = app.emit("hotkey-paused", mode);
                            continue;
                        }
                        crate::commands::note_hotkey_press(&app, at);
                        let _ = app.emit("hotkey-pressed", mode);
                        log::debug!("Input dispatch: {:?}", at.elapsed());
                    }
                    Output::CaptureKeys(keys) => {
                        let labels: Vec<String> = Binding::new(keys, false).keys.iter().map(|k| k.label()).collect();
                        let _ = app.emit("binding-capture-keys", labels);
                    }
                    Output::Cancel => {
                        let _ = app.emit("dictation-escape", ());
                    }
                    Output::Release(at) => {
                        if let Some(trace) = crate::performance::Trace::current() {
                            trace.input_at(true, at);
                        }
                        let _ = app.emit("hotkey-released", "hold");
                        log::debug!("Input release dispatch: {:?}", at.elapsed());
                    }
                    Output::Capture(reply, keys) => {
                        let _ = reply.send(keys);
                    }
                }
            }
        });
    if let Err(e) = dispatch {
        let _ = tx.send(Control::Stop);
        let _ = thread.join();
        return Err(e.to_string());
    }
    *guard = Some(Service { tx, thread });
    Ok(())
}
unsafe fn run(
    [hold, toggle, cancel]: [Option<Binding>; 3],
    output: Sender<Output>,
    rx: Receiver<Control>,
    ready: std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let instance = GetModuleHandleW(std::ptr::null());
    let name: Vec<u16> = "ChirpInputV2\0".encode_utf16().collect();
    let class = WNDCLASSW {
        lpfnWndProc: Some(window),
        hInstance: instance,
        lpszClassName: name.as_ptr(),
        ..std::mem::zeroed()
    };
    RegisterClassW(&class);
    // Hidden top-level window, not HWND_MESSAGE: power broadcasts exclude message-only windows.
    let hwnd = CreateWindowExW(
        0,
        name.as_ptr(),
        name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return Err(format!("Input window: {}", GetLastError()));
    }
    if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) == 0 {
        DestroyWindow(hwnd);
        return Err(format!("Session notifications: {}", GetLastError()));
    }
    INPUT.with(|s| {
        *s.borrow_mut() = Some(Input {
            bindings: [hold, toggle],
            cancel,
            held: Vec::with_capacity(256),
            swallowed: Vec::with_capacity(256),
            active: [false; 2],
            cancel_active: false,
            resync_requested: false,
            inhibit: false,
            win_mask_pending: false,
            capture: None,
            output,
            foreground: GetForegroundWindow() as isize,
        })
    });
    let keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), instance, 0);
    let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), instance, 0);
    let success = !keyboard_hook.is_null() && !mouse_hook.is_null();
    let error = GetLastError();
    if success {
        INPUT.with(|s| s.borrow_mut().as_mut().unwrap().resync(true));
    }
    let _ = ready.send(if success {
        Ok(())
    } else {
        Err(format!("Input hooks: {error}"))
    });
    let mut running = success;
    let mut last_resync = Instant::now();
    while running {
        // Wait with a timeout so commands and foreground changes are handled even while idle.
        MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 20, QS_ALLINPUT);
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_QUIT {
                running = false;
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        INPUT.with(|cell| {
            let mut state = cell.borrow_mut();
            let s = state.as_mut().unwrap();
            while let Ok(command) = rx.try_recv() {
                match command {
                    Control::Stop => {
                        running = false;
                    }
                    Control::Cancel => s.cancel("Capture cancelled"),
                    Control::Configure([hold, toggle, cancel]) => {
                        s.cancel("Bindings changed");
                        if s.active[0] {
                            let _ = s.output.send(Output::Release(Instant::now()));
                        }
                        s.inhibit = !s.held.is_empty();
                        s.bindings = [hold, toggle];
                        s.cancel = cancel;
                        s.active = [false; 2];
                        s.cancel_active = false;
                        s.resync(true);
                    }
                    Control::Capture(reply, escape_cancels) => {
                        s.resync(true);
                        if s.capture.is_some() {
                            let _ = s.output.send(Output::Capture(
                                reply,
                                Err("Another binding capture is active".into()),
                            ));
                        } else if s.active.iter().any(|v| *v) {
                            let _ = s.output.send(Output::Capture(
                                reply,
                                Err("Release the current hotkey first".into()),
                            ));
                        } else {
                            s.resync(true);
                            s.capture = Some(Capture {
                                reply,
                                keys: Vec::with_capacity(17),
                                deadline: Instant::now() + Duration::from_secs(15),
                                escape_cancels,
                            });
                        }
                    }
                }
            }
            if s.resync_requested || last_resync.elapsed() >= Duration::from_millis(250) {
                last_resync = Instant::now();
                s.resync_requested = false;
                s.resync(false);
            }
            let foreground = GetForegroundWindow() as isize;
            if foreground != s.foreground {
                s.foreground = foreground;
                s.resync(true);
            }
            if s.capture
                .as_ref()
                .is_some_and(|c| c.deadline <= Instant::now() || c.reply.is_closed())
            {
                s.cancel("Capture timed out or cancelled");
            }
        });
    }
    UnhookWindowsHookEx(keyboard_hook);
    UnhookWindowsHookEx(mouse_hook);
    WTSUnRegisterSessionNotification(hwnd);
    DestroyWindow(hwnd);
    INPUT.with(|s| {
        if let Some(mut s) = s.borrow_mut().take() {
            s.cancel("Input service stopped");
        }
    });
    Ok(())
}
pub fn stop() {
    if let Ok(mut guard) = service().lock() {
        if let Some(s) = guard.take() {
            let _ = s.tx.send(Control::Stop);
            let _ = s.thread.join();
        }
    }
}
pub fn update(settings: &crate::state::Settings, app: AppHandle) -> Result<(), String> {
    start(settings, app)
}
/// Captures a binding and validates it against `others`. With `escape_cancels`
/// (hold / toggle), Esc ends the capture; without it (cancel), Esc is recorded
/// as a key and the capture ends by button, timeout or session change.
pub async fn record_binding(
    others: &[Option<Binding>],
    escape_cancels: bool,
) -> Result<Binding, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    send(Control::Capture(tx, escape_cancels))?;
    let keys = tokio::time::timeout(Duration::from_secs(17), rx)
        .await
        .map_err(|_| "Capture timed out")?
        .map_err(|_| "Input service stopped")??;
    let binding = Binding::new(keys, true);
    conflict(&binding, others)?;
    Ok(binding)
}
pub fn cancel_record_binding() -> Result<(), String> {
    send(Control::Cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine(hold: &[u16], toggle: &[u16]) -> (Input, Receiver<Output>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let binding = |vks: &[u16]| {
            if vks.is_empty() {
                None
            } else {
                Some(Binding::new(
                    vks.iter().copied().map(Key::from_vk).collect(),
                    true,
                ))
            }
        };
        (
            Input {
                bindings: [binding(hold), binding(toggle)],
                cancel: None,
                held: vec![],
                swallowed: vec![],
                active: [false; 2],
                cancel_active: false,
                resync_requested: false,
                inhibit: false,
                win_mask_pending: false,
                capture: None,
                output: tx,
                foreground: 0,
            },
            rx,
        )
    }
    #[test]
    fn repeat_fires_once_and_release_fires_once() {
        let (mut s, rx) = engine(&[VK_F13], &[]);
        let k = Key::from_vk(VK_F13);
        assert!(s.event(k, true).0);
        assert!(s.event(k, true).0);
        assert!(s.event(k, false).0);
        assert!(!s.event(k, false).0);
        assert!(matches!(rx.try_recv(), Ok(Output::Press("hold", _))));
        assert!(matches!(rx.try_recv(), Ok(Output::Release(_))));
        assert!(rx.is_empty());
    }
    #[test]
    fn toggle_has_press_edges_but_no_release_actions() {
        let (mut s, rx) = engine(&[], &[VK_F24]);
        let k = Key::from_vk(VK_F24);
        for _ in 0..2 {
            s.event(k, true);
            s.event(k, true);
            s.event(k, false);
        }
        assert!(matches!(rx.try_recv(), Ok(Output::Press("toggle", _))));
        assert!(matches!(rx.try_recv(), Ok(Output::Press("toggle", _))));
        assert!(rx.is_empty());
    }
    #[test]
    fn swallowed_up_is_paired_after_modifier_is_released_first() {
        let (mut s, _) = engine(&[VK_LCONTROL, VK_SPACE], &[]);
        let ctrl = Key::from_vk(VK_LCONTROL);
        let space = Key::from_vk(VK_SPACE);
        assert!(!s.event(ctrl, true).0);
        assert!(s.event(space, true).0);
        assert!(!s.event(ctrl, false).0);
        assert!(s.event(space, false).0);
        assert!(s.swallowed.is_empty());
    }
    #[test]
    fn modifier_only_does_not_suppress_and_win_combo_requests_mask() {
        let (mut s, _) = engine(&[VK_LWIN, VK_SPACE], &[]);
        assert_eq!(s.event(Key::from_vk(VK_LWIN), true), (false, false));
        assert_eq!(s.event(Key::from_vk(VK_SPACE), true), (true, true));
        let (mut s, _) = engine(&[VK_RCONTROL], &[]);
        assert_eq!(s.event(Key::from_vk(VK_RCONTROL), true), (false, false));
    }
    #[test]
    fn capture_completes_only_after_all_recorded_keys_are_up() {
        let (mut s, rx) = engine(&[VK_RCONTROL], &[]);
        let (tx, _) = tokio::sync::oneshot::channel();
        s.capture = Some(Capture {
            reply: tx,
            keys: vec![],
            deadline: Instant::now() + Duration::from_secs(15),
            escape_cancels: true,
        });
        let ctrl = Key::from_vk(VK_RCONTROL);
        let key = Key::from_vk(VK_F24);
        s.event(ctrl, true);
        s.event(key, true);
        s.event(ctrl, false);
        let progress: Vec<usize> = rx
            .try_iter()
            .map(|o| match o {
                Output::CaptureKeys(keys) => keys.len(),
                _ => panic!("capture must not complete while a recorded key is down"),
            })
            .collect();
        assert_eq!(progress, vec![1, 2]);
        s.event(key, false);
        match rx.try_recv().unwrap() {
            Output::Capture(_, Ok(keys)) => assert_eq!(keys, vec![ctrl, key]),
            _ => panic!("expected capture"),
        }
        assert!(rx.is_empty());
    }
    #[test]
    fn escape_cancels_without_firing_underlying_binding() {
        let (mut s, rx) = engine(&[VK_RCONTROL], &[]);
        let (tx, _) = tokio::sync::oneshot::channel();
        s.capture = Some(Capture {
            reply: tx,
            keys: vec![],
            deadline: Instant::now() + Duration::from_secs(15),
            escape_cancels: true,
        });
        let ctrl = Key::from_vk(VK_RCONTROL);
        let esc = Key::from_vk(VK_ESCAPE);
        s.event(ctrl, true);
        s.event(esc, true);
        s.event(esc, false);
        s.event(ctrl, false);
        assert!(matches!(rx.try_recv(), Ok(Output::CaptureKeys(_))));
        assert!(matches!(rx.try_recv(), Ok(Output::Capture(_, Err(_)))));
        assert!(rx.is_empty());
        s.event(ctrl, true);
        assert!(matches!(rx.try_recv(), Ok(Output::Press("hold", _))));
    }
    #[test]
    fn escape_is_a_key_when_capturing_the_cancel_binding() {
        let (mut s, rx) = engine(&[VK_RCONTROL], &[]);
        let (tx, _) = tokio::sync::oneshot::channel();
        s.capture = Some(Capture {
            reply: tx,
            keys: vec![],
            deadline: Instant::now() + Duration::from_secs(15),
            escape_cancels: false,
        });
        let esc = Key::from_vk(VK_ESCAPE);
        assert!(s.event(esc, true).0, "Esc is swallowed while capturing");
        assert!(s.event(esc, false).0);
        assert!(matches!(rx.try_recv(), Ok(Output::CaptureKeys(_))));
        match rx.try_recv().unwrap() {
            Output::Capture(_, Ok(keys)) => assert_eq!(keys, vec![esc]),
            _ => panic!("expected Esc to be captured"),
        }
        assert!(rx.is_empty());
    }

    /// ESCAPE_ARMED is process-wide; serialise the tests that set it.
    static ARMED: Mutex<()> = Mutex::new(());
    fn arm(on: bool) {
        ESCAPE_ARMED.store(on, std::sync::atomic::Ordering::SeqCst);
    }
    fn with_cancel(hold: &[u16], cancel: Option<&[u16]>) -> (Input, Receiver<Output>) {
        let (mut s, rx) = engine(hold, &[]);
        s.cancel = cancel.map(|vks| Binding::new(vks.iter().copied().map(Key::from_vk).collect(), true));
        (s, rx)
    }
    #[test]
    fn armed_escape_cancels_dictation_and_is_swallowed_with_its_release() {
        let _guard = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let (mut s, rx) = engine(&[VK_RCONTROL], &[]);
        s.cancel = crate::hotkey::default_cancel();
        let esc = Key::from_vk(VK_ESCAPE);
        arm(true);
        assert!(s.event(esc, true).0);
        assert!(s.event(esc, true).0);
        arm(false);
        assert!(s.event(esc, false).0, "the release pairs with the swallowed press");
        assert!(matches!(rx.try_recv(), Ok(Output::Cancel)));
        assert!(rx.is_empty());
        assert!(!s.event(esc, true).0, "unarmed Esc passes through");
        assert!(!s.event(esc, false).0);
        assert!(rx.is_empty());
    }
    #[test]
    fn escape_cancels_while_hold_to_talk_is_held() {
        let _guard = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let (mut s, rx) = with_cancel(&[VK_RCONTROL], Some(&[VK_ESCAPE]));
        let ctrl = Key::from_vk(VK_RCONTROL);
        let esc = Key::from_vk(VK_ESCAPE);
        s.event(ctrl, true);
        assert!(matches!(rx.try_recv(), Ok(Output::Press("hold", _))));
        arm(true);
        assert!(s.event(esc, true).0);
        arm(false);
        assert!(s.event(esc, false).0);
        assert!(matches!(rx.try_recv(), Ok(Output::Cancel)));
        assert!(rx.is_empty(), "no extra hold edges");
        s.event(ctrl, false);
        assert!(matches!(rx.try_recv(), Ok(Output::Release(_))));
    }
    #[test]
    fn rebound_cancel_key_fires_and_escape_no_longer_does() {
        let _guard = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let (mut s, rx) = with_cancel(&[VK_RCONTROL], Some(&[VK_F8]));
        let f8 = Key::from_vk(VK_F8);
        let esc = Key::from_vk(VK_ESCAPE);
        arm(true);
        assert!(!s.event(esc, true).0, "Esc is just a key now");
        assert!(!s.event(esc, false).0);
        assert!(rx.is_empty());
        assert!(s.event(f8, true).0);
        assert!(s.event(f8, true).0, "repeats stay swallowed");
        assert!(s.event(f8, false).0);
        arm(false);
        assert!(matches!(rx.try_recv(), Ok(Output::Cancel)));
        assert!(rx.is_empty());
        assert!(s.swallowed.is_empty());
    }
    #[test]
    fn rebound_cancel_combo_requires_its_modifiers_and_no_others() {
        let _guard = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let (mut s, rx) = with_cancel(&[VK_RCONTROL], Some(&[VK_LCONTROL, 0x51]));
        let ctrl = Key::from_vk(VK_LCONTROL);
        let shift = Key::from_vk(VK_LSHIFT);
        let q = Key::from_vk(0x51);
        arm(true);
        // Q alone does nothing.
        assert!(!s.event(q, true).0);
        assert!(!s.event(q, false).0);
        // Shift + Ctrl + Q: extra modifier, no match.
        s.event(shift, true);
        s.event(ctrl, true);
        assert!(!s.event(q, true).0);
        assert!(!s.event(q, false).0);
        s.event(shift, false);
        s.event(ctrl, false);
        assert!(rx.is_empty());
        // Ctrl + Q: Ctrl passes through, Q is swallowed with its release.
        arm(true);
        assert!(!s.event(ctrl, true).0);
        assert!(s.event(q, true).0);
        arm(false);
        assert!(!s.event(ctrl, false).0);
        assert!(s.event(q, false).0);
        assert!(matches!(rx.try_recv(), Ok(Output::Cancel)));
        assert!(rx.is_empty());
    }
    #[test]
    fn unarmed_or_disabled_cancel_passes_through() {
        let _guard = ARMED.lock().unwrap_or_else(|e| e.into_inner());
        let (mut s, rx) = with_cancel(&[VK_RCONTROL], Some(&[VK_F8]));
        let f8 = Key::from_vk(VK_F8);
        arm(false);
        assert!(!s.event(f8, true).0);
        // Arming while the key is already down does not manufacture a press.
        arm(true);
        assert!(!s.event(f8, true).0);
        assert!(!s.event(f8, false).0);
        arm(false);
        assert!(rx.is_empty());
        let (mut s, rx) = with_cancel(&[VK_RCONTROL], None);
        let esc = Key::from_vk(VK_ESCAPE);
        arm(true);
        assert!(!s.event(esc, true).0, "no cancel binding: Esc passes through");
        assert!(!s.event(esc, false).0);
        arm(false);
        assert!(rx.is_empty());
    }
}

#[cfg(test)]
mod resync_tests {
    use super::*;
    #[test]
    fn lost_release_stops_hold_and_does_not_autostart_on_resume() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let key = Key::from_vk(VK_RCONTROL);
        let mut s = Input {
            bindings: [Some(Binding::new(vec![key], false)), None],
            cancel: None,
            held: vec![],
            swallowed: vec![],
            active: [false; 2],
            cancel_active: false,
            resync_requested: false,
            inhibit: false,
            win_mask_pending: false,
            capture: None,
            output: tx,
            foreground: 0,
        };
        s.event(key, true);
        rx.try_recv().unwrap();
        s.reconcile(&[], true);
        assert!(matches!(rx.try_recv(), Ok(Output::Release(_))));
        s.reconcile(&[key], true);
        assert!(rx.is_empty());
        s.event(key, false);
        rx.try_recv().unwrap();
        s.event(key, true);
        assert!(matches!(rx.try_recv(), Ok(Output::Press("hold", _))));
    }
    #[test]
    fn suppressed_down_survives_normal_poll_but_not_session_reset() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let key = Key::from_vk(VK_F13);
        let mut s = Input {
            bindings: [Some(Binding::new(vec![key], true)), None],
            cancel: None,
            held: vec![],
            swallowed: vec![],
            active: [false; 2],
            cancel_active: false,
            resync_requested: false,
            inhibit: false,
            win_mask_pending: false,
            capture: None,
            output: tx,
            foreground: 0,
        };
        s.event(key, true);
        rx.try_recv().unwrap();
        s.reconcile(&[], false);
        assert!(rx.is_empty());
        assert!(s.active[0]);
        s.reconcile(&[], true);
        assert!(matches!(rx.try_recv(), Ok(Output::Release(_))));
        assert!(s.event(key, false).0); // still consume the orphaned up if it arrives
    }
}

#[cfg(test)]
mod win_release_tests {
    use super::*;
    #[test]
    fn win_pressed_last_is_masked_before_its_release() {
        let (tx, _) = crossbeam_channel::unbounded();
        let win = Key::from_vk(VK_LWIN);
        let space = Key::from_vk(VK_SPACE);
        let mut s = Input {
            bindings: [Some(Binding::new(vec![win, space], true)), None],
            cancel: None,
            held: vec![],
            swallowed: vec![],
            active: [false; 2],
            cancel_active: false,
            resync_requested: false,
            inhibit: false,
            win_mask_pending: false,
            capture: None,
            output: tx,
            foreground: 0,
        };
        assert_eq!(s.event(space, true), (false, false));
        assert_eq!(s.event(win, true), (false, true));
        s.event(space, false);
        assert_eq!(s.event(win, false), (false, true));
        assert!(!s.win_mask_pending);
    }
}
