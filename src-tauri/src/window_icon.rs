//! Use Windows' ICO decoder so window icons retain a valid monochrome mask.
//!
//! Tao 0.34's RGBA icon conversion passes one alpha byte per pixel to CreateIcon,
//! which expects a packed 1-bit AND mask. Alpha-aware surfaces still look right,
//! but Task Manager's window rows can show a black box and striped edges.

use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED,
        SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    },
};

pub fn set_native_icons(window: &tauri::WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    let hwnd = window.hwnd()?.0;

    // SAFETY: setup runs on the UI thread and hwnd belongs to a live Tauri window.
    // Tauri-build embeds icons/icon.ico as resource 32512. LR_SHARED keeps these
    // handles valid for the process lifetime; they must not be DestroyIcon'd.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        if module.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        for (kind, width, height) in [
            (ICON_SMALL, SM_CXSMICON, SM_CYSMICON),
            (ICON_BIG, SM_CXICON, SM_CYICON),
        ] {
            let icon = LoadImageW(
                module,
                32512usize as *const u16,
                IMAGE_ICON,
                GetSystemMetrics(width),
                GetSystemMetrics(height),
                LR_SHARED,
            );
            if icon.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            // Tauri owns the previous icons; do not destroy the returned handle.
            SendMessageW(hwnd, WM_SETICON, kind as usize, icon as isize);
        }
    }
    Ok(())
}
