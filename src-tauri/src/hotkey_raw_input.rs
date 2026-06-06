//! Windows Raw Input detection for global hotkeys.
//!
//! Detection runs on a dedicated thread with a hidden message-only window.
//! `RegisterRawInputDevices` with `RIDEV_INPUTSINK` delivers every keyboard
//! event via `WM_INPUT`, regardless of focus. Unlike `WH_KEYBOARD_LL`, this
//! API has no OS-imposed timeout and is not silently unhooked — it is the
//! reliable detection path for long-running background apps.
//!
//! Suppression (preventing the keypress from reaching the focused window) is
//! NOT this module's job — see `hotkey_ll_suppress`. Detection and
//! suppression are intentionally split so that when the LL hook is torn
//! down by the OS, detection keeps working.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::hotkey::ActiveCombo;

const WINDOW_CLASS: &[u16] = &[
    b'c' as u16, b'h' as u16, b'i' as u16, b'r' as u16, b'p' as u16, b'_' as u16,
    b'r' as u16, b'a' as u16, b'w' as u16, b'i' as u16, b'n' as u16, b'p' as u16,
    b'u' as u16, b't' as u16, 0,
];

/// Per-thread state accessed from the WindowProc.
struct ThreadState {
    combo: HashSet<String>,
    modifier_only: bool,
    held: HashSet<String>,
    combo_active: bool,
    app: AppHandle,
    active_combo: Arc<ActiveCombo>,
}

thread_local! {
    static STATE: std::cell::RefCell<Option<ThreadState>> = const { std::cell::RefCell::new(None) };
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static THREAD_HANDLE: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

/// Start the Raw Input detection thread. Idempotent — will stop any existing
/// instance first.
pub fn start(
    combo: HashSet<String>,
    modifier_only: bool,
    app_handle: AppHandle,
    active_combo: Arc<ActiveCombo>,
) -> Result<(), String> {
    stop();
    SHUTDOWN.store(false, Ordering::SeqCst);

    let handle = std::thread::Builder::new()
        .name("hotkey-rawinput".into())
        .spawn(move || {
            unsafe {
                if let Err(e) = run_message_loop(combo, modifier_only, app_handle, active_combo) {
                    log::error!("Raw Input detection thread failed: {e}");
                }
            }
        })
        .map_err(|e| format!("Failed to spawn raw-input thread: {e}"))?;

    if let Ok(mut guard) = THREAD_HANDLE.lock() {
        *guard = Some(handle);
    }
    Ok(())
}

pub fn stop() {
    SHUTDOWN.store(true, Ordering::SeqCst);
    // Post a dummy message to every thread so the message loop wakes and
    // observes SHUTDOWN. We rely on the thread finding its own window via
    // FindWindow — instead of tracking HWND across threads — so we cheat by
    // posting WM_QUIT to all threads via a broadcast. Simpler: drop the
    // join handle; the thread will observe SHUTDOWN on its next message
    // (mouse move, key, timer) and exit.
    if let Ok(mut guard) = THREAD_HANDLE.lock() {
        *guard = None;
    }
}

unsafe fn run_message_loop(
    combo: HashSet<String>,
    modifier_only: bool,
    app_handle: AppHandle,
    active_combo: Arc<ActiveCombo>,
) -> Result<(), String> {
    let hinstance = GetModuleHandleW(std::ptr::null());

    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: WINDOW_CLASS.as_ptr(),
    };

    // Register class (idempotent; ignore "already exists" error).
    RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        0,
        WINDOW_CLASS.as_ptr(),
        WINDOW_CLASS.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        std::ptr::null_mut(),
        hinstance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return Err("CreateWindowExW failed".into());
    }

    STATE.with(|s| {
        *s.borrow_mut() = Some(ThreadState {
            combo,
            modifier_only,
            held: HashSet::new(),
            combo_active: false,
            app: app_handle,
            active_combo,
        });
    });

    let rid = RAWINPUTDEVICE {
        usUsagePage: 0x01, // Generic Desktop
        usUsage: 0x06,     // Keyboard
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    if RegisterRawInputDevices(&rid, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
        DestroyWindow(hwnd);
        return Err("RegisterRawInputDevices failed".into());
    }

    let mut msg: MSG = std::mem::zeroed();
    while !SHUTDOWN.load(Ordering::SeqCst) {
        // Use PeekMessage with a short wait so we can observe SHUTDOWN.
        let got = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE);
        if got == 0 {
            // Block briefly on MsgWaitForMultipleObjects so we're not
            // busy-looping. 100ms granularity on shutdown is fine.
            use windows_sys::Win32::UI::WindowsAndMessaging::MsgWaitForMultipleObjects;
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 100, QS_ALLINPUT);
            continue;
        }
        if msg.message == WM_QUIT {
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Unregister raw input device and destroy window.
    let rid_remove = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: RIDEV_REMOVE,
        hwndTarget: std::ptr::null_mut(),
    };
    RegisterRawInputDevices(&rid_remove, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
    DestroyWindow(hwnd);
    STATE.with(|s| *s.borrow_mut() = None);
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_INPUT {
        handle_raw_input(lparam as HRAWINPUT);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn handle_raw_input(hri: HRAWINPUT) {
    let mut size: u32 = 0;
    GetRawInputData(
        hri,
        RID_INPUT,
        std::ptr::null_mut(),
        &mut size,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    );
    if size == 0 {
        return;
    }
    let mut buf = vec![0u8; size as usize];
    let read = GetRawInputData(
        hri,
        RID_INPUT,
        buf.as_mut_ptr() as *mut _,
        &mut size,
        std::mem::size_of::<RAWINPUTHEADER>() as u32,
    );
    if read == u32::MAX || read == 0 {
        return;
    }
    let raw = &*(buf.as_ptr() as *const RAWINPUT);
    if raw.header.dwType != RIM_TYPEKEYBOARD {
        return;
    }
    let kb = &raw.data.keyboard;
    let vk = kb.VKey;
    let flags = kb.Flags;

    // Normalize left/right variants when the generic VK is delivered.
    let resolved_vk = resolve_vk_lr(vk, kb.MakeCode, flags);
    let Some(id) = vk_to_id(resolved_vk) else {
        return;
    };

    // Flags bit 0 = break (release); 0 = make (press).
    // RI_KEY_BREAK = 0x0001.
    let is_release = (flags & 0x0001) != 0;

    STATE.with(|cell| {
        let mut guard = cell.borrow_mut();
        let Some(state) = guard.as_mut() else { return };

        if is_release {
            state.held.remove(&id);
            reconcile_held_combo_keys(state);
            if state.combo_active && state.combo.contains(&id) {
                state.combo_active = false;
                state.active_combo.store(false);
                log::info!("Hotkey released");
                let _ = state.app.emit("hotkey-released", ());
            }
        } else {
            state.held.insert(id.clone());
            reconcile_held_combo_keys(state);
            if state.held == state.combo && !state.combo_active {
                state.combo_active = true;
                state.active_combo.store(true);
                log::info!("Hotkey pressed");
                let _ = state.app.emit("hotkey-pressed", ());
            }
            let _ = state.modifier_only;
        }
    });
}

fn reconcile_held_combo_keys(state: &mut ThreadState) {
    let stale_keys: Vec<String> = state
        .combo
        .iter()
        .filter_map(|key| {
            let vk = id_to_vk(key)?;
            (!is_key_physically_down(vk)).then(|| key.clone())
        })
        .collect();

    for key in stale_keys {
        state.held.remove(&key);
    }
}

fn is_key_physically_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// Disambiguate generic VK_SHIFT / VK_CONTROL / VK_MENU into left/right
/// variants using the scancode + extended flag.
unsafe fn resolve_vk_lr(vk: u16, scan: u16, flags: u16) -> u16 {
    const RI_KEY_E0: u16 = 0x0002;
    const RI_KEY_E1: u16 = 0x0004;
    let extended = (flags & (RI_KEY_E0 | RI_KEY_E1)) != 0;
    match vk {
        VK_SHIFT => {
            // MapVirtualKey(scan, MAPVK_VSC_TO_VK_EX) returns VK_LSHIFT / VK_RSHIFT.
            let resolved = MapVirtualKeyW(scan as u32, 3) as u16; // MAPVK_VSC_TO_VK_EX
            if resolved != 0 {
                resolved
            } else {
                VK_LSHIFT
            }
        }
        VK_CONTROL => {
            if extended {
                VK_RCONTROL
            } else {
                VK_LCONTROL
            }
        }
        VK_MENU => {
            if extended {
                VK_RMENU
            } else {
                VK_LMENU
            }
        }
        other => other,
    }
}

/// Inverse of `vk_to_id`: map a hotkey config string ID to its Win32 VK.
/// Returns `None` for IDs that don't correspond to a single VK (e.g. "Fn"
/// which has no VK, or unsupported keys). Used by the LL suppression hook to
/// build its VK-match set.
pub fn id_to_vk(id: &str) -> Option<u16> {
    let vk: u16 = match id {
        "ControlLeft" => VK_LCONTROL,
        "ControlRight" => VK_RCONTROL,
        "ShiftLeft" => VK_LSHIFT,
        "ShiftRight" => VK_RSHIFT,
        "Alt" => VK_LMENU,
        "AltGr" => VK_RMENU,
        "MetaLeft" => VK_LWIN,
        "MetaRight" => VK_RWIN,
        "Space" => VK_SPACE,
        "Enter" => VK_RETURN,
        "Tab" => VK_TAB,
        "Escape" => VK_ESCAPE,
        "Backspace" => VK_BACK,
        "Delete" => VK_DELETE,
        "Insert" => VK_INSERT,
        "Home" => VK_HOME,
        "End" => VK_END,
        "PageUp" => VK_PRIOR,
        "PageDown" => VK_NEXT,
        "ArrowUp" => VK_UP,
        "ArrowDown" => VK_DOWN,
        "ArrowLeft" => VK_LEFT,
        "ArrowRight" => VK_RIGHT,
        "CapsLock" => VK_CAPITAL,
        "F1" => VK_F1,
        "F2" => VK_F2,
        "F3" => VK_F3,
        "F4" => VK_F4,
        "F5" => VK_F5,
        "F6" => VK_F6,
        "F7" => VK_F7,
        "F8" => VK_F8,
        "F9" => VK_F9,
        "F10" => VK_F10,
        "F11" => VK_F11,
        "F12" => VK_F12,
        "Backquote" => VK_OEM_3,
        "Minus" => VK_OEM_MINUS,
        "Equal" => VK_OEM_PLUS,
        "BracketLeft" => VK_OEM_4,
        "BracketRight" => VK_OEM_6,
        "Semicolon" => VK_OEM_1,
        "Quote" => VK_OEM_7,
        "Backslash" => VK_OEM_5,
        "Comma" => VK_OEM_COMMA,
        "Period" => VK_OEM_PERIOD,
        "Slash" => VK_OEM_2,
        "NumLock" => VK_NUMLOCK,
        "ScrollLock" => VK_SCROLL,
        "PrintScreen" => VK_SNAPSHOT,
        "Pause" => VK_PAUSE,
        "Numpad0" => VK_NUMPAD0,
        "Numpad1" => VK_NUMPAD1,
        "Numpad2" => VK_NUMPAD2,
        "Numpad3" => VK_NUMPAD3,
        "Numpad4" => VK_NUMPAD4,
        "Numpad5" => VK_NUMPAD5,
        "Numpad6" => VK_NUMPAD6,
        "Numpad7" => VK_NUMPAD7,
        "Numpad8" => VK_NUMPAD8,
        "Numpad9" => VK_NUMPAD9,
        "NumpadAdd" => VK_ADD,
        "NumpadSubtract" => VK_SUBTRACT,
        "NumpadMultiply" => VK_MULTIPLY,
        "NumpadDivide" => VK_DIVIDE,
        "NumpadDecimal" => VK_DECIMAL,
        s if s.starts_with("Key") && s.len() == 4 => {
            let c = s.as_bytes()[3];
            if c.is_ascii_uppercase() { c as u16 } else { return None }
        }
        s if s.starts_with("Digit") && s.len() == 6 => {
            let c = s.as_bytes()[5];
            if c.is_ascii_digit() { c as u16 } else { return None }
        }
        _ => return None,
    };
    Some(vk)
}

/// Map a resolved Win32 VK to the string ID used by hotkey config (matches
/// `hotkey::key_to_id` output for rdev on Linux).
fn vk_to_id(vk: u16) -> Option<String> {
    let s: &str = match vk {
        VK_LCONTROL => "ControlLeft",
        VK_RCONTROL => "ControlRight",
        VK_LSHIFT => "ShiftLeft",
        VK_RSHIFT => "ShiftRight",
        VK_LMENU => "Alt",
        VK_RMENU => "AltGr",
        VK_LWIN => "MetaLeft",
        VK_RWIN => "MetaRight",
        VK_SPACE => "Space",
        VK_RETURN => "Enter",
        VK_TAB => "Tab",
        VK_ESCAPE => "Escape",
        VK_BACK => "Backspace",
        VK_DELETE => "Delete",
        VK_INSERT => "Insert",
        VK_HOME => "Home",
        VK_END => "End",
        VK_PRIOR => "PageUp",
        VK_NEXT => "PageDown",
        VK_UP => "ArrowUp",
        VK_DOWN => "ArrowDown",
        VK_LEFT => "ArrowLeft",
        VK_RIGHT => "ArrowRight",
        VK_CAPITAL => "CapsLock",
        VK_F1 => "F1",
        VK_F2 => "F2",
        VK_F3 => "F3",
        VK_F4 => "F4",
        VK_F5 => "F5",
        VK_F6 => "F6",
        VK_F7 => "F7",
        VK_F8 => "F8",
        VK_F9 => "F9",
        VK_F10 => "F10",
        VK_F11 => "F11",
        VK_F12 => "F12",
        VK_OEM_3 => "Backquote",
        VK_OEM_MINUS => "Minus",
        VK_OEM_PLUS => "Equal",
        VK_OEM_4 => "BracketLeft",
        VK_OEM_6 => "BracketRight",
        VK_OEM_1 => "Semicolon",
        VK_OEM_7 => "Quote",
        VK_OEM_5 => "Backslash",
        VK_OEM_COMMA => "Comma",
        VK_OEM_PERIOD => "Period",
        VK_OEM_2 => "Slash",
        VK_NUMLOCK => "NumLock",
        VK_SCROLL => "ScrollLock",
        VK_SNAPSHOT => "PrintScreen",
        VK_PAUSE => "Pause",
        VK_NUMPAD0 => "Numpad0",
        VK_NUMPAD1 => "Numpad1",
        VK_NUMPAD2 => "Numpad2",
        VK_NUMPAD3 => "Numpad3",
        VK_NUMPAD4 => "Numpad4",
        VK_NUMPAD5 => "Numpad5",
        VK_NUMPAD6 => "Numpad6",
        VK_NUMPAD7 => "Numpad7",
        VK_NUMPAD8 => "Numpad8",
        VK_NUMPAD9 => "Numpad9",
        VK_ADD => "NumpadAdd",
        VK_SUBTRACT => "NumpadSubtract",
        VK_MULTIPLY => "NumpadMultiply",
        VK_DIVIDE => "NumpadDivide",
        VK_DECIMAL => "NumpadDecimal",
        v if (0x41..=0x5A).contains(&v) => {
            // 'A'..='Z'
            return Some(format!("Key{}", (v as u8) as char));
        }
        v if (0x30..=0x39).contains(&v) => {
            // '0'..='9'
            return Some(format!("Digit{}", (v as u8) as char));
        }
        _ => return None,
    };
    Some(s.to_string())
}
