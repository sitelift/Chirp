//! Lightweight monotonic timing shared by capture, ASR and injection.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Breakdown {
    pub llm: Option<serde_json::Value>,
    pub stages_ms: BTreeMap<String, f64>,
    pub asr_chunks_ms: Vec<f64>,
    pub release_to_paste_ms: Option<f64>,
    pub clipboard_restored: Option<bool>,
    pub release_to_restore_ms: Option<f64>,
}
struct Inner {
    data: Breakdown,
    release: Option<Instant>,
    hold: bool,
}
#[derive(Clone)]
pub struct Trace(Arc<Mutex<Inner>>, Arc<tokio::sync::Notify>);
static ACTIVE: Mutex<Option<Trace>> = Mutex::new(None);
impl Trace {
    pub fn start() -> Self {
        let trace = Self(
            Arc::new(Mutex::new(Inner {
                data: Breakdown::default(),
                release: None,
                hold: true,
            })),
            Arc::new(tokio::sync::Notify::new()),
        );
        *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(trace.clone());
        trace
    }
    pub fn current() -> Option<Self> {
        ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn set_hold(&self, hold: bool) { self.0.lock().unwrap_or_else(|e|e.into_inner()).hold = hold; }
    pub fn input(&self, key_up: bool) {
        let mut inner = self.0.lock().unwrap_or_else(|e|e.into_inner());
        if inner.hold == key_up { inner.release.get_or_insert_with(Instant::now); }
    }
    pub fn release(&self) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .release
            .get_or_insert_with(Instant::now);
    }
    pub fn stage(&self, name: &str, start: Instant) {
        *self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .data
            .stages_ms
            .entry(name.into())
            .or_default() += start.elapsed().as_secs_f64() * 1000.0;
    }
    pub fn asr(&self, start: Instant) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .data
            .asr_chunks_ms
            .push(start.elapsed().as_secs_f64() * 1000.0);
    }
    pub fn pasted(&self) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.data.release_to_paste_ms = inner.release.map(|t| t.elapsed().as_secs_f64() * 1000.0);
        let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|t| Arc::ptr_eq(&t.0, &self.0)) { *active = None; }
    }
    pub fn restored(&self, restored: bool) {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.data.clipboard_restored = Some(restored);
        inner.data.release_to_restore_ms =
            inner.release.map(|t| t.elapsed().as_secs_f64() * 1000.0);
        self.1.notify_one();
        log::info!(
            "pipeline_timing {}",
            serde_json::to_string(&inner.data).unwrap_or_default()
        );
    }
    pub fn llm(&self, value: serde_json::Value) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).data.llm = Some(value);
    }
    pub async fn wait_restored(&self) {
        self.1.notified().await;
    }
    pub fn snapshot(&self) -> Breakdown {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .data
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_and_toggle_use_the_correct_input_boundary() {
        let hold = Trace::start();
        hold.input(false);
        assert!(hold.0.lock().unwrap().release.is_none());
        hold.input(true);
        let release = hold.0.lock().unwrap().release.unwrap();
        hold.release();
        assert_eq!(hold.0.lock().unwrap().release, Some(release));
        hold.pasted();
        assert!(Trace::current().is_none());

        let toggle = Trace::start();
        toggle.set_hold(false);
        toggle.input(true);
        assert!(toggle.0.lock().unwrap().release.is_none());
        toggle.input(false);
        assert!(toggle.0.lock().unwrap().release.is_some());
        toggle.pasted();
        assert!(Trace::current().is_none());
    }
}
