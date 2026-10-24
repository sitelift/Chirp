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
    #[serde(default)]
    dictionary_terms: Vec<String>,
    #[serde(default)]
    needs_review: bool,
}
#[derive(Serialize)]
struct Run {
    speculation: Option<app_lib::speculative::Metrics>,
    baseline_llm: Option<serde_json::Value>,
    route: String,
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
// Stable non-security fingerprints identify the exact compiled prompt/client and
// loaded manifest without embedding their contents in each performance artifact.
fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}
type CloudResult = Result<app_lib::llm::LlmOutcome, String>;
async fn routed_cleanup(
    client: &app_lib::llm::LlmClient,
    ctx: &app_lib::llm::AppContext,
    text: &str,
    terms: &[String],
    speculation: Option<&app_lib::speculative::Speculation>,
    compare: bool,
) -> (
    CloudResult,
    Option<app_lib::speculative::Metrics>,
    f64,
    Option<(CloudResult, f64)>,
) {
    let start = Instant::now();
    let normal = || async {
        client
            .cleanup_with(text, ctx, terms, Duration::from_millis(2500))
            .await
            .map_err(|e| e.to_string())
    };
    if let Some(spec) = speculation {
        if compare {
            // Paired counterfactuals share audio/ASR and start at the same release.
            // Retain individual completion times: waiting for the slower control
            // must not inflate the speculative path's latency.
            let ((candidate, metrics, spec_ms), (baseline, baseline_ms)) =
                futures_util::future::join(
                    async {
                        let (out, metrics) = spec.finish(text).await;
                        (out, metrics, ms(start))
                    },
                    async {
                        let out = normal().await;
                        (out, ms(start))
                    },
                )
                .await;
            return match candidate {
                Some(out) => (out, Some(metrics), spec_ms, Some((baseline, baseline_ms))),
                None => (
                    baseline.clone(),
                    Some(metrics),
                    baseline_ms,
                    Some((baseline, baseline_ms)),
                ),
            };
        }
        let (candidate, metrics) = spec.finish(text).await;
        let out = match candidate {
            Some(out) => out,
            None => normal().await,
        };
        return (out, Some(metrics), ms(start), None);
    }
    (normal().await, None, ms(start), None)
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
    let requested = arg("--provider", "auto");
    let fp32 = transcribe::model_dir("parakeet-tdt-0.6b")
        .parent()
        .unwrap()
        .join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3");
    let mut provider = if requested == "auto" {
        if [
            "encoder.onnx",
            "encoder.weights",
            "decoder.onnx",
            "joiner.onnx",
            "tokens.txt",
        ]
        .iter()
        .all(|f| fp32.join(f).exists())
            && transcribe::cuda_runtime_available()
        {
            "cuda"
        } else {
            "cpu"
        }
        .to_string()
    } else {
        requested.clone()
    };
    let threads: i32 = arg("--threads", if provider == "cuda" { "4" } else { "6" }).parse()?;
    let mut dir = PathBuf::from(arg(
        "--model-dir",
        &if provider == "cuda" {
            fp32
        } else {
            transcribe::model_dir("parakeet-tdt-0.6b")
        }
        .to_string_lossy(),
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
    let compare_speculation = args.iter().any(|a| a == "--compare-speculation");
    let speculate = compare_speculation || args.iter().any(|a| a == "--speculate");
    if speculate && (!use_llm || vad_path.is_none() || !paced) {
        return Err("speculation requires --llm, --vad and paced input".into());
    }
    let trim_silence: Option<f64> = args
        .windows(2)
        .find(|a| a[0] == "--trailing-silence-ms")
        .map(|a| a[1].parse())
        .transpose()?;
    if trim_silence.is_some_and(|v| !v.is_finite() || v < 0.0) {
        return Err("trailing silence must be nonnegative".into());
    }
    let warm_only = args.iter().any(|a| a == "--warm-only");
    let client = app_lib::llm::LlmClient::from_env();
    if use_llm && !client.has_key() {
        return Err("--llm requires ANTHROPIC_API_KEY".into());
    }
    let manifest_bytes = std::fs::read(&manifest)?;
    let manifest_fingerprint = fingerprint(&manifest_bytes);
    let mut corpus: Vec<Item> = serde_json::from_slice(&manifest_bytes)?;
    let min_seconds: f64 = arg("--min-seconds", "0").parse()?;
    let max_seconds: f64 = arg("--max-seconds", "100000").parse()?;
    let mut filtered = Vec::new();
    for item in corpus {
        let wav = hound::WavReader::open(manifest.parent().unwrap().join(&item.wav))?;
        let seconds = wav.duration() as f64 / wav.spec().sample_rate as f64;
        if seconds >= min_seconds && seconds <= max_seconds {
            filtered.push(item);
        }
    }
    corpus = filtered;
    let limit: usize = arg("--limit", "0").parse()?;
    if limit > 0 {
        corpus.truncate(limit);
    }
    if corpus.is_empty() {
        return Err("empty corpus".into());
    }
    if corpus.iter().any(|item| item.needs_review) {
        return Err("Human references are drafts: correct spoken/ideal and set needs_review=false before scoring them".into());
    }
    let gpu_before = gpu();
    let load = Instant::now();
    let offline = if !online {
        let loaded = transcribe::load_model_config(&dir, false, &[], &provider, threads, false);
        Some(match loaded {
            Ok(rec) => rec,
            Err(e) if requested == "auto" && provider == "cuda" => {
                eprintln!("CUDA load failed: {e}; falling back to CPU int8");
                provider = "cpu".into();
                dir = transcribe::model_dir("parakeet-tdt-0.6b");
                transcribe::load_model_config(&dir, false, &[], "cpu", threads, false)?
            }
            Err(e) => return Err(e.into()),
        })
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
    let warmup = Instant::now();
    if let Some(rec) = &offline {
        transcribe::transcribe(rec, &vec![0.0; 16000])?;
        let _ = cleanup::cleanup_text("warm up the deterministic cleanup", true);
    }
    let warmup_ms = ms(warmup);
    let gpu_loaded = gpu();
    let mut runs = Vec::new();
    println!("Model load: {model_load_ms:.1}ms; {provider}, {threads} threads; online={online}");
    println!(
        "{:24} {:>5} {:>10} {:>10} {:>10} {:>8}",
        "file", "run", "ASR ms", "tail ms", "cleanup", "WER"
    );
    // One true first decode, then N warm corpus passes. The first pass is retained separately.
    for iteration in usize::from(warm_only)..=iterations {
        for (index, item) in corpus.iter().enumerate() {
            let ctx = app_lib::llm::AppContext {
                app: item.context["app"].as_str().unwrap_or_default().into(),
                title: item.context["title"].as_str().unwrap_or_default().into(),
                url: item.context["url"].as_str().map(Into::into),
            };
            let speculation = speculate.then(|| {
                app_lib::speculative::Speculation::new_with_terms(
                    client.clone(),
                    ctx.clone(),
                    item.dictionary_terms.clone(),
                    Duration::from_millis(2500),
                )
            });
            if use_llm {
                client.prewarm_with(&item.dictionary_terms);
            }
            let pipeline_start = Instant::now();
            let mut stages = BTreeMap::new();
            let t = Instant::now();
            let mut wav = hound::WavReader::open(manifest.parent().unwrap().join(&item.wav))?;
            let spec = wav.spec();
            if spec.channels != 1
                || spec.sample_rate != 16000
                || !matches!(
                    (spec.sample_format, spec.bits_per_sample),
                    (hound::SampleFormat::Int, 16) | (hound::SampleFormat::Float, 32)
                )
            {
                return Err(format!("{} must be PCM16 or float32 mono 16kHz", item.wav).into());
            }
            let mut samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
                wav.samples::<f32>().collect::<Result<_, _>>()?
            } else {
                wav.samples::<i16>()
                    .map(|s| s.map(|s| s as f32 / 32768.0))
                    .collect::<Result<_, _>>()?
            };
            if samples.iter().any(|s| !s.is_finite()) {
                return Err(format!("{} contains non-finite audio samples", item.wav).into());
            }
            if let Some(quiet_ms) = trim_silence {
                // Preserve every nonzero PCM sample; only remove digital silence.
                if let Some(last) = samples.iter().rposition(|s| *s != 0.0) {
                    samples.truncate((last + 1 + (quiet_ms * 16.0) as usize).min(samples.len()));
                }
            }
            stages.insert("wav_read".into(), ms(t));
            stages.insert("resample".into(), 0.0); // Corpus is already 16 kHz mono.
            let (raw, asr, tail, precleaned) = if let Some(rec) = &streaming {
                let (raw, asr, tail) = online_decode(rec, &samples, paced);
                (raw, asr, tail, None)
            } else {
                let rec = offline.as_ref().unwrap();
                let mut texts = Vec::new();
                let mut cleaned_segments = Vec::new();
                let mut asr_ms = 0.0;
                let mut asr_capture_ms = 0.0;
                let mut cleanup_capture_ms = 0.0;
                let mut vad_ms = 0.0;
                let mut decode = |segment: &[f32],
                                  release_at: Option<Instant>|
                 -> Result<(), String> {
                    let t = Instant::now();
                    let mut chunks = Vec::new();
                    for chunk in transcribe::chunk_audio(segment, 16000, 30.0, 1.0) {
                        chunks.push(transcribe::transcribe(rec, chunk)?);
                    }
                    let elapsed = ms(t);
                    asr_ms += elapsed;
                    if let Some(release_at) = release_at {
                        asr_capture_ms += elapsed
                            .min(release_at.saturating_duration_since(t).as_secs_f64() * 1000.0);
                    }
                    let raw = transcribe::merge_transcriptions(chunks);
                    if vad_path.is_some() && !raw.is_empty() {
                        let t = Instant::now();
                        cleaned_segments.push(cleanup::apply_replacements(
                            &cleanup::cleanup_text(&raw, true),
                            &[],
                        ));
                        cleanup_capture_ms += ms(t);
                        if let Some(spec) = &speculation {
                            if release_at.is_none_or(|at| Instant::now() >= at) {
                                spec.seal();
                            }
                            let joined = cleanup::join_cleaned_segments_with_formatting(
                                &cleaned_segments,
                                true,
                            );
                            spec.offer(cleanup::repair_vad_boundary_artifacts(&joined));
                        }
                    }
                    texts.push(raw);
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
                            decode(
                                &segment,
                                Some(
                                    begin + Duration::from_secs_f64(samples.len() as f64 / 16000.0),
                                ),
                            )?;
                        }
                    }
                    let backlog = if paced {
                        (ms(begin) - samples.len() as f64 / 16.0).max(0.0)
                    } else {
                        0.0
                    };
                    let tail = Instant::now();
                    if let Some(spec) = &speculation {
                        spec.seal();
                    }
                    let t = Instant::now();
                    audio::flush_vad(&state);
                    vad_ms += ms(t);
                    for segment in rx.try_iter() {
                        decode(&segment, None)?;
                    }
                    tail_ms = backlog + ms(tail);
                } else {
                    decode(&samples, None)?;
                    tail_ms = asr_ms;
                }
                stages.insert("vad".into(), vad_ms);
                stages.insert("asr_during_capture".into(), asr_capture_ms);
                stages.insert("asr_at_release".into(), asr_ms - asr_capture_ms);
                stages.insert("segment_cleanup".into(), cleanup_capture_ms);
                let precleaned = if vad_path.is_some() {
                    Some(cleaned_segments)
                } else {
                    None
                };
                (texts.join(" "), asr_ms, tail_ms, precleaned)
            };
            stages.insert("asr".into(), asr);
            stages.insert("end_of_audio_to_text".into(), tail);
            let post_asr = Instant::now();
            let t = Instant::now();
            let cleaned = if let Some(segments) = precleaned {
                cleanup::join_cleaned_segments_with_formatting(&segments, true)
            } else {
                cleanup::cleanup_text(&raw, true)
            };
            stages.insert("cleanup".into(), ms(t));
            let t = Instant::now();
            let cleaned = cleanup::apply_replacements(&cleaned, &[]);
            stages.insert("vocabulary".into(), ms(t));
            let t = Instant::now();
            let cleaned = cleanup::repair_vad_boundary_artifacts(&cleaned);
            stages.insert("boundary_and_snippets".into(), ms(t)); // Empty snippet configuration.
            let route = if app_lib::router::needs_llm(&cleaned, &ctx.app) {
                "haiku"
            } else {
                "skip"
            };
            let mut llm = None;
            let mut spec_metrics = None;
            let mut baseline_llm = None;
            let mut llm_ms = 0.0;
            let deterministic_ms = ms(post_asr);
            let cleaned = if use_llm && route == "haiku" {
                let (out, metrics, elapsed, baseline) =
                    tauri::async_runtime::block_on(routed_cleanup(
                        &client,
                        &ctx,
                        &cleaned,
                        &item.dictionary_terms,
                        speculation.as_ref(),
                        compare_speculation,
                    ));
                llm_ms = elapsed;
                spec_metrics = metrics;
                if let Some((out, elapsed)) = baseline {
                    stages.insert("baseline_llm".into(), elapsed);
                    stages.insert(
                        "baseline_end_of_audio_to_final".into(),
                        tail + deterministic_ms + elapsed,
                    );
                    baseline_llm = Some(match out {
                        Ok(out) => serde_json::to_value(out)?,
                        Err(e) => json!({"error":e}),
                    });
                }
                match out {
                    Ok(out) => {
                        stages.insert("llm_ttft".into(), out.ttft_ms as f64);
                        stages.insert("llm_service_total".into(), out.total_ms as f64);
                        llm = Some(serde_json::to_value(&out)?);
                        out.text
                    }
                    Err(e) => {
                        llm = Some(json!({"error":e.to_string()}));
                        cleaned
                    }
                }
            } else {
                if let Some(spec) = &speculation {
                    let (_, metrics) = tauri::async_runtime::block_on(spec.finish(&cleaned));
                    spec_metrics = Some(metrics);
                }
                cleaned
            };
            stages.insert("llm".into(), llm_ms);
            stages.insert("pipeline_wall".into(), ms(pipeline_start));
            stages.insert(
                "end_of_audio_to_final".into(),
                tail + deterministic_ms + llm_ms,
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
                speculation: spec_metrics,
                baseline_llm,
                route: route.into(),
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
        json!({"samples":selected.len(),"speculative_hits":selected.iter().filter(|r|r.speculation.as_ref().is_some_and(|s|s.hit)).count(),"speculative_calls":selected.iter().filter_map(|r|r.speculation.as_ref().map(|s|s.calls)).sum::<usize>(),"speculative_overlap_ms":stats(selected.iter().filter_map(|r|r.speculation.as_ref().map(|s|s.time_saved_ms)).collect()),"baseline_llm_errors":selected.iter().filter(|r|r.baseline_llm.as_ref().is_some_and(|v|v.get("error").is_some())).count(),"llm_errors":selected.iter().filter(|r| r.llm.as_ref().is_some_and(|v| v.get("error").is_some())).count(),"stages_ms":stages,"wer":if words>0 {Some(errors as f64/words as f64)}else{None},"word_errors":errors,"reference_words":words})
    };
    let warm = aggregate(runs.iter().filter(|r| r.iteration > 0).collect());
    let groups = json!({"skip":aggregate(runs.iter().filter(|r|r.iteration>0 && r.route=="skip").collect()),"haiku":aggregate(runs.iter().filter(|r|r.iteration>0 && r.route=="haiku").collect())});
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
    let result = json!({"model_dir":dir,"provider_requested":provider,"threads":threads,"online":online,"paced":paced,"manifest":manifest,"manifest_fingerprint":manifest_fingerprint,"prompt_fingerprint":fingerprint(include_bytes!("../prompts/cleanup_system.md")),"llm_client_fingerprint":fingerprint(include_bytes!("../llm.rs")),"iterations":iterations,"limit":limit,"llm_enabled":use_llm,"speculation_enabled":speculate,"paired_control":compare_speculation,"warm_only":warm_only,"trailing_silence_ms":trim_silence,"vad_model":vad_path,"model_load_ms":model_load_ms,"warmup_ms":warmup_ms,"gpu_before":gpu_before,"gpu_loaded":gpu_loaded,"gpu_after":gpu(),"aggregate_warm":warm,"groups_warm":groups,"provider_selected":provider,"provider_option":requested,"per_file_warm":per_file,"runs":runs,"notes":"GPU memory is device-wide snapshots, not process peak. First corpus pass excluded from warm aggregates. WAV loading excluded from tail. VAD simulation uses the real segmenter; ASR runs synchronously between feeds, so paced tail conservatively includes backlog. No clipboard or microphone in this harness. LLM is opt-in. Zero resample cost: corpus is already 16 kHz."});
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
