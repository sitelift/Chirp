//! Active-app context for LLM cleanup: app name, window title, and for browsers the
//! domain of the active tab. Never screenshots, never text from around the cursor
//! (see DIRECTION.md, "Context").
//!
//! Two entry points:
//! - [`capture`] at hotkey press: app + title synchronously (< 1 ms), and for browsers
//!   a UI Automation read of the address bar posted to a background worker. Call
//!   [`PendingContext::resolve`] when the transcript is ready; by then the lookup has
//!   long finished, so it does not wait. Resolve waits at most until press + [`URL_BUDGET`].
//! - [`foreground`]: synchronous app + title only. It cannot detect Chrome Incognito
//!   (the window title has no marker), so it drops titles of Chromium browsers.
//!
//! Privacy rules:
//! - Only the host of the URL (minus `www.`) is kept; never the scheme, path, query or fragment.
//! - Password managers: app name only.
//! - Private windows (Incognito, InPrivate, Private Browsing, Guest): app name only.
//! - Chirp's own windows: nothing.

use crate::llm::AppContext;
use std::time::{Duration, Instant};

/// Longest the URL lookup may delay dictation, measured from the hotkey press.
pub const URL_BUDGET: Duration = Duration::from_millis(150);

/// Context captured at key press, possibly with an address-bar lookup still running.
pub struct PendingContext {
    context: AppContext,
    lookup: Option<Lookup>,
}

struct Lookup {
    browser: Browser,
    deadline: Instant,
    rx: std::sync::mpsc::Receiver<Reading>,
}

/// Raw strings read from a browser window over UI Automation.
#[derive(Debug, Default, Clone)]
struct Reading {
    /// Value of the address bar (may be empty, user-typed text, or a URL with or without scheme).
    address: Option<String>,
    /// Name of Chromium's `BrowserRootView`: "<page title> - Google Chrome[ (Incognito)]".
    root_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Browser {
    Chromium,
    Firefox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Normal,
    Browser(Browser),
    PasswordManager,
}

impl PendingContext {
    /// What is known without UI Automation (app + privacy-filtered title).
    pub fn context(&self) -> &AppContext {
        &self.context
    }

    /// Final context. Blocks at most until press + [`URL_BUDGET`]; a result that arrives
    /// later than that but before this call is still used.
    pub fn resolve(self) -> AppContext {
        let PendingContext { mut context, lookup } = self;
        let Some(lookup) = lookup else { return context };
        let wait = lookup.deadline.saturating_duration_since(Instant::now());
        let reading = lookup.rx.recv_timeout(wait).ok();
        apply_reading(&mut context, lookup.browser, reading);
        context
    }
}

/// Merge a UIA reading (or its absence) into a browser context.
fn apply_reading(ctx: &mut AppContext, browser: Browser, reading: Option<Reading>) {
    match reading {
        Some(r) => {
            if r.root_name.as_deref().is_some_and(root_name_marks_private) {
                ctx.title.clear();
                ctx.url = None;
                return;
            }
            if let Some(root) = r.root_name.as_deref() {
                // The root view name lacks Edge's profile label and tab count.
                let t = clean_title(root, &ctx.app);
                if !t.is_empty() {
                    ctx.title = t;
                }
            }
            ctx.url = r.address.as_deref().and_then(domain_from_address);
        }
        None => {
            // Without UIA we cannot rule out Chrome Incognito, whose title has no marker.
            if browser == Browser::Chromium {
                ctx.title.clear();
            }
            ctx.url = None;
        }
    }
}

/// Synchronous app + title. No URL; drops Chromium browser titles (see module docs).
pub fn foreground() -> AppContext {
    let (ctx, kind, _) = imp::probe();
    let mut ctx = ctx;
    if kind == Kind::Browser(Browser::Chromium) {
        ctx.title.clear();
    }
    ctx
}

/// Capture at hotkey press. Cheap; the address-bar read runs on a background thread.
pub fn capture() -> PendingContext {
    let start = Instant::now();
    capture_from(start, imp::probe())
}

fn capture_from(start: Instant, probed: (AppContext, Kind, isize)) -> PendingContext {
    let (context, kind, hwnd) = probed;
    let lookup = match kind {
        Kind::Browser(browser) if !context.app.is_empty() && !context.title.is_empty() => {
            let deadline = start + URL_BUDGET;
            imp::request_reading(hwnd, deadline).map(|rx| Lookup { browser, deadline, rx })
        }
        _ => None,
    };
    PendingContext { context, lookup }
}

/// Start the UI Automation worker ahead of the first dictation (CoCreateInstance ~10-15 ms).
pub fn prewarm() {
    imp::prewarm();
}

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested on every platform)
// ---------------------------------------------------------------------------

fn exe_stem(exe_path: &str) -> &str {
    let file = exe_path.rsplit(['\\', '/']).next().unwrap_or("");
    match file.len().checked_sub(4) {
        Some(i) if file.is_char_boundary(i) && file[i..].eq_ignore_ascii_case(".exe") => &file[..i],
        _ => file,
    }
}

/// Curated names. Wins over FileDescription, which is wrong for some apps
/// ("Windows Terminal Host", "Windows Command Processor", empty for many CLIs).
fn table_name(stem_lower: &str) -> Option<&'static str> {
    Some(match stem_lower {
        "chrome" => "Google Chrome",
        "msedge" => "Microsoft Edge",
        "firefox" => "Firefox",
        "arc" => "Arc",
        "brave" => "Brave",
        "opera" => "Opera",
        "vivaldi" => "Vivaldi",
        "zen" => "Zen Browser",
        "slack" => "Slack",
        "ms-teams" | "teams" => "Microsoft Teams",
        "discord" => "Discord",
        "whatsapp" | "whatsapp.root" => "WhatsApp",
        "telegram" => "Telegram",
        "signal" => "Signal",
        "outlook" | "olk" => "Outlook",
        "thunderbird" => "Thunderbird",
        "winword" => "Microsoft Word",
        "excel" => "Microsoft Excel",
        "powerpnt" => "PowerPoint",
        "onenote" => "OneNote",
        "notion" => "Notion",
        "obsidian" => "Obsidian",
        "code" => "Visual Studio Code",
        "code - insiders" => "Visual Studio Code Insiders",
        "cursor" => "Cursor",
        "windsurf" => "Windsurf",
        "devenv" => "Visual Studio",
        "windowsterminal" => "Windows Terminal",
        "powershell" | "pwsh" => "PowerShell",
        "cmd" => "Command Prompt",
        "claude" => "Claude",
        "chatgpt" => "ChatGPT",
        "notepad" => "Notepad",
        "explorer" => "File Explorer",
        "systemsettings" => "Settings",
        _ => return None,
    })
}

/// FileDescription values that are not the name people use for the app.
fn usable_description(desc: &str) -> Option<&str> {
    let d = desc.trim();
    let lower = d.to_ascii_lowercase();
    let generic = [
        "electron",
        "application frame host",
        "node.js: server-side javascript",
        "node.js javascript runtime",
        "java(tm) platform se binary",
        "openjdk platform binary",
        "python",
        "runtime broker",
        "windows host process (rundll32)",
    ];
    if d.is_empty() || d.len() > 60 || lower.ends_with(".exe") || generic.contains(&lower.as_str()) {
        None
    } else {
        Some(d)
    }
}

/// Name people call the app: curated table, then the exe's FileDescription, then the exe stem.
fn friendly_app_name(exe_path: &str, description: Option<&str>) -> String {
    let stem = exe_stem(exe_path);
    if let Some(name) = table_name(&stem.to_ascii_lowercase()) {
        return name.to_string();
    }
    if let Some(d) = description.and_then(usable_description) {
        return d.to_string();
    }
    stem.to_string()
}

fn browser_kind(stem_lower: &str) -> Option<Browser> {
    match stem_lower {
        "chrome" | "msedge" | "brave" | "vivaldi" | "opera" | "arc" | "chromium" => Some(Browser::Chromium),
        "firefox" | "zen" | "librewolf" | "waterfox" | "floorp" => Some(Browser::Firefox),
        _ => None,
    }
}

fn is_password_manager(stem_lower: &str, app_name: &str) -> bool {
    const STEMS: &[&str] = &[
        "1password", "bitwarden", "keepass", "keepassxc", "lastpass", "dashlane", "keeperpasswordmanager",
        "keeper", "enpass", "nordpass", "proton pass", "protonpass", "roboform", "pwsafe", "passwordsafe",
    ];
    let name = app_name.to_ascii_lowercase();
    STEMS.contains(&stem_lower) || name.contains("password") || name.contains("keepass") || name == "bitwarden"
}

fn kind_of(stem_lower: &str, app_name: &str) -> Kind {
    if is_password_manager(stem_lower, app_name) {
        Kind::PasswordManager
    } else if let Some(b) = browser_kind(stem_lower) {
        Kind::Browser(b)
    } else {
        Kind::Normal
    }
}

/// Title markers of private windows. Edge: "- [InPrivate] -"; Firefox: "Private Browsing";
/// Chrome puts no marker in the title (only in its UIA root view name).
fn title_marks_private(title: &str) -> bool {
    let t = title.to_lowercase();
    ["inprivate", "incognito", "private browsing", "(private)", "(guest)"]
        .iter()
        .any(|m| t.contains(m))
}

/// Chromium's root view name is "<title> - <Browser>" and gains a trailing
/// "(Incognito)" / "(InPrivate)" / "(Guest)" (localized) only in those windows.
fn root_name_marks_private(root_name: &str) -> bool {
    let r = root_name.trim_end();
    r.ends_with(')') || title_marks_private(r)
}

/// Zero-width and bidi control characters (Edge writes "Microsoft\u{200B} Edge").
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}')
}

/// Normalise a window title: drop invisible characters, Edge's "and N more pages", and
/// the trailing " - <App>" suffix that only repeats the <app> slot.
fn clean_title(title: &str, app: &str) -> String {
    let mut t: String = title.chars().filter(|c| !is_invisible(*c)).collect();
    t = t.trim().to_string();

    let alias = if app == "Firefox" { "Mozilla Firefox" } else { "" };
    for s in [app, alias].iter().filter(|s| !s.is_empty()) {
        for sep in [" - ", " — ", " – "] {
            let suffix = format!("{sep}{s}");
            if t.len() > suffix.len() && t.ends_with(&suffix) {
                t.truncate(t.len() - suffix.len());
            }
        }
    }

    // Edge: "<title> and 3 more pages - <Profile>" -> "<title>"
    if app == "Microsoft Edge" {
        if let Some(i) = t.rfind(" more page") {
            let head = &t[..i];
            if let Some(j) = head.rfind(" and ") {
                if head[j + 5..].chars().all(|c| c.is_ascii_digit()) && j > 0 {
                    t.truncate(j);
                }
            }
        }
    }
    t.trim().to_string()
}

/// Reduce an address-bar value to a host, or None if it is not a web URL.
/// "github.com/a/b?q" -> "github.com"; "https://www.bbc.com/x" -> "bbc.com";
/// "", "how to cook rice", "chrome://settings", "file:///C:/x" -> None.
fn domain_from_address(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() || v.chars().any(char::is_whitespace) {
        return None;
    }
    let rest = match v.find("://") {
        Some(i) => {
            let scheme = v[..i].to_ascii_lowercase();
            if scheme != "http" && scheme != "https" {
                return None;
            }
            &v[i + 3..]
        }
        None => {
            // "about:blank", "edge:newtab", "mailto:x" have a scheme but no "//".
            if let Some(i) = v.find(':') {
                let before = &v[..i];
                let after = &v[i + 1..];
                let is_port = !after.is_empty() && after.chars().take_while(|c| *c != '/').all(|c| c.is_ascii_digit());
                if !is_port && !before.contains('/') {
                    return None;
                }
            }
            v
        }
    };
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host = if host_port.starts_with('[') {
        return None; // IPv6 literal: nothing useful for register
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let host = host.trim_end_matches('.').to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let valid_chars = host.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.');
    let labels_ok = host.split('.').all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-'));
    if host.is_empty() || !valid_chars || !labels_ok || host.len() > 253 {
        return None;
    }
    if !host.contains('.') && host != "localhost" {
        return None;
    }
    Some(host)
}

/// Shared tail of the probe: from exe path + raw title to a filtered context.
fn build_context(exe: &str, description: Option<&str>, raw_title: &str) -> (AppContext, Kind) {
    let app = friendly_app_name(exe, description);
    let stem_lower = exe_stem(exe).to_ascii_lowercase();
    let kind = kind_of(&stem_lower, &app);
    let title = match kind {
        Kind::PasswordManager => String::new(),
        _ if title_marks_private(raw_title) => String::new(),
        _ => clean_title(raw_title, &app),
    };
    (AppContext { app, title, url: None }, kind)
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use super::{build_context, AppContext, Kind, Reading};
    use std::collections::HashMap;
    use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    /// Foreground window -> (context, kind, hwnd). Empty context for Chirp itself.
    pub(super) fn probe() -> (AppContext, Kind, isize) {
        probe_window(unsafe { GetForegroundWindow() } as isize)
    }

    pub(super) fn probe_window(hwnd: isize) -> (AppContext, Kind, isize) {
        unsafe {
            let hwnd = hwnd as HWND;
            if hwnd.is_null() {
                return (AppContext::default(), Kind::Normal, 0);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == std::process::id() {
                return (AppContext::default(), Kind::Normal, 0);
            }

            let mut title_buf = [0u16; 512];
            let n = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
            let raw_title = String::from_utf16_lossy(&title_buf[..n.max(0) as usize]);

            let mut exe = exe_path(pid);
            if super::exe_stem(&exe).eq_ignore_ascii_case("ApplicationFrameHost") {
                // UWP apps: the frame belongs to ApplicationFrameHost; the app is a child window.
                if let Some(child_pid) = uwp_child_pid(hwnd, pid) {
                    let child_exe = exe_path(child_pid);
                    if !child_exe.is_empty() {
                        exe = child_exe;
                    }
                }
            }
            let desc = if super::exe_stem(&exe).eq_ignore_ascii_case("ApplicationFrameHost") {
                Some(raw_title.clone()) // unresolved UWP frame: its title is the app name
            } else {
                file_description(&exe)
            };
            let (ctx, kind) = build_context(&exe, desc.as_deref(), &raw_title);
            (ctx, kind, hwnd as isize)
        }
    }

    unsafe fn exe_path(pid: u32) -> String {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(process);
        if ok {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }

    unsafe fn uwp_child_pid(hwnd: HWND, frame_pid: u32) -> Option<u32> {
        struct Search {
            frame_pid: u32,
            found: Option<u32>,
        }
        unsafe extern "system" fn cb(child: HWND, l: LPARAM) -> BOOL {
            let s = &mut *(l as *mut Search);
            let mut pid = 0u32;
            GetWindowThreadProcessId(child, &mut pid);
            if pid != 0 && pid != s.frame_pid {
                s.found = Some(pid);
                return 0;
            }
            1
        }
        let mut s = Search { frame_pid, found: None };
        EnumChildWindows(hwnd, Some(cb), &mut s as *mut Search as LPARAM);
        s.found
    }

    /// FileDescription from the exe's version resource, cached per path.
    fn file_description(exe: &str) -> Option<String> {
        if exe.is_empty() {
            return None;
        }
        static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        if let Some(hit) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(exe) {
            return hit.clone();
        }
        let desc = unsafe { read_file_description(exe) };
        cache.lock().unwrap_or_else(|e| e.into_inner()).insert(exe.to_string(), desc.clone());
        desc
    }

    unsafe fn read_file_description(exe: &str) -> Option<String> {
        use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let path = wide(exe);
        let size = GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let query = |sub: &str| -> Option<(*const u8, u32)> {
            let sub = wide(sub);
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            (VerQueryValueW(data.as_ptr().cast(), sub.as_ptr(), &mut ptr, &mut len) != 0 && !ptr.is_null() && len > 0)
                .then_some((ptr as *const u8, len))
        };
        let mut langs: Vec<(u16, u16)> = Vec::new();
        if let Some((ptr, len)) = query("\\VarFileInfo\\Translation") {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            langs.extend(pairs.chunks_exact(2).map(|p| (p[0], p[1])));
        }
        langs.extend([(0x0409, 0x04B0), (0x0409, 0x04E4), (0x0000, 0x04B0)]);
        for (lang, cp) in langs {
            if let Some((ptr, len)) = query(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription")) {
                let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
                let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
                let s = String::from_utf16_lossy(&chars[..end]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }

    // ---- UI Automation worker -------------------------------------------------

    struct Request {
        hwnd: isize,
        deadline: Instant,
        reply: SyncSender<Reading>,
    }

    fn worker() -> Option<&'static Mutex<Sender<Request>>> {
        static TX: OnceLock<Option<Mutex<Sender<Request>>>> = OnceLock::new();
        TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Request>();
            std::thread::Builder::new()
                .name("context-uia".into())
                .spawn(move || uia::run(rx))
                .ok()
                .map(|_| Mutex::new(tx))
        })
        .as_ref()
    }

    pub(super) fn prewarm() {
        let _ = worker();
    }

    pub(super) fn request_reading(hwnd: isize, deadline: Instant) -> Option<Receiver<Reading>> {
        let (reply, rx) = mpsc::sync_channel(1);
        let tx = worker()?.lock().unwrap_or_else(|e| e.into_inner());
        tx.send(Request { hwnd, deadline, reply }).ok()?;
        Some(rx)
    }

    mod uia {
        use super::{Reading, Request};
        use std::sync::mpsc::Receiver;
        use std::time::Instant;
        use windows::core::{Interface, BSTR};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
        use windows::Win32::System::Variant::{VARIANT, VT_BSTR};
        use windows::Win32::UI::Accessibility::{
            CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCondition, TreeScope_Children,
            TreeScope_Descendants, UIA_AutomationIdPropertyId, UIA_ClassNamePropertyId, UIA_ValueValuePropertyId,
        };

        /// Per-call cap on cross-process UIA calls, so a hung browser cannot stall the worker long.
        const UIA_TIMEOUT_MS: u32 = 150;

        struct Uia {
            automation: IUIAutomation,
            address_bar: IUIAutomationCondition,
            root_view: IUIAutomationCondition,
        }

        unsafe fn init() -> windows::core::Result<Uia> {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
                .or_else(|_| CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER))?;
            if let Ok(a2) = automation.cast::<IUIAutomation2>() {
                let _ = a2.SetConnectionTimeout(UIA_TIMEOUT_MS);
                let _ = a2.SetTransactionTimeout(UIA_TIMEOUT_MS);
            }
            // Chromium omnibox (Chrome, Edge, Brave, ...) or Firefox URL bar.
            let omnibox = automation
                .CreatePropertyCondition(UIA_ClassNamePropertyId, &VARIANT::from(BSTR::from("OmniboxViewViews")))?;
            let urlbar =
                automation.CreatePropertyCondition(UIA_AutomationIdPropertyId, &VARIANT::from(BSTR::from("urlbar-input")))?;
            let address_bar = automation.CreateOrCondition(&omnibox, &urlbar)?;
            let root_view = automation
                .CreatePropertyCondition(UIA_ClassNamePropertyId, &VARIANT::from(BSTR::from("BrowserRootView")))?;
            Ok(Uia { automation, address_bar, root_view })
        }

        fn bstr_of(v: &VARIANT) -> Option<String> {
            unsafe {
                if v.vt() == VT_BSTR {
                    Some(v.Anonymous.Anonymous.Anonymous.bstrVal.to_string())
                } else {
                    None
                }
            }
        }

        unsafe fn read(uia: &Uia, hwnd: isize) -> Reading {
            let mut reading = Reading::default();
            let Ok(window) = uia.automation.ElementFromHandle(HWND(hwnd as *mut _)) else { return reading };
            if let Ok(root) = window.FindFirst(TreeScope_Children, &uia.root_view) {
                reading.root_name = root.CurrentName().ok().map(|b| b.to_string());
            }
            // The toolbar precedes web content in tree order, so this stops early (~5 ms).
            if let Ok(bar) = window.FindFirst(TreeScope_Descendants, &uia.address_bar) {
                reading.address = bar.GetCurrentPropertyValue(UIA_ValueValuePropertyId).ok().as_ref().and_then(bstr_of);
            }
            reading
        }

        pub(super) fn run(rx: Receiver<Request>) {
            let uia = match unsafe { init() } {
                Ok(u) => u,
                Err(e) => {
                    log::warn!("context: UI Automation unavailable: {e}");
                    return; // senders see a closed channel; lookups resolve to None
                }
            };
            for req in rx {
                if Instant::now() >= req.deadline {
                    continue; // stale (worker was busy); the caller has moved on
                }
                let started = Instant::now();
                let reading = unsafe { read(&uia, req.hwnd) };
                log::debug!("context: uia read in {}ms", started.elapsed().as_millis());
                let _ = req.reply.try_send(reading);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{AppContext, Kind, Reading};
    use std::sync::mpsc::Receiver;
    use std::time::Instant;
    pub(super) fn probe() -> (AppContext, Kind, isize) {
        (AppContext::default(), Kind::Normal, 0)
    }
    pub(super) fn prewarm() {}
    pub(super) fn request_reading(_hwnd: isize, _deadline: Instant) -> Option<Receiver<Reading>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_and_unknown() {
        assert_eq!(friendly_app_name(r"C:\Program Files\Google\Chrome\Application\chrome.exe", None), "Google Chrome");
        assert_eq!(friendly_app_name(r"C:\x\SomeTool.exe", None), "SomeTool");
        assert_eq!(friendly_app_name(r"C:\x\SomeTool.EXE", Some("Some Tool Pro")), "Some Tool Pro");
        assert_eq!(friendly_app_name(r"C:\x\WindowsTerminal.exe", Some("Windows Terminal Host")), "Windows Terminal");
        assert_eq!(friendly_app_name(r"C:\x\olk.exe", Some("Microsoft Outlook")), "Outlook");
        assert_eq!(friendly_app_name(r"C:\x\T3 Code (Nightly).exe", Some("T3 Code (Nightly)")), "T3 Code (Nightly)");
        assert_eq!(friendly_app_name(r"C:\x\codex.exe", Some("")), "codex");
        assert_eq!(friendly_app_name(r"C:\x\foo.exe", Some("Electron")), "foo");
        assert_eq!(friendly_app_name(r"C:\x\foo.exe", Some("foo.exe")), "foo");
        assert_eq!(friendly_app_name("", None), "");
    }

    #[test]
    fn domains() {
        let d = |s| domain_from_address(s);
        assert_eq!(d("github.com/k2-fsa/sherpa-onnx/releases?q=1.12").as_deref(), Some("github.com"));
        assert_eq!(d("https://www.bbc.com/technology").as_deref(), Some("bbc.com"));
        assert_eq!(d("en.wikipedia.org/wiki/Speech_recognition").as_deref(), Some("en.wikipedia.org"));
        assert_eq!(d("https://user:pw@mail.google.com:443/mail/u/0/#inbox").as_deref(), Some("mail.google.com"));
        assert_eq!(d("HTTP://Docs.Google.COM./x").as_deref(), Some("docs.google.com"));
        assert_eq!(d("localhost:5173/settings").as_deref(), Some("localhost"));
        assert_eq!(d("192.168.1.1/admin").as_deref(), Some("192.168.1.1"));
        assert_eq!(d("acme.slack.com").as_deref(), Some("acme.slack.com"));
        assert_eq!(d(""), None);
        assert_eq!(d("how to cook rice"), None);
        assert_eq!(d("rice"), None);
        assert_eq!(d("chrome://settings"), None);
        assert_eq!(d("edge://newtab"), None);
        assert_eq!(d("about:blank"), None);
        assert_eq!(d("file:///C:/Users/me/doc.pdf"), None);
        assert_eq!(d("chrome-extension://abc/popup.html"), None);
        assert_eq!(d("mailto:a@b.com"), None);
        assert_eq!(d("https://[::1]:8080/"), None);
    }

    #[test]
    fn titles() {
        assert_eq!(clean_title("1.12 · Releases · k2-fsa/sherpa-onnx - Google Chrome", "Google Chrome"), "1.12 · Releases · k2-fsa/sherpa-onnx");
        assert_eq!(
            clean_title("BBC Technology | AI and 1 more page - Personal - Microsoft\u{200B} Edge", "Microsoft Edge"),
            "BBC Technology | AI"
        );
        assert_eq!(clean_title("Example Domain - Microsoft Edge", "Microsoft Edge"), "Example Domain");
        assert_eq!(clean_title("context.rs - Chirp - Visual Studio Code", "Visual Studio Code"), "context.rs - Chirp");
        assert_eq!(clean_title("Inbox — Mozilla Firefox", "Firefox"), "Inbox");
        assert_eq!(clean_title("Slack", "Slack"), "Slack");
        assert_eq!(clean_title("RE: Q3 invoice - Outlook", "Outlook"), "RE: Q3 invoice");
    }

    #[test]
    fn privacy() {
        assert!(title_marks_private("Example Domain - [InPrivate] - Microsoft\u{200B} Edge"));
        assert!(title_marks_private("Inbox — Mozilla Firefox Private Browsing"));
        assert!(!title_marks_private("Example Domain - Google Chrome"));
        assert!(root_name_marks_private("Example Domain - Google Chrome (Incognito)"));
        assert!(root_name_marks_private("Example Domain - Microsoft Edge (InPrivate)"));
        assert!(root_name_marks_private("Beispiel - Google Chrome (Inkognito)"));
        assert!(!root_name_marks_private("Foo (2024) - Google Chrome"));

        let (ctx, kind) = build_context(r"C:\x\1Password.exe", Some("1Password"), "GitHub login – Personal vault");
        assert_eq!((ctx.app.as_str(), ctx.title.as_str(), kind), ("1Password", "", Kind::PasswordManager));
        let (ctx, _) = build_context(r"C:\x\KeePassXC.exe", None, "secrets.kdbx - KeePassXC");
        assert_eq!(ctx.title, "");
        let (ctx, kind) = build_context(r"C:\x\msedge.exe", None, "Bank - [InPrivate] - Microsoft Edge");
        assert_eq!((ctx.title.as_str(), kind), ("", Kind::Browser(Browser::Chromium)));
    }

    /// Live check against whatever browser windows are open:
    /// `cargo test --lib context::tests::live -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn live_browser_windows() {
        use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, IsWindowVisible, GW_OWNER};
        unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
            if IsWindowVisible(h) != 0 && GetWindow(h, GW_OWNER).is_null() {
                (*(l as *mut Vec<isize>)).push(h as isize);
            }
            1
        }
        let mut wins: Vec<isize> = Vec::new();
        unsafe { EnumWindows(Some(cb), &mut wins as *mut Vec<isize> as LPARAM) };
        prewarm();
        std::thread::sleep(Duration::from_millis(100));
        for h in wins {
            let t0 = Instant::now();
            let probed = imp::probe_window(h);
            if !matches!(probed.1, Kind::Browser(_)) || probed.0.title.is_empty() {
                if !probed.0.title.is_empty() {
                    println!("{:?} in {:?}", probed.0, t0.elapsed());
                }
                continue;
            }
            let mut times = Vec::new();
            let mut last = AppContext::default();
            for _ in 0..20 {
                let s = Instant::now();
                let p = capture_from(s, imp::probe_window(h));
                // Wait for the worker explicitly to time the full lookup.
                let lookup = p.lookup.as_ref().expect("browser lookup");
                let reading = lookup.rx.recv_timeout(URL_BUDGET).ok();
                times.push(s.elapsed().as_secs_f64() * 1000.0);
                let mut ctx = p.context.clone();
                apply_reading(&mut ctx, lookup.browser, reading);
                last = ctx;
            }
            let first = times[0];
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("{last:?}\n    capture+uia: first {first:.1}ms p50 {:.1}ms max {:.1}ms", times[10], times[19]);
        }
    }

    #[test]
    fn readings() {
        let base = || AppContext { app: "Google Chrome".into(), title: "Example Domain".into(), url: None };

        let mut c = base();
        apply_reading(&mut c, Browser::Chromium, Some(Reading {
            address: Some("example.com/secret/path".into()),
            root_name: Some("Example Domain - Google Chrome (Incognito)".into()),
        }));
        assert_eq!((c.title.as_str(), c.url.as_deref()), ("", None));

        let mut c = base();
        apply_reading(&mut c, Browser::Chromium, Some(Reading {
            address: Some("github.com/k2-fsa/sherpa-onnx/pull/42".into()),
            root_name: Some("Pull request #42 - Google Chrome".into()),
        }));
        assert_eq!((c.title.as_str(), c.url.as_deref()), ("Pull request #42", Some("github.com")));

        let mut c = base();
        apply_reading(&mut c, Browser::Chromium, None);
        assert_eq!((c.title.as_str(), c.url.as_deref()), ("", None));

        let mut c = AppContext { app: "Firefox".into(), title: "Inbox".into(), url: None };
        apply_reading(&mut c, Browser::Firefox, None);
        assert_eq!(c.title, "Inbox");
    }
}
