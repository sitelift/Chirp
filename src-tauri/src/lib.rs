mod audio;
mod cleanup;
mod commands;
mod dictionary;
mod inject;
mod settings;
mod state;
mod transcribe;

use commands::StreamHandle;
use state::{AppState, AudioBuffer, SharedState};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Read settings early so we can configure the shortcut before building
    let initial_settings = settings::load_settings();
    let initial_dictionary = settings::load_dictionary();
    let hotkey_str = initial_settings.hotkey.clone();

    // Parse the hotkey for the global shortcut plugin
    // The parser natively understands CmdOrCtrl, Ctrl, Shift, Alt, etc.
    let shortcut: tauri_plugin_global_shortcut::Shortcut = hotkey_str
        .parse()
        .unwrap_or_else(|_| "CmdOrCtrl+Shift+Space".parse().unwrap());

    log::info!("Registering global hotkey: {hotkey_str}");

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(shortcut)
                .expect("Failed to configure shortcut")
                .with_handler(|app, _shortcut, event| {
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
                })
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage::<SharedState>({
            Arc::new(tokio::sync::Mutex::new(AppState::new(
                initial_settings,
                initial_dictionary,
            )))
        })
        .manage::<AudioBuffer>(Arc::new(std::sync::Mutex::new(Vec::new())))
        .manage(StreamHandle(std::sync::Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::update_dictionary,
            commands::get_audio_devices,
            commands::get_input_level,
            commands::start_recording,
            commands::stop_recording,
            commands::cancel_recording,
            commands::download_model,
            commands::get_model_status,
            commands::check_for_updates,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Sync autostart with the launch_at_login setting
            {
                let autostart = app.autolaunch();
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                if s.settings.launch_at_login {
                    let _ = autostart.enable();
                } else {
                    let _ = autostart.disable();
                }
            }

            // Load speech model if available (all sync operations, no async needed)
            {
                let state = handle.state::<SharedState>();
                let mut s = state.blocking_lock();
                let model = s.settings.model.clone();
                if transcribe::model_exists(&model) {
                    match transcribe::load_model(&model) {
                        Ok(recognizer) => {
                            s.recognizer = Some(recognizer);
                            log::info!("Speech model '{model}' loaded");
                        }
                        Err(e) => log::error!("Failed to load speech model: {e}"),
                    }
                }

                // Load cleanup model if available
                if cleanup::cleanup_model_exists() {
                    match (cleanup::load_encoder(), cleanup::load_decoder()) {
                        (Ok(enc), Ok(dec)) => {
                            s.cleanup_encoder = Some(enc);
                            s.cleanup_decoder = Some(dec);
                            log::info!("Cleanup model loaded");
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            log::warn!("Cleanup model not loaded: {e}");
                        }
                    }
                }
            }

            // Build system tray
            let version = env!("CARGO_PKG_VERSION");
            let version_item =
                MenuItem::new(app, &format!("Chirp v{version}"), false, None::<&str>)?;
            let toggle_item =
                MenuItem::with_id(app, "toggle", "Start Listening", true, None::<&str>)?;
            let settings_item =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let updates_item =
                MenuItem::with_id(app, "updates", "Check for Updates", true, None::<&str>)?;
            let quit_item =
                MenuItem::with_id(app, "quit", "Quit Chirp", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &version_item,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &toggle_item,
                    &settings_item,
                    &updates_item,
                    &tauri::menu::PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;

            TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Chirp — Voice to Text")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "settings" => {
                        if let Some(win) = app.get_webview_window("settings") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    "toggle" => {
                        // Tray toggle acts as press/release toggle
                        let _ = app.emit("toggle-recording", ());
                    }
                    _ => {}
                })
                .build(app)?;

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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
