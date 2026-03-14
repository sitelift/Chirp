use futures_util::StreamExt;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::settings::models_dir;

/// Model metadata: (filename, download url suffix, size in bytes)
fn model_info(model: &str) -> (&'static str, &'static str, u64) {
    match model {
        "tiny" => ("ggml-tiny.en.bin", "ggml-tiny.en.bin", 77_700_000),
        "base" => ("ggml-base.en.bin", "ggml-base.en.bin", 147_500_000),
        "small" => ("ggml-small.en.bin", "ggml-small.en.bin", 488_000_000),
        "medium" => ("ggml-medium.en.bin", "ggml-medium.en.bin", 1_533_000_000),
        // Also support the multilingual filenames (for existing downloads)
        _ => ("ggml-small.bin", "ggml-small.bin", 488_000_000),
    }
}

/// Model file sizes for status reporting
pub fn model_size_bytes(model: &str) -> u64 {
    model_info(model).2
}

/// Get the expected path for a whisper model file
pub fn model_path(model: &str) -> PathBuf {
    let (filename, _, _) = model_info(model);
    models_dir().join("whisper").join(filename)
}

/// Check if a model file exists on disk (check both .en and non-.en variants)
pub fn model_exists(model: &str) -> bool {
    if model_path(model).exists() {
        return true;
    }
    // Fallback: check for non-English variant (from previous downloads)
    let fallback = models_dir()
        .join("whisper")
        .join(format!("ggml-{model}.bin"));
    fallback.exists()
}

/// Find the actual path of the model (preferring .en variant)
fn resolve_model_path(model: &str) -> Option<PathBuf> {
    let en_path = model_path(model);
    if en_path.exists() {
        return Some(en_path);
    }
    let fallback = models_dir()
        .join("whisper")
        .join(format!("ggml-{model}.bin"));
    if fallback.exists() {
        return Some(fallback);
    }
    None
}

/// Load a whisper model from disk
pub fn load_model(model: &str) -> Result<WhisperContext, String> {
    let path = resolve_model_path(model)
        .ok_or_else(|| format!("Model file not found for: {model}"))?;

    let mut params = WhisperContextParameters::default();
    params.use_gpu(true);
    params.flash_attn(true);
    log::info!(
        "Loading whisper model '{}' with GPU + flash attention",
        path.display()
    );
    WhisperContext::new_with_params(path.to_str().unwrap(), params)
        .map_err(|e| format!("Failed to load whisper model: {e}"))
}

/// Download a whisper model from HuggingFace with progress events
pub async fn download_model(model: &str, app_handle: AppHandle) -> Result<(), String> {
    let (filename, url_suffix, _) = model_info(model);
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{url_suffix}"
    );

    let dest = models_dir().join("whisper").join(filename);
    let parent = dest.parent().unwrap();
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("Failed to create models dir: {e}"))?;

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(model_size_bytes(model));
    let mut downloaded: u64 = 0;

    let tmp_path = dest.with_extension("bin.tmp");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("Failed to create file: {e}"))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {e}"))?;

        downloaded += chunk.len() as u64;
        let progress = ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u32;
        let _ = app_handle.emit("model-download-progress", progress);
    }

    tokio::fs::rename(&tmp_path, &dest)
        .await
        .map_err(|e| format!("Failed to finalize download: {e}"))?;

    let _ = app_handle.emit("model-download-progress", 100u32);
    Ok(())
}

/// Run whisper transcription on audio samples
pub fn transcribe(
    ctx: &WhisperContext,
    audio: &[f32],
    language: &str,
) -> Result<String, String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // For .en models, always set English; for multilingual, respect setting
    if language != "auto" {
        params.set_language(Some(language));
    } else {
        // Default to English for speed (skip language detection)
        params.set_language(Some("en"));
    }

    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_no_timestamps(true);
    params.set_single_segment(true);

    // Use all CPU threads for non-GPU work
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);
    params.set_n_threads(n_threads);

    // Speed: disable token-level timestamps
    params.set_token_timestamps(false);

    let mut state = ctx.create_state().map_err(|e| format!("Failed to create state: {e}"))?;

    let start = std::time::Instant::now();
    state
        .full(params, audio)
        .map_err(|e| format!("Transcription failed: {e}"))?;
    let elapsed = start.elapsed();
    log::info!("Whisper inference took {:.0}ms", elapsed.as_millis());

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(s) = segment.to_str_lossy() {
                text.push_str(&s);
            }
        }
    }

    Ok(text.trim().to_string())
}
