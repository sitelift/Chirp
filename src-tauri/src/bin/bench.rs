//! Reproducible ASR benchmark. Run from the repository root; see bench/README.md.
use app_lib::{audio, cleanup, transcribe};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Item {
    id: String,
    category: String,
    wav: String,
    spoken: String,
    ideal: String,
    context: serde_json::Value,
}
#[derive(Serialize)]
struct Run {
    llm: Option<serde_json::Value>,
    id: String,
    category: String,
    iteration: usize,
    cold: bool,
    audio_ms: f64,
    raw: String,
    cleaned: String,
    word_errors: usize,
    reference_words: usize,
    wer: f64,
    stages_ms: BTreeMap<String, f64>,
}
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}
fn wer(reference: &str, hypothesis: &str) -> (usize, usize, f64) {
    let a = words(reference);
    let b = words(hypothesis);
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j] + 1).min(old + 1).min(prev + usize::from(x != y));
            prev = old;
        }
    }
    (
        row[b.len()],
        a.len(),
        row[b.len()] as f64 / a.len().max(1) as f64,
    )
}
fn stats(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return json!(null);
    }
    let q = |p: f64| values[((values.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    json!({"p50":q(0.5),"p95":q(0.95),"max":values[values.len()-1]})
}
fn gpu() -> Option<String> {
    std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,driver_version,memory.used",
            "--format=csv,noheader",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
}
fn model_file(dir: &Path, prefix: &str) -> Result<String, String> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "onnx")
                && p.file_name().unwrap().to_string_lossy().contains(prefix)
        })
        .collect();
    files.sort();
    if files.len() != 1 {
        return Err(format!(
            "Expected exactly one {prefix} ONNX in {}: {files:?}",
            dir.display()
        ));
    }
    Ok(files[0].to_string_lossy().into())
}
fn online_decode(
    rec: &sherpa_onnx::OnlineRecognizer,
    samples: &[f32],
    paced: bool,
) -> (String, f64, f64) {
    let stream = rec.create_stream();
    let begin = Instant::now();
    let mut compute = 0.0;
    for (i, chunk) in samples.chunks(1280).enumerate() {
        if paced {
            let deadline = Duration::from_secs_f64(((i * 1280 + chunk.len()) as f64) / 16000.0);
            std::thread::sleep(deadline.saturating_sub(begin.elapsed()));
        }
        let t = Instant::now();
        stream.accept_waveform(16000, chunk);
        while rec.is_ready(&stream) {
            rec.decode(&stream);
        }
        compute += ms(t);
    }
    // Latency includes any backlog beyond the scheduled end of the waveform.
    let backlog = if paced {
        (ms(begin) - samples.len() as f64 / 16.0).max(0.0)
    } else {
        0.0
    };
    let tail = Instant::now();
    stream.accept_waveform(16000, &vec![0.0; 8000]);
    stream.input_finished();
    while rec.is_ready(&stream) {
        rec.decode(&stream);
    }
    let raw = rec.get_result(&stream).map(|r| r.text).unwrap_or_default();
    let tail_ms = ms(tail);
    (raw, compute + tail_ms, backlog + tail_ms)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--mic-lifecycle") {
        return mic_lifecycle();
    }
    let arg = |key: &str, default: &str| {
        args.windows(2)
            .find(|a| a[0] == key)
            .map(|a| a[1].clone())
            .unwrap_or_else(|| default.into())
    };
    let manifest = PathBuf::from(arg("--manifest", "bench/corpus/manifest.json"));
    let iterations: usize = arg("--iterations", "3").parse()?;
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    let provider = arg("--provider", "cpu");
    let threads: i32 = arg("--threads", "4").parse()?;
    let dir = PathBuf::from(arg(
        "--model-dir",
        &transcribe::model_dir("parakeet-tdt-0.6b").to_string_lossy(),
    ));
    let online = args.iter().any(|a| a == "--online");
    let paced = !args.iter().any(|a| a == "--unpaced");
    let vad_path = args
        .windows(2)
        .find(|a| a[0] == "--vad")
        .map(|a| a[1].clone());
    if online && vad_path.is_some() {
        return Err("--vad applies to offline Parakeet only".into());
    }
    let use_llm = args.iter().any(|a| a == "--llm");
    let client = app_lib::llm::LlmClient::from_env();
    if use_llm && !client.has_key() {
        return Err("--llm requires ANTHROPIC_API_KEY".into());
    }
    let mut corpus: Vec<Item> = serde_json::from_slice(&std::fs::read(&manifest)?)?;
    let limit: usize = arg("--limit", "0").parse()?;
    if limit > 0 {
        corpus.truncate(limit);
    }
    if corpus.is_empty() {
        return Err("empty corpus".into());
    }
    let gpu_before = gpu();
    let load = Instant::now();
    let offline = if !online {
        Some(transcribe::load_model_config(
            &dir,
            false,
            &[],
            &provider,
            threads,
            false,
        )?)
    } else {
        None
    };
    let streaming = if online {
        if provider == "cuda" && !transcribe::cuda_runtime_available() {
            return Err("CUDA runtime unavailable".into());
        }
        let mut config = sherpa_onnx::OnlineRecognizerConfig::default();
        config.model_config.transducer.encoder = Some(model_file(&dir, "encoder")?);
        config.model_config.transducer.decoder = Some(model_file(&dir, "decoder")?);
        config.model_config.transducer.joiner = Some(model_file(&dir, "joiner")?);
        config.model_config.tokens = Some(dir.join("tokens.txt").to_string_lossy().into());
        config.model_config.provider = Some(provider.clone());
        config.model_config.num_threads = threads;
        config.decoding_method = Some("greedy_search".into());
        config.enable_endpoint = false;
        Some(sherpa_onnx::OnlineRecognizer::create(&config).ok_or("online model creation failed")?)
    } else {
        None
    };
    let model_load_ms = ms(load);
    let gpu_loaded = gpu();
    let mut runs = Vec::new();
    println!("Model load: {model_load_ms:.1}ms; {provider}, {threads} threads; online={online}");
    println!(
        "{:24} {:>5} {:>10} {:>10} {:>10} {:>8}",
        "file", "run", "ASR ms", "tail ms", "cleanup", "WER"
    );
    // One true first decode, then N warm corpus passes. The first pass is retained separately.
    for iteration in 0..=iterations {
        for (index, item) in corpus.iter().enumerate() {
            if use_llm {
                client.prewarm();
            }
            let pipeline_start = Instant::now();
            let mut stages = BTreeMap::new();
            let t = Instant::now();
            let mut wav = hound::WavReader::open(manifest.parent().unwrap().join(&item.wav))?;
            let spec = wav.spec();
            if spec.channels != 1
                || spec.sample_rate != 16000
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
            {
                return Err(format!("{} must be PCM16 mono 16kHz", item.wav).into());
            }
            let samples: Vec<f32> = wav
                .samples::<i16>()
                .map(|s| s.map(|s| s as f32 / 32768.0))
                .collect::<Result<_, _>>()?;
            stages.insert("wav_read".into(), ms(t));
            stages.insert("resample".into(), 0.0); // Corpus is already 16 kHz mono.
            let (raw, asr, tail) = if let Some(rec) = &streaming {
                online_decode(rec, &samples, paced)
            } else {
                let rec = offline.as_ref().unwrap();
                let mut texts = Vec::new();
                let mut asr_ms = 0.0;
                let mut vad_ms = 0.0;
                let mut decode = |segment: &[f32]| -> Result<(), String> {
                    let t = Instant::now();
                    for chunk in transcribe::chunk_audio(segment, 16000, 30.0, 1.0) {
                        texts.push(transcribe::transcribe(rec, chunk)?);
                    }
                    asr_ms += ms(t);
                    Ok(())
                };
                let tail_ms;
                if let Some(path) = &vad_path {
                    let (tx, rx) = crossbeam_channel::unbounded();
                    let t = Instant::now();
                    let state = Arc::new(Mutex::new(
                        audio::create_vad_state(path, tx).ok_or("VAD creation failed")?,
                    ));
                    stages.insert("vad_load".into(), ms(t));
                    let begin = Instant::now();
                    for (i, chunk) in samples.chunks(160).enumerate() {
                        if paced {
                            let deadline =
                                Duration::from_secs_f64((i * 160 + chunk.len()) as f64 / 16000.0);
                            std::thread::sleep(deadline.saturating_sub(begin.elapsed()));
                        }
                        let t = Instant::now();
                        audio::feed_vad(chunk, &state);
                        vad_ms += ms(t);
                        for segment in rx.try_iter() {
                            decode(&segment)?;
                        }
                    }
                    let backlog = if paced {
                        (ms(begin) - samples.len() as f64 / 16.0).max(0.0)
                    } else {
                        0.0
                    };
                    let tail = Instant::now();
                    let t = Instant::now();
                    audio::flush_vad(&state);
                    vad_ms += ms(t);
                    for segment in rx.try_iter() {
                        decode(&segment)?;
                    }
                    tail_ms = backlog + ms(tail);
                } else {
                    decode(&samples)?;
                    tail_ms = asr_ms;
                }
                stages.insert("vad".into(), vad_ms);
                (transcribe::merge_transcriptions(texts), asr_ms, tail_ms)
            };
            stages.insert("asr".into(), asr);
            stages.insert("end_of_audio_to_text".into(), tail);
            let t = Instant::now();
            let cleaned = cleanup::cleanup_text(&raw, true);
            stages.insert("cleanup".into(), ms(t));
            let t = Instant::now();
            let cleaned = cleanup::apply_replacements(&cleaned, &[]);
            stages.insert("vocabulary".into(), ms(t));
            let mut llm = None;
            let t = Instant::now();
            let cleaned = if use_llm {
                let ctx = app_lib::llm::AppContext {
                    app: item.context["app"].as_str().unwrap_or_default().into(),
                    title: item.context["title"].as_str().unwrap_or_default().into(),
                };
                match tauri::async_runtime::block_on(client.cleanup(
                    &cleaned,
                    &ctx,
                    Duration::from_millis(2500),
                )) {
                    Ok(out) => {
                        llm = Some(serde_json::to_value(&out)?);
                        out.text
                    }
                    Err(e) => {
                        llm = Some(json!({"error":e.to_string()}));
                        cleaned
                    }
                }
            } else {
                cleaned
            };
            stages.insert("llm".into(), ms(t));
            stages.insert("pipeline_wall".into(), ms(pipeline_start));
            stages.insert(
                "end_of_audio_to_final".into(),
                tail + stages["cleanup"] + stages["vocabulary"] + stages["llm"],
            );
            let (errors, reference, wer) = wer(&item.spoken, &raw);
            println!(
                "{:24} {:>5} {:>10.2} {:>10.2} {:>10.2} {:>7.1}%",
                item.id,
                iteration,
                asr,
                tail,
                stages["cleanup"],
                wer * 100.0
            );
            let _ = (&item.ideal, &item.context); // retained in the input manifest; no cloud cleanup in this bench.
            runs.push(Run {
                llm,
                id: item.id.clone(),
                category: item.category.clone(),
                iteration,
                cold: iteration == 0 && index == 0,
                audio_ms: samples.len() as f64 / 16.0,
                raw,
                cleaned,
                word_errors: errors,
                reference_words: reference,
                wer,
                stages_ms: stages,
            });
        }
    }
    let aggregate = |selected: Vec<&Run>| {
        let mut stages = BTreeMap::new();
        for run in &selected {
            for key in run.stages_ms.keys() {
                stages.entry(key.clone()).or_insert_with(|| {
                    stats(
                        selected
                            .iter()
                            .filter_map(|r| r.stages_ms.get(key).copied())
                            .collect(),
                    )
                });
            }
        }
        let errors: usize = selected.iter().map(|r| r.word_errors).sum();
        let words: usize = selected.iter().map(|r| r.reference_words).sum();
        json!({"stages_ms":stages,"wer":errors as f64/words.max(1) as f64,"word_errors":errors,"reference_words":words})
    };
    let warm = aggregate(runs.iter().filter(|r| r.iteration > 0).collect());
    let mut per_file = BTreeMap::new();
    for item in &corpus {
        per_file.insert(
            item.id.clone(),
            aggregate(
                runs.iter()
                    .filter(|r| r.iteration > 0 && r.id == item.id)
                    .collect(),
            ),
        );
    }
    let label = arg(
        "--label",
        &format!(
            "{provider}-{threads}t-{}",
            if online { "online" } else { "offline" }
        ),
    );
    if !label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("label must be alphanumeric, dash or underscore".into());
    }
    let output = PathBuf::from("bench/results");
    std::fs::create_dir_all(&output)?;
    let path = output.join(format!(
        "{}-{label}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
    ));
    let result = json!({"model_dir":dir,"provider_requested":provider,"threads":threads,"online":online,"paced":paced,"manifest":manifest,"iterations":iterations,"limit":limit,"llm_enabled":use_llm,"vad_model":vad_path,"model_load_ms":model_load_ms,"gpu_before":gpu_before,"gpu_loaded":gpu_loaded,"gpu_after":gpu(),"aggregate_warm":warm,"per_file_warm":per_file,"runs":runs,"notes":"GPU memory is device-wide snapshots, not process peak. First corpus pass excluded from warm aggregates. WAV loading excluded from tail. VAD simulation uses the real segmenter; ASR runs synchronously between feeds, so paced tail conservatively includes backlog. No clipboard or microphone in this harness. LLM is opt-in. Zero resample cost: corpus is already 16 kHz."});
    std::fs::write(&path, serde_json::to_vec_pretty(&result)?)?;
    println!(
        "Warm stages (ms): {}",
        serde_json::to_string_pretty(&result["aggregate_warm"])?
    );
    println!("Saved {}", path.display());
    Ok(())
}

/// Does not recognize, persist or paste microphone audio. Measures actual CPAL
/// open/drop versus the app's warm callback gate on the default input device.
fn mic_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::atomic::Ordering;
    let mut cold_open = Vec::new();
    let mut cold_first_sample = Vec::new();
    let mut cold_stop = Vec::new();
    let mut warm_start = Vec::new();
    let mut warm_stop = Vec::new();
    let mut warm_first_sample = Vec::new();
    let mut flush = Vec::new();
    for _ in 0..8 {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let t = Instant::now();
        let (stream, error, active, resampler) =
            audio::start_capture_headless("default", buffer.clone())?;
        cold_open.push(ms(t));
        while buffer.lock().unwrap().is_empty() && t.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if buffer.lock().unwrap().is_empty() {
            return Err("no microphone samples within 1s of opening".into());
        }
        cold_first_sample.push(ms(t));
        std::thread::sleep(Duration::from_millis(250));
        active.store(false, Ordering::SeqCst);
        let t = Instant::now();
        drop(stream);
        cold_stop.push(ms(t));
        let t = Instant::now();
        resampler.flush(&buffer);
        flush.push(ms(t));
        if error.load(Ordering::SeqCst) {
            return Err("device reported a capture error".into());
        }
    }
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (stream, error, active, resampler) =
        audio::start_capture_headless("default", buffer.clone())?;
    active.store(false, Ordering::SeqCst);
    resampler.quiesce();
    // WASAPI Play is asynchronous; let the newly opened device become warm.
    std::thread::sleep(Duration::from_millis(300));
    for _ in 0..8 {
        let t = Instant::now();
        buffer.lock().unwrap().clear();
        resampler.prepare(None);
        active.store(true, Ordering::SeqCst);
        warm_start.push(ms(t));
        while buffer.lock().unwrap().is_empty() && t.elapsed() < Duration::from_millis(100) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if buffer.lock().unwrap().is_empty() {
            return Err("no microphone samples within 100ms".into());
        }
        warm_first_sample.push(ms(t));
        std::thread::sleep(Duration::from_millis(250));
        let t = Instant::now();
        active.store(false, Ordering::SeqCst);
        resampler.quiesce();
        warm_stop.push(ms(t));
        let t = Instant::now();
        resampler.flush(&buffer);
        flush.push(ms(t));
    }
    drop(stream);
    if error.load(Ordering::SeqCst) {
        return Err("device reported a capture error".into());
    }
    let result = json!({"iterations":8,"cold_open_ms":stats(cold_open),"cold_first_sample_ms":stats(cold_first_sample),"cold_stream_drop_ms":stats(cold_stop),"warm_activate_ms":stats(warm_start),"warm_stop_gate_ms":stats(warm_stop),"warm_first_sample_ms":stats(warm_first_sample),"resample_flush_ms":stats(flush),"note":"Default device, 250ms captures; waveform discarded. Actual app capture/resampler code without UI event transport, VAD or ASR. Legacy extra 50ms sleep excluded from measured drop."});
    std::fs::create_dir_all("bench/results")?;
    let path = format!(
        "bench/results/{}-mic-lifecycle.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
    );
    std::fs::write(&path, serde_json::to_vec_pretty(&result)?)?;
    println!("{}\nSaved {path}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_error_edits() {
        assert_eq!(wer("one two three", "one four").0, 2);
        assert_eq!(wer("Hello, WORLD!", "hello world").0, 0);
        assert_eq!(wer("one", "one two").0, 1);
    }
}
