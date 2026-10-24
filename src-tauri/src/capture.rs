//! Explicitly opt-in, local-only development captures. No network code lives here.
use crate::performance::{Breakdown, Trace};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct Session(Arc<Mutex<Draft>>);
struct Draft {
    folder: String,
    meta: Value,
    audio: Vec<f32>,
    inputs: Vec<Vec<f32>>,
}

impl Session {
    pub fn new(configuration: Value) -> Self {
        let now = chrono::Utc::now();
        let id = format!(
            "{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        );
        Self(Arc::new(Mutex::new(Draft {
            folder: format!("{}-{id}", now.format("%Y%m%dT%H%M%S%.3fZ")),
            meta: json!({"schema_version":1,"id":id,"created_at":now.to_rfc3339(),"status":"recording",
                "sample_rate":16000,"channels":1,"sample_format":"float32",
                "audio_description":"audio.wav is the complete resampled capture, preserving pauses. asr-input-NNN.wav files contain the exact float samples passed to each recognizer call, including chunk overlaps.",
                "configuration":configuration,"raw_asr_text":null,"deterministic_text":null,
                "router_decision":null,"final_text":null,"pasted_text":null,"llm":null,
                "context":null,"context_sent":false,"asr_inputs":[],"speculative_result_used":false}),
            audio: Vec::new(),
            inputs: Vec::new(),
        })))
    }
    pub fn set(&self, key: &str, value: impl serde::Serialize) {
        if let Ok(value) = serde_json::to_value(value) {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).meta[key] = value;
        }
    }
    pub fn audio(&self, samples: &[f32]) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).audio = samples.to_vec();
    }
    pub fn asr_input(&self, samples: &[f32], text: Option<&str>, error: Option<&str>) {
        let mut d = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let index = d.inputs.len();
        d.inputs.push(samples.to_vec());
        d.meta["asr_inputs"].as_array_mut().unwrap().push(json!({
            "wav":format!("asr-input-{index:03}.wav"),"samples":samples.len(),"text":text,"error":error
        }));
    }
    pub fn save(self, timings: Breakdown) {
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = self.write_to(&crate::settings::config_dir().join("captures"), timings)
            {
                log::error!("Could not save local dictation capture: {e}");
            }
        });
    }
    fn write_to(&self, root: &Path, timings: Breakdown) -> Result<(), String> {
        let mut d = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let dir = root.join(&d.folder);
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        write_wav(&dir.join("audio.wav"), &d.audio)?;
        for (i, samples) in d.inputs.iter().enumerate() {
            write_wav(&dir.join(format!("asr-input-{i:03}.wav")), samples)?;
        }
        if d.meta["raw_asr_text"].is_null() {
            d.meta["raw_asr_text"] = json!(d.meta["asr_inputs"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v["text"].as_str())
                .collect::<Vec<_>>()
                .join(" "));
        }
        if d.meta["status"] == "recording" {
            d.meta["status"] = json!("failed");
            if d.meta.get("error").is_none() {
                d.meta["error"] = json!("Pipeline ended before paste dispatch");
            }
        }
        d.meta["speculative_result_used"] = json!(
            timings.speculation.as_ref().is_some_and(|s| s.hit)
                && d.meta["llm"].get("error").is_none()
                && !d.meta["llm"].is_null()
        );
        if timings.speculation.as_ref().is_some_and(|s| s.calls > 0) {
            d.meta["context_sent"] = json!(true);
        }
        d.meta["timings"] = serde_json::to_value(timings).map_err(|e| e.to_string())?;
        // Meta is the completion marker; importers ignore incomplete directories.
        let tmp = dir.join("meta.json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&d.meta).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(tmp, dir.join("meta.json")).map_err(|e| e.to_string())?;
        log::info!("Saved local dictation capture: {}", dir.display());
        Ok(())
    }
}
fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &sample in samples {
        wav.write_sample(sample).map_err(|e| e.to_string())?;
    }
    wav.finalize().map_err(|e| e.to_string())
}

/// Saves failures on early return. Success hands the session to the existing
/// clipboard-restoration task so the final timing breakdown is included.
pub struct Guard {
    session: Option<Session>,
    trace: Trace,
}
impl Guard {
    pub fn new(trace: &Trace) -> Self {
        Self {
            session: trace.capture(),
            trace: trace.clone(),
        }
    }
    pub fn session(&self) -> Option<Session> {
        self.session.clone()
    }
    pub fn handoff(&mut self) -> Option<Session> {
        self.session.take()
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            session.save(self.trace.snapshot());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_preserves_float_samples_and_marks_unpasted_failures() {
        let session = Session::new(json!({"test":true}));
        let samples = [0.12345679_f32, -0.9876543, 0.0];
        session.audio(&samples);
        session.asr_input(&samples[1..], Some("raw words"), None);
        let root = std::env::temp_dir().join(format!(
            "chirp-capture-test-{}",
            session.0.lock().unwrap().folder
        ));
        session.write_to(&root, Breakdown::default()).unwrap();
        let dir = root.join(&session.0.lock().unwrap().folder);
        let got: Vec<f32> = hound::WavReader::open(dir.join("audio.wav"))
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(got, samples);
        let exact: Vec<f32> = hound::WavReader::open(dir.join("asr-input-000.wav"))
            .unwrap()
            .samples()
            .map(Result::unwrap)
            .collect();
        assert_eq!(exact, samples[1..]);
        let meta: Value =
            serde_json::from_slice(&std::fs::read(dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["raw_asr_text"], "raw words");
        assert_eq!(meta["status"], "failed");
        assert!(meta["pasted_text"].is_null());
        // Only this test's fresh directory under the OS temp directory.
        let resolved = root.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert!(resolved.starts_with(&temp) && resolved != temp);
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
