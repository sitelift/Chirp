mod audio;
mod cleanup;
mod commands;
mod dictionary;
mod history;
mod inject;
mod llm;
mod settings;
mod snippets;
mod state;
mod transcribe;

use commands::{RecordingStartTime, ResamplerFlushState, StreamActiveState, StreamErrorState, StreamHandle};
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

    // Initialize Sentry crash reporting (only when user has opted in).
    // The guard must live for the entire run() scope — dropping it disables Sentry.
    let _sentry_guard = if initial_settings.help_improve {
        let client = sentry::init(sentry::ClientOptions {
            dsn: Some(
                "https://examplePublicKey@o0.ingest.sentry.io/0"
                    .parse()
                    .unwrap(),
            ),
            release: Some(std::borrow::Cow::Borrowed(env!("CARGO_PKG_VERSION"))),
            before_breadcrumb: Some(Arc::new(|breadcrumb| {
                // Drop breadcrumbs that may contain transcription text
                if let Some(msg) = &breadcrumb.message {
                    let skip = [
                        "After regex",
                        "Parakeet chunk",
                        "LLM cleanup:",
                        "After AI cleanup",
                        "clipboard",
                        "dictionary",
                    ];
                    if skip.iter().any(|p| msg.contains(p)) {
                        return None;
                    }
                }
                Some(breadcrumb)
            })),
            before_send: Some(Arc::new(|mut event| {
                // Scrub exception values that could contain user text
                for exc_val in event.exception.values.iter_mut() {
                    if let Some(ref v) = exc_val.value {
                        if v.len() > 200 {
                            exc_val.value = Some("[scrubbed — value too long]".into());
                        }
                    }
                }
                Some(event)
            })),
            ..Default::default()
        });
        Some(client)
    } else {
        None
    };
    let initial_dictionary = settings::load_dictionary();
    let initial_snippets = settings::load_snippets();
    let mut initial_history = history::load_history();
    history::prune_history(&mut initial_history, initial_settings.history_retention_days);
    let hotkey_str = initial_settings.hotkey.clone();

    // Build the global shortcut plugin with the configured hotkey
    let shortcut_plugin = {
        let shortcut: tauri_plugin_global_shortcut::Shortcut = hotkey_str
            .parse()
            .unwrap_or_else(|_| {
                log::warn!("Failed to parse hotkey '{hotkey_str}', falling back to CmdOrCtrl+Shift+Space");
                "CmdOrCtrl+Shift+Space".parse().unwrap()
            });
        log::info!("Registering global hotkey: {hotkey_str}");
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
            .build()
    };

    let mut builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(shortcut_plugin)
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init());

    // Register Sentry plugin (auto-injects @sentry/browser into webviews)
    if let Some(ref guard) = _sentry_guard {
        builder = builder.plugin(tauri_plugin_sentry::init(guard));
    }

    builder.plugin({
            // Only send telemetry when the user has opted in.
            // An empty app key disables the Aptabase client entirely (all track_event calls become no-ops).
            let app_key = if initial_settings.help_improve {
                "A-US-6633912873"
            } else {
                ""
            };
            tauri_plugin_aptabase::Builder::new(app_key).build()
        })
        .manage::<SharedState>({
            Arc::new(tokio::sync::Mutex::new(AppState::new(
                initial_settings,
                initial_dictionary,
                initial_snippets,
                initial_history,
            )))
        })
        .manage::<AudioBuffer>(Arc::new(std::sync::Mutex::new(Vec::new())))
        .manage(StreamHandle(std::sync::Mutex::new(None)))
        .manage(StreamErrorState(std::sync::Mutex::new(None)))
        .manage(ResamplerFlushState(std::sync::Mutex::new(None)))
        .manage(RecordingStartTime(std::sync::Mutex::new(None)))
        .manage(StreamActiveState(std::sync::Mutex::new(None)))
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
            commands::get_history,
            commands::clear_history,
            commands::delete_history_entry,
            commands::get_llm_status,
            commands::download_llm,
            commands::start_llm,
            commands::stop_llm,
            commands::test_llm_cleanup,
            commands::test_microphone,
            commands::get_snippets,
            commands::update_snippets,
            commands::play_completion_sound,
            commands::get_hotkey_status,
        ])
        .setup(|app| {
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

            // Load speech model if available (all sync operations, no async needed)
            {
                let state = handle.state::<SharedState>();
                let mut s = state.blocking_lock();
                let model = s.settings.model.clone();
                if transcribe::model_exists(&model) {
                    match transcribe::load_model(&model) {
                        Ok(recognizer) => {
                            s.recognizer = Some(Arc::new(recognizer));
                            log::info!("Speech model '{model}' loaded");
                        }
                        Err(e) => log::error!("Failed to load speech model: {e}"),
                    }

                }

            }

            // Track app_started event (only fires if help_improve is on)
            {
                use tauri_plugin_aptabase::EventTracker;
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let model_loaded = s.recognizer.is_some();
                drop(s);
                let _ = app.track_event("app_started", Some(serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "model_loaded": model_loaded,
                })));
            }

            // Kill any stale llama-server from a previous crash (C2 fix)
            llm::kill_stale_server();

            // Auto-start LLM server if AI cleanup is enabled and files exist
            {
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let ai_cleanup = s.settings.ai_cleanup;
                drop(s);

                if ai_cleanup && llm::binary_exists() && llm::model_exists() {
                    let state_clone = handle.state::<SharedState>().inner().clone();
                    tauri::async_runtime::spawn(async move {
                        let port = match std::net::TcpListener::bind("127.0.0.1:0") {
                            Ok(listener) => match listener.local_addr() {
                                Ok(addr) => addr.port(),
                                Err(e) => {
                                    log::warn!("Failed to get local address for LLM: {e}");
                                    return;
                                }
                            },
                            Err(e) => {
                                log::warn!("Failed to find free port for LLM: {e}");
                                return;
                            }
                        };

                        match llm::start_server(port).await {
                            Ok(child) => {
                                let mut s = state_clone.lock().await;
                                if let Some(pid) = child.id() {
                                    llm::save_server_pid(pid);
                                }
                                s.llm_process = Some(child);
                                s.llm_port = Some(port);
                                log::info!("LLM server auto-started on port {port}");
                            }
                            Err(e) => {
                                log::warn!("Failed to auto-start LLM server: {e}");
                            }
                        }
                    });
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

            let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
            let tray_icon = tauri::image::Image::from_bytes(tray_icon_bytes)
                .expect("Failed to load tray icon");

            TrayIconBuilder::new()
                .icon(tray_icon)
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
                    "updates" => {
                        if let Some(win) = app.get_webview_window("settings") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                        let _ = app.emit("check-for-updates", ());
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Kill llama-server on app exit to prevent orphan processes (C1 fix)
                llm::kill_stale_server();
                llm::clear_server_pid();
                log::info!("App exiting — cleaned up LLM process");
                let _ = app_handle; // suppress unused warning
            }
        });
}
