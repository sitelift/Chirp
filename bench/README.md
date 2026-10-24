# Chirp performance bench

Run from the repository root on Windows. The Rust binary calls the application's
actual ASR, chunk merging, deterministic cleanup and (with `-Vad`) Silero segmenter.
It never opens a microphone or pastes text. `corpus/` is owned by the corpus track;
the benchmark reads it without modifying it.

## Prepare and run

Rust, VS 2022 C++ Build Tools, CMake and LLVM must already be installed. Do not
install a CUDA toolkit: the setup script downloads the CPU/CUDA sherpa-onnx
1.12.36 releases and pinned NVIDIA Windows runtime wheels, verifies wheel hashes,
and extracts their DLLs. It does not run Python or pip.

```powershell
powershell -ExecutionPolicy Bypass -File bench/setup-native.ps1 -Fp32Model
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Provider cpu -Threads 6 -Iterations 3
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Provider cuda -Threads 4 -Iterations 3 -SkipBuild
```

`run.ps1` builds with `CARGO_TARGET_DIR=C:/tmp/chirp-target`, initializes the VS
developer environment, and copies the executable into separate `bench-cpu` and
`bench-cuda` runtime directories. **Changing PATH alone does not switch ONNX
Runtime**: Windows prefers DLLs adjacent to the executable.

Explicit models and optional stages:

```powershell
$models = "$env:APPDATA/com.chirp.app/models/sherpa"
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Provider cuda -ModelDir "$models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3" -Label parakeet-v3-fp32-cuda
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Provider cpu -Online -ModelDir "$models/sherpa-onnx-nemotron-speech-streaming-en-0.6b-80ms-int8-2026-04-25" -Limit 3 -Iterations 1 -Label nemotron80-cpu-paced
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Provider cpu -Vad "$env:APPDATA/com.chirp.app/models/silero_vad.onnx" -Limit 3 -Iterations 1 -Label parakeet-vad-paced
# ANTHROPIC_API_KEY must be in the process environment for this optional run:
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Llm -Limit 3 -Iterations 1 -Label parakeet-haiku
```

The equivalent direct command, after copying the desired runtime beside the EXE:

```powershell
C:/tmp/chirp-target/bench-cpu/bench.exe --manifest bench/corpus/manifest.json --provider cpu --threads 6 --iterations 3 --label baseline
```

Options: `--model-dir PATH`, `--online`, `--unpaced`, `--vad PATH`, `--llm`,
`--limit N`. All model directories must contain exactly one encoder, decoder and
joiner ONNX file plus `tokens.txt` (and any external weights referenced by ONNX).

## Interpreting results

- JSON goes to `bench/results/<UTC timestamp>-<label>.json`; stdout prints each
  file/run and aggregate p50/p95/max. Results are intentionally gitignored.
- The entire first corpus pass is retained but excluded from warm aggregates.
  Only the very first inference is marked `cold`; subsequent first-pass files
  may still trigger shape-specific runtime initialization. Model load is separate.
- WER is edit distance on lowercased, punctuation-stripped words against `spoken`,
  weighted by total reference words. It measures raw recognition, not cleanup
  quality. `ideal` and context stay in the manifest; context is used by `--llm`.
- Online mode feeds 80 ms chunks at wall-clock audio pace and adds 500 ms of
  zero-valued tail samples for finalization. Tail latency includes backlog past
  the scheduled end and all final decoding, including padding. `--unpaced` is a
  compute experiment, not a real-time latency claim.
- VAD mode feeds 10 ms chunks through the real segmenter and decodes completed
  segments between feeds. Its paced tail conservatively includes backlog from
  those synchronous decodes. The app uses a separate ASR worker.
- The corpus is already PCM16, 16 kHz mono; resampling is therefore a no-op.
  WAV reading, model load and deliberate real-time waits are reported separately
  from end-of-audio latency. No clipboard or microphone latency is inferred.
- GPU memory fields are **device-wide snapshots**, not isolated process usage
  or peak VRAM. Close competing workloads for stable comparisons.
- `--llm` prewarms before each item and uses a 2.5 s timeout. Errors are recorded
  and preserve deterministic text. It sends every benchmark output to cleanup;
  the live app skips keyed cleanup at six words or fewer.

## Live pipeline instrumentation

Each successful dictation has a `timings` history field. One `pipeline_timing`
JSON log is emitted when the background clipboard restoration attempt finishes;
the persisted history is then updated with that result. `clipboard_restored`
distinguishes restoration from a newer clipboard copy that must be preserved.
The 800 ms restore grace period is outside time-to-paste.

`release_to_paste_ms` ends at successful input dispatch, not a target app's paint
or acknowledgement. Hold mode starts at the raw key release; tap mode at its
second press. Command-triggered recordings use command receipt. `capture_start`
includes raw-press dispatch when available. Overlapping stages such as ASR drain
and per-chunk ASR are not additive; capture-stage VAD/resampling happen while the
user talks. The LLM record includes TTFT, total latency and cache/token usage.

The microphone and VAD stay warm. Idle callbacks discard audio. A callback gate
synchronizes stop/drain with any in-flight callback, eliminating the fixed 50 ms
sleep. Clipboard verification is immediate, eliminating its fixed 30 ms sleep.
ASR and deterministic-regex warmup run at model load.

App configuration uses `CHIRP_ASR_PROVIDER=auto|cpu|cuda` (default `auto`), `CHIRP_ASR_THREADS`, and
`CHIRP_ASR_MODEL_DIR`. Missing CUDA runtime dependencies fall back to CPU in the
app; the benchmark refuses this fallback so a CPU run is not mislabeled CUDA.
Native model/runtime crashes cannot be recovered by a Rust `Result`; do not
enable an export that has not passed the benchmark.

Auto selects v3 FP32/CUDA when both its files and CUDA DLLs are available; otherwise it uses v3 int8/CPU (up to six threads). To launch the app with the CUDA runtime after setup, set `SHERPA_ONNX_LIB_DIR` to the absolute `src-tauri/sherpa-onnx-lib-cuda` directory before `npx tauri dev`. The sherpa build script copies runtime DLLs next to the executable. CPU release bundling remains the portable default; CUDA installer packaging is not part of this performance change.
