# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Chirp?

Chirp is a local-only voice-to-text desktop app built with Tauri 2. Users press a global hotkey, speak, and transcribed text is injected at their cursor in any application. All processing is on-device — no cloud, no accounts.

## Build Commands

### Prerequisites (Windows)

sherpa-onnx DLLs must be in `src-tauri/sherpa-onnx-lib/` (not committed to git). Download from https://github.com/k2-fsa/sherpa-onnx/releases — get the `win-x64-shared-MD-Release` archive, copy `lib/*.dll` and `lib/*.lib` into that directory. Also copy DLLs flat into `src-tauri/` root for release bundling.

Required environment variables (bash):
```
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export VULKAN_SDK="C:/VulkanSDK/1.4.341.1"
export CARGO_TARGET_DIR="C:/tmp/chirp-target"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```
The short `CARGO_TARGET_DIR` avoids Windows MAX_PATH (260 char) limit that breaks Vulkan shader compilation.

### Dev

```bash
npm install
npx tauri dev        # Launches both Vite dev server (port 5173) and Rust backend
```

Kill stale `node.exe` and `chirp.exe` processes before restarting dev server if port 5173 is in use or hotkey is already registered.

### Build

```bash
npm run build        # TypeScript check + Vite bundle (frontend only)
npx tauri build      # Full release build (frontend + Rust + NSIS/MSI installer)
```

Installers output to `src-tauri/target/release/bundle/nsis/` and `bundle/msi/`.

### Lint

```bash
npm run lint         # ESLint
```

No test suite exists. Validation is manual via the dev server.

## Architecture

**Two-process Tauri app:** React frontend communicates with Rust backend via `invoke()` IPC calls. Two separate webview windows share no in-memory state — cross-window sync uses Tauri events.

### Frontend (src/)
- **React 19 + TypeScript + Vite** with **Tailwind CSS** styling
- **Zustand** single global store (`src/stores/appStore.ts`) — each window has its own store instance, synced via `settings-changed` Tauri event
- **Two windows** defined in `tauri.conf.json`:
  - `overlay` — 300x64px, transparent, always-on-top, no decorations, click-through (the recording pill)
  - `settings` — 1400x900px, custom titlebar (decorations: false), main app window
- **Pages** (3 nav items + settings): Home (dashboard + history), Dictionary, Snippets, Settings
- **Onboarding** — 4-step split-panel wizard: Welcome, Hotkey Setup, Model Download, Smart Cleanup

### Backend (src-tauri/src/)
- `commands.rs` — All `#[tauri::command]` handlers (IPC surface). Emits `settings-changed` event for cross-window sync.
- `audio.rs` — cpal microphone capture, resampling to 16kHz mono, amplitude extraction
- `transcribe.rs` — sherpa-onnx Parakeet ASR model loading and inference
- `cleanup.rs` — Regex-based text cleanup (filler words, spoken punctuation, email formatting, number conversion)
- `llm.rs` — AI cleanup via llama-server subprocess. Uses datamarking (^ between words) + JSON schema constraint to prevent prompt injection.
- `hotkey.rs` (macOS) / `hotkey_windows.rs` (Windows) — Platform-specific global hotkey via low-level keyboard hooks
- `inject.rs` — Clipboard write + Ctrl+V/Cmd+V simulation via enigo
- `state.rs` — Shared app state (Settings, recognizer, LLM process handle)
- `settings.rs` — Disk persistence for settings.json, dictionary.json, snippets.json
- `history.rs` — Transcription history persistence with retention-based pruning

### Data flow
```
Hotkey press → cpal audio capture (16kHz mono)
→ sherpa-onnx Parakeet model → raw transcript
→ regex cleanup → optional llama-server AI cleanup (datamarked + JSON schema)
→ dictionary replacements → clipboard + Ctrl+V injection
```

### Cross-window sync
Settings window and overlay window are separate webviews. When settings change:
1. Frontend calls `invoke('update_settings', { partial })`
2. Rust saves to disk and emits `settings-changed` event to all windows
3. Each window's `useSettingsSync` hook listens for the event and updates its local Zustand store
4. A `suppressSync` ref prevents infinite loops (don't re-sync changes received from another window)

### Native libraries
- sherpa-onnx DLLs/dylibs in `src-tauri/sherpa-onnx-lib/` (+ flat copies in `src-tauri/` for Windows release bundling)
- `build.rs` configures linker search paths per platform
- Speech model and LLM model downloaded at runtime to `%APPDATA%/com.chirp.app/`

## Design System

Dark sidebar (`#1a1917`) with noise texture and amber glow. Warm off-white content area (`#F5F4F0`). Cards with `#EDECE8` borders, 14px radius. Nunito 800/900 for display text, Inter for body. Dark near-black (`#1a1a1a`) for toggles/buttons, amber (`#F0B723`) only as accent. Spring easing (`cubic-bezier(0.34, 1.56, 0.64, 1)`) for hover animations. Custom titlebar with dark bg. No emojis anywhere in the UI — use Lucide React icons.

## Key Design Constraints

- **Never use full-screen transparent overlay windows on Windows.** The overlay is sized to fit content (300x64px); transitions are CSS-animated.
- **No cloud dependencies.** Everything runs locally. No analytics, telemetry, or user accounts.
- **No emojis.** Use Lucide React icons for all iconography.
- **No console.log/debug/warn in production.** All removed. Only `console.error` in ErrorBoundary.
- **Right-click context menu disabled** in production via `contextmenu` event prevention.
- **Prompt injection defense** in LLM cleanup: datamarking (^ between words), JSON schema constraint (`response_format`), and output length guard.
- **Platform-conditional code** uses `#[cfg(target_os)]` in Rust and runtime detection in TypeScript.
- **Unified hotkey capture** — single flow via Win32 keyboard hook (`captureHotkeyKey`), no two-mode system exposed to users.

## Spec Documents

- `APP.md` — Complete feature spec, UI screens, user flows
- `CHIRP-SPEC.md` — Tech stack and architecture overview
- `docs/superpowers/specs/2026-03-21-ui-redesign-design.md` — UI redesign design spec
