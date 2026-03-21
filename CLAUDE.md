# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Chirp?

Chirp is a local-only voice-to-text desktop app built with Tauri 2. Users press a global hotkey, speak, and transcribed text is injected at their cursor in any application. All processing is on-device — no cloud, no accounts.

## Build Commands

### Prerequisites (Windows)

sherpa-onnx DLLs must be in `src-tauri/sherpa-onnx-lib/` (not committed to git). Download from https://github.com/k2-fsa/sherpa-onnx/releases — get the `win-x64-shared-MD-Release` archive, copy `lib/*.dll` and `lib/*.lib` into that directory.

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

Kill stale `node.exe` processes before restarting dev server if port 5173 is in use.

### Build

```bash
npm run build        # TypeScript check + Vite bundle (frontend only)
npx tauri build      # Full release build (frontend + Rust + installer)
```

### Lint

```bash
npm run lint         # ESLint
```

No test suite exists. Validation is manual via the dev server.

## Architecture

**Two-process Tauri app:** React frontend communicates with Rust backend via `invoke()` IPC calls.

### Frontend (src/)
- **React 19 + TypeScript + Vite** with **Tailwind CSS** styling
- **Zustand** single global store (`src/stores/appStore.ts`) — all UI state lives here
- **Two windows** defined in `tauri.conf.json`:
  - `overlay` — 300x64px, transparent, always-on-top, no decorations, click-through via `setIgnoreCursorEvents(true)` (the recording pill)
  - `settings` — 1400x900px, main settings/onboarding window
- Key UI areas: `components/overlay/`, `components/settings/`, `components/onboarding/`

### Backend (src-tauri/src/)
- `commands.rs` — All `#[tauri::command]` handlers (IPC surface between frontend and Rust)
- `audio.rs` — cpal microphone capture, resampling to 16kHz mono, amplitude extraction
- `transcribe.rs` — sherpa-onnx Parakeet ASR model loading and inference
- `cleanup.rs` — Regex-based text cleanup (filler words, punctuation)
- `llm.rs` — Optional AI cleanup via llama-server subprocess (downloads binary on first use)
- `hotkey.rs` / `hotkey_windows.rs` — Platform-specific global hotkey listeners (conditional compilation via `#[cfg(target_os)]`)
- `inject.rs` — Clipboard write + Ctrl+V/Cmd+V simulation via enigo
- `state.rs` — Shared app state (Settings, recognizer, LLM process handle)
- `settings.rs` — Disk persistence for settings.json, dictionary.json, snippets.json
- `file_transcribe.rs` — Batch transcription of audio files

### Data flow
```
Hotkey press → cpal audio capture (16kHz mono)
→ sherpa-onnx Parakeet model → raw transcript
→ regex cleanup → optional llama-server AI cleanup
→ dictionary replacements → clipboard + Ctrl+V injection
```

### Native libraries
- sherpa-onnx DLLs/dylibs live in `src-tauri/sherpa-onnx-lib/` (platform subdirectories: `windows/`, `macos/`)
- `build.rs` configures linker search paths per platform, with flat directory fallback
- Speech model and LLM model are downloaded at runtime to app data directory

## Key Design Constraints

- **Never use full-screen transparent overlay windows on Windows.** The overlay window is sized to tightly fit its content (300x64px); transitions are CSS-animated, not window-resized. Click-through is enabled via `setIgnoreCursorEvents(true)` so transparent areas don't block mouse interaction.
- **Overlay pill design is warm frosted glass** (`bg-white/90 backdrop-blur-xl` with amber border glow). Text is minimal: no text for listening (waveform only), no text for processing/polishing (spinner only), short word count for done. The bird icon never turns green — it stays amber across all states.
- **No cloud dependencies.** Everything runs locally. No analytics, telemetry, or user accounts.
- **Platform-conditional code** uses `#[cfg(target_os)]` in Rust and runtime detection in TypeScript.

## Spec Documents

- `APP.md` — Complete feature spec, UI screens, user flows
- `CHIRP-SPEC.md` — Tech stack and architecture overview
- `STYLE.md` — Design tokens, color system, typography, component specs
