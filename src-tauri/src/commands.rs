use crate::audio;
use crate::cleanup;
use crate::dictionary;
use crate::inject;
use crate::settings;
use crate::state::*;
use crate::transcribe;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

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

    // Sync autostart with launch_at_login setting
    let autostart = app_handle.autolaunch();
    if s.settings.launch_at_login {
        let _ = autostart.enable();
    } else {
        let _ = autostart.disable();
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
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    Ok(audio::list_devices())
}

#[tauri::command]
pub async fn get_input_level(buffer: State<'_, AudioBuffer>) -> Result<f32, String> {
    let buf = buffer.lock().unwrap();
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
    buffer.lock().unwrap().clear();

    let device_id = s.settings.input_device.clone();
    s.recording_state = RecordingState::Recording;
    drop(s);

    // Start audio capture
    let stream =
        audio::start_capture(&device_id, buffer.inner().clone(), app_handle.clone()).map_err(
            |e| {
                if e.contains("No default input") || e.contains("Device not found") {
                    "mic_not_found".to_string()
                } else {
                    "mic_permission".to_string()
                }
            },
        )?;

    // Store stream handle
    *stream_handle.0.lock().unwrap() = Some(StreamWrapper(stream));

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
        let mut handle = stream_handle.0.lock().unwrap();
        *handle = None; // Drop stream → stops capture
    }

    {
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Processing;
    }

    let _ = app_handle.emit("recording-state", "processing");

    // Get the audio data, prepending 150ms of silence so the resampler's
    // internal delay and model warm-up don't eat the first word.
    let audio_data = {
        let buf = buffer.lock().unwrap();
        let pad_samples = 16000 * 150 / 1000; // 150ms at 16kHz
        let mut padded = vec![0.0f32; pad_samples];
        padded.extend_from_slice(&buf);
        padded
    };

    let sample_count = audio_data.len();
    let duration_secs = sample_count as f32 / 16000.0;
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

    // Grab what we need from state before entering blocking thread
    let (language, smart_fmt, dict) = {
        let s = state.lock().await;
        (
            s.settings.language.clone(),
            s.settings.smart_formatting,
            s.dictionary.clone(),
        )
    };

    // Transcribe using the sherpa-onnx recognizer.
    // OfflineRecognizer is Send+Sync so we can use it from spawn_blocking safely.
    let state_inner = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        let s = rt.block_on(state_inner.lock());

        let recognizer = s.recognizer.as_ref().ok_or("model_not_loaded".to_string())?;

        log::info!("Starting Parakeet TDT transcription...");
        let raw = transcribe::transcribe(recognizer, &audio_data, &language)
            .map_err(|e| {
                log::error!("Transcription error: {e}");
                "transcription_failed".to_string()
            })?;

        log::info!("Transcription raw output: '{raw}'");

        if raw.is_empty() {
            log::warn!("Transcription returned empty text");
            return Err("transcription_failed".to_string());
        }

        // Cleanup/formatting
        let formatted = cleanup::cleanup_text(
            &raw,
            smart_fmt,
            s.cleanup_encoder.as_ref(),
            s.cleanup_decoder.as_ref(),
        );

        // Dictionary replacements
        let final_text = dictionary::apply_dictionary(&formatted, &dict);

        Ok(final_text)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?;

    // If transcription failed, reset state before returning error
    let result = match result {
        Ok(text) => text,
        Err(e) => {
            let mut s = state.lock().await;
            s.recording_state = RecordingState::Idle;
            return Err(e);
        }
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let word_count = result.split_whitespace().count();

    // Inject text at cursor
    let text_for_inject = result.clone();
    let inject_result =
        tokio::task::spawn_blocking(move || inject::inject_text(&text_for_inject))
            .await
            .map_err(|e| format!("Task failed: {e}"))?;

    if let Err(e) = inject_result {
        let mut s = state.lock().await;
        s.recording_state = RecordingState::Idle;
        log::error!("Injection failed: {e}");
        return Err("injection_failed".into());
    }

    // Reset state
    let mut s = state.lock().await;
    s.recording_state = RecordingState::Idle;

    Ok(TranscriptionResult {
        text: result,
        word_count,
        duration_ms,
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
        let mut handle = stream_handle.0.lock().unwrap();
        *handle = None;
    }

    // Clear buffer
    buffer.lock().unwrap().clear();

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
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    Ok(UpdateInfo {
        available: false,
        version: None,
        url: None,
    })
}
