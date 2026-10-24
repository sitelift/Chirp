use std::thread;
use std::time::Duration;

#[cfg(windows)]
use crate::clipboard_win;

#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEINPUT, MapVirtualKeyW, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_V,
};
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow};

/// Inject text at the current cursor position: Win32 clipboard with
/// exclusion flags, wait for held modifiers to release, then Ctrl+V.
pub fn inject_text(text: &str) -> Result<(), String> {
    inject_text_windows(text)
}

/// Windows-only modifier VK codes we care about.
#[cfg(windows)]
const MODIFIER_VKS: &[VIRTUAL_KEY] = &[
    VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN,
];

/// Return true if the given virtual-key is currently down (async).
#[cfg(windows)]
fn is_key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// Send a single keyboard input event via SendInput.
#[cfg(windows)]
fn send_key_event(vk: VIRTUAL_KEY, key_up: bool) {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let mut flags = KEYEVENTF_SCANCODE;
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Poll GetAsyncKeyState until all tracked modifiers are released, or timeout expires.
/// On timeout, synthesize a KEYUP for any still-held modifier as a safety net.
#[cfg(windows)]
fn wait_for_modifiers_released(timeout_ms: u64) {
    let start = std::time::Instant::now();
    let deadline = Duration::from_millis(timeout_ms);

    loop {
        if MODIFIER_VKS.iter().all(|&vk| !is_key_down(vk)) {
            return;
        }
        if start.elapsed() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    // Timeout — collect stuck modifiers and force a KEYUP on each.
    let stuck: Vec<VIRTUAL_KEY> = MODIFIER_VKS
        .iter()
        .copied()
        .filter(|&vk| is_key_down(vk))
        .collect();
    if !stuck.is_empty() {
        log::warn!(
            "wait_for_modifiers_released timeout; forcing KEYUP on stuck vks: {:?}",
            stuck
        );
        let mut win_stuck = false;
        for &vk in &stuck {
            send_key_event(vk, true);
            if vk == VK_LWIN || vk == VK_RWIN {
                win_stuck = true;
            }
        }
        if win_stuck {
            cancel_pending_start_menu();
        }
    }
}

/// When Win is released alone, Windows queues a Start Menu activation.
/// Sending a dummy unassigned VK press/release swallows it.
#[cfg(windows)]
fn cancel_pending_start_menu() {
    const VK_DUMMY: VIRTUAL_KEY = 0xFF;
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_DUMMY,
                wScan: 0,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_DUMMY,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &down, std::mem::size_of::<INPUT>() as i32);
        SendInput(1, &up, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Check whether the foreground window is the legacy cmd.exe console.
#[cfg(windows)]
fn foreground_is_console() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len <= 0 {
            return false;
        }
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        class == "ConsoleWindowClass"
    }
}

/// Send Ctrl+V via SendInput using scan codes, avoiding enigo entirely.
#[cfg(windows)]
fn send_ctrl_v() {
    send_key_event(VK_CONTROL, false);
    send_key_event(VK_V, false);
    send_key_event(VK_V, true);
    send_key_event(VK_CONTROL, true);
}

/// Send a single right-click at the current cursor position.
#[cfg(windows)]
fn send_right_click() {
    let down = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_RIGHTDOWN,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_RIGHTUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &down, std::mem::size_of::<INPUT>() as i32);
        SendInput(1, &up, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Windows: Win32 clipboard with exclusion flags, wait-for-release modifiers, Ctrl+V paste.
#[cfg(windows)]
fn inject_text_windows(text: &str) -> Result<(), String> {
    // 1. Save current clipboard
    let saved = clipboard_win::save_clipboard()?;

    // 2. Set clipboard with retry + verification
    let mut verified = false;
    for attempt in 0..5 {
        if let Err(e) = clipboard_win::set_clipboard_with_exclusion(text) {
            log::warn!("Clipboard set attempt {}/5: {e}", attempt + 1);
            thread::sleep(Duration::from_millis(20));
            continue;
        }

        thread::sleep(Duration::from_millis(30));

        match clipboard_win::verify_clipboard_text(text) {
            Ok(true) => {
                log::info!("Clipboard verified on attempt {}/5", attempt + 1);
                verified = true;
                break;
            }
            Ok(false) => {
                log::warn!("Clipboard mismatch on attempt {}/5", attempt + 1);
            }
            Err(e) => {
                log::warn!("Clipboard verify failed on attempt {}/5: {e}", attempt + 1);
            }
        }

        thread::sleep(Duration::from_millis(20));
    }

    if !verified {
        return Err("Failed to set clipboard — text did not persist after 5 attempts".into());
    }

    // 3. Wait for the user to physically release the hotkey modifiers before pasting.
    wait_for_modifiers_released(300);

    // 4. Paste. cmd.exe legacy console doesn't respond to Ctrl+V; right-click pastes in QuickEdit mode.
    if foreground_is_console() {
        // TODO: ConsoleWindowClass paste — right-click only works with QuickEdit enabled.
        send_right_click();
    } else {
        send_ctrl_v();
    }

    // 5. Restore clipboard after short delay (background thread)
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(800));
        if let Err(e) = clipboard_win::restore_clipboard(saved) {
            log::warn!("Failed to restore clipboard: {e}");
        }
    });

    Ok(())
}
