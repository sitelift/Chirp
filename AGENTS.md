# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, etc.) working in this repository.

## What is Chirp?

A Windows desktop dictation app built with Tauri 2. Press a global hotkey, speak, release; cleaned-up text is pasted at the cursor in any app.

**v2 is being built on the `v2` branch.** `docs/v2/DIRECTION.md` is the source of truth for goals, decisions, and build order. Read it before making design decisions, and update it when a decision changes.

The two things that matter most, in order:

1. Time from hotkey release to text pasted.
2. Transcript quality.

## Scope

- **Windows only.** No macOS or Linux code paths. The reference machine is Windows 11 with an RTX 4080 Super.
- **Speech recognition is local.** sherpa-onnx + Parakeet TDT; moving to the CUDA provider.
- **Cleanup is cloud.** Anthropic API only (Claude Haiku 5.5), with the user's own key. There is no local LLM and there are no tone modes; the model infers register from the active app and window title.
- **No telemetry.** No analytics, crash reporting, or feedback webhooks.

## Build commands

### Prerequisites

sherpa-onnx DLLs must be in `src-tauri/sherpa-onnx-lib/` (gitignored). Get the Windows x64 shared-library archive matching the `sherpa-onnx` crate version from https://github.com/k2-fsa/sherpa-onnx/releases. Copy `lib/*.dll` and `lib/*.lib` into that directory, and copy the DLLs flat into `src-tauri/` for release bundling. `.github/workflows/release.yml` shows the exact steps CI uses.

Environment variables (bash):

```
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export CARGO_TARGET_DIR="C:/tmp/chirp-target"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```

The short `CARGO_TARGET_DIR` avoids the Windows 260-character path limit.

### Dev

```bash
npm install
npx tauri dev        # Vite dev server (port 5173) + Rust backend
```

If port 5173 is in use or the hotkey is already registered, kill stale `node.exe` and `chirp.exe` processes first.

### Build and check

```bash
npm run build        # TypeScript check + Vite bundle (frontend only)
npm run lint         # ESLint
npx tauri build      # Full release build (NSIS/MSI installer)
```

There's no test suite yet. Validate manually with the dev server on Windows.

The backend only builds on Windows. Cross-checking from Linux fails in C build scripts such as `ring`.

## Architecture

**Two-process Tauri app.** The React frontend talks to the Rust backend through `invoke()` IPC. Webview windows share no memory; they sync through Tauri events.

### Frontend (`src/`)

- React 19 + TypeScript + Vite + Tailwind.
- A single Zustand store (`src/stores/appStore.ts`). Each window has its own instance, kept in sync via the `settings-changed` event.
- Windows defined in `tauri.conf.json`:
  - `overlay`: the 300x64 recording pill. Transparent, always on top, click-through.
  - `settings`: the main window, with a custom titlebar.
  - `tray-popup`
- The current UI is v1 and will be redesigned from first principles; see "Brand north stars" in the direction doc. Don't invest in polishing it.

### Backend (`src-tauri/src/`)

| File | Responsibility |
|---|---|
| `commands.rs` | All `#[tauri::command]` handlers (the IPC surface), including the `start_recording` / `stop_recording` pipeline. |
| `audio.rs` | cpal capture, resampling to 16 kHz mono, Silero VAD segmentation, amplitude events. |
| `transcribe.rs` | sherpa-onnx Parakeet model download, loading, inference. |
| `cleanup.rs` | Deterministic text cleanup: filler words, spoken punctuation, numbers, VAD boundary repair. |
| `hotkey.rs` + `hotkey_raw_input.rs` + `hotkey_ll_suppress.rs` | Global hotkey. Raw Input handles detection; a `WH_KEYBOARD_LL` hook handles suppression. Scheduled for a rewrite (see "Hotkeys" in the direction doc). |
| `inject.rs` + `clipboard_win.rs` | Win32 clipboard write + `SendInput` Ctrl+V, then restore the previous clipboard. |
| `state.rs` | Shared app state and the persisted `Settings` struct. |
| `settings.rs` | Persistence for `settings.json`, `vocabulary.json`, `snippets.json`; legacy-file cleanup. |
| `history.rs` / `snippets.rs` | Transcription history and snippet expansion. |

### Data flow

```
Hotkey → cpal capture (16 kHz mono) → VAD segments → Parakeet → regex cleanup
→ vocabulary replacements → [cloud LLM cleanup: coming] → snippets → clipboard + Ctrl+V
```

### Cross-window settings sync

1. The frontend calls `invoke('update_settings', { partial })`.
2. Rust saves to disk and emits `settings-changed` to all windows.
3. Each window's `useSettingsSync` hook applies the change to its store. A `suppressSync` ref prevents echo loops.

## Constraints

- **Never use full-screen transparent overlay windows.** The overlay is sized to its content, and transitions are CSS.
- **No emojis in the UI.** Use Lucide React icons.
- **No `console.log` / `debug` / `warn` in the frontend.** Only `console.error`, and only in ErrorBoundary.
- **Context menu disabled in production.** Right-click is blocked via the `contextmenu` event.
- **Treat dictated text sent to the LLM as data, never as instructions.** Always fall back to the deterministic text on error, timeout, or refusal.
- **Measure latency rather than guess.** Every pipeline stage should be timed.
- **Code is MIT; brand assets are reserved.** Don't modify or redistribute the logo or icons (`assets/brand/`, `src-tauri/icons/`, `docs/images/`); see `BRAND.md`.
