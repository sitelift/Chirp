use crate::audio;
use crate::cleanup;
use crate::history;
use crate::inject;
use crate::settings;
use crate::snippets;
use crate::state::*;
use crate::transcribe;

use std::io::Cursor;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

const CHIRP_SOUND: &[u8] = include_bytes!("../sounds/chirp.wav");

/// Active audio stream handle — wrapped in an unsafe Send wrapper because
/// cpal::Stream is !Send but we only access it from the main thread.
pub struct StreamHandle(pub std::sync::Mutex<Option<StreamWrapper>>);

#[allow(dead_code)]
pub struct StreamWrapper(cpal::Stream);
// SAFETY: StreamWrapper is only accessed via Mutex in command handlers
// dispatched on Tauri's async runtime. The stream itself is never sent
// across threads — only the Mutex guard crosses await points.
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

/// Holds the stream error flag set by cpal when the audio device reports an error.
pub struct StreamErrorState(pub std::sync::Mutex<Option<audio::StreamErrorFlag>>);

/// Holds resampler state so we can flush remaining samples when recording stops.
pub struct ResamplerFlushState(pub std::sync::Mutex<Option<audio::ResamplerState>>);

/// Tracks when the current recording started (wall-clock) for sample rate sanity checks.
pub struct RecordingStartTime(pub std::sync::Mutex<Option<Instant>>);

/// Holds the active flag for the current audio stream so we can deactivate zombie callbacks.
#[derive(Default)]
pub struct RecordingContext(pub std::sync::Mutex<Option<Arc<DictationContext>>>, pub std::sync::Mutex<Option<(Instant, crate::context::PendingContext)>>);

/// A dictation's prompt prefix is frozen at its first cleanup request. Resolving
/// at recording start would put the browser lookup on the microphone hot path.
pub struct DictationContext {
    terms: Vec<String>,
    pending: std::sync::Mutex<Option<crate::context::PendingContext>>,
    resolved: std::sync::OnceLock<crate::llm::AppContext>,
}
impl DictationContext {
    fn resolve(&self) -> crate::llm::AppContext {
        self.resolved.get_or_init(|| {
            self.pending.lock().unwrap_or_else(|e|e.into_inner()).take()
                .map(crate::context::PendingContext::resolve)
                .unwrap_or_else(crate::context::foreground)
        }).clone()
    }
}

pub fn note_hotkey_press(app: &AppHandle, at: Instant) {
    *app.state::<RecordingContext>().1.lock().unwrap_or_else(|e|e.into_inner()) = Some((at, crate::context::capture()));
    if let Some(trace) = crate::performance::Trace::current() { trace.input_at(false, at); }
}

pub struct StreamActiveState(pub std::sync::Mutex<Option<audio::StreamActiveFlag>>);

/// Open the selected device once at startup. Idle callbacks discard audio.
pub fn warm_microphone(app: &AppHandle) {
    let device = app.state::<SharedState>().blocking_lock().settings.input_device.clone();
    let buffer = app.state::<AudioBuffer>().inner().clone();
    let vad_path = settings::vad_model_path();
    let (tx, _rx) = crossbeam_channel::unbounded();
    let vad = if vad_path.exists() { audio::create_vad_state(&vad_path.to_string_lossy(), tx).map(|v| Arc::new(std::sync::Mutex::new(v))) } else { None };
    match audio::start_capture(&device, buffer.clone(), app.clone(), vad) {
        Ok((stream, error, active, resampler)) => {
            active.store(false, Ordering::SeqCst);
            resampler.quiesce();
            buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();
            *app.state::<StreamHandle>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));
            *app.state::<StreamErrorState>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
            *app.state::<StreamActiveState>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(active);
            *app.state::<ResamplerFlushState>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(resampler);
        }
        Err(e) => log::warn!("Mic prewarm unavailable; will retry on press: {e}"),
    }
}

fn arm_escape(armed: bool) {
    crate::hotkey_input::ESCAPE_ARMED.store(armed, Ordering::SeqCst);
}

/// Copy text from Recent / the last dictation. Excluded from clipboard history
/// like pasted dictations.
#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    crate::clipboard_win::set_clipboard_with_exclusion(&text).map(|_| ())
}

/// First-run microphone check: "ok", "none" (no input device) or "blocked"
/// (Windows privacy settings deny access).
#[tauri::command]
pub async fn probe_microphone(state: State<'_, SharedState>) -> Result<String, String> {
    let device = state.lock().await.settings.input_device.clone();
    tokio::task::spawn_blocking(move || {
        let buffer: AudioBuffer = Arc::new(std::sync::Mutex::new(Vec::new()));
        match audio::start_capture_headless(&device, buffer) {
            Ok((stream, _, _, _)) => {
                drop(stream);
                "ok".to_string()
            }
            Err(e) if e.contains("No default input") || e.contains("Device not found") => "none".into(),
            Err(e) => {
                log::warn!("Microphone probe failed: {e}");
                "blocked".into()
            }
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_mic_privacy_settings() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:privacy-microphone")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, SharedState>) -> Result<Settings, String> {
    let s = state.lock().await;
    Ok(s.settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    app_handle: AppHandle,
    partial: serde_json::Value,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut s = state.lock().await;
    let old_hotkey = (s.settings.hold_binding.clone(), s.settings.toggle_binding.clone());

    // Merge partial into current settings
    let mut settings_val = serde_json::to_value(&s.settings).unwrap();
    if let (Some(base), Some(patch)) = (settings_val.as_object_mut(), partial.as_object()) {
        for (k, v) in patch {
            base.insert(k.clone(), v.clone());
        }
    }
    let mut updated: Settings = serde_json::from_value(settings_val).map_err(|e| format!("Invalid settings: {e}"))?;
    for binding in [&mut updated.hold_binding, &mut updated.toggle_binding].into_iter().flatten() {
        *binding = crate::hotkey::Binding::new(binding.keys.clone(), binding.suppress);
    }
    if let Some(b) = &updated.hold_binding { crate::hotkey::conflict(b, updated.toggle_binding.as_ref())?; }
    if let Some(b) = &updated.toggle_binding { crate::hotkey::conflict(b, updated.hold_binding.as_ref())?; }
    s.settings = updated;
    settings::save_settings(&s.settings)?;

    // If history retention changed, prune immediately
    let new_retention = s.settings.history_retention_days;
    if partial.get("historyRetentionDays").is_some() && new_retention > 0 {
        history::prune_history(&mut s.history, new_retention);
    }

    // Broadcast settings change to all windows (cross-window sync)
    let _ = app_handle.emit("settings-changed", &s.settings);

    // Notify frontend to refresh history if retention changed
    if partial.get("historyRetentionDays").is_some() {
        let _ = app_handle.emit("history-changed", ());
    }


    // Sync autostart
    let autostart = app_handle.autolaunch();
    if s.settings.launch_at_login {
        let _ = autostart.enable();
    } else {
        let _ = autostart.disable();
    }

    // Rebuild recognizer if beam_search setting changed
    if partial.get("beamSearch").is_some() || partial.get("beam_search").is_some() {
        let model = s.settings.model.clone();
        let beam_search = s.settings.beam_search;
        let vocab = s.vocabulary.clone();
        if transcribe::model_exists(&model) {
            match transcribe::load_model(&model, beam_search, &vocab) {
                Ok(recognizer) => {
                    s.recognizer = Some(Arc::new(recognizer));
                    log::info!("Recognizer rebuilt (beam_search={beam_search})");
                }
                Err(e) => {
                    log::error!("Failed to rebuild recognizer: {e}");
                    // Fallback to greedy
                    if beam_search {
                        if let Ok(recognizer) = transcribe::load_model(&model, false, &vocab) {
                            s.recognizer = Some(Arc::new(recognizer));
                            log::info!("Recognizer rebuilt with greedy search (fallback)");
                        }
                    }
                }
            }
        }
    }

    // Re-register hotkey if changed
    if (s.settings.hold_binding.clone(), s.settings.toggle_binding.clone()) != old_hotkey {
        let new_hotkey = s.settings.clone();
        drop(s);
        match crate::hotkey::update(&new_hotkey, app_handle.clone()) {
            Ok(()) => {
                log::info!("Updated hotkey bindings");
                let _ = app_handle.emit("hotkey-status", "active");
            }
            Err(e) => {
                log::error!("Failed to update hotkey: {e}");
                let _ = app_handle.emit("hotkey-status", "failed");
            }
        }
    } else {
        drop(s);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_vocabulary(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::state::VocabEntry>, String> {
    let s = state.lock().await;
    Ok(s.vocabulary.clone())
}

#[tauri::command]
pub async fn update_vocabulary(
    entries: Vec<crate::state::VocabEntry>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    if entries.len() > 500 {
        return Err("Vocabulary cannot exceed 500 entries".to_string());
    }
    let mut s = state.lock().await;

    // Compare the OLD term list to the NEW term list. The recognizer's
    // hotwords automaton is built from `term` only — it does not depend on
    // `replaces`. So if the user edited only `replaces` lists (the common
    // case while building up the find/replace dictionary), we should NOT
    // mark the recognizer dirty, because rebuilding the recognizer
    // unnecessarily across many edits has been observed to cause sherpa-onnx
    // heap corruption (STATUS_HEAP_CORRUPTION 0xc0000374).
    //
    // The post-ASR find/replace pass (cleanup::apply_replacements) reads
    // s.vocabulary live each call, so replaces edits take effect on the very
    // next dictation regardless of dirty flag — no rebuild required.
    let old_terms: Vec<&str> = s.vocabulary.iter().map(|e| e.term.as_str()).collect();
    let new_terms: Vec<&str> = entries.iter().map(|e| e.term.as_str()).collect();
    let terms_changed = old_terms != new_terms;

    s.vocabulary = entries;
    settings::save_vocabulary(&s.vocabulary)?;

    if terms_changed {
        // Term list actually changed — recognizer needs rebuild for hotwords
        // automaton to reflect the new canonical terms. Lazy rebuild happens
        // at the end of the next stop_recording (NOT inline here, to prevent
        // rapid create/destroy churn under settings-sync echo).
        s.recognizer_dirty = true;
        log::info!(
            "Vocabulary terms changed ({} entries) — recognizer marked for rebuild",
            s.vocabulary.len()
        );
    }
    // else: only `replaces` lists changed — find/replace will pick them up
    // automatically on the next dictation, no rebuild needed.

    Ok(())
}

#[tauri::command]
pub async fn get_snippets(state: State<'_, SharedState>) -> Result<Vec<SnippetEntry>, String> {
    let s = state.lock().await;
    Ok(s.snippets.clone())
}

#[tauri::command]
pub async fn update_snippets(
    entries: Vec<SnippetEntry>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    if entries.len() > 100 {
        return Err("Snippets cannot exceed 100 entries".to_string());
    }
    let mut s = state.lock().await;
    s.snippets = entries.clone();
    settings::save_snippets(&s.snippets)?;
    Ok(())
}

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    Ok(audio::list_devices())
}

#[tauri::command]
pub async fn get_input_level(buffer: State<'_, AudioBuffer>) -> Result<f32, String> {
    let buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
    if buf.is_empty() {
        return Ok(0.0);
    }
    // RMS of last 1600 samples (~100ms at 16kHz)
    let window = 1600.min(buf.len());
    let tail = &buf[buf.len() - window..];
    let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
    Ok((rms * 5.0).min(1.0))
}

#[tauri::command]
pub async fn start_recording(
    hold: Option<bool>,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    stream_error_state: State<'_, StreamErrorState>,
    resampler_flush: State<'_, ResamplerFlushState>,
    recording_start: State<'_, RecordingStartTime>,
    stream_active_state: State<'_, StreamActiveState>,
    vad_transcripts: State<'_, VadTranscripts>,
    vad_cleaned: State<'_, VadCleanedTranscripts>,
    vad_receiver_handle: State<'_, VadReceiverHandle>,
    vad_sender: State<'_, VadSender>,
    vad_flush_handle: State<'_, VadFlushHandle>,
) -> Result<(), String> {
    let (capture_start, pending) = app_handle.state::<RecordingContext>().1.lock().unwrap_or_else(|e|e.into_inner()).take().unwrap_or_else(|| (Instant::now(), crate::context::capture()));
    let context_app = pending.context().app.clone();
    let (vocabulary_at_press, ai_on, speculation_on) = {
        let s = state.lock().await;
        (s.vocabulary.clone(), s.settings.ai_cleanup, s.settings.speculative_cleanup)
    };
    let dictionary_terms: Vec<String> = vocabulary_at_press.iter().map(|v|v.term.clone()).collect();
    let context = Arc::new(DictationContext { terms:dictionary_terms.clone(), pending: std::sync::Mutex::new(Some(pending)), resolved: std::sync::OnceLock::new() });
    *app_handle.state::<RecordingContext>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(context.clone());
    // AI cleanup switched off: same pool, no key, so prewarm and speculation stay silent.
    let llm = {
        let client = app_handle.state::<crate::llm::LlmClient>().inner().clone();
        if ai_on { client } else { client.without_key() }
    };
    llm.prewarm_with(&dictionary_terms);
    let trace = crate::performance::Trace::start();
    let context_trace = trace.clone();
    let speculation = speculation_on.then(|| crate::speculative::Speculation::new_deferred(
        llm, context_app, move || {
            let resolved = context.resolve();
            if let Some(capture) = context_trace.capture() {
                capture.set("context", &resolved);
                capture.set("context_sent", true);
            }
            resolved
        },
        dictionary_terms.clone(),
        std::time::Duration::from_millis(2500),
    ));
    if let Some(old) = std::mem::replace(&mut *app_handle.state::<crate::speculative::RecordingSpeculation>().0.lock().unwrap_or_else(|e|e.into_inner()), speculation.clone()) { old.cancel(); }
    // Self-heal: if we're already in a Recording/Processing state, a prior
    // session got stuck (e.g. stop_recording never fired due to a press/release
    // race). Tear down the stale stream + buffer and proceed fresh rather than
    // returning "Already recording" and stranding the user in a permanent
    // error loop.
    {
        let mut s = state.lock().await;
        if s.recording_state != RecordingState::Idle {
            log::warn!(
                "start_recording: state was {:?}, self-healing to Idle",
                s.recording_state
            );
            s.recording_state = RecordingState::Idle;
            drop(s);

            // Deactivate any zombie callbacks from the stale stream
            {
                let active = stream_active_state
                    .0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if let Some(ref flag) = *active {
                    flag.store(false, Ordering::SeqCst);
                }
            }
            // Drop the stale stream
            {
                let mut handle = stream_handle.0.lock().unwrap_or_else(|e| e.into_inner());
                *handle = None;
            }
            // Drain any pending VAD shutdown state
            {
                let mut sender = vad_sender.0.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(tx) = sender.take() {
                    let _ = tx.send(Vec::new());
                }
            }
            {
                let mut handle = vad_receiver_handle
                    .0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if let Some(h) = handle.take() {
                    let _ = h.join();
                }
            }
            {
                let mut flush = vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner());
                *flush = None;
            }
        }
    }

    let mut s = state.lock().await;

    trace.set_hold(hold.unwrap_or(false));
    // Check model is loaded
    if s.recognizer.is_none() {
        return Err("model_not_loaded".into());
    }
    if settings::capture_dictations_enabled(&s.settings) {
        let capture = crate::capture::Session::new(serde_json::json!({
            "model":s.settings.model,"beam_search":s.settings.beam_search,
            "smart_formatting":s.settings.smart_formatting,
            "speculative_cleanup":s.settings.speculative_cleanup,
            "vocabulary":vocabulary_at_press,"snippets":s.snippets,
            "provider_requested":std::env::var("CHIRP_ASR_PROVIDER").unwrap_or_else(|_|"auto".into())
        }));
        capture.retention(&s.settings);
        capture.set("dictionary_terms", &dictionary_terms);
        trace.set_capture(capture);
    }

    // NOTE: vocab-dirty rebuild does NOT happen here. Rebuilding the
    // recognizer takes ~3 seconds (loads onnx files), and doing that inside
    // start_recording blocks audio capture until it completes — the user
    // presses the hotkey, sees nothing for 3 seconds, releases, and the
    // recording is empty. The rebuild instead happens at the END of
    // stop_recording so the first dictation after a vocab edit uses stale
    // hotwords (one cycle of staleness — acceptable) but recording-start is
    // never blocked.

    // Deactivate any zombie callbacks from a previous stream before clearing the buffer.
    // On macOS, cpal/CoreAudio callbacks can outlive the Stream drop.
    {
        let prev_active = stream_active_state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref flag) = *prev_active {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    { let rs = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner()); if let Some(rs) = &*rs { rs.quiesce(); } }
    // Clear audio buffer and VAD transcripts
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();
    vad_transcripts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    vad_cleaned
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();

    // Record wall-clock start time for sample rate sanity check
    *recording_start.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());

    let device_id = s.settings.input_device.clone();
    let recognizer_for_vad = s.recognizer.clone();
    let smart_fmt_for_vad = s.settings.smart_formatting;
    let vocab_for_vad = vocabulary_at_press;
    let snippets_for_vad = s.snippets.clone();
    s.recording_state = RecordingState::Recording;
    s.vad_was_active = false;
    drop(s);

    // Closed segments decode during recording. Matching speculative cleanup
    // can overlap the remaining recording time.
    let vad_model = settings::vad_model_path();
    let vad_arc: Option<Arc<std::sync::Mutex<crate::audio::VadState>>> = if vad_model.exists() {
        let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        *vad_sender.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx.clone());

        let cached = {
            let rs = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner());
            rs.as_ref().and_then(|rs| rs.vad_slot.lock().unwrap_or_else(|e| e.into_inner()).clone())
        };
        let vad = if let Some(vad) = cached {
            vad.lock().unwrap_or_else(|e| e.into_inner()).reset_for_recording(tx);
            Some(vad)
        } else { audio::create_vad_state(&vad_model.to_string_lossy(), tx).map(|vs| Arc::new(std::sync::Mutex::new(vs))) };
        match vad {
            Some(vad_arc) => {
                *vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(vad_arc.clone());

                let transcripts = vad_transcripts.inner().clone();
                let cleaned = vad_cleaned.0.clone();
                let state_for_thread = state.inner().clone();
                let rec = recognizer_for_vad.clone().unwrap();
                let speculation_for_vad = speculation.clone();

                let handle = std::thread::Builder::new()
                    .name("vad-receiver".into())
                    .spawn(move || {
                        let mut seg_idx = 0u32;
                        while let Ok(segment_audio) = rx.recv() {
                            if segment_audio.is_empty() {
                                break;
                            } // poison pill
                            let dur = segment_audio.len() as f32 / 16000.0;
                            log::info!(
                                "VAD segment {seg_idx}: {:.1}s ({} samples)",
                                dur,
                                segment_audio.len()
                            );

                            // Mark VAD as active so stop_recording uses streamed output.
                            let s_handle = state_for_thread.clone();
                            tauri::async_runtime::block_on(async move {
                                let mut s = s_handle.lock().await;
                                s.vad_was_active = true;
                            });

                            let raw = match transcribe::transcribe(&rec, &segment_audio) {
                                Ok(t) => t,
                                Err(e) => {
                                    log::error!("VAD segment {seg_idx} transcription failed: {e}");
                                    seg_idx += 1;
                                    continue;
                                }
                            };
                            if raw.is_empty() {
                                seg_idx += 1;
                                continue;
                            }
                            log::info!("VAD segment {seg_idx} transcript: '{raw}'");
                            transcripts
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push(raw.clone());

                            // Regex/vocab pre-pass. Snippets are expanded on
                            // the joined prefix below, in the release order.
                            let stage = Instant::now();
                            let after_regex = cleanup::cleanup_text(&raw, smart_fmt_for_vad);
                            if let Some(t) = crate::performance::Trace::current() { t.stage("cleanup", stage); }
                            let stage = Instant::now();
                            let after_vocab =
                                cleanup::apply_replacements(&after_regex, &vocab_for_vad);
                            if let Some(t) = crate::performance::Trace::current() { t.stage("vocabulary", stage); }

                            // Speculate on the entire deterministic prefix;
                            // newer prefixes supersede older requests.
                            let text_so_far = {
                                let mut segments = cleaned.lock().unwrap_or_else(|e| e.into_inner());
                                segments.push(after_vocab);
                                speculation_for_vad.as_ref().map(|_| cleanup::join_cleaned_segments_with_formatting(&segments, smart_fmt_for_vad))
                            };
                            if let (Some(speculation), Some(text_so_far)) = (&speculation_for_vad, text_so_far) {
                                let text_so_far = cleanup::repair_vad_boundary_artifacts(&text_so_far);
                                let text_so_far = snippets::apply_snippets(&text_so_far, &snippets_for_vad);
                                speculation.offer(text_so_far);
                            }
                            seg_idx += 1;
                        }
                        log::info!("VAD receiver thread exiting after {seg_idx} segments");
                    })
                    .ok();
                *vad_receiver_handle
                    .0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = handle;
                Some(vad_arc)
            }
            None => {
                log::warn!("VAD state creation failed, using non-streaming transcription");
                None
            }
        }
    } else {
        log::info!("VAD model not found, using non-streaming transcription");
        None
    };

    // Start audio capture — convert Result to Option immediately so the
    // non-Send cpal::Stream doesn't live across an await point.
    let reused = {
        let rs = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner());
        let error = stream_error_state.0.lock().unwrap_or_else(|e| e.into_inner());
        let handle = stream_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rs) = rs.as_ref().filter(|rs| rs.device_id == device_id && handle.is_some() && !error.as_ref().is_some_and(|e| e.load(Ordering::SeqCst))) {
            rs.prepare(vad_arc.clone());
            if let Some(active) = stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref() { active.store(true, Ordering::SeqCst); }
            true
        } else { false }
    };
    let capture_err = if reused { None } else { match audio::start_capture(
        &device_id,
        buffer.inner().clone(),
        app_handle.clone(),
        vad_arc,
    ) {
        Ok((stream, error_flag, active_flag, resampler_state)) => {
            *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(StreamWrapper(stream));
            *stream_error_state
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(error_flag);
            *stream_active_state
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(active_flag);
            *resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(resampler_state);
            None
        }
        Err(e) => {
            let msg = if e.contains("No default input") || e.contains("Device not found") {
                "mic_not_found".to_string()
            } else {
                "mic_permission".to_string()
            };
            Some(msg)
        }
    } };

    if let Some(err) = capture_err {
        // Reset state so future recordings aren't permanently blocked
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        return Err(err);
    }

    trace.stage("capture_start", capture_start);
    // Bump recording generation and spawn a 10-minute safety timeout.
    // If the user forgets to release the hotkey, this prevents unbounded RAM usage.
    let generation = {
        let mut s = state.lock().await;
        s.recording_generation += 1;
        s.recording_generation
    };
    let state_for_timeout = state.inner().clone();
    let app_for_timeout = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        let s = state_for_timeout.lock().await;
        if s.recording_state == RecordingState::Recording && s.recording_generation == generation {
            drop(s);
            log::warn!("Recording auto-stopped after 10 minutes");
            let _ = app_for_timeout.emit("hotkey-released", ());
        }
    });

    arm_escape(true);
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    stream_error_state: State<'_, StreamErrorState>,
    resampler_flush: State<'_, ResamplerFlushState>,
    recording_start: State<'_, RecordingStartTime>,
    stream_active_state: State<'_, StreamActiveState>,
    vad_transcripts: State<'_, VadTranscripts>,
    vad_cleaned: State<'_, VadCleanedTranscripts>,
    vad_receiver_handle: State<'_, VadReceiverHandle>,
    vad_sender: State<'_, VadSender>,
    vad_flush_handle: State<'_, VadFlushHandle>,
) -> Result<TranscriptionResult, String> {
    arm_escape(false);
    let start_time = Instant::now();
    let trace = crate::performance::Trace::current().unwrap_or_else(crate::performance::Trace::start);
    trace.release();
    let mut capture_guard = crate::capture::Guard::new(&trace);
    let speculation = app_handle.state::<crate::speculative::RecordingSpeculation>().0.lock().unwrap_or_else(|e|e.into_inner()).take();
    if let Some(speculation) = &speculation { speculation.seal(); }
    let _ = &stream_handle;

    // Grab wall-clock recording duration for sample rate sanity check
    let wall_clock_secs = recording_start
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .map(|t| t.elapsed().as_secs_f32());

    // Deactivate callbacks BEFORE dropping the stream — this ensures zombie
    // callbacks from macOS CoreAudio cannot write to the buffer anymore.
    {
        let active = stream_active_state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref flag) = *active {
            flag.store(false, Ordering::SeqCst);
        }
    }

    // Keep WASAPI open; synchronize with the callback before draining.
    {
        let rs = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(rs) = &*rs { rs.quiesce(); }
    }

    trace.stage("capture_stop", start_time);
    let resample_start = Instant::now();
    // Flush any remaining resampler samples into the audio buffer
    {
        let flush_state = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref rs) = *flush_state {
            let tail = rs.flush(buffer.inner());
            let vad = vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(vad) = &*vad { audio::feed_vad(&tail, vad); }
        }
    }

    trace.stage("resample_flush", resample_start);
    // Check if the audio stream reported an error (e.g. device disconnected)
    let had_stream_error = stream_error_state
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or(false, |flag| flag.load(Ordering::SeqCst));
    if had_stream_error {
        log::warn!(
            "Audio stream reported an error during recording — device may have disconnected"
        );
    }

    let vad_start = Instant::now();
    // Flush VAD to capture the final speech segment, then shut down receiver thread
    {
        let flush = vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref vad_arc) = *flush {
            audio::flush_vad(vad_arc);
        }
    }
    trace.stage("vad_flush", vad_start);
    let drain_start = Instant::now();
    // Send poison pill to stop receiver thread
    {
        let mut sender = vad_sender.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tx) = sender.take() {
            let _ = tx.send(Vec::new());
        }
    }
    // Join receiver thread (with timeout via try_join)
    {
        let mut handle = vad_receiver_handle
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(h) = handle.take() {
            if h.join().is_err() {
                log::warn!("VAD receiver thread panicked");
            }
        }
    }
    // Clean up VAD flush handle
    {
        let mut flush = vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        *flush = None;
    }

    trace.stage("asr_drain", drain_start);
    // The callback gate synchronizes the WASAPI worker before draining.

    {
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Processing;
    }

    let _ = app_handle.emit("recording-state", "processing");

    // Get the audio data and clear the buffer for next recording.
    let audio_data = {
        let mut buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
        let data = buf.clone();
        buf.clear();
        data
    };

    let sample_count = audio_data.len();
    if let Some(capture) = capture_guard.session() {
        let t = Instant::now();
        capture.audio(&audio_data);
        trace.stage("capture_audio_snapshot", t);
    }
    let duration_secs = sample_count as f32 / 16000.0;
    let speech_duration_ms = (duration_secs * 1000.0) as u64;

    log::info!("Audio buffer: {sample_count} samples ({duration_secs:.1}s)");

    // C3 fix: check empty BEFORE any division by len to avoid divide-by-zero
    if audio_data.is_empty() {
        log::error!("Audio buffer is empty!");
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        return Err("transcription_failed".into());
    }

    // Compare wall-clock recording time to buffer duration to detect sample rate mismatches
    if let Some(wall_secs) = wall_clock_secs {
        let ratio = duration_secs / wall_secs;
        log::info!("Wall-clock: {wall_secs:.1}s, buffer: {duration_secs:.1}s, ratio: {ratio:.2}x (should be ~1.0)");
        if ratio > 1.5 || ratio < 0.5 {
            log::error!("SAMPLE RATE MISMATCH: buffer has {ratio:.1}x more audio than recording time! Device may report wrong sample rate. Effective rate: {:.0}Hz", sample_count as f32 / wall_secs);
        }
    }

    // Only scan the waveform when diagnosing a reported device error.
    let rms = if had_stream_error {
        (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32).sqrt()
    } else { 1.0 };

    // If the buffer is near-silent and the stream had an error, the mic likely disconnected
    if rms < 0.0001 && had_stream_error {
        log::error!(
            "Silent buffer with stream error — audio device likely disconnected during recording"
        );
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        return Err("mic_disconnected".into());
    }

    if sample_count < 16000 {
        log::warn!("Audio too short (<1s), may produce poor results");
    }

    // Grab what we need from state before entering blocking thread.
    // Clone the Arc<SherpaRecognizer> so we can release the state lock
    // before the expensive transcription step.
    let (recognizer, smart_fmt, vocab, snips, vad_was_active) = {
        let mut s = state.lock().await;
        let rec = s.recognizer.clone().ok_or("model_not_loaded".to_string())?;
        let vad_was_active = s.vad_was_active;
        // Reset for the next recording session, regardless of which path we take.
        s.vad_was_active = false;
        (
            rec,
            s.settings.smart_formatting,
            s.vocabulary.clone(),
            s.snippets.clone(),
            vad_was_active,
        )
    };

    // Drain VAD transcripts accumulated by the receiver thread. Whether we
    // USE them is decided by vad_was_active, NOT by vad_texts.is_empty().
    // See stop_recording doc-comment for why.
    let vad_texts: Vec<String> = {
        let mut vt = vad_transcripts.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *vt)
    };
    // Drain the streaming-cleaned segments produced by the VAD receiver thread.
    let vad_cleaned_texts: Vec<String> = {
        let mut vc = vad_cleaned.0.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *vc)
    };
    let use_vad = vad_was_active;
    if use_vad {
        if let Some(capture) = capture_guard.session() { capture.set("raw_asr_text", vad_texts.join(" ")); }
    }
    log::info!(
        "transcription path: vad_was_active={}, vad_texts_count={}, vad_cleaned_count={}, fallback_used={}",
        vad_was_active,
        vad_texts.len(),
        vad_cleaned_texts.len(),
        !vad_was_active
    );

    // The streaming path bypasses the blocking ASR fallback.
    let post_start = Instant::now();
    let formatted_opt = if use_vad {
        if vad_cleaned_texts.is_empty() {
            log::warn!("VAD cleaned transcripts empty — returning empty result (no fallback)");
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err("transcription_failed".into());
        }
        // Smart join: strip mid-sentence orphan periods, merge stub-end
        // segments with their continuation, drop internal paragraph breaks,
        // and re-run the regex pre-pass on the joined output to catch
        // cross-boundary fillers. See cleanup::join_cleaned_segments_with_formatting.
        let joined = cleanup::join_cleaned_segments_with_formatting(&vad_cleaned_texts, smart_fmt);
        if joined.trim().is_empty() {
            log::warn!(
                "VAD cleaned transcripts all whitespace after join — returning empty result"
            );
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err("transcription_failed".into());
        }
        log::info!(
            "Streaming cleanup: {} segments smart-joined, total {} chars",
            vad_cleaned_texts.len(),
            joined.len()
        );
        Some(joined)
    } else {
        None
    };

    let fallback_capture = capture_guard.session();
    let result = if let Some(text) = formatted_opt {
        Ok(text)
    } else {
        tokio::task::spawn_blocking(move || {
            // Fallback: chunk the full audio buffer and transcribe (original behavior)
            let chunks = crate::transcribe::chunk_audio(&audio_data, 16000, 30.0, 1.0);
            log::info!("Starting Parakeet TDT transcription ({} chunk(s))...", chunks.len());

            let mut transcriptions = Vec::new();
            for (i, chunk) in chunks.iter().enumerate() {
                log::debug!("ASR chunk {i}: {} samples", chunk.len());

                let chunk_raw = transcribe::transcribe(&recognizer, chunk)
                    .map_err(|e| {
                        log::error!("Transcription error on chunk {i}: {e}");
                        "transcription_failed".to_string()
                    })?;
                log::info!("Parakeet chunk {i}: '{chunk_raw}'");
                transcriptions.push(chunk_raw);
            }

            let merged = crate::transcribe::merge_transcriptions(transcriptions);
            if let Some(capture) = &fallback_capture { capture.set("raw_asr_text", &merged); }
            if merged.is_empty() {
                log::warn!("Transcription returned empty text");
                return Err("transcription_failed".to_string());
            }

            // Regex pre-pass: remove fillers, spoken punctuation, format numbers
            let stage = Instant::now();
            let formatted = cleanup::cleanup_text(&merged, smart_fmt);
            if let Some(t) = crate::performance::Trace::current() { t.stage("cleanup", stage); }

            // Vocabulary find/replace: deterministic post-ASR fixup for things
            // hotword biasing can't fix (homophones, brand spellings, stable
            // mishearings). Each VocabEntry's `replaces` list maps mishearings
            // to its canonical `term`, case-insensitive at word boundaries.
            let stage = Instant::now();
            let after_replace = cleanup::apply_replacements(&formatted, &vocab);
            if let Some(t) = crate::performance::Trace::current() { t.stage("vocabulary", stage); }

            log::info!("After regex+replace: '{after_replace}'");
            Ok(after_replace)
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    };

    // If transcription failed, reset state before returning error
    let formatted = match result {
        Ok(text) => text,
        Err(e) => {
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err(e);
        }
    };

    // Cloud LLM cleanup slots in here (see docs/v2/DIRECTION.md). Until then
    // the deterministic regex + vocabulary result is final.
    trace.stage("asr_and_cleanup_tail", post_start);
    let mut was_cleaned_up = false;
    let snippets_start = Instant::now();

    // One last deterministic boundary pass catches artifacts from VAD
    // splits, e.g. "that Much" or "and then Than".
    let result = cleanup::repair_vad_boundary_artifacts(&formatted);

    // Snippet expansion runs last so expansions (URLs, emails, addresses)
    // survive as literal text through any cleanup pass.
    let result = snippets::apply_snippets(&result, &snips);

    trace.stage("boundary_and_snippets", snippets_start);
    let llm_start = Instant::now();
    let client = app_handle.state::<crate::llm::LlmClient>().inner().clone();
    let context = app_handle.state::<RecordingContext>().0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let dictionary_terms = context.as_ref().map(|context|context.terms.clone()).unwrap_or_default();
    let ctx = context.map(|context|context.resolve()).unwrap_or_else(crate::context::foreground);
    let ai_on = state.lock().await.settings.ai_cleanup;
    let needs_llm = ai_on && client.has_key() && crate::router::needs_llm(&result, &ctx.app);
    let mut fallback_reason: Option<String> = None;
    if let Some(capture) = capture_guard.session() {
        capture.set("deterministic_text", &result);
        capture.set("context", &ctx);
        capture.set("context_sent", needs_llm);
        capture.set("router_decision", serde_json::json!({"needs_llm":crate::router::needs_llm(&result, &ctx.app),"has_key":client.has_key(),"route":if needs_llm {"llm"} else {"skip"}}));
    }
    let speculative_outcome = if let Some(speculation) = speculation {
        let (outcome, metrics) = speculation.finish(&result).await;
        trace.speculation(metrics);
        outcome
    } else { None };
    let result = if needs_llm {
        // Speculative outcomes arrive as strings; a fresh call keeps the typed
        // error so the pill can say why AI cleanup was skipped.
        let outcome: Result<crate::llm::LlmOutcome, (String, Option<crate::llm::LlmError>)> = match speculative_outcome {
            Some(outcome) => outcome.map_err(|e| (e, None)),
            None => client
                .cleanup_with(&result, &ctx, &dictionary_terms, std::time::Duration::from_millis(2500))
                .await
                .map_err(|e| (e.to_string(), Some(e))),
        };
        match outcome {
            Ok(out) => {
                crate::ai::report(&app_handle, Ok(()));
                if let Some(capture) = capture_guard.session() { capture.set("llm", &out); }
                trace.llm(serde_json::json!({"ttft_ms":out.ttft_ms,"total_ms":out.total_ms,"cache_read_tokens":out.cache_read_tokens,"cache_write_tokens":out.cache_write_tokens,"input_tokens":out.input_tokens,"output_tokens":out.output_tokens}));
                was_cleaned_up = true; out.text
            }
            Err((err, typed)) => {
                fallback_reason = Some(match &typed {
                    Some(e) => { crate::ai::report(&app_handle, Err(e)); e.reason().to_string() }
                    None => reason_from_message(&err).to_string(),
                });
                if let Some(capture) = capture_guard.session() { capture.set("llm", serde_json::json!({"error":err})); }
                log::warn!("LLM cleanup failed: {err}; using deterministic text"); trace.llm(serde_json::json!({"error":err})); result
            }
        }
    } else { result };
    trace.stage("llm", llm_start);
    let word_count = result.split_whitespace().count();
    if let Some(capture) = capture_guard.session() { capture.set("final_text", &result); }

    // Inject text at cursor
    log::debug!("Injecting transcribed text: '{result}'");
    let text_for_inject = result.clone();
    let app_for_inject = app_handle.clone();
    let inject_result: Result<(), String> = {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app_for_inject
            .run_on_main_thread(move || {
                let r = inject::inject_text(&text_for_inject);
                let _ = tx.send(r);
            })
            .map_err(|e| format!("Dispatch failed: {e}"))?;
        rx.await.map_err(|e| format!("Channel failed: {e}"))?
    };

    let mut paste_blocked = false;
    if let Err(e) = inject_result {
        if let Some(capture) = capture_guard.session() { capture.set("error", &e); }
        log::error!("Injection failed: {e}");
        // Leave the text on the clipboard so the user can paste it themselves.
        if crate::clipboard_win::set_clipboard_with_exclusion(&result).is_err() {
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err("injection_failed".into());
        }
        paste_blocked = true;
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;
    if let Some(capture) = capture_guard.session() {
        capture.set("pasted_text", &result);
        capture.set("status", "pasted");
        capture.set("paste_confirmation", "SendInput dispatched; target application acknowledgement is not measured");
    }
    // Save to history and reset state
    let mut s = state.lock().await;
    s.history.push(TranscriptionEntry {
        timings: trace.snapshot(),
        text: result.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        word_count,
        duration_ms,
        speech_duration_ms,
        was_cleaned_up,
    });
    // Cap in-memory history to 1000 entries (matches save_history cap)
    if s.history.len() > 1000 {
        let excess = s.history.len() - 1000;
        s.history.drain(..excess);
    }
    let _ = history::save_history(&s.history);
    let new_entry = s.history.last().cloned();
    if let Some(entry) = &new_entry {
        let timestamp = entry.timestamp.clone();
        let history_state = state.inner().clone();
        let restore_trace = trace.clone();
        let capture = capture_guard.handoff();
        tauri::async_runtime::spawn(async move {
            restore_trace.wait_restored().await;
            let mut s = history_state.lock().await;
            if let Some(entry) = s.history.iter_mut().find(|e| e.timestamp == timestamp) { entry.timings = restore_trace.snapshot(); }
            let _ = history::save_history(&s.history);
            drop(s);
            if let Some(capture) = capture { capture.save(restore_trace.snapshot()); }
        });
    }
    s.recording_state = RecordingState::Idle;

    // If vocabulary terms changed since the last recording, rebuild the
    // recognizer NOW that the user has their text and we're back in Idle.
    // Doing this here instead of in start_recording avoids blocking the next
    // hotkey press for the ~3 second model load time.
    //
    // The rebuild is wrapped in catch_unwind so a sherpa-onnx C-side panic
    // produces an error log instead of killing the process. The recognizer
    // would then stay stale (old vocab) but the app keeps running.
    if s.recognizer_dirty {
        // Drop the old Arc BEFORE building the new recognizer. Any concurrent
        // user of the old recognizer (VAD receiver thread, in-flight transcribe)
        // already finished above. Forcing the Drop now means sherpa-onnx's
        // SherpaOnnxDestroyOfflineRecognizer runs synchronously, before we
        // allocate the next hotword automaton — minimizing the window where
        // both old and new recognizers exist simultaneously, which is the
        // configuration that empirically triggered heap corruption.
        s.recognizer = None;

        let model = s.settings.model.clone();
        let beam_search = s.settings.beam_search;
        let vocab = s.vocabulary.clone();
        if transcribe::model_exists(&model) {
            let load_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transcribe::load_model(&model, beam_search, &vocab)
            }));
            match load_result {
                Ok(Ok(rec)) => {
                    s.recognizer = Some(Arc::new(rec));
                    s.recognizer_dirty = false;
                    log::info!(
                        "Recognizer rebuilt post-recording with {} vocabulary terms",
                        vocab.len()
                    );
                }
                Ok(Err(e)) => {
                    log::error!("Failed to rebuild recognizer post-recording: {e}");
                    // Try to fall back to a no-vocab recognizer so dictation
                    // still works, just without hotword biasing.
                    if let Ok(rec) = transcribe::load_model(&model, beam_search, &[]) {
                        s.recognizer = Some(Arc::new(rec));
                        log::warn!("Fell back to recognizer without hotwords");
                    }
                    s.recognizer_dirty = false;
                }
                Err(panic) => {
                    let msg = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    log::error!(
                        "sherpa-onnx PANICKED during recognizer rebuild: {msg} — falling back to no-vocab recognizer"
                    );
                    if let Ok(rec) = transcribe::load_model(&model, beam_search, &[]) {
                        s.recognizer = Some(Arc::new(rec));
                    }
                    s.recognizer_dirty = false;
                }
            }
        }
    }

    // Notify all windows (including settings) that history changed
    if let Some(entry) = new_entry {
        let _ = app_handle.emit("history-updated", entry);
    }

    log::info!(
        "Transcription complete: {} chunk(s), {:.1}s audio, {}ms total",
        (sample_count as f32 / 16000.0 / 15.0).ceil() as u32,
        duration_secs,
        duration_ms
    );

    if paste_blocked {
        return Err("paste_blocked".into());
    }

    Ok(TranscriptionResult {
        text: result,
        word_count,
        duration_ms,
        was_cleaned_up,
        fallback_reason,
    })
}

/// Map a stringified `LlmError` (speculative path) to the pill reason.
fn reason_from_message(err: &str) -> &'static str {
    if err.contains("timed out") {
        "slow"
    } else if err.starts_with("http error") {
        "offline"
    } else if err.starts_with("api error 401") || err.starts_with("api error 403") {
        "key invalid"
    } else if err.starts_with("api error 429") {
        "rate limited"
    } else if err.starts_with("api error") {
        "API error"
    } else {
        "AI skipped"
    }
}

#[tauri::command]
pub async fn cancel_recording(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    stream_active_state: State<'_, StreamActiveState>,
) -> Result<(), String> {
    arm_escape(false);
    if let Some(speculation) = app_handle.state::<crate::speculative::RecordingSpeculation>().0.lock().unwrap_or_else(|e|e.into_inner()).take() { speculation.cancel(); }
    // Deactivate zombie callbacks before dropping the stream
    {
        let active = stream_active_state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref flag) = *active {
            flag.store(false, Ordering::SeqCst);
        }
    }

    // Stop stream
    {
        let mut handle = stream_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        *handle = None;
    }

    // Save cancelled opt-in recordings as incomplete examples, never as pasted text.
    {
        let mut audio = buffer.lock().unwrap_or_else(|e|e.into_inner());
        if let Some(trace) = crate::performance::Trace::current() {
            if let Some(capture) = trace.capture() {
                capture.audio(&audio);
                capture.set("status", "cancelled");
                capture.save(trace.snapshot());
            }
        }
        audio.clear();
    }

    // Reset state
    let mut s = state.lock().await;
    s.recording_state = RecordingState::Idle;

    Ok(())
}

#[tauri::command]
pub async fn download_model(
    model: String,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    transcribe::download_model(&model, app_handle.clone()).await?;

    // Download Silero VAD model alongside (small, ~2MB)
    if let Err(e) = settings::download_vad_model().await {
        log::warn!("VAD model download failed (non-fatal): {e}");
    }

    // Load the recognizer into app state immediately so recording works
    // without requiring a restart.
    let mut s = state.lock().await;
    let vocab = s.vocabulary.clone();
    let recognizer = transcribe::load_model(&model, s.settings.beam_search, &vocab)
        .map_err(|e| format!("Model downloaded but failed to load: {e}"))?;
    s.recognizer = Some(Arc::new(recognizer));
    log::info!("Recognizer loaded after model download");

    Ok(())
}

#[tauri::command]
pub async fn get_model_status(model: String) -> Result<ModelStatus, String> {
    Ok(ModelStatus {
        downloaded: transcribe::model_exists(&model),
        size_bytes: transcribe::model_size_bytes(&model),
        model,
    })
}

#[tauri::command]
pub async fn get_history(state: State<'_, SharedState>) -> Result<Vec<TranscriptionEntry>, String> {
    let s = state.lock().await;
    Ok(s.history.clone())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.lock().await;
    s.history.clear();
    history::save_history(&s.history)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_history_entry(
    timestamp: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut s = state.lock().await;
    s.history.retain(|e| e.timestamp != timestamp);
    history::save_history(&s.history)?;
    Ok(())
}

// ── Mic test command ──────────────────────────────────────────────────

#[tauri::command]
pub async fn test_microphone(
    app_handle: AppHandle,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    state: State<'_, SharedState>,
) -> Result<Vec<u8>, String> {
    let device_id = {
        let s = state.lock().await;
        s.settings.input_device.clone()
    };

    // Clear buffer before recording
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();

    // Start capture
    let (stream, _error_flag, active_flag, _resampler_state) =
        audio::start_capture(&device_id, buffer.inner().clone(), app_handle, None)?;
    *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));

    // Record for 3 seconds
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Deactivate zombie callbacks before dropping the stream
    active_flag.store(false, Ordering::SeqCst);

    // Stop capture
    *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Get audio and encode as WAV
    let wav_bytes = {
        let buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
        audio::encode_wav(&buf, 16000)?
    };

    // Clear the buffer
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();

    Ok(wav_bytes)
}

#[tauri::command]
pub async fn play_completion_sound() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        let cursor = Cursor::new(CHIRP_SOUND);
        let (_stream, stream_handle) =
            rodio::OutputStream::try_default().map_err(|e| format!("Audio output error: {e}"))?;
        let sink = rodio::Sink::try_new(&stream_handle).map_err(|e| format!("Sink error: {e}"))?;
        let source = rodio::Decoder::new(cursor).map_err(|e| format!("Decode error: {e}"))?;
        sink.append(source);
        sink.sleep_until_end();
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {e}"))?
}

#[tauri::command]
pub async fn get_hotkey_status(state: State<'_, SharedState>) -> Result<String, String> {
    let s = state.lock().await;
    let status = match s.hotkey_status {
        crate::state::HotkeyStatus::Idle => "idle",
        crate::state::HotkeyStatus::Retrying => "retrying",
        crate::state::HotkeyStatus::Active => "active",
        crate::state::HotkeyStatus::Failed => "failed",
    };
    Ok(status.to_string())
}

// ── System-level key capture ──────────────────────────────────────────

#[tauri::command]
pub async fn record_binding(
    state: State<'_, crate::state::SharedState>,
    target: String,
) -> Result<crate::hotkey::Binding, String> {
    let other = {
        let s = state.lock().await;
        match target.as_str() {
            "hold" => s.settings.toggle_binding.clone(),
            "toggle" => s.settings.hold_binding.clone(),
            _ => return Err("Unknown binding target".into()),
        }
    };
    crate::hotkey::record_binding(other).await
}

#[tauri::command]
pub fn cancel_record_binding() -> Result<(), String> { crate::hotkey::cancel_record_binding() }
