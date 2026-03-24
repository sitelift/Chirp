# Chirp v1.0 Launch Pass — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add telemetry, crash reporting, announcements, and feedback to Chirp, plus fix startup/icon/bug issues for first-user distribution.

**Architecture:** Four new features integrate through existing Tauri plugin system (Aptabase, Sentry) and new Rust command handlers (announcements, feedback). All gated behind a single `help_improve` opt-in setting. A new onboarding step and Settings section provide the UX.

**Tech Stack:** Tauri 2, `tauri-plugin-aptabase`, `sentry` + `tauri-plugin-sentry`, `tauri-plugin-single-instance`, `semver` crate, `@aptabase/tauri` (JS), Discord webhooks, GitHub raw JSON

**Spec:** `docs/superpowers/specs/2026-03-24-v1-launch-telemetry-design.md` and `C:\Users\dutch\.claude\plans\frolicking-greeting-clarke.md`

---

## File Structure

### New files
| File | Purpose |
|------|---------|
| `src-tauri/src/announcements.rs` | Fetch, cache, filter announcements from GitHub |
| `src-tauri/src/feedback.rs` | Discord webhook POST with rate limiting |
| `src/components/onboarding/HelpImprove.tsx` | 5th onboarding step for opt-in toggle |
| `src/components/settings/AnnouncementBanner.tsx` | Dismissable banner for HomePage |

### Modified files
| File | Changes |
|------|---------|
| `src-tauri/Cargo.toml` | Add deps: `tauri-plugin-aptabase`, `sentry`, `tauri-plugin-sentry`, `tauri-plugin-single-instance`, `semver` ; change `panic = "unwind"` |
| `package.json` | Add deps: `@aptabase/tauri`, `@sentry/browser` |
| `src-tauri/src/state.rs` | Add `help_improve: bool` to `Settings` |
| `src-tauri/src/lib.rs` | Register new plugins + commands, Sentry init, `--minimized` detection, single-instance |
| `src-tauri/src/commands.rs` | New commands: `get_announcements`, `dismiss_announcement`, `send_feedback`; add Aptabase tracking calls |
| `src/stores/appStore.ts` | Add `helpImprove` field |
| `src/lib/constants.ts` | Add `helpImprove: false` to DEFAULT_SETTINGS |
| `src/hooks/useSettingsSync.ts` | Add `helpImprove` to SYNCED_KEYS |
| `src/components/onboarding/Onboarding.tsx` | Bump STEPS to 5, add HelpImprove step |
| `src/components/settings/SettingsPage.tsx` | Add "Privacy & Feedback" section |
| `src/components/settings/HomePage.tsx` | Add AnnouncementBanner, JS-side Aptabase tracking |
| `src-tauri/tauri.conf.json` | CSP update (Sentry), settings window `visible: false`, single-instance config |

---

## Task 1: Settings & State Foundation

**Files:**
- Modify: `src-tauri/src/state.rs:36-56` (Settings struct)
- Modify: `src-tauri/src/state.rs:70-88` (Default impl)
- Modify: `src/lib/constants.ts:1-15`
- Modify: `src/stores/appStore.ts:26-115` (interface) and `117-214` (store)
- Modify: `src/hooks/useSettingsSync.ts:9-18` (SYNCED_KEYS)

- [ ] **Step 1: Add `help_improve` to Rust Settings**

In `src-tauri/src/state.rs`, add to the `Settings` struct after line 55:

```rust
    #[serde(default)]
    pub help_improve: bool,
```

And add to `Default` impl after `history_retention_days: 0,` (line 85):

```rust
            help_improve: false,
```

- [ ] **Step 2: Add `helpImprove` to frontend constants**

In `src/lib/constants.ts`, add after `historyRetentionDays: 0,` (line 14):

```typescript
  helpImprove: false,
```

- [ ] **Step 3: Add `helpImprove` to Zustand store**

In `src/stores/appStore.ts`, add to `AppState` interface after `historyRetentionDays: number` (line 74):

```typescript
  // Telemetry
  helpImprove: boolean
```

Add to store initial values after `historyRetentionDays` (around line 165):

```typescript
  // Telemetry
  helpImprove: DEFAULT_SETTINGS.helpImprove,
```

- [ ] **Step 4: Add `helpImprove` to SYNCED_KEYS**

In `src/hooks/useSettingsSync.ts`, add `'helpImprove'` to the SYNCED_KEYS array (line 17):

```typescript
const SYNCED_KEYS = [
  'hotkey', 'launchAtLogin', 'playSoundOnComplete',
  'autoDismissOverlay', 'smartFormatting',
  'inputDevice', 'model', 'onboardingComplete',
  'aiCleanup',
  'toneMode',
  'overlayPosition',
  'showPassiveOverlay',
  'historyRetentionDays',
  'helpImprove',
] as const
```

- [ ] **Step 5: Verify — run `npm run build` to confirm TypeScript compiles**

Run: `npm run build`
Expected: Compiles without errors. The new field has defaults so existing settings.json files won't break.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/state.rs src/lib/constants.ts src/stores/appStore.ts src/hooks/useSettingsSync.ts
git commit -m "feat: add help_improve setting for telemetry opt-in"
```

---

## Task 2: Install Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`

- [ ] **Step 1: Verify Aptabase Tauri v2 crate version**

Run: `cargo search tauri-plugin-aptabase` to confirm the latest version and Tauri v2 compatibility.

- [ ] **Step 2: Add Rust dependencies to Cargo.toml**

Add to `[dependencies]` section:

```toml
tauri-plugin-aptabase = "2"
sentry = "0.37"
tauri-plugin-sentry = "0.5"
tauri-plugin-single-instance = "2"
semver = "1"
```

Note: Use `sentry = "0.37"` (the version compatible with `tauri-plugin-sentry` 0.5). Verify exact compatible versions at install time.

- [ ] **Step 3: Change release panic to unwind**

In `Cargo.toml` `[profile.release]` section, change:

```toml
panic = "unwind"
```

- [ ] **Step 4: Add JS dependencies**

Run:
```bash
npm install @aptabase/tauri @sentry/browser
```

- [ ] **Step 5: Verify — run `cargo check` in src-tauri**

Run: `cd src-tauri && cargo check`
Expected: Compiles (new plugins not yet registered, just added as deps).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json
git commit -m "feat: add telemetry, crash reporting, and single-instance deps"
```

---

## Task 3: Aptabase Telemetry Integration

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Register Aptabase plugin in lib.rs**

In `src-tauri/src/lib.rs`, add to the builder chain after the dialog plugin (line 70):

```rust
        .plugin({
            let s = initial_settings_for_aptabase; // need to clone settings before builder
            if s.help_improve {
                tauri_plugin_aptabase::Builder::new("A-YOUR-APP-KEY")
                    .with_panic_hook(Box::new(|client, info| {
                        client.track_event("app_crashed", Some(serde_json::json!({
                            "info": info.to_string().chars().take(500).collect::<String>(),
                        })));
                    }))
                    .build()
            } else {
                // Aptabase needs to be registered even when disabled (plugin system requires it)
                // but won't send events without initialization
                tauri_plugin_aptabase::Builder::new("")
                    .build()
            }
        })
```

**Important:** Before the builder chain, clone the `help_improve` value:

```rust
    let initial_help_improve = initial_settings.help_improve;
```

**API verification required:** The exact Aptabase Tauri v2 API may differ from the snippet above. Before writing code, check `https://github.com/aptabase/tauri-plugin-aptabase` for:
- How to conditionally enable/disable the plugin
- The correct event tracking method (may be `app.track_event()` extension trait, not `try_state`)
- Whether the plugin can be registered with an empty key safely

The tracking calls in Steps 2-4 below also depend on the actual API shape. Adjust all `track_event` calls to match the real API.

- [ ] **Step 2: Add Aptabase event tracking to stop_recording in commands.rs**

After the history push (around line 519 in `commands.rs`), add Aptabase tracking:

```rust
    // Track telemetry event (Aptabase — no-op if not initialized)
    if let Ok(client) = app_handle.try_state::<tauri_plugin_aptabase::AppState>() {
        let _ = client.track_event("dictation_completed", Some(serde_json::json!({
            "duration_seconds": (duration_ms as f64 / 1000.0).round(),
            "word_count": word_count,
            "used_ai_cleanup": was_cleaned_up,
            "used_dictionary": !dict.is_empty(),
        })));
    }
```

- [ ] **Step 3: Add Aptabase tracking to cancel_recording**

In the `cancel_recording` command, add tracking before returning:

```rust
    if let Ok(client) = app_handle.try_state::<tauri_plugin_aptabase::AppState>() {
        let _ = client.track_event("dictation_cancelled", Some(serde_json::json!({
            "duration_seconds": 0,
        })));
    }
```

- [ ] **Step 4: Add app_started event in lib.rs setup**

In the `.setup()` callback, after model loading (around line 145):

```rust
            // Track app start (telemetry — no-op if not initialized)
            {
                let state = handle.state::<SharedState>();
                let s = state.blocking_lock();
                let model_loaded = s.recognizer.is_some();
                drop(s);
                let version = env!("CARGO_PKG_VERSION");
                if let Some(client) = handle.try_state::<tauri_plugin_aptabase::AppState>() {
                    let _ = client.track_event("app_started", Some(serde_json::json!({
                        "version": version,
                        "model_loaded": model_loaded,
                    })));
                }
            }
```

- [ ] **Step 5: Add model_downloaded event to download_model command**

In `commands.rs`, in the `download_model` command, after successful download completion, add:

```rust
    // Track model download (Aptabase)
    // Use the same pattern as stop_recording tracking above
    // Properties: model_name, duration_seconds
```

- [ ] **Step 6: Verify — `cargo check` compiles**

Run: `cd src-tauri && cargo check`

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands.rs
git commit -m "feat: integrate Aptabase telemetry with opt-in gating"
```

---

## Task 4: Sentry Crash Reporting Integration

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Init Sentry before Builder in lib.rs**

At the top of `run()`, before any other initialization (before line 26):

```rust
    // Initialize Sentry crash reporting (must be before Builder to catch early panics)
    let _sentry_guard = if settings::load_settings().help_improve {
        Some(sentry::init(sentry::ClientOptions {
            dsn: Some("https://YOUR_DSN@YOUR_ORG.ingest.sentry.io/YOUR_ID".parse().unwrap()),
            release: Some(std::borrow::Cow::Borrowed(env!("CARGO_PKG_VERSION"))),
            environment: Some(std::borrow::Cow::Borrowed(if cfg!(debug_assertions) { "development" } else { "production" })),
            before_send: Some(Arc::new(|mut event| {
                // Strip exception values that may contain transcription text
                if let Some(ref mut exceptions) = event.exception.values {
                    for exc in exceptions.iter_mut() {
                        if let Some(ref val) = exc.value {
                            // Clear exception values containing potential transcript text
                            // (keep the type and stacktrace, just scrub the message)
                            if val.len() > 200 {
                                exc.value = Some("[scrubbed — long string]".into());
                            }
                        }
                    }
                }
                Some(event)
            })),
            before_breadcrumb: Some(Arc::new(|breadcrumb| {
                // Drop breadcrumbs that may contain transcription text
                if let Some(msg) = &breadcrumb.message {
                    let skip_patterns = ["After regex", "Raw transcript", "After AI cleanup", "LLM cleanup:", "Parakeet chunk"];
                    if skip_patterns.iter().any(|p| msg.contains(p)) {
                        return None;
                    }
                }
                Some(breadcrumb)
            })),
            ..Default::default()
        }))
    } else {
        None
    };
```

Note: `_sentry_guard` lives until end of `run()`, flushing events on drop.

- [ ] **Step 2: Register tauri-plugin-sentry**

Add to the builder chain:

```rust
        .plugin(tauri_plugin_sentry::init())
```

This auto-injects `@sentry/browser` into the webview for JS error capture.

- [ ] **Step 3: Update CSP for Sentry JS SDK**

In `src-tauri/tauri.conf.json`, add `https://*.ingest.sentry.io` to the `connect-src` directive:

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost https://huggingface.co https://*.huggingface.co https://github.com https://objects.githubusercontent.com https://*.ingest.sentry.io"
```

- [ ] **Step 4: Verify — `cargo check` compiles**

Run: `cd src-tauri && cargo check`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat: integrate Sentry crash reporting with breadcrumb scrubbing"
```

---

## Task 5: Announcements Module

**Files:**
- Create: `src-tauri/src/announcements.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Create announcements.rs**

```rust
use serde::{Deserialize, Serialize};

const ANNOUNCEMENTS_URL: &str = "https://raw.githubusercontent.com/sitelift/chirp-meta/main/announcements.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub max_version: Option<String>,
}

/// Use the same config directory as settings.rs to keep all app data together
fn app_data_dir() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| ".".into())
        .join("com.chirp.app")
}

fn cache_path() -> std::path::PathBuf {
    app_data_dir().join("announcements_cache.json")
}

fn seen_path() -> std::path::PathBuf {
    app_data_dir().join("announcements_seen.json")
}

pub fn load_seen() -> Vec<String> {
    std::fs::read_to_string(seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_seen(seen: &[String]) {
    if let Ok(json) = serde_json::to_string(seen) {
        let _ = std::fs::write(seen_path(), json);
    }
}

fn load_cache() -> Vec<Announcement> {
    std::fs::read_to_string(cache_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(announcements: &[Announcement]) {
    if let Ok(json) = serde_json::to_string_pretty(announcements) {
        let _ = std::fs::write(cache_path(), json);
    }
}

fn version_matches(announcement: &Announcement, app_version: &str) -> bool {
    let Ok(current) = semver::Version::parse(app_version) else {
        return true; // Can't parse version, show announcement
    };

    if let Some(min) = &announcement.min_version {
        if let Ok(min_v) = semver::Version::parse(min) {
            if current < min_v {
                return false;
            }
        }
    }

    if let Some(max) = &announcement.max_version {
        if let Ok(max_v) = semver::Version::parse(max) {
            if current > max_v {
                return false;
            }
        }
    }

    true
}

pub async fn fetch_announcements(app_version: &str) -> Vec<Announcement> {
    let seen = load_seen();

    // Try fetching fresh data
    let announcements = match reqwest::get(ANNOUNCEMENTS_URL).await {
        Ok(resp) => match resp.json::<Vec<Announcement>>().await {
            Ok(data) => {
                save_cache(&data);
                data
            }
            Err(_) => load_cache(),
        },
        Err(_) => load_cache(),
    };

    // Filter by version and seen status
    announcements
        .into_iter()
        .filter(|a| version_matches(a, app_version) && !seen.contains(&a.id))
        .collect()
}
```

- [ ] **Step 2: Add mod declaration and commands**

In `src-tauri/src/lib.rs`, add `mod announcements;` to the module declarations.

In `src-tauri/src/commands.rs`, add the two new commands:

```rust
#[tauri::command]
pub async fn get_announcements() -> Result<Vec<crate::announcements::Announcement>, String> {
    let version = env!("CARGO_PKG_VERSION");
    Ok(crate::announcements::fetch_announcements(version).await)
}

#[tauri::command]
pub async fn dismiss_announcement(id: String) -> Result<(), String> {
    let mut seen = crate::announcements::load_seen();
    if !seen.contains(&id) {
        seen.push(id);
        crate::announcements::save_seen(&seen);
    }
    Ok(())
}
```

- [ ] **Step 3: Register commands in lib.rs**

Add `commands::get_announcements` and `commands::dismiss_announcement` to the `invoke_handler` macro.

- [ ] **Step 4: Verify — `cargo check`**

Run: `cd src-tauri && cargo check`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/announcements.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add announcements module with GitHub fetch and local cache"
```

---

## Task 6: Feedback Module

**Files:**
- Create: `src-tauri/src/feedback.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create feedback.rs**

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// Discord webhook URL — replace with actual URL
const DISCORD_WEBHOOK_URL: &str = "YOUR_DISCORD_WEBHOOK_URL";

// Rate limit: 5 minutes between submissions
const RATE_LIMIT_SECS: u64 = 300;

static LAST_FEEDBACK_TIME: AtomicU64 = AtomicU64::new(0);

pub fn check_rate_limit() -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_FEEDBACK_TIME.load(Ordering::Relaxed);
    if last > 0 && now - last < RATE_LIMIT_SECS {
        let remaining = RATE_LIMIT_SECS - (now - last);
        return Err(format!("Please wait {} minutes before sending again", remaining / 60 + 1));
    }
    LAST_FEEDBACK_TIME.store(now, Ordering::Relaxed);
    Ok(())
}

pub async fn send_to_discord(
    text: &str,
    app_version: &str,
    os_info: &str,
    dictation_count: usize,
    days_since_install: i64,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "embeds": [{
            "title": "Chirp Feedback",
            "description": text,
            "color": 15771648,
            "fields": [
                { "name": "App", "value": format!("v{}", app_version), "inline": true },
                { "name": "OS", "value": os_info, "inline": true },
                { "name": "Dictations", "value": dictation_count.to_string(), "inline": true },
                { "name": "Installed", "value": format!("{} days ago", days_since_install), "inline": true },
            ]
        }]
    });

    client
        .post(DISCORD_WEBHOOK_URL)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send feedback: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Discord rejected feedback: {e}"))?;

    Ok(())
}
```

- [ ] **Step 2: Add send_feedback command**

In `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn send_feedback(
    text: String,
    state: tauri::State<'_, SharedState>,
) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Feedback cannot be empty".into());
    }
    if text.len() > 2000 {
        return Err("Feedback is too long (max 2000 characters)".into());
    }

    crate::feedback::check_rate_limit()?;

    // Single lock acquisition for all state reads
    let (dictation_count, days_since_install) = {
        let s = state.lock().await;
        let count = s.history.len();
        let days = if let Some(oldest) = s.history.first() {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&oldest.timestamp) {
                (chrono::Utc::now() - ts.with_timezone(&chrono::Utc)).num_days()
            } else { 0 }
        } else { 0 };
        (count, days)
    };

    let version = env!("CARGO_PKG_VERSION");
    let os_info = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

    crate::feedback::send_to_discord(&text, version, &os_info, dictation_count, days_since_install).await
}
```

- [ ] **Step 3: Register in lib.rs**

Add `mod feedback;` and `commands::send_feedback` to the invoke handler.

- [ ] **Step 4: Verify — `cargo check`**

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/feedback.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add feedback module with Discord webhook and rate limiting"
```

---

## Task 7: Onboarding — HelpImprove Step

**Files:**
- Create: `src/components/onboarding/HelpImprove.tsx`
- Modify: `src/components/onboarding/Onboarding.tsx`

- [ ] **Step 1: Create HelpImprove.tsx**

Follow the same pattern as `SmartCleanup.tsx` — simple component with toggle and description.

```tsx
import { useState } from 'react'
import { BarChart3 } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { Toggle } from '../shared/Toggle'
import { Button } from '../shared/Button'

interface HelpImproveProps {
  onNext: () => void
}

export function HelpImprove({ onNext }: HelpImproveProps) {
  const store = useAppStore()
  const [opted, setOpted] = useState(false)

  const handleToggle = (value: boolean) => {
    setOpted(value)
    store.updateSettings({ helpImprove: value })
  }

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Help Improve Chirp
      </h1>
      {/* PLACEHOLDER COPY — needs human rewrite */}
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        Share anonymous usage stats and crash reports to help us make Chirp better.
      </p>

      <div className="rounded-lg border border-card-border bg-chirp-stone-50 p-4 mt-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <BarChart3 size={18} className="text-chirp-stone-400" />
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Anonymous analytics</div>
              <div className="text-[11px] text-chirp-stone-400 mt-0.5">
                No audio, no text, no personal info
              </div>
            </div>
          </div>
          <Toggle checked={opted} onChange={handleToggle} />
        </div>
      </div>

      <p className="font-body text-xs text-chirp-stone-400 mt-3">
        You can change this anytime in Settings.
      </p>

      <div className="mt-6 flex flex-col gap-2">
        <Button size="onboarding" className="min-w-[160px] text-base" onClick={onNext}>
          {opted ? 'Continue' : 'Skip'}
        </Button>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Update Onboarding.tsx**

Add import and bump steps:

```tsx
import { HelpImprove } from './HelpImprove'

const STEPS = 5
```

Update the step rendering (add step 4, move SmartCleanup's `handleFinish` call to HelpImprove):

```tsx
          {step === 0 && <Welcome onNext={() => setStep(1)} />}
          {step === 1 && <SetupStep onNext={() => setStep(2)} />}
          {step === 2 && <ModelDownload onFinish={() => setStep(3)} />}
          {step === 3 && <SmartCleanup onNext={() => setStep(4)} />}
          {step === 4 && <HelpImprove onNext={handleFinish} />}
```

- [ ] **Step 3: Verify — `npm run build`**

- [ ] **Step 4: Commit**

```bash
git add src/components/onboarding/HelpImprove.tsx src/components/onboarding/Onboarding.tsx
git commit -m "feat: add Help Improve onboarding step with opt-in toggle"
```

---

## Task 8: Settings UI — Privacy & Feedback Section

**Files:**
- Modify: `src/components/settings/SettingsPage.tsx`

- [ ] **Step 1: Add Privacy & Feedback section**

At the bottom of the SettingsPage component (before the closing `</div>` of the main content area), add a new section using the existing `SectionLabel`, `Card`, `Row`, `Toggle` patterns:

```tsx
      {/* Privacy & Feedback */}
      <div className="mt-6">
        <SectionLabel>Privacy & Feedback</SectionLabel>
        <Card>
          <Row>
            <div className="flex-1">
              <div className="text-[13px] font-medium text-[#1a1a1a]">Help improve Chirp</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">
                Share anonymous usage stats and crash reports
              </div>
              <div className="text-[10px] text-chirp-stone-400 mt-1">
                Changes take effect on restart
              </div>
            </div>
            <Toggle
              checked={store.helpImprove}
              onChange={(v) => store.updateSettings({ helpImprove: v })}
            />
          </Row>
          <Row last>
            <FeedbackSection />
          </Row>
        </Card>
      </div>
```

- [ ] **Step 2: Add FeedbackSection component**

Inside SettingsPage.tsx (or as a separate component), add the inline feedback form:

```tsx
function FeedbackSection() {
  const [expanded, setExpanded] = useState(false)
  const [text, setText] = useState('')
  const [status, setStatus] = useState<'idle' | 'sending' | 'sent' | 'error'>('idle')
  const [errorMsg, setErrorMsg] = useState('')

  const handleSend = async () => {
    setStatus('sending')
    try {
      await invoke('send_feedback', { text })
      setStatus('sent')
      setText('')
      setTimeout(() => {
        setStatus('idle')
        setExpanded(false)
      }, 3000)
    } catch (e) {
      setStatus('error')
      setErrorMsg(String(e))
    }
  }

  if (!expanded) {
    return (
      <button
        onClick={() => setExpanded(true)}
        className="text-[13px] font-medium text-[#1a1a1a] hover:text-chirp-amber-500 transition-colors"
      >
        Send Feedback
      </button>
    )
  }

  return (
    <div className="w-full">
      <div className="text-[13px] font-medium text-[#1a1a1a] mb-2">Send Feedback</div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Tell us what you think..."
        maxLength={2000}
        className="w-full h-24 rounded-lg border border-card-border bg-white p-3 text-sm font-body text-chirp-stone-900 resize-none focus:outline-none focus:border-chirp-amber-400 transition-colors"
      />
      <div className="flex items-center justify-between mt-2">
        <span className="text-[11px] text-chirp-stone-400">
          {status === 'sent' ? 'Sent!' : status === 'error' ? errorMsg : `${text.length}/2000`}
        </span>
        <div className="flex gap-2">
          <button
            onClick={() => { setExpanded(false); setText(''); setStatus('idle') }}
            className="text-[12px] text-chirp-stone-400 hover:text-chirp-stone-600"
          >
            Cancel
          </button>
          <Button
            onClick={handleSend}
            disabled={text.trim().length === 0 || status === 'sending' || status === 'sent'}
          >
            {status === 'sending' ? 'Sending...' : status === 'sent' ? 'Sent!' : 'Send'}
          </Button>
        </div>
      </div>
    </div>
  )
}
```

Add `import { invoke } from '@tauri-apps/api/core'` if not already imported.

- [ ] **Step 3: Verify — `npm run build`**

- [ ] **Step 4: Commit**

```bash
git add src/components/settings/SettingsPage.tsx
git commit -m "feat: add Privacy & Feedback section to Settings"
```

---

## Task 9: Announcement Banner on HomePage

**Files:**
- Create: `src/components/settings/AnnouncementBanner.tsx`
- Modify: `src/components/settings/HomePage.tsx`

- [ ] **Step 1: Create AnnouncementBanner.tsx**

```tsx
import { useState, useEffect } from 'react'
import { X, Info } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'

interface Announcement {
  id: string
  title: string
  body: string
}

export function AnnouncementBanner() {
  const [announcements, setAnnouncements] = useState<Announcement[]>([])

  useEffect(() => {
    invoke<Announcement[]>('get_announcements')
      .then(setAnnouncements)
      .catch(() => {}) // Fail silently
  }, [])

  const dismiss = async (id: string) => {
    setAnnouncements((prev) => prev.filter((a) => a.id !== id))
    try {
      await invoke('dismiss_announcement', { id })
    } catch {
      // Fail silently
    }
  }

  if (announcements.length === 0) return null

  const announcement = announcements[0]

  return (
    <div className="rounded-lg border border-chirp-amber-200 bg-chirp-amber-50 p-3 mb-4">
      <div className="flex items-start gap-2">
        <Info size={16} className="text-chirp-amber-500 mt-0.5 shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-medium text-chirp-stone-900">{announcement.title}</div>
          <div className="text-[12px] text-chirp-stone-500 mt-0.5">{announcement.body}</div>
        </div>
        <button
          onClick={() => dismiss(announcement.id)}
          className="text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors shrink-0"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Add banner to HomePage**

In `src/components/settings/HomePage.tsx`, import and render at the top of the main content:

```tsx
import { AnnouncementBanner } from './AnnouncementBanner'
```

Add `<AnnouncementBanner />` at the beginning of the component's return JSX, before the greeting/stats area.

- [ ] **Step 3: Verify — `npm run build`**

- [ ] **Step 4: Commit**

```bash
git add src/components/settings/AnnouncementBanner.tsx src/components/settings/HomePage.tsx
git commit -m "feat: add dismissable announcement banner on HomePage"
```

---

## Task 10: JS-Side Aptabase Event Tracking

**Files:**
- Modify: `src/components/settings/HomePage.tsx`
- Modify: `src/components/settings/DictionaryPage.tsx`
- Modify: `src/components/settings/SnippetsPage.tsx`
- Modify: `src/components/onboarding/Onboarding.tsx`

- [ ] **Step 1: Add trackEvent helper**

The `@aptabase/tauri` package provides `trackEvent(name, props?)`. Import and call at relevant interaction points.

In each file, add:
```tsx
import { trackEvent } from '@aptabase/tauri'
```

Then call at these locations:
- **HomePage** — when user copies a transcription: `trackEvent('feature_used', { feature: 'history_copy' })`
- **DictionaryPage** — when adding an entry: `trackEvent('feature_used', { feature: 'dictionary_add' })`
- **SnippetsPage** — when adding a snippet: `trackEvent('feature_used', { feature: 'snippet_add' })`
- **Onboarding** — on completion: `trackEvent('onboarding_completed', { steps_completed: STEPS })`

Note: `trackEvent` from `@aptabase/tauri` is a no-op if the plugin wasn't initialized (user opted out), so no conditional check needed in JS.

- [ ] **Step 2: Verify — `npm run build`**

- [ ] **Step 3: Commit**

```bash
git add src/components/settings/HomePage.tsx src/components/settings/DictionaryPage.tsx src/components/settings/SnippetsPage.tsx src/components/onboarding/Onboarding.tsx
git commit -m "feat: add JS-side Aptabase event tracking for feature usage"
```

---

## Task 11: Single Instance + Startup Fix

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Register single-instance plugin**

In `lib.rs`, add to the builder chain:

```rust
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus the existing settings window when a second instance tries to launch
            if let Some(win) = app.get_webview_window("settings") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
```

- [ ] **Step 2: Add `--minimized` flag to autostart**

Change the autostart init from:
```rust
.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
```
To:
```rust
.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
```

- [ ] **Step 3: Change settings window to hidden by default**

In `tauri.conf.json`, change the settings window:
```json
"visible": false,
"maximized": false
```

- [ ] **Step 4: Add conditional show in setup**

In `lib.rs` `.setup()`, at the end (before `Ok(())`), add window visibility logic:

```rust
            // Show settings window unless launched with --minimized (autostart)
            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                if let Some(win) = app.get_webview_window("settings") {
                    let _ = win.show();
                    let _ = win.maximize();
                    let _ = win.set_focus();
                }
            }
```

- [ ] **Step 5: Check for single-instance plugin permission requirements**

Verify if `tauri-plugin-single-instance` v2 needs entries in `tauri.conf.json` `plugins` section. If so, add them. Check the plugin's README.

- [ ] **Step 6: Verify — `cargo check`**

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/tauri.conf.json
git commit -m "feat: add single-instance lock and minimize-to-tray on autostart"
```

---

## Task 12: Bug Fix Verification

**Files:**
- Read: `src-tauri/src/commands.rs` (recording timeout, cancel_recording, test_microphone)
- Read: `src/components/settings/HomePage.tsx` (clear history)
- Read: `src/components/settings/SettingsPage.tsx` (mic test intervals)

- [ ] **Step 1: Verify 10-min recording timeout exists**

Read `commands.rs` lines 250-270. Confirm generation-based timeout is implemented. Document findings.

- [ ] **Step 2: Verify clear-history confirmation exists**

Read `HomePage.tsx` around line 149. Confirm `window.confirm()` or equivalent is present.

- [ ] **Step 3: Review cancel_recording for zombie callbacks (H3)**

Read `cancel_recording` in `commands.rs`. Verify cpal stream handle is dropped properly. If `StreamHandle` and `StreamActiveState` are cleared, the bug is fixed.

- [ ] **Step 4: Review test_microphone for zombie callbacks (H4)**

Same pattern — verify stream is properly cleaned up after mic test ends.

- [ ] **Step 5: Review mic test interval stacking (H6)**

Read `SettingsPage.tsx` mic test section. Verify `clearInterval` is called before starting a new interval. Fix if needed.

- [ ] **Step 6: Fix any issues found, commit if changes made**

```bash
# Add only the specific files that were changed during verification
git add src-tauri/src/commands.rs src/components/settings/SettingsPage.tsx
git commit -m "fix: verify and address audit bug fixes H3/H4/H6"
```

---

## Task 13: App Icon Consistency

**Files:**
- Modify: `src-tauri/icons/icon.ico`
- Modify: `src-tauri/icons/icon.png`
- Modify: `src-tauri/icons/32x32.png`
- Modify: `src-tauri/icons/128x128.png`
- Modify: `src-tauri/icons/128x128@2x.png`
- Modify: `src-tauri/icons/icon.icns`

- [ ] **Step 1: Generate new icon from tray-icon**

The tray icon (`src-tauri/icons/tray-icon.png`) is the yellow bird on transparent background that looks correct. Use this as the source to generate all required sizes.

Use an icon generation tool (e.g., `tauri icon` command or manual resize) to create:
- `32x32.png` — 32x32px
- `128x128.png` — 128x128px
- `128x128@2x.png` — 256x256px
- `icon.png` — 512x512px
- `icon.ico` — multi-resolution Windows icon
- `icon.icns` — macOS icon bundle

**Note:** The tray icon may be too small as a source. May need to create a higher-resolution version of the yellow bird first. Check `tray-icon.png` dimensions. If it's only 32x32 or similar, need to either:
- Use the BirdMark SVG component as source (render at 512px)
- Create a new high-res icon manually

- [ ] **Step 2: Replace icon files**

Copy generated files into `src-tauri/icons/`, replacing existing ones.

- [ ] **Step 3: Verify — rebuild and check taskbar**

Run: `npx tauri build` or `npx tauri dev` and check:
- Windows taskbar shows yellow bird clearly
- Alt-Tab shows yellow bird
- System tray matches

- [ ] **Step 4: Commit**

```bash
git add src-tauri/icons/
git commit -m "fix: replace app icons with consistent yellow bird design"
```

---

## Verification Checklist

After all tasks are complete, run through these end-to-end tests:

- [ ] Fresh install: onboarding shows 5 steps including Help Improve
- [ ] Help Improve toggle (default off) persists across restart
- [ ] With opt-in: Aptabase dashboard shows events after usage
- [ ] With opt-in: Sentry captures a test JS error
- [ ] Without opt-in: no network calls to Aptabase or Sentry (verify in dev tools)
- [ ] Sentry breadcrumbs do NOT contain transcription text
- [ ] Announcements: banner appears on HomePage, dismisses, survives restart
- [ ] Announcements: works offline (cached version or empty)
- [ ] Feedback: sends to Discord webhook with correct embed format
- [ ] Feedback: rate-limited to 5-minute cooldown
- [ ] Single instance: second launch focuses existing window
- [ ] Auto-launch: starts minimized to tray
- [ ] Manual launch: shows window maximized
- [ ] Icon: consistent yellow bird in taskbar, alt-tab, tray
- [ ] `npm run build` and `cargo check` pass
- [ ] `npm run lint` passes
