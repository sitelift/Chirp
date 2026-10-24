use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

use rdev::{Event, EventType, Key};

// ---------------------------------------------------------------------------
// ActiveCombo — shared flag between Raw Input (detection) and LL hook
// (suppression) on Windows. Raw Input writes; LL hook reads to decide whether
// to swallow a key.
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ActiveCombo(AtomicBool);

impl ActiveCombo {
    pub fn new() -> Self {
        Self(AtomicBool::new(false))
    }
    pub fn store(&self, v: bool) {
        self.0.store(v, Ordering::SeqCst);
    }
    pub fn load(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// CapturedKey — returned by capture_next_key()
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct CapturedKey {
    pub code: String,
    pub label: String,
}

/// Modifier key identifiers — combos consisting entirely of these are never
/// suppressed so that e.g. Ctrl alone as a hotkey doesn't break Ctrl+C.
const MODIFIERS: &[&str] = &[
    "ControlLeft",
    "ControlRight",
    "ShiftLeft",
    "ShiftRight",
    "Alt",
    "AltGr",
    "MetaLeft",
    "MetaRight",
    "Fn",
];

// ---------------------------------------------------------------------------
// Key mapping (rdev Key -> string id, used by capture_next_key)
// ---------------------------------------------------------------------------

pub fn key_to_id(key: &Key) -> String {
    match key {
        Key::KeyA => "KeyA".into(),
        Key::KeyB => "KeyB".into(),
        Key::KeyC => "KeyC".into(),
        Key::KeyD => "KeyD".into(),
        Key::KeyE => "KeyE".into(),
        Key::KeyF => "KeyF".into(),
        Key::KeyG => "KeyG".into(),
        Key::KeyH => "KeyH".into(),
        Key::KeyI => "KeyI".into(),
        Key::KeyJ => "KeyJ".into(),
        Key::KeyK => "KeyK".into(),
        Key::KeyL => "KeyL".into(),
        Key::KeyM => "KeyM".into(),
        Key::KeyN => "KeyN".into(),
        Key::KeyO => "KeyO".into(),
        Key::KeyP => "KeyP".into(),
        Key::KeyQ => "KeyQ".into(),
        Key::KeyR => "KeyR".into(),
        Key::KeyS => "KeyS".into(),
        Key::KeyT => "KeyT".into(),
        Key::KeyU => "KeyU".into(),
        Key::KeyV => "KeyV".into(),
        Key::KeyW => "KeyW".into(),
        Key::KeyX => "KeyX".into(),
        Key::KeyY => "KeyY".into(),
        Key::KeyZ => "KeyZ".into(),
        Key::Num0 => "Digit0".into(),
        Key::Num1 => "Digit1".into(),
        Key::Num2 => "Digit2".into(),
        Key::Num3 => "Digit3".into(),
        Key::Num4 => "Digit4".into(),
        Key::Num5 => "Digit5".into(),
        Key::Num6 => "Digit6".into(),
        Key::Num7 => "Digit7".into(),
        Key::Num8 => "Digit8".into(),
        Key::Num9 => "Digit9".into(),
        Key::F1 => "F1".into(),
        Key::F2 => "F2".into(),
        Key::F3 => "F3".into(),
        Key::F4 => "F4".into(),
        Key::F5 => "F5".into(),
        Key::F6 => "F6".into(),
        Key::F7 => "F7".into(),
        Key::F8 => "F8".into(),
        Key::F9 => "F9".into(),
        Key::F10 => "F10".into(),
        Key::F11 => "F11".into(),
        Key::F12 => "F12".into(),
        Key::ControlLeft => "ControlLeft".into(),
        Key::ControlRight => "ControlRight".into(),
        Key::ShiftLeft => "ShiftLeft".into(),
        Key::ShiftRight => "ShiftRight".into(),
        Key::Alt => "Alt".into(),
        Key::AltGr => "AltGr".into(),
        Key::MetaLeft => "MetaLeft".into(),
        Key::MetaRight => "MetaRight".into(),
        Key::Function => "Fn".into(),
        Key::UpArrow => "ArrowUp".into(),
        Key::DownArrow => "ArrowDown".into(),
        Key::LeftArrow => "ArrowLeft".into(),
        Key::RightArrow => "ArrowRight".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Backspace => "Backspace".into(),
        Key::Return => "Enter".into(),
        Key::Space => "Space".into(),
        Key::Tab => "Tab".into(),
        Key::Escape => "Escape".into(),
        Key::Delete => "Delete".into(),
        Key::Insert => "Insert".into(),
        Key::CapsLock => "CapsLock".into(),
        Key::BackQuote => "Backquote".into(),
        Key::Minus => "Minus".into(),
        Key::Equal => "Equal".into(),
        Key::LeftBracket => "BracketLeft".into(),
        Key::RightBracket => "BracketRight".into(),
        Key::SemiColon => "Semicolon".into(),
        Key::Quote => "Quote".into(),
        Key::BackSlash => "Backslash".into(),
        Key::IntlBackslash => "IntlBackslash".into(),
        Key::Comma => "Comma".into(),
        Key::Dot => "Period".into(),
        Key::Slash => "Slash".into(),
        Key::PrintScreen => "PrintScreen".into(),
        Key::ScrollLock => "ScrollLock".into(),
        Key::Pause => "Pause".into(),
        Key::NumLock => "NumLock".into(),
        Key::Kp0 => "Numpad0".into(),
        Key::Kp1 => "Numpad1".into(),
        Key::Kp2 => "Numpad2".into(),
        Key::Kp3 => "Numpad3".into(),
        Key::Kp4 => "Numpad4".into(),
        Key::Kp5 => "Numpad5".into(),
        Key::Kp6 => "Numpad6".into(),
        Key::Kp7 => "Numpad7".into(),
        Key::Kp8 => "Numpad8".into(),
        Key::Kp9 => "Numpad9".into(),
        Key::KpReturn => "NumpadEnter".into(),
        Key::KpMinus => "NumpadSubtract".into(),
        Key::KpPlus => "NumpadAdd".into(),
        Key::KpMultiply => "NumpadMultiply".into(),
        Key::KpDivide => "NumpadDivide".into(),
        Key::KpDelete => "NumpadDecimal".into(),
        Key::Unknown(code) => format!("Unknown_{code}"),
    }
}

// ---------------------------------------------------------------------------
// Label mapping: identifier -> human-readable display label
// ---------------------------------------------------------------------------

pub fn id_to_label(id: &str) -> String {
    match id {
        "ControlLeft" | "ControlRight" => "Ctrl".into(),
        "ShiftLeft" | "ShiftRight" => "Shift".into(),
        "Alt" => "Alt".into(),
        "AltGr" => "AltGr".into(),
        "MetaLeft" | "MetaRight" => "Win".into(),
        "Fn" => "fn".into(),
        s if s.starts_with("Key") && s.len() == 4 => s[3..].into(),
        s if s.starts_with("Digit") && s.len() == 6 => s[5..].into(),
        "ArrowUp" => "Up".into(),
        "ArrowDown" => "Down".into(),
        "ArrowLeft" => "Left".into(),
        "ArrowRight" => "Right".into(),
        "Backquote" => "`".into(),
        "Minus" => "-".into(),
        "Equal" => "=".into(),
        "BracketLeft" => "[".into(),
        "BracketRight" => "]".into(),
        "Semicolon" => ";".into(),
        "Quote" => "'".into(),
        "Backslash" => "\\".into(),
        "IntlBackslash" => "\\".into(),
        "Comma" => ",".into(),
        "Period" => ".".into(),
        "Slash" => "/".into(),
        other => other.into(),
    }
}

// ---------------------------------------------------------------------------
// Parse a hotkey string like "ControlLeft+ShiftLeft+KeyA" into a set of ids.
// ---------------------------------------------------------------------------

fn parse_hotkey(hotkey: &str) -> HashSet<String> {
    hotkey.split('+').map(|s| s.trim().to_string()).collect()
}

fn is_modifier_only_combo(combo: &HashSet<String>) -> bool {
    combo.iter().all(|k| MODIFIERS.contains(&k.as_str()))
}

// ===========================================================================
// start(): Raw Input detection + LL hook suppression (split architecture)
// ===========================================================================

pub fn start(hotkey: &str, app_handle: AppHandle) -> Result<(), String> {
    stop();

    let combo = parse_hotkey(hotkey);
    let modifier_only = is_modifier_only_combo(&combo);
    let active_combo = Arc::new(ActiveCombo::new());

    // Compute the VK set the LL hook should suppress. Modifier-only combos
    // never suppress (would break Ctrl+C, etc.).
    let vk_set: HashSet<u16> = if modifier_only {
        HashSet::new()
    } else {
        combo.iter().filter_map(|id| crate::hotkey_raw_input::id_to_vk(id)).collect()
    };

    crate::hotkey_raw_input::start(
        combo.clone(),
        modifier_only,
        app_handle,
        active_combo.clone(),
    )?;

    if !modifier_only {
        if let Err(e) = crate::hotkey_ll_suppress::start(vk_set, active_combo) {
            log::warn!("LL suppression hook failed to start: {e} — detection still active, but keys will leak through to focused app");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// stop()
// ---------------------------------------------------------------------------

pub fn stop() {

    crate::hotkey_raw_input::stop();
    crate::hotkey_ll_suppress::stop();
}

// ---------------------------------------------------------------------------
// update()
// ---------------------------------------------------------------------------

pub fn update(hotkey: &str, app_handle: AppHandle) -> Result<(), String> {
    stop();
    std::thread::sleep(std::time::Duration::from_millis(100));
    start(hotkey, app_handle)
}

// ---------------------------------------------------------------------------
// capture_next_key() — rdev listen
// ---------------------------------------------------------------------------

pub async fn capture_next_key() -> Result<CapturedKey, String> {

    // Release the Raw Input / LL hook listeners so the key we're about to
    // capture isn't suppressed and doesn't double-fire.
    crate::hotkey_raw_input::stop();
    crate::hotkey_ll_suppress::stop();

    let (tx, rx) = tokio::sync::oneshot::channel::<CapturedKey>();

    std::thread::Builder::new()
        .name("hotkey-capture".into())
        .spawn(move || {
            let tx = std::sync::Mutex::new(Some(tx));

            let _ = rdev::listen(move |event: Event| {
                if let EventType::KeyPress(k) = event.event_type {
                    let id = key_to_id(&k);
                    let label = id_to_label(&id);
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(sender) = guard.take() {
                            let _ = sender.send(CapturedKey { code: id, label });
                        }
                    }
                }
            });
        })
        .map_err(|e| format!("Failed to spawn capture thread: {e}"))?;

    match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
        Ok(Ok(key)) => Ok(key),
        Ok(Err(_)) => Err("Capture channel closed unexpectedly".into()),
        Err(_) => Err("Hotkey capture timed out after 10 seconds".into()),
    }
}
