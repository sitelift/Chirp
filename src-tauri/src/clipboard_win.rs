use std::ffi::c_void;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;

const CF_UNICODETEXT: u32 = 13;

/// Return true if a clipboard format's data is stored as an HGLOBAL handle
/// (safe to pass to GlobalSize/GlobalLock/SetClipboardData via GlobalAlloc).
/// Formats backed by GDI handles (CF_BITMAP, CF_METAFILEPICT, CF_PALETTE,
/// CF_ENHMETAFILE, DSP* variants) will cause heap corruption if treated as
/// HGLOBAL. Private (0x0200-0x02FF) and GDIOBJ (0x0300-0x03FF) ranges are
/// owner-defined and skipped. Registered formats (>= 0xC000) are HGLOBAL
/// by contract.
fn is_hglobal_format(fmt: u32) -> bool {
    match fmt {
        // Standard HGLOBAL-backed formats
        1    // CF_TEXT
        | 4  // CF_SYLK
        | 5  // CF_DIF
        | 6  // CF_TIFF
        | 7  // CF_OEMTEXT
        | 8  // CF_DIB
        | 11 // CF_RIFF
        | 12 // CF_WAVE
        | 13 // CF_UNICODETEXT
        | 15 // CF_HDROP
        | 16 // CF_LOCALE
        | 17 // CF_DIBV5
        => true,
        // Registered formats are always HGLOBAL
        0xC000..=0xFFFF => true,
        _ => false,
    }
}

/// Saved clipboard state for restore after paste
pub struct SavedClipboard {
    formats: Vec<(u32, Vec<u8>)>,
}

// Allow sending SavedClipboard to background thread for restore
unsafe impl Send for SavedClipboard {}

/// Registered clipboard format IDs (cached)
struct FormatIds {
    exclude_monitoring: u32,
    exclude_history: u32,
    exclude_cloud: u32,
}

fn format_ids() -> &'static FormatIds {
    static IDS: OnceLock<FormatIds> = OnceLock::new();
    IDS.get_or_init(|| unsafe {
        FormatIds {
            exclude_monitoring: RegisterClipboardFormatW(
                wide_str("ExcludeClipboardContentFromMonitorProcessing").as_ptr(),
            ),
            exclude_history: RegisterClipboardFormatW(
                wide_str("CanIncludeInClipboardHistory").as_ptr(),
            ),
            exclude_cloud: RegisterClipboardFormatW(
                wide_str("CanUploadToCloudClipboard").as_ptr(),
            ),
        }
    })
}

/// Convert a &str to null-terminated UTF-16 Vec
fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Allocate a GMEM_MOVEABLE HGLOBAL with the given bytes, returning the handle.
/// The caller must NOT free this handle after passing it to SetClipboardData.
unsafe fn alloc_global(data: &[u8]) -> Result<*mut c_void, String> {
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if h.is_null() {
        return Err("GlobalAlloc failed".into());
    }
    let ptr = GlobalLock(h);
    if ptr.is_null() {
        GlobalFree(h);
        return Err("GlobalLock failed".into());
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
    GlobalUnlock(h);
    Ok(h)
}

/// Try to open the clipboard with retry loop
unsafe fn open_clipboard_retry() -> Result<(), String> {
    for attempt in 0..5 {
        if OpenClipboard(0 as HWND) != 0 {
            return Ok(());
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    Err("Failed to open clipboard after 5 attempts".into())
}

/// Save current clipboard contents (all formats)
pub fn save_clipboard() -> Result<SavedClipboard, String> {
    unsafe {
        open_clipboard_retry()?;

        let mut formats = Vec::new();
        let mut fmt = EnumClipboardFormats(0);
        while fmt != 0 {
            if !is_hglobal_format(fmt) {
                fmt = EnumClipboardFormats(fmt);
                continue;
            }
            let h = GetClipboardData(fmt);
            if !h.is_null() {
                let size = GlobalSize(h);
                if size > 0 {
                    let ptr = GlobalLock(h);
                    if !ptr.is_null() {
                        let mut data = vec![0u8; size];
                        std::ptr::copy_nonoverlapping(ptr as *const u8, data.as_mut_ptr(), size);
                        GlobalUnlock(h);
                        formats.push((fmt, data));
                    }
                }
            }
            fmt = EnumClipboardFormats(fmt);
        }

        CloseClipboard();
        log::debug!("Saved {} clipboard formats", formats.len());
        Ok(SavedClipboard { formats })
    }
}

/// Set clipboard with plain text and exclusion flags
pub fn set_clipboard_with_exclusion(plain: &str) -> Result<(), String> {
    let ids = format_ids();

    unsafe {
        open_clipboard_retry()?;
        EmptyClipboard();

        // CF_UNICODETEXT: null-terminated UTF-16LE
        let utf16: Vec<u8> = plain
            .encode_utf16()
            .chain(std::iter::once(0u16))
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let h = alloc_global(&utf16)?;
        if SetClipboardData(CF_UNICODETEXT, h).is_null() {
            CloseClipboard();
            return Err("SetClipboardData CF_UNICODETEXT failed".into());
        }

        // Exclusion flags — each is a DWORD(0)
        let zero: [u8; 4] = [0, 0, 0, 0];
        if let Ok(h) = alloc_global(&zero) {
            SetClipboardData(ids.exclude_monitoring, h);
        }
        if let Ok(h) = alloc_global(&zero) {
            SetClipboardData(ids.exclude_history, h);
        }
        if let Ok(h) = alloc_global(&zero) {
            SetClipboardData(ids.exclude_cloud, h);
        }

        CloseClipboard();
        log::info!("Clipboard set with exclusion flags");
        Ok(())
    }
}

/// Verify clipboard contains expected plain text
pub fn verify_clipboard_text(expected: &str) -> Result<bool, String> {
    unsafe {
        open_clipboard_retry()?;

        let h = GetClipboardData(CF_UNICODETEXT);
        if h.is_null() {
            CloseClipboard();
            return Ok(false);
        }

        let ptr = GlobalLock(h);
        if ptr.is_null() {
            CloseClipboard();
            return Ok(false);
        }

        // Read UTF-16LE null-terminated string
        let size = GlobalSize(h);
        let u16_slice =
            std::slice::from_raw_parts(ptr as *const u16, size / std::mem::size_of::<u16>());
        // Find null terminator
        let len = u16_slice.iter().position(|&c| c == 0).unwrap_or(u16_slice.len());
        let text = String::from_utf16_lossy(&u16_slice[..len]);

        GlobalUnlock(h);
        CloseClipboard();

        Ok(text == expected)
    }
}

/// Restore previously saved clipboard contents
pub fn restore_clipboard(saved: SavedClipboard) -> Result<(), String> {
    unsafe {
        open_clipboard_retry()?;
        EmptyClipboard();

        for (fmt, data) in &saved.formats {
            if let Ok(h) = alloc_global(data) {
                SetClipboardData(*fmt, h);
            }
        }

        CloseClipboard();
        log::debug!("Clipboard restored ({} formats)", saved.formats.len());
        Ok(())
    }
}
