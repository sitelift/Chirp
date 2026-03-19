use crate::audio;
use crate::cleanup;
use crate::dictionary;
use crate::history;
use crate::hotkey;
use crate::inject;
use crate::llm;
use crate::settings;
use crate::snippets;
use crate::state::*;
use crate::transcribe;
use std::io::Cursor;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

const CHIRP_SOUND: &[u8] = include_bytes!("../sounds/chirp.wav");

/// Active audio stream handle — wrapped in an unsafe Send wrapper because
/// cpal::Stream is !Send but we only access it from the main thread.
pub struct StreamHandle(pub std::sync::Mutex<Option<StreamWrapper>>);

pub struct StreamWrapper(cpal::Stream);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}


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
    let old_hotkey_mode = s.settings.hotkey_mode.clone();
    let old_hotkey_keycode = s.settings.hotkey_keycode;

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

    // Re-register global shortcut if hotkey changed
    if s.settings.hotkey != old_hotkey {
        let new_hotkey = s.settings.hotkey.clone();

        // Sync autostart before dropping lock
        let autostart = app_handle.autolaunch();
        if s.settings.launch_at_login {
            let _ = autostart.enable();
        } else {
            let _ = autostart.disable();
        }
        drop(s); // Release lock before accessing shortcut plugin

        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let gs = app_handle.global_shortcut();

        // Unregister all existing shortcuts
        let _ = gs.unregister_all();

        // Register the new one
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
        } else {
            log::error!("Failed to parse new hotkey: {new_hotkey}");
        }
    } else {
        // Sync autostart with launch_at_login setting
        let autostart = app_handle.autolaunch();
        let launch_at_login = s.settings.launch_at_login;
        let new_mode = s.settings.hotkey_mode.clone();
        let new_keycode = s.settings.hotkey_keycode;
        drop(s); // Release lock before restart

        if launch_at_login {
            let _ = autostart.enable();
        } else {
            let _ = autostart.disable();
        }

        // Auto-restart key listener if mode or keycode changed
        if new_mode != old_hotkey_mode || new_keycode != old_hotkey_keycode {
            hotkey::stop_key_listener();
            if new_mode == "dedicated_key" && new_keycode > 0 {
                let shared = state.inner().clone();
                hotkey::start_key_listener(app_handle.clone(), new_keycode, shared);
            } else {
                let mut s = state.lock().await;
                s.hotkey_status = crate::state::HotkeyStatus::Idle;
                let _ = app_handle.emit("hotkey-status", "idle");
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn update_dictionary(
    entries: Vec<DictionaryEntry>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
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
) -> Result<(), String> {
    let mut s = state.lock().await;

    if s.recording_state != RecordingState::Idle {
        return Err("Already recording".into());
    }

    // Check model is loaded
    if s.recognizer.is_none() {
        return Err("model_not_loaded".into());
    }

    // Clear audio buffer
    buffer.lock().unwrap_or_else(|e| e.into_inner()).clear();

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
        Ok(stream) => {
            *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));
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

    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
) -> Result<TranscriptionResult, String> {
    let start_time = Instant::now();

    // Stop the audio stream
    {
        let mut handle = stream_handle.0.lock().unwrap_or_else(|e| e.into_inner());
        *handle = None; // Drop stream → stops capture
    }

    // WASAPI audio callbacks run on a separate thread; give in-flight callbacks
    // time to finish before we read the buffer.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    {
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Processing;
    }

    let _ = app_handle.emit("recording-state", "processing");

    // Get the audio data, prepending 150ms of silence so the resampler's
    // internal delay and model warm-up don't eat the first word.
    // Clear the buffer afterward to prevent stale data on next recording.
    let audio_data = {
        let mut buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
        let pad_samples = 16000 * 150 / 1000; // 150ms at 16kHz
        let mut padded = vec![0.0f32; pad_samples];
        padded.extend_from_slice(&buf);
        buf.clear();
        padded
    };

    let sample_count = audio_data.len();
    let duration_secs = sample_count as f32 / 16000.0;
    let speech_duration_ms = (duration_secs * 1000.0) as u64;

    log::info!("Audio buffer: {sample_count} samples ({duration_secs:.1}s)");

    if audio_data.is_empty() {
        log::error!("Audio buffer is empty!");
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        return Err("transcription_failed".into());
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

    // Transcribe using the sherpa-onnx recognizer.
    // The recognizer Arc is cloned above so the state lock is NOT held during inference.
    let result = tokio::task::spawn_blocking(move || {
        log::info!("Starting Parakeet TDT transcription...");
        let raw = transcribe::transcribe(&recognizer, &audio_data)
            .map_err(|e| {
                log::error!("Transcription error: {e}");
                "transcription_failed".to_string()
            })?;

        log::debug!("Transcription raw output: '{raw}'");

        if raw.is_empty() {
            log::warn!("Transcription returned empty text");
            return Err("transcription_failed".to_string());
        }

        // Cleanup/formatting — skip list detection when AI cleanup will handle it
        let formatted = if ai_cleanup {
            cleanup::cleanup_text_for_ai(&raw, smart_fmt)
        } else {
            cleanup::cleanup_text(&raw, smart_fmt)
        };

        // Apply dictionary and snippet expansions BEFORE AI cleanup so the
        // LLM doesn't alter trigger phrases (e.g. "my email" → "My E-mail").
        let after_dict = dictionary::apply_dictionary(&formatted, &dict);
        let after_snips = snippets::apply_snippets(&after_dict, &snips);

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
                log::debug!("AI cleanup result: '{cleaned}'");
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
    let _ = history::save_history(&s.history);
    let new_entry = s.history.last().cloned();
    s.recording_state = RecordingState::Idle;

    // Notify all windows (including settings) that history changed
    if let Some(entry) = new_entry {
        let _ = app_handle.emit("history-updated", entry);
    }

    Ok(TranscriptionResult {
        text: result,
        word_count,
        duration_ms,
        was_cleaned_up,
    })
}

#[tauri::command]
pub async fn cancel_recording(
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
) -> Result<(), String> {
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

    Ok(())
}

#[tauri::command]
pub async fn download_model(model: String, app_handle: AppHandle) -> Result<(), String> {
    transcribe::download_model(&model, app_handle).await
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
    let stream = audio::start_capture(&device_id, buffer.inner().clone(), app_handle)?;
    *stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));

    // Record for 3 seconds
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

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

// ── File transcription ────────────────────────────────────────────────

#[tauri::command]
pub async fn transcribe_file(
    path: String,
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<FileTranscriptionResult, String> {
    use crate::file_transcribe;

    let file_path = std::path::PathBuf::from(&path);
    if !file_path.exists() {
        return Err("File not found".to_string());
    }

    let _ = app_handle.emit("file-transcribe-progress", serde_json::json!({"phase": "decoding", "progress": 0}));

    // Decode audio file (blocking - heavy CPU work)
    let (samples, _sample_rate) = tokio::task::spawn_blocking(move || {
        file_transcribe::decode_audio_file(&file_path)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))??;

    let duration_secs = samples.len() as f32 / 16000.0;
    let _ = app_handle.emit("file-transcribe-progress", serde_json::json!({"phase": "transcribing", "progress": 0}));

    // Chunk the audio
    let chunks = file_transcribe::chunk_audio(&samples, 16000, 30.0, 1.0);
    let total_chunks = chunks.len();
    let mut transcriptions = Vec::new();

    // Clone the recognizer Arc so we don't hold the state lock during transcription
    let recognizer = {
        let s = state.lock().await;
        s.recognizer.clone().ok_or("model_not_loaded".to_string())?
    };

    // Transcribe each chunk
    for (i, chunk) in chunks.into_iter().enumerate() {
        let chunk_owned = chunk.to_vec();
        let rec = recognizer.clone();

        let segment = tokio::task::spawn_blocking(move || {
            crate::transcribe::transcribe(&rec, &chunk_owned)
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))??;

        transcriptions.push(segment);

        let progress = ((i + 1) as f32 / total_chunks as f32 * 100.0) as u32;
        let _ = app_handle.emit("file-transcribe-progress", serde_json::json!({
            "phase": "transcribing",
            "progress": progress,
            "chunk": i + 1,
            "totalChunks": total_chunks,
        }));
    }

    let text = file_transcribe::merge_transcriptions(transcriptions);
    let word_count = text.split_whitespace().count();

    let _ = app_handle.emit("file-transcribe-progress", serde_json::json!({"phase": "done", "progress": 100}));

    Ok(FileTranscriptionResult {
        text,
        duration_secs,
        word_count,
        chunks: total_chunks,
    })
}

#[tauri::command]
pub async fn restart_hotkey_listener(
    app_handle: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    hotkey::stop_key_listener();

    let s = state.lock().await;
    if s.settings.hotkey_mode == "dedicated_key" && s.settings.hotkey_keycode > 0 {
        let kc = s.settings.hotkey_keycode;
        let name = s.settings.hotkey_key_name.clone();
        let shared = state.inner().clone();
        drop(s);
        log::info!("Restarting key listener for '{name}' (keycode {kc})");
        hotkey::start_key_listener(app_handle, kc, shared);
    } else {
        let mut s_mut = state.lock().await;
        s_mut.hotkey_status = crate::state::HotkeyStatus::Idle;
        drop(s_mut);
        let _ = app_handle.emit("hotkey-status", "idle");
    }

    Ok(())
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

#[tauri::command]
pub async fn check_input_monitoring() -> Result<bool, String> {
    Ok(hotkey::preflight_listen_access())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedKey {
    pub keycode: i64,
    pub name: String,
}

#[tauri::command]
pub async fn capture_hotkey_key() -> Result<CapturedKey, String> {
    // Run capture on a blocking thread since it blocks until a key is pressed
    let (keycode, name) = tokio::task::spawn_blocking(hotkey::capture_next_key)
        .await
        .map_err(|e| format!("capture task failed: {e}"))?;
    Ok(CapturedKey { keycode, name })
}
