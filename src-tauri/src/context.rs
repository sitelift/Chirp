//! Active-app context for LLM cleanup: the foreground window's app name and title.
//! Never screenshots, never text from around the cursor (see DIRECTION.md).

use crate::llm::AppContext;

#[cfg(windows)]
pub fn foreground() -> AppContext {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return AppContext::default();
        }

        let mut title_buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf[..n.max(0) as usize]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut exe = String::new();
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !process.is_null() {
            let mut path_buf = [0u16; 1024];
            let mut len = path_buf.len() as u32;
            if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, path_buf.as_mut_ptr(), &mut len) != 0 {
                exe = String::from_utf16_lossy(&path_buf[..len as usize]);
            }
            CloseHandle(process);
        }

        AppContext { app: friendly_app_name(&exe), title }
    }
}

#[cfg(not(windows))]
pub fn foreground() -> AppContext {
    AppContext::default()
}

/// Map an executable path to the name people call the app.
fn friendly_app_name(exe_path: &str) -> String {
    let stem = exe_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE");
    let name = match stem.to_ascii_lowercase().as_str() {
        "chrome" => "Google Chrome",
        "msedge" => "Microsoft Edge",
        "firefox" => "Firefox",
        "arc" => "Arc",
        "brave" => "Brave",
        "slack" => "Slack",
        "ms-teams" | "teams" => "Microsoft Teams",
        "discord" => "Discord",
        "whatsapp" => "WhatsApp",
        "telegram" => "Telegram",
        "outlook" | "olk" => "Outlook",
        "thunderbird" => "Thunderbird",
        "winword" => "Microsoft Word",
        "excel" => "Microsoft Excel",
        "powerpnt" => "PowerPoint",
        "onenote" => "OneNote",
        "notion" => "Notion",
        "obsidian" => "Obsidian",
        "code" => "Visual Studio Code",
        "cursor" => "Cursor",
        "windowsterminal" => "Windows Terminal",
        "powershell" | "pwsh" => "PowerShell",
        "cmd" => "Command Prompt",
        "claude" => "Claude",
        "chatgpt" => "ChatGPT",
        "notepad" => "Notepad",
        "explorer" => "File Explorer",
        _ => return stem.to_string(),
    };
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::friendly_app_name;

    #[test]
    fn maps_known_and_unknown() {
        assert_eq!(friendly_app_name(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), "Google Chrome");
        assert_eq!(friendly_app_name(r"C:\x\SomeTool.exe"), "SomeTool");
        assert_eq!(friendly_app_name(""), "");
    }
}
