//! Replays saved dictation captures through the app's VAD and ASR, locally.
//!
//! Each capture's `audio.wav` is fed through the real Silero segmenter (10 ms
//! chunks, unpaced) and every segment through `transcribe::transcribe_segment`,
//! exactly as the VAD receiver thread does. Nothing is uploaded; results go to
//! `bench/results/` (gitignored) because they contain dictated text.
//!
//! ```powershell
//! C:/tmp/chirp-target/bench-cuda/replay.exe --provider cuda --label greedy
//! C:/tmp/chirp-target/bench-cuda/replay.exe --provider cuda --beam --label beam
//! ```
use app_lib::{audio, transcribe};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn read_wav(path: &std::path::Path) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut wav = hound::WavReader::open(path)?;
    let spec = wav.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 {
        return Err(format!("{} must be mono 16 kHz", path.display()).into());
    }
    Ok(match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => wav.samples::<f32>().collect::<Result<_, _>>()?,
        (hound::SampleFormat::Int, 16) => wav
            .samples::<i16>()
            .map(|s| s.map(|s| s as f32 / 32768.0))
            .collect::<Result<_, _>>()?,
        _ => return Err(format!("{} must be PCM16 or float32", path.display()).into()),
    })
}

fn stats(mut v: Vec<f64>) -> serde_json::Value {
    if v.is_empty() {
        return json!(null);
    }
    v.sort_by(f64::total_cmp);
    let p = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    json!({"n":v.len(),"mean":v.iter().sum::<f64>()/v.len() as f64,"p50":p(0.5),"p95":p(0.95),"max":v[v.len()-1]})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |key: &str| args.windows(2).find(|a| a[0] == key).map(|a| a[1].clone());
    let flag = |key: &str| args.iter().any(|a| a == key);
    let appdata = PathBuf::from(std::env::var("APPDATA")?).join("com.chirp.app");
    let captures = arg("--captures").map(PathBuf::from).unwrap_or_else(|| appdata.join("captures"));
    let vad_model = arg("--vad").map(PathBuf::from).unwrap_or_else(|| appdata.join("models/silero_vad.onnx"));
    let provider = arg("--provider").unwrap_or_else(|| "cuda".into());
    let threads: i32 = arg("--threads").unwrap_or_else(|| if provider == "cuda" { "4" } else { "6" }.into()).parse()?;
    let dir = arg("--model-dir").map(PathBuf::from).unwrap_or_else(|| {
        let int8 = transcribe::model_dir("parakeet-tdt-0.6b");
        if provider == "cuda" { int8.parent().unwrap().join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3") } else { int8 }
    });
    let beam = flag("--beam");
    let passes: usize = arg("--passes").unwrap_or_else(|| "2".into()).parse()?;
    let label = arg("--label").unwrap_or_else(|| format!("{provider}-{}", if beam { "beam" } else { "greedy" }));

    let load = Instant::now();
    let rec = transcribe::load_model_config(&dir, beam, &provider, threads, false)?;
    let load_ms = ms(load);
    let t = Instant::now();
    transcribe::transcribe(&rec, &vec![0.0; 16000])?;
    println!("{label}: load {load_ms:.0} ms, warmup {:.0} ms, {provider}, beam={beam}", ms(t));

    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&captures)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("audio.wav").is_file())
        .collect();
    dirs.sort();
    if let Some(limit) = arg("--limit") {
        dirs.truncate(limit.parse()?);
    }

    let mut items = Vec::new();
    let (mut seg_ms, mut dictation_ms, mut tail_ms) = (Vec::new(), Vec::new(), Vec::new());
    // Pass 0 warms shape-specific kernels; timings come from the last pass.
    for pass in 0..passes {
        items.clear();
        seg_ms.clear();
        dictation_ms.clear();
        tail_ms.clear();
        for path in &dirs {
            eprint!(".");
            let samples = read_wav(&path.join("audio.wav"))?;
            let (tx, rx) = crossbeam_channel::unbounded();
            let vad = Arc::new(Mutex::new(
                audio::create_vad_state(&vad_model.to_string_lossy(), tx).ok_or("VAD creation failed")?,
            ));
            for chunk in samples.chunks(160) {
                audio::feed_vad(chunk, &vad);
            }
            audio::flush_vad(&vad);
            drop(vad);
            let mut segments = Vec::new();
            for segment in rx.try_iter() {
                let t = Instant::now();
                let text = transcribe::transcribe_segment(&rec, &segment)?;
                let elapsed = ms(t);
                seg_ms.push(elapsed);
                segments.push(json!({"samples":segment.audio.len(),"lead":segment.lead,"ms":elapsed,"text":text}));
            }
            let total: f64 = segments.iter().map(|s| s["ms"].as_f64().unwrap()).sum();
            let last = segments.last().map_or(0.0, |s| s["ms"].as_f64().unwrap());
            dictation_ms.push(total);
            tail_ms.push(last);
            let texts: Vec<String> = segments
                .iter()
                .map(|s| s["text"].as_str().unwrap().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let raw = texts.join(" ");
            items.push(json!({
                "id": path.file_name().unwrap().to_string_lossy(),
                "audio_ms": samples.len() as f64 / 16.0,
                "segments": segments,
                "empty_segments": segments.iter().filter(|s| s["text"].as_str().unwrap().is_empty()).count(),
                "asr_ms": total,
                "last_segment_ms": last,
                "words": raw.split_whitespace().count(),
                "raw": raw,
                "spoken_text": app_lib::cleanup::spoken_text(&texts),
            }));
        }
        println!("pass {pass}: {} captures, {} segments", items.len(), seg_ms.len());
    }
    let words: usize = items.iter().map(|i| i["words"].as_u64().unwrap() as usize).sum();
    let empty: usize = items.iter().map(|i| i["empty_segments"].as_u64().unwrap() as usize).sum();
    let summary = json!({
        "label": label, "provider": provider, "threads": threads, "beam": beam, "model_dir": dir,
        "captures": items.len(), "segments": seg_ms.len(), "empty_segments": empty, "words": words,
        "segment_ms": stats(seg_ms.clone()), "dictation_asr_ms": stats(dictation_ms.clone()),
        "last_segment_ms": stats(tail_ms.clone()), "load_ms": load_ms,
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| "bench/results".into()));
    std::fs::create_dir_all(&out)?;
    let path = out.join(format!("replay-{label}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(&json!({"summary":summary,"items":items}))?)?;
    println!("Saved {}", path.display());
    Ok(())
}
