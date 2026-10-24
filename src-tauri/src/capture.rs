//! Explicitly opt-in, local-only development captures. No network code lives here.
use crate::performance::{Breakdown, Trace};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime},
};

// Only background I/O takes this lock, never dictation.
static IO_LOCK: Mutex<()> = Mutex::new(());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct Session(Arc<Mutex<Draft>>);
struct Draft {
    folder: String,
    meta: Value,
    audio: Vec<f32>,
    inputs: Vec<Vec<f32>>,
    retention: Retention,
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
            retention: Retention::default(),
        })))
    }
    pub fn retention(&self, settings: &crate::state::Settings) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).retention =
            Retention::from_settings(settings);
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
            let _io = IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let root = crate::settings::config_dir().join("captures");
            if let Err(e) = self.write_to(&root, timings) {
                log::error!("Could not save local dictation capture: {e}");
            }
            let retention = self.0.lock().unwrap_or_else(|e| e.into_inner()).retention;
            maintain_logged(&root, retention, false);
        });
    }
    fn write_to(&self, root: &Path, timings: Breakdown) -> Result<(), String> {
        let mut d = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let dir = root.join(format!(".{}.tmp", d.folder));
        let completed = root.join(&d.folder);
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
        std::fs::rename(&dir, &completed).map_err(|e| e.to_string())?;
        log::info!("Saved local dictation capture: {}", completed.display());
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

#[derive(Clone, Copy)]
struct Retention {
    days: u64,
    max_bytes: u64,
}
impl Default for Retention {
    fn default() -> Self {
        Self {
            days: crate::state::default_capture_retention_days(),
            max_bytes: crate::state::default_capture_max_bytes(),
        }
    }
}
impl Retention {
    fn from_settings(s: &crate::state::Settings) -> Self {
        Self {
            days: s.capture_retention_days,
            max_bytes: s.capture_max_bytes,
        }
    }
}
struct CaptureFile {
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
}

// Expire first, then evict oldest remaining captures until within the budget.
// Zero days expires past captures; zero bytes removes nonempty captures.
fn retention_selection(
    mut captures: Vec<CaptureFile>,
    policy: Retention,
    now: SystemTime,
) -> Vec<PathBuf> {
    captures.sort_by(|a, b| a.modified.cmp(&b.modified).then(a.path.cmp(&b.path)));
    let age = Duration::from_secs(policy.days.saturating_mul(86400));
    let mut total: u128 = captures.iter().map(|c| u128::from(c.bytes)).sum();
    let mut remove = Vec::new();
    for capture in captures {
        if now
            .duration_since(capture.modified)
            .is_ok_and(|elapsed| elapsed > age)
            || total > u128::from(policy.max_bytes)
        {
            total -= u128::from(capture.bytes);
            remove.push(capture.path);
        }
    }
    remove
}
fn is_partial(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.') && n.ends_with(".tmp"))
}
fn directory_bytes(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        // Never traverse junctions/symlinks out of the capture directory.
        if kind.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        } else if kind.is_dir() {
            total = total.saturating_add(directory_bytes(&entry.path())?);
        }
    }
    Ok(total)
}
fn maintain(root: &Path, policy: Retention, startup: bool) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut captures = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if is_partial(&path) {
            if startup {
                remove_logged(&path);
            }
            continue;
        }
        // Include old, interrupted pre-atomic captures in retention too.
        match (
            directory_bytes(&path),
            entry.metadata().and_then(|m| m.modified()),
        ) {
            (Ok(bytes), Ok(modified)) => captures.push(CaptureFile {
                path,
                bytes,
                modified,
            }),
            _ => log::warn!("Could not inspect local capture {}", path.display()),
        }
    }
    for path in retention_selection(captures, policy, SystemTime::now()) {
        remove_logged(&path);
    }
    Ok(())
}
fn remove_logged(path: &Path) {
    if let Err(e) = std::fs::remove_dir_all(path) {
        log::warn!("Could not remove local capture {}: {e}", path.display());
    }
}
fn maintain_logged(root: &Path, policy: Retention, startup: bool) {
    if let Err(e) = maintain(root, policy, startup) {
        log::warn!("Could not maintain local captures: {e}");
    }
}
pub fn maintain_at_startup(settings: &crate::state::Settings) {
    let policy = Retention::from_settings(settings);
    tauri::async_runtime::spawn_blocking(move || {
        let _io = IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        maintain_logged(
            &crate::settings::config_dir().join("captures"),
            policy,
            true,
        );
    });
}

#[cfg(test)]
mod hygiene_tests {
    use super::*;
    #[test]
    fn retention_expires_then_caps_oldest_with_stable_ties() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(30 * 86400);
        let entry = |name: &str, days: u64, bytes| CaptureFile {
            path: name.into(),
            modified: now - Duration::from_secs(days * 86400),
            bytes,
        };
        let selected = retention_selection(
            vec![
                entry("new", 0, 6),
                entry("b", 1, 3),
                entry("expired", 15, 100),
                entry("a", 1, 3),
            ],
            Retention {
                days: 14,
                max_bytes: 9,
            },
            now,
        );
        assert_eq!(selected, vec![PathBuf::from("expired"), PathBuf::from("a")]);
        assert!(retention_selection(
            vec![entry("boundary", 14, 1)],
            Retention {
                days: 14,
                max_bytes: 1
            },
            now
        )
        .is_empty());
        assert_eq!(
            retention_selection(
                vec![entry("new", 0, 1)],
                Retention {
                    days: 14,
                    max_bytes: 0
                },
                now
            )
            .len(),
            1
        );
        assert!(retention_selection(vec![], Retention::default(), now).is_empty());
    }
    #[test]
    fn startup_removes_partial_but_preserves_published_capture() {
        let session = Session::new(json!({}));
        let folder = session.0.lock().unwrap().folder.clone();
        let root = std::env::temp_dir().join(format!("chirp-hygiene-{folder}"));
        session.write_to(&root, Breakdown::default()).unwrap();
        let partial = root.join(".interrupted.tmp");
        std::fs::create_dir(&partial).unwrap();
        std::fs::write(partial.join("meta.json"), b"{}").unwrap();
        maintain(&root, Retention::default(), false).unwrap();
        assert!(partial.exists()); // Routine retention never touches an active write.
        maintain(&root, Retention::default(), true).unwrap();
        assert!(!partial.exists());
        assert!(root.join(&folder).join("meta.json").exists());
        assert!(!root.join(format!(".{folder}.tmp")).exists());
        // Simulate failure before metadata/rename: final destination stays invisible.
        let failed = Session::new(json!({}));
        let name = failed.0.lock().unwrap().folder.clone();
        let tmp = root.join(format!(".{name}.tmp"));
        std::fs::create_dir(&tmp).unwrap();
        assert!(failed.write_to(&root, Breakdown::default()).is_err());
        assert!(!root.join(name).exists());
        maintain(&root, Retention::default(), true).unwrap();
        assert!(!tmp.exists());
        let resolved = root.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert!(resolved.starts_with(&temp) && resolved != temp);
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
