use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Inject text at the current cursor position by:
/// 1. Saving the current clipboard contents
/// 2. Setting the clipboard to the new text
/// 3. Simulating Ctrl+V
/// 4. Restoring the original clipboard
pub fn inject_text(text: &str) -> Result<(), String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {e}"))?;

    // Save current clipboard content
    let saved = clipboard.get_text().ok();

    // Set new text
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Failed to set clipboard: {e}"))?;

    // Brief delay to ensure clipboard is ready
    thread::sleep(Duration::from_millis(20));

    // Simulate Ctrl+V
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init enigo: {e}"))?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("Key press failed: {e}"))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Key click failed: {e}"))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("Key release failed: {e}"))?;

    // Delay before restoring — must be long enough for the target app to read clipboard
    thread::sleep(Duration::from_millis(100));

    // Restore original clipboard
    if let Some(original) = saved {
        let _ = clipboard.set_text(original);
    }

    Ok(())
}
