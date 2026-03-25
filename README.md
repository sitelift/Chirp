<p align="center">
  <img src="docs/images/logo.png" width="80" height="80" alt="Chirp" />
</p>

# Chirp

**Free, local voice-to-text for Windows & macOS.**  
No cloud. No account. No subscription.





---

## What is Chirp?

Chirp turns your voice into clean text on your machine. You press a hotkey, speak, and formatted text drops at your cursor in whatever app you're using. Your audio stays on your computer. Nothing leaves.

The speech engine is NVIDIA's [Parakeet](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) model, running through [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx). The app shell is [Tauri 2](https://tauri.app) (Rust + React). No servers sit between you and your transcription.

## Features

- **Local processing.** Speech recognition and AI cleanup run on your hardware. Nothing phones home.
- **Works in any app.** Text appears at your cursor: email, Slack, docs, code editors, browsers.
- **Smart Cleanup.** A local LLM strips filler words, fixes grammar, adds punctuation.
- **Custom dictionary.** Add your team's jargon, acronyms, and names. Chirp spells them right.
- **Snippets.** Save phrases you use often. Insert them in one action.
- **History.** Browse, search, and re-use past transcriptions.
- **Global hotkey.** Customize the key combo. Works system-wide.
- **Lightweight.** Rust and C++ under the hood. You won't notice it running.

## Download

Get Chirp from the website:

**[chirptype.com/download](https://chirptype.com/download)**  
Windows 10+  ·  macOS 13+

Installers are also on the [Releases](https://github.com/sitelift/Chirp/releases/latest) page.

## How It Works

1. **Press the hotkey.** A small overlay pill appears.
2. **Speak.** Talk the way you talk. Filler words, run-on sentences, all of it.
3. **Text appears at your cursor.** Cleaned up, punctuated, ready to send.

Chirp downloads the speech model on first launch (~1.5 GB). After that, it works offline.

## System Requirements


|          | Minimum               | Recommended                  |
| -------- | --------------------- | ---------------------------- |
| **OS**   | Windows 10 / macOS 13 | Windows 11 / macOS 14+       |
| **RAM**  | 4 GB                  | 8 GB                         |
| **Disk** | ~1.5 GB               | ~2.5 GB (with cleanup model) |
| **CPU**  | x64 or Apple Silicon  |                              |
| **GPU**  | Not required          |                              |


## Building from Source

> Most people should [download Chirp](https://chirptype.com/download) from the website. This section is for developers who want to read or modify the code.

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable)
- [CMake](https://cmake.org/)
- [LLVM/Clang](https://releases.llvm.org/)
- [Vulkan SDK](https://vulkan.lunarg.com/) (Windows)
- sherpa-onnx shared libraries (see below)

### sherpa-onnx Setup

Download the `win-x64-shared-MD-Release` archive (Windows) or the macOS equivalent from [sherpa-onnx releases](https://github.com/k2-fsa/sherpa-onnx/releases). Copy `.dll`/`.dylib` and `.lib` files into `src-tauri/sherpa-onnx-lib/`. On Windows, copy the DLLs into `src-tauri/` as well for release bundling.

### Environment Variables (Windows, bash)

```bash
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export VULKAN_SDK="C:/VulkanSDK/<version>"
export CARGO_TARGET_DIR="C:/tmp/chirp-target"   # Short path avoids MAX_PATH issues
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
```

### Dev & Build

```bash
npm install
npx tauri dev          # Dev server + Rust backend
npx tauri build        # Release build (creates installer)
```

## Tech Stack


| Layer              | Technology                               |
| ------------------ | ---------------------------------------- |
| App framework      | Tauri 2 (Rust)                           |
| Frontend           | React 19 + TypeScript + Vite             |
| Styling            | Tailwind CSS                             |
| Speech recognition | sherpa-onnx + NVIDIA Parakeet TDT        |
| AI cleanup         | llama-server (local LLM)                 |
| Audio capture      | cpal                                     |
| Text injection     | enigo (clipboard + keystroke simulation) |


## License

Chirp's source code is available for inspection and personal use. See the [LICENSE](LICENSE) file for details.

---

[chirptype.com](https://chirptype.com)