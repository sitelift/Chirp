use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Inject text at the current cursor position by:
/// 1. Saving the current clipboard contents
/// 2. Setting the clipboard to the new text (with verification)
/// 3. Simulating Ctrl+V
/// 4. Restoring the original clipboard (deferred to background thread)
pub fn inject_text(text: &str) -> Result<(), String> {
    // Save current clipboard content using a short-lived Clipboard instance
    let saved = {
        let mut cb = Clipboard::new().map_err(|e| format!("Failed to access clipboard: {e}"))?;
        let s = cb.get_text().ok();
        log::debug!(
            "Clipboard before inject: {:?}",
            s.as_deref().map(|t| if t.len() > 80 { &t[..80] } else { t })
        );
        s
    };

    // Set new text with retry loop and read-back verification.
    let mut verified = false;
    for attempt in 0..5 {
        {
            let mut cb =
                Clipboard::new().map_err(|e| format!("Failed to access clipboard: {e}"))?;
            if let Err(e) = cb.set_text(text.to_string()) {
                log::warn!("Clipboard set_text attempt {}/5: {e}", attempt + 1);
                thread::sleep(Duration::from_millis(20));
                continue;
            }
        }

        // Small delay then verify with a fresh handle
        thread::sleep(Duration::from_millis(30));

        {
            let mut cb =
                Clipboard::new().map_err(|e| format!("Failed to access clipboard: {e}"))?;
            match cb.get_text() {
                Ok(current) if current == text => {
                    log::info!("Clipboard verified on attempt {}/5", attempt + 1);
                    verified = true;
                    break;
                }
                Ok(current) => {
                    log::warn!(
                        "Clipboard mismatch on attempt {}/5: expected '{}', got '{}'",
                        attempt + 1,
                        if text.len() > 60 { &text[..60] } else { text },
                        if current.len() > 60 {
                            &current[..60]
                        } else {
                            &current
                        }
                    );
                }
                Err(e) => {
                    log::warn!("Clipboard read-back failed on attempt {}/5: {e}", attempt + 1);
                }
            }
        }

        thread::sleep(Duration::from_millis(20));
    }

    if !verified {
        return Err("Failed to set clipboard — text did not persist after 5 attempts".into());
    }

    // Simulate Cmd+V (macOS) or Ctrl+V (other platforms)
    let paste_modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init enigo: {e}"))?;
    enigo
        .key(paste_modifier, Direction::Press)
        .map_err(|e| format!("Key press failed: {e}"))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Key click failed: {e}"))?;
    enigo
        .key(paste_modifier, Direction::Release)
        .map_err(|e| format!("Key release failed: {e}"))?;

    // Restore clipboard in a background thread after a generous delay.
    // The target app needs time to process the Ctrl+V and read the clipboard.
    // 2s was racing some slow apps (e.g. Electron apps under load) — bumped
    // to 3s. Trade-off: user copies within 3s get overwritten.
    if let Some(original) = saved {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(3));
            match Clipboard::new() {
                Ok(mut cb) => {
                    if let Err(e) = cb.set_text(original) {
                        log::warn!("Failed to restore clipboard: {e}");
                    } else {
                        log::debug!("Clipboard restored after 3s delay");
                    }
                }
                Err(e) => log::warn!("Failed to open clipboard for restore: {e}"),
            }
        });
    }

    Ok(())
}
