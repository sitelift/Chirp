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
- **Cleanup is cloud and optional.** Anthropic API only (Claude Haiku 5.5), with the user's own key, stored in Windows Credential Manager. There is no local LLM and there are no tone modes; the model infers register from the active app and window title.
- **No telemetry.** No analytics, crash reporting, or feedback webhooks.

## Build commands

### Prerequisites

Install VS 2022 Build Tools with the Desktop development with C++ workload and Windows SDK, plus CMake and LLVM. Then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup-windows.ps1
```

This downloads SHA-256-pinned sherpa-onnx 1.12.36 CPU MT libraries, copies DLLs/import libraries into the gitignored build locations, and generates the ONNX import library. No CUDA or speech model downloads are needed to compile.

For an installer, run the same script with `-Bundle`. It also copies the required x64 VC143 CRT redistributables (including `concrt140`, `msvcp140*`, `vccorlib140` and `vcruntime140*`) beside the app. It discovers the installed VS C++ redist directory (14.44 or newer); `-VcRedistDir <path-to-Microsoft.VC143.CRT>` selects an exact toolchain. CI uses this same script. Missing files fail with an actionable error.

Plain `cargo build` and `tauri dev` use only CPU resources. Bundling is opt-in through `src-tauri/tauri.bundle.conf.json`, which adds the CRT files; this avoids making development depend on installer-only resources or silently producing incomplete installers. Production also merges `src-tauri/tauri.release.conf.json` for signing.

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
npx tauri build --config src-tauri/tauri.bundle.conf.json  # NSIS; run setup -Bundle first
```

There's no test suite yet. Validate manually with the dev server on Windows.

The backend only builds on Windows. Cross-checking from Linux fails in C build scripts such as `ring`.

## Architecture

**Two-process Tauri app with one window.** The React frontend talks to the Rust backend through `invoke()` IPC and listens to Tauri events. The design model is "the pill is the app": see "Design direction" in the direction doc.

### Frontend (`src/`)

- React 19 + TypeScript + Vite + Tailwind + Zustand. Lucide icons.
- One window, `overlay` (`tauri.conf.json`): transparent, undecorated, always on top, no taskbar button. It draws the resting tick, the ink pill and every expanded surface, and is resized to fit them (see `surface.rs`). There is no settings window and no tray popup.
- `src/surface/`:
  - `store.ts`: a Zustand store per surface instance, provided through React context with the backend `Api` (`api.ts`), so the preview can mount many stores with a mock backend.
  - `Surface.tsx`: the stage and the morphing container; measures content, sets the window size and mode (`surface_update`), keyboard navigation for the opened surface.
  - `Pill.tsx`, `views/` (Home, Microphone, Words, AiCleanup, Recent, FirstRun), `parts.tsx` (rows, key caps, toggle, level bars).
  - `dictation.ts`: the live controller: boot, backend events, the hotkey → record → paste flow, first run, update checks.
  - `actions.ts`: store changes plus their backend calls (settings, words, rebind, pause, API key). `words.ts` maps vocabulary + snippets to the Words list.
- `src/preview/`: dev-only gallery of every state in both themes (`http://localhost:5173/?preview=1`, `&section=03` for one artboard). Excluded from production builds.
- Design tokens are CSS variables per `[data-theme]` in `src/styles/globals.css`, wired into `tailwind.config.ts`. Surfaces follow the Windows theme (`prefers-color-scheme`); the ink pill does not.

### Backend (`src-tauri/src/`)

| File | Responsibility |
|---|---|
| `commands.rs` | Most `#[tauri::command]` handlers, including the `start_recording` / `stop_recording` pipeline, `copy_text`, `probe_microphone`. |
| `surface.rs` | The one window: bottom-center placement on a monitor's work area (one `SetWindowPos`), click-through / focus modes via extended styles, hover poll for the resting tick, focus hand-back on close, blur closes. |
| `tray.rs` | Tray icon and native menu (Restart to update, Open, Pause/Resume, Quit), pause state checked by the hotkey dispatcher, paused / update icon variants drawn in memory. |
| `ai.rs` | Anthropic key in Windows Credential Manager (`keyring`), validation, live AI status events. |
| `levels.rs` | Per-device input levels for the microphone picker. |
| `audio.rs` | cpal capture, resampling to 16 kHz mono, Silero VAD segmentation, amplitude events. |
| `transcribe.rs` | sherpa-onnx Parakeet model download, loading, inference. |
| `cleanup.rs` | Deterministic text cleanup: filler words, spoken punctuation, numbers, VAD boundary repair. |
| `router.rs` / `llm.rs` / `speculative.rs` / `context.rs` | When to call the LLM, the Anthropic client (shared, updatable key), speculative cleanup at VAD pauses, active-app context. |
| `hotkey.rs` + `hotkey_input.rs` | Global hotkeys: physical bindings and matching; one input thread owns the low-level hooks, binding capture (with live key labels), and Esc-to-cancel while dictating. See `docs/v2/HOTKEYS.md`. |
| `inject.rs` + `clipboard_win.rs` | Win32 clipboard write + `SendInput` Ctrl+V, then restore the previous clipboard. |
| `state.rs` | Shared app state and the persisted `Settings` struct. |
| `settings.rs` | Persistence for `settings.json`, `vocabulary.json`, `snippets.json`; legacy-file cleanup. |
| `history.rs` / `snippets.rs` | Transcription history and snippet expansion. |

### Data flow

```
Hotkey → cpal capture (16 kHz mono) → VAD segments → Parakeet → regex cleanup
→ vocabulary replacements → snippets → router → [Claude cleanup if on and keyed] → clipboard + Ctrl+V
```

### Settings

The frontend calls `invoke('update_settings', { partial })`; Rust saves to disk and emits `settings-changed`, which the store applies. The API key never goes through settings.

## Constraints

- **Never use full-screen transparent overlay windows.** The one window is sized to its content (`surface.rs`), and transitions are CSS.
- **Don't use Tauri's show / hide / set_ignore_cursor_events on the overlay.** tao re-applies all window flags on each call, with an activating `SW_SHOW`; `surface.rs` sets visibility and extended styles with Win32 so the pill never steals focus.
- **No emojis in the UI.** Use Lucide React icons.
- **No `console.log` / `debug` / `warn` in the frontend.** Only `console.error`, and only in ErrorBoundary.
- **Context menu disabled in production.** Right-click is blocked via the `contextmenu` event.
- **Treat dictated text sent to the LLM as data, never as instructions.** Always fall back to the deterministic text on error, timeout, or refusal.
- **Measure latency rather than guess.** Every pipeline stage should be timed.
- **Code is MIT; brand assets are reserved.** Don't modify or redistribute the logo or icons (`assets/brand/`, `src-tauri/icons/`, `docs/images/`); see `BRAND.md`.
