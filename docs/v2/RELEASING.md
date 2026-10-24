# Windows v2 release and engine integration

## Runtime and downloads

The installer contains CPU MT sherpa-onnx 1.12.36 DLLs and the small Microsoft
VC143 app-local CRT redistributables beside `chirp.exe` (CUDA ORT needs these).
Never bundle the CUDA directory or wildcard development DLLs. The benchmark
binary lives at `bench/native.rs` and requires the `bench` Cargo feature,
avoiding Tauri 2.10.1 auto-discovery of `src/bin`, so NSIS does not ship development
tools; `bench/run.ps1` enables that feature explicitly. `downloads.json`
pins sizes, URLs and SHA-256 for int8, VAD, CUDA sherpa, NVIDIA wheels and FP32.
Versions match `bench/setup-native.ps1`: cuBLAS 12.9.2.10, cudart 12.9.79,
cuFFT 11.4.1.4, cuDNN 9.27.0.42. FP32 is pinned to upstream revision
`1a468a35cbba69418f126de829e75261dea4a4e4`.
Review upstream redistribution licenses before publishing; extracted runtime
license files are retained with the runtime.

Files live under `%APPDATA%/com.chirp.app/models`. Interrupted `.part` files
resume via HTTP Range; a server ignoring Range restarts the transfer. Bad ranges,
sizes and hashes fail closed. Extraction rejects traversal and links. Verified
staging directories are promoted atomically. Disk checks cover downloads and
staging. CPU is installed first, and optional GPU downloads run in the background.

NVIDIA detection uses the system CUDA driver API (`cuInit`, `cuDeviceGetCount`).
Activation requires **next restart/model load** because an already-loaded ONNX
runtime cannot safely be swapped. MSVC delay-load imports allow absolute-path
preloading of the selected runtime before any audio/VAD calls. This is the
intentional deviation from immediate hot activation after download.

A hidden child constructs and warms CUDA with a 90-second timeout. Native crash,
timeout, or initialization failure leaves `cuda-failure.txt`; selection stays
on CPU until explicit retry. A guard also covers unexpected termination during
main-process model initialization. `CHIRP_ASR_PROVIDER=cpu` forces CPU; `auto`
and `cuda` both preserve fallback. `CHIRP_ASR_THREADS` overrides the CPU thread
count (default up to six). Benchmarks retain explicit provider configuration.

For isolated installation checks, set `CHIRP_MODELS_DIR` to an empty test folder.
`--chirp-download-cpu` and `--chirp-download-gpu` run the production verified
downloader without UI. `--chirp-check-engine --wav <16kHz-mono-PCM16.wav>` loads,
warms, transcribes and exits nonzero on error or empty text. These diagnostic
commands do not change settings or send cloud requests.

## Frontend contract (engine UI is a separate task)

- `get_engine_status`: `{active: null|"cpu"|"gpu", model, gpuAvailable,
  gpuDownloaded, restartRequired, failure}`. Active stays null until warmup.
- `retry_gpu`: clears failure, schedules missing downloads, returns status.
  Restart when ready. Failed GPU initialization is never retried every launch.
- `get_download_status`: latest progress snapshot or null; query on mount.
- `engine-download-progress`: `{group, file, phase, downloaded, total, error}`.
  Groups: cpu, vad, cuda, fp32. Phases: downloading, verifying, extracting, ready,
  error. Byte counts are per file, not the entire multi-file download.
- `engine-status`: refreshed status after GPU download completes.
- `model-download-progress`: existing numeric CPU progress remains compatible.
  `download_model` installs CPU and VAD, then loads the recognizer immediately.

## Signing and publishing

The existing checked-in updater public key is not secret. Keep it only if the
maintainer owns its matching private key. To change release identity, generate
with `npx tauri signer generate`, then replace `plugins.updater.pubkey` with the
public value. Never commit private keys. Configure GitHub repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`: `<private key from the maintainer's secret store>`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: `<password if set>`
- `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`: signing credentials
  for the account/profile in `tauri.release.conf.json`.

Updater signing and Authenticode are separate. CI signs the application and NSIS
with Azure, then updater-signs the final installer bytes. Local unsigned builds
use the README override to disable updater artifacts. The base config needs no
production credentials.

1. Validate CPU/NVIDIA clean installs, hotkeys, microphone, paste/clipboard
   restore, interrupted downloads and offline/error fallback.
2. Run `node scripts/set-version.cjs 2.x.y`, commit version/lock changes, and tag
   `v2.x.y` through the maintainer's normal process.
3. Windows CI creates a **draft** with NSIS EXE, `.sig`, and `latest.json`
   containing only `windows-x86_64`. Review installer size and signatures.
4. Validate a signed upgrade, then publish as latest stable. The workflow
   intentionally excludes prereleases; use a separate test feed for betas.
5. Test from a lower-version installation. The frontend checks after 30 seconds,
   downloads, then calls `set_update_ready`. The tray's “Restart to update” emits
   `update-install`; the frontend calls `Update.install()` and relaunches.
   Windows may exit during installer handoff. Failed checks/downloads never
   advertise readiness.

The stable feed remains
`https://github.com/sitelift/Chirp/releases/latest/download/latest.json`.
Never publish unsigned installers there. Preserve the key across updates or
plan a key-rotation migration. Signed update handoff needs a maintainer release
test, not merely a local build.

References: [Tauri updater](https://v2.tauri.app/plugin/updater/),
[MSVC delay loading](https://learn.microsoft.com/en-us/cpp/build/reference/delayload-delay-load-import).
## Implementation verification (2026-10-24)

- Before the final benchmark packaging change, 121 library tests and the
  benchmark test passed; one interactive browser-context test was ignored.
  After moving/gating the benchmark, both test targets compile, but Windows
  Application Control blocks execution with OS error 4551, including a normal
  retry. No security settings were changed. Final `cargo build` passes.
  Download tests cover valid resume,
  servers ignoring Range, wrong Content-Range, and hash mismatch rejection.
- `cargo build`, `npm.cmd run build`, and `npm.cmd run lint` passed on Windows.
- A fresh isolated `CHIRP_MODELS_DIR` downloaded int8 and VAD over HTTPS,
  checked the pinned hashes, installed, warmed and transcribed a corpus WAV.
- The production GPU installer verified/extracted the cached upstream archives
  and FP32 files, then selected FP32/CUDA and transcribed the same WAV.
- An actual native cuDNN probe crash during development returned to working CPU
  dictation and persisted its failure. The following launch skipped CUDA.
  After fixing cuDNN's secondary DLL search and clearing the marker, GPU passed.
  Forcing CPU also passed with the complete GPU pack present.
- The full unsigned Tauri/NSIS build and final rebundle succeed. The installer
  is **10,699,984 bytes (10.20 MiB)**. Inspection of the generated NSIS script
  confirms no CUDA DLLs or benchmark binary are bundled. The release executable
  staged with exactly the installer resources transcribes on both CPU and GPU.
  No signing credentials were used.

Still requires a human: a real machine without NVIDIA drivers/developer tools,
real microphone/hotkey/paste dogfooding, and a signed release plus tray-driven
upgrade from an older installed version. CPU simulation is not a clean-machine
installer test. GPU artifacts were verified from cache, not all redownloaded
from the internet during this run. Brand files were not changed.
