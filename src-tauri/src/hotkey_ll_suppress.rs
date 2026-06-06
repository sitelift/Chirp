//! Windows `WH_KEYBOARD_LL` hook used *only* for suppression.
//!
//! Detection happens in `hotkey_raw_input`. This module's single job is to
//! prevent hotkey keys from reaching the focused application while the combo
//! is pressed. It reads `ActiveCombo` (set by the Raw Input thread) and
//! returns 1 from the hook when the current VK is in the configured combo
//! AND the combo is currently active.
//!
//! Why split detection from suppression? `WH_KEYBOARD_LL` is fragile — the
//! OS silently unhooks it under load, during UAC prompts, or on
//! resume-from-sleep. When that happens, detection via Raw Input keeps
//! working; the user loses suppression but the app still responds to the
//! hotkey. If this hook dies, it stays dead — no revival, no heartbeat.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

struct HookConfig {
    vk_set: HashSet<u16>,
    active_combo: Arc<crate::hotkey::ActiveCombo>,
}

static HOOK_CONFIG: OnceLock<Mutex<Option<HookConfig>>> = OnceLock::new();
static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static THREAD_HANDLE: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

/// VKs whose *down* event we actually swallowed. On key-up we only suppress
/// releases that match a swallowed press — otherwise we let the up through so
/// the target app doesn't end up thinking the key is still held (stuck
/// modifier bug). Raw Input (detection) is unaffected by LL suppression, so
/// the Rust side still sees the release cleanly.
static SUPPRESSED_DOWN: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();

fn config_slot() -> &'static Mutex<Option<HookConfig>> {
    HOOK_CONFIG.get_or_init(|| Mutex::new(None))
}

fn suppressed_down() -> &'static Mutex<HashSet<u16>> {
    SUPPRESSED_DOWN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn start(
    vk_set: HashSet<u16>,
    active_combo: Arc<crate::hotkey::ActiveCombo>,
) -> Result<(), String> {
    stop();
    SHUTDOWN.store(false, Ordering::SeqCst);

    if let Ok(mut slot) = config_slot().lock() {
        *slot = Some(HookConfig { vk_set, active_combo });
    }

    let handle = std::thread::Builder::new()
        .name("hotkey-ll-suppress".into())
        .spawn(move || unsafe {
            let hook: HHOOK =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), std::ptr::null_mut(), 0);
            if hook.is_null() {
                log::error!("SetWindowsHookExW(WH_KEYBOARD_LL) returned null");
                return;
            }

            let mut msg: MSG = std::mem::zeroed();
            while !SHUTDOWN.load(Ordering::SeqCst) {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r <= 0 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(hook);
        })
        .map_err(|e| format!("Failed to spawn LL suppression thread: {e}"))?;

    if let Ok(mut guard) = THREAD_HANDLE.lock() {
        *guard = Some(handle);
    }
    Ok(())
}

pub fn stop() {
    SHUTDOWN.store(true, Ordering::SeqCst);
    if let Ok(mut slot) = config_slot().lock() {
        *slot = None;
    }
    if let Ok(mut set) = suppressed_down().lock() {
        set.clear();
    }
    if let Ok(mut guard) = THREAD_HANDLE.lock() {
        *guard = None;
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode as u16;

    // Normalize generic VK_SHIFT/VK_CONTROL/VK_MENU to left/right using the
    // extended flag + scancode so the set membership check matches the
    // detection side.
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    const LLKHF_EXTENDED: u32 = 0x00000001;
    let extended = (kb.flags & LLKHF_EXTENDED) != 0;
    let resolved = match vk {
        VK_SHIFT => MapVirtualKeyW(kb.scanCode, 3) as u16,
        VK_CONTROL => if extended { VK_RCONTROL } else { VK_LCONTROL },
        VK_MENU => if extended { VK_RMENU } else { VK_LMENU },
        other => other,
    };
    let resolved = if resolved == 0 { vk } else { resolved };

    // Distinguish press vs release. Suppressing a release whose press we let
    // through would leave the target app with a "stuck" key — it saw the
    // down but never the up. So the rule is: only suppress a release if we
    // actually suppressed its matching press.
    let msg = wparam as u32;
    let is_release = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
    let is_press = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);

    if is_release {
        let had_suppressed_down = suppressed_down()
            .lock()
            .map(|mut set| set.remove(&resolved))
            .unwrap_or(false);
        if had_suppressed_down {
            return 1;
        }
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    if is_press {
        let should_suppress = if let Ok(slot) = config_slot().lock() {
            if let Some(cfg) = slot.as_ref() {
                cfg.active_combo.load() && cfg.vk_set.contains(&resolved)
            } else {
                false
            }
        } else {
            false
        };
        if should_suppress {
            if let Ok(mut set) = suppressed_down().lock() {
                set.insert(resolved);
            }
            return 1;
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}
