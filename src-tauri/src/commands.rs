use crate::audio;
use crate::cleanup;
use crate::history;
use crate::inject;
use crate::llm;
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
const MIN_AI_CLEANUP_WORDS: usize = 7;

/// Join VAD segments with boundary cleanup. When VAD splits mid-sentence,
/// each segment starts with a capital letter and may end with a period that
/// doesn't belong. This fixes the most obvious artifacts.
///
/// Currently unused — retained because the streaming cleanup path (v3) does
/// per-segment LLM cleanup instead, but we may want this helper again if the
/// streaming path is reverted.
#[allow(dead_code)]
fn join_vad_segments(segments: &[String]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let mut result = String::new();

    for (i, seg) in segments.iter().enumerate() {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }

        if i == 0 {
            // First segment: keep as-is
            result.push_str(trimmed);
            continue;
        }

        // Check if previous segment ended with sentence-ending punctuation
        let prev_ended_sentence =
            result.ends_with('.') || result.ends_with('?') || result.ends_with('!');

        // If previous segment ended with a period but was very short (1-3 words),
        // it's likely a VAD artifact — strip the period
        if result.ends_with('.') {
            let last_segment_words = result
                .rsplit(' ')
                .take_while(|w| !w.ends_with('.') && !w.ends_with('?') && !w.ends_with('!'))
                .count();
            // If the trailing sentence fragment is 1-3 words, strip the period
            // (e.g., "Time." or "Is like." are artifacts)
            if last_segment_words <= 3 {
                // Check the actual last "sentence" by finding the last sentence break before this one
                let before_period = &result[..result.len() - 1];
                let last_real_end = before_period.rfind(|c: char| c == '.' || c == '?' || c == '!');
                let fragment = match last_real_end {
                    Some(pos) => &before_period[pos + 1..],
                    None => before_period,
                };
                let word_count = fragment.split_whitespace().count();
                if word_count <= 3 {
                    result.pop(); // remove the trailing period
                }
            }
        }

        result.push(' ');

        let prev_ended_sentence_now = result.trim_end().ends_with('.')
            || result.trim_end().ends_with('?')
            || result.trim_end().ends_with('!');

        if prev_ended_sentence_now {
            // Previous was a real sentence end — keep the capitalization
            result.push_str(trimmed);
        } else {
            // Mid-sentence join — lowercase the first character
            let mut chars = trimmed.chars();
            if let Some(first) = chars.next() {
                for c in first.to_lowercase() {
                    result.push(c);
                }
                result.push_str(chars.as_str());
            }
        }
    }

    result
}

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
pub struct StreamActiveState(pub std::sync::Mutex<Option<audio::StreamActiveFlag>>);

#[tauri::command]
pub fn show_settings(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
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
    let old_hotkey = s.settings.hotkey.clone();

    // Merge partial into current settings
    let mut settings_val = serde_json::to_value(&s.settings).unwrap();
    if let (Some(base), Some(patch)) = (settings_val.as_object_mut(), partial.as_object()) {
        for (k, v) in patch {
            base.insert(k.clone(), v.clone());
        }
    }
    s.settings =
        serde_json::from_value(settings_val).map_err(|e| format!("Invalid settings: {e}"))?;
    settings::save_settings(&s.settings)?;

    // If history retention changed, prune immediately
    let new_retention = s.settings.history_retention_days;
    if partial.get("historyRetentionDays").is_some() && new_retention > 0 {
        history::prune_history(&mut s.history, new_retention);
    }

    // Broadcast settings change to all windows (cross-window sync)
    let _ = app_handle.emit("settings-changed", &partial);

    // Notify frontend to refresh history if retention changed
    if partial.get("historyRetentionDays").is_some() {
        let _ = app_handle.emit("history-changed", ());
    }

    if partial.get("hotkeyMode").is_some() {
        log::info!("Hotkey mode updated: {:?}", s.settings.hotkey_mode);
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
    if s.settings.hotkey != old_hotkey {
        let new_hotkey = s.settings.hotkey.clone();
        drop(s);
        match crate::hotkey::update(&new_hotkey, app_handle.clone()) {
            Ok(()) => {
                log::info!("Re-registered hotkey: {new_hotkey}");
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

    // Check model is loaded
    if s.recognizer.is_none() {
        return Err("model_not_loaded".into());
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
    let vocab_for_vad = s.vocabulary.clone();
    s.recording_state = RecordingState::Recording;
    s.vad_was_active = false;
    drop(s);

    // Set up VAD streaming if the model is available. Segments are transcribed
    // and AI-cleaned as they arrive so stop_recording has no LLM latency.
    let vad_model = settings::vad_model_path();
    let vad_arc: Option<Arc<std::sync::Mutex<crate::audio::VadState>>> = if vad_model.exists() {
        let (tx, rx) = crossbeam_channel::unbounded::<Vec<f32>>();
        *vad_sender.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx.clone());

        match audio::create_vad_state(&vad_model.to_string_lossy(), tx) {
            Some(vs) => {
                let vad_arc = Arc::new(std::sync::Mutex::new(vs));
                *vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(vad_arc.clone());

                let transcripts = vad_transcripts.inner().clone();
                let cleaned = vad_cleaned.0.clone();
                let state_for_thread = state.inner().clone();
                let rec = recognizer_for_vad.clone().unwrap();

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

                            // Regex/vocab pre-pass. Snippet expansion is
                            // deferred to after LLM cleanup so the LLM
                            // doesn't mangle URLs / emails / identifiers.
                            let after_regex = cleanup::cleanup_text(&raw, smart_fmt_for_vad);
                            let after_vocab =
                                cleanup::apply_replacements(&after_regex, &vocab_for_vad);

                            // No per-segment LLM cleanup — the joined text
                            // gets a single polish pass in stop_recording
                            // after the hotkey is released.
                            cleaned
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push(after_vocab);
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
    let capture_err = match audio::start_capture(
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
    };

    if let Some(err) = capture_err {
        // Reset state so future recordings aren't permanently blocked
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        return Err(err);
    }

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
    let start_time = Instant::now();

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

    // Stop the audio stream
    {
        let mut handle = stream_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        *handle = None; // Drop stream → stops capture
    }

    // Flush any remaining resampler samples into the audio buffer
    {
        let flush_state = resampler_flush.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref rs) = *flush_state {
            rs.flush(buffer.inner());
        }
    }

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

    // Flush VAD to capture the final speech segment, then shut down receiver thread
    {
        let flush = vad_flush_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref vad_arc) = *flush {
            audio::flush_vad(vad_arc);
        }
    }
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

    // Audio callbacks run on a separate thread; give in-flight callbacks
    // time to finish before we read the buffer.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

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

    // Log audio level to diagnose silent buffer issues
    let rms = (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32).sqrt();
    let peak = audio_data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let nonzero = audio_data.iter().filter(|&&s| s.abs() > 0.001).count();
    log::info!(
        "Audio level: rms={rms:.6}, peak={peak:.4}, nonzero={nonzero}/{sample_count} ({:.1}%)",
        nonzero as f64 / sample_count as f64 * 100.0
    );

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
    let (
        recognizer,
        smart_fmt,
        vocab,
        snips,
        ai_cleanup,
        llm_port,
        tone_mode,
        http_client,
        vad_was_active,
    ) = {
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
            s.settings.ai_cleanup,
            s.llm_port,
            s.settings.tone_mode.clone(),
            s.http_client.clone(),
            vad_was_active,
        )
    };

    let had_vocabulary = !vocab.is_empty();

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
    log::info!(
        "transcription path: vad_was_active={}, vad_texts_count={}, vad_cleaned_count={}, fallback_used={}",
        vad_was_active,
        vad_texts.len(),
        vad_cleaned_texts.len(),
        !vad_was_active
    );

    // The streaming path bypasses the blocking ASR fallback. It still runs the
    // same single LLM cleanup pass after the hotkey is released.
    let (formatted_opt, streaming_was_cleaned_up) = if use_vad {
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
        // VAD segments are pre-cleaned (regex/vocab/snippets) but LLM cleanup
        // is deferred to the single post-release pass below.
        (Some(joined), false)
    } else {
        (None, false)
    };

    let result = if let Some(text) = formatted_opt {
        Ok(text)
    } else {
        tokio::task::spawn_blocking(move || {
            // Fallback: chunk the full audio buffer and transcribe (original behavior)
            let chunks = crate::transcribe::chunk_audio(&audio_data, 16000, 30.0, 1.0);
            log::info!("Starting Parakeet TDT transcription ({} chunk(s))...", chunks.len());

            let mut transcriptions = Vec::new();
            for (i, chunk) in chunks.iter().enumerate() {
                let chunk_rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
                let chunk_peak = chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
                let chunk_nonzero = chunk.iter().filter(|&&s| s.abs() > 0.001).count();
                log::info!("Chunk {i}: {} samples ({:.1}s), rms={chunk_rms:.6}, peak={chunk_peak:.4}, nonzero={chunk_nonzero}/{} ({:.1}%)",
                    chunk.len(), chunk.len() as f32 / 16000.0,
                    chunk.len(), chunk_nonzero as f64 / chunk.len() as f64 * 100.0);

                let chunk_raw = transcribe::transcribe(&recognizer, chunk)
                    .map_err(|e| {
                        log::error!("Transcription error on chunk {i}: {e}");
                        "transcription_failed".to_string()
                    })?;
                log::info!("Parakeet chunk {i}: '{chunk_raw}'");
                transcriptions.push(chunk_raw);
            }

            let merged = crate::transcribe::merge_transcriptions(transcriptions);
            if merged.is_empty() {
                log::warn!("Transcription returned empty text");
                return Err("transcription_failed".to_string());
            }

            // Regex pre-pass: remove fillers, spoken punctuation, format numbers
            let formatted = cleanup::cleanup_text(&merged, smart_fmt);

            // Vocabulary find/replace: deterministic post-ASR fixup for things
            // hotword biasing can't fix (homophones, brand spellings, stable
            // mishearings). Each VocabEntry's `replaces` list maps mishearings
            // to its canonical `term`, case-insensitive at word boundaries.
            let after_replace = cleanup::apply_replacements(&formatted, &vocab);

            // Snippet expansion is deferred to after LLM cleanup (below)
            // so the LLM doesn't mangle URLs / emails / identifiers.

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

    // AI cleanup pass (if enabled and server is running). Both VAD and
    // fallback paths arrive here after deterministic regex/vocabulary cleanup.
    let mut was_cleaned_up = streaming_was_cleaned_up;
    let formatted_word_count = formatted.split_whitespace().count();
    let skip_ai_for_short_message =
        tone_mode != "email" && formatted_word_count < MIN_AI_CLEANUP_WORDS;
    let result = if !streaming_was_cleaned_up
        && ai_cleanup
        && llm_port.is_some()
        && !skip_ai_for_short_message
    {
        let port = llm_port.unwrap();
        let _ = app_handle.emit("recording-state", "polishing");
        log::info!("Running AI cleanup on text...");
        match llm::cleanup_text(port, &formatted, &tone_mode, &http_client).await {
            Ok(cleaned) => {
                log::info!("LLM cleanup: '{cleaned}'");
                was_cleaned_up = true;
                cleaned
            }
            Err(e) => {
                log::warn!("AI cleanup failed, using regex-only result: {e}");
                formatted
            }
        }
    } else {
        if !streaming_was_cleaned_up
            && ai_cleanup
            && llm_port.is_some()
            && skip_ai_for_short_message
        {
            log::info!("Skipping AI cleanup for short message ({formatted_word_count} words)");
        }
        formatted
    };

    // One last deterministic boundary pass catches artifacts the LLM may
    // preserve from VAD splits, e.g. "that Much" or "and then Than".
    let result = cleanup::repair_vad_boundary_artifacts(&result);

    // Snippet expansion happens AFTER LLM cleanup so expansions (URLs,
    // emails, addresses) survive as literal text — the LLM would otherwise
    // "correct" things like "chirptype.com" into "chirptype. Com".
    let result = snippets::apply_snippets(&result, &snips);

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let word_count = result.split_whitespace().count();

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

    if let Err(e) = inject_result {
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        log::error!("Injection failed: {e}");
        // Pass through accessibility_denied so the frontend can show a specific message
        if e == "accessibility_denied" {
            return Err("accessibility_denied".into());
        }
        return Err("injection_failed".into());
    }

    // Save to history and reset state
    let mut s = state.lock().await;
    s.history.push(TranscriptionEntry {
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

    // Track dictation_completed telemetry (no-op if help_improve is off)
    {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app_handle.track_event(
            "dictation_completed",
            Some(serde_json::json!({
                "duration_seconds": (duration_ms as f64 / 1000.0),
                "word_count": word_count,
                "used_ai_cleanup": was_cleaned_up,
                "used_vocabulary": had_vocabulary,
            })),
        );
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

    Ok(TranscriptionResult {
        text: result,
        word_count,
        duration_ms,
        was_cleaned_up,
    })
}

#[tauri::command]
pub async fn cancel_recording(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    stream_active_state: State<'_, StreamActiveState>,
) -> Result<(), String> {
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

    // Clear buffer
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();

    // Reset state
    let mut s = state.lock().await;
    s.recording_state = RecordingState::Idle;
    drop(s);

    // Track dictation_cancelled telemetry (no-op if help_improve is off)
    {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app_handle.track_event("dictation_cancelled", None);
    }

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

    // Track model_downloaded telemetry (no-op if help_improve is off)
    {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app_handle.track_event(
            "model_downloaded",
            Some(serde_json::json!({
                "model": &model,
            })),
        );
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

// ── LLM commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_llm_status(state: State<'_, SharedState>) -> Result<llm::LlmStatus, String> {
    let mut s = state.lock().await;
    let mut server_running = s.llm_port.is_some();

    let child_status = s.llm_process.as_mut().map(|child| child.try_wait());
    match child_status {
        Some(Ok(Some(status))) => {
            log::warn!("llama-server exited unexpectedly: {status}");
            s.llm_process = None;
            s.llm_port = None;
            llm::clear_server_pid();
            server_running = false;
        }
        Some(Ok(None)) => {}
        Some(Err(e)) => {
            log::warn!("Failed to check llama-server status: {e}");
            s.llm_process = None;
            s.llm_port = None;
            llm::clear_server_pid();
            server_running = false;
        }
        None if s.llm_port.is_some() => {
            s.llm_port = None;
            llm::clear_server_pid();
            server_running = false;
        }
        None => {}
    }

    Ok(llm::LlmStatus {
        binary_downloaded: llm::binary_exists(),
        model_downloaded: llm::model_exists(),
        server_running,
    })
}

#[tauri::command]
pub async fn download_llm(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    llm::download_binary(&app_handle).await?;
    llm::download_model(&app_handle).await?;
    Ok(())
}

#[tauri::command]
pub async fn start_llm(state: State<'_, SharedState>) -> Result<(), String> {
    {
        let s = state.lock().await;
        if s.llm_port.is_some() {
            return Ok(()); // Already running
        }
    }

    // Pick a random port in the ephemeral range
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to find free port: {e}"))?;
        listener
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {e}"))?
            .port()
    };

    let child = llm::start_server(port).await?;

    let mut s = state.lock().await;
    if let Some(pid) = child.id() {
        llm::save_server_pid(pid);
    }
    s.llm_process = Some(child);
    s.llm_port = Some(port);

    Ok(())
}

#[tauri::command]
pub async fn stop_llm(state: State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.lock().await;
    if let Some(ref mut child) = s.llm_process {
        llm::stop_server(child).await;
    }
    s.llm_process = None;
    s.llm_port = None;
    llm::clear_server_pid();
    Ok(())
}

#[tauri::command]
pub async fn test_llm_cleanup(
    text: String,
    mode: Option<String>,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let (port, http_client) = {
        let s = state.lock().await;
        (
            s.llm_port.ok_or("LLM server is not running")?,
            s.http_client.clone(),
        )
    };
    llm::cleanup_text(
        port,
        &text,
        &mode.unwrap_or_else(|| "message".to_string()),
        &http_client,
    )
    .await
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
        crate::state::HotkeyStatus::AccessibilityRequired => "accessibility_required",
    };
    Ok(status.to_string())
}

// ── Mic permission (macOS early-trigger) ───────────────────────────────

#[tauri::command]
pub async fn request_mic_permission() -> Result<bool, String> {
    // Briefly open the default audio input to trigger the macOS permission dialog.
    // On Windows/Linux this is a no-op (permissions granted by default).
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(device) => {
            // Try to get the default config — this triggers the permission dialog on macOS
            match device.default_input_config() {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
        None => Ok(false),
    }
}

// ── Announcements commands ─────────────────────────────────────────────

#[tauri::command]
pub async fn get_announcements() -> Result<Vec<crate::announcements::Announcement>, String> {
    let app_version = env!("CARGO_PKG_VERSION");
    Ok(crate::announcements::fetch_announcements(app_version).await)
}

#[tauri::command]
pub async fn dismiss_announcement(id: String) -> Result<(), String> {
    let mut seen = crate::announcements::load_seen();
    if !seen.contains(&id) {
        seen.push(id);
        crate::announcements::save_seen(&seen)?;
    }
    Ok(())
}

// ── Feedback command ───────────────────────────────────────────────────

#[tauri::command]
pub async fn send_feedback(text: String, state: State<'_, SharedState>) -> Result<(), String> {
    crate::feedback::send_feedback_command(text, state.inner()).await
}

// ── Accessibility permission (macOS) ──────────────────────────────────

#[tauri::command]
pub async fn check_accessibility_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        Ok(unsafe { AXIsProcessTrusted() })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

#[tauri::command]
pub async fn request_accessibility_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::nil;
        use cocoa::foundation::{NSDictionary, NSString};
        use objc::class;
        use objc::msg_send;
        use objc::runtime::Object;
        use objc::sel;
        use objc::sel_impl;
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const Object) -> bool;
        }
        unsafe {
            let key = NSString::alloc(nil).init_str("AXTrustedCheckOptionPrompt");
            let value: *mut Object = msg_send![class!(NSNumber), numberWithBool: true];
            let options = NSDictionary::dictionaryWithObject_forKey_(nil, value, key);
            AXIsProcessTrustedWithOptions(options as *const Object);
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

// ── System-level key capture ──────────────────────────────────────────

#[tauri::command]
pub async fn capture_next_key(
    state: State<'_, crate::state::SharedState>,
    app_handle: AppHandle,
) -> Result<crate::hotkey::CapturedKey, String> {
    let result = crate::hotkey::capture_next_key().await;
    // Resume the main hotkey grab after capture
    let s = state.lock().await;
    let hotkey_str = s.settings.hotkey.clone();
    drop(s);
    let _ = crate::hotkey::start(&hotkey_str, app_handle);
    result
}
