//! The one window. Chirp has a single transparent window ("overlay") at a
//! fixed envelope size, anchored bottom-center on a monitor's work area. The
//! frontend reports a size, an interaction mode and a hit rect (the one
//! interactive element: hover pill, pill or panel); this module places the
//! window, and makes it click-through whenever the cursor is outside the hit
//! rect (a 60 Hz `GetCursorPos` poll that runs in `Tick`, `Interactive` and
//! `Focused` and parks otherwise).
//!
//! The window is layered for its whole life: adding or removing
//! `WS_EX_LAYERED` makes Windows rebuild the window's redirection surface,
//! which blinks. Only `WS_EX_TRANSPARENT` and `WS_EX_NOACTIVATE` change.
use serde::Deserialize;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY},
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
    UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        GWL_EXSTYLE, GW_HWNDPREV, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
    },
};

pub const LABEL: &str = "overlay";

/// Cursor poll interval while a hit rect is active (~60 Hz).
const POLL: Duration = Duration::from_millis(16);
/// Logical px the hit rect grows by once the cursor is inside, so it does not
/// flicker at the edge.
const SLACK: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Window hidden.
    Hidden,
    /// Resting tick: the hover pill is the hit rect; hover is reported to the frontend.
    Tick,
    /// Always click-through, no polling (dictation pills).
    Passive,
    /// The pill is the hit rect; never takes focus (hands-free stop, action pills).
    Interactive,
    /// The panel is the hit rect; receives keyboard focus (the opened surface).
    Focused,
}

impl Mode {
    /// Modes where the cursor is polled against the hit rect.
    fn has_hit_rect(self) -> bool {
        matches!(self, Mode::Tick | Mode::Interactive | Mode::Focused)
    }
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
    /// Hit rect as logical insets from the window edges: top, right, bottom, left.
    inset: [f64; 4],
    /// Cursor inside the hit rect: the window takes the mouse. In `Tick` this
    /// is also the hover state reported to the frontend.
    inside: bool,
    /// Monitor of the last placement; the DPI double-set only runs when it changes.
    placed_on: Option<HMONITOR>,
    last_blur: Option<Instant>,
    /// The app that had focus before the surface opened; it gets focus back on close.
    prev_foreground: isize,
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

/// The hit rect in physical px, as `[left, top, right, bottom]`, from the
/// window rect, the logical insets (top, right, bottom, left) and the DPI
/// scale. Once the cursor is inside, the rect grows by `SLACK` on every side.
fn hit_rect(window: [i32; 4], inset: [f64; 4], scale: f64, inside: bool) -> [i32; 4] {
    let [t, r, b, l] = inset.map(|v| (v * scale).round() as i32);
    let slack = if inside { (SLACK * scale).round() as i32 } else { 0 };
    [window[0] + l - slack, window[1] + t - slack, window[2] - r + slack, window[3] - b + slack]
}

fn contains(rect: [i32; 4], x: i32, y: i32) -> bool {
    x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]
}

/// Next inside state. Leaving waits while a mouse button is held, so a drag
/// that started inside (a slider, a text selection) keeps the mouse until it
/// is released; entering is immediate.
fn next_inside(inside: bool, in_rect: bool, button_down: bool) -> bool {
    in_rect || (inside && button_down)
}

fn click_through(mode: Mode, inside: bool) -> bool {
    !(mode.has_hit_rect() && inside)
}

fn button_down() -> bool {
    // Physical buttons; covers swapped buttons too.
    unsafe { [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|&vk| GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0) }
}

fn rect_of(h: HWND) -> [i32; 4] {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(h, &mut r) };
    [r.left, r.top, r.right, r.bottom]
}

/// Cursor inside the hit rect of `g`, for a window at `window`.
fn cursor_inside(window: [i32; 4], g: &Geometry) -> bool {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    let in_rect = contains(hit_rect(window, g.inset, scale_for(g.monitor), g.inside), p.x, p.y);
    next_inside(g.inside, in_rect, button_down())
}

/// Bottom-center of the monitor's work area (above the taskbar), physical px
/// as `[left, top, right, bottom]`. The 12 px gap is part of the window: the
/// frontend pads its content.
fn target_rect(g: &Geometry) -> Option<[i32; 4]> {
    unsafe {
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(g.monitor, &mut mi) == 0 {
            return None;
        }
        let work = mi.rcWork;
        let scale = scale_for(g.monitor);
        let w = (g.width * scale).round() as i32;
        let hgt = (g.height * scale).round() as i32;
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.bottom - hgt;
        Some([x, y, x + w, y + hgt])
    }
}

/// A visible window above ours in the z-order that overlaps `rect`. Only
/// topmost windows can be above a topmost one, so the walk is short.
fn covered(h: HWND, rect: [i32; 4]) -> bool {
    unsafe {
        let mut above = GetWindow(h, GW_HWNDPREV);
        for _ in 0..256 {
            if above.is_null() {
                return false;
            }
            if IsWindowVisible(above) != 0 {
                let r = rect_of(above);
                if r[0] < rect[2] && rect[0] < r[2] && r[1] < rect[3] && rect[1] < r[3] {
                    return true;
                }
            }
            above = GetWindow(above, GW_HWNDPREV);
        }
        true
    }
}

/// Move the window to `rect`, topmost. Skipped when it is already there and
/// nothing covers it: every resize makes WebView2 re-layout asynchronously,
/// which shows as a frame or two of lag.
fn place(h: HWND, rect: [i32; 4], monitor_changed: bool) {
    if !monitor_changed && rect_of(h) == rect {
        if covered(h, rect) {
            // Z-order only: no move or size, so no WM_SIZE.
            unsafe { SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }
        return;
    }
    let [x, y, r, b] = rect;
    // Twice when changing monitors: if this move crosses into a monitor with
    // another DPI, the first call triggers WM_DPICHANGED (which resizes to a
    // suggested rect); the second call, now on the target monitor, restores
    // the exact rect.
    for _ in 0..if monitor_changed { 2 } else { 1 } {
        unsafe { SetWindowPos(h, HWND_TOPMOST, x, y, r - x, b - y, SWP_NOACTIVATE) };
    }
}

/// Click-through and focusability as extended styles, set directly. tao's
/// setters re-apply all of its window flags on every change, including an
/// activating `SW_SHOW` (or `SW_HIDE`), which would steal focus from the app
/// being dictated into.
///
/// `WS_EX_LAYERED` is always kept (and restored if anything stripped it).
/// `WS_EX_TRANSPARENT` and `WS_EX_NOACTIVATE` are read live by hit testing
/// and activation, so no `SetWindowPos` follows; `SWP_FRAMECHANGED` would
/// recompute the frame for nothing.
fn set_styles(h: HWND, click_through: bool, no_activate: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        let mut next = (ex & !(WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)) | WS_EX_LAYERED;
        if click_through {
            next |= WS_EX_TRANSPARENT;
        }
        if no_activate {
            next |= WS_EX_NOACTIVATE;
        }
        if next != ex {
            SetWindowLongPtrW(h, GWL_EXSTYLE, next as isize);
        }
    }
}

/// Before the frontend draws anything: layered, click-through, non-activating,
/// shown once through tao so its idea of visibility stays "visible".
pub fn init(win: &WebviewWindow) {
    let Some(h) = hwnd(win) else {
        let _ = win.show();
        return;
    };
    set_styles(h, true, true);
    let _ = win.show();
    // tao rewrites the extended style from its own flags when visibility
    // changes, dropping ours; set them again, once, for good.
    set_styles(h, true, true);
}

/// Styles for the current mode and cursor state, on the main thread. The
/// poll thread posts here instead of setting styles itself: style changes
/// send messages to the window's thread, and whatever runs here last reads
/// the latest state.
fn sync_styles(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else { return };
    let Some(h) = hwnd(&win) else { return };
    let guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
    let Some(g) = guard.as_ref() else { return };
    let (mode, inside) = (g.mode, g.inside);
    drop(guard);
    if mode != Mode::Hidden {
        set_styles(h, click_through(mode, inside), mode != Mode::Focused);
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
    log::debug!("surface_update {mode:?} {width}x{height} anchor={anchor:?} inset={inset:?}");
    let win = app.get_webview_window(LABEL).ok_or("no window")?;
    let h = hwnd(&win).ok_or("no window handle")?;
    let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
    let g = guard.get_or_insert_with(|| Geometry {
        monitor: monitor_for(Anchor::Primary, Some(h)),
        width,
        height,
        mode,
        inset: [0.0; 4],
        inside: false,
        placed_on: None,
        last_blur: None,
        prev_foreground: 0,
    });
    if let Some(anchor) = anchor {
        g.monitor = monitor_for(anchor, Some(h));
    }
    g.width = width.clamp(1.0, 2000.0);
    g.height = height.clamp(1.0, 2000.0);
    let fg = unsafe { GetForegroundWindow() };
    if mode == Mode::Focused && g.mode != Mode::Focused && fg != h {
        g.prev_foreground = fg as isize;
    }
    let hand_back = g.mode == Mode::Focused && mode != Mode::Focused && fg == h && g.prev_foreground != 0;
    let entering_tick = mode == Mode::Tick && g.mode != Mode::Tick;
    g.mode = mode;
    // No inset: the whole window is the hit rect.
    g.inset = inset.unwrap_or([0.0; 4]);
    let placement = if mode == Mode::Hidden { None } else { target_rect(g) };
    let monitor_changed = g.placed_on != Some(g.monitor);
    if placement.is_some() {
        g.placed_on = Some(g.monitor);
    }
    // In the tick, hover starts false (the frontend assumes so) and only the
    // poll changes it, since every change must be reported. Elsewhere the
    // cursor is checked now, so a pill or panel that appears under it takes
    // the click without waiting for the poll.
    g.inside = match mode {
        _ if !mode.has_hit_rect() || entering_tick => false,
        Mode::Tick => g.inside,
        _ => cursor_inside(placement.unwrap_or_else(|| rect_of(h)), g),
    };
    let (styles_click_through, no_activate) = (click_through(mode, g.inside), mode != Mode::Focused);
    let prev_foreground = g.prev_foreground;
    drop(guard);

    if hand_back {
        unsafe { SetForegroundWindow(prev_foreground as HWND) };
    }
    if mode == Mode::Hidden {
        unsafe { ShowWindow(h, SW_HIDE) };
    } else {
        if let Some(rect) = placement {
            place(h, rect, monitor_changed);
        }
        set_styles(h, styles_click_through, no_activate);
        if unsafe { IsWindowVisible(h) } == 0 {
            unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
        }
        if mode == Mode::Focused {
            let _ = win.set_focus();
        }
    }
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

/// Click-through outside the hit rect, and hover for the resting tick.
pub fn start_hover_poll(app: AppHandle) {
    let _ = std::thread::Builder::new().name("chirp-hover".into()).spawn(move || loop {
        let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
        while !guard.as_ref().is_some_and(|g| g.mode.has_hit_rect()) {
            guard = WAKE.wait(guard).unwrap_or_else(|e| e.into_inner());
        }
        drop(guard);
        std::thread::sleep(POLL);
        let Some(win) = app.get_webview_window(LABEL) else { continue };
        let Some(h) = hwnd(&win) else { continue };
        let mut guard = GEOMETRY.lock().unwrap_or_else(|e| e.into_inner());
        let Some(g) = guard.as_mut() else { continue };
        if !g.mode.has_hit_rect() {
            continue;
        }
        let inside = cursor_inside(rect_of(h), g);
        if inside == g.inside {
            continue;
        }
        g.inside = inside;
        let tick = g.mode == Mode::Tick;
        drop(guard);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || sync_styles(&handle));
        if tick {
            let _ = app.emit("surface-hover", inside);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: [i32; 4] = [100, 200, 780, 692];

    #[test]
    fn hit_rect_applies_insets() {
        // top, right, bottom, left
        assert_eq!(hit_rect(WIN, [10.0, 20.0, 30.0, 40.0], 1.0, false), [140, 210, 760, 662]);
        assert_eq!(hit_rect(WIN, [0.0; 4], 1.0, false), WIN);
    }

    #[test]
    fn hit_rect_scales_insets_and_slack() {
        // 150 %: insets and slack are logical px.
        assert_eq!(hit_rect(WIN, [10.0, 20.0, 30.0, 40.0], 1.5, false), [160, 215, 750, 647]);
        assert_eq!(hit_rect(WIN, [10.0, 20.0, 30.0, 40.0], 1.5, true), [151, 206, 759, 656]);
        // 125 % rounds rather than truncates: 6 * 1.25 = 7.5 -> 8, 3 * 1.25 = 3.75 -> 4.
        assert_eq!(hit_rect(WIN, [3.0, 3.0, 3.0, 3.0], 1.25, true), [96, 196, 784, 696]);
    }

    #[test]
    fn slack_only_once_inside() {
        let inset = [100.0, 100.0, 100.0, 100.0];
        // 3 px left of the rect: outside until hovering, then within the slack.
        let (x, y) = (WIN[0] + 97, WIN[1] + 150);
        assert!(!contains(hit_rect(WIN, inset, 1.0, false), x, y));
        assert!(contains(hit_rect(WIN, inset, 1.0, true), x, y));
        // Beyond the slack.
        assert!(!contains(hit_rect(WIN, inset, 1.0, true), WIN[0] + 93, y));
    }

    #[test]
    fn contains_is_half_open() {
        let r = [10, 10, 20, 20];
        assert!(contains(r, 10, 10));
        assert!(contains(r, 19, 19));
        assert!(!contains(r, 20, 15));
        assert!(!contains(r, 15, 20));
        assert!(!contains(r, 9, 15));
    }

    #[test]
    fn drag_keeps_the_mouse_until_release() {
        assert!(next_inside(false, true, false));
        assert!(next_inside(false, true, true));
        assert!(!next_inside(true, false, false));
        // Slider drag leaves the rect: stays interactive while held.
        assert!(next_inside(true, false, true));
        // A drag from elsewhere passing over stays click-through.
        assert!(!next_inside(false, false, true));
    }

    #[test]
    fn click_through_by_mode() {
        for mode in [Mode::Tick, Mode::Interactive, Mode::Focused] {
            assert!(!click_through(mode, true));
            assert!(click_through(mode, false));
        }
        assert!(click_through(Mode::Passive, true));
        assert!(click_through(Mode::Hidden, true));
    }
}
