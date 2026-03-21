use crate::state::SharedState;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

// Win32 FFI
mod win32 {
    use std::ffi::c_void;

    pub const WH_KEYBOARD_LL: i32 = 13;
    pub const WM_KEYDOWN: u32 = 0x0100;
    pub const WM_KEYUP: u32 = 0x0101;
    pub const WM_SYSKEYDOWN: u32 = 0x0104;
    pub const WM_SYSKEYUP: u32 = 0x0105;
    pub const WM_QUIT: u32 = 0x0012;
    pub const LLKHF_INJECTED: u32 = 0x00000010;

    pub type HHOOK = *mut c_void;
    pub type HINSTANCE = *mut c_void;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LRESULT = isize;
    pub type HOOKPROC =
        unsafe extern "system" fn(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT;

    #[repr(C)]
    #[derive(Debug)]
    pub struct KBDLLHOOKSTRUCT {
        pub vk_code: u32,
        pub scan_code: u32,
        pub flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    pub struct MSG {
        pub hwnd: *mut c_void,
        pub message: u32,
        pub w_param: WPARAM,
        pub l_param: LPARAM,
        pub time: u32,
        pub pt_x: i32,
        pub pt_y: i32,
    }

    extern "system" {
        pub fn SetWindowsHookExW(
            id_hook: i32,
            lpfn: HOOKPROC,
            hmod: HINSTANCE,
            dw_thread_id: u32,
        ) -> HHOOK;
        pub fn UnhookWindowsHookEx(hhk: HHOOK) -> i32;
        pub fn CallNextHookEx(
            hhk: HHOOK,
            n_code: i32,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> LRESULT;
        pub fn GetMessageW(
            msg: *mut MSG,
            hwnd: *mut c_void,
            msg_filter_min: u32,
            msg_filter_max: u32,
        ) -> i32;
        pub fn TranslateMessage(msg: *const MSG) -> i32;
        pub fn DispatchMessageW(msg: *const MSG) -> LRESULT;
        pub fn PostThreadMessageW(
            id_thread: u32,
            msg: u32,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> i32;
        pub fn GetCurrentThreadId() -> u32;
    }
}

// Module-level statics — Win32 hook callbacks have no user_info pointer
static LISTENER_THREAD_ID: Mutex<Option<u32>> = Mutex::new(None);
static TARGET_KEYCODE: AtomicI64 = AtomicI64::new(-1);
static PRESSED: AtomicBool = AtomicBool::new(false);
// OnceLock: the AppHandle is valid for the entire process lifetime and never
// changes, so we set it once. This eliminates lock contention in the hook
// callback, preventing hotkey events from being silently dropped.
static APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

// Statics for capture mode
static CAPTURE_MODE: AtomicBool = AtomicBool::new(false);
static CAPTURED_KEYCODE: AtomicI64 = AtomicI64::new(-2);
static CAPTURE_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// Low-level keyboard hook callback for the main listener
unsafe extern "system" fn listener_hook_proc(
    code: i32,
    w_param: win32::WPARAM,
    l_param: win32::LPARAM,
) -> win32::LRESULT {
    if code >= 0 {
        let kb = &*(l_param as *const win32::KBDLLHOOKSTRUCT);

        // Skip injected keystrokes (from SendInput / enigo text injection)
        if (kb.flags & win32::LLKHF_INJECTED) != 0 {
            return win32::CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param);
        }

        let target = TARGET_KEYCODE.load(Ordering::SeqCst);
        if target >= 0 && kb.vk_code == target as u32 {
            let msg = w_param as u32;
            match msg {
                win32::WM_KEYDOWN | win32::WM_SYSKEYDOWN => {
                    // Ignore repeats
                    if !PRESSED.swap(true, Ordering::SeqCst) {
                        log::info!("Hotkey pressed → start recording");
                        if let Some(app) = APP_HANDLE.get() {
                            let _ = app.emit("hotkey-pressed", ());
                        }
                    }
                }
                win32::WM_KEYUP | win32::WM_SYSKEYUP => {
                    if PRESSED.swap(false, Ordering::SeqCst) {
                        log::info!("Hotkey released → stop recording");
                        if let Some(app) = APP_HANDLE.get() {
                            let _ = app.emit("hotkey-released", ());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    win32::CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
}

/// Low-level keyboard hook callback for one-shot capture
unsafe extern "system" fn capture_hook_proc(
    code: i32,
    w_param: win32::WPARAM,
    l_param: win32::LPARAM,
) -> win32::LRESULT {
    if code >= 0 && CAPTURE_MODE.load(Ordering::SeqCst) {
        let msg = w_param as u32;
        if msg == win32::WM_KEYDOWN || msg == win32::WM_SYSKEYDOWN {
            let kb = &*(l_param as *const win32::KBDLLHOOKSTRUCT);
            let vk = kb.vk_code;

            // VK_ESCAPE (0x1B) = cancel
            if vk == 0x1B {
                CAPTURED_KEYCODE.store(-1, Ordering::SeqCst);
            } else {
                CAPTURED_KEYCODE.store(vk as i64, Ordering::SeqCst);
            }

            CAPTURE_MODE.store(false, Ordering::SeqCst);

            // Post WM_QUIT to break the message pump
            let tid = CAPTURE_THREAD_ID.load(Ordering::SeqCst);
            if tid != 0 {
                win32::PostThreadMessageW(tid, win32::WM_QUIT, 0, 0);
            }
        }
    }
    win32::CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param)
}

/// Run a Win32 message pump until WM_QUIT
fn run_message_pump() {
    unsafe {
        let mut msg: win32::MSG = std::mem::zeroed();
        while win32::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            win32::TranslateMessage(&msg);
            win32::DispatchMessageW(&msg);
        }
    }
}

/// Start a low-level keyboard hook to monitor a specific key.
/// Press emits `hotkey-pressed`, release emits `hotkey-released`.
pub fn start_key_listener(app: AppHandle, keycode: i64, shared_state: SharedState) {
    let key_name = keycode_name(keycode);

    // Store app handle and target keycode in statics
    TARGET_KEYCODE.store(keycode, Ordering::SeqCst);
    PRESSED.store(false, Ordering::SeqCst);
    let _ = APP_HANDLE.set(app.clone());

    std::thread::spawn(move || {
        // Set status to Retrying
        {
            let mut s = shared_state.blocking_lock();
            s.hotkey_status = crate::state::HotkeyStatus::Retrying;
        }
        let _ = app.emit("hotkey-status", "retrying");

        unsafe {
            // Store this thread's ID so stop_key_listener can post WM_QUIT
            let tid = win32::GetCurrentThreadId();
            {
                let mut guard = LISTENER_THREAD_ID.lock().unwrap();
                *guard = Some(tid);
            }

            // Install the low-level keyboard hook
            let hook = win32::SetWindowsHookExW(
                win32::WH_KEYBOARD_LL,
                listener_hook_proc,
                std::ptr::null_mut(),
                0,
            );

            if hook.is_null() {
                log::error!("Failed to install keyboard hook");
                {
                    let mut s = shared_state.blocking_lock();
                    s.hotkey_status = crate::state::HotkeyStatus::Failed;
                }
                let _ = app.emit("hotkey-status", "failed");
                return;
            }

            // Mark as active
            {
                let mut s = shared_state.blocking_lock();
                s.hotkey_status = crate::state::HotkeyStatus::Active;
            }
            let _ = app.emit("hotkey-status", "active");

            log::info!(
                "Keyboard hook for key '{}' (VK 0x{:02X}) started",
                key_name,
                keycode
            );

            // Run message pump — required for low-level hooks
            run_message_pump();

            // Pump exited (WM_QUIT received) — clean up
            if win32::UnhookWindowsHookEx(hook) == 0 {
                log::warn!("UnhookWindowsHookEx failed for listener hook");
            }

            {
                let mut guard = LISTENER_THREAD_ID.lock().unwrap();
                *guard = None;
            }

            log::info!("Keyboard hook stopped");
        }
    });
}

/// Stop the currently-running key listener (if any)
pub fn stop_key_listener() {
    let guard = LISTENER_THREAD_ID.lock().unwrap();
    if let Some(tid) = *guard {
        unsafe {
            win32::PostThreadMessageW(tid, win32::WM_QUIT, 0, 0);
        }
    }
    drop(guard);

    // Clear statics (APP_HANDLE stays set — valid for process lifetime)
    TARGET_KEYCODE.store(-1, Ordering::SeqCst);
    PRESSED.store(false, Ordering::SeqCst);
}

/// Capture the next key press and return (keycode, name).
/// Returns keycode=-1 if cancelled (Escape).
/// This blocks the calling thread until a key is pressed.
pub fn capture_next_key() -> (i64, String) {
    CAPTURED_KEYCODE.store(-2, Ordering::SeqCst);
    CAPTURE_MODE.store(true, Ordering::SeqCst);

    unsafe {
        let tid = win32::GetCurrentThreadId();
        CAPTURE_THREAD_ID.store(tid, Ordering::SeqCst);

        let hook = win32::SetWindowsHookExW(
            win32::WH_KEYBOARD_LL,
            capture_hook_proc,
            std::ptr::null_mut(),
            0,
        );

        if hook.is_null() {
            CAPTURE_MODE.store(false, Ordering::SeqCst);
            return (-1, "Error: failed to install keyboard hook".into());
        }

        // Block until a key is pressed
        run_message_pump();

        win32::UnhookWindowsHookEx(hook);
    }

    let code = CAPTURED_KEYCODE.load(Ordering::SeqCst);
    let name = if code >= 0 {
        keycode_name(code)
    } else {
        "Cancelled".into()
    };

    (code, name)
}

/// Check if we have permission to listen for key events.
/// On Windows, no special permission is needed — always returns true.
pub fn preflight_listen_access() -> bool {
    true
}

/// Returns a human-readable name for a Windows virtual key code
pub fn keycode_name(vk: i64) -> String {
    match vk as u32 {
        // Letters A-Z (0x41-0x5A)
        0x41 => "A", 0x42 => "B", 0x43 => "C", 0x44 => "D",
        0x45 => "E", 0x46 => "F", 0x47 => "G", 0x48 => "H",
        0x49 => "I", 0x4A => "J", 0x4B => "K", 0x4C => "L",
        0x4D => "M", 0x4E => "N", 0x4F => "O", 0x50 => "P",
        0x51 => "Q", 0x52 => "R", 0x53 => "S", 0x54 => "T",
        0x55 => "U", 0x56 => "V", 0x57 => "W", 0x58 => "X",
        0x59 => "Y", 0x5A => "Z",

        // Numbers 0-9 (0x30-0x39)
        0x30 => "0", 0x31 => "1", 0x32 => "2", 0x33 => "3",
        0x34 => "4", 0x35 => "5", 0x36 => "6", 0x37 => "7",
        0x38 => "8", 0x39 => "9",

        // Function keys F1-F24
        0x70 => "F1",  0x71 => "F2",  0x72 => "F3",  0x73 => "F4",
        0x74 => "F5",  0x75 => "F6",  0x76 => "F7",  0x77 => "F8",
        0x78 => "F9",  0x79 => "F10", 0x7A => "F11", 0x7B => "F12",
        0x7C => "F13", 0x7D => "F14", 0x7E => "F15", 0x7F => "F16",
        0x80 => "F17", 0x81 => "F18", 0x82 => "F19", 0x83 => "F20",
        0x84 => "F21", 0x85 => "F22", 0x86 => "F23", 0x87 => "F24",

        // Common keys
        0x20 => "Space",
        0x0D => "Enter",
        0x09 => "Tab",
        0x08 => "Backspace",
        0x1B => "Escape",
        0x2E => "Delete",
        0x2D => "Insert",
        0x24 => "Home",
        0x23 => "End",
        0x21 => "Page Up",
        0x22 => "Page Down",

        // Arrow keys
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",

        // Modifier keys
        0xA0 => "Left Shift",
        0xA1 => "Right Shift",
        0xA2 => "Left Ctrl",
        0xA3 => "Right Ctrl",
        0xA4 => "Left Alt",
        0xA5 => "Right Alt",
        0x5B => "Left Win",
        0x5C => "Right Win",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x14 => "Caps Lock",
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",

        // Numpad
        0x60 => "Numpad 0", 0x61 => "Numpad 1", 0x62 => "Numpad 2",
        0x63 => "Numpad 3", 0x64 => "Numpad 4", 0x65 => "Numpad 5",
        0x66 => "Numpad 6", 0x67 => "Numpad 7", 0x68 => "Numpad 8",
        0x69 => "Numpad 9",
        0x6A => "Numpad *",
        0x6B => "Numpad +",
        0x6D => "Numpad -",
        0x6E => "Numpad .",
        0x6F => "Numpad /",

        // OEM keys
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",

        // Media keys
        0xAD => "Volume Mute",
        0xAE => "Volume Down",
        0xAF => "Volume Up",
        0xB0 => "Next Track",
        0xB1 => "Prev Track",
        0xB3 => "Play/Pause",

        _ => "Unknown",
    }
    .into()
}
