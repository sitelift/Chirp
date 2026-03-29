use crate::state::{DictionaryEntry, Settings, SnippetEntry};
use std::path::PathBuf;

/// Migrate old Tauri shortcut format (e.g., "CmdOrCtrl+Shift+Space") to new
/// event.code-based format (e.g., "MetaLeft+ShiftLeft+Space").
fn migrate_hotkey(hotkey: &str) -> String {
    // Quick check: if it already uses new-style identifiers, return as-is
    let has_new_style = hotkey.contains("Left") || hotkey.contains("Right")
        || hotkey.contains("Key") || hotkey.contains("Digit") || hotkey == "Fn";
    let has_old_style = hotkey.contains("CmdOrCtrl") || hotkey.contains("Cmd")
        || (hotkey.contains("Ctrl") && !hotkey.contains("Control"))
        || (hotkey.contains("Shift") && !hotkey.contains("ShiftLeft") && !hotkey.contains("ShiftRight"))
        || (hotkey.contains("Alt") && !hotkey.contains("AltGr"));

    if has_new_style && !has_old_style {
        return hotkey.to_string();
    }
    if !has_old_style {
        return hotkey.to_string();
    }

    let parts: Vec<&str> = hotkey.split('+').collect();
    let mut new_parts: Vec<String> = Vec::new();

    for part in parts {
        let migrated = match part.trim() {
            "CmdOrCtrl" => {
                if cfg!(target_os = "macos") { "MetaLeft" } else { "ControlLeft" }
            }
            "Ctrl" | "Control" => "ControlLeft",
            "Cmd" | "Command" | "Meta" | "Super" => "MetaLeft",
            "Shift" => "ShiftLeft",
            "Alt" | "Option" => "Alt",
            "Space" => "Space",
            "Tab" => "Tab",
            "Backspace" => "Backspace",
            "Delete" => "Delete",
            "Enter" | "Return" => "Enter",
            "Escape" | "Esc" => "Escape",
            "Up" => "ArrowUp",
            "Down" => "ArrowDown",
            "Left" => "ArrowLeft",
            "Right" => "ArrowRight",
            s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
                new_parts.push(format!("Key{}", s.to_uppercase()));
                continue;
            }
            s if s.starts_with('F') && s[1..].parse::<u32>().is_ok() => s,
            other => other,
        };
        new_parts.push(migrated.to_string());
    }

    new_parts.join("+")
}

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
    let mut settings = match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            log::warn!("Corrupted settings JSON, using defaults: {e}");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    };

    // Migrate old whisper model IDs to new default
    match settings.model.as_str() {
        "tiny" | "base" | "small" | "medium" => {
            settings.model = "parakeet-tdt-0.6b".into();
        }
        _ => {}
    }

    // Migrate old Tauri shortcut format to new event.code-based format
    let migrated = migrate_hotkey(&settings.hotkey);
    if migrated != settings.hotkey {
        log::info!("Migrated hotkey '{}' → '{}'", settings.hotkey, migrated);
        settings.hotkey = migrated;
        let _ = save_settings(&settings);
    }

    settings
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
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            log::warn!("Corrupted dictionary JSON, resetting: {e}");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

fn snippets_path() -> PathBuf {
    config_dir().join("snippets.json")
}

/// Load snippets from disk, providing defaults on first run
pub fn load_snippets() -> Vec<SnippetEntry> {
    let path = snippets_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            log::warn!("Corrupted snippets JSON, resetting: {e}");
            default_snippets()
        }),
        Err(_) => default_snippets(),
    }
}

/// Save snippets to disk
pub fn save_snippets(entries: &[SnippetEntry]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(snippets_path(), data)
        .map_err(|e| format!("Failed to write snippets: {e}"))
}

fn default_snippets() -> Vec<SnippetEntry> {
    vec![
        SnippetEntry {
            trigger: "my email address".into(),
            expansion: "user@example.com".into(),
        },
        SnippetEntry {
            trigger: "my signature".into(),
            expansion: "Best regards,\n[Your Name]".into(),
        },
    ]
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
