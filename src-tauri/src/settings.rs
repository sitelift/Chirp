use crate::state::{Settings, SnippetEntry};
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

fn vocabulary_path() -> PathBuf {
    config_dir().join("vocabulary.json")
}

/// Remove files left behind by earlier versions: the local cleanup LLM
/// (llama-server + GGUF models, 1-3 GB) and its log, the FLAN-T5
/// experiment, and the shelved Moonshine streaming model.
fn cleanup_old_models() {
    let dirs = [
        config_dir().join("llm"),
        config_dir().join("chirp-cleanup"),
        models_dir().join("sherpa").join("sherpa-onnx-moonshine-base-en-int8"),
    ];
    for dir in dirs {
        if dir.exists() {
            match std::fs::remove_dir_all(&dir) {
                Ok(_) => log::info!("Removed old model dir {}", dir.display()),
                Err(e) => log::warn!("Failed to remove {}: {e}", dir.display()),
            }
        }
    }

    let _ = std::fs::remove_file(config_dir().join("llm-server.log"));
}

/// Load settings from disk, returning defaults if file doesn't exist
pub fn load_settings() -> Settings {
    let path = settings_path();
    let mut settings = match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str::<serde_json::Value>(&data).and_then(|mut value| {
            crate::hotkey_migration::migrate(&mut value);
            serde_json::from_value(value)
        }).unwrap_or_else(|e| {
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

    // Reclaim disk from local cleanup models that v2 no longer uses
    cleanup_old_models();

    for binding in [&mut settings.hold_binding, &mut settings.toggle_binding].into_iter().flatten() {
        *binding = crate::hotkey::Binding::new(binding.keys.clone(), binding.suppress);
    }
    let _ = save_settings(&settings);
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
