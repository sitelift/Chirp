# APP.md — Chirp Desktop Application Spec

> **Complete specification for every screen, feature, and user interaction in the Chirp desktop app.**
> Claude Code: reference this alongside CHIRP-SPEC.md (architecture) and STYLE.md (design tokens).
> This document defines WHAT the app does. STYLE.md defines HOW it looks. CHIRP-SPEC.md defines HOW it's built.

---

## 1. App Overview

Chirp is a system utility that lives in the system tray. It has no main window. The user interacts with Chirp through three surfaces:

1. **System Tray** — always running, right-click menu for quick actions
2. **Floating Overlay** — appears during dictation, dismisses automatically
3. **Settings Window** — opened from tray menu, full settings and configuration

On first launch, a fourth surface appears:

4. **Onboarding Window** — 3-step setup wizard, shown once

---

## 2. App Lifecycle

### 2.1 First Launch

```
App starts
  → Check if onboarding is complete (settings.json)
  → If NOT complete:
      → Show Onboarding Window
      → After onboarding completes, write onboardingComplete: true
      → Minimize to system tray
  → If complete:
      → Load settings from disk
      → Load dictionary from disk
      → Load Whisper model into memory (if downloaded)
      → Load cleanup model into memory
      → Register global hotkey
      → Show system tray icon
      → App is ready (no visible window)
```

### 2.2 Normal Operation

```
App sits in system tray (no visible windows)
  → User presses hotkey → Overlay appears, recording starts
  → User presses hotkey again → Recording stops, processing begins
  → Processing completes → Text injected, overlay dismisses
  → App returns to idle in system tray
```

### 2.3 Shutdown

```
User clicks "Quit Chirp" in tray menu
  → Save any pending settings
  → Unregister global hotkey
  → Release audio device
  → Unload models from memory
  → Exit process
```

---

## 3. System Tray

The system tray icon is always present while Chirp is running. It's the primary way users access settings and quit the app.

### 3.1 Tray Icon States

| State | Icon | Description |
|-------|------|-------------|
| Idle | Bird silhouette, monochrome | App is ready, waiting for hotkey |
| Listening | Bird silhouette, green (#16A34A) | Actively recording audio |
| Processing | Bird silhouette, yellow (#F0B723) | Transcribing and cleaning text |
| Error | Bird silhouette, red (#DC2626) | Something went wrong |

On macOS the icon appears in the menu bar. On Windows it appears in the notification area. The icon should be crisp at both 16px (Windows) and 22px (macOS) sizes.

### 3.2 Tray Right-Click Menu

```
┌──────────────────────────────┐
│  Chirp v1.0.0                │
│──────────────────────────────│
│  Start Listening     ⌘⇧C    │
│  Settings...                 │
│──────────────────────────────│
│  Check for Updates           │
│  Quit Chirp                  │
└──────────────────────────────┘
```

**Menu items:**

- **Version display** — "Chirp v1.0.0" in secondary text. Not clickable.
- **Start Listening** — Same as pressing the hotkey. Toggles to "Stop Listening" while recording. Shows the current hotkey combo on the right.
- **Settings...** — Opens the Settings Window. If already open, brings it to focus.
- **Check for Updates** — Checks GitHub releases for a newer version. Shows a subtle notification if an update is available. Does not auto-update in v1.
- **Quit Chirp** — Shuts down the app completely.

### 3.3 Tray Left-Click (macOS only)

Single left-click on the menu bar icon opens the same menu as right-click. This is standard macOS menu bar behavior.

---

## 4. Floating Overlay

The overlay is the core interaction surface. It's a small, borderless, always-on-top window that appears when the user triggers a dictation and dismisses itself when done.

### 4.1 Window Behavior

- Appears centered horizontally, positioned at roughly 20% from the top of the screen
- Always on top of all other windows
- Cannot be moved, resized, or minimized by the user
- Does not appear in the taskbar or dock
- Transparent background with a white rounded card inside
- Appears with a 150ms fade-in + slide-down animation
- Dismisses with a 150ms fade-out animation

### 4.2 State: Listening

This state activates immediately when the hotkey is pressed.

**Layout:**
```
┌──────────────────────────────────────────┐
│  24px padding                            │
│                                          │
│  [bird 20px] [pulse 8px] Listening...    │
│                                          │
│  [||||||||||||||||||||░░░░░░░░░░░░]      │
│  (waveform visualization, full width)    │
│                                          │
│  ⌘⇧C to stop  ·  Esc to cancel          │
│                                          │
│  24px padding                            │
└──────────────────────────────────────────┘
```

**Elements:**
- Bird mark: 20px, chirp-yellow, vertically aligned with text
- Pulse dot: 8px, green (#16A34A), pulsing animation, positioned between bird and text
- "Listening..." — Inter 500, 16px, stone-900
- Waveform: full content width, 40px tall, amber-400 bars, 3px wide with 3px gap, pill-shaped, heights driven by real-time mic amplitude data
- Hint text: Inter 400, 12px, stone-500. Shows the current hotkey and Esc option. Separated by a middle dot (·).

**Behavior:**
- Waveform bars update at 60fps from mic amplitude data
- The overlay stays open until the user presses the hotkey again, presses Esc, or the silence timeout triggers
- Silence timeout: 3 seconds of silence (amplitude below threshold) automatically stops recording

### 4.3 State: Processing

Activates after recording stops. The overlay transitions smoothly from the listening state.

**Layout:**
```
┌──────────────────────────────────────────┐
│  24px padding                            │
│                                          │
│  [bird 20px]  Processing...              │
│                                          │
│  [═══════════════▶ shimmer]              │
│  (shimmer progress bar, full width)      │
│                                          │
│  24px padding                            │
└──────────────────────────────────────────┘
```

**Elements:**
- Bird mark: 20px, chirp-yellow
- "Processing..." — Inter 500, 16px, stone-900
- Shimmer bar: full width, 4px tall, amber-100 track with amber-400 shimmer sweeping left to right on loop
- No hint text (nothing for user to do except wait)

**Behavior:**
- Waveform fades out and shimmer fades in (200ms crossfade)
- This state should last under 2 seconds for most inputs
- If processing takes longer than 5 seconds, show "Still working..." below the shimmer in stone-500

### 4.4 State: Done

Brief confirmation that text was injected successfully.

**Layout:**
```
┌──────────────────────────────────────────┐
│  24px padding                            │
│                                          │
│  [check icon]  Inserted 47 words         │
│                                          │
│  24px padding                            │
└──────────────────────────────────────────┘
```

**Elements:**
- Check icon: Lucide `Check`, 20px, success green (#16A34A)
- "Inserted 47 words" — Inter 500, 14px, stone-700. Word count is dynamic.

**Behavior:**
- Overlay height shrinks smoothly to fit the compact content (200ms ease-out)
- Auto-dismisses after 800ms (300ms fade-out animation)
- If the user has disabled auto-dismiss in settings, the overlay stays until they click away or press Esc

### 4.5 State: Error

Shown when something goes wrong: mic access denied, model not loaded, transcription failed, etc.

**Layout:**
```
┌──────────────────────────────────────────┐
│  24px padding                            │
│                                          │
│  [warning icon]  Couldn't access mic     │
│                                          │
│  Check your system permissions           │
│                                          │
│  Open Settings                           │
│                                          │
│  24px padding                            │
└──────────────────────────────────────────┘
```

**Elements:**
- Warning icon: Lucide `AlertTriangle`, 20px, error red (#DC2626)
- Error title: Inter 500, 14px, stone-900
- Help text: Inter 400, 13px, stone-500
- Action link: Inter 500, 13px, info blue (#2563EB), underline on hover

**Error messages by type:**

| Error | Title | Help Text | Action |
|-------|-------|-----------|--------|
| Mic not found | No microphone detected | Connect a microphone and try again | — |
| Mic permission denied | Couldn't access microphone | Check your system permissions | Open Settings (opens OS settings) |
| Model not loaded | Speech model not ready | Download a model in settings | Open Settings |
| Transcription failed | Couldn't process audio | Try speaking more clearly | Try Again |
| Injection failed | Couldn't paste text | Make sure a text field is focused | Copy to Clipboard |
| Unknown | Something went wrong | Please try again | Try Again |

**Behavior:**
- Does NOT auto-dismiss — stays until user presses Esc, clicks away, or clicks the action link
- Tray icon switches to error red while this state is active

---

## 5. Settings Window

A standard decorated window with a sidebar navigation. This is where all configuration happens.

### 5.1 Window Behavior

- Opens from tray menu → "Settings..."
- Centered on screen when first opened
- Remembers position if moved
- Can be resized (min 560×440)
- Closing the window hides it (does not quit the app)
- Only one instance can be open at a time
- Default size: 640×520

### 5.2 Sidebar

Left side, 160px wide, stone-100 background.

**Top: Logo lockup**
- Bird mark (24px) + "chirp" wordmark (Nunito 800, 16px)
- Below logo: 1px stone-200 divider with 16px margin below

**Navigation items (top-aligned):**
1. General
2. Audio
3. Model
4. Dictionary
5. About

Active item has white background, stone-900 text, shadow-subtle. Inactive items have transparent background, stone-500 text. Items are 36px tall with rounded-lg radius.

### 5.3 Page: General

This is the default page shown when settings opens.

**Section: Hotkey**

```
┌─ Hotkey ─────────────────────────────────┐
│                                          │
│  Dictation toggle                        │
│  [⌘] [⇧] [C]              [Change]     │
│                                          │
└──────────────────────────────────────────┘
```

- Label: "Dictation toggle" — Inter 400, 14px, stone-700
- Current hotkey displayed as KeyBadge components
- "Change" secondary button on the right
- Clicking "Change" puts the hotkey display into capture mode: the key badges are replaced by a dashed border box with "Press new shortcut..." in stone-500. The next key combo pressed becomes the new hotkey. Press Esc to cancel.
- If the chosen hotkey conflicts with a system shortcut, show a warning below in error red: "This shortcut might conflict with other apps."

**Section: Behavior**

```
┌─ Behavior ───────────────────────────────┐
│                                          │
│  [✓] Launch at login                     │
│  [✓] Show in menu bar                    │
│  [ ] Play sound on complete              │
│  [✓] Auto-dismiss overlay                │
│                                          │
│  Silence timeout                         │
│  [3 seconds ▾]                           │
│                                          │
└──────────────────────────────────────────┘
```

- Each option is a checkbox with label
- "Launch at login" — registers/unregisters the app as a login item
- "Show in menu bar" — macOS only, toggles menu bar icon visibility
- "Play sound on complete" — plays a subtle chirp sound when text is inserted (sound bundled with app)
- "Auto-dismiss overlay" — when off, the Done state stays until user dismisses manually
- "Silence timeout" — dropdown with options: 2 seconds, 3 seconds (default), 5 seconds, 10 seconds, Never (manual stop only)

**Section: Output**

```
┌─ Output ─────────────────────────────────┐
│                                          │
│  Language                                │
│  [Auto-detect ▾]                         │
│                                          │
│  [✓] Smart formatting                    │
│      Automatically format lists,         │
│      paragraphs, and structure            │
│                                          │
└──────────────────────────────────────────┘
```

- Language dropdown: Auto-detect (default), English, Spanish, French, German, Portuguese, Italian, Japanese, Chinese, Korean, and other languages Whisper supports
- "Smart formatting" toggle — enables/disables the T5 cleanup model pass. When off, only Whisper's raw output + dictionary replacements are applied. When on, full cleanup pipeline runs. Default: on.
- Description text below the smart formatting toggle in stone-500, 13px

All settings save automatically when changed (debounced 500ms). No save button needed. Show a subtle "Saved" text that fades in/out near the bottom of the content area after changes.

### 5.4 Page: Audio

**Section: Input Device**

```
┌─ Input Device ───────────────────────────┐
│                                          │
│  Microphone                              │
│  [MacBook Pro Microphone ▾]              │
│                                          │
│  Input Level                             │
│  [████████████░░░░░░░░░░░░░░░░]         │
│  (live level meter)                      │
│                                          │
│  [Test Microphone]                       │
│                                          │
└──────────────────────────────────────────┘
```

- Microphone dropdown: lists all system audio input devices via cpal. Refreshes when dropdown opens. Shows "Default" as first option which uses the system default.
- Input level meter: horizontal bar, full width. Background: stone-200. Fill: success green, width driven by live mic amplitude. Updates at 30fps. Shows real-time audio level even when not recording, so users can verify their mic works.
- "Test Microphone" secondary button: when clicked, starts a 5-second recording and plays it back through the default output device. Button text changes to "Recording... (5s)" with a countdown, then "Playing back..." during playback, then returns to "Test Microphone."

**Section: Processing**

```
┌─ Processing ─────────────────────────────┐
│                                          │
│  [✓] Noise suppression                   │
│      Reduces background noise before     │
│      transcription                       │
│                                          │
└──────────────────────────────────────────┘
```

- Noise suppression toggle with description text. This applies a basic noise gate/suppression filter to the audio before sending to Whisper. Default: on.

### 5.5 Page: Model

**Section: Speech Model (Whisper)**

```
┌─ Speech Model ───────────────────────────┐
│                                          │
│  Current: Whisper Small (English)        │
│  Size: 488 MB                            │
│                                          │
│  Model                                   │
│  ( ) Tiny    — 75 MB, fastest, lower     │
│               accuracy                   │
│  ( ) Base    — 150 MB, good balance      │
│  (●) Small   — 500 MB, recommended       │
│  ( ) Medium  — 1.5 GB, best accuracy,    │
│               slower                     │
│                                          │
│  [████████████████████░░░░] 78%          │
│  Downloading whisper-small.bin...        │
│                                          │
└──────────────────────────────────────────┘
```

- Current model display: shows name and file size in stone-700
- Radio button group for model selection. Each option shows size and a brief description.
- When a model is selected that isn't downloaded, a download progress bar appears. Progress bar uses amber-400 fill on stone-200 track, 4px height. Percentage shown to the right. Status text below.
- "Recommended" badge next to Small option — small amber-100 bg badge with amber-500 text, rounded-md, Inter 500 11px.
- If a model is already downloaded, selecting it just loads it (near-instant). Show a green check icon next to downloaded models.

**Section: Text Cleanup**

```
┌─ Text Cleanup ───────────────────────────┐
│                                          │
│  Model: Chirp Cleanup v1                 │
│  Size: 38 MB  ·  Bundled                 │
│                                          │
│  This model runs locally to format       │
│  your transcripts. It handles            │
│  punctuation, lists, paragraphs,         │
│  and filler word removal.                │
│                                          │
└──────────────────────────────────────────┘
```

- Display only, not user-configurable in v1. Shows the cleanup model name, size, and that it's bundled with the app.
- Description text in stone-500 explaining what it does.

### 5.6 Page: Dictionary

The dictionary is a user-editable list of text replacements that run after the cleanup model. This handles proper nouns, brand names, acronyms, and corrections.

**Layout:**

```
┌─────────────────────────────────────────────┐
│                                             │
│  Personal Dictionary                        │
│                                             │
│  Words and phrases Chirp should always      │
│  spell or format a specific way.            │
│                                             │
│  ┌──────────────┬──────────────┬───┐        │
│  │ Heard        │ Replace with │   │        │
│  ├──────────────┼──────────────┼───┤        │
│  │ iowa state   │ Iowa State   │ 🗑 │        │
│  │ chirp        │ Chirp        │ 🗑 │        │
│  │ react        │ React        │ 🗑 │        │
│  │ wispr flow   │ Wispr Flow   │ 🗑 │        │
│  │ api          │ API          │ 🗑 │        │
│  └──────────────┴──────────────┴───┘        │
│                                             │
│  ┌──────────────┐ ┌──────────────┐          │
│  │ New phrase... │ │ Replaced by..│ [+ Add]  │
│  └──────────────┘ └──────────────┘          │
│                                             │
└─────────────────────────────────────────────┘
```

**Elements:**
- Page title: "Personal Dictionary" — Nunito 700, 18px
- Description: Inter 400, 14px, stone-500
- Table with two columns: "Heard" (what the model outputs) and "Replace with" (what it should be)
- Table header: Inter 600, 12px, stone-500, uppercase, letter-spacing 0.5px, stone-100 bg
- Table rows: Inter 400, 14px, stone-700. Alternating white/stone-50 rows for readability. 44px row height.
- Delete button per row: Lucide `Trash2`, 16px, stone-400. Hover: error red. Clicking removes the entry immediately.
- Add row at bottom: two text inputs side by side + "Add" primary button. Input placeholders: "New phrase..." and "Replaced by..."
- Pressing Enter in either input submits the new entry (same as clicking Add)
- Empty state (no entries): show a centered message "No entries yet. Add words and phrases Chirp should always format correctly." in stone-500, 14px.

**Behavior:**
- Dictionary entries are case-insensitive on the "Heard" side (matching) but preserve case on the "Replace with" side
- Entries are applied as a post-processing pass after the cleanup model, before text injection
- Changes save immediately to dictionary.json
- Maximum 500 entries (show a warning at 450+)

### 5.7 Page: About

**Layout:**

```
┌──────────────────────────────────────────┐
│                                          │
│            [bird mark, 64px]             │
│                                          │
│               chirp                      │
│             v1.0.0                       │
│                                          │
│   Free, local voice-to-text             │
│   for everyone.                          │
│                                          │
│         trychirp.app                     │
│                                          │
│       [Check for Updates]                │
│                                          │
│──────────────────────────────────────────│
│                                          │
│  Made by Pieter de Bruijn               │
│                                          │
│  Speech recognition: whisper.cpp         │
│  Text cleanup: Chirp Cleanup v1          │
│  Built with Tauri + React               │
│                                          │
│  Source code on GitHub →                 │
│                                          │
└──────────────────────────────────────────┘
```

**Elements:**
- Bird mark: 64px, chirp-yellow, centered
- "chirp" — Nunito 800, 28px, stone-900, centered
- Version — JetBrains Mono 400, 13px, stone-500, centered
- Tagline — Inter 400, 14px, stone-500, italic, centered
- Domain — JetBrains Mono 400, 13px, info blue, clickable (opens browser)
- "Check for Updates" — secondary button, centered
- Divider: 1px stone-200, full width, 24px vertical margin
- Credits section: Inter 400, 13px, stone-500
- "Source code on GitHub →" — Inter 500, 13px, info blue, clickable (opens GitHub repo)

**Update check behavior:**
- Button text changes to "Checking..." while checking
- If update available: "Update available! v1.1.0" with a "Download" link that opens the GitHub release page
- If current: "You're on the latest version." in success green, fades after 3 seconds
- If check fails: "Couldn't check for updates." in stone-500

---

## 6. Onboarding Window

Shown on first launch only. A 3-step wizard that handles essential setup. Same window dimensions as settings (640×520) but no sidebar.

### 6.1 Common Elements

All three steps share:
- Content centered horizontally, max-width 400px
- Step indicator dots at the bottom: 3 dots, 8px each, 8px gap. Active: amber-400. Inactive: stone-300.
- Generous vertical centering

### 6.2 Step 1: Welcome

```
┌──────────────────────────────────────────────┐
│                                              │
│                                              │
│              [bird mark, 80px]               │
│                                              │
│            Welcome to Chirp                  │
│                                              │
│    Free, local voice-to-text for everyone.   │
│                                              │
│    Your voice never leaves your device.      │
│    No accounts. No cloud. No subscriptions.  │
│                                              │
│            [ Get Started → ]                 │
│                                              │
│               ● ○ ○                          │
│                                              │
└──────────────────────────────────────────────┘
```

- Bird mark: 80px, chirp-yellow, centered
- Title: Nunito 800, 28px, stone-900, centered
- Body: Inter 400, 15px, stone-700, centered, max-width 360px, line-height 1.7
- CTA: primary button, 44px height, min-width 180px
- No back button on step 1

### 6.3 Step 2: Microphone Permission

```
┌──────────────────────────────────────────────┐
│                                              │
│                                              │
│              [mic icon, 48px]                │
│                                              │
│         Chirp needs your microphone          │
│                                              │
│    We only listen when you press your        │
│    hotkey. That's it. Nothing runs in        │
│    the background.                           │
│                                              │
│          [ Allow Microphone → ]              │
│                                              │
│               ○ ● ○                          │
│                                              │
└──────────────────────────────────────────────┘
```

- Mic icon: Lucide `Mic`, 48px, stone-700, centered
- Title: Nunito 800, 24px, stone-900, centered
- Body: Inter 400, 15px, stone-700, centered, max-width 360px
- CTA: primary button, triggers the OS microphone permission dialog
- After permission granted: automatically advances to step 3
- If permission denied: body text changes to "Microphone access was denied. You can enable it in your system settings." with a "Open System Settings" secondary button below the primary CTA.

### 6.4 Step 3: Hotkey Setup

```
┌──────────────────────────────────────────────┐
│                                              │
│                                              │
│            [keyboard icon, 48px]             │
│                                              │
│            Set your hotkey                   │
│                                              │
│   This is the shortcut you'll press to       │
│   start and stop dictation.                  │
│                                              │
│   ┌─────────────────────────────────┐        │
│   │        [⌘]  [⇧]  [C]          │        │
│   │     Press keys to change...     │        │
│   └─────────────────────────────────┘        │
│                                              │
│         [ Start Using Chirp → ]              │
│                                              │
│               ○ ○ ●                          │
│                                              │
└──────────────────────────────────────────────┘
```

- Keyboard icon: Lucide `Keyboard`, 48px, stone-700, centered
- Title: Nunito 800, 24px, stone-900, centered
- Body: Inter 400, 15px, stone-700, centered
- Hotkey capture area: 320px × 80px, centered. Default state shows the default hotkey (⌘⇧C / Ctrl+Shift+C) as KeyBadge components. Below in stone-500 12px: "Press keys to change..."
  - Border: 2px dashed stone-300 (default), 2px solid amber-400 (focused/capturing)
  - Background: stone-100
  - Radius: rounded-xl
  - Clicking the area activates key capture mode. Next key combo pressed becomes the hotkey. Esc cancels and reverts to previous.
- CTA: primary button, 44px height. Clicking this:
  1. Saves the hotkey setting
  2. Marks onboarding complete
  3. Closes the onboarding window
  4. Minimizes to system tray
  5. If Whisper model isn't downloaded, begins download in background (user can start using the app once download completes)

---

## 7. Model Download Flow

Since the Whisper model is not bundled with the app, it needs to be downloaded before first use.

### 7.1 During Onboarding

After onboarding completes and the app minimizes to tray:
- If no Whisper model is downloaded, begin downloading the default model (whisper-small) automatically in the background
- Tray tooltip shows: "Chirp — Downloading speech model (34%)..."
- If the user tries to dictate before the model is ready, show the Error overlay: "Speech model not ready" / "Downloading now... 34%" / with no action button (just wait)

### 7.2 In Settings

The Model page (section 5.5) shows download progress for any model being downloaded. Users can switch models here. Downloading a new model doesn't delete the old one — both remain on disk. Users can see disk usage per model.

### 7.3 Download Source

Whisper models downloaded from Hugging Face (ggerganov/whisper.cpp repository). URLs hardcoded in the app with the model size as the variable.

---

## 8. Dictation Flow (Detailed)

### 8.1 Happy Path

```
1. User presses Ctrl+Shift+C (or custom hotkey)
2. App checks: is a Whisper model loaded? If no → error state
3. App checks: is mic accessible? If no → error state
4. Overlay window becomes visible (listening state)
5. Rust begins capturing audio via cpal
   - Format: 16kHz, mono, f32 samples
   - Audio stored in a growing buffer
6. Amplitude data sent to frontend at 60fps for waveform
7. User speaks
8. User presses hotkey again to stop
   OR silence detected (3s below amplitude threshold)
9. Overlay transitions to processing state
10. Audio buffer passed to whisper-rs
    - Model: loaded Whisper model
    - Language: user setting (auto-detect or specific)
    - Output: raw text string
11. Raw transcript passed to T5 cleanup model via ort
    - Input: raw transcript
    - Output: formatted, cleaned text
12. Cleaned text run through dictionary replacements
    - Case-insensitive match on "heard" column
    - Replace with exact "replace with" value
13. Final text copied to system clipboard via arboard
14. Previous clipboard contents saved temporarily
15. Ctrl+V (or Cmd+V) simulated via enigo
16. Wait 100ms for paste to complete
17. Restore original clipboard contents
18. Overlay transitions to done state
    - Shows word count
19. After 800ms, overlay dismisses
20. Tray icon returns to idle state
```

### 8.2 Clipboard Handling

Critical: the user likely has something on their clipboard already. Chirp must not destroy it.

```
1. Read current clipboard contents (text and/or image)
2. Write transcription result to clipboard
3. Simulate paste keystroke
4. Wait 100ms
5. Restore original clipboard contents
```

If the original clipboard contained an image or non-text data, arboard should handle this gracefully. If restoration fails, log the error but don't crash or show a user-facing error.

### 8.3 Edge Cases

| Scenario | Behavior |
|----------|----------|
| User presses hotkey with no text field focused | Text gets pasted into whatever has focus. If nothing, it goes nowhere. Not an error. |
| User speaks but no speech is detected | Whisper returns empty string. Skip cleanup, don't inject. Show "No speech detected" in the done state instead of word count. |
| User cancels with Esc during listening | Discard audio buffer, dismiss overlay immediately, return to idle. |
| User cancels with Esc during processing | Cannot cancel processing mid-inference. Overlay stays in processing state until complete, then discards the result. |
| Very long dictation (60+ seconds) | No hard limit. Audio buffer grows as needed. Processing will take longer. Show "Still working..." text after 5 seconds. |
| App loses mic access mid-recording | Transition to error state: "Microphone disconnected." |
| Multiple rapid hotkey presses | Debounce: ignore hotkey presses within 300ms of the last one. |

---

## 9. Keyboard Shortcuts

| Shortcut | Scope | Action | Customizable |
|----------|-------|--------|-------------|
| Ctrl+Shift+C (Win/Linux) / Cmd+Shift+C (Mac) | Global | Toggle dictation (start/stop) | Yes |
| Esc | While overlay is visible | Cancel recording or dismiss error | No |

The global hotkey works even when Chirp is not focused. This is the whole point of the app.

---

## 10. Data Storage

All data stored locally. No cloud sync, no remote storage.

### 10.1 Files

| File | Location | Content |
|------|----------|---------|
| settings.json | App config dir | All user preferences |
| dictionary.json | App config dir | User dictionary entries |
| whisper-*.bin | App data dir / models/ | Downloaded Whisper model files |
| chirp-cleanup.onnx | App resources (bundled) | T5 cleanup model |

### 10.2 Settings Schema

```json
{
  "version": 1,
  "hotkey": "CmdOrCtrl+Shift+C",
  "launchAtLogin": true,
  "showInMenuBar": true,
  "playSoundOnComplete": false,
  "autoDismissOverlay": true,
  "silenceTimeout": 3,
  "language": "auto",
  "smartFormatting": true,
  "inputDevice": "default",
  "noiseSuppression": true,
  "whisperModel": "small",
  "onboardingComplete": false,
  "windowPositions": {
    "settings": { "x": null, "y": null }
  }
}
```

### 10.3 Dictionary Schema

```json
{
  "version": 1,
  "entries": [
    { "from": "iowa state", "to": "Iowa State" },
    { "from": "chirp", "to": "Chirp" },
    { "from": "api", "to": "API" }
  ]
}
```

---

## 11. Notifications

Chirp uses minimal system notifications. Most feedback happens through the overlay and tray icon.

| Event | Notification |
|-------|-------------|
| Model download complete | System notification: "Chirp is ready! Press [hotkey] to start dictating." |
| Update available | System notification: "Chirp v1.1.0 is available. Open settings to update." |
| App started (after first setup) | Tray tooltip: "Chirp is running. Press [hotkey] to dictate." |

No notification sounds except the optional completion sound (user preference). Never show notifications during dictation — the overlay handles all feedback.

---

## 12. Performance Targets

| Metric | Target |
|--------|--------|
| App startup (to tray ready) | < 3 seconds |
| Overlay appear (hotkey to visible) | < 150ms |
| Whisper transcription (10s audio, small model) | < 2 seconds |
| T5 cleanup pass | < 500ms |
| End-to-end (hotkey stop to text injected) | < 3 seconds |
| Memory usage (idle) | < 200MB |
| Memory usage (during dictation) | < 600MB |
| Binary size (without models) | < 15MB |
| Installer size (with cleanup model, without Whisper) | < 50MB |

---

## 13. Future Considerations (NOT in v1)

These are explicitly out of scope for v1. Do not build these.

- Dark mode
- Multiple language simultaneous detection
- Voice commands ("delete that", "undo", "select all")
- Text history / transcript log
- Cloud sync of settings or dictionary
- Mobile companion app
- Browser extension
- Auto-update mechanism (v1 uses manual check + GitHub releases)
- Streaming transcription (v1 is batch: record → process → inject)
- Custom wake word (v1 uses hotkey only)
- Plugin system
- Team/enterprise features
