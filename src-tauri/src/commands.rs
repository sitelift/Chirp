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

/// Request microphone permission on macOS via AVCaptureDevice.
/// Returns "granted", "denied", or "undetermined".
#[tauri::command]
pub async fn request_mic_permission() -> String {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::c_long;

        extern "C" {
            fn AVCaptureDevice_authorizationStatusForAudio() -> c_long;
            fn AVCaptureDevice_requestAccessForAudio();
        }

        let status = unsafe { AVCaptureDevice_authorizationStatusForAudio() };
        match status {
            3 => "granted".to_string(),
            0 => {
                // Not determined — trigger the prompt
                unsafe { AVCaptureDevice_requestAccessForAudio() };
                // Give the system a moment, then re-check
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let new_status = unsafe { AVCaptureDevice_authorizationStatusForAudio() };
                if new_status == 3 { "granted".to_string() } else { "undetermined".to_string() }
            }
            _ => "denied".to_string(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        "granted".to_string()
    }
}

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
        if s.settings.launch_at_login {
            let _ = autostart.enable();
        } else {
            let _ = autostart.disable();
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

    // Start audio capture — convert Result to Option immediately so the
    // non-Send cpal::Stream doesn't live across an await point.
    let capture_err = match audio::start_capture(
        &device_id,
        buffer.inner().clone(),
        app_handle.clone(),
    ) {
        Ok(stream) => {
            *stream_handle.0.lock().unwrap() = Some(StreamWrapper(stream));
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
        let mut handle = stream_handle.0.lock().unwrap();
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
        let mut buf = buffer.lock().unwrap();
        let pad_samples = 16000 * 150 / 1000; // 150ms at 16kHz
        let mut padded = vec![0.0f32; pad_samples];
        padded.extend_from_slice(&buf);
        buf.clear();
        padded
    };

    let sample_count = audio_data.len();
    let duration_secs = sample_count as f32 / 16000.0;

    // Audio fingerprint: log RMS, min/max, and a few sample values so we can
    // verify the buffer actually contains NEW audio on each recording.
    let rms = (audio_data.iter().map(|s| s * s).sum::<f32>() / sample_count as f32).sqrt();
    let min_val = audio_data.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = audio_data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    // Grab a few samples from the middle of the (non-padding) audio
    let mid = sample_count / 2;
    let samples_snapshot: Vec<f32> = audio_data[mid..mid.min(sample_count).max(mid) + 5.min(sample_count - mid)]
        .to_vec();
    log::info!(
        "Audio buffer: {sample_count} samples ({duration_secs:.1}s), RMS={rms:.6}, min={min_val:.6}, max={max_val:.6}, mid_samples={samples_snapshot:?}"
    );

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
        let mut s = rt.block_on(state_inner.lock());

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

        // Cleanup/formatting — take sessions out temporarily to satisfy borrow checker
        let mut enc = s.cleanup_encoder.take();
        let mut dec = s.cleanup_decoder.take();
        let tok = &s.cleanup_tokenizer;
        let formatted = cleanup::cleanup_text(
            &raw,
            smart_fmt,
            enc.as_mut(),
            dec.as_mut(),
            tok.as_ref(),
        );
        s.cleanup_encoder = enc;
        s.cleanup_decoder = dec;

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
    log::info!("Injecting transcribed text: '{result}'");
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
pub async fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
pub async fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| format!("Failed to open settings: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    Ok(UpdateInfo {
        available: false,
        version: None,
        url: None,
    })
}
