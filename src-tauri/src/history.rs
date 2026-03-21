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

/// Prune entries older than retention_days. If retention_days is 0, keep all.
pub fn prune_history(entries: &mut Vec<TranscriptionEntry>, retention_days: i64) {
    if retention_days <= 0 {
        return;
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
    let cutoff_str = cutoff.to_rfc3339();
    let before = entries.len();
    entries.retain(|e| e.timestamp >= cutoff_str);
    let pruned = before - entries.len();
    if pruned > 0 {
        log::info!("Pruned {pruned} history entries older than {retention_days} days");
        if let Err(e) = save_history(entries) {
            log::warn!("Failed to save pruned history: {e}");
        }
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
