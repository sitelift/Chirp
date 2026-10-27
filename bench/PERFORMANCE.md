# Performance track

## Human dictation replay and cleanup — 2026-10-25

134 saved human recordings were frozen for comparison. Replaying exact saved
ASR chunks reproduced quiet-word loss: a missing negation changed the meaning
before cloud cleanup. Full-recording ASR recovered that negation on both CUDA
FP32 and CPU int8. Adding real lead-in recovered it; adding silence did not.
These observations establish an audio-boundary defect, not a prompt failure.

Production now decodes the complete recording, retaining all samples. Clips up
to 90 seconds stay together. Longer recordings split near sustained low-energy
regions with one second of overlap. No VAD gap is discarded. VAD only supplies
optional speculative cleanup; its text must exactly match final deterministic
text before reuse. With speculation off, no prefix ASR worker runs.

| Measurement | Result |
|---|---:|
| Real clips, CUDA FP32 greedy, four threads | 134, up to 68.96s |
| Repeated raw transcripts | 134/134 identical, none empty |
| Warm complete-recording ASR p50 / p95 / max | 89 / 440 / 662ms |
| CPU int8, six threads, confirmed negation clip | 1135ms |
| CPU int8, two sampled 62–69s clips | 7236 / 2983ms |
| Constructed 162s and 189s stress clips, final warm ASR | 920 / 593ms |

The stress clips concatenate existing recordings with one-second quiet gaps;
they are not new human recordings. They exposed loss at fixed long-window
boundaries and duplicate tail syllables when a cut used the beginning of a
pause. The final boundary selection retained the constituent messages and all
six repeated negations in the repeated-clip case. Sample-coverage tests verify
that quiet gaps, overlap and the last sample survive chunking.

These are local compute times, not live hotkey-to-paste latency. First passes
warm shapes and are excluded from timing. Real audio has no complete manually
corrected reference, so there is no WER claim. Two unclear passages remain
unconfirmed because the speaker could not recall their intended wording.

Prompt comparisons use identical deterministic text, context and dictionary
within each round. ASR revisions create a new frozen input version; improvements
from recovered audio are not attributed to prompt changes. This investigation
generated another 2,278 cleanup outputs after the earlier 5,507-output prompt
study. The final text corpus has 134 inputs. Final comparisons used two repeats
per candidate, Haiku 5.5 with thinking disabled and low effort, three concurrent
requests, no explicit prewarm, and a 15s measurement ceiling. The app's 2.5s
deadline is reported separately:

Every candidate input was sent to cleanup for comparison. The application's
router is unchanged and can still skip cleanup for short plain dictations.

| Candidate, same final inputs | Outputs | API p50 / p95 | Over 2.5s |
|---|---:|---:|---:|
| Existing prompt, plain text | 268 | 783 / 1553ms | 1 |
| Revised editing prompt, plain text | 268 | 797 / 1717ms | 1 |
| Revised editing prompt, one-field JSON (selected) | 268 | 1180 / 1834ms | 2 |
| Short faithful prompt + final reminder, one-field JSON | 268 | 1256 / 2140ms | 5 |

The plain revised prompt was rejected after it appended its own editing notes.
The selected combination used the same revised prompt with a JSON `text` field;
both repeats preserved the confirmed negation, rendered the dictated email,
and avoided unspoken sign-offs and editorial asides in the inspected failures.
The field accepts arbitrary message text: there is no phrase filter, content
classifier or repair request. Empty/error/truncated responses retain the
existing deterministic fallback. JSON parsing is a transport concern, not a
claim that the text is correct. An actual API smoke check confirmed one-token
prewarm succeeds; zero-token structured prewarm returned HTTP 400.

Remaining defects include inconsistent name spelling and interpretation of
ambiguous ASR such as an `OF` provider option. Stronger paraphrasing sometimes
reverses claims, and even the selected prompt can guess at ambiguous wording.
The corpus was used for iteration, so these results are not independent
validation and do not establish perfection. Automated anonymous-label review
was used to find defects, then concrete outputs were inspected; judge scores
are not treated as human ground truth. OpenAI live cleanup was not benchmarked.

Private artifacts remain gitignored in `asr-study.local`: `initial-file.json`,
`lead-file.json`, `full-corpus-recording90.json`, `full-corpus-final.json`,
`cpu-recording90.json`, `stress-final.json`, `corpus-final.json`,
`final_confirmation-results.json`, and `structured_comparison-results.json`.
The reusable local audio replay tool is `bench/asr_study.rs` (see README).

Final verification: 142 backend tests passed, two device-interaction tests
remained ignored, the release app built, and three prewarmed API smoke calls
completed without errors. The new local executable passed the installed CUDA
runtime probe and started with FP32/CUDA. An existing interrupted-startup GPU
failure marker was backed up and cleared after the successful probe. The
updated app is running from `asr-study.local/app/Chirp.exe`; no installer or
remote release was published. Live microphone-to-target-app paste latency has
not been measured for this build.

## Historical synthetic benchmark — 2026-10-24

## Final state: collect real dictations before more tuning

The SAPI corpus is a development fixture, not representative human speech.
Performance tuning stops here per the user's direction; hotword measurement is
deferred. Opt-in local capture and a private draft importer are ready.

- `ceaade6`: merged the completed global hotkey rewrite into v2.
- `fe03a02` / `ad6ab4a`: one speculative cleanup request at a time, coalescing,
  exact-input reuse, and no queued replacement starting after release.
- `53889a0`: capture browser context at press; resolve once at the first cleanup
  request or release; reuse it; prewarm UI Automation at startup.
- `922a45e`: default-off local capture, plus one frozen dictionary term list for
  prewarm, speculative and release-time calls. The same list is in metadata.
- `407fa09`: private human-draft importer, float-audio replay, and draft-reference
  scoring guard. No pushes were made.

Final validation: **111 Rust tests passed**, one interactive context test ignored;
**two importer tests passed**; `npm run build` passed. The earlier Application
Control test-executable block did not recur on these final builds. No security
settings were changed. Live capture/paste behavior still needs human dogfooding.

## Final paired speculation comparison

45 clips per variant, 39 Haiku items, one measured paced pass per variant after
startup warmup. These results end at **final text availability, excluding paste
dispatch**. Both variants use the same compiled prompt/client and the same
45 original inputs. The interrupted trimmed run was restarted using a recovered
manifest whose source fingerprint exactly matched the original run; only WAV
paths were made absolute. Its local snapshot is `results/spec-45-manifest.json`.

| Ending | Without speculation p50 / p95 / max ms | With speculation p50 / p95 / max ms | Exact-match hit rate | Speculative calls / Haiku item |
|---|---:|---:|---:|---:|
| Original quiet | 779 / 1430 / 1710 | 661 / 1609 / 1710 | 30/39 (76.9%) | 47/39 (1.21) |
| Digital silence trimmed to 50 ms | 996 / 1991 / 2151 | 996 / 1991 / 2151 | 0/39 (0%) | 17/39 (0.44) |

No cleanup errors occurred in either path. Excluding unchanged prewarm calls and
the benchmark-only paired controls, production-equivalent cleanup requests are
56/39 = **1.44 per Haiku item** in each variant (speculative calls plus misses).
The original-ending median improved by 118 ms, but p95 did not improve. This is
not evidence of reliably instant cloud cleanup. The unchanged 700 ms Silero
pause threshold limits the available head start; near-immediate release had no
hits. One pass and variable cloud response times limit tail conclusions.

Paired controls share audio/ASR. On hits they issue a separate ordinary request
at release while the speculative path is independently timed; on misses both
paths use the same release-time request. Thus misses have identical measured
latency by construction. Earlier work occurred while talking and is excluded
from the release metric. See README for the exact harness semantics.

Artifacts:
- `results/20261024T215850.552Z-spec-45-original.json`
- `results/20261024T221223.255Z-spec-45-trim50.json`
- Prompt fingerprint: `fnv1a64:bb7976741d1b5922`
- Client fingerprint: `fnv1a64:967528f8c43041b5`

The quality track subsequently changed the prompt and added dictionary-aware
cleanup. These are historical measured snapshots, not fresh latency claims for
every subsequent prompt edit.

## Speculation default and capture hygiene (2026-10-24)

Speculative cleanup at VAD pauses is now **off by default**, including existing
settings files. Opt in with `"speculativeCleanup": true` (Rust/load alias:
`speculative_cleanup`). The paired result above improved p50 only with a quiet
ending (779 ? 661 ms), worsened p95 (1430 ? 1609 ms), and had no hits with
near-immediate release. With the setting off, no coordinator is created, no
VAD prefix is prepared for cloud cleanup, and release uses the ordinary cleanup
call without speculative waits. Normal press-time prewarm remains unchanged.
The opt-in coordinator and its tests remain available for human-data evaluation.

Capture retention runs in background work at startup (even when capture is off)
and after each save. Settings `captureRetentionDays` (default 14) and
`captureMaxBytes` (default 2147483648, 2 GiB) also accept snake-case names.
Expired captures are removed first, then oldest folders until under the size cap;
age/order use folder modification time. Zero days expires past captures and
zero bytes removes nonempty captures. Cleanup errors only log and may leave the
folder over budget until a later pass succeeds. Retention settings are frozen
for each recording; restart applies edits made directly in settings.json.

Each save writes into a sibling `.<capture-name>.tmp` directory, then atomically
renames the completed folder into place. Startup removes stranded temp folders;
background writes and maintenance are serialized, outside the dictation path.
The importer skips temp folders and captures missing metadata or referenced
audio, including interrupted captures made by older versions. Existing complete
captures remain importable. No additional dictation latency benchmark was run
for these lifecycle changes; live dogfooding remains necessary.

Validation: `cargo test` passed 117 library tests and one benchmark test (one
interactive context test ignored); `cargo build`, `npm.cmd run build`, and all
four importer tests passed. Rust used `C:/tmp/chirp-target-hygiene`.

## Turn on real-world capture

Default is off. In the shell launching the app:

```powershell
$env:CHIRP_CAPTURE_DICTATIONS = '1'
npx.cmd tauri dev
```

Alternatively, close the app, set `"captureDictations": true` in
`%APPDATA%/com.chirp.app/settings.json`, and restart. The Rust field is
`capture_dictations`; that snake-case spelling is also accepted when loading.
Set the environment override to `0` to force capture off on the next launch.

Each folder under `%APPDATA%/com.chirp.app/captures/<timestamp>-<id>/` contains
the full resampled `audio.wav` (16 kHz mono float32, no requantization), exact
per-recognizer-call `asr-input-NNN.wav` files, and `meta.json`. Metadata includes
raw/deterministic/final/pasted text, router decision, selected LLM output/error,
resolved context, dictionary terms, configuration, speculation use and timing.
The full waveform preserves pauses for replay; exact input files preserve VAD
segmentation and overlapping chunks. Successful metadata includes the eventual
clipboard-restoration timing. Failure/cancel captures are marked accordingly.

```powershell
node bench/tools/import_captures.mjs --list
node bench/tools/import_captures.mjs "<selected-folder-name>"
```

The importer copies audio into `bench/corpus/wav/human-*.wav` and appends to
`bench/corpus/manifest.local.json`, initially copied from the normal manifest.
Both are gitignored. Existing entries remain byte-identical; the tracked
manifest is untouched. `spoken` starts as raw ASR, `ideal` as pasted text, and
`needs_review` is true. Correct both references and set it false before scoring.
Context and dictionary terms are preserved. Capture/import adds no network
operation; normal user-enabled cloud cleanup is unchanged.

```powershell
powershell -ExecutionPolicy Bypass -File bench/run.ps1 -Manifest bench/corpus/manifest.local.json -Provider cuda -Iterations 2 -Llm
```

## ASR decision

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
All 77 successful warm calls read 4,318 cached prompt tokens. Raw ASR WER: 0% skipped, 11.84% Haiku group, 11.66% overall. Corpus is already
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

## Known open issues

1. No real maintainer dictation dataset or live key-release-to-paste measurements
   were collected in this task. Capture is ready for that next step.
2. Speculation did not improve p95 in the original-ending sample and had no hits
   at 50 ms trailing silence. Further tuning is deferred until human data exists.
3. Capture writes run in the background after paste/restoration; abrupt exit or
   disk errors can leave incomplete folders (no meta.json completion marker).
   There is no automatic capture retention or capture UI; manage local files manually.
4. CUDA installer packaging remains separate from the tested development runtime.
   CPU fallback on non-NVIDIA machines and rapid restart/device-disconnect/paste
   behavior still require manual validation.
5. Hotword biasing measurement was explicitly skipped at the user's request.
   Dictionary terms are wired into the LLM path, not claimed as measured ASR biasing.
See [README.md](README.md) for exact reproduction commands and measurement
definitions. JSON artifacts are in `results/` (gitignored).

Sources: [sherpa 1.12.36 binaries](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.12.36),
[official ASR exports](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models),
[maintainer's v3 FP32 export](https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3).

## Hotkey merge validation

Performance snapshot `e173a67`; clean merge `ceaade6` of
`t3/rewrite-global-hotkey-system`; timestamp integration `4869adc` uses the
hook's original event time for the release boundary, including dispatch delay.
The merge passed 83 library tests plus one benchmark test and `npm run build`.
After the timestamp integration, all 83 library tests passed again; Windows
Application Control blocked launching the benchmark test EXE (error 4551),
including a normal retry. Security settings were not changed.
## Long-clip incremental ASR comparison

Same nine 15–45 second clips, two warm passes (18 observations), paced 10 ms
input through the app's Silero segmenter. The app already had an incremental
ASR worker; this work preserves it, repairs its tail/drain, and measures it.

| Metric | Full waveform at release p50 / p95 / max ms | Incremental p50 / p95 / max ms |
|---|---:|---:|
| Remaining ASR / finalization | 213.24 / 352.62 / 352.62 | 0.50 / 16.85 / 16.85 |
| End-of-audio to final text, including Haiku | 1125.16 / 2637.24 / 2637.24 | 841.14 / 1140.13 / 1140.13 |
| LLM stage | 908.52 / 2504.33 / 2504.33 | 832.23 / 1139.46 / 1139.46 |
| Total ASR compute, including work during recording | 213.24 / 352.62 / 352.62 | 178.42 / 1130.85 / 1130.85 |

Raw WER: 10.98% full waveform, 8.49% segmented. The baseline includes one cloud
timeout; incremental had none. The prompt changed between runs (4,318 versus
4,376 cached tokens), so the e2e difference is not solely attributable to ASR.
Use the remaining-ASR row to assess the local latency improvement.

The synthetic WAVs contain roughly 0.98–1.15 seconds of trailing quiet at a
100/32768 amplitude threshold. That gives decoding time before file end. The
16.85 ms worst remaining tail includes paced-feed backlog; all VAD segments had
been dispatched before the final explicit flush. These are not immediate
last-syllable hotkey-release measurements. New app traces split ASR compute
using the actual release timestamp; the harness now also splits work that
crosses the scheduled file-end boundary.

Artifact: `results/20261024T214004.002Z-e2e-long-incremental.json`.
