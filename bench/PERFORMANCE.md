# Performance track — 2026-10-24

## Decision

Use **Parakeet TDT 0.6b v3 FP32 with CUDA**, with int8 CPU fallback. On this
Windows 11 / RTX 4080 SUPER / NVIDIA 617.42 machine, median offline ASR is 3.2×
faster than the best tested CPU configuration, with slightly lower raw WER.
Keep Silero on CPU and keep its session and the microphone warm.

These are measurements of the 45-file **synthetic SAPI corpus**, not human
dictation accuracy. The <200 ms release-to-paste target is not yet validated.

## End-to-end result (headline)

Full 45-file corpus, FP32 CUDA, prewarm before every item, initial pass plus two
warm passes. Six short commands skip Haiku; 39 items call it. The metric ends at
final text availability, **not clipboard dispatch**. No live paste cost is added.

| Stage (ms) | Skip p50 / p95 / max | Haiku p50 / p95 / max |
|---|---:|---:|
| **Release/end-of-audio to final text** | **52.53 / 55.41 / 55.41** | **895.85 / 1824.31 / 2637.24** |
| ASR | 52.47 / 55.34 / 55.34 | 106.49 / 326.43 / 358.70 |
| Deterministic cleanup | 0.068 / 0.085 / 0.085 | 0.088 / 0.128 / 0.142 |
| Vocabulary (empty) | <0.001 / <0.001 / <0.001 | <0.001 / <0.001 / <0.001 |
| Boundary repair, empty snippets | 0.003 / 0.004 / 0.004 | 0.008 / 0.016 / 0.017 |
| LLM, including failures | <0.001 / <0.001 / <0.001 | 778.30 / 1593.32 / 2504.33 |
| LLM TTFT, successes only | n/a | 553 / 1252 / 2129 |
| LLM service total, successes only | n/a | 778 / 1554 / 2277 |
| WAV read (outside release metric) | 0.330 / 0.580 / 0.580 | 1.256 / 3.800 / 4.195 |
| Resampling / VAD | 0 / 0 / 0 | 0 / 0 / 0 |

12 skipped and 78 Haiku warm observations. One Haiku timeout preserved the
deterministic text; it remains included in end-to-end and LLM distributions.
Raw ASR WER: 0% skipped, 11.84% Haiku group, 11.66% overall. Corpus is already
16 kHz; this baseline decodes the full waveform at release without VAD.
Startup warmup decode: 270.3 ms, excluded from release timing. Cloud latency
clearly dominates the routed end-to-end result; the <200 ms goal is not met
for Haiku-routed text. Prompt versions can change as the quality track develops.

Artifact: `results/20261024T212352.529Z-e2e-fp32-cuda-45.json`.
## ASR comparison

Full corpus, one initial pass plus two warm passes (90 warm observations).
Greedy decoding, no vocabulary or snippets. ASR includes stream creation,
feature acceptance, decode and result retrieval. LLM is disabled in this table.

| Model / provider | Threads | p50 ms | p95 ms | Max ms | Raw WER | Model load ms |
|---|---:|---:|---:|---:|---:|---:|
| v3 int8 / CPU | 2 | 463.5 | 1813.7 | 2268.3 | 12.14% | 2484 |
| v3 int8 / CPU | 4 | 383.4 | 1308.1 | 1680.8 | 12.14% | see JSON |
| v3 int8 / CPU | 6 | 317.2 | 1106.7 | 1475.5 | 12.14% | 2695 |
| v3 int8 / CUDA | 4 | 525.8 | 1540.9 | 1783.1 | 11.80% | 19046 |
| **v3 FP32 / CUDA** | **4** | **98.6** | **243.8** | **326.2** | **11.66%** | **4543** |
| v2 FP16 / CUDA | 4 | invalid | invalid | invalid | 100% on 12 completed files | 10039 |

The four-thread CPU row is the clean rerun, `*-cpu4-clean.json`; the earlier
exploratory run overlapped development rebuilds and is not the comparison row.
The FP16 run returned empty transcripts and then exited with native access
violation `-1073741819`. Its apparent speed is not usable performance.

The first FP32 CUDA decode of `chat-01` took 412.0 ms, versus 76.0 ms on the
next pass. The app now performs a decode at model load before accepting
dictation. A CUDA dev launch confirmed `requested=auto, selected=cuda` and
completed its startup warmup. Shape-specific kernel initialization can still
affect subsequent recordings of different lengths.

Device-wide GPU memory in the FP32 run rose from 3304 MiB before loading to
6806 MiB after loading and 6853 MiB after decoding: roughly 3.5 GiB additional
steady allocation. These snapshots are not isolated process measurements or
peak VRAM. Int8 CUDA is deliberately not the default despite being smaller.

## Streaming screening

Nemotron export: `sherpa-onnx-nemotron-speech-streaming-en-0.6b-80ms-int8-2026-04-25`.
Three files (`chat-01` through `chat-03`), one initial and one warm pass. Feed
80 ms chunks in real time, then finalize with 500 ms of zero padding; padding
compute and any accumulated backlog count toward the tail.

| Provider | Finalization p50 ms | p95/max ms | Raw WER |
|---|---:|---:|---:|
| CPU, 4 threads | 358.6 | 377.3 | 43.18% |
| CUDA, 4 threads | 1914.3 | 2192.5 | 43.18% |

This screening rejects this particular model/runtime configuration; it does
not establish that every Nemotron export or chunk size is unsuitable. Other
official exports include 160/560/1120 ms Nemotron and streaming Parakeet Unified.

## VAD and cloud cleanup checks

Three-file paced Parakeet FP32/CUDA + CPU Silero run:

- VAD compute p50/p95: 84.0/144.3 ms **spread across 8.6–14.2 seconds of audio**.
- VAD session creation p50: 7.36 ms; the live app now caches this session.
- Warm final ASR tail p50: 0.44 ms because these synthetic files include enough
  trailing silence for the segment to finish before end-of-file. This is not a
  prediction for a person who releases immediately after the last syllable.
- Raw WER on that subset: 10.23%.

The optional `--llm` run used the same three files, FP32/CUDA, and the connected
Anthropic key. All six requests succeeded, each reading 2885 cached input tokens.
On the three warm requests, cleanup p50/p95 was **710.9/726.0 ms**, TTFT was
498/500/575 ms, and ASR → final text p50/p95 was **822.0/846.8 ms**. This excludes
microphone and clipboard operations. These are point-in-time prompt measurements;
the prompt is being tuned independently by the LLM track.

## Before / after pipeline evidence

| Stage | Before | After |
|---|---|---|
| Microphone | Opened per press; destroyed at release | Opened at startup, idle callbacks discard audio; reset and gate each recording |
| Capture drain | Stream destruction plus unconditional 50 ms sleep | Callback gate; no fixed sleep |
| Resampling / VAD | Already incremental during capture | Still incremental; flushed resampler tail reaches VAD and its partial final window is preserved |
| Silero initialization | Reloaded per press | Cached and reset with a fresh segment sender |
| Offline ASR, full corpus | Best CPU p50/p95 317.2/1106.7 ms | FP32 CUDA 98.6/243.8 ms |
| Deterministic cleanup | First use compiled regexes during dictation | Regexes warmed at model load; warm FP32 run p50/p95 0.092/0.143 ms |
| Vocabulary (empty) | Same logic | p50 about 0.0002 ms; nonempty vocab not benchmarked |
| Clipboard verification | Unconditional 30 ms sleep | Synchronous write/close, immediate verification; retries only on failure |
| Clipboard restoration | Background 800 ms grace | Same grace, timed separately; skips restoration if clipboard changed |
| “Total” log | Measured before injection | Monotonic raw input → successful input dispatch; restoration completion recorded separately |

The historical `45 ms ASR / 236 ms total` entry is real, but its log does not
contain enough stage detail to attribute the entire remaining 191 ms. The
removed capture sleep accounts for 50 ms; the additional 30 ms clipboard sleep
was **outside** that old total. Do not claim that subtracting 80 from 236 gives
the new live latency. New history/log records make the remaining attribution
possible during actual dictation.

## Measured capture overhead (release build)

Eight real default-device captures using the app's capture/resampler code,
without UI, VAD or ASR. Waveforms were discarded.

| Stage | p50 ms | p95 / max ms |
|---|---:|---:|
| Cold microphone open | 13.92 | 25.25 |
| Cold first sample | 233.52 | 246.49 |
| Legacy stream destruction | 126.48 | 138.46 |
| Warm callback activation | 0.0004 | 0.0095 |
| Warm first sample | 9.56 | 10.49 |
| Warm stop gate | 0.0001 | 0.0005 |
| Resampler tail flush | 0.0004 | 0.0015 |

Stream destruction plus the removed 50 ms sleep explains about 176 ms of the
historical 191 ms non-ASR gap on this device, although these are separate runs.
The removed 30 ms clipboard wait was additional to the historical total.
This is lifecycle evidence, not a measured live key-release-to-paste result.
Artifact: `results/20261024T211227.366Z-mic-lifecycle.json`.
## Build and validation

- Installed tools were detected as winget finished; no reinstall was attempted.
- PowerShell blocks `npm.ps1` here, so commands used `npm.cmd` / `npx.cmd`.
- VS developer environment, CMake/LLVM PATH, `LIBCLANG_PATH`, and the short
  `CARGO_TARGET_DIR=C:/tmp/chirp-target` were needed.
- CPU MT and CUDA 12/cuDNN 9 Windows archives exist for sherpa-onnx **1.12.36**;
  the crate did not need a version bump. CPU libraries and flat bundling copies
  are present; CUDA libraries are isolated in `src-tauri/sherpa-onnx-lib-cuda`.
- CUDA dependencies came from Windows wheels: cublas 12.9.2.10, cudart 12.9.79,
  cuFFT 11.4.1.4, cuDNN 9.27.0.42. No CUDA toolkit was installed.
- `npm install`, frontend build/lint, Rust checks, 61 Rust tests, release bench
  build, and CPU/CUDA `npx tauri dev` startup were verified.

## Remaining validation

1. Real maintainer recordings and live key-release-to-paste traces. WAV results
   cannot establish target-app paste/paint latency or first-key reliability.
2. Long/tail recordings: offline full-corpus p95 is still above 200 ms. Measure
   how much the existing VAD worker removes from the live release path.
3. CPU fallback on a machine without NVIDIA hardware, device disconnect/reopen,
   rapid cancel/restart and clipboard behavior in target apps.
4. CUDA installer packaging and model-download UX. The development runtime and
   optional setup script work; the portable installer still uses CPU DLLs.

See [README.md](README.md) for exact reproduction commands and measurement
definitions. JSON artifacts are in `results/` (gitignored).

Sources: [sherpa 1.12.36 binaries](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.12.36),
[official ASR exports](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models),
[maintainer's v3 FP32 export](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3).
