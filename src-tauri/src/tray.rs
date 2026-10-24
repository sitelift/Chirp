//! Tray icon: left click opens the surface, right click shows the native menu
//! (Restart to update when one is ready, Open, Pause / Resume, Quit). Also owns
//! the pause state, which the hotkey dispatcher checks.
//!
//! The icon is the existing brand tray icon. Paused and update-ready variants
//! are drawn on top of it in memory (a pause badge, an update dot); the brand
//! files are never modified.
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Wry,
};

const TRAY_ID: &str = "chirp";
static PAUSED: AtomicBool = AtomicBool::new(false);
static PAUSE_GENERATION: AtomicU64 = AtomicU64::new(0);
static PAUSED_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static UPDATE: Mutex<Option<String>> = Mutex::new(None);

pub fn is_paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseState {
    pub paused: bool,
    /// Unix ms when a timed pause ends; `None` for "until I resume".
    pub until: Option<u64>,
}

fn pause_state() -> PauseState {
    let until = PAUSED_UNTIL_MS.load(Ordering::SeqCst);
    PauseState { paused: is_paused(), until: (is_paused() && until > 0).then_some(until) }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Pause for `minutes`, or until resumed when `None`.
pub fn set_paused(app: &AppHandle, paused: bool, minutes: Option<u64>) {
    let generation = PAUSE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    PAUSED.store(paused, Ordering::SeqCst);
    let until = match (paused, minutes) {
        (true, Some(m)) => now_ms() + m * 60_000,
        _ => 0,
    };
    PAUSED_UNTIL_MS.store(until, Ordering::SeqCst);
    if let (true, Some(m)) = (paused, minutes) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(m * 60)).await;
            if PAUSE_GENERATION.load(Ordering::SeqCst) == generation {
                set_paused(&app, false, None);
            }
        });
    }
    refresh(app);
    let _ = app.emit("pause-changed", pause_state());
}

#[tauri::command]
pub fn get_pause_state() -> PauseState {
    pause_state()
}

#[tauri::command]
pub fn pause_hotkeys(app: AppHandle, paused: bool, minutes: Option<u64>) -> PauseState {
    set_paused(&app, paused, minutes);
    pause_state()
}

/// The frontend downloads updates quietly and reports when one is ready to install.
#[tauri::command]
pub fn set_update_ready(app: AppHandle, version: Option<String>) {
    *UPDATE.lock().unwrap_or_else(|e| e.into_inner()) = version;
    refresh(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn base_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-icon.png")).expect("tray icon").to_owned()
}

/// Paint a filled circle with an optional ring, in place, on an RGBA buffer.
fn disc(px: &mut [u8], w: usize, cx: f32, cy: f32, r: f32, ring: f32, fill: [u8; 4], ring_color: [u8; 4]) {
    let h = px.len() / 4 / w;
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let c = if d <= r { fill } else if d <= r + ring { ring_color } else { continue };
            px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
        }
    }
}

fn variant(paused: bool, update: bool) -> Image<'static> {
    let base = base_icon();
    if !paused && !update {
        return base;
    }
    let (w, h) = (base.width() as usize, base.height() as usize);
    let mut px = base.rgba().to_vec();
    let s = w as f32 / 32.0;
    if paused {
        // Dim the bird to grey, then a dark badge with two pause bars.
        for p in px.chunks_exact_mut(4) {
            let l = (p[0] as u32 * 3 + p[1] as u32 * 6 + p[2] as u32) / 10;
            let l = (l as f32 * 0.75) as u8;
            p[0] = l;
            p[1] = l;
            p[2] = l;
        }
        let (cx, cy, r) = (23.0 * s, 23.0 * s, 8.0 * s);
        disc(&mut px, w, cx, cy, r, 1.5 * s, [0x1A, 0x19, 0x17, 255], [255, 255, 255, 255]);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let in_bar = |left: f32| fx >= left * s && fx < (left + 2.5) * s && fy >= 19.0 * s && fy < 27.0 * s;
                if in_bar(19.75) || in_bar(23.75) {
                    px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    if update {
        disc(&mut px, w, 26.0 * s, 6.0 * s, 4.5 * s, 1.5 * s, [0xF0, 0xB7, 0x23, 255], [0x1A, 0x19, 0x17, 255]);
    }
    Image::new_owned(px, w as u32, h as u32)
}

fn amber_dot() -> Image<'static> {
    let w = 16usize;
    let mut px = vec![0u8; w * w * 4];
    disc(&mut px, w, 8.0, 8.0, 3.5, 0.0, [0xC9, 0x90, 0x0A, 255], [0, 0, 0, 0]);
    Image::new_owned(px, w as u32, w as u32)
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let update = UPDATE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let menu = Menu::new(app)?;
    if update.is_some() {
        menu.append(&IconMenuItem::with_id(app, "update", "Restart to update", true, Some(amber_dot()), None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(app, "open", "Open", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "pause", if is_paused() { "Resume" } else { "Pause" }, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;
    Ok(menu)
}

fn tooltip(paused: bool, update: bool) -> &'static str {
    match (paused, update) {
        (true, _) => "Chirp: paused",
        (false, true) => "Chirp: update ready",
        _ => "Chirp",
    }
}

/// Rebuild icon, tooltip and menu from the current pause / update state.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let update = UPDATE.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let paused = is_paused();
    let _ = tray.set_icon(Some(variant(paused, update)));
    let _ = tray.set_tooltip(Some(tooltip(paused, update)));
    if let Ok(m) = menu(app) {
        let _ = tray.set_menu(Some(m));
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(variant(false, false))
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .tooltip(tooltip(false, false))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => crate::surface::request_open(app, false),
            "pause" => set_paused(app, !is_paused(), None),
            "update" => {
                let _ = app.emit("update-install", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                crate::surface::request_open(tray.app_handle(), true);
            }
        })
        .build(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_keep_the_icon_size_and_only_draw_in_memory() {
        let base = base_icon();
        for (p, u) in [(true, false), (false, true), (true, true)] {
            let v = variant(p, u);
            assert_eq!((v.width(), v.height()), (base.width(), base.height()));
            assert_ne!(v.rgba(), base.rgba());
        }
    }
}
