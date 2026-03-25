use crate::audio;
use crate::cleanup;
use crate::dictionary;
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
use tauri::{AppHandle, Emitter, State};
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
pub struct StreamActiveState(pub std::sync::Mutex<Option<audio::StreamActiveFlag>>);


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

    // Sync autostart
    let autostart = app_handle.autolaunch();
    if s.settings.launch_at_login {
        let _ = autostart.enable();
    } else {
        let _ = autostart.disable();
    }

    // Re-register global shortcut if hotkey changed
    if s.settings.hotkey != old_hotkey {
        let new_hotkey = s.settings.hotkey.clone();
        drop(s); // Release lock before accessing shortcut plugin

        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let gs = app_handle.global_shortcut();
        let _ = gs.unregister_all();

        if let Ok(shortcut) = new_hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = gs.on_shortcut(shortcut, move |app, _shortcut, event| {
                match event.state {
                    tauri_plugin_global_shortcut::ShortcutState::Pressed => {
                        log::info!("Hotkey pressed → start recording");
                        let _ = app.emit("hotkey-pressed", ());
                    }
                    tauri_plugin_global_shortcut::ShortcutState::Released => {
                        log::info!("Hotkey released → stop recording");
                        let _ = app.emit("hotkey-released", ());
                    }
                }
            });
            log::info!("Re-registered global hotkey: {new_hotkey}");
            let _ = app_handle.emit("hotkey-status", "active");
        } else {
            log::error!("Failed to parse new hotkey: {new_hotkey}");
            let _ = app_handle.emit("hotkey-status", "failed");
        }
    } else {
        drop(s);
    }

    Ok(())
}

#[tauri::command]
pub async fn update_dictionary(
    entries: Vec<DictionaryEntry>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    if entries.len() > 500 {
        return Err("Dictionary cannot exceed 500 entries".to_string());
    }
    let mut s = state.lock().await;
    s.dictionary = entries.clone();
    settings::save_dictionary(&s.dictionary)?;
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
) -> Result<(), String> {
    let mut s = state.lock().await;

    if s.recording_state != RecordingState::Idle {
        return Err("Already recording".into());
    }

    // Check model is loaded
    if s.recognizer.is_none() {
        return Err("model_not_loaded".into());
    }

    // Deactivate any zombie callbacks from a previous stream before clearing the buffer.
    // On macOS, cpal/CoreAudio callbacks can outlive the Stream drop.
    {
        let prev_active = stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref flag) = *prev_active {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    // Clear audio buffer
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();

    // Record wall-clock start time for sample rate sanity check
    *recording_start.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());

    let device_id = s.settings.input_device.clone();
    s.recording_state = RecordingState::Recording;
    drop(s);

    // Start audio capture — convert Result to Option immediately so the
    // non-Send cpal::Stream doesn't live across an await point.
    let capture_err = match audio::start_capture(
        &device_id,
        buffer.inner().clone(),
        app_handle.clone(),
    ) {
        Ok((stream, error_flag, active_flag, resampler_state)) => {
            *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));
            *stream_error_state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(error_flag);
            *stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(active_flag);
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
) -> Result<TranscriptionResult, String> {
    let start_time = Instant::now();

    // Grab wall-clock recording duration for sample rate sanity check
    let wall_clock_secs = recording_start.0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .map(|t| t.elapsed().as_secs_f32());

    // Deactivate callbacks BEFORE dropping the stream — this ensures zombie
    // callbacks from macOS CoreAudio cannot write to the buffer anymore.
    {
        let active = stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner());
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
    let had_stream_error = stream_error_state.0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or(false, |flag| flag.load(Ordering::SeqCst));
    if had_stream_error {
        log::warn!("Audio stream reported an error during recording — device may have disconnected");
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
    log::info!("Audio level: rms={rms:.6}, peak={peak:.4}, nonzero={nonzero}/{sample_count} ({:.1}%)",
        nonzero as f64 / sample_count as f64 * 100.0);

    // If the buffer is near-silent and the stream had an error, the mic likely disconnected
    if rms < 0.0001 && had_stream_error {
        log::error!("Silent buffer with stream error — audio device likely disconnected during recording");
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
    let (recognizer, smart_fmt, dict, snips, ai_cleanup, llm_port, tone_mode) = {
        let s = state.lock().await;
        let rec = s.recognizer.clone().ok_or("model_not_loaded".to_string())?;
        (
            rec,
            s.settings.smart_formatting,
            s.dictionary.clone(),
            s.snippets.clone(),
            s.settings.ai_cleanup,
            s.llm_port,
            s.settings.tone_mode.clone(),
        )
    };

    let had_dictionary = !dict.is_empty();

    // Transcribe using the sherpa-onnx recognizer.
    // Chunk long audio into 30s segments (1s overlap) so the 0.6B model
    // stays in its comfort zone — longer buffers produce gibberish.
    let result = tokio::task::spawn_blocking(move || {
        let chunks = crate::transcribe::chunk_audio(&audio_data, 16000, 30.0, 1.0);
        log::info!("Starting Parakeet TDT transcription ({} chunk(s))...", chunks.len());

        let mut transcriptions = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            // Per-chunk audio diagnostics
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

        let raw = crate::transcribe::merge_transcriptions(transcriptions);

        if raw.is_empty() {
            log::warn!("Transcription returned empty text");
            return Err("transcription_failed".to_string());
        }

        // Cleanup/formatting
        let formatted = cleanup::cleanup_text(&raw, smart_fmt);

        // Apply dictionary and snippet expansions BEFORE AI cleanup so the
        // LLM doesn't alter trigger phrases (e.g. "my email" → "My E-mail").
        let after_dict = dictionary::apply_dictionary(&formatted, &dict);
        let after_snips = snippets::apply_snippets(&after_dict, &snips);

        log::info!("After regex+dict+snips: '{after_snips}'");
        Ok(after_snips)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?;

    // If transcription failed, reset state before returning error
    let formatted = match result {
        Ok(text) => text,
        Err(e) => {
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err(e);
        }
    };

    // AI cleanup pass (if enabled and server is running)
    let mut was_cleaned_up = false;
    let after_llm = if ai_cleanup && llm_port.is_some() {
        let port = llm_port.unwrap();
        let _ = app_handle.emit("recording-state", "polishing");
        log::info!("Running AI cleanup on text...");
        match llm::cleanup_text(port, &formatted, &tone_mode).await {
            Ok(cleaned) => {
                log::info!("LLM cleanup: '{cleaned}'");
                was_cleaned_up = true;
                cleaned
            }
            Err(e) => {
                log::warn!("AI cleanup failed, using regex-only result: {e}");
                formatted.clone()
            }
        }
    } else {
        formatted.clone()
    };

    let result = after_llm;

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

    // Track dictation_completed telemetry (no-op if help_improve is off)
    {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app_handle.track_event("dictation_completed", Some(serde_json::json!({
            "duration_seconds": (duration_ms as f64 / 1000.0),
            "word_count": word_count,
            "used_ai_cleanup": was_cleaned_up,
            "used_dictionary": had_dictionary,
        })));
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
        let active = stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner());
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

    // Track model_downloaded telemetry (no-op if help_improve is off)
    {
        use tauri_plugin_aptabase::EventTracker;
        let _ = app_handle.track_event("model_downloaded", Some(serde_json::json!({
            "model": &model,
        })));
    }

    // Load the recognizer into app state immediately so recording works
    // without requiring a restart.
    let recognizer = transcribe::load_model(&model)
        .map_err(|e| format!("Model downloaded but failed to load: {e}"))?;
    let mut s = state.lock().await;
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
    let (stream, _error_flag, active_flag, _resampler_state) = audio::start_capture(&device_id, buffer.inner().clone(), app_handle)?;
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
pub async fn get_llm_status(
    state: State<'_, SharedState>,
) -> Result<llm::LlmStatus, String> {
    let s = state.lock().await;
    Ok(llm::LlmStatus {
        binary_downloaded: llm::binary_exists(),
        model_downloaded: llm::model_exists(),
        server_running: s.llm_port.is_some(),
    })
}

#[tauri::command]
pub async fn download_llm(
    app_handle: AppHandle,
) -> Result<(), String> {
    llm::download_binary(&app_handle).await?;
    llm::download_model(&app_handle).await?;
    Ok(())
}

#[tauri::command]
pub async fn start_llm(
    state: State<'_, SharedState>,
) -> Result<(), String> {
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
        listener.local_addr()
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
pub async fn stop_llm(
    state: State<'_, SharedState>,
) -> Result<(), String> {
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
    let port = {
        let s = state.lock().await;
        s.llm_port.ok_or("LLM server is not running")?
    };
    llm::cleanup_text(port, &text, &mode.unwrap_or_else(|| "message".to_string())).await
}

#[tauri::command]
pub async fn play_completion_sound() -> Result<(), String> {
    tokio::task::spawn_blocking(|| {
        let cursor = Cursor::new(CHIRP_SOUND);
        let (_stream, stream_handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("Audio output error: {e}"))?;
        let sink = rodio::Sink::try_new(&stream_handle)
            .map_err(|e| format!("Sink error: {e}"))?;
        let source = rodio::Decoder::new(cursor)
            .map_err(|e| format!("Decode error: {e}"))?;
        sink.append(source);
        sink.sleep_until_end();
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {e}"))?
}

#[tauri::command]
pub async fn get_hotkey_status(
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let s = state.lock().await;
    let status = match s.hotkey_status {
        crate::state::HotkeyStatus::Idle => "idle",
        crate::state::HotkeyStatus::Retrying => "retrying",
        crate::state::HotkeyStatus::Active => "active",
        crate::state::HotkeyStatus::Failed => "failed",
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
pub async fn send_feedback(
    text: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    crate::feedback::send_feedback_command(text, state.inner()).await
}

