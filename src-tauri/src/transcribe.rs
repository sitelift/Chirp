use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineTransducerModelConfig,
};
use std::path::PathBuf;
use tar::Archive;
use tauri::{AppHandle, Emitter};

use crate::settings::models_dir;
use crate::state::SherpaRecognizer;

/// Model metadata: (archive name, download url, extracted dir name, size in bytes)
fn model_info(model: &str) -> (&'static str, &'static str, &'static str, u64) {
    match model {
        "parakeet-tdt-0.6b" => (
            "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
            "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
            487_000_000,
        ),
        _ => model_info("parakeet-tdt-0.6b"),
    }
}

/// Model file sizes for status reporting
pub fn model_size_bytes(model: &str) -> u64 {
    model_info(model).3
}

/// Get the expected path for the model directory
pub fn model_dir(model: &str) -> PathBuf {
    let (_, _, dir_name, _) = model_info(model);
    models_dir().join("sherpa").join(dir_name)
}

/// Check if a model exists on disk
pub fn model_exists(model: &str) -> bool {
    let dir = model_dir(model);
    dir.join("tokens.txt").exists()
}

/// Load a sherpa-onnx offline recognizer from disk
pub fn load_model(model: &str) -> Result<SherpaRecognizer, String> {
    let dir = model_dir(model);
    if !dir.exists() {
        return Err(format!("Model directory not found: {}", dir.display()));
    }

    // Find model files — Parakeet TDT uses transducer (encoder/decoder/joiner)
    let encoder = find_model_file(&dir, "encoder")
        .ok_or_else(|| "Encoder model file not found".to_string())?;
    let decoder = find_model_file(&dir, "decoder")
        .ok_or_else(|| "Decoder model file not found".to_string())?;
    let joiner = find_model_file(&dir, "joiner")
        .ok_or_else(|| "Joiner model file not found".to_string())?;
    let tokens = dir.join("tokens.txt");
    if !tokens.exists() {
        return Err("tokens.txt not found".to_string());
    }

    // Use at most half the available threads (min 2, max 6) so the LLM server
    // and OS have headroom. On the Intel Core Ultra 7 256V (8 threads) this
    // gives sherpa 4 threads instead of 8.
    let n_threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 6) as i32)
        .unwrap_or(4);

    log::info!(
        "Loading Parakeet TDT model from {} with {} threads",
        dir.display(),
        n_threads
    );

    let config = OfflineRecognizerConfig {
        model_config: OfflineModelConfig {
            transducer: OfflineTransducerModelConfig {
                encoder: Some(encoder.to_string_lossy().into_owned()),
                decoder: Some(decoder.to_string_lossy().into_owned()),
                joiner: Some(joiner.to_string_lossy().into_owned()),
            },
            tokens: Some(tokens.to_string_lossy().into_owned()),
            num_threads: n_threads,
            provider: Some("cpu".to_string()),
            debug: false,
            ..Default::default()
        },
        decoding_method: Some("greedy_search".to_string()),
        ..Default::default()
    };

    OfflineRecognizer::create(&config)
        .map(SherpaRecognizer)
        .ok_or_else(|| "Failed to create sherpa-onnx recognizer — check model files".to_string())
}

/// Find a model file matching a prefix (e.g. "encoder") with .onnx extension
fn find_model_file(dir: &std::path::Path, prefix: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(prefix) && name.ends_with(".onnx") {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Download a model archive and extract it
pub async fn download_model(model: &str, app_handle: AppHandle) -> Result<(), String> {
    let (archive_name, url, _, _) = model_info(model);
    let sherpa_dir = models_dir().join("sherpa");

    tokio::fs::create_dir_all(&sherpa_dir)
        .await
        .map_err(|e| format!("Failed to create models dir: {e}"))?;

    let tmp_path = sherpa_dir.join(format!("{archive_name}.tmp"));
    let archive_path = sherpa_dir.join(archive_name);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(model_size_bytes(model));
    let mut downloaded: u64 = 0;

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
        // Reserve last 5% for extraction
        let progress = ((downloaded as f64 / total_size as f64) * 95.0).min(95.0) as u32;
        let _ = app_handle.emit("model-download-progress", progress);
    }

    drop(file);

    tokio::fs::rename(&tmp_path, &archive_path)
        .await
        .map_err(|e| format!("Failed to finalize download: {e}"))?;

    // Extract the tar.bz2 archive
    let _ = app_handle.emit("model-download-progress", 96u32);

    let extract_dir = sherpa_dir.clone();
    let archive_path_clone = archive_path.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path_clone)
            .map_err(|e| format!("Failed to open archive: {e}"))?;
        let decompressor = BzDecoder::new(file);
        let mut archive = Archive::new(decompressor);

        // Manually iterate entries to validate paths (prevent directory traversal)
        for entry in archive.entries().map_err(|e| format!("Failed to read tar: {e}"))? {
            let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {e}"))?;
            let path = entry.path()
                .map_err(|e| format!("Invalid tar path: {e}"))?
                .to_path_buf();

            // Reject paths with ".." components
            if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                log::warn!("Skipping suspicious archive path: {}", path.display());
                continue;
            }

            let full_path = extract_dir.join(&path);

            if entry.header().entry_type().is_dir() {
                std::fs::create_dir_all(&full_path)
                    .map_err(|e| format!("Failed to create dir {}: {e}", path.display()))?;
            } else if entry.header().entry_type().is_file() {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent dir: {e}"))?;
                }
                let mut out = std::fs::File::create(&full_path)
                    .map_err(|e| format!("Failed to create {}: {e}", path.display()))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| format!("Failed to extract {}: {e}", path.display()))?;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Extract task failed: {e}"))??;

    // Clean up the archive file
    if let Err(e) = tokio::fs::remove_file(&archive_path).await {
        log::warn!("Failed to clean up model archive: {e}");
    }

    // Verify critical model files were extracted
    let extracted_dir = model_dir(model);
    if !extracted_dir.join("tokens.txt").exists() {
        return Err("Model extraction incomplete: tokens.txt missing".to_string());
    }

    let _ = app_handle.emit("model-download-progress", 100u32);
    Ok(())
}

/// Run transcription on audio samples using sherpa-onnx
pub fn transcribe(
    recognizer: &SherpaRecognizer,
    audio: &[f32],
) -> Result<String, String> {
    let stream = recognizer.0.create_stream();
    stream.accept_waveform(16000, audio);

    let start = std::time::Instant::now();
    recognizer.0.decode(&stream);
    let elapsed = start.elapsed();
    log::info!("Sherpa-onnx inference took {:.0}ms", elapsed.as_millis());

    let result = stream
        .get_result()
        .ok_or_else(|| "No recognition result".to_string())?;

    Ok(result.text.trim().to_string())
}
