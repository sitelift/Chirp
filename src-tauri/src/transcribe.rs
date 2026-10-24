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
use crate::state::{SherpaRecognizer, VocabEntry};

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

/// Load a sherpa-onnx offline recognizer from disk.
///
/// `vocabulary` is the user's hotword list — proper nouns, technical terms,
/// or any rare words the ASR should bias toward. It's baked into the
/// recognizer at construction time as an Aho-Corasick contextual biasing
/// automaton. Vocabulary changes require this function to be called again
/// (i.e., the recognizer must be reloaded).
///
/// Hotwords ONLY take effect with `modified_beam_search` decoding. With
/// greedy decoding, sherpa-onnx silently ignores them — we log a warning
/// when this combination is requested.
pub fn load_model(
    model: &str,
    beam_search: bool,
    vocabulary: &[VocabEntry],
) -> Result<SherpaRecognizer, String> {
    let requested = std::env::var("CHIRP_ASR_PROVIDER").unwrap_or_else(|_| "auto".into());
    if !matches!(requested.as_str(), "auto" | "cpu" | "cuda") { return Err("CHIRP_ASR_PROVIDER must be auto, cpu or cuda".into()); }
    let fp32 = models_dir().join("sherpa/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3");
    let fp32_ready = ["encoder.onnx", "encoder.weights", "decoder.onnx", "joiner.onnx", "tokens.txt"].iter().all(|f| fp32.join(f).exists());
    let provider = if requested == "auto" { if fp32_ready && cuda_runtime_available() { "cuda" } else { "cpu" } } else { requested.as_str() };
    let default_threads = if provider == "cuda" { 4 } else { std::thread::available_parallelism().map(|n| (n.get()/2).clamp(2,6) as i32).unwrap_or(4) };
    let threads = std::env::var("CHIRP_ASR_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(default_threads);
    let dir = std::env::var_os("CHIRP_ASR_MODEL_DIR").map(PathBuf::from).unwrap_or_else(|| if provider == "cuda" && fp32_ready { fp32 } else { model_dir(model) });
    let load_start = std::time::Instant::now();
    let (rec, effective_provider) = match load_model_config(&dir, beam_search, vocabulary, provider, threads, false) {
        Ok(rec) => (rec, provider),
        Err(e) if provider == "cuda" => {
            log::warn!("CUDA load failed ({e}); falling back to CPU int8");
            (load_model_config(&model_dir(model), beam_search, vocabulary, "cpu", threads, false)?, "cpu")
        }
        Err(e) => return Err(e),
    };
    log::info!("ASR model load: {:.1}ms (requested={requested}, selected={effective_provider})", load_start.elapsed().as_secs_f64()*1000.0);
    let start = std::time::Instant::now();
    transcribe(&rec, &vec![0.0; 16000])?;
    log::info!("ASR warmup: {:.1}ms", start.elapsed().as_secs_f64() * 1000.0);
    let _ = crate::cleanup::cleanup_text("warm up the deterministic cleanup", true);
    Ok(rec)
}

/// Explicit configuration for reproducible benchmarks; fallback may be disabled.
pub fn load_model_config(dir: &std::path::Path, beam_search: bool, vocabulary: &[VocabEntry], provider: &str, n_threads: i32, fallback: bool) -> Result<SherpaRecognizer, String> {
    if !matches!(provider, "cpu" | "cuda") || n_threads < 1 { return Err("Expected cpu/cuda and positive thread count".into()); }
    let provider = if provider == "cuda" && !cuda_runtime_available() {
        if !fallback { return Err("CUDA provider/driver dependencies cannot be loaded; use bench/run.ps1 -Provider cuda".into()); }
        log::warn!("CUDA runtime unavailable; using CPU");
        "cpu"
    } else { provider };
    if !dir.exists() {
        return Err(format!("Model directory not found: {}", dir.display()));
    }

    let tokens = dir.join("tokens.txt");
    if !tokens.exists() {
        return Err("tokens.txt not found".to_string());
    }


    // Parakeet TDT uses transducer (encoder/decoder/joiner)
    let encoder = find_model_file(&dir, "encoder")
        .ok_or_else(|| "Encoder model file not found".to_string())?;
    let decoder = find_model_file(&dir, "decoder")
        .ok_or_else(|| "Decoder model file not found".to_string())?;
    let joiner = find_model_file(&dir, "joiner")
        .ok_or_else(|| "Joiner model file not found".to_string())?;

    // Hotwords contextual biasing setup. Per sherpa-onnx docs, four config
    // items must all be set at construction time for hotwords to take effect:
    //   1. modeling_unit (BPE for Parakeet)
    //   2. bpe_vocab (the model's BPE vocab file)
    //   3. hotwords_file (one phrase per line, optional ":score" suffix)
    //   4. hotwords_score (global boost, overridden by per-line scores)
    let bpe_vocab_path = dir.join("bpe.vocab");
    let hotwords_path = dir.join("hotwords.txt");

    let vocab_filtered: Vec<&str> = vocabulary
        .iter()
        .map(|e| e.term.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Hotwords biasing only works with modified_beam_search. Passing a
    // hotwords_file while decoding_method = greedy_search crashes
    // sherpa-onnx (STATUS_ACCESS_VIOLATION) rather than being ignored, so
    // we must actively suppress hotwords when beam_search is off.
    let mut hotwords_active = false;
    if !vocab_filtered.is_empty() && beam_search {
        let mut content = String::new();
        for term in &vocab_filtered {
            content.push_str(term);
            content.push('\n');
        }
        if let Err(e) = std::fs::write(&hotwords_path, content) {
            log::warn!("Failed to write hotwords file: {e}");
        } else {
            hotwords_active = true;
        }
    }
    if !hotwords_active && hotwords_path.exists() {
        let _ = std::fs::remove_file(&hotwords_path);
    }

    let decoding_method = if beam_search { "modified_beam_search" } else { "greedy_search" };

    if !vocab_filtered.is_empty() && !beam_search {
        log::warn!(
            "Vocabulary has {} terms but beam search is OFF — hotwords biasing disabled (requires modified_beam_search)",
            vocab_filtered.len()
        );
    }

    log::info!(
        "Loading Parakeet TDT model from {} with {} threads, decoding={}, hotwords={} ({} terms)",
        dir.display(),
        n_threads,
        decoding_method,
        if hotwords_active { "on" } else { "off" },
        vocab_filtered.len(),
    );

    let mut mc = OfflineModelConfig {
        transducer: OfflineTransducerModelConfig {
            encoder: Some(encoder.to_string_lossy().into_owned()),
            decoder: Some(decoder.to_string_lossy().into_owned()),
            joiner: Some(joiner.to_string_lossy().into_owned()),
        },
        tokens: Some(tokens.to_string_lossy().into_owned()),
        num_threads: n_threads,
        provider: Some(provider.to_string()),
        debug: false,
        ..Default::default()
    };

    if hotwords_active && bpe_vocab_path.exists() {
        mc.modeling_unit = Some("bpe".to_string());
        mc.bpe_vocab = Some(bpe_vocab_path.to_string_lossy().into_owned());
    } else if hotwords_active {
        log::warn!(
            "Hotwords requested but bpe.vocab not found at {} — biasing will not work",
            bpe_vocab_path.display()
        );
    }

    let mut config = OfflineRecognizerConfig {
        model_config: mc,
        decoding_method: Some(decoding_method.to_string()),
        max_active_paths: if beam_search { 8 } else { 4 },
        hotwords_file: if hotwords_active {
            Some(hotwords_path.to_string_lossy().into_owned())
        } else {
            None
        },
        hotwords_score: if hotwords_active { 1.5 } else { 0.0 },
        ..Default::default()
    };
    let rec = OfflineRecognizer::create(&config);
    let rec = if rec.is_none() && provider == "cuda" && fallback {
        log::warn!("CUDA failed; retrying CPU");
        config.model_config.provider = Some("cpu".into());
        OfflineRecognizer::create(&config)
    } else { rec };
    rec.map(SherpaRecognizer)
        .ok_or_else(|| "Failed to create sherpa-onnx recognizer — check model files".to_string())
}

/// Catch absent driver/runtime DLLs before constructing ONNX sessions. A native
/// inference crash cannot be caught by Rust; incompatible models remain opt-in.
pub fn cuda_runtime_available() -> bool {
    use windows_sys::Win32::System::LibraryLoader::{LoadLibraryW, GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::Foundation::FreeLibrary;
    // A CPU rebuild may leave old CUDA dependency files beside the executable.
    // Check the loaded ORT core too (the pinned CPU build lacks this export).
    let ort_name: Vec<u16> = "onnxruntime.dll".encode_utf16().chain(Some(0)).collect();
    let ort = unsafe { GetModuleHandleW(ort_name.as_ptr()) };
    if ort.is_null() || unsafe { GetProcAddress(ort, b"OrtSessionOptionsAppendExecutionProvider_CUDA\0".as_ptr()) }.is_none() { return false; }
    // ORT must initialize its shared provider host before loading the CUDA
    // provider itself. Loading that DLL as a preflight can fail even on a
    // working runtime; inspect its presence and probe only standalone deps.
    let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_owned())) else { return false; };
    if !exe_dir.join("onnxruntime_providers_cuda.dll").is_file() { return false; }
    for name in ["nvcuda.dll", "cublasLt64_12.dll", "cublas64_12.dll", "cufft64_11.dll", "cudart64_12.dll", "cudnn64_9.dll"] {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let module = unsafe { LoadLibraryW(name.as_ptr()) };
        if module.is_null() { return false; }
        unsafe { FreeLibrary(module); }
    }
    true
}

/// Find a model file matching a prefix (e.g. "encoder") with .onnx extension
fn find_model_file(dir: &std::path::Path, prefix: &str) -> Option<PathBuf> {
    let mut matches = std::fs::read_dir(dir).ok()?.flatten().map(|entry| entry.path()).filter(|path| {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        name.contains(prefix) && name.ends_with(".onnx")
    });
    let first = matches.next()?;
    if matches.next().is_some() { return None; } // Never choose precision by directory iteration order.
    Some(first)
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

/// Run transcription on audio samples using sherpa-onnx. Hotwords biasing is
/// configured at recognizer construction time (see `load_model`) — there is
/// no per-call hotword override.
pub fn transcribe(
    recognizer: &SherpaRecognizer,
    audio: &[f32],
) -> Result<String, String> {
    let total_start = std::time::Instant::now();
    let stream = recognizer.0.create_stream();
    stream.accept_waveform(16000, audio);

    let start = std::time::Instant::now();
    recognizer.0.decode(&stream);
    let elapsed = start.elapsed();
    log::info!("Sherpa-onnx inference took {:.0}ms", elapsed.as_millis());

    let result = stream
        .get_result()
        .ok_or_else(|| "No recognition result".to_string())?;

    if let Some(trace) = crate::performance::Trace::current() { trace.asr(total_start); }
    Ok(result.text.trim().to_string())
}

/// Split audio into overlapping chunks for transcription
pub fn chunk_audio(samples: &[f32], sample_rate: u32, chunk_secs: f32, overlap_secs: f32) -> Vec<&[f32]> {
    let chunk_samples = (chunk_secs * sample_rate as f32) as usize;
    let overlap_samples = (overlap_secs * sample_rate as f32) as usize;
    let step = chunk_samples - overlap_samples;

    if samples.len() <= chunk_samples {
        return vec![samples];
    }

    let mut chunks = Vec::new();
    let mut pos = 0;

    while pos < samples.len() {
        let end = (pos + chunk_samples).min(samples.len());
        chunks.push(&samples[pos..end]);
        if end >= samples.len() {
            break;
        }
        pos += step;
    }

    chunks
}

/// Merge transcriptions from overlapping chunks, deduplicating words at boundaries.
/// Finds the longest suffix of chunk N that matches a prefix of chunk N+1 and removes it.
pub fn merge_transcriptions(segments: Vec<String>) -> String {
    let segments: Vec<&str> = segments.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return String::new();
    }

    let mut merged = segments[0].to_string();

    for next in &segments[1..] {
        let prev_words: Vec<&str> = merged.split_whitespace().collect();
        let next_words: Vec<&str> = next.split_whitespace().collect();

        // Look for the longest suffix of prev that matches a prefix of next.
        // Only check up to 8 words (overlap region is small).
        let max_check = prev_words.len().min(next_words.len()).min(8);
        let mut best_overlap = 0;

        for len in 1..=max_check {
            let suffix = &prev_words[prev_words.len() - len..];
            let prefix = &next_words[..len];
            if suffix.iter().zip(prefix.iter()).all(|(a, b)| {
                a.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation())
                    == b.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation())
            }) {
                best_overlap = len;
            }
        }

        if best_overlap > 0 {
            let remainder = next_words[best_overlap..].join(" ");
            if !remainder.is_empty() {
                merged.push(' ');
                merged.push_str(&remainder);
            }
        } else {
            merged.push(' ');
            merged.push_str(next);
        }
    }

    merged
}
