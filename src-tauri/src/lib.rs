pub mod audio;
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
pub mod llm;
pub mod router;
pub mod speculative;
mod settings;
mod snippets;
mod state;
pub mod transcribe;


use commands::{RecordingStartTime, ResamplerFlushState, StreamActiveState, StreamErrorState, StreamHandle};
use state::{AppState, AudioBuffer, SharedState, VadCleanedTranscripts, VadFlushHandle, VadReceiverHandle, VadSender, VadTranscripts};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    Emitter, Manager, WindowEvent,
};
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
            // Focus existing settings window when second instance tries to launch
            if let Some(win) = app.get_webview_window("settings") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
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
        .manage(crate::llm::LlmClient::from_env())
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
            commands::play_completion_sound,
            commands::get_hotkey_status,
            commands::record_binding,
            commands::cancel_record_binding,
            commands::show_settings,
            commands::quit_app,
        ])
        .setup(|app| {
            crate::context::prewarm();
            let handle = app.handle().clone();

            // Sync autostart with the launch_at_login setting
            {
                let autostart = app.autolaunch();
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
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

            // Build system tray — left-click opens custom popup, right-click shows minimal menu
            let version = env!("CARGO_PKG_VERSION");
            let version_item =
                MenuItem::new(app, &format!("Chirp v{version}"), false, None::<&str>)?;
            let quit_item =
                MenuItem::with_id(app, "quit", "Quit Chirp", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &version_item,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
            let tray_icon = tauri::image::Image::from_bytes(tray_icon_bytes)
                .expect("Failed to load tray icon");

            TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&menu)
                .menu_on_left_click(false)
                .tooltip("Chirp — Voice to Text")
                .on_menu_event(move |app, event| {
                    if event.id().as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("tray-popup") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                // Get the scale factor so we can normalize to logical coords.
                                // Tray rect may arrive as physical pixels on Windows with DPI scaling.
                                let scale = win.scale_factor().unwrap_or(1.0);

                                let icon_x = match rect.position {
                                    tauri::Position::Physical(p) => p.x as f64 / scale,
                                    tauri::Position::Logical(p) => p.x,
                                };
                                let icon_y = match rect.position {
                                    tauri::Position::Physical(p) => p.y as f64 / scale,
                                    tauri::Position::Logical(p) => p.y,
                                };
                                let icon_w = match rect.size {
                                    tauri::Size::Physical(s) => s.width as f64 / scale,
                                    tauri::Size::Logical(s) => s.width,
                                };

                                let popup_w = 320.0_f64;
                                let popup_h = 440.0_f64;
                                let gap = 8.0_f64;

                                // Center popup horizontally on the tray icon
                                let x = icon_x + icon_w / 2.0 - popup_w / 2.0;
                                // Above the taskbar
                                let y = icon_y - popup_h - gap;

                                let x = if x < 0.0 { 0.0 } else { x };
                                let y = if y < 0.0 { 0.0 } else { y };

                                let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                                let _ = win.show();
                                let _ = win.set_focus();
                                let _ = app.emit("tray-popup-shown", ());
                            }
                        }
                    }
                })
                .build(app)?;

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

            // Show settings window unless launched with --minimized (autostart)
            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                if let Some(win) = app.get_webview_window("settings") {
                    let _ = win.show();
                    let _ = win.maximize();
                    let _ = win.set_focus();
                }
            }

            // Prevent settings window from fully closing — hide to tray instead
            if let Some(settings_win) = app.get_webview_window("settings") {
                let handle_for_close = handle.clone();
                settings_win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(win) = handle_for_close.get_webview_window("settings") {
                            let _ = win.hide();
                        }
                    }
                });
            }

            // Hide tray popup when it loses focus (click outside)
            if let Some(popup_win) = app.get_webview_window("tray-popup") {
                let handle_for_popup = handle.clone();
                popup_win.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        if let Some(win) = handle_for_popup.get_webview_window("tray-popup") {
                            let _ = win.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                hotkey::stop();
                log::info!("App exiting — cleaned up hotkey");
            }
        });
}
