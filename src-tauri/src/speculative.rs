//! One speculative cleanup request per dictation at a time. Only exact text
//! matches may cross the release boundary; stale results never reach the clipboard.
use crate::llm::{AppContext, LlmClient, LlmOutcome};
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

type Outcome = Result<LlmOutcome, String>;
type Cleanup = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Outcome> + Send>> + Send + Sync>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub hit: bool,
    pub calls: usize,
    pub stale_completions: usize,
    /// Observed request work overlapped with recording, not a prediction of
    /// how fast a separate network request would have been.
    pub time_saved_ms: f64,
    pub release_wait_ms: f64,
    pub reason: String,
}
struct Completed {
    text: String,
    outcome: Outcome,
    start: Instant,
    end: Instant,
}
struct Inner {
    accepting: bool,
    released_at: Option<Instant>,
    desired: Option<String>,
    inflight: Option<String>,
    completed: Option<Completed>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
    calls: usize,
    stale: usize,
}
#[derive(Clone)]
pub struct Speculation {
    inner: Arc<Mutex<Inner>>,
    changed: Arc<tokio::sync::Notify>,
    cleanup: Cleanup,
    app: String,
    enabled: bool,
}
#[derive(Default)]
pub struct RecordingSpeculation(pub Mutex<Option<Speculation>>);

impl Speculation {
    pub fn new(client: LlmClient, context: AppContext, timeout: Duration) -> Self {
        let enabled = client.has_key();
        let app = context.app.clone();
        let cleanup: Cleanup = Arc::new(move |text| {
            let client = client.clone();
            let context = context.clone();
            Box::pin(async move {
                client
                    .cleanup(&text, &context, timeout)
                    .await
                    .map_err(|e| e.to_string())
            })
        });
        Self::with_cleanup(app, enabled, cleanup)
    }
    fn with_cleanup(app: String, enabled: bool, cleanup: Cleanup) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                accepting: true,
                released_at: None,
                desired: None,
                inflight: None,
                completed: None,
                task: None,
                calls: 0,
                stale: 0,
            })),
            changed: Arc::new(tokio::sync::Notify::new()),
            cleanup,
            app,
            enabled,
        }
    }
    pub fn offer(&self, text: String) {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !s.accepting {
            return;
        }
        s.desired = if self.enabled && crate::router::needs_llm(&text, &self.app) {
            Some(text)
        } else {
            None
        };
        if s.inflight.is_some() || s.desired.is_none() {
            return;
        }
        if s.completed
            .as_ref()
            .is_some_and(|c| Some(&c.text) == s.desired.as_ref())
        {
            return;
        }
        // Reserve the slot before scheduling, so consecutive offers cannot
        // launch concurrent workers even when the runtime has not polled yet.
        s.inflight = s.desired.clone();
        let this = self.clone();
        s.task = Some(tauri::async_runtime::spawn(async move {
            this.run().await;
        }));
    }
    async fn run(&self) {
        loop {
            let text = {
                let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(text) = s.inflight.clone() else {
                    return;
                };
                s.calls += 1;
                text
            };
            let start = Instant::now();
            let outcome = (self.cleanup)(text.clone()).await;
            let end = Instant::now();
            let again = {
                let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                if s.desired.as_ref() != Some(&text) {
                    s.stale += 1;
                }
                s.completed = Some(Completed {
                    text: text.clone(),
                    outcome,
                    start,
                    end,
                });
                s.inflight = None;
                if s.accepting && s.desired.as_ref().is_some_and(|next| next != &text) {
                    s.inflight = s.desired.clone();
                    true
                } else {
                    false
                }
            };
            self.changed.notify_one();
            if !again {
                return;
            }
        }
    }
    /// Called at key release, before draining the last audio/VAD segment.
    pub fn seal(&self) {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        s.accepting = false;
        s.released_at.get_or_insert_with(Instant::now);
    }
    pub fn cancel(&self) {
        let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        s.accepting = false;
        if let Some(task) = s.task.take() {
            task.abort();
        }
        s.inflight = None;
        s.completed = None;
        s.desired = None;
        self.changed.notify_one();
    }
    /// None means the caller must issue the ordinary release-time request.
    /// Some(Err) means an exact-match speculation failed: use deterministic text.
    pub async fn finish(&self, final_text: &str) -> (Option<Outcome>, Metrics) {
        self.seal();
        let release = Instant::now();
        loop {
            let notified = self.changed.notified();
            {
                let mut s = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let mut metrics = Metrics {
                    calls: s.calls,
                    stale_completions: s.stale,
                    ..Default::default()
                };
                if s.inflight.as_deref().is_some_and(|text| text != final_text) {
                    metrics.reason = "stale_request".into();
                    if let Some(task) = s.task.take() {
                        task.abort();
                    }
                    s.inflight = None;
                    return (None, metrics);
                }
                if s.inflight.is_none() {
                    if let Some(done) = s.completed.take().filter(|c| c.text == final_text) {
                        metrics.hit = true;
                        metrics.reason = if done.outcome.is_ok() {
                            "exact_match"
                        } else {
                            "matched_error"
                        }
                        .into();
                        metrics.release_wait_ms = release.elapsed().as_secs_f64() * 1000.0;
                        metrics.time_saved_ms = done
                            .end
                            .min(s.released_at.unwrap_or(release))
                            .saturating_duration_since(done.start)
                            .as_secs_f64()
                            * 1000.0;
                        return (Some(done.outcome), metrics);
                    }
                    metrics.reason = "no_matching_request".into();
                    return (None, metrics);
                }
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn outcome(text: String) -> LlmOutcome {
        LlmOutcome {
            text,
            ttft_ms: 0,
            total_ms: 0,
            input_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            output_tokens: 0,
        }
    }
    #[test]
    fn newer_segments_coalesce_and_only_exact_final_text_is_reused() {
        tauri::async_runtime::block_on(async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let cleanup: Cleanup = Arc::new(move |text| {
                let tx = tx.clone();
                Box::pin(async move {
                    let (reply, wait) = tokio::sync::oneshot::channel();
                    tx.send((text.clone(), reply)).unwrap();
                    wait.await.unwrap();
                    Ok(outcome(text))
                })
            });
            let spec = Speculation::with_cleanup("Outlook".into(), true, cleanup);
            spec.offer("first segment".into());
            let (first, done) = rx.recv().await.unwrap();
            assert_eq!(first, "first segment");
            spec.offer("second version".into());
            spec.offer("latest version".into());
            assert!(rx.try_recv().is_err());
            done.send(()).unwrap();
            let (latest, done) = rx.recv().await.unwrap();
            assert_eq!(latest, "latest version");
            spec.seal();
            spec.offer("must not start after release".into());
            done.send(()).unwrap();
            let (out, stats) = spec.finish("latest version").await;
            assert_eq!(out.unwrap().unwrap().text, "latest version");
            assert!(stats.hit);
            assert_eq!(stats.calls, 2);
            assert_eq!(stats.stale_completions, 1);
            assert!(rx.try_recv().is_err());
        });
    }
    #[test]
    fn mismatched_text_never_reuses_a_speculative_result() {
        tauri::async_runtime::block_on(async {
            let cleanup: Cleanup = Arc::new(|text| Box::pin(async move { Ok(outcome(text)) }));
            let spec = Speculation::with_cleanup("Outlook".into(), true, cleanup);
            spec.offer("old text".into());
            let (out, stats) = spec.finish("new text").await;
            assert!(out.is_none());
            assert!(!stats.hit);
        });
    }
}
