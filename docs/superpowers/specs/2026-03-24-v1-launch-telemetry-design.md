# Chirp v1.0 Launch Pass — Design Spec

## Context

Chirp is ready to distribute to initial testers. This pass adds four user-facing features (telemetry, crash reporting, announcements, feedback), fixes launch-blocking bugs, fixes the icon inconsistency, and fixes the auto-launch startup issue. Everything in one pass on a new branch.

---

## 1. Usage Telemetry (Aptabase)

**SDK:** `tauri-plugin-aptabase` (verify exact crate version compatible with Tauri 2.10.3 before install). First-party Tauri v2 plugin. 20K events/mo free.

**Integration:** Register as Tauri plugin in `lib.rs` builder chain. Conditionally initialized based on `help_improve` setting.

**Events tracked (all anonymous, no content):**

| Event | Properties | Tracked from |
|-------|-----------|-------------|
| `app_started` | `version`, `os`, `model_loaded` | Rust (`lib.rs` setup) |
| `dictation_completed` | `duration_seconds`, `word_count`, `used_ai_cleanup`, `used_dictionary` | Rust (`commands.rs` stop_recording) |
| `dictation_cancelled` | `duration_seconds` | Rust (`commands.rs` cancel_recording) |
| `feature_used` | `feature` name (e.g. "dictionary_add", "snippet_add", "history_copy") | JS (`@aptabase/tauri` in React components) |
| `model_downloaded` | `model_name`, `duration_seconds` | Rust (`commands.rs` download_model) |
| `onboarding_completed` | `steps_completed` | JS (onboarding component) |

**Never sent:** Audio, transcription text, dictionary entries, snippet content, clipboard content.

**Offline:** Aptabase SDK batches locally, sends when connectivity returns.

### Files to modify
- `src-tauri/Cargo.toml` — add `tauri-plugin-aptabase`
- `package.json` — add `@aptabase/tauri`
- `src-tauri/src/lib.rs` — register plugin in builder, conditional on `help_improve`
- `src-tauri/src/commands.rs` — add `track_event()` calls at transcription completion, cancel, model download
- `src/components/settings/HomePage.tsx` — JS-side `trackEvent` for history_copy, feature interactions
- `src/components/settings/DictionaryPage.tsx` — JS-side `trackEvent` for dictionary_add
- `src/components/settings/SnippetsPage.tsx` — JS-side `trackEvent` for snippet_add
- `src/components/onboarding/Onboarding.tsx` — JS-side `trackEvent` for onboarding_completed

---

## 2. Crash Reporting (Sentry)

**SDK:** `sentry` crate + `tauri-plugin-sentry` — captures Rust panics, JS errors, and native crash minidumps.

**Integration:** Must initialize before `tauri::Builder` to catch early panics. Auto-injects `@sentry/browser` into webview.

**Panic mode:** `Cargo.toml` sets `panic = "abort"` in release profile. Sentry's panic hook may not flush events before abort. Two options:
- Switch release profile to `panic = "unwind"` (slightly larger binary, but Sentry captures rich Rust panic context)
- Keep `abort` and rely on native crashpad minidumps only (loses structured Rust panic info)
**Decision: switch to `panic = "unwind"` for release.** The binary size increase is negligible and crash context is far more valuable.

**Guard lifetime:** `sentry::init()` returns a `ClientInitGuard`. Store as a local variable in the `run()` function in `lib.rs` so it lives for the entire app lifetime. When the guard drops on exit, it flushes pending events.

**Privacy:**
- `before_send` hook strips any string values that could contain user content
- **Critical:** `commands.rs` line 446 logs transcription text via `log::info!`. Sentry captures log breadcrumbs. Must filter breadcrumbs from log messages containing transcription content, OR suppress content-containing log lines when Sentry is active. **Approach: add a `before_breadcrumb` hook that drops breadcrumbs whose message matches transcription log patterns (e.g., "After regex", "Raw transcript", "After AI cleanup").**

**Toggle:** Gated on `help_improve`. When off, Sentry SDK is never initialized. Toggle changes require app restart — show note in UI.

**CSP:** The `@sentry/browser` JS SDK sends events from the webview, so CSP needs `*.ingest.sentry.io` added to `connect-src`. (Rust-side reqwest is not subject to CSP.)

### Files to modify
- `src-tauri/Cargo.toml` — add `sentry`, `tauri-plugin-sentry`; change release `panic = "unwind"`
- `package.json` — add `@sentry/browser`
- `src-tauri/src/lib.rs` — init Sentry before Builder (guard as local var), conditional on `help_improve`; add `before_send` + `before_breadcrumb` hooks
- `src-tauri/tauri.conf.json` — add `*.ingest.sentry.io` to CSP `connect-src`

---

## 3. In-App Announcements

**Source:** JSON file hosted on GitHub raw URL (e.g., `https://raw.githubusercontent.com/sitelift/chirp-meta/main/announcements.json`).

**JSON format (simplified):**
```json
[
  {
    "id": "2026-03-24-welcome",
    "title": "Welcome to Chirp!",
    "body": "Short message here.",
    "min_version": "1.0.0",
    "max_version": null
  }
]
```

**Version filtering:** Add the `semver` crate for proper version comparison on `min_version`/`max_version` fields.

**Backend flow:**
1. New command `get_announcements()` fetches JSON via reqwest (Rust-side, no CSP change needed)
2. Caches to `%APPDATA%/com.chirp.app/announcements_cache.json`
3. On fetch failure → serve cache. No cache → empty array.
4. Filters by app version (semver) and `announcements_seen` list

**Frontend:** Dismissable banner at top of HomePage (settings window only). Fetched once on settings window load. Dismissed IDs persisted to `announcements_seen.json` in app data dir.

**Cross-window note:** Only the settings window shows announcements. The overlay window does not. No cross-window sync needed for `announcements_seen`.

**Offline:** Fails silently, serves cached version.

### Files to modify/create
- `src-tauri/Cargo.toml` — add `semver` crate
- `src-tauri/src/commands.rs` — new `get_announcements`, `dismiss_announcement` commands
- `src-tauri/src/announcements.rs` — new module for fetch/cache/filter logic
- `src-tauri/src/lib.rs` — register new commands, add `mod announcements`
- `src/components/settings/HomePage.tsx` — announcement banner component

---

## 4. In-App Feedback

**Destination:** Discord webhook URL (embedded as compile-time constant or env var in Rust).

**Backend:** New command `send_feedback(text: String)`:
1. Validate: non-empty, cap at 2000 chars
2. **Rate limit:** In-memory cooldown of 5 minutes between submissions (prevents spam)
3. Collect anonymous context: app version, OS, total dictation count, days since install
4. POST to Discord webhook as rich embed via reqwest (Rust-side, no CSP change needed)
5. Return success/failure

**Frontend:** "Send Feedback" button in Settings → expands inline textarea. Submit disabled while empty. Shows "Sent!" on success, error message on failure. Disabled for 5 min after successful send.

**Discord embed:** Formatted with feedback text, app version, OS, usage stats. No PII.

**Security:** Webhook URL is write-only. Worst case if extracted: spam to your channel (rotate URL to fix).

### Files to modify/create
- `src-tauri/src/commands.rs` — new `send_feedback` command
- `src-tauri/src/feedback.rs` — new module for webhook POST + rate limiting logic
- `src-tauri/src/lib.rs` — register new command, add `mod feedback`
- `src/components/settings/SettingsPage.tsx` — feedback textarea in new "Privacy & Feedback" section

---

## 5. Settings & Opt-In UX

**New setting:** `help_improve: bool` (default `false`) in `Settings` struct.

**Onboarding:** New 5th step between SmartCleanup and completion. Toggle (default off) with copy explaining what's collected and what's not. **Copy needs human writing — placeholder only in implementation.**

**Settings page:** New "Privacy & Feedback" section at bottom of SettingsPage:
- "Help improve Chirp" toggle (with note: "Changes take effect on restart")
- "Send Feedback" button → inline expanding textarea

### Files to modify
- `src-tauri/src/state.rs` — add `help_improve: bool` to Settings struct + Default
- `src/stores/appStore.ts` — add `helpImprove` field
- `src/lib/constants.ts` — add `helpImprove: false` to DEFAULT_SETTINGS
- `src/hooks/useSettingsSync.ts` — add `helpImprove` to SYNCED_KEYS
- `src/components/onboarding/Onboarding.tsx` — bump STEPS to 5, add new step
- `src/components/onboarding/HelpImprove.tsx` — new component (5th onboarding step)
- `src/components/settings/SettingsPage.tsx` — new Privacy & Feedback section (toggle + feedback button + textarea)

---

## 6. Bug Fixes (from Audit)

### 6a. Max recording duration — VERIFY ONLY
Already implemented at `commands.rs` lines 252-269 with a generation-based timeout. **Verify it works correctly, do not re-implement.**

### 6b. Clear history confirmation — VERIFY ONLY
Already implemented at `HomePage.tsx` line 149. **Verify it works correctly, do not re-implement.**

### 6c. Verify zombie audio callbacks (H3/H4)
- Files: `src-tauri/src/commands.rs` — `cancel_recording` and `test_microphone`
- Code review + verify cpal stream is properly dropped and callbacks deactivated
- Fix if needed

### 6d. Verify stacking mic test intervals (H6)
- File: `src/components/settings/SettingsPage.tsx`
- Verify interval is cleared before starting new one
- Fix if needed

### 6e. Verify hotkey reliability (H1)
- Action: Manual stress test after all changes applied
- 30 min rapid hotkey usage → confirm no stuck states

---

## 7. App Icon Consistency

**Problem:** Taskbar icon (`icon.ico`, `32x32.png`, etc.) is yellow bird on yellow background — blends together and is nearly invisible at small sizes. System tray icon (`tray-icon.png`) is just the yellow bird on transparent background and looks correct.

**Fix:** Regenerate all app icon files to use the yellow bird on transparent or dark background (matching the tray icon aesthetic). All contexts should show the same recognizable bird:
- Taskbar icon (Windows)
- Window title bar icon
- Alt-Tab switcher
- System tray (already correct)
- Overlay (already correct via BirdMark component)

### Files to modify
- `src-tauri/icons/icon.ico` — regenerate with new design
- `src-tauri/icons/icon.png` — regenerate (512x512)
- `src-tauri/icons/32x32.png` — regenerate
- `src-tauri/icons/128x128.png` — regenerate
- `src-tauri/icons/128x128@2x.png` — regenerate
- `src-tauri/icons/icon.icns` — regenerate (macOS)

**Note:** Icon design/generation is a manual step — need to create the icon asset first, then replace files.

---

## 8. Startup/Auto-Launch Fix

**Problem 1: No single-instance lock.** If app auto-launches at startup and user clicks to open it again, two instances run simultaneously competing for the global hotkey.

**Fix:** Add `tauri-plugin-single-instance` — blocks second instance, focuses the existing settings window instead. Check if Tauri v2 requires a `plugins` entry in `tauri.conf.json` for permissions.

**Problem 2: Settings window opens visible and maximized on auto-launch.** When system starts and app auto-launches, the big settings window pops up in your face.

**Fix:** Use `--minimized` flag approach:
1. Change `tauri-plugin-autostart::init()` args from `None` to `Some(vec!["--minimized"])`
2. Change `tauri.conf.json` settings window to `"visible": false` by default
3. In `lib.rs` setup, check `std::env::args()` for `--minimized` flag
4. If `--minimized`: leave window hidden, user opens via tray icon
5. If no flag (manual launch): show window and maximize programmatically

### Files to modify
- `src-tauri/Cargo.toml` — add `tauri-plugin-single-instance`
- `src-tauri/src/lib.rs` — register single-instance plugin, handle focus-existing-window; `--minimized` flag detection; programmatic show/maximize
- `src-tauri/tauri.conf.json` — settings window `"visible": false`; add single-instance plugin config if needed

---

## Implementation Order

1. **Settings & state** — add `help_improve` field, update store, constants, sync
2. **Aptabase** — install plugin, conditional init, add Rust + JS event tracking
3. **Sentry** — install plugin, conditional init, guard lifetime, before_send + before_breadcrumb hooks; change `panic = "unwind"`
4. **Announcements** — new module, commands, semver filtering, frontend banner
5. **Feedback** — new module, command, rate limiting, Discord webhook POST
6. **Onboarding step** — new HelpImprove component, bump steps to 5
7. **Settings UI** — Privacy & Feedback section with toggle + feedback textarea
8. **Bug fixes** — verify 6a/6b already work, review H3/H4/H6, fix if needed
9. **Icon** — regenerate icon files (manual asset creation + file replacement)
10. **Startup fix** — single-instance plugin, `--minimized` flag, conditional window visibility

---

## Verification

1. **Telemetry:** Enable toggle → use app → check Aptabase dashboard for events → disable → restart → verify no events sent
2. **Crash reporting:** Enable toggle → restart → trigger JS error → check Sentry dashboard → disable toggle → restart → verify Sentry not loaded
3. **Breadcrumb scrubbing:** Enable Sentry → dictate something → check Sentry breadcrumbs contain no transcription text
4. **Announcements:** Add entry to JSON file → launch app → banner appears on HomePage → dismiss → banner doesn't return → go offline → cached version shows
5. **Feedback:** Type feedback → send → verify Discord webhook receives embed → try again within 5 min → rate limited → go offline → graceful failure message
6. **Opt-in:** Fresh install → onboarding shows help-improve step → default off → toggle on → setting persists across restart
7. **Recording cap (verify):** Confirm existing 10-min auto-stop works correctly
8. **Clear history (verify):** Confirm existing confirmation dialog works correctly
9. **Single instance:** Launch app → try to launch again → existing window focuses, no second instance
10. **Auto-launch:** Reboot → app starts minimized to tray → click tray → window appears
11. **Manual launch:** Double-click app → window appears visible and maximized (not hidden)
12. **Icon:** Check taskbar, alt-tab, tray, overlay — all show consistent yellow bird
