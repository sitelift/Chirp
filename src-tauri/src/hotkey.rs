use rdev::{Event, EventType, Key};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// CapturedKey — returned by capture_next_key()
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct CapturedKey {
    pub code: String,
    pub label: String,
}

// ---------------------------------------------------------------------------
// Statics
// ---------------------------------------------------------------------------

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static GRAB_THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

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
// Key mapping: rdev::Key -> platform-agnostic identifier (DOM event.code)
// ---------------------------------------------------------------------------

pub fn key_to_id(key: &Key) -> String {
    match key {
        // Letters
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

        // Digits
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

        // F-keys
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

        // Modifiers
        Key::ControlLeft => "ControlLeft".into(),
        Key::ControlRight => "ControlRight".into(),
        Key::ShiftLeft => "ShiftLeft".into(),
        Key::ShiftRight => "ShiftRight".into(),
        Key::Alt => "Alt".into(),
        Key::AltGr => "AltGr".into(),
        Key::MetaLeft => "MetaLeft".into(),
        Key::MetaRight => "MetaRight".into(),
        Key::Function => "Fn".into(),

        // Navigation
        Key::UpArrow => "ArrowUp".into(),
        Key::DownArrow => "ArrowDown".into(),
        Key::LeftArrow => "ArrowLeft".into(),
        Key::RightArrow => "ArrowRight".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),

        // Editing
        Key::Backspace => "Backspace".into(),
        Key::Return => "Enter".into(),
        Key::Space => "Space".into(),
        Key::Tab => "Tab".into(),
        Key::Escape => "Escape".into(),
        Key::Delete => "Delete".into(),
        Key::Insert => "Insert".into(),
        Key::CapsLock => "CapsLock".into(),

        // Punctuation
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

        // Lock / media
        Key::PrintScreen => "PrintScreen".into(),
        Key::ScrollLock => "ScrollLock".into(),
        Key::Pause => "Pause".into(),
        Key::NumLock => "NumLock".into(),

        // Numpad
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

        // Unknown
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
        "Alt" => {
            if cfg!(target_os = "macos") {
                "Option".into()
            } else {
                "Alt".into()
            }
        }
        "AltGr" => "AltGr".into(),
        "MetaLeft" | "MetaRight" => {
            if cfg!(target_os = "macos") {
                "Cmd".into()
            } else {
                "Win".into()
            }
        }
        "Fn" => "fn".into(),

        // Letters: strip "Key" prefix
        s if s.starts_with("Key") && s.len() == 4 => s[3..].into(),

        // Digits: strip "Digit" prefix
        s if s.starts_with("Digit") && s.len() == 6 => s[5..].into(),

        // Arrows
        "ArrowUp" => "Up".into(),
        "ArrowDown" => "Down".into(),
        "ArrowLeft" => "Left".into(),
        "ArrowRight" => "Right".into(),

        // Punctuation symbols
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

        // Everything else returned as-is
        other => other.into(),
    }
}

// ---------------------------------------------------------------------------
// Parse a hotkey string like "ControlLeft+ShiftLeft+KeyA" into a set of ids.
// ---------------------------------------------------------------------------

fn parse_hotkey(hotkey: &str) -> HashSet<String> {
    hotkey.split('+').map(|s| s.trim().to_string()).collect()
}

/// Returns true when every key in the combo is a modifier.
fn is_modifier_only_combo(combo: &HashSet<String>) -> bool {
    combo.iter().all(|k| MODIFIERS.contains(&k.as_str()))
}

// ---------------------------------------------------------------------------
// start() — spawn the rdev grab loop on a dedicated thread
// ---------------------------------------------------------------------------

pub fn start(hotkey: &str, app_handle: AppHandle) -> Result<(), String> {
    // Stop any existing listener first
    stop();

    SHUTDOWN.store(false, Ordering::SeqCst);

    let combo = parse_hotkey(hotkey);
    let modifier_only = is_modifier_only_combo(&combo);
    let app = app_handle.clone();

    let handle = std::thread::Builder::new()
        .name("hotkey-grab".into())
        .spawn(move || {
            // rdev::grab requires Fn (not FnMut), so we use RefCell for
            // interior mutability. This is safe because the callback is always
            // invoked on a single thread.
            let held: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
            let combo_active: RefCell<bool> = RefCell::new(false);

            let combo_for_cb = combo.clone();
            let app_for_cb = app.clone();

            let grab_result = rdev::grab(move |event: Event| -> Option<Event> {
                // If shutdown requested, pass everything through.
                if SHUTDOWN.load(Ordering::SeqCst) {
                    return Some(event);
                }

                match event.event_type {
                    EventType::KeyPress(k) => {
                        let id = key_to_id(&k);
                        held.borrow_mut().insert(id.clone());

                        // Check if the held keys match the configured combo exactly
                        if *held.borrow() == combo_for_cb {
                            if !*combo_active.borrow() {
                                *combo_active.borrow_mut() = true;
                                log::info!("Hotkey pressed");
                                let _ = app_for_cb.emit("hotkey-pressed", ());
                            }
                            // Suppress constituent keys unless modifier-only combo
                            if !modifier_only {
                                return None;
                            }
                        }

                        // If the combo is active and this key is part of it, suppress
                        if *combo_active.borrow() && combo_for_cb.contains(&id) && !modifier_only {
                            return None;
                        }

                        Some(event)
                    }
                    EventType::KeyRelease(k) => {
                        let id = key_to_id(&k);
                        held.borrow_mut().remove(&id);

                        if *combo_active.borrow() && combo_for_cb.contains(&id) {
                            *combo_active.borrow_mut() = false;
                            log::info!("Hotkey released");
                            let _ = app_for_cb.emit("hotkey-released", ());

                            // Suppress the release of constituent keys unless modifier-only
                            if !modifier_only {
                                return None;
                            }
                        }

                        Some(event)
                    }
                    _ => Some(event),
                }
            });

            // If grab failed (e.g. no Accessibility permission on macOS), fall
            // back to observe-only rdev::listen().
            if let Err(e) = grab_result {
                log::warn!("rdev::grab failed ({e:?}), falling back to rdev::listen()");
                let _ = app.emit("hotkey-status", "accessibility_required");

                let combo_for_listen = combo;
                let app_for_listen = app;
                let mut held: HashSet<String> = HashSet::new();
                let mut combo_active = false;

                let _ = rdev::listen(move |event: Event| {
                    if SHUTDOWN.load(Ordering::SeqCst) {
                        return;
                    }

                    match event.event_type {
                        EventType::KeyPress(k) => {
                            let id = key_to_id(&k);
                            held.insert(id);

                            if held == combo_for_listen && !combo_active {
                                combo_active = true;
                                log::info!("Hotkey pressed (listen mode)");
                                let _ = app_for_listen.emit("hotkey-pressed", ());
                            }
                        }
                        EventType::KeyRelease(k) => {
                            let id = key_to_id(&k);
                            held.remove(&id);

                            if combo_active && combo_for_listen.contains(&id) {
                                combo_active = false;
                                log::info!("Hotkey released (listen mode)");
                                let _ = app_for_listen.emit("hotkey-released", ());
                            }
                        }
                        _ => {}
                    }
                });
            }
        })
        .map_err(|e| format!("Failed to spawn hotkey thread: {e}"))?;

    if let Ok(mut guard) = GRAB_THREAD.lock() {
        *guard = Some(handle);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// stop() — signal the grab loop to shut down
// ---------------------------------------------------------------------------

pub fn stop() {
    SHUTDOWN.store(true, Ordering::SeqCst);
    // The grab thread will eventually exit once the OS delivers the next event
    // and the callback sees the SHUTDOWN flag. We don't join here to avoid
    // blocking — the old thread will wind down on its own.
    if let Ok(mut guard) = GRAB_THREAD.lock() {
        // Drop the old handle (detaches the thread).
        *guard = None;
    }
}

// ---------------------------------------------------------------------------
// update() — stop and restart with a new hotkey
// ---------------------------------------------------------------------------

pub fn update(hotkey: &str, app_handle: AppHandle) -> Result<(), String> {
    stop();
    // Brief pause to let the old grab thread release the hook.
    std::thread::sleep(std::time::Duration::from_millis(100));
    start(hotkey, app_handle)
}

// ---------------------------------------------------------------------------
// capture_next_key() — one-shot capture via rdev::listen()
// ---------------------------------------------------------------------------

pub async fn capture_next_key() -> Result<CapturedKey, String> {
    // Pause the main grab so it doesn't interfere.
    SHUTDOWN.store(true, Ordering::SeqCst);

    let (tx, rx) = tokio::sync::oneshot::channel::<CapturedKey>();

    std::thread::Builder::new()
        .name("hotkey-capture".into())
        .spawn(move || {
            // Wrap in Mutex so the closure can be FnMut while ensuring we only
            // send once (oneshot::Sender is consumed on send).
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

    // Wait for the first key press, with a 10-second timeout.
    match tokio::time::timeout(std::time::Duration::from_secs(10), rx).await {
        Ok(Ok(key)) => Ok(key),
        Ok(Err(_)) => Err("Capture channel closed unexpectedly".into()),
        Err(_) => Err("Hotkey capture timed out after 10 seconds".into()),
    }
}
