use crate::state::{Settings, SnippetEntry};
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

fn vocabulary_path() -> PathBuf {
    config_dir().join("vocabulary.json")
}

/// Remove old cleanup-model files from previous versions. The current cleanup
/// model is Qwen 3 1.7B Q4_K_M (~1.1 GB); everything below was used in some
/// earlier release or v1.3.0-dev iteration and should be reclaimed on upgrade.
///   - `qwen2.5-3b-instruct-q4_k_m.gguf` — v1.2.5 cleanup model (~1.9 GB)
///   - `chirp-cleanup-0.6b-q4_k_m.gguf` — chirp-cleanup-v2 fine-tune
///   - `Qwen3-0.6B-Q4_K_M.gguf` — superseded by Qwen 3 1.7B
///   - `Ministral-3-8B-Instruct-2512-Q4_K_M.gguf` — tried in v1.3.0 dev
///   - `gemma-4-E2B-it-Q4_K_M.gguf` — v1.2.6 cleanup model (~3.1 GB)
///   - FLAN-T5 chirp-cleanup directory — pre-Qwen experimental backend
fn cleanup_old_models() {
    let llm_dir = config_dir().join("llm");

    let old_files = [
        "qwen2.5-3b-instruct-q4_k_m.gguf",
        "chirp-cleanup-0.6b-q4_k_m.gguf",
        "Qwen3-0.6B-Q4_K_M.gguf",
        "Ministral-3-8B-Instruct-2512-Q4_K_M.gguf",
        "gemma-4-E2B-it-Q4_K_M.gguf",
    ];
    for name in old_files {
        let path = llm_dir.join(name);
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(_) => log::info!("Cleaned up old cleanup model ({})", path.display()),
                Err(e) => log::warn!("Failed to remove {name}: {e}"),
            }
        }
    }

    let chirp_cleanup_dir = config_dir().join("chirp-cleanup");
    if chirp_cleanup_dir.exists() {
        match std::fs::remove_dir_all(&chirp_cleanup_dir) {
            Ok(_) => log::info!("Cleaned up old chirp-cleanup T5 dir"),
            Err(e) => log::warn!("Failed to remove chirp-cleanup dir: {e}"),
        }
    }

    // Shelved Moonshine streaming experiment — free ~272 MB on upgrade.
    let moonshine_dir = models_dir()
        .join("sherpa")
        .join("sherpa-onnx-moonshine-base-en-int8");
    if moonshine_dir.exists() {
        match std::fs::remove_dir_all(&moonshine_dir) {
            Ok(_) => log::info!("Cleaned up shelved Moonshine model dir"),
            Err(e) => log::warn!("Failed to remove Moonshine model dir: {e}"),
        }
    }
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

    // Migrate any non-current model identifier to the current default
    // (Parakeet). Covers whisper tiny/base/small/medium and the shelved
    // Moonshine experiment (moonshine-base, medium-streaming-en, etc.).
    // Parakeet is the only supported ASR today; transcribe::model_info falls
    // back to Parakeet for unknown ids anyway, but the stale string was
    // leaking into log lines and the settings UI.
    if settings.model != "parakeet-tdt-0.6b" {
        log::info!("Migrated model '{}' → 'parakeet-tdt-0.6b'", settings.model);
        settings.model = "parakeet-tdt-0.6b".into();
        let _ = save_settings(&settings);
    }

    // Force the cleanup_model identifier to "chirp-v2" — the only value the
    // app currently understands. The actual model bytes are Qwen 3 1.7B
    // (see llm::MODEL_FILENAME); the setting field is kept as a forward-compat
    // hook so we can swap the underlying model without a migration dance.
    if settings.cleanup_model != "chirp-v2" {
        log::info!("Migrated cleanup_model '{}' → 'chirp-v2'", settings.cleanup_model);
        settings.cleanup_model = "chirp-v2".into();
    }

    // Clean up old model files (Qwen, chirp-cleanup) in background
    cleanup_old_models();

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

/// Load vocabulary from disk.
///
/// Accepts both shapes via `VocabEntryWire`:
///   - Legacy: `["Pieter", "Akilan", ...]`  → widened to entries with empty replaces
///   - New:    `[{"term": "Pieter", "replaces": ["Peter"]}, ...]`
///
/// Also migrates from the much older `dictionary.json` (from→to entries) when
/// `vocabulary.json` is missing or empty. The legacy dictionary's `from`
/// field is used as a `replaces` entry — that's actually the EXACT semantics
/// the user originally wanted, and now we have the schema to express it.
pub fn load_vocabulary() -> Vec<crate::state::VocabEntry> {
    use crate::state::{VocabEntry, VocabEntryWire};

    let path = vocabulary_path();
    let existing: Vec<VocabEntry> = match std::fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str::<Vec<VocabEntryWire>>(&data) {
            Ok(wires) => wires.into_iter().map(VocabEntry::from).collect(),
            Err(e) => {
                log::warn!("Corrupted vocabulary JSON, resetting: {e}");
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    };

    if !existing.is_empty() {
        return existing;
    }

    // vocabulary.json is missing or empty — try to recover from legacy dictionary.json
    let old_path = config_dir().join("dictionary.json");
    if let Ok(old_data) = std::fs::read_to_string(&old_path) {
        #[derive(serde::Deserialize)]
        struct OldEntry { from: String, to: String }
        if let Ok(entries) = serde_json::from_str::<Vec<OldEntry>>(&old_data) {
            let vocab: Vec<VocabEntry> = entries
                .into_iter()
                .map(|e| VocabEntry { term: e.to, replaces: vec![e.from] })
                .collect();
            if !vocab.is_empty() {
                log::info!(
                    "Migrated {} legacy dictionary entries to vocabulary (with replaces preserved)",
                    vocab.len()
                );
                let _ = save_vocabulary(&vocab);
                let _ = std::fs::remove_file(&old_path);
                return vocab;
            }
        }
    }

    Vec::new()
}

fn snippets_path() -> PathBuf {
    config_dir().join("snippets.json")
}

/// Load snippets from disk, providing defaults on first run
pub fn load_snippets() -> Vec<SnippetEntry> {
    let path = snippets_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str::<Vec<SnippetEntry>>(&data) {
            Ok(entries) => {
                if entries == legacy_default_snippets() {
                    log::info!("Removed legacy placeholder snippets");
                    let _ = save_snippets(&[]);
                    Vec::new()
                } else {
                    entries
                }
            }
            Err(e) => {
                log::warn!("Corrupted snippets JSON, resetting: {e}");
                Vec::new()
            }
        },
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
    Vec::new()
}

fn legacy_default_snippets() -> Vec<SnippetEntry> {
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

/// Path to the Silero VAD model
pub fn vad_model_path() -> PathBuf {
    models_dir().join("silero_vad.onnx")
}

/// Check if the VAD model exists on disk
pub fn vad_model_exists() -> bool {
    vad_model_path().exists()
}

const VAD_MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";

/// Download the Silero VAD model (~2MB)
pub async fn download_vad_model() -> Result<(), String> {
    let dest = vad_model_path();
    if dest.exists() {
        return Ok(());
    }

    let dir = models_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create models dir: {e}"))?;

    let tmp_path = dest.with_extension("onnx.tmp");

    let client = reqwest::Client::new();
    let response = client
        .get(VAD_MODEL_URL)
        .send()
        .await
        .map_err(|e| format!("VAD model download failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("VAD download failed with status: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read VAD model: {e}"))?;

    tokio::fs::write(&tmp_path, &bytes)
        .await
        .map_err(|e| format!("Failed to write VAD model: {e}"))?;

    tokio::fs::rename(&tmp_path, &dest)
        .await
        .map_err(|e| format!("Failed to finalize VAD model: {e}"))?;

    log::info!("Silero VAD model downloaded ({} bytes)", bytes.len());
    Ok(())
}

/// Save vocabulary to disk in the new VocabEntry shape.
pub fn save_vocabulary(entries: &[crate::state::VocabEntry]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(vocabulary_path(), data)
        .map_err(|e| format!("Failed to write vocabulary: {e}"))
}
