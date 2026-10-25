pub mod ai;
pub mod audio;
mod capture;
pub mod performance;
pub mod cleanup;
mod clipboard_win;
mod commands;
mod context;
mod history;
mod hotkey;
mod hotkey_input;
mod hotkey_migration;
mod inject;
mod levels;
mod media;
pub mod llm;
pub mod router;
pub mod speculative;
mod settings;
mod snippets;
mod sound;
mod state;
mod surface;
mod tray;
pub mod transcribe;
pub mod engine;
mod downloads;
#[cfg(windows)]
mod window_icon;


use commands::{RecordingStartTime, ResamplerFlushState, StreamActiveState, StreamErrorState, StreamHandle};
use state::{AppState, AudioBuffer, SharedState, VadCleanedTranscripts, VadFlushHandle, VadReceiverHandle, VadSender, VadTranscripts};
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Read settings early so we can configure the shortcut before building
    let initial_settings = settings::load_settings();

    let initial_vocabulary = settings::load_vocabulary();
    let initial_snippets = settings::load_snippets();
    let mut initial_history = history::load_history();
    history::prune_history(&mut initial_history, initial_settings.history_retention_days);
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch opens the surface of the running instance.
            crate::surface::request_open(app, false);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage::<SharedState>({
            Arc::new(tokio::sync::Mutex::new(AppState::new(
                initial_settings,
                initial_vocabulary,
                initial_snippets,
                initial_history,
            )))
        })
        .manage::<AudioBuffer>(Arc::new(std::sync::Mutex::new(Vec::new())))
        .manage(crate::llm::LlmClient::new(None))
        .manage(crate::ai::AiState::default())
        .manage(commands::RecordingContext::default())
        .manage(crate::speculative::RecordingSpeculation::default())
        .manage(StreamHandle(std::sync::Mutex::new(None)))
        .manage(StreamErrorState(std::sync::Mutex::new(None)))
        .manage(ResamplerFlushState(std::sync::Mutex::new(None)))
        .manage(RecordingStartTime(std::sync::Mutex::new(None)))
        .manage(StreamActiveState(std::sync::Mutex::new(None)))
        .manage::<VadTranscripts>(Arc::new(std::sync::Mutex::new(Vec::new())))
        .manage(VadCleanedTranscripts(Arc::new(std::sync::Mutex::new(Vec::new()))))
        .manage(VadReceiverHandle(std::sync::Mutex::new(None)))
        .manage(VadSender(std::sync::Mutex::new(None)))
        .manage(VadFlushHandle(std::sync::Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            engine::get_engine_status,
            engine::retry_gpu,
            downloads::get_download_status,
            commands::get_settings,
            commands::update_settings,
            commands::get_vocabulary,
            commands::update_vocabulary,
            commands::get_audio_devices,
            commands::get_input_level,
            commands::start_recording,
            commands::stop_recording,
            commands::cancel_recording,
            commands::download_model,
            commands::get_model_status,
            commands::get_history,
            commands::clear_history,
            commands::delete_history_entry,
            commands::test_microphone,
            commands::get_snippets,
            commands::update_snippets,
            commands::play_sound,
            commands::get_hotkey_status,
            commands::record_binding,
            commands::cancel_record_binding,
            commands::copy_text,
            commands::probe_microphone,
            commands::open_mic_privacy_settings,
            ai::get_ai_status,
            ai::set_api_key,
            ai::remove_api_key,
            surface::surface_update,
            tray::get_pause_state,
            tray::pause_hotkeys,
            tray::set_update_ready,
            tray::quit_app,
            levels::start_level_monitor,
            levels::stop_level_monitor,
        ])
        .setup(|app| {
            crate::context::prewarm();
            let handle = app.handle().clone();

            #[cfg(windows)]
            for window in app.webview_windows().values() {
                if let Err(error) = window_icon::set_native_icons(window) {
                    log::warn!("Failed to set native window icons: {error}");
                }
            }

            // Sync autostart with the launch_at_login setting
            {
                let autostart = app.autolaunch();
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                crate::capture::maintain_at_startup(&s.settings);
                if s.settings.launch_at_login {
                    if let Err(e) = autostart.enable() {
                        log::warn!("Failed to enable autostart: {e}");
                    }
                } else {
                    if let Err(e) = autostart.disable() {
                        log::warn!("Failed to disable autostart: {e}");
                    }
                }
            }

            commands::warm_microphone(&handle);

            // Load speech model if available (all sync operations, no async needed)
            {
                let state = handle.state::<SharedState>();
                let mut s = state.blocking_lock();
                let model = s.settings.model.clone();
                if transcribe::model_exists(&model) {
                    let vocab = s.vocabulary.clone();
                    match transcribe::load_model(&model, s.settings.beam_search, &vocab) {
                        Ok(recognizer) => {
                            s.recognizer = Some(Arc::new(recognizer));
                            log::info!("Speech model '{model}' loaded");
                        }
                        Err(e) => log::error!("Failed to load speech model: {e}"),
                    }
                }

            }

            crate::downloads::background(handle.clone());
            crate::tray::build(&handle)?;
            crate::ai::init(&handle);
            crate::media::init();

            // Start the global hotkey listener
            {
                let hotkey_handle = handle.clone();
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let hotkey_str = s.settings.clone();
                drop(s);
                if let Err(e) = hotkey::start(&hotkey_str, hotkey_handle) {
                    log::error!("Failed to start hotkey listener: {e}");
                }
            }

            // Shown once, click-through and empty; the frontend sizes and places it.
            if let Some(win) = app.get_webview_window(crate::surface::LABEL) {
                crate::surface::init(&win);
                let handle_for_blur = handle.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        crate::surface::on_blur(&handle_for_blur);
                    }
                });
            }
            crate::surface::start_hover_poll(handle.clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                hotkey::stop();
                crate::media::shutdown();
                log::info!("App exiting — cleaned up hotkey");
            }
        });
}
