# Chirp Codebase Audit Report

**Date:** 2026-03-21
**Scope:** Full codebase — every Rust, TypeScript, config, and build file
**Method:** Manual line-by-line review of all source files
**Status:** All issues below have been **fixed** except L7 (async listener cleanup — low severity, high risk of regression from touching 8+ files, and harmless in practice since React handles late setState gracefully). M8 (binary hash verification) has infrastructure added but requires the actual SHA-256 hash of the current llama.cpp release to complete. M2 (settings key validation) was found to be a non-issue upon deeper analysis — serde already drops unknown keys during deserialization, so they never persist.

---

## Critical Issues

### C1. LLM child process not killed on app exit

**File:** `src-tauri/src/lib.rs:229-263`
**Severity:** Critical

The app calls `app.exit(0)` from the tray menu (line 230) and uses `tauri::generate_context!()` to run, but there is no shutdown hook that kills the `llm_process` stored in `AppState`. When the app exits, the llama-server process becomes an orphan consuming GPU memory and a network port.

**Fix:** Add a shutdown handler in `setup()` that kills the LLM process:

```rust
// After building the tray, before Ok(())
let state_for_exit = handle.state::<SharedState>().inner().clone();
app.on_exit(move |_app| {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut s = state_for_exit.lock().await;
        if let Some(ref mut child) = s.llm_process {
            let _ = child.kill().await;
            let _ = child.wait().await;
            log::info!("Killed llama-server on app exit");
        }
    });
});
```

---

### C2. Stale llama-server process on restart

**File:** `src-tauri/src/lib.rs:149-186`
**Severity:** Critical

If the app crashes while the LLM server is running, restarting the app auto-starts a *new* llama-server without checking if a previous instance is still alive on some port. This leaves a zombie process consuming GPU/RAM indefinitely.

**Fix:** Before starting a new server, check for and kill existing llama-server processes:

```rust
// In the auto-start block, before llm::start_server(port):
#[cfg(windows)]
{
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "llama-server.exe"])
        .output();
}
#[cfg(unix)]
{
    let _ = std::process::Command::new("pkill")
        .arg("-f")
        .arg("llama-server")
        .output();
}
```

---

### C3. Division by zero in `get_input_level` with empty buffer

**File:** `src-tauri/src/commands.rs:340`
**Severity:** Critical

In `stop_recording`, the RMS calculation at line 340 divides by `audio_data.len()` which could be zero if the buffer is empty. The `audio_data.is_empty()` check at line 354 comes *after* the division.

```rust
// Line 340 — divides by len BEFORE checking is_empty at line 354
let rms = (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32).sqrt();
```

**Fix:** Move the empty check before the RMS calculation:

```rust
if audio_data.is_empty() {
    log::error!("Audio buffer is empty!");
    let mut s = state.lock().await;
    s.recording_state = RecordingState::Idle;
    return Err("transcription_failed".into());
}

// Now safe to compute RMS
let rms = (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32).sqrt();
```

---

## High Issues

### H1. Hotkey press/release events silently dropped under lock contention (Windows)

**File:** `src-tauri/src/hotkey_windows.rs:112-130`
**Severity:** High

The Windows keyboard hook callback uses `try_lock()` on `APP_HANDLE`. If the mutex is held (e.g., during `start_key_listener` or `stop_key_listener`), the hotkey event is silently dropped with only a log warning. This means a recording can fail to start or — worse — fail to stop, leaving the user stuck in recording state.

```rust
if let Ok(guard) = APP_HANDLE.try_lock() {
    // emit event
} else {
    log::warn!("APP_HANDLE lock contention in hook callback — press event dropped");
}
```

**Fix:** Replace the `Mutex<Option<AppHandle>>` with an `std::sync::OnceLock<AppHandle>` since the handle is set once and read many times. This eliminates lock contention entirely:

```rust
static APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

// In start_key_listener:
let _ = APP_HANDLE.set(app.clone());

// In hook callback:
if let Some(app) = APP_HANDLE.get() {
    let _ = app.emit("hotkey-pressed", ());
}
```

---

### H2. `Box::leak` memory leak in macOS hotkey listener

**File:** `src-tauri/src/hotkey.rs:252, 433`
**Severity:** High

`TapState` and `CaptureState` are allocated with `Box::leak()` to pass across the FFI boundary but are never freed. Each call to `start_key_listener` or `capture_next_key` leaks ~200 bytes. While the listener is typically started once, `capture_next_key` can be called multiple times during hotkey configuration.

**Fix for `capture_next_key` (line 433):** Convert the leaked box back to a `Box` after use and drop it:

```rust
// After CFRunLoopRun() returns:
let code = state.captured_keycode.load(Ordering::SeqCst);
let name = if code >= 0 { keycode_name(code) } else { "Cancelled".into() };

// Reclaim the leaked box
let _ = unsafe { Box::from_raw(state as *const CaptureState as *mut CaptureState) };

(code, name)
```

For `start_key_listener` (line 252), the `TapState` lives for the app's lifetime so the leak is acceptable, but should be documented with a comment.

---

### H3. `cancel_recording` doesn't deactivate zombie audio callbacks

**File:** `src-tauri/src/commands.rs:523-543`
**Severity:** High

`stop_recording` correctly deactivates the `StreamActiveFlag` before dropping the stream (line 274-279), but `cancel_recording` does not. On macOS, cpal/CoreAudio callbacks can outlive the stream drop, causing zombie callbacks to write to the buffer after it's been cleared.

**Fix:** Add the same deactivation logic to `cancel_recording`:

```rust
#[tauri::command]
pub async fn cancel_recording(
    state: State<'_, SharedState>,
    buffer: State<'_, AudioBuffer>,
    stream_handle: State<'_, StreamHandle>,
    stream_active_state: State<'_, StreamActiveState>,  // Add this parameter
) -> Result<(), String> {
    // Deactivate zombie callbacks first
    {
        let active = stream_active_state.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref flag) = *active {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    // Then stop stream and clear buffer
    // ...existing code...
}
```

---

### H4. Mic test doesn't deactivate stream active flag or flush resampler

**File:** `src-tauri/src/commands.rs:600-636`
**Severity:** High

`test_microphone` starts an audio capture but doesn't store or use the `active_flag` or `resampler_state` returned by `start_capture`. The active flag is never set to false, so zombie callbacks can continue after the test. The resampler is never flushed, so the last ~10ms of audio is lost (minor for a test but indicates inconsistency).

**Fix:** Store and deactivate the active flag:

```rust
let (stream, _error_flag, active_flag, _resampler_state) = audio::start_capture(&device_id, buffer.inner().clone(), app_handle)?;
*stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(StreamWrapper(stream));

tokio::time::sleep(std::time::Duration::from_secs(3)).await;

// Deactivate before dropping
active_flag.store(false, std::sync::atomic::Ordering::SeqCst);
*stream_handle.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
```

---

### H5. Unhandled clipboard write rejection

**File:** `src/components/settings/HomePage.tsx:138-142`
**Severity:** High

`navigator.clipboard.writeText()` can reject if the page doesn't have focus or clipboard permission is denied. The rejection is unhandled, causing an unhandled promise rejection.

```typescript
const handleCopy = async (text: string, timestamp: string) => {
    await navigator.clipboard.writeText(text)  // No .catch()
    setCopiedTimestamp(timestamp)
```

**Fix:**

```typescript
const handleCopy = async (text: string, timestamp: string) => {
    try {
        await navigator.clipboard.writeText(text)
        setCopiedTimestamp(timestamp)
        setTimeout(() => setCopiedTimestamp(null), 1500)
    } catch {
        // Clipboard access denied — silently fail
    }
}
```

---

### H6. Multiple mic test intervals can stack

**File:** `src/components/settings/SettingsPage.tsx:133-171`
**Severity:** High

If the user clicks "Test mic" rapidly, `handleTest` creates a new `setInterval` each time without clearing the previous one. The button has `disabled={testState !== 'idle'}` but there's a brief window between the click and `setTestState('recording')` where a double-click can slip through.

**Fix:** Track the interval in a ref and clear it before starting a new one:

```typescript
const testIntervalRef = useRef<number | null>(null)

const handleTest = async () => {
    if (testIntervalRef.current) clearInterval(testIntervalRef.current)
    setTestState('recording')
    setTestCountdown(3)
    testIntervalRef.current = setInterval(() => {
        // ...existing code...
    }, 1000)
    // ...rest of handler...
}
```

---

## Medium Issues

### M1. Clipboard restore race condition — 2-second fixed delay

**File:** `src-tauri/src/inject.rs:97-111`
**Severity:** Medium

After simulating Ctrl+V, the clipboard is restored after a hardcoded 2-second delay. If the target app takes >2s to read the clipboard (e.g., under heavy load), it reads the *restored* (old) content. Conversely, the user's own clipboard operations within 2s are silently overwritten.

**Fix:** Increase to 3s and document the trade-off, or use a platform-specific approach to detect when the paste is consumed. As a pragmatic fix:

```rust
thread::sleep(Duration::from_secs(3));
```

---

### M2. Settings merge accepts arbitrary keys without validation

**File:** `src-tauri/src/commands.rs:59-63`
**Severity:** Medium

The `update_settings` command merges any key from the `partial` JSON into the settings object. While serde deserialization validates known fields, unknown keys are silently inserted and then ignored during serialization (serde's `deny_unknown_fields` is not used). This clutters the persisted JSON file.

**Fix:** Add `#[serde(deny_unknown_fields)]` to the `Settings` struct, or validate keys against a whitelist before merging:

```rust
const ALLOWED_KEYS: &[&str] = &[
    "hotkey", "launchAtLogin", "playSoundOnComplete", "autoDismissOverlay",
    "smartFormatting", "inputDevice", "model", "onboardingComplete",
    "aiCleanup", "overlayPosition", "showPassiveOverlay", "toneMode",
    "historyRetentionDays", "modelDownloaded",
];

if let Some(patch) = partial.as_object() {
    for (k, v) in patch {
        if ALLOWED_KEYS.contains(&k.as_str()) {
            base.insert(k.clone(), v.clone());
        }
    }
}
```

---

### M3. No maximum recording duration

**File:** `src-tauri/src/commands.rs:177-250`
**Severity:** Medium

There is no cap on recording duration. A stuck hotkey or forgotten recording will accumulate audio samples indefinitely, consuming RAM at ~32 KB/s (16kHz * 2 bytes). After 10 minutes that's ~19 MB; after an hour, ~115 MB; after a day (if the user forgets), ~2.7 GB.

**Fix:** Add a maximum recording duration (e.g., 10 minutes) with automatic stop:

```rust
// In start_recording, after setting recording state:
let app_clone = app_handle.clone();
let state_clone = state.inner().clone();
tauri::async_runtime::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_secs(600)).await;
    let s = state_clone.lock().await;
    if s.recording_state == RecordingState::Recording {
        drop(s);
        log::warn!("Recording auto-stopped after 10 minutes");
        let _ = app_clone.emit("hotkey-released", ());
    }
});
```

---

### M4. History append can exceed 1000-entry cap in memory

**File:** `src-tauri/src/commands.rs:491` + `src-tauri/src/history.rs:44`
**Severity:** Medium

History is capped at 1000 entries during `save_history` (line 44), but entries are appended to the in-memory `Vec` without checking (line 491). Between saves, the in-memory list can grow unbounded. Each entry is cloned to all windows via events.

**Fix:** Cap in memory before saving:

```rust
s.history.push(entry);
if s.history.len() > 1000 {
    s.history.drain(..s.history.len() - 1000);
}
let _ = history::save_history(&s.history);
```

---

### M5. Cross-window sync race in `useSettingsSync`

**File:** `src/hooks/useSettingsSync.ts:110-113`
**Severity:** Medium

When a `settings-changed` event arrives from another window, `suppressSync` is set to `true` to prevent echo. It's reset via `setTimeout(() => { suppressSync.current = false }, 0)`. But if a Zustand subscription fires synchronously during `updateSettings` (before the setTimeout callback runs), the sync is correctly suppressed. However, if *two* settings-changed events arrive in rapid succession (e.g., user toggling settings quickly in the other window), the second event's `setTimeout` can reset the flag while the first event's store update is still propagating, causing a brief sync echo.

**Fix:** Use a counter instead of a boolean:

```typescript
const suppressCount = useRef(0)

// On settings-changed:
suppressCount.current++
updateSettings(partial)
setTimeout(() => { suppressCount.current-- }, 0)

// In subscription:
if (suppressCount.current > 0) return
```

---

### M6. `handleDelete` doesn't handle backend failure

**File:** `src/components/settings/HomePage.tsx:144-147`
**Severity:** Medium

If `tauri.deleteHistoryEntry(timestamp)` fails, the local store is still updated via `store.removeHistoryEntry(timestamp)`, causing a divergence between frontend and backend.

```typescript
const handleDelete = async (timestamp: string) => {
    await tauri.deleteHistoryEntry(timestamp)  // Can throw
    store.removeHistoryEntry(timestamp)         // Runs even if above throws? No — await throws
}
```

Actually, if the `await` throws, `removeHistoryEntry` is skipped. But the error is completely unhandled — no user feedback.

**Fix:**

```typescript
const handleDelete = async (timestamp: string) => {
    try {
        await tauri.deleteHistoryEntry(timestamp)
        store.removeHistoryEntry(timestamp)
    } catch {
        // Silently fail — entry stays in UI
    }
}
```

---

### M7. LLM timeout in onboarding doesn't cancel backend process

**File:** `src/components/onboarding/SmartCleanup.tsx:65-76`
**Severity:** Medium

The onboarding SmartCleanup step races the LLM start against a 30-second timeout:

```typescript
const startPromise = tauri.startLlm()
const timeoutPromise = new Promise<never>((_, reject) =>
    setTimeout(() => reject(new Error('LLM start timed out')), 30000)
)
await Promise.race([startPromise, timeoutPromise])
```

If the timeout fires, the frontend shows an error, but the backend's `start_server` call continues running. The server may eventually start successfully, but the frontend doesn't know about it. If the user retries, a second server starts on a different port while the first one is still alive.

**Fix:** After timeout, call `stop_llm` to ensure cleanup:

```typescript
try {
    await Promise.race([startPromise, timeoutPromise])
} catch (e) {
    // Ensure backend process is killed on timeout
    try { await tauri.stopLlm() } catch { /* ignore */ }
    throw e
}
```

---

### M8. No download integrity verification for LLM binary

**File:** `src-tauri/src/llm.rs:185-255`
**Severity:** Medium

After downloading and extracting the llama-server binary, there's no checksum or signature verification. A network-level attacker (or CDN compromise) could substitute a malicious binary that gets executed with user privileges.

**Fix:** Add a SHA-256 checksum constant and verify after download:

```rust
const LLAMA_SERVER_SHA256: &str = "<expected hash>";

// After extraction, verify:
let hash = sha256_file(&binary_path())?;
if hash != LLAMA_SERVER_SHA256 {
    let _ = tokio::fs::remove_file(&binary_path()).await;
    return Err("Binary checksum mismatch — download may be corrupted".to_string());
}
```

---

### M9. `unsafe impl Send/Sync` for `StreamWrapper` is unsound

**File:** `src-tauri/src/commands.rs:24-27`
**Severity:** Medium

```rust
pub struct StreamWrapper(cpal::Stream);
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}
```

`cpal::Stream` is `!Send` because audio stream handles are thread-affine on some platforms. The wrapper claims it's safe to send across threads, but the stream is only ever accessed on the main thread (stored in a Mutex, only set/cleared in command handlers). This works in practice but is technically undefined behavior if Tauri ever dispatches commands on different threads.

**Fix:** Document the safety invariant with a comment:

```rust
// SAFETY: StreamWrapper is only accessed via Mutex in command handlers
// which are dispatched on Tauri's main thread. The stream is never
// actually sent across threads — only the Mutex guard crosses await points.
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}
```

---

### M10. `unsafe impl Send/Sync` for `SherpaRecognizer` undocumented

**File:** `src-tauri/src/state.rs:8-10`
**Severity:** Medium

```rust
pub struct SherpaRecognizer(pub OfflineRecognizer);
unsafe impl Send for SherpaRecognizer {}
unsafe impl Sync for SherpaRecognizer {}
```

The comment says "The C API is thread-safe" but there's no link to documentation proving this. If sherpa-onnx's `OfflineRecognizer` is not actually thread-safe, concurrent transcription calls would cause data races.

**Fix:** Add a reference and ensure single-threaded access:

```rust
// SAFETY: sherpa-onnx's OfflineRecognizer uses thread-safe C API internals.
// See: https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/c-api/c-api.h
// We additionally wrap in Arc and only call from spawn_blocking tasks.
```

---

### M11. `err_fn` closure moved into multiple stream builders

**File:** `src-tauri/src/audio.rs:147-150, 160-226`
**Severity:** Medium

The `err_fn` closure captures `stream_error_clone` by move and is used across three match arms (`F32`, `I16`, `U16`). This works because only one arm executes, but the pattern is fragile — if someone adds a format that also needs the error callback, the compiler won't catch the double-move because each arm is a separate closure. More importantly, the `I16` and `U16` arms clone `buffer_clone`, `resampler_clone`, and `resample_buf_clone` *again* inside the match, but they shadow the outer variables with no indication.

Actually, looking more carefully: the `buffer_clone`, `resampler_clone`, and `resample_buf_clone` are declared once (lines 140-142) and then moved into the first closure that captures them. The other arms reference the same variables, which works because only one arm executes. However, `err_fn` is moved into the first arm's `build_input_stream` call, making it unavailable for subsequent arms. This compiles because Rust evaluates only one match arm.

**Fix:** This is not a bug per se, but add a comment clarifying the pattern for future maintainers.

---

## Low Issues

### L1. Dummy regexes wasting memory in cleanup

**File:** `src-tauri/src/cleanup.rs:113-118`
**Severity:** Low

The `number_words` field stores `(Regex::new("^$").unwrap(), *digit)` tuples where the regex is never used for matching — the actual matching happens through `numeric_contexts`. These dummy regexes waste memory.

**Fix:** Change `number_words` to `Vec<&'static str>` (just the digit strings):

```rust
number_words: compiled_numbers,  // Vec<&'static str> instead of Vec<(Regex, &str)>
```

And update the accessor at line 219:

```rust
let digit = re.number_words[i];  // instead of re.number_words[i].1
```

---

### L2. `UnhookWindowsHookEx` return value ignored

**File:** `src-tauri/src/hotkey_windows.rs:246`
**Severity:** Low

```rust
win32::UnhookWindowsHookEx(hook);
```

The return value indicates success/failure but is ignored. A failed unhook means the hook callback continues running after the thread exits.

**Fix:**

```rust
if win32::UnhookWindowsHookEx(hook) == 0 {
    log::warn!("UnhookWindowsHookEx failed");
}
```

---

### L3. `debug!` log statements left in snippets.rs

**File:** `src-tauri/src/snippets.rs:8-11, 33, 37, 39, 45`
**Severity:** Low

Multiple `log::debug!` calls are left in `apply_snippets` that log every snippet trigger and expansion. While debug-level logs are filtered in production, they add unnecessary overhead in the hot path (called for every transcription).

**Fix:** Remove or gate behind a feature flag:

```rust
// Remove lines 8-11, 33, 37, 39, 45
```

---

### L4. Archive cleanup silently ignored

**File:** `src-tauri/src/llm.rs:251`, `src-tauri/src/transcribe.rs:206`
**Severity:** Low

```rust
let _ = tokio::fs::remove_file(&archive_path).await;
```

If the archive file can't be deleted (e.g., antivirus lock on Windows), it stays on disk consuming hundreds of MB. The failure is silently swallowed.

**Fix:** Log a warning:

```rust
if let Err(e) = tokio::fs::remove_file(&archive_path).await {
    log::warn!("Failed to clean up archive {}: {e}", archive_path.display());
}
```

---

### L5. No model file verification after extraction

**File:** `src-tauri/src/transcribe.rs:165-203`
**Severity:** Low

After extracting the tar.bz2 archive, there's no check that required files (`tokens.txt`, `encoder*.onnx`, `decoder*.onnx`, `joiner*.onnx`) were actually extracted. A truncated or corrupt archive could silently produce an incomplete model directory that fails later during `load_model`.

**Fix:** Add a post-extraction check:

```rust
// After extraction
let model_dir_path = model_dir(model);
if !model_dir_path.join("tokens.txt").exists() {
    return Err("Model extraction incomplete: tokens.txt missing".to_string());
}
```

---

### L6. `useTauri` hook creates new function objects every render

**File:** `src/hooks/useTauri.ts:35-205`
**Severity:** Low

`useTauri()` is a custom hook that returns an object of async functions. Since it's not memoized, every component that calls `useTauri()` gets new function references on every render. This doesn't cause bugs but means any `useEffect` with `tauri` as a dependency would re-run unnecessarily.

**Fix:** Memoize the return value:

```typescript
export function useTauri() {
    return useMemo(() => ({
        startRecording,
        // ...all functions...
    }), [])
}
```

Or even better, since none of the functions close over component state, export them as module-level constants instead of wrapping in a hook.

---

### L7. Async event listener cleanup pattern is racy

**File:** `src/components/settings/Settings.tsx:45-49`, `src/components/shared/AboutModal.tsx:51-55`, and throughout hooks

**Severity:** Low

Throughout the codebase, Tauri event listeners are registered and cleaned up with this pattern:

```typescript
useEffect(() => {
    const unlisten = listen('event', handler)
    return () => { unlisten.then((f) => f()) }
}, [])
```

`listen()` returns a Promise. If the component unmounts before the promise resolves, the cleanup function calls `.then()` on a pending promise — the unsubscribe happens asynchronously *after* unmount. Events can fire between unmount and unsubscribe. This is unlikely to cause crashes (React doesn't throw on setState after unmount anymore) but could cause unexpected behavior.

**Fix:** Use an abort pattern:

```typescript
useEffect(() => {
    let cancelled = false
    let unlistenFn: (() => void) | null = null
    listen('event', handler).then((fn) => {
        if (cancelled) fn()
        else unlistenFn = fn
    })
    return () => {
        cancelled = true
        unlistenFn?.()
    }
}, [])
```

---

### L8. `handleClearHistory` has no confirmation dialog

**File:** `src/components/settings/HomePage.tsx:165-168`
**Severity:** Low

Clicking "Clear all history" immediately deletes everything with no undo. This is a destructive action on potentially valuable transcription data.

**Fix:** Add a confirmation:

```typescript
const handleClearHistory = async () => {
    if (!window.confirm('Delete all transcription history? This cannot be undone.')) return
    await tauri.clearHistory()
    store.setHistory([])
}
```

---

### L9. Hardcoded version string in multiple places

**File:** `src/components/shared/AboutModal.tsx:90`, `src/components/settings/Settings.tsx:152`
**Severity:** Low

The version "v1.0.0" is hardcoded in the frontend UI. It should be read from `Cargo.toml` via `tauri.conf.json` or the Tauri runtime so it stays in sync with the actual build version.

**Fix:** Use `@tauri-apps/api/app` to get the version:

```typescript
import { getVersion } from '@tauri-apps/api/app'
const [version, setVersion] = useState('...')
useEffect(() => { getVersion().then(setVersion) }, [])
```

---

### L10. `showInMenuBar` setting is defined but never used

**File:** `src-tauri/src/state.rs:37`
**Severity:** Low

The `Settings` struct has a `show_in_menu_bar` field, but no code reads it. It's persisted to JSON and synced to the frontend but has no effect.

**Fix:** Either remove the field or implement the functionality. If removing:

```rust
// Remove from Settings struct
// Remove from default implementation
// Remove from frontend store
```

---

### L11. Duplicate `handleCleanupToggle` logic

**File:** `src/components/settings/HomePage.tsx:95-113` and `src/components/settings/SettingsPage.tsx:112-131`
**Severity:** Low

The LLM toggle handler is duplicated verbatim in both `HomePage` and `SettingsPage`. Any bug fix to one must be applied to both.

**Fix:** Extract to a shared hook:

```typescript
// src/hooks/useCleanupToggle.ts
export function useCleanupToggle() {
    const store = useAppStore()
    const tauri = useTauri()
    const [starting, setStarting] = useState(false)

    const toggle = async (enabled: boolean) => {
        store.updateSettings({ aiCleanup: enabled })
        // ...shared logic...
    }

    return { toggle, starting }
}
```

---

### L12. `noise_suppression` setting exists but is not used anywhere

**File:** `src-tauri/src/state.rs:42`
**Severity:** Low

The `noise_suppression` field in `Settings` is defined, serialized, and has a default (`true`), but no code in the audio pipeline reads or applies it. It's dead configuration.

**Fix:** Either implement noise suppression or remove the field to avoid user confusion.

---

## Summary

| Severity | Count |
|----------|-------|
| Critical | 3     |
| High     | 6     |
| Medium   | 11    |
| Low      | 12    |
| **Total**| **32**|

## Recommended Fix Order

1. **C1** — LLM process not killed on exit (orphan process, GPU leak)
2. **C2** — Stale llama-server on restart (resource accumulation)
3. **C3** — Division by zero in stop_recording (crash)
4. **H1** — Hotkey events silently dropped (stuck recording state)
5. **H3** — cancel_recording zombie callbacks (audio corruption)
6. **H4** — test_microphone zombie callbacks (same class of bug)
7. **H5** — Unhandled clipboard rejection (unhandled promise)
8. **H2** — macOS Box::leak (memory leak over time)
9. **H6** — Stacking mic test intervals (timer leak)
10. **M3** — No max recording duration (unbounded RAM)
11. **M4** — History exceeds 1000 cap in memory
12. **M7** — LLM timeout doesn't kill backend process
13. **M8** — No binary integrity verification
14. **M1** — Clipboard restore timing
15. **M2** — Settings accepts arbitrary keys
16. Remaining medium and low issues in any order
