# Chirp v2

Press a hotkey, talk, release. Cleaned-up text appears at your cursor.

Chirp is a Windows-only Tauri 2 dictation app. Hold Right Ctrl to talk or use
Left Ctrl + Left Win to toggle recording. Both bindings can be changed. The
small pill expands into settings, microphone selection, vocabulary, snippets,
and recent dictations.

## Install

Get the Windows x64 installer from [Releases](https://github.com/sitelift/Chirp/releases).
Windows 11 is the reference platform. Internet is required for the first model
download; speech recognition works offline afterward. No Python, CUDA toolkit,
or manual model setup is required.

- **CPU is supported.** Parakeet v3 int8 uses up to six CPU threads. The base
  installer includes the CPU runtime; first run downloads the speech model.
- **NVIDIA acceleration is optional.** A usable NVIDIA driver triggers background
  downloads of the CUDA runtime and Parakeet v3 FP32 model. Allow about 10 GB
  free during installation. The next restart probes and selects GPU acceleration.
  CPU remains available if the download or GPU initialization fails.
- **Optional Claude cleanup.** Bring your own Anthropic API key, stored in Windows
  Credential Manager. Claude Haiku 5.5 handles structure and self-corrections.
  Deterministic cleanup works without a key and on API errors or timeouts.
  There are no tone modes or local LLM downloads.

Warm offline ASR measured about 99 ms p50 on the reference RTX 4080 Super versus
317 ms with six CPU threads. These are synthetic-corpus ASR measurements, not
end-to-end guarantees. Cloud cleanup adds latency. See [bench/PERFORMANCE.md](bench/PERFORMANCE.md).

## Privacy

Audio is transcribed locally and never sent to Anthropic. With cleanup enabled
and a key present, Chirp sends transcript text, dictionary terms, and active
app/window/browser-domain context to Anthropic. It sends no screenshots or
surrounding document text. Model/runtime downloads contact GitHub, Hugging Face,
and NVIDIA packages on PyPI; update checks contact GitHub. No analytics, crash
reporting, or feedback webhooks are included.

History and settings stay on this computer. Development capture is **off by
default**. Explicitly enabling `captureDictations` or `CHIRP_CAPTURE_DICTATIONS=1`
saves audio, text, context, and timing locally in
`%APPDATA%/com.chirp.app/captures`. It adds no upload. Captures have no automatic
retention; remove sensitive recordings when no longer needed. See the
[performance notes](bench/PERFORMANCE.md#turn-on-real-world-capture).

## Build from source

Read [AGENTS.md](AGENTS.md) and [the v2 direction](docs/v2/DIRECTION.md).
Use Windows x64, Node.js 22+, stable Rust, Visual Studio 2022 Build Tools (Desktop
development with C++ and Windows SDK), CMake, and LLVM. Use an x64 Visual Studio
developer shell. WebView2 is required; the installer can bootstrap it if missing.

1. Run `npm.cmd ci`.
2. Download **1.12.36 Windows x64 shared MT Release** from
   [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.12.36).
   Copy `lib/*.dll` and `lib/*.lib` to `src-tauri/sherpa-onnx-lib/`, then copy the
   DLLs flat into `src-tauri/`. The release workflow pins the archive hash.
3. Generate the import library:
   `lib /DEF:src-tauri\sherpa-onnx-lib\onnxruntime.def /OUT:src-tauri\sherpa-onnx-lib\onnxruntime.lib /MACHINE:X64`.
4. Copy the x64 `Microsoft.VC143.CRT/*.dll` redistributables from your Visual
   Studio `VC/Redist/MSVC/<version>/x64/` directory into `src-tauri/`. These small
   app-local libraries support the optional CUDA runtime.
5. Set the environment (PowerShell):

```powershell
$env:PATH = "C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;$env:PATH"
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
$env:CARGO_TARGET_DIR = 'C:\tmp\chirp-target'
npx.cmd tauri dev
```

The short target path avoids MAX_PATH issues. Stop your stale dev instance if
port 5173 or the hotkey is occupied. The dev gallery is at
`http://localhost:5173/?preview=1`.

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
npm.cmd run build
npm.cmd run lint
# Unsigned local installer; production signing is configured separately:
npx.cmd tauri build --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

Set `CHIRP_ASR_PROVIDER=cpu` before launch to force CPU.
`chirp.exe --chirp-check-engine` loads and warms the model, prints status, and
exits without opening the app. GPU failures are remembered; `retry_gpu` clears
the failure and schedules missing downloads (restart to retry).
See [release and integration notes](docs/v2/RELEASING.md).

## Contribute and license

See [CONTRIBUTING.md](CONTRIBUTING.md). Code is [MIT](LICENSE). Logos, icons, and
brand artwork are reserved; [BRAND.md](BRAND.md) explains rules for forks.
