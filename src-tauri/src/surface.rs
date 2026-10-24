//! The one window. Chirp has a single transparent window ("overlay") that is
//! always sized to its content (pill, resting tick or expanded surface) and
//! anchored bottom-center on a monitor's work area. The frontend reports the
//! size it needs and an interaction mode; this module places the window,
//! toggles click-through, and detects hover over the resting tick while the
//! window ignores the mouse (a 20 Hz `GetCursorPos` poll that only runs in
//! `Tick` mode and parks otherwise).
use serde::Deserialize;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY},
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowRect, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SW_SHOWNOACTIVATE},
};

pub const LABEL: &str = "overlay";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Window hidden.
    Hidden,
    /// Resting tick: click-through, hover detected by polling.
    Tick,
    /// Click-through, no polling (dictation pills).
    Passive,
    /// Receives the mouse but never takes focus (hands-free stop, action pills).
    Interactive,
    /// Receives mouse and keyboard focus (the opened surface).
    Focused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// Monitor of the foreground window (where the user is typing).
    Foreground,
    /// Monitor under the mouse (tray clicks).
    Cursor,
    Primary,
}

struct Geometry {
    monitor: HMONITOR,
    width: f64,
    height: f64,
    mode: Mode,
    /// Hit area for the tick hover, as logical insets from the window edges: top, right, bottom, left.
    inset: [f64; 4],
    hovering: bool,
    last_blur: Option<Instant>,
}
// HMONITOR is a plain handle value.
unsafe impl Send for Geometry {}

static GEOMETRY: Mutex<Option<Geometry>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();

fn hwnd(win: &WebviewWindow) -> Option<HWND> {
    win.hwnd().ok().map(|h| h.0 as HWND)
}

fn monitor_for(anchor: Anchor, own: Option<HWND>) -> HMONITOR {
    unsafe {
        match anchor {
            Anchor::Foreground => {
                let fg = GetForegroundWindow();
                if fg.is_null() || Some(fg) == own {
                    return monitor_for(Anchor::Cursor, own);
                }
                MonitorFromWindow(fg, MONITOR_DEFAULTTOPRIMARY)
            }
            Anchor::Cursor => {
                let mut p = POINT { x: 0, y: 0 };
                GetCursorPos(&mut p);
                MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST)
            }
            Anchor::Primary => MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
        }
    }
}

fn scale_for(monitor: HMONITOR) -> f64 {
    let (mut x, mut y) = (96u32, 96u32);
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
    x.max(96) as f64 / 96.0
}

/// Bottom-center of the monitor's work area (above the taskbar). The 12 px gap
/// is part of the window: the frontend pads its content.
fn place(win: &WebviewWindow, g: &Geometry) {
    let Some(h) = hwnd(win) else { return };
    unsafe {
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(g.monitor, &mut mi) == 0 {
            return;
        }
        let work = mi.rcWork;
        let scale = scale_for(g.monitor);
        let w = (g.width * scale).round() as i32;
        let hgt = (g.height * scale).round() as i32;
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.bottom - hgt;
        // Twice: if this move crosses into a monitor with another DPI, the first
        // call triggers WM_DPICHANGED (which resizes to a suggested rect); the
        // second call, now on the target monitor, restores the exact rect.
        for _ in 0..2 {
            SetWindowPos(h, HWND_TOPMOST, x, y, w, hgt, SWP_NOACTIVATE);
        }
    }
}

fn apply_mode(win: &WebviewWindow, mode: Mode, hovering: bool) {
    match mode {
        Mode::Hidden => {
            let _ = win.hide();
        }
        _ => {
            if !win.is_visible().unwrap_or(false) {
                if let Some(h) = hwnd(win) {
                    unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
                }
            }
            let ignore = matches!(mode, Mode::Tick | Mode::Passive) && !hovering;
            let _ = win.set_ignore_cursor_events(ignore);
            if mode == Mode::Focused {
                let _ = win.set_focus();
            }
        }
    }
}

/// Size the window to the content and set how it handles the mouse.
#[tauri::command]
pub fn surface_update(
    app: AppHandle,
    width: f64,
    height: f64,
    mode: Mode,
    anchor: Option<Anchor>,
    inset: Option<[f64; 4]>,
) -> Result<(), String> {
    let win = app.get_webview_window(LABEL).ok_or("no window")?;
    let own = hwnd(&win);
    let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
    let g = guard.get_or_insert_with(|| Geometry {
        monitor: monitor_for(Anchor::Primary, own),
        width,
        height,
        mode,
        inset: [0.0; 4],
        hovering: false,
        last_blur: None,
    });
    if let Some(anchor) = anchor {
        g.monitor = monitor_for(anchor, own);
    }
    g.width = width.clamp(1.0, 2000.0);
    g.height = height.clamp(1.0, 2000.0);
    if mode != Mode::Tick {
        g.hovering = false;
    }
    g.mode = mode;
    g.inset = inset.unwrap_or([0.0; 4]);
    if mode != Mode::Hidden {
        place(&win, g);
    }
    apply_mode(&win, mode, g.hovering);
    drop(guard);
    WAKE.notify_all();
    Ok(())
}

/// The tray icon and second-instance launches open the surface. A tray click
/// that lands right after the surface closed on blur (the click itself caused
/// the blur) is a "close", not a re-open.
pub fn request_open(app: &AppHandle, from_tray: bool) {
    if from_tray {
        let guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
        if guard.as_ref().and_then(|g| g.last_blur).is_some_and(|t| t.elapsed() < Duration::from_millis(400)) {
            return;
        }
    }
    let _ = app.emit("surface-open", if from_tray { "cursor" } else { "primary" });
}

/// Window focus lost while the surface was open: the frontend closes it.
pub fn on_blur(app: &AppHandle) {
    let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(g) = guard.as_mut() {
        if g.mode == Mode::Focused {
            g.last_blur = Some(Instant::now());
            drop(guard);
            let _ = app.emit("surface-blur", ());
        }
    }
}

/// Hover detection for the click-through resting tick.
pub fn start_hover_poll(app: AppHandle) {
    let _ = std::thread::Builder::new().name("chirp-hover".into()).spawn(move || loop {
        let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
        while guard.as_ref().map_or(true, |g| g.mode != Mode::Tick) {
            guard = WAKE.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
        drop(guard);
        std::thread::sleep(Duration::from_millis(50));
        let Some(win) = app.get_webview_window(LABEL) else { continue };
        let Some(h) = hwnd(&win) else { continue };
        let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
        let Some(g) = guard.as_mut() else { continue };
        if g.mode != Mode::Tick {
            continue;
        }
        let inside = unsafe {
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let mut p = POINT { x: 0, y: 0 };
            GetWindowRect(h, &mut r);
            GetCursorPos(&mut p);
            let s = scale_for(g.monitor);
            let [t, rt, b, l] = g.inset.map(|v| (v * s) as i32);
            // A little slack once hovering so the pill does not flicker at its edge.
            let slack = if g.hovering { (6.0 * s) as i32 } else { 0 };
            let inside = p.x >= r.left + l - slack
                && p.x < r.right - rt + slack
                && p.y >= r.top + t - slack
                && p.y < r.bottom - b + slack;
            inside
        };
        if inside != g.hovering {
            g.hovering = inside;
            drop(guard);
            let _ = win.set_ignore_cursor_events(!inside);
            let _ = app.emit("surface-hover", inside);
        }
    });
}
