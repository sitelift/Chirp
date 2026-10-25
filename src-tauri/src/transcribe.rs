use sherpa_onnx::{
    OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineTransducerModelConfig,
};
use std::path::PathBuf;
use tauri::AppHandle;

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
    model_files_exist(&model_dir(model))
}

/// Load a sherpa-onnx offline recognizer from disk.
///
/// Parakeet is speech recognition only: there is no contextual biasing, so
/// editing Words never rebuilds the recognizer. Spellings reach the cleanup
/// model as its dictionary instead.
pub fn load_model(model: &str, beam_search: bool) -> Result<SherpaRecognizer, String> {
    let requested = std::env::var("CHIRP_ASR_PROVIDER").unwrap_or_else(|_| "auto".into());
    if !matches!(requested.as_str(), "auto" | "cpu" | "cuda") { return Err("CHIRP_ASR_PROVIDER must be auto, cpu or cuda".into()); }
    let cpu_threads = std::thread::available_parallelism().map(|n| n.get().clamp(1,6) as i32).unwrap_or(4);
    let cpu_threads = std::env::var("CHIRP_ASR_THREADS").ok().and_then(|s| s.parse::<i32>().ok()).filter(|n| *n > 0).unwrap_or(cpu_threads);
    let load = |dir: &std::path::Path, provider, threads| {
        let start = std::time::Instant::now();
        let rec = load_model_config(dir, beam_search, provider, threads, false)?;
        log::info!("ASR {provider} load: {:.1}ms", start.elapsed().as_secs_f64()*1000.0);
        let start = std::time::Instant::now();
        transcribe(&rec, &vec![0.0; 16000])?;
        log::info!("ASR {provider} warmup: {:.1}ms", start.elapsed().as_secs_f64()*1000.0);
        Ok::<_,String>(rec)
    };
    if requested != "cpu" && crate::engine::gpu_runtime() {
        crate::engine::remember_failure("CUDA model load/warmup did not finish")?;
        match load(&crate::downloads::fp32_dir(), "cuda", 4) {
            Ok(rec) => { crate::engine::warmed_gpu(); crate::cleanup::warm_up(); return Ok(rec); }
            Err(e) => { crate::engine::disable_gpu(&e); log::warn!("CUDA failed: {e}; using CPU int8"); }
        }
    }
    let dir = std::env::var_os("CHIRP_ASR_MODEL_DIR").map(PathBuf::from).unwrap_or_else(|| model_dir(model));
    let rec = load(&dir, "cpu", cpu_threads)?;
    crate::engine::set_active("cpu");
    crate::cleanup::warm_up();
    Ok(rec)
}

/// Explicit configuration for reproducible benchmarks; fallback may be disabled.
pub fn load_model_config(dir: &std::path::Path, beam_search: bool, provider: &str, n_threads: i32, fallback: bool) -> Result<SherpaRecognizer, String> {
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

    // Older versions wrote a hotwords file next to the model.
    let _ = std::fs::remove_file(dir.join("hotwords.txt"));

    let decoding_method = if beam_search { "modified_beam_search" } else { "greedy_search" };
    log::info!(
        "Loading Parakeet TDT model from {} with {} threads, decoding={}",
        dir.display(),
        n_threads,
        decoding_method,
    );

    let mc = OfflineModelConfig {
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

    let mut config = OfflineRecognizerConfig {
        model_config: mc,
        decoding_method: Some(decoding_method.to_string()),
        max_active_paths: if beam_search { 8 } else { 4 },
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
    if crate::engine::gpu_runtime() { return true; }
    // Benchmarks use isolated DLLs beside their executable.
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS};
    let name: Vec<u16> = "onnxruntime.dll".encode_utf16().chain(Some(0)).collect();
    let mut ort = unsafe { GetModuleHandleW(name.as_ptr()) };
    if ort.is_null() {
        if let Ok(exe) = std::env::current_exe() {
            use std::os::windows::ffi::OsStrExt;
            let path: Vec<u16> = exe.parent().unwrap().join("onnxruntime.dll").as_os_str().encode_wide().chain(Some(0)).collect();
            ort = unsafe { LoadLibraryExW(path.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) };
        }
    }
    !ort.is_null() && unsafe { GetProcAddress(ort, b"OrtSessionOptionsAppendExecutionProvider_CUDA\0".as_ptr()) }.is_some() && crate::engine::nvidia_available()
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

pub fn model_files_exist(dir: &std::path::Path) -> bool {
    dir.join("tokens.txt").is_file() && ["encoder", "decoder", "joiner"].iter().all(|p| find_model_file(dir,p).is_some_and(|f| std::fs::metadata(f).is_ok_and(|m|m.len()>0)))
}

pub async fn download_model(_model: &str, app_handle: AppHandle) -> Result<(), String> {
    crate::downloads::cpu(Some(&app_handle)).await
}

/// Segments shorter than this are never re-split (each half must stay >= 2s).
const DROPOUT_MIN_SAMPLES: usize = 4 * 16000;
/// Dense VAD speech runs ~2-3 words/s; far below this means words were lost.
const DROPOUT_WORDS_PER_SEC: f32 = 0.5;

/// Run transcription on audio samples using sherpa-onnx.
///
/// Parakeet TDT occasionally decodes a long, clearly voiced segment to
/// nothing (both int8 and fp32; observed on a 17.3s VAD segment whose halves
/// transcribe cleanly). When the output is implausibly sparse for the audio
/// length, split at the quietest point near the middle and decode the halves.
pub fn transcribe(
    recognizer: &SherpaRecognizer,
    audio: &[f32],
) -> Result<String, String> {
    let text = decode(recognizer, audio)?;
    if audio.len() < DROPOUT_MIN_SAMPLES {
        return Ok(text);
    }
    let words = text.split_whitespace().count();
    let secs = audio.len() as f32 / 16000.0;
    if words as f32 >= secs * DROPOUT_WORDS_PER_SEC {
        return Ok(text);
    }
    let mid = quietest_split(audio);
    let (Ok(left), Ok(right)) = (transcribe(recognizer, &audio[..mid]), transcribe(recognizer, &audio[mid..])) else {
        return Ok(text);
    };
    let recovered = format!("{left} {right}").trim().to_string();
    let recovered_words = recovered.split_whitespace().count();
    if recovered_words >= 3 && recovered_words > words {
        log::warn!("ASR dropout on {secs:.1}s segment ({words} words); split decode recovered {recovered_words} words");
        Ok(recovered)
    } else {
        Ok(text)
    }
}

/// Index of the lowest-energy 20ms window within the middle half of `audio`,
/// so a re-split lands in a pause rather than mid-word where possible.
fn quietest_split(audio: &[f32]) -> usize {
    const WIN: usize = 320;
    let (lo, hi) = (audio.len() / 4, audio.len() * 3 / 4);
    (lo..hi.saturating_sub(WIN))
        .step_by(WIN / 2)
        .min_by(|&a, &b| {
            let e = |i: usize| audio[i..i + WIN].iter().map(|s| s * s).sum::<f32>();
            e(a).total_cmp(&e(b))
        })
        .map_or(audio.len() / 2, |i| i + WIN / 2)
}

fn decode(
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

    let result = stream.get_result();
    if let Some(trace) = crate::performance::Trace::current() {
        trace.asr(total_start);
        if let Some(capture) = trace.capture() {
            let t = std::time::Instant::now();
            capture.asr_input(audio, result.as_ref().map(|r|r.text.trim()), if result.is_none() {Some("No recognition result")} else {None});
            trace.stage("capture_copy", t);
        }
    }
    let result = result.ok_or_else(|| "No recognition result".to_string())?;
    Ok(result.text.trim().to_string())
}

/// Transcribe a VAD segment with the preferred lead-in. Parakeet sometimes
/// returns nothing depending on where audio starts, so an empty result is
/// retried with no lead-in, then with all of it.
pub fn transcribe_segment(
    recognizer: &SherpaRecognizer,
    segment: &crate::audio::Segment,
) -> Result<String, String> {
    let lead = segment.lead;
    let starts = [lead - lead.min(crate::audio::VAD_LEAD_IN_SAMPLES), lead, 0];
    let mut text = String::new();
    for (i, &from) in starts.iter().enumerate() {
        if starts[..i].contains(&from) { continue; }
        if i > 0 { log::info!("Empty segment transcript; retrying with {from} of {lead} lead-in samples skipped"); }
        text = transcribe(recognizer, &segment.audio[from..])?;
        if !text.is_empty() { break; }
    }
    Ok(text)
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

#[cfg(test)]
mod tests {
    use super::quietest_split;

    #[test]
    fn split_lands_in_the_pause_near_the_middle() {
        let mut audio = vec![0.3f32; 16000 * 10];
        audio[16000 * 6..16000 * 6 + 1600].fill(0.0);
        let at = quietest_split(&audio);
        assert!((16000 * 6..16000 * 6 + 1600).contains(&at), "{at}");
    }

    #[test]
    fn split_stays_in_the_middle_half_when_the_pause_is_at_the_edge() {
        let mut audio = vec![0.3f32; 16000 * 10];
        audio[..16000].fill(0.0);
        let at = quietest_split(&audio);
        assert!((audio.len() / 4..audio.len() * 3 / 4).contains(&at), "{at}");
    }
}
