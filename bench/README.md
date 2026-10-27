# Chirp performance bench

Run from the repository root on Windows. The Rust binary calls the application's
actual ASR, chunk merging, deterministic pass (segment join + spoken commands) and (with `-Vad`) Silero segmenter. The default offline mode uses the production
complete-recording path; `-Vad` reproduces the old segmented path for comparison.
The WAV benchmark never opens a microphone or pastes text. `corpus/` is owned by the corpus track;
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

For a separate device lifecycle measurement, run:

```powershell
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -MicLifecycle
```

This explicitly opens the default microphone for short captures, discards all
audio, and measures cold open/first-sample/drop versus warm activation,
first-sample arrival and callback synchronization. It performs no ASR or paste.
The direct binary flag is `--mic-lifecycle`.

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
  and preserve deterministic text. The benchmark and app skip cleanup at six words or fewer.

## Live pipeline instrumentation

### Saved-audio diagnostics without reference transcripts

`asr-study` compares exact saved chunks, complete recordings and VAD replay
without treating historical ASR or pasted text as a ground-truth transcript.
Build it in the same VS/native environment as the regular bench:

```powershell
cargo build --release --manifest-path src-tauri/Cargo.toml --features bench --bin asr-study
# Copy asr-study.exe beside the appropriate CPU or CUDA runtime DLLs, as run.ps1 does for bench.exe.
C:/tmp/chirp-target/bench-cuda/asr-study.exe --manifest bench/corpus/audio.local.json --out bench/results/recording.json --mode recording --passes 2
```

The JSON manifest is an array of `{ "id": "example", "wav": "relative/path.wav" }`,
with paths relative to the manifest. An optional `snippets` array accepts the
app's `{ "trigger": "...", "expansion": "..." }` entries, so the output includes
the actual spoken-command and snippet stages. Use mono 16 kHz PCM16 or float32
WAVs. Keep manifests, audio and text results in gitignored local paths.

Modes: `recording` calls the production complete-audio path; `file` decodes
one whole WAV without a duration bound; `vad` feeds the real segmenter and
segment recognizer. Explicit `--provider`, `--model-dir`, `--threads` and
`--beam` select the recognizer. Defaults are CUDA, FP32 and four threads;
for CPU, pass the int8 model directory and `--provider cpu --threads 6`.
Pass zero warms shapes; retain it but use subsequent passes for timing.
The tool refuses to overwrite an output file and records manifest and compiled
ASR/VAD source hashes. VAD feeding is unpaced; its wall time is compute cost,
not live release latency. No audio is uploaded and no text is pasted.

Prompt experiments must freeze deterministic text, context and dictionary
before comparing candidates. Changing ASR creates a new corpus version; do
not attribute that improvement to the prompt. Repeat promising variants and
review their actual outputs, especially claims, names and unspoken additions.

### Timing fields

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
app and auto benchmark mode; explicit benchmark providers refuse fallback.
Native model/runtime crashes cannot be recovered by a Rust `Result`; do not
enable an export that has not passed the benchmark.

Auto selects v3 FP32/CUDA when both its files and CUDA DLLs are available; otherwise it uses v3 int8/CPU (up to six threads). To launch the app with the CUDA runtime after setup, set `SHERPA_ONNX_LIB_DIR` to the absolute `src-tauri/sherpa-onnx-lib-cuda` directory before `npx tauri dev`. The sherpa build script copies runtime DLLs next to the executable. CPU release bundling remains the portable default; CUDA installer packaging is not part of this performance change.

Default bench mode selects FP32 CUDA when available, with CPU fallback. Use -Llm for routed end-to-end cleanup and -MinSeconds 15 -MaxSeconds 45 for long clips. JSON groups_warm separates skip/Haiku routes; empty groups have no latency distribution. End-of-audio-to-final excludes clipboard dispatch and WAV loading. Model-load and warmup are separate.

Full routed e2e run (45 files, initial pass plus two warm passes):
```powershell
# ANTHROPIC_API_KEY is already set in this shell; do not put it in command history.
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Provider cuda -Iterations 2 -Llm -Label e2e-fp32-cuda-45
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Provider cuda -Iterations 2 -Llm -Vad "$env:APPDATA/com.chirp.app/models/silero_vad.onnx" -MinSeconds 15 -MaxSeconds 45 -Label e2e-long-incremental
```
## Speculative cleanup at pauses

The app uses the router on each deterministic prefix after a closed VAD segment.
Only one speculative request can be in flight. Newer prefixes coalesce; release
seals the queue before the final VAD flush. An exact final-input match reuses the
completed result or waits for the remaining request. A mismatch cancels the stale
request and uses ordinary release-time cleanup. Matching errors use deterministic
text, just like release-time errors. History includes hit/miss, calls, stale
completions, release wait, and observed request time overlapped with recording.
`time_saved_ms` measures that overlap; it is not a promise about a hypothetical
second request's network latency.

```powershell
# ANTHROPIC_API_KEY must already be set in this shell.
$vad = "$env:APPDATA/com.chirp.app/models/silero_vad.onnx"
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Provider cuda -Iterations 1 -WarmOnly -Llm -CompareSpeculation -Vad $vad -Label spec-45-original
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -SkipBuild -Provider cuda -Iterations 1 -WarmOnly -Llm -CompareSpeculation -Vad $vad -TrailingSilenceMs 50 -Label spec-45-trim50
```

`-CompareSpeculation` creates paired controls with shared, paced audio/ASR.
At release it records independent completion times for ordinary cleanup and
speculative reuse. A stale speculation uses the same ordinary request as the
control, avoiding duplicate identical fallback calls. Matching speculations and
controls may have concurrent cloud calls: this measures paired paths, not an
isolated network service latency test. Paired waiting does not inflate the
speculative path's recorded completion time. Use `-Speculate` instead for an
unpaired app-like run; omit both flags for ordinary cleanup.

`-TrailingSilenceMs 50` removes only trailing digital-zero PCM samples, retaining
50 ms after the last nonzero sample. WAV files remain unchanged. `-WarmOnly`
omits the initial full-corpus pass; startup model/regex warmup still runs, but
shape-specific cold effects can remain. Set `-Iterations 2` for two measured
passes. Source fingerprints identify the loaded manifest and compiled prompt
and client. `groups_warm.haiku` contains the routed comparison, hit/call counts,
and errors for both paths. Paste dispatch remains outside this harness.
## Real-world dictation captures (opt-in, local)

Capture is off by default. Launch from a shell with
`$env:CHIRP_CAPTURE_DICTATIONS = '1'`, or set `captureDictations: true` in the
app's settings.json while the app is closed. An environment value of `0` forces
it off. The files go to `%APPDATA%/com.chirp.app/captures/<timestamp>-<id>/`.
There is no capture UI or automatic retention.

- `audio.wav`: complete 16 kHz mono float32 recording after resampling, preserving
  pauses and sample precision.
- `asr-input-NNN.wav`: exact float samples supplied to each ASR call, preserving
  VAD segmentation and any chunk overlaps.
- `meta.json`: completion marker with raw ASR, deterministic text, router choice,
  final/pasted text, selected LLM output/error, resolved context, dictionary terms,
  settings used, speculation use and timings. Pasted means input was dispatched,
  not that a target app acknowledged it. Failed/cancelled captures are marked.

To replay every capture's `audio.wav` through the real VAD and
`transcribe_segment` (no cloud call, no paste), build `--bin replay` with
`--features bench`, put it beside the runtime DLLs like `bench.exe`, and run
`replay.exe --provider cuda [--beam] [--limit N] [--passes 2] --label NAME`.
It writes per-segment text and ASR ms plus `spoken_text` to
`bench/results/replay-NAME.json` (gitignored; it contains dictated text).

Writes happen in the background; an interrupted write can leave an incomplete
folder. Capture adds no upload or telemetry and does not alter normal cloud
cleanup routing. The dictionary list is snapshotted once per recording and
shared by prewarm, speculation and release cleanup.

```powershell
node bench/tools/import_captures.mjs --list
node bench/tools/import_captures.mjs "<selected-folder-name>"
```

Only explicitly selected captures are imported. The default output is the
**gitignored** `bench/corpus/manifest.local.json`, initially copied from the
tracked manifest; WAVs are copied unchanged to gitignored `wav/human-*.wav`.
Existing manifest entries remain byte-identical. The importer never commits,
transmits, or deletes source captures. An incomplete/unpasted selection is
rejected. Re-importing an unchanged capture is a no-op.

`spoken` starts as raw ASR, `ideal` as pasted text, and `needs_review` is true.
Correct both references against the recording, then set `needs_review` to false.
The harness refuses to score unreviewed drafts: ASR-derived text is not ground
truth. Imported context and dictionary terms are reused by the LLM benchmark.

```powershell
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Manifest bench/corpus/manifest.local.json -Provider cuda -Iterations 2 -Llm
node --test bench/tools/import_captures.test.mjs
```

For an isolated import/test location use `--root`, `--manifest`,
`--base-manifest` and `--wav-dir`. Audio may be PCM16 or float32, always mono
16 kHz. Corpus file conversion is not performed.
