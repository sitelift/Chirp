# Changelog

All notable changes to Chirp.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Entries are dated and reference the commit hash for full diff context.

## [1.3.4] — 2026-04-25

v1.3.0–v1.3.3 were tagged but never shipped: each release uncovered a
different Windows-side CI issue. v1.3.4 carries all of the app-side
changes plus the full set of CI fixes.

### 2026-04-25 — Smart Cleanup CPU fallback, Gemma 4 disk reclaim, llm-server log file, macOS + Windows CI fixes

#### Fixed
- **Smart Cleanup now starts on machines without a usable GPU.**
  `start_server` previously always passed `--gpu-layers 99 --flash-attn on`
  and discarded stdout/stderr, so on systems without a working Vulkan
  runtime (older drivers, some iGPUs, headless VMs) llama-server died
  silently and the user got an opaque "failed to start within 30s".
  We now try GPU first and on failure retry once on CPU
  (`--gpu-layers 0 --flash-attn off`). Both attempts stream stdout +
  stderr to `<config_dir>/llm-server.log` so we have something to
  triage when users report problems.
  `start_server` also `try_wait()`s mid-poll so a subprocess that
  exits during model load surfaces as an error within ~500 ms instead
  of waiting the full 30 s.
- **v1.2.6 → v1.3.x upgrade reclaims ~3.1 GB.** The cleanup-old-models
  migration list was missing `gemma-4-E2B-it-Q4_K_M.gguf`, so users
  upgrading from v1.2.6 kept the old Gemma cleanup model on disk
  forever. Added it to the list; the file is now removed on first
  launch alongside the other superseded LLMs.
- **Windows launch crash on machines without VC++ Redistributable
  (issue #6, "Application error 0xc0000142").** The release workflow
  was downloading the `MD-Release` build of sherpa-onnx, which links
  against the dynamic MSVC runtime and requires the VC++
  Redistributable to be installed. Switched to the `MT-Release`
  variant — same library, same version, but the C/C++ runtime is
  statically linked so the installer is fully self-contained. Fix
  confirmed by the sherpa-onnx maintainer (csukuangfj) in the issue
  thread.
- **macOS release build (CI).** `sherpa-onnx-sys` 1.12.36's build
  script defaults to a flat `<manifest>/sherpa-onnx-lib` lookup and
  panics if no `.dylib`s are there; our workflow places dylibs under
  the `macos/` subdir (which our own `build.rs` already handles).
  Workflow now sets `SHERPA_ONNX_LIB_DIR` on the macOS build step so
  the upstream sys-crate finds them too. v1.2.x didn't hit this
  because it used the older `sherpa-onnx 0.1.10` wrapper crate.
- **Windows link error: 13 unresolved sherpa-onnx symbols (CI).** The
  workflow downloaded sherpa-onnx C lib v1.12.29 while our Rust crate
  is pinned to 1.12.36; symbols added in 1.12.30+ (Online/OfflineStream
  *Option, OnlineSpeechDenoiser*, etc.) were missing from the .lib
  and MSVC's link-time check failed with LNK2019 × 13. macOS didn't
  hit this because its dylibs resolve symbols lazily at load time and
  our code never calls those new APIs. Bumped both jobs'
  `SHERPA_VERSION` to 1.12.36 to match the Rust crate.
- **Windows bundle error: missing cargs.dll resource (CI).** With the
  bump to sherpa-onnx 1.12.36 the Windows MT-Release archive no longer
  ships `cargs.dll` (it's an internal CLI-tools dependency we never
  used at runtime; sherpa-onnx made it static). Removed it from
  `tauri.windows.conf.json`'s bundle resource list so Tauri stops
  trying to bundle a file that doesn't exist.

### 2026-04-21 — Hotkey rewrite, stuck-key fix, Qwen 3 1.7B cleanup LLM, non-English preservation

#### Changed (additional)
- **Cleanup LLM upgraded to Qwen 3 1.7B Q4_K_M (~1.1 GB).** Replaces
  the Qwen 2.5 3B interim revert. Internal benchmarks show comparable
  cleanup quality at roughly half the disk footprint and ~2× inference
  speed. Thinking mode is disabled at the llama-server level via
  `--reasoning-budget 0` so the model cannot emit `<think>` tokens —
  hidden reasoning would break the latency budget and the
  output-length / prompt-injection guard. `--jinja` is passed so the
  GGUF's embedded chat template is used (required for Qwen 3; the
  default llama-server template causes mode-collapse on instruct
  tasks). `llama-server` bumped to b8653 (b8429 predates
  `--reasoning-budget`), with a version marker so stale binaries from
  v1.2.5 get replaced on first launch.

### 2026-04-21 — Hotkey rewrite, stuck-key fix, cleanup-LLM revert to Qwen 2.5, non-English preservation

#### Fixed (additional)
- **Non-English input was being translated to English.** Parakeet v3
  transcribes Dutch/French/etc. correctly (the model is multilingual
  across 25 European languages), but the cleanup LLM was "helpfully"
  translating the output to match its English system prompt. Added
  an explicit LANGUAGE rule to both cleanup prompts instructing Qwen
  to output in the exact same language as the input and never
  translate.

---


#### Added
- **Global hotkey rewrite (Windows):** split into two mechanisms —
  Raw Input API for detection (`src-tauri/src/hotkey_raw_input.rs`)
  and a low-level keyboard hook for suppression only
  (`src-tauri/src/hotkey_ll_suppress.rs`). Detection reads modifier
  state without interfering with normal keyboard processing; the
  suppression hook only swallows downstream delivery of the configured
  chord so the target app doesn't see it as a literal keypress.

#### Fixed
- **Stuck modifier after hotkey release:** the LL suppression hook
  was swallowing key-ups whose key-downs it had not swallowed,
  leaving target apps with a key they believed was still held
  (e.g. Shift latched on after releasing the hotkey). The hook now
  tracks swallowed presses in a HashSet and only suppresses matching
  releases. Raw Input detection is unaffected, so Chirp still sees
  the release cleanly.

#### Changed
- **Cleanup LLM reverted to Qwen 2.5 3B Instruct (v1.2.5 config):**
  Gemma 4 E2B was introduced in v1.2.6 and carried through v1.3.0
  dev, but Qwen 2.5 3B with JSON-schema output + datamarking is the
  battle-tested cleanup path. `src-tauri/src/llm.rs` is restored
  wholesale to the v1.2.5 version, with one signature tweak
  (`cleanup_text` takes `&reqwest::Client` from the shared pool
  instead of building its own). `llama-server` pinned back to
  `b8429` to match. Users upgrading from v1.2.6 will have the old
  `gemma-4-E2B-it-Q4_K_M.gguf` auto-removed on first run of the new
  model downloader (~3.1 GB reclaimed).

### 2026-04-18 — Overlay cold-start crash fix + Moonshine removal

#### Fixed
- **Overlay "Something went wrong" on cold start (Sentry RUST-R):** The
  overlay window's settings-sync hook fired 6+ `invoke()` calls at mount.
  When the Tauri IPC shim (`window.__TAURI_INTERNALS__`) wasn't ready yet
  on cold start, those invokes rejected and the unhandled rejection
  cascaded into the error boundary, showing "Something went wrong" and
  preventing `hotkey-pressed` listeners from attaching — which also
  explains the intermittent cold-start "hotkey not detected" reports
  (workaround was toggling the hotkey in settings, which re-emitted state).
- `src/hooks/useOverlaySync.ts` — new lightweight sync hook with
  IPC-ready retry (20× × 100ms, detects `__TAURI_IPC__`/
  `__TAURI_INTERNALS__` in error strings). Replaces the heavy
  `useSettingsSync` on the overlay + tray-popup windows.
- `src/App.tsx` — hoisted the window-label branch above
  `useSettingsSync()` so the heavy sync hook only runs in the settings
  window.

#### Removed
- **Moonshine ASR:** removed completely. Parakeet v3 TDT 0.6B is the
  only ASR engine going forward.
  - `src-tauri/src/transcribe.rs` — dropped
    `OfflineMoonshineModelConfig` import, removed the `moonshine-base`
    entry from `model_info()`, inlined the Parakeet path in
    `load_model()` (no more `is_moonshine` branch).
  - `src-tauri/src/state.rs` — default `model` flipped from
    `moonshine-base` to `parakeet-tdt-0.6b`.
  - `src-tauri/src/settings.rs` — migration now maps `moonshine-base`
    (and the legacy whisper sizes) → `parakeet-tdt-0.6b`;
    `cleanup_old_models()` deletes the `sherpa-onnx-moonshine-base-en-int8`
    dir (~272 MB) on upgrade so existing v1.3.0-dev users reclaim disk.
  - `src-tauri/src/commands.rs` — stale "Moonshine spike" comments
    rewritten neutrally; behavior unchanged (capture entire recording,
    no VAD segmentation on this branch).

### 2026-04-11 — Revert Moonshine streaming, restore Parakeet v3 + Gemma 4 E2B

#### Changed
- **ASR:** Reverted the Moonshine streaming migration. Real-world
  transcription quality regressed vs. Parakeet v3 + VAD streaming in
  dogfooding, so the decision is to ship the known-good Parakeet path.
  The Moonshine work is preserved at tag `archive/moonshine-migration`
  for future reference.
- **Cleanup LLM:** Swapped from Ministral 3 8B Instruct 2512 (5.2 GB)
  back to Gemma 4 E2B Instruct (3.11 GB). Gemma's plain-text prompt
  format with in-prompt few-shot examples is what shipped successfully
  in v1.2.6; the Ministral swap was a v1.3.0 dev experiment.
- `src-tauri/src/llm.rs`: model constants now point at
  `gemma-4-E2B-it-Q4_K_M.gguf` via the unsloth GGUF mirror. Removed
  Ministral-specific JSON-output + `<transcription>`-wrapper prompt
  plumbing (`parse_cleaned_text`, `unwrap_cleaned_text`,
  `raw_is_unsafe_fallback`, `tokenize_text`, few-shot chat-turn arrays).
  Cleanup now sends a plain-text `{system, user}` chat and consumes
  the response verbatim, with the length-guard prompt-injection defense
  unchanged.
- `src-tauri/src/settings.rs::cleanup_old_models`: added the stale
  Ministral GGUF to the auto-cleanup list and removed Gemma from it
  (Gemma is the active model again).

#### Why
- Mobile/MLX portability is a real constraint on the cleanup model, and
  Gemma 4 E2B runs well on edge devices — Ministral 3 8B doesn't.
  Picking Gemma now unblocks the mobile port later without forcing
  another cleanup-model migration.
- v3 benchmark numbers favored Ministral, but the benchmark is too
  narrow to be a shipping signal on its own; real-world feel + mobile
  constraints win for a today-ship.

### 2026-04-07 — Smart-join post-pass for streaming cleanup

#### Changed
- **`cleanup::join_cleaned_segments`** added — replaces the dumb
  `vad_cleaned_texts.join(" ")` in `stop_recording`. Pure deterministic
  Rust, no LLM calls, ~microsecond runtime. Three rules in order:
  1. **Strip internal `\n\n`** — single VAD segments shouldn't contain
     paragraph breaks; the model occasionally adds them as a leftover
     habit from training data.
  2. **Sentence-level smart merge** — split the joined output into
     sentences, then for each adjacent pair decide whether they should
     stay separate or merge. Two heuristics:
     - **Stub-end merge**: if the previous sentence's last word is in a
       conservative `STUB_END_WORDS` list (articles, prepositions,
       auxiliaries, possessives, "just", and the fillers "um"/"uh"/"hmm"
       which the regex strips next), strip the terminal period and
       merge — the previous sentence was a mid-sentence VAD break.
     - **Continuation-start merge**: if the next sentence starts with a
       continuation word (`and`, `but`, `or`, `because`, `since`, etc.),
       strip the terminal period and lowercase the conjunction.
     - When merging, lowercase the next word only if it's in a
       hand-curated list of safe English words (never proper nouns).
  3. **Re-run regex pre-pass** (`cleanup_text`) on the merged output to
     catch fillers that were preserved at segment boundaries (e.g. an
     "Um" at the start of a segment looked like a discourse marker to
     the model and survived per-segment cleanup) and to normalize
     whitespace / punctuation spacing.
- 29 unit tests in `cleanup::tests::test_join_*` cover: empty input,
  single passthrough, internal `\n\n`, real sentence boundaries kept,
  stub-end merges (article/preposition/modal/just), continuation-word
  merges, lowercase-start merges, proper noun preservation, three-way
  merges, filler-as-stub edge case, session 5 blast-radius regression.

#### Why
- The first cut of streaming cleanup left orphan periods between
  segments when VAD broke mid-sentence. Example: a segment ending in
  "Just." merged with "See if it's viable." produced
  `"Just. See if it's viable."`. The smart-join recognizes "just" as a
  stub end and produces `"Just see if it's viable."` instead.
- Benchmark on 5 real multi-segment dictations from the dev's log:
  session 3 went from `"Don't do any coding at this point in time. Just um. See if it's viable. And if we can. Make it work."` to
  `"Don't do any coding at this point in time. Just see if it's viable and if we can make it work."` — three orphan periods removed,
  two continuation merges, one filler strip.

### 2026-04-07 — Streaming per-segment cleanup

#### Changed
- **Cleanup now runs per VAD segment, in parallel with continued recording.**
  Previously the LLM cleanup ran ONCE at end-of-recording over the entire
  concatenated multi-segment transcript. The model's wide context made it
  prone to paraphrasing across sentence boundaries, hallucinating duplicates,
  and feeling "too harsh" by aggressively rewriting clauses it could see
  globally. Now cleanup runs inside the VAD receiver thread the moment a
  segment is transcribed: regex pre-pass → vocab find/replace → snippet
  expansion → `llm::cleanup_text` (one short segment at a time) → push to
  the new `vad_cleaned` accumulator. By the time the user releases the
  hotkey, most cleanup is already done — `stop_recording` just drains the
  cleaned segments, joins them, and injects.
- **Two wins from one change.** (1) The model can no longer paraphrase
  across segments because it never sees more than one VAD segment of input,
  bounding its blast radius by construction. (2) Cleanup latency is hidden
  behind dictation time — the user only waits for the LAST segment's
  cleanup after release, not the whole pipeline.
- New `VadCleanedTranscripts` state alongside `VadTranscripts`. The raw
  transcripts are still pushed to the diagnostic accumulator so the
  existing log lines work; the cleaned outputs go to a separate accumulator
  consumed by `stop_recording`.
- Threading: the VAD receiver thread is a `std::thread`, not tokio. It
  calls the async `llm::cleanup_text` via a captured `tokio::runtime::Handle`
  + `block_on`. No new dependencies, no duplicated HTTP code, the existing
  `reqwest::Client` from `AppState` is reused.
- **Fallback path unchanged.** If the Silero VAD model isn't installed,
  `stop_recording` still runs the original chunked transcription + monolithic
  regex + LLM cleanup at the end. Streaming cleanup applies only when VAD
  is active.

#### Known limitations
- Cross-segment self-corrections (e.g., user says "send it to John" then
  pauses long enough to break a VAD segment, then says "no, Mike") will
  not be resolved — each segment is cleaned in isolation. Most
  self-corrections happen within a single phrase that VAD groups
  together, so this is rare. Users can re-dictate.
- Per-segment LLM calls serialize inside llama-server (`--parallel 1`).
  On very fast dictation with many short segments, the receiver thread can
  briefly fall behind cleanup; not a correctness issue.
- The "polishing" UI event currently fires only on the fallback path.
  The streaming path skips that overlay state because cleanup is no longer
  a discrete end-of-recording phase.

### 2026-04-07 — Inject pipeline streamline + VAD/fallback exclusivity (bug fixes)

#### Fixed
- **Duplicated text in a single paste** when VAD streaming and the chunked
  fallback both ran over the same audio buffer. The decision was previously
  `use_vad = !vad_texts.is_empty()` — when VAD ran but produced empty/partial
  output, the fallback re-transcribed the whole buffer that VAD had already
  partially processed, concatenating duplicate content into a single paste.
  Now tracked via a deterministic `vad_was_active` flag set in
  `start_recording` the moment the VAD receiver thread spawns, and consulted
  in `stop_recording` instead of inspecting `vad_texts`. If VAD ran, its
  output is trusted (even if empty); the fallback is never entered. Same
  audio cannot be transcribed twice.
- **Modifier keys handling on paste.** Inject previously synthesized KEYUPs
  for `Shift/Ctrl/Meta/Alt` via enigo, but enigo's `Key::Alt` only releases
  LeftAlt on Windows — Right Alt / Right Ctrl / Right Shift / Right Win
  were never released, and synthesizing KEYUPs for keys the user might
  still be physically holding can leak modifier events to the host app or
  trigger Paste Special dialogs. Replaced with `wait_for_modifiers_released`:
  polls `GetAsyncKeyState` for all eight modifier VKs at 5 ms intervals up
  to 300 ms, returns as soon as all are physically released. Only on
  timeout does it force a `SendInput` KEYUP on specifically-stuck modifiers.
- **Stuck Win modifier triggering Start Menu after paste.** Added
  `cancel_pending_start_menu()` — sends a dummy `VK 0xFF` down/up via
  `SendInput` to swallow the queued Start Menu activation, but only when
  the timeout safety net actually had to release Win.
- **Silent hotkey suppression failure.** When `rdev::grab` failed it logged
  a `warn` and silently fell back to passive `rdev::listen` mode, leaving
  the user's hotkey leaking through to the active app — a `Ctrl+Shift+Z`
  binding would trigger REDO in Word/VS Code on every dictation. The grab
  failure now logs at `error` level and emits a `hotkey-grab-failed` Tauri
  event the UI can surface as a banner. Passive fallback behavior is
  unchanged so recording still works.
- **Paste in cmd.exe consoles.** `inject_text_windows` now detects
  `ConsoleWindowClass` foreground windows and uses a right-click via
  `SendInput` (works in QuickEdit mode, the Win10+ default) instead of
  `Ctrl+V` which the legacy console swallows. Falls back silently if
  QuickEdit is disabled.

#### Changed
- **Inject path collapsed.** Dropped HTML/RTF rich-text generation in
  `inject_text_windows` (was unverified, the verify loop only checked
  `CF_UNICODETEXT`, and rich-format mishandling has its own bug class in
  Word/Slack-style editors). Plain text only. Kept the three Win32
  exclusion flags (`ExcludeClipboardContentFromMonitorProcessing`,
  `CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard`) — they're
  small and useful.
- **Paste path now uses `SendInput` scan codes directly** instead of enigo
  for the Ctrl+V. Removes a layer of indirection that had its own
  modifier-tracking quirks.
- **Clipboard restore delay reduced 3000 ms → 800 ms.** Voquill ships at
  800 ms in production without complaints; 3 s was set defensively for an
  Electron-app race that hasn't been seen recently.

#### Removed
- `src-tauri/src/richtext.rs` — HTML and RTF generators (no live caller).
- `html` and `rtf` parameters from `clipboard_win::set_clipboard_with_exclusion`.
- `mod richtext;` declaration in `lib.rs`.

### 2026-04-07 — chirp-cleanup-v2 swap, ASR hotwords, vocab find/replace (`95e3434`)

#### Added
- **chirp-cleanup-v2 cleanup model.** Replaces Gemma 4 E2B with our own
  fine-tuned 0.6B (Qwen3 base) hosted at `sitelift/chirp-cleanup-v2`.
  Benchmark on 50 cases (RTX 4080): 32 EXACT vs Gemma's 24, 0 hallucinations
  vs 4, 59 ms p50 vs 141 ms (2.4× faster), 397 MB on disk vs 3.1 GB (8×
  smaller). Greedy decoding (`temperature: 0.0`).
- **VAD streaming activated.** The wiring already existed in `audio.rs` /
  `commands.rs` but the `silero_vad.onnx` model file was missing on this
  install. Long dictations now run at ~60× realtime (75 s clip → ~1.1 s
  wall-clock end to end) because VAD segments transcribe in parallel
  during recording instead of sequentially after release.
- **Two-layer vocabulary correction system.** Each vocab entry is now a
  `VocabEntry { term, replaces }` instead of a bare string:
  - `term` drives **ASR-time hotwords biasing** via sherpa-onnx
    (`modeling_unit: bpe`, `bpe_vocab`, `hotwords_file`,
    `hotwords_score: 3.0`). All four config items required at recognizer
    construction time — per-stream `create_stream_with_hotwords` API alone
    was a no-op.
  - `replaces` drives **post-ASR find/replace** for homophones, brand
    spellings, and stable mishearings the ASR can't fix. Case-insensitive,
    smart word-boundary anchored, applied between regex pre-pass and LLM
    cleanup. 8 unit tests pin the match semantics.
- **`benchmark_chirp_v2.py`** in `training/` for re-running the 50-case
  comparison locally against any GGUF cleanup model.
- **`CHANGELOG.md`** (this file).

#### Changed
- `BASE_SYSTEM_PROMPT` collapsed from the verbose 30-line Gemma-era prompt
  to the single-line training prompt that won the chirp-cleanup-v2
  benchmark. Email mode prompt similarly minimized (flagged for benchmarking
  later — chirp-cleanup-v2 wasn't trained on email examples).
- `llama-server` flag `--reasoning off` → `--reasoning-budget 0`. The newer
  flag is the one verified to suppress Qwen3 thinking blocks in the v2
  benchmark.
- `update_vocabulary` no longer rebuilds the recognizer inline. It now
  defers to a `recognizer_dirty` flag, with the actual rebuild happening
  at the end of the next `stop_recording` (after the user has their text
  and the lock window is safe).
- The dirty flag now only fires when the canonical **term list** actually
  changed — editing `replaces` lists (the common case while building up
  the find/replace dictionary) does not trigger a recognizer rebuild,
  because find/replace is read live from `s.vocabulary` per dictation.
- Recognizer rebuild wrapped in `catch_unwind` with a no-hotwords fallback
  recognizer. A future sherpa-onnx panic now produces an error log instead
  of killing the process.
- Old recognizer `Arc` is dropped explicitly **before** allocating the new
  one in the rebuild path, minimizing the window where both exist
  simultaneously (the configuration that empirically triggered heap
  corruption).
- Vocabulary settings UI rewritten with two columns per row: editable
  canonical term + comma-separated replaces input. Each row is its own
  `VocabRow` component with raw-string local state and blur-to-commit, so
  partially-typed commas don't get parsed away mid-keystroke.
- Cleanup pipeline order is now:
  `Parakeet ASR → cleanup_text (regex) → apply_replacements → snippets → llm::cleanup_text → clipboard`.

#### Fixed
- **`cleanup::strip_corrections` was silently destroying dictations.**
  The regex used `.*\b(?:wait|i mean|actually|sorry|...)`, which with
  greedy `.*` matched everything from the start of input through the
  *last* occurrence of any discourse filler. Speaking "I mean" or
  "actually" mid-monologue erased everything before it — sometimes
  hundreds of characters per dictation. Removed the entire function;
  chirp-cleanup-v2 handles self-corrections semantically. Latent since
  the function was first written.
- **`llm::cleanup_text` injected vocab terms into the system prompt**
  ("speaker frequently uses these terms..."). chirp-cleanup-v2 isn't
  trained to in-context-learn instructions outside its training prompt
  and would hallucinate, including swapping speaker/roommate identities
  to make vocab terms appear in unexpected places. Removed the injection
  entirely; vocabulary is now ASR-only.
- **`load_vocabulary` migration only fired when `vocabulary.json` was
  missing entirely.** Installs that had auto-created an empty vocab file
  silently orphaned their legacy `dictionary.json` since v1.2.5. Now
  migrates on missing OR empty, AND preserves the legacy `from` field as
  a `replaces` entry (its original intent finally implementable).
- **STATUS_HEAP_CORRUPTION 0xc0000374 from rapid recognizer churn.**
  Frontend was firing `update_vocabulary` on every keystroke + blur in
  the vocab editor. Each call rebuilt the recognizer inline, allocating
  and freeing sherpa-onnx hotword automatons in rapid succession. The
  C-side cleanup has a slow-burn use-after-free that compounds across
  rebuilds. Fixed by the dirty-flag deferral + terms-changed check
  described above.
- Old Gemma GGUF (3.1 GB) is now removed from existing installs by
  `cleanup_old_models()` so the disk gets reclaimed on first launch
  after upgrade.
- `VocabularyPage` add path wraps `trackEvent` in a try/catch so the
  pre-existing Aptabase / Tauri IPC race condition (RUST-E in Sentry,
  first seen 7 days ago) can no longer surface as an unhandled rejection
  on the vocabulary page.

#### Removed
- `cleanup::strip_corrections` function and its `correction` regex (see
  Fixed above).
- LLM vocab-injection code path in `llm::cleanup_text` (see Fixed above).
- `vocabulary` parameter from `llm::cleanup_text` signature — was unused
  after the injection removal.
- `transcribe::vocabulary_to_hotwords` helper and the per-call
  `create_stream_with_hotwords` code path. Hotwords are now configured
  at recognizer construction time, not per-stream.
