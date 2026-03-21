use crate::state::SharedState;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use tauri::{AppHandle, Emitter};

// Known modifier keycodes on macOS
const MODIFIER_KEYCODES: &[i64] = &[
    54, // Right Command
    55, // Left Command
    56, // Left Shift
    57, // Caps Lock
    58, // Left Option
    59, // Left Control
    60, // Right Shift
    61, // Right Option
    62, // Right Control
    63, // Fn
];

/// Returns the modifier flag bit for a given keycode, or 0 if not a modifier
fn modifier_flag_for_keycode(keycode: i64) -> u64 {
    match keycode {
        54 | 55 => 0x00100000, // Command
        56 | 60 => 0x00020000, // Shift
        57 => 0x00010000,      // Caps Lock
        58 | 61 => 0x00080000, // Option/Alt
        59 | 62 => 0x00040000, // Control
        63 => 0x00800000,      // Fn
        _ => 0,
    }
}

/// Returns a human-readable name for a macOS keycode
pub fn keycode_name(keycode: i64) -> String {
    match keycode {
        0 => "A", 1 => "S", 2 => "D", 3 => "F", 4 => "H", 5 => "G",
        6 => "Z", 7 => "X", 8 => "C", 9 => "V", 11 => "B", 12 => "Q",
        13 => "W", 14 => "E", 15 => "R", 16 => "Y", 17 => "T",
        18 => "1", 19 => "2", 20 => "3", 21 => "4", 22 => "6", 23 => "5",
        24 => "=", 25 => "9", 26 => "7", 27 => "-", 28 => "8", 29 => "0",
        30 => "]", 31 => "O", 32 => "U", 33 => "[", 34 => "I", 35 => "P",
        36 => "Return", 37 => "L", 38 => "J", 39 => "'", 40 => "K",
        41 => ";", 42 => "\\", 43 => ",", 44 => "/", 45 => "N", 46 => "M",
        47 => ".", 48 => "Tab", 49 => "Space", 50 => "`",
        51 => "Delete", 53 => "Escape",
        54 => "Right Cmd", 55 => "Left Cmd",
        56 => "Left Shift", 57 => "Caps Lock",
        58 => "Left Option", 59 => "Left Ctrl",
        60 => "Right Shift", 61 => "Right Option",
        62 => "Right Ctrl", 63 => "Fn",
        96 => "F5", 97 => "F6", 98 => "F7", 99 => "F3",
        100 => "F8", 101 => "F9", 103 => "F11", 105 => "F13",
        107 => "F14", 109 => "F10", 111 => "F12", 113 => "F15",
        118 => "F4", 120 => "F2", 122 => "F1",
        123 => "Left", 124 => "Right", 125 => "Down", 126 => "Up",
        _ => "Unknown",
    }
    .into()
}

// Raw CoreGraphics FFI
mod cg {
    use std::ffi::c_void;

    pub type CGEventRef = *const c_void;
    pub type CGEventTapProxy = *const c_void;
    pub type CFMachPortRef = *const c_void;
    pub type CFRunLoopSourceRef = *const c_void;
    pub type CFRunLoopRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CGEventMask = u64;
    pub type CGEventType = u32;
    pub type CGEventField = u32;

    pub const K_CG_EVENT_KEY_DOWN: CGEventType = 10;
    pub const K_CG_EVENT_KEY_UP: CGEventType = 11;
    pub const K_CG_EVENT_FLAGS_CHANGED: CGEventType = 12;
    pub const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = 0xFFFFFFFE;
    pub const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: CGEventType = 0xFFFFFFFF;
    pub const K_CG_KEYBOARD_EVENT_KEYCODE: CGEventField = 9;

    pub type CGEventTapCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    extern "C" {
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: CGEventMask,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;

        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> u64;
        pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

        pub fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: i64,
        ) -> CFRunLoopSourceRef;

        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(
            rl: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        pub fn CFRunLoopRun();
        pub fn CFRunLoopStop(rl: CFRunLoopRef);

        pub static kCFRunLoopDefaultMode: CFStringRef;
    }

    // Input Monitoring permission checks (correct for kCGEventTapOptionListenOnly)
    extern "C" {
        pub fn CGPreflightListenEventAccess() -> bool;
        pub fn CGRequestListenEventAccess() -> bool;
    }
}

/// Wrapper to allow storing CFMachPortRef (raw pointer) in a Mutex across threads
struct SendableTap(cg::CFMachPortRef);
unsafe impl Send for SendableTap {}
unsafe impl Sync for SendableTap {}

/// Wrapper to allow storing CFRunLoopRef across threads for stop support
struct SendableRunLoop(cg::CFRunLoopRef);
unsafe impl Send for SendableRunLoop {}
unsafe impl Sync for SendableRunLoop {}

/// Module-level static so we can stop the listener from another thread
static LISTENER_RUN_LOOP: std::sync::Mutex<Option<SendableRunLoop>> =
    std::sync::Mutex::new(None);

/// Stop the currently-running key listener (if any)
pub fn stop_key_listener() {
    let mut guard = LISTENER_RUN_LOOP.lock().unwrap();
    if let Some(ref rl) = *guard {
        unsafe {
            cg::CFRunLoopStop(rl.0);
        }
    }
    *guard = None;
}

/// Prompt the user for Input Monitoring permission if not already granted
pub fn request_listen_access() -> bool {
    unsafe { cg::CGRequestListenEventAccess() }
}

/// Check Input Monitoring permission without prompting
pub fn preflight_listen_access() -> bool {
    unsafe { cg::CGPreflightListenEventAccess() }
}

/// Shared state for the main hotkey event tap
struct TapState {
    app: AppHandle,
    target_keycode: i64,
    is_modifier: bool,
    modifier_flag: u64,
    pressed: AtomicBool,
    tap: std::sync::Mutex<Option<SendableTap>>,
}

unsafe extern "C" fn tap_callback(
    _proxy: cg::CGEventTapProxy,
    event_type: cg::CGEventType,
    event: cg::CGEventRef,
    user_info: *mut c_void,
) -> cg::CGEventRef {
    let state = &*(user_info as *const TapState);

    // Re-enable if macOS disabled the tap
    if event_type == cg::K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT
        || event_type == cg::K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT
    {
        log::warn!("CGEventTap was disabled by system, re-enabling");
        if let Ok(guard) = state.tap.lock() {
            if let Some(ref tap) = *guard {
                cg::CGEventTapEnable(tap.0, true);
            }
        }
        return event;
    }

    let keycode = cg::CGEventGetIntegerValueField(event, cg::K_CG_KEYBOARD_EVENT_KEYCODE);

    if keycode != state.target_keycode {
        return event;
    }

    let (is_press, is_release) = if state.is_modifier {
        // Modifier keys use flagsChanged — detect press/release from flags
        if event_type != cg::K_CG_EVENT_FLAGS_CHANGED {
            return event;
        }
        let flags = cg::CGEventGetFlags(event);
        let pressed = (flags & state.modifier_flag) != 0;
        (pressed, !pressed)
    } else {
        // Regular keys use keyDown/keyUp
        match event_type {
            cg::K_CG_EVENT_KEY_DOWN => (true, false),
            cg::K_CG_EVENT_KEY_UP => (false, true),
            _ => return event,
        }
    };

    if is_press {
        // Ignore repeats (key already pressed)
        if state.pressed.swap(true, Ordering::SeqCst) {
            return event;
        }
        log::info!("Hotkey pressed → start recording");
        let _ = state.app.emit("hotkey-pressed", ());
    } else if is_release {
        // Ignore if not currently pressed
        if !state.pressed.swap(false, Ordering::SeqCst) {
            return event;
        }
        log::info!("Hotkey released → stop recording");
        let _ = state.app.emit("hotkey-released", ());
    }

    event
}

/// Start a CGEventTap to monitor a specific key — press emits `hotkey-pressed`,
/// release emits `hotkey-released`. Emits `hotkey-status` events as it progresses.
pub fn start_key_listener(app: AppHandle, keycode: i64, shared_state: SharedState) {
    let key_name = keycode_name(keycode);
    let is_modifier = MODIFIER_KEYCODES.contains(&keycode);
    let modifier_flag = modifier_flag_for_keycode(keycode);

    std::thread::spawn(move || {
        // Set status to Retrying
        {
            let mut s = shared_state.blocking_lock();
            s.hotkey_status = crate::state::HotkeyStatus::Retrying;
        }
        let _ = app.emit("hotkey-status", "retrying");

        unsafe {
            // Intentionally leaked: TapState lives for the app's lifetime (one listener
            // thread). Reclaiming it would require synchronizing with the CFRunLoop which
            // is not worth the complexity for a single ~200-byte allocation.
            let state = Box::leak(Box::new(TapState {
                app: app.clone(),
                target_keycode: keycode,
                is_modifier,
                modifier_flag,
                pressed: AtomicBool::new(false),
                tap: std::sync::Mutex::new(None),
            }));

            // Listen for flagsChanged (modifiers) and keyDown/keyUp (regular keys)
            let mask: cg::CGEventMask = (1 << cg::K_CG_EVENT_FLAGS_CHANGED)
                | (1 << cg::K_CG_EVENT_KEY_DOWN)
                | (1 << cg::K_CG_EVENT_KEY_UP);

            // Check Input Monitoring permission upfront and prompt if needed.
            // CGEventTapCreate with kCGEventTapOptionListenOnly can return non-null
            // even WITHOUT Input Monitoring — it just silently only receives events
            // when the app is focused. So we must check the actual permission first.
            if !cg::CGPreflightListenEventAccess() {
                log::warn!("Input Monitoring not granted — prompting user");
                cg::CGRequestListenEventAccess();

                // Wait for user to grant permission (retry up to 15 times)
                for attempt in 1..=15 {
                    if cg::CGPreflightListenEventAccess() {
                        log::info!("Input Monitoring granted after {attempt} attempts");
                        break;
                    }
                    log::info!(
                        "Waiting for Input Monitoring permission (attempt {attempt}/15)"
                    );
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }

                if !cg::CGPreflightListenEventAccess() {
                    log::error!(
                        "Input Monitoring not granted after 15 attempts"
                    );
                    {
                        let mut s = shared_state.blocking_lock();
                        s.hotkey_status = crate::state::HotkeyStatus::Failed;
                    }
                    let _ = app.emit("hotkey-status", "failed");
                    return;
                }
            }

            // Permission confirmed — create the tap
            let mut tap = std::ptr::null();
            for attempt in 1..=5 {
                tap = cg::CGEventTapCreate(
                    0, // kCGHIDEventTap
                    0, // kCGHeadInsertEventTap
                    1, // kCGEventTapOptionListenOnly
                    mask,
                    tap_callback,
                    state as *mut TapState as *mut c_void,
                );
                if !tap.is_null() {
                    break;
                }
                log::warn!(
                    "CGEventTapCreate failed despite permission granted \
                     (attempt {attempt}/5) — retrying in 2s"
                );
                std::thread::sleep(std::time::Duration::from_secs(2));
            }

            if tap.is_null() {
                log::error!(
                    "Failed to create CGEventTap after 15 attempts — \
                     Input Monitoring permission needed"
                );
                {
                    let mut s = shared_state.blocking_lock();
                    s.hotkey_status = crate::state::HotkeyStatus::Failed;
                }
                let _ = app.emit("hotkey-status", "failed");
                return;
            }

            *state.tap.lock().unwrap() = Some(SendableTap(tap));

            let source = cg::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                log::error!("Failed to create run loop source for event tap");
                {
                    let mut s = shared_state.blocking_lock();
                    s.hotkey_status = crate::state::HotkeyStatus::Failed;
                }
                let _ = app.emit("hotkey-status", "failed");
                return;
            }

            let run_loop = cg::CFRunLoopGetCurrent();
            cg::CFRunLoopAddSource(run_loop, source, cg::kCFRunLoopDefaultMode);
            cg::CGEventTapEnable(tap, true);

            // Store run loop so stop_key_listener() can stop it
            {
                let mut guard = LISTENER_RUN_LOOP.lock().unwrap();
                *guard = Some(SendableRunLoop(run_loop));
            }

            // Mark as active
            {
                let mut s = shared_state.blocking_lock();
                s.hotkey_status = crate::state::HotkeyStatus::Active;
            }
            let _ = app.emit("hotkey-status", "active");

            log::info!("CGEventTap for key '{key_name}' (code {keycode}) started");
            cg::CFRunLoopRun();

            // Run loop exited (stopped) — clear the stored run loop
            {
                let mut guard = LISTENER_RUN_LOOP.lock().unwrap();
                *guard = None;
            }
        }
    });
}

// --- Key capture for settings ---

/// Shared state for the one-shot capture tap
struct CaptureState {
    captured_keycode: AtomicI64,
    done: AtomicBool,
    run_loop: std::sync::Mutex<Option<cg::CFRunLoopRef>>,
}

unsafe extern "C" fn capture_callback(
    _proxy: cg::CGEventTapProxy,
    event_type: cg::CGEventType,
    event: cg::CGEventRef,
    user_info: *mut c_void,
) -> cg::CGEventRef {
    let state = &*(user_info as *const CaptureState);

    // Accept keyDown or flagsChanged (for modifier-only keys)
    match event_type {
        cg::K_CG_EVENT_KEY_DOWN | cg::K_CG_EVENT_FLAGS_CHANGED => {}
        _ => return event,
    }

    let keycode = cg::CGEventGetIntegerValueField(event, cg::K_CG_KEYBOARD_EVENT_KEYCODE);

    // For flagsChanged, only capture on press (flag set), not release
    if event_type == cg::K_CG_EVENT_FLAGS_CHANGED {
        let flag = modifier_flag_for_keycode(keycode);
        if flag == 0 {
            return event;
        }
        let flags = cg::CGEventGetFlags(event);
        if (flags & flag) == 0 {
            return event; // This is the release, ignore
        }
    }

    // Ignore Escape — that's our cancel key
    if keycode == 53 {
        state.captured_keycode.store(-1, Ordering::SeqCst);
    } else {
        state.captured_keycode.store(keycode, Ordering::SeqCst);
    }
    state.done.store(true, Ordering::SeqCst);

    // Stop the run loop
    let rl = state.run_loop.lock().unwrap();
    if let Some(rl) = *rl {
        cg::CFRunLoopStop(rl);
    }

    event
}

/// Capture the next key press and return (keycode, name).
/// Returns keycode=-1 if cancelled (Escape).
/// This blocks the calling thread until a key is pressed.
pub fn capture_next_key() -> (i64, String) {
    // Leak to pass across the FFI boundary; reclaimed below after CFRunLoopRun returns.
    let state = Box::leak(Box::new(CaptureState {
        captured_keycode: AtomicI64::new(-2),
        done: AtomicBool::new(false),
        run_loop: std::sync::Mutex::new(None),
    }));

    unsafe {
        let mask: cg::CGEventMask = (1 << cg::K_CG_EVENT_FLAGS_CHANGED)
            | (1 << cg::K_CG_EVENT_KEY_DOWN);

        let tap = cg::CGEventTapCreate(
            0, 0, 1, mask,
            capture_callback,
            state as *mut CaptureState as *mut c_void,
        );

        if tap.is_null() {
            return (-1, "Error: accessibility permission needed".into());
        }

        let source = cg::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if source.is_null() {
            return (-1, "Error: failed to create run loop source".into());
        }

        let run_loop = cg::CFRunLoopGetCurrent();
        *state.run_loop.lock().unwrap() = Some(run_loop);
        cg::CFRunLoopAddSource(run_loop, source, cg::kCFRunLoopDefaultMode);
        cg::CGEventTapEnable(tap, true);

        // This blocks until a key is pressed
        cg::CFRunLoopRun();
    }

    let code = state.captured_keycode.load(Ordering::SeqCst);
    let name = if code >= 0 {
        keycode_name(code)
    } else {
        "Cancelled".into()
    };

    // SAFETY: CFRunLoopRun has returned and the tap callback is no longer active.
    // Reclaim the leaked CaptureState to avoid a memory leak on repeated captures.
    unsafe { let _ = Box::from_raw(state as *mut CaptureState); }

    (code, name)
}
