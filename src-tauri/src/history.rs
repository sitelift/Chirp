use crate::settings::config_dir;
use crate::state::TranscriptionEntry;
use std::path::PathBuf;

fn history_path() -> PathBuf {
    config_dir().join("history.json")
}

/// Load transcription history from disk, returning empty vec on error
pub fn load_history() -> Vec<TranscriptionEntry> {
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            log::warn!("Corrupted history JSON, resetting: {e}");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// Save transcription history to disk, capping at 1000 entries (drops oldest)
pub fn save_history(entries: &[TranscriptionEntry]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;

    let to_save = if entries.len() > 1000 {
        &entries[entries.len() - 1000..]
    } else {
        entries
    };

    let data = serde_json::to_string_pretty(to_save)
        .map_err(|e| format!("Failed to serialize history: {e}"))?;
    std::fs::write(history_path(), data)
        .map_err(|e| format!("Failed to write history: {e}"))
}
