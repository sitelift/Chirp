mod announcements;
mod audio;
mod cleanup;
mod commands;
mod feedback;
mod history;
#[cfg(windows)]
mod clipboard_win;
mod inject;
mod llm;
mod hotkey;
#[cfg(target_os = "windows")]
mod hotkey_raw_input;
#[cfg(target_os = "windows")]
mod hotkey_ll_suppress;
#[cfg(target_os = "macos")]
mod native_hotkey;
mod settings;
mod snippets;
mod state;
mod transcribe;


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
    // Ensure a Tokio runtime context exists before plugin setup.
    // tauri-plugin-aptabase calls tokio::spawn() during plugin setup, which
    // panics if no runtime is present. Tauri creates its own runtime later
    // (during .run()), but plugin .setup() hooks run during .build() which
    // is too early. This runtime lives for the duration of run().
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _rt_guard = rt.enter();

    // Read settings early so we can configure the shortcut before building
    let initial_settings = settings::load_settings();

    // Initialize Sentry crash reporting (only when user has opted in).
    // The guard must live for the entire run() scope — dropping it disables Sentry.
    let _sentry_guard = if initial_settings.help_improve {
        let client = sentry::init(sentry::ClientOptions {
            dsn: "https://25fcb0687861c03dfc6a02254aa057a8@o4511102179278848.ingest.us.sentry.io/4511102182293504".parse().ok(),
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
                        "vocabulary",
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
    let initial_vocabulary = settings::load_vocabulary();
    let initial_snippets = settings::load_snippets();
    let mut initial_history = history::load_history();
    history::prune_history(&mut initial_history, initial_settings.history_retention_days);
    let mut builder = tauri::Builder::default()
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
        .plugin(tauri_plugin_dialog::init());

    // Register Sentry plugin (auto-injects @sentry/browser into webviews)
    if let Some(ref guard) = _sentry_guard {
        builder = builder.plugin(tauri_plugin_sentry::init(guard));
    }

    // Aptabase telemetry — empty key disables all tracking (no-op).
    // Key is hardcoded; it's write-only (can only send events, not read data).
    let aptabase_key = if initial_settings.help_improve {
        "A-US-1585552886"
    } else {
        ""
    };

    builder.plugin(tauri_plugin_aptabase::Builder::new(aptabase_key).build())
        .manage::<SharedState>({
            Arc::new(tokio::sync::Mutex::new(AppState::new(
                initial_settings,
                initial_vocabulary,
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
            commands::get_announcements,
            commands::dismiss_announcement,
            commands::send_feedback,
            commands::request_mic_permission,
            commands::check_accessibility_permission,
            commands::request_accessibility_permission,
            commands::capture_next_key,
            commands::show_settings,
            commands::quit_app,
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

            // Auto-start cleanup server if AI cleanup is enabled and files exist
            {
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let ai_cleanup = s.settings.ai_cleanup;
                drop(s);

                let should_start = ai_cleanup && llm::binary_exists() && llm::model_exists();

                if should_start {
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

                        let result = llm::start_server(port).await;

                        match result {
                            Ok(child) => {
                                let mut s = state_clone.lock().await;
                                if let Some(pid) = child.id() {
                                    llm::save_server_pid(pid);
                                }
                                s.llm_process = Some(child);
                                s.llm_port = Some(port);
                                log::info!("Cleanup model server auto-started on port {port}");
                            }
                            Err(e) => {
                                log::warn!("Failed to auto-start cleanup model server: {e}");
                            }
                        }
                    });
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
                                let y = if cfg!(target_os = "macos") {
                                    icon_y + gap
                                } else {
                                    // Windows: above the taskbar
                                    icon_y - popup_h - gap
                                };

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

            // macOS: make overlay float above fullscreen apps
            #[cfg(target_os = "macos")]
            {
                if let Some(overlay) = app.get_webview_window("overlay") {
                    use cocoa::appkit::NSWindow;
                    use cocoa::appkit::NSWindowCollectionBehavior;
                    let ns_win = overlay.ns_window().unwrap() as cocoa::base::id;
                    unsafe {
                        // Level 1000 = NSScreenSaverWindowLevel, above fullscreen spaces
                        ns_win.setLevel_(1000);
                        ns_win.setCollectionBehavior_(
                            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                        );
                    }
                }
            }

            // macOS: make tray popup float above fullscreen apps
            #[cfg(target_os = "macos")]
            {
                if let Some(popup) = app.get_webview_window("tray-popup") {
                    use cocoa::appkit::NSWindow;
                    use cocoa::appkit::NSWindowCollectionBehavior;
                    let ns_win = popup.ns_window().unwrap() as cocoa::base::id;
                    unsafe {
                        ns_win.setLevel_(1000);
                        ns_win.setCollectionBehavior_(
                            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                        );
                    }
                }
            }

            // macOS: enable native stoplight buttons on settings window
            #[cfg(target_os = "macos")]
            {
                if let Some(settings_win) = app.get_webview_window("settings") {
                    use cocoa::appkit::{NSWindow, NSWindowStyleMask, NSWindowTitleVisibility};
                    let ns_win = settings_win.ns_window().unwrap() as cocoa::base::id;
                    unsafe {
                        let mut mask = ns_win.styleMask();
                        mask |= NSWindowStyleMask::NSTitledWindowMask
                            | NSWindowStyleMask::NSClosableWindowMask
                            | NSWindowStyleMask::NSMiniaturizableWindowMask
                            | NSWindowStyleMask::NSResizableWindowMask
                            | NSWindowStyleMask::NSFullSizeContentViewWindowMask;
                        ns_win.setStyleMask_(mask);
                        ns_win.setTitlebarAppearsTransparent_(cocoa::base::YES);
                        ns_win.setTitleVisibility_(NSWindowTitleVisibility::NSWindowTitleHidden);
                    }
                }
            }

            // Start the global hotkey listener
            {
                let hotkey_handle = handle.clone();
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let hotkey_str = s.settings.hotkey.clone();
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
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    hotkey::stop();
                    llm::kill_stale_server();
                    llm::clear_server_pid();
                    log::info!("App exiting — cleaned up hotkey and LLM process");
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { has_visible_windows, .. } => {
                    // Dock icon clicked — restore the settings window
                    if !has_visible_windows {
                        if let Some(win) = app_handle.get_webview_window("settings") {
                            let _ = win.unminimize();
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                }
                _ => {}
            }
        });
}
