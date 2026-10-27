//! Local recognition diagnostics without invented reference transcripts or WER.
//! A JSON manifest supplies {id, wav}; files must be mono 16 kHz PCM16/float32.
//! --mode file decodes each file whole; recording uses the app's bounded
//! overlapping chunks; vad replays speculative segmentation for comparison.
use app_lib::{audio, transcribe};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Deserialize)]
struct Item {
    id: String,
    wav: PathBuf,
    #[serde(default)]
    snippets: Vec<app_lib::SnippetEntry>,
}

fn read_wav(path: &std::path::Path) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut wav = hound::WavReader::open(path)?;
    let s = wav.spec();
    if s.channels != 1 || s.sample_rate != 16000 {
        return Err("expected mono 16 kHz".into());
    }
    let samples: Vec<f32> = match (s.sample_format, s.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => wav.samples::<f32>().collect::<Result<_, _>>()?,
        (hound::SampleFormat::Int, 16) => wav
            .samples::<i16>()
            .map(|v| v.map(|v| v as f32 / 32768.0))
            .collect::<Result<_, _>>()?,
        _ => return Err("expected float32 or PCM16".into()),
    };
    if samples.iter().any(|s| !s.is_finite()) {
        return Err("non-finite audio".into());
    }
    Ok(samples)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.windows(2).find(|p| p[0] == k).map(|p| p[1].clone());
    let manifest = PathBuf::from(arg("--manifest").ok_or("--manifest required")?);
    let output = PathBuf::from(arg("--out").ok_or("--out required")?);
    if output.exists() {
        return Err("output already exists".into());
    }
    let mode = arg("--mode").unwrap_or_else(|| "file".into());
    if !["file", "recording", "vad"].contains(&mode.as_str()) {
        return Err("--mode must be file, recording or vad".into());
    }
    let provider = arg("--provider").unwrap_or_else(|| "cuda".into());
    let threads = arg("--threads").unwrap_or_else(|| "4".into()).parse()?;
    let passes: usize = arg("--passes").unwrap_or_else(|| "2".into()).parse()?;
    let appdata = PathBuf::from(std::env::var("APPDATA")?).join("com.chirp.app");
    let model = arg("--model-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| appdata.join("models/sherpa/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3"));
    let vad_model = appdata.join("models/silero_vad.onnx");
    let manifest_bytes = std::fs::read(&manifest)?;
    let items: Vec<Item> = serde_json::from_slice(&manifest_bytes)?;
    let beam = args.iter().any(|a| a == "--beam");
    let t = Instant::now();
    let recognizer = transcribe::load_model_config(&model, beam, &provider, threads, false)?;
    let load_ms = t.elapsed().as_secs_f64() * 1000.0;
    transcribe::transcribe(&recognizer, &vec![0.0; 16000])?;
    let mut results = Vec::new();
    for pass in 0..passes {
        for item in &items {
            let samples = read_wav(&manifest.parent().unwrap().join(&item.wav))?;
            let t = Instant::now();
            let mut segments = Vec::new();
            if mode != "vad" {
                let text = if mode == "recording" {
                    transcribe::transcribe_recording(&recognizer, &samples)?
                } else {
                    transcribe::transcribe(&recognizer, &samples)?
                };
                segments.push(json!({"samples":samples.len(),"lead":0,"text":text,"ms":t.elapsed().as_secs_f64()*1000.0}));
            } else {
                let (tx, rx) = crossbeam_channel::unbounded();
                let vad = Arc::new(Mutex::new(
                    audio::create_vad_state(&vad_model.to_string_lossy(), tx)
                        .ok_or("VAD creation failed")?,
                ));
                for chunk in samples.chunks(160) {
                    audio::feed_vad(chunk, &vad);
                }
                audio::flush_vad(&vad);
                for segment in rx.try_iter() {
                    let t = Instant::now();
                    let text = transcribe::transcribe_segment(&recognizer, &segment)?;
                    segments.push(json!({"samples":segment.audio.len(),"lead":segment.lead,"text":text,"ms":t.elapsed().as_secs_f64()*1000.0}));
                }
            }
            let wall_ms = t.elapsed().as_secs_f64() * 1000.0;
            let texts: Vec<String> = segments
                .iter()
                .map(|s| s["text"].as_str().unwrap().to_string())
                .collect();
            let spoken = app_lib::cleanup::spoken_text(&texts);
            let deterministic = app_lib::apply_snippets(&spoken, &item.snippets);
            results.push(json!({"id":item.id,"pass":pass,"samples":samples.len(),"wall_ms":wall_ms,"segments":segments,"raw":texts.join(" ").trim(),"spoken":spoken,"deterministic":deterministic}));
            println!("{} pass {pass}: {:.0} ms", item.id, wall_ms);
        }
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(
            &json!({"mode":mode,"model":model,"provider":provider,"threads":threads,"beam":beam,"passes":passes,"load_ms":load_ms,"manifest_sha256":format!("{:x}",Sha256::digest(&manifest_bytes)),"transcribe_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src-tauri/src/transcribe.rs"))),"audio_sha256":format!("{:x}",Sha256::digest(include_bytes!("../src-tauri/src/audio.rs"))),"notes":"Local audio diagnostic; no reference or WER, no LLM or paste. Pass 0 warms shapes. VAD replay is unpaced, not live tail latency.","results":results}),
        )?,
    )?;
    println!("Saved {}", output.display());
    Ok(())
}
