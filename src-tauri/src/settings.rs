use crate::state::{DictionaryEntry, Settings};
use std::path::PathBuf;

/// Get the app config directory (%APPDATA%/com.chirp.app/)
pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("com.chirp.app")
}

/// Get the models directory
pub fn models_dir() -> PathBuf {
    config_dir().join("models")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn dictionary_path() -> PathBuf {
    config_dir().join("dictionary.json")
}

/// Load settings from disk, returning defaults if file doesn't exist
pub fn load_settings() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Save settings to disk
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data =
        serde_json::to_string_pretty(settings).map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(settings_path(), data).map_err(|e| format!("Failed to write settings: {e}"))
}

/// Load dictionary from disk
pub fn load_dictionary() -> Vec<DictionaryEntry> {
    let path = dictionary_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Save dictionary to disk
pub fn save_dictionary(entries: &[DictionaryEntry]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(dictionary_path(), data)
        .map_err(|e| format!("Failed to write dictionary: {e}"))
}
