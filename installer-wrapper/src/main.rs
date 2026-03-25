#![windows_subsystem = "windows"]
#![allow(unused_must_use)]

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

// Window dimensions
const WIN_W: i32 = 420;
const WIN_H: i32 = 320;

// Colors (BGR for GDI)
const BG_COLOR: u32 = 0x0017191a; // #1a1917
const TEXT_COLOR: u32 = 0x00FFFFFF; // white
const MUTED_COLOR: u32 = 0x00999999; // ~40% white on dark
const AMBER_COLOR: u32 = 0x0023B7F0; // #F0B723 in BGR

// Progress bar geometry
const BAR_H: i32 = 4;
const BAR_MARGIN: i32 = 32;
const BAR_BOTTOM: i32 = 40;

// Timer ID for animation
const TIMER_ID: usize = 1;
const TIMER_MS: u32 = 16; // ~60fps

// Installer config
const NSIS_INSTALLER: &str = "Chirp_setup.exe";
const INSTALLED_EXE: &str = "chirp.exe";

/// Shared state between the window proc and the installer thread.
struct AppState {
    start_time: Instant,
    /// 0 = installing, 1 = done (briefly), 2 = exit
    phase: AtomicU32,
    /// Raw HWND stored atomically for cross-thread PostMessage.
    hwnd_raw: AtomicIsize,
}

/// Global state, initialized once at startup.
static STATE: OnceLock<Arc<AppState>> = OnceLock::new();

fn state() -> &'static Arc<AppState> {
    STATE.get().expect("state not initialized")
}

fn main() -> Result<()> {
    let app_state = Arc::new(AppState {
        start_time: Instant::now(),
        phase: AtomicU32::new(0),
        hwnd_raw: AtomicIsize::new(0),
    });
    STATE.set(app_state.clone()).ok();

    // Spawn installer subprocess
    std::thread::spawn(move || {
        run_installer();
    });

    // Create and run the window
    unsafe { create_window() }
}

fn run_installer() {
    let state = state();

    // Look for the NSIS installer next to our exe
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let installer_path = exe_dir.join(NSIS_INSTALLER);

    if !installer_path.exists() {
        // If installer not found, simulate a 3-second install for demo/testing
        std::thread::sleep(std::time::Duration::from_secs(3));
    } else {
        // Run NSIS silently
        let install_dir = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| String::from("C:\\Users\\Default\\AppData\\Local"));
        let install_dir = format!("{}\\com.chirp.app", install_dir);

        let _result = std::process::Command::new(&installer_path)
            .arg("/S")
            .arg(format!("/D={}", install_dir))
            .status();
    }

    // Mark done
    state.phase.store(1, Ordering::SeqCst);

    // Wait 1.5s showing "Done!", then signal exit
    std::thread::sleep(std::time::Duration::from_millis(1500));
    state.phase.store(2, Ordering::SeqCst);

    // Launch the installed app
    launch_app();

    // Post quit to the window
    let raw = state.hwnd_raw.load(Ordering::SeqCst);
    if raw != 0 {
        let hwnd = HWND(raw as *mut _);
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

fn launch_app() {
    let install_dir = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| String::from("C:\\Users\\Default\\AppData\\Local"));
    let app_path = format!("{}\\com.chirp.app\\{}", install_dir, INSTALLED_EXE);

    if std::path::Path::new(&app_path).exists() {
        unsafe {
            let path_wide: Vec<u16> =
                app_path.encode_utf16().chain(std::iter::once(0)).collect();
            let open_wide: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
            ShellExecuteW(
                None,
                PCWSTR(open_wide.as_ptr()),
                PCWSTR(path_wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

unsafe fn create_window() -> Result<()> {
    let instance = GetModuleHandleW(None)?;
    let class_name = w!("ChirpInstallerSplash");

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: HINSTANCE(instance.0),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hbrBackground: HBRUSH(std::ptr::null_mut()),
        lpszClassName: class_name,
        ..Default::default()
    };

    RegisterClassExW(&wc);

    // Get screen size for centering
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);
    let x = (screen_w - WIN_W) / 2;
    let y = (screen_h - WIN_H) / 2;

    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST, // No taskbar button, always on top
        class_name,
        w!("Chirp Installer"),
        WS_POPUP | WS_VISIBLE,
        x,
        y,
        WIN_W,
        WIN_H,
        None,
        None,
        instance,
        None,
    )?;

    // Store HWND for cross-thread access
    state()
        .hwnd_raw
        .store(hwnd.0 as isize, Ordering::SeqCst);

    // Apply rounded corners via DWM (Windows 11)
    let preference = DWM_WINDOW_CORNER_PREFERENCE(2); // DWMWCP_ROUND
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &preference as *const _ as *const std::ffi::c_void,
        std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
    );

    // Start animation timer
    SetTimer(hwnd, TIMER_ID, TIMER_MS, None);

    // Message loop
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    Ok(())
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hdc);
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            // We handle background in WM_PAINT
            LRESULT(1)
        }
        // Allow dragging the borderless window
        WM_NCHITTEST => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            if result == LRESULT(HTCLIENT as isize) {
                return LRESULT(HTCAPTION as isize);
            }
            result
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint(hdc: HDC) {
    let state = state();
    let phase = state.phase.load(Ordering::SeqCst);
    let elapsed = state.start_time.elapsed().as_secs_f64();

    // Double-buffer: create offscreen DC
    let mem_dc = CreateCompatibleDC(hdc);
    let bmp = CreateCompatibleBitmap(hdc, WIN_W, WIN_H);
    let old_bmp = SelectObject(mem_dc, bmp);

    // Fill background
    let bg_brush = CreateSolidBrush(COLORREF(BG_COLOR));
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: WIN_W,
        bottom: WIN_H,
    };
    FillRect(mem_dc, &bg_rect, bg_brush);
    DeleteObject(bg_brush);

    // Set up text rendering
    SetBkMode(mem_dc, TRANSPARENT);

    // Draw the bird icon (embedded PNG rendered via GDI)
    draw_bird_png(mem_dc, WIN_W / 2, 90);

    // Draw "chirp" title text
    let title_font = create_font(32, 800, "Segoe UI");
    let old_font = SelectObject(mem_dc, title_font);
    SetTextColor(mem_dc, COLORREF(TEXT_COLOR));

    let mut title_buf = encode_wide("chirp");
    let mut title_rect = RECT {
        left: 0,
        top: 155,
        right: WIN_W,
        bottom: 200,
    };
    DrawTextW(
        mem_dc,
        &mut title_buf,
        &mut title_rect,
        DT_CENTER | DT_SINGLELINE,
    );

    // Draw status text
    let status_font = create_font(16, 400, "Segoe UI");
    SelectObject(mem_dc, status_font);
    SetTextColor(mem_dc, COLORREF(MUTED_COLOR));

    let status_text = if phase >= 1 { "Done!" } else { "Installing..." };
    let mut status_buf = encode_wide(status_text);
    let mut status_rect = RECT {
        left: 0,
        top: 195,
        right: WIN_W,
        bottom: 230,
    };
    DrawTextW(
        mem_dc,
        &mut status_buf,
        &mut status_rect,
        DT_CENTER | DT_SINGLELINE,
    );

    // Draw progress bar
    let bar_y = WIN_H - BAR_BOTTOM;
    let bar_x = BAR_MARGIN;
    let bar_w = WIN_W - BAR_MARGIN * 2;

    // Bar background (slightly lighter than bg)
    let bar_bg_brush = CreateSolidBrush(COLORREF(0x002D2F30));
    let bar_bg_rect = RECT {
        left: bar_x,
        top: bar_y,
        right: bar_x + bar_w,
        bottom: bar_y + BAR_H,
    };
    FillRect(mem_dc, &bar_bg_rect, bar_bg_brush);
    DeleteObject(bar_bg_brush);

    // Bar fill (amber)
    let progress = calculate_progress(elapsed, phase);
    let fill_w = (bar_w as f64 * progress) as i32;

    if fill_w > 0 {
        let bar_fill_brush = CreateSolidBrush(COLORREF(AMBER_COLOR));
        let bar_fill_rect = RECT {
            left: bar_x,
            top: bar_y,
            right: bar_x + fill_w,
            bottom: bar_y + BAR_H,
        };
        FillRect(mem_dc, &bar_fill_rect, bar_fill_brush);
        DeleteObject(bar_fill_brush);
    }

    // Blit to screen
    BitBlt(hdc, 0, 0, WIN_W, WIN_H, mem_dc, 0, 0, SRCCOPY);

    // Cleanup GDI objects
    SelectObject(mem_dc, old_bmp);
    SelectObject(mem_dc, old_font);
    DeleteObject(title_font);
    DeleteObject(status_font);
    DeleteObject(bmp);
    DeleteDC(mem_dc);
}

/// Embedded bird icon PNG (64x64, from the app's tray icon)
const BIRD_PNG: &[u8] = include_bytes!("../../src-tauri/icons/64x64.png");

/// Draw the real bird icon from the embedded PNG
unsafe fn draw_bird_png(hdc: HDC, cx: i32, cy: i32) {
    // Decode PNG
    let decoder = png::Decoder::new(std::io::Cursor::new(BIRD_PNG));
    let mut reader = match decoder.read_info() {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = match reader.next_frame(&mut buf) {
        Ok(i) => i,
        Err(_) => return,
    };
    let w = info.width as i32;
    let h = info.height as i32;

    // Create a DIB section for alpha-aware rendering
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0, // BI_RGB
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0);
    if dib.is_err() || bits_ptr.is_null() {
        return;
    }
    let dib = dib.unwrap();

    // Fill DIB with BGRA pixel data, alpha-blending against our dark background
    let pixels = std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (w * h * 4) as usize);
    let bg_r = 0x1au8;
    let bg_g = 0x19u8;
    let bg_b = 0x17u8;

    for y in 0..h as usize {
        for x in 0..w as usize {
            let src_idx = (y * w as usize + x) * 4; // RGBA from PNG
            let dst_idx = (y * w as usize + x) * 4; // BGRA for DIB

            if src_idx + 3 < buf.len() {
                let r = buf[src_idx];
                let g = buf[src_idx + 1];
                let b = buf[src_idx + 2];
                let a = buf[src_idx + 3] as u16;

                // Alpha blend against dark background
                let blended_r = ((r as u16 * a + bg_r as u16 * (255 - a)) / 255) as u8;
                let blended_g = ((g as u16 * a + bg_g as u16 * (255 - a)) / 255) as u8;
                let blended_b = ((b as u16 * a + bg_b as u16 * (255 - a)) / 255) as u8;

                pixels[dst_idx] = blended_b;     // B
                pixels[dst_idx + 1] = blended_g; // G
                pixels[dst_idx + 2] = blended_r; // R
                pixels[dst_idx + 3] = 0xFF;      // A (opaque after blending)
            }
        }
    }

    // Blit the DIB to the target DC
    let mem_dc2 = CreateCompatibleDC(hdc);
    let old = SelectObject(mem_dc2, dib);
    let dest_x = cx - w / 2;
    let dest_y = cy - h / 2;
    BitBlt(hdc, dest_x, dest_y, w, h, mem_dc2, 0, 0, SRCCOPY);
    SelectObject(mem_dc2, old);
    DeleteObject(dib);
    DeleteDC(mem_dc2);
}

fn calculate_progress(elapsed: f64, phase: u32) -> f64 {
    if phase >= 1 {
        // Done - fill to 100%
        1.0
    } else if elapsed < 10.0 {
        // Ease-out cubic: fast start, slow finish, 0 to 90% over 10s
        let t = elapsed / 10.0;
        let eased = 1.0 - (1.0 - t).powi(3);
        eased * 0.9
    } else {
        // Slowly creep from 0.9 towards 0.95
        let extra = (elapsed - 10.0) / 60.0;
        0.9 + (extra * 0.05).min(0.05)
    }
}

unsafe fn create_font(size: i32, weight: i32, face: &str) -> HFONT {
    let height = -size; // Approximate: 1px ~ 1pt at 96 DPI

    let mut face_wide = [0u16; 32];
    for (i, c) in face.encode_utf16().enumerate() {
        if i >= 31 {
            break;
        }
        face_wide[i] = c;
    }

    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        PCWSTR(face_wide.as_ptr()),
    )
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
