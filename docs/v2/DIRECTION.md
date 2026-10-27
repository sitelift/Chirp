# Chirp v2 — Direction

Status: draft, 2026-10-24. This is the source of truth for v2 decisions. Update it when a decision changes.

## What Chirp is

Press a hotkey, talk, release. What you meant appears at your cursor as well-written text, in any app, effectively instantly.

## Goals

**Primary — everything else yields to these two:**

1. **Time to text.** Measured from hotkey release to text pasted. Aspirational target <200ms. It must *feel* instant every time, not just usually.
2. **Transcript quality.** Turn exactly what was said into the best written form of it: punctuation, numbered/bulleted lists, paragraphs, self-corrections ("no wait, make that Tuesday"), spoken formatting, email structure when writing an email, code-ish identifiers when in an editor. Context-aware via the active app / window / page title.

**Secondary:**

3. **The Chirp brand.** Name, logo and amber stay. Everything else in the UI — overlay, settings, onboarding — is redesigned from first principles around how people actually use voice. North-star references gathered via Mobbin. The v1.4 redesign is discarded.
4. **Users and contributors.** FOSS, built in public. Someone should be able to clone, read a sensible README, build, and be excited. The maintainer has no obligation to support setups he doesn't run.

## Non-goals

- macOS and Linux. Windows only; the maintainer's setup (Windows 11, RTX 4080 Super) is the reference platform and the one that gets dogfooded to perfection.
- Local LLM cleanup (llama-server, model downloads, GPU/CPU fallback). Removed.
- LLM providers other than Anthropic and OpenAI. Users bring their own key; the provider is a setting (Off / Anthropic / OpenAI), see `design/SPEC.md`.
- Live transcript preview while speaking.
- "Local-only" as a principle. Speech recognition stays local because it's fastest, not on principle.
- Tone modes. The model infers register and format from the active app; the user never switches anything.
- Telemetry and extras. Sentry, Aptabase, the Discord feedback webhook, announcements, and the Chirp Pro page are removed.

## Pipeline and latency budget

Every stage is timed and logged on every transcript from day one. Targets are revised once the benchmark harness produces real numbers.

| Stage | Approach | Target |
|---|---|---|
| Hotkey → recording | Win32 low-level hook (port from v1.3.4). Mic stream kept warm so pressing the key never waits on device open. | <20ms |
| On key press (parallel) | Pre-open the TLS/HTTP connection to the cleanup provider and warm its prompt cache so both are done before release. | off the critical path |
| Speech → text | Local Parakeet TDT with CUDA. Decode the complete recording on release; VAD prefixes are only for optional speculative cleanup. Preserve quiet speech between detected segments. | measured warm p50 89ms / p95 440ms on 134 human clips; excludes paste |
| Deterministic pass | Segment join, spoken commands, snippets. No filler, number or dictionary rewriting; no ASR biasing (Words go to the cleanup model as spellings). Unit-tested. | <5ms |
| Router | Decide whether the LLM is needed (see below). | <1ms |
| LLM cleanup | GPT-6 Luna (`gpt-6-luna`) on the Fast service tier, reasoning off, by default; Claude Haiku 5.5 (`claude-haiku-5-5`, thinking disabled, low effort) as the alternative. Cached system prompt, active-app context. Hard timeout → paste the deterministic result instead. | measured warm API p50 0.93s / p95 1.29s (Luna Fast) |
| Inject | Clipboard + Ctrl+V, restore the previous clipboard. Wait for the final text, then paste once (no streaming insertion). | <20ms |

### Router: when to skip the LLM

v1.3.4 already skips AI cleanup for messages under 7 words (`MIN_AI_CLEANUP_WORDS`). v2 keeps the idea and makes it smarter. **Skip** when the deterministic result is short and has no formatting signals. **Send to the LLM** when any of these apply:

- Length over a threshold.
- Spoken formatting cues ("new paragraph", "bullet", "number one", "first… second…").
- Self-corrections ("scratch that", "I mean", "no wait").
- The context implies structure (email client, document editor).

Thresholds are tuned against the corpus, not guessed.

### Context

Send the active app name plus window or page title (e.g. `Outlook — "RE: Q3 invoice"`, `Chrome — "Pull request #42 · GitHub"`). Never send screenshots, and never send text from around the cursor. There are no tone modes: the model picks register and format (casual chat, formal email, terse prompt, code-aware) from this context.

## Hotkeys

**Two independent bindings, both optional:**

- **Hold-to-talk:** record while held, transcribe on release.
- **Toggle:** press to start, press again to stop.

Each binding can be any single key or any combination of keys the keyboard can produce:

- modifiers alone (e.g. `RightCtrl`, `Ctrl+Win`)
- F13–F24
- media and OEM keys
- left vs. right modifiers
- optionally mouse side buttons

**Problems in v1.3.4 to design out:**

- **Three naming schemes for keys:** browser `event.code` in the webview capture, rdev names, and Win32 VK codes, translated through hand-written tables (`id_to_vk` / `vk_to_id`). Any key missing from a table silently can't be bound. F13–F24 aren't in the tables at all.
- **Left/right asymmetry:** `Alt` means left Alt only and `AltGr` means right Alt. Combos also record a specific side for Ctrl and Shift, so recording LeftCtrl means RightCtrl doesn't trigger it.
- **Exact-set matching** (`held == combo`): any extra key held, or a key whose key-up was missed (lock screen, UAC, sleep), stops the hotkey firing until state is reset. This likely explains "hotkey needs re-setting".
- **Detection and suppression are split across two mechanisms:** Raw Input for detection, a `WH_KEYBOARD_LL` hook for suppression. Their state can drift apart.

**v2 approach:**

- **Capture in Rust, not the webview.** The settings UI asks the backend to "record a binding". The backend listens to raw key events and stores **scan code + extended flag (+ VK for display)**. No string translation tables.
- **Display labels** come from `GetKeyNameTextW`, so every key gets a real name for free.
- **One input thread** owns key state, and resyncs held keys with `GetAsyncKeyState` whenever it might be stale (focus change, session unlock, resume).
- **Matching:** a binding fires when all its keys are down and no *other modifier* is down. Unrelated keys being held doesn't block it.
- **Suppression is per binding and configurable.** Modifier-only bindings are never suppressed.
- **Conflicts** between the two bindings, and with common system shortcuts, are detected at record time.

Implementation and verification notes: [HOTKEYS.md](HOTKEYS.md). Defaults are Right Ctrl for hold and Left Ctrl + Left Win for toggle. Physical-key and live-dictation validation remain on the maintainer checklist.

### Prompt and safety

- The system prompt is a stable, cached editing guide. Substantial rewriting is welcome when it improves the message and preserves its claims, details, uncertainty and voice. Destination context determines register, not extra content such as unspoken greetings or sign-offs.
- The transcript is wrapped as data, never as instructions.
- Anthropic returns one unrestricted `text` field using its JSON output schema.
  The app extracts that field; it does not classify or rewrite its contents.
  Plain-text prompt revisions still produced editorial asides during repeated
  human-corpus tests. The selected prompt plus this envelope avoided those
  asides in 268 observed outputs, at roughly 400ms higher median API latency.
  This is evidence of improvement, not a guarantee against content mistakes.
  OpenAI retains its existing plain-text response protocol.
- An output-length guard rejects runaway responses.
- On any error, timeout or refusal, fall back to the deterministic text. Dictation must never fail just because the API did.

## Measurement first: the bench harness

Before any UI work, build a headless harness: WAV in → per-stage timings + output text out.

- **Corpus.** 30–50 real dictations recorded by the maintainer: Slack replies, emails, AI prompts, lists, rambles, self-corrections. Each has a hand-written ideal output.
- **Speed.** Speech-model latency on the 4080 (Parakeet offline vs. Nemotron streaming), plus Haiku 5.5 latency (time to first token, total; cold vs. cached prompt).
- **Quality.** Scored against the ideal outputs, so every prompt change is measured, not eyeballed.

It runs on the Windows machine. The Linux devbox has no GPU and can only run the CPU and API parts.

### ASR benchmark decision (2026-10-24)

The first Windows run uses the 45-file synthetic SAPI corpus, not maintainer recordings. Parakeet v3 **FP32 on CUDA** is the measured choice: warm ASR p50 98.6 ms / p95 243.8 ms, raw WER 11.66%, versus CPU int8 with six threads at 317.2 / 1106.7 ms and 12.14%. CUDA int8 is slower; the v2 FP16 export returned empty text and crashed. The tested Nemotron 80 ms int8 model was slower and less accurate on a three-file screening subset, so keep Parakeet with incremental Silero segmentation for now.

The app defaults to automatic selection: FP32/CUDA when that model and runtime are installed, otherwise int8/CPU (up to six threads). Silero stays on CPU. Keep the microphone and VAD warm; warm ASR and deterministic regexes at model load. The FP32 model is available through the maintainer's Hugging Face export; `bench/setup-native.ps1 -Fp32Model` installs it alongside the switchable native runtimes. See `bench/README.md` and `bench/PERFORMANCE.md` for reproduction and limitations.

The full 45-file routed benchmark (two warm passes, prewarm per item) measures end-of-audio to final text at p50/p95 52.5/55.4 ms for the six short commands that skip Haiku, versus 895.9/1824.3 ms for the 39 Haiku-routed items. One warm call timed out at 2.5 s and used deterministic text. All 77 successful warm calls hit the prompt cache. Thus the <200 ms aspiration is not met for cloud-cleaned text; ASR speed alone does not establish end-to-end performance. Actual key-release-to-paste still needs live validation: the WAV harness excludes clipboard dispatch. History/log timing records the original input timestamp through paste dispatch and restoration.

### Stop tuning synthetic speech; collect human data (2026-10-24)

Latency is sufficient for the next development step; cleanup quality needs real
maintainer dictations. Capture is a default-off, local-only development feature
(`capture_dictations`, JSON `captureDictations`, or `CHIRP_CAPTURE_DICTATIONS`).
It preserves full resampled float audio, exact ASR chunks, intermediate/final
texts, context, dictionary terms and timing under the app-data captures folder.
The importer appends selected examples to a gitignored local corpus manifest;
raw-ASR and pasted-text references are drafts requiring manual correction.
Capture adds no upload, telemetry or extra cloud request. Background retention
defaults to 14 days and 2 GiB, oldest first; completed capture directories are
published by atomic rename and interrupted temporary folders are cleared at startup.

Speculative cleanup is implemented at closed VAD segments, one request at a
time, with exact-input reuse at release. The paired synthetic run improved the
original-ending median (779 to 661 ms), but not p95; 50 ms trailing silence had
zero hits. Keep the feature **off by default** (`speculativeCleanup`, with
missing settings migrated to false), measure real behavior when opted in, and defer further latency
and hotword tuning. Browser context is captured at press and resolved once;
the same dictionary term snapshot is used for prewarm and every cleanup call.
See bench/PERFORMANCE.md for the measured scope and known open issues.

### Preserve complete audio before tuning cleanup (2026-10-25)

Replay of 134 saved human dictations supersedes the synthetic-corpus decision
to use VAD segments as the final transcript. In a confirmed failure, VAD omitted
quiet speech containing a negation; cleanup received the opposite claim. Exact
saved-chunk replay reproduced the error. Whole-recording replay restored it,
and adding actual earlier audio to the damaged chunk worked while adding
silence did not. A cleanup prompt cannot reliably recover discarded speech.

Final ASR now uses the complete waveform. Ordinary recordings stay together;
recordings over 90 seconds split near low-energy points, with one second of
overlap and no discarded gaps. The old fixed 30-second cuts damaged a long
human dictation and introduced duplicate fragments. VAD decoding runs only
when speculative cleanup is enabled, and its output is never appended to the
final recording transcript. Speculative cleanup still requires an exact match
to the final deterministic input before reuse; it remains off by default.

Repeated CUDA replay of all 134 clips (up to 69 seconds) produced identical,
nonempty raw transcripts, warm p50/p95/max 89/440/662 ms. This is local ASR
compute, not key-release-to-paste latency or a word-accuracy score. The CPU
fallback also recovered the confirmed negation; sampled 62–69 second clips
took roughly 3–7 seconds on CPU. Audio quality remains imperfect, and two
unclear passages have no user-confirmed reference. Keep prompt comparisons
separate from ASR comparisons: freeze text, context and dictionary for every
candidate in a round, repeat the finalists, and manually inspect meaning
changes rather than accepting an automated judge score alone.

The selected editing prompt replaces the exact-word restriction with permission
to rewrite sentences while preserving meaning, and removes forced email layout
that encouraged unspoken sign-offs. A much shorter prompt was tested repeatedly
but still changed claims or missed addresses. No phrase blacklist, correction
retry or second model pass is added. Anthropic prewarm and key validation use
one output token because the structured API rejects a zero-token budget; the
warmup response is discarded.

### Cleanup model: GPT-6 Luna Fast by default (2026-10-26)

Chirp has to be affordable and fast per dictation, so the choice was Haiku 5.5
or GPT-6 Luna, not a larger model. Sonnet 5.5 scored highest but costs about
17x Haiku per dictation, which adds up over thousands of dictations.

Evaluation: 134 human dictations with written references (94 for iteration,
40 held out), an Opus judge blind to the setup, and blind pairwise preference
on the held-out set. On pass/fail, Luna Fast and Haiku tie (held-out 81% vs
82.5%; dev 92.6% vs 87.2%; both CIs span zero) with similar meaning-error
rates. Head to head on writing quality, Luna Fast is preferred 32 to 10 over
shipped Haiku (38 ties) and 34 to 11 over Haiku plain: it joins run-ons,
merges repeats and drops false starts where Haiku tends to stay near-verbatim.
Warm API latency in one interleaved session: Luna Fast p50/p95 929/1294 ms,
Haiku plain 802/1911 ms, Haiku JSON 1053/1727 ms. Cost per warm dictation is
about $0.00010 vs $0.00007-0.00008. Fast is `service_tier: "fast"` on
`gpt-6-luna`, not a separate model id.

Prompt tuning has plateaued: a one-line prompt, two shorter rewrites and a
targeted "keep qualifiers" line all tied the current prompt on writing
quality. The one change kept is "use straight quotes": Luna put curly quotes
in about a third of outputs (bad in terminals and code), and the line takes
that to zero. Remaining errors are model judgment: dropped hedges while
tightening, a repeated question kept, guesses at garbled audio. Product names
the model cannot know belong in the user's Words list, not the prompt. Luna
produced no editorial asides, alternative drafts or answers in 100 stress runs
of the inputs that triggered the original "Wait, that's not clean" paste.

## Keep, port, drop

**Port into v2 (re-read and clean up, don't copy blindly):**

- Windows hotkey hook (v1.3.4 rewrite: suppression + Raw Input)
- `inject.rs`
- settings / history / dictionary / snippets persistence
- The useful parts of `cleanup.rs`, with tests
- Brand assets

**Drop:**

- All macOS code (`hotkey.rs` mac path, `native_hotkey.rs`, stoplight buttons, permission onboarding)
- `llm.rs` / llama-server
- The LLM model download
- v1.4 redesign and multi-provider cloud code
- Vocabulary-training leftovers
- Cross-platform build plumbing

Also drop:

- Sentry
- Aptabase
- Discord feedback webhook (`feedback.rs`)
- `announcements.rs`
- Chirp Pro page
- Tone modes and the `t5.rs` experiment

## Licensing

- Code: MIT.
- The name "Chirp" isn't a registered trademark, so we only claim what we actually own.
  - **Brand artwork:** the logo, app icon and brand artwork (`assets/brand/`, `src-tauri/icons/`) are copyrighted and excluded from MIT ("all rights reserved").
  - **The name:** a short `BRAND.md` asks forks and redistributions to use a different name and icon.
- All commits so far are by the maintainer (plus AI-assisted commits), so relicensing from the current source-available license needs no outside sign-off.

## Build order

0. **Housekeeping.**
   - Fast-forward `master` to v1.3.4.
   - Delete merged and stale branches.
   - Archive `dev` / `v1.4` as tags.
   - Create a `v2` branch.
   - Switch to MIT + `TRADEMARKS.md`.
1. **Bench harness + corpus.** Settle the speech model and the LLM numbers with data.
2. **Minimal v2 app.** Hotkey → capture → ASR (CUDA) → router → Haiku → inject, with a placeholder overlay and stage timings visible. Dogfood daily.
3. **Prompt quality.** Iterate on the cleanup prompt against the corpus until it's best in class.
4. **UX redesign.** First-principles design using Mobbin references: overlay, settings, onboarding, brand system.
5. **Public polish.** README, contributing guide, Windows-only release pipeline, updater.

## Redesign (2026-10-25, built)

Supersedes the window, Opened, Words, AI cleanup, Recent, keyboard-cancel and Motion points of the next section where they differ. The approved mockup and its numbers are in `docs/v2/design/` (`motion.html` is the visual and motion reference, `SPEC.md` the decisions and contracts); `?preview=1` renders every state and `?play=1` runs the live surface against an in-memory backend.

- **One window, sized once.** The overlay stays at a fixed 680 x 492 envelope and never resizes during an animation (WebView2 re-layout lag was the flicker). `WS_EX_LAYERED` is permanent; click-through follows a hit rect (hover pill, pill or panel) polled at 60 Hz, so clicks outside the panel reach the app underneath and close it.
- **Home:** last dictation (click to copy), Recent, Words, Snippets, then Pause and Settings. No close button: Esc or click away.
- **Settings** is a 600 x 400 panel with a sidebar: Chirp (Appearance: System / Light / Dark), Shortcuts (click the keys to rebind; Hold to talk, Hands-free, and a rebindable Cancel, Esc by default), Microphone, Sound (Sounds, Music while dictating), AI cleanup (Provider Off / Anthropic / OpenAI, one key per provider).
- **Words** are plain spellings handed to the cleanup model; no "you say → Chirp types" pairs. **Snippets** are their own screen.
- **Pipeline:** Parakeet is recognition only (greedy, no hotword biasing). The only local text change is spoken commands ("new line", "comma", ...), always on; with the provider Off that is what gets pasted.
- **Motion:** the panel grows out of the hover pill at its real size and shrinks back through the pill into the tick, solid until tick-sized; screens push like paper, edge to edge; no clip-path animation, no grey blends. Nested corners follow the corner rule (outer radius minus inset).
- **Icons:** a custom 24-grid set replaces Lucide; Chirp is the bird outline, AI cleanup a robot head.

## Design direction: the pill is the app (2026-10-24, built)

Supersedes the north-star list below. Designs: Paper file "Chirp v2" (artboards "Overlay / v2" and 01 Idle through 13 Keyboard, each in Light and Dark; "Overlay / Current (v1)" is the before). Implemented on `v2`; `http://localhost:5173/?preview=1` (dev only, `&section=03` for one artboard) renders every state in both themes for side-by-side review.

Clean, minimal, usable. Every surface has to justify existing. There is **one surface**: the pill, bottom-center just above the taskbar. It grows only to show what you need, then shrinks back. There is no settings window, no onboarding wizard and no tray popup.

- **One window.** The `overlay` window is the only window. It is always sized to its content plus a transparent margin for shadows (40 px top and sides; the bottom 12 px is the gap above the taskbar) and anchored bottom-center on the work area of a monitor: the foreground window's monitor when a dictation starts, the tray's monitor when opened from the tray, the primary monitor at startup. It grows before a morph and shrinks after it; CSS does the morph. Never full-screen.
- **The pill:** ink (`#1A1917`) in both themes, 36 px high, radius 18. No logo.
  - **Listening:** 11 amber level bars follow the mic. **Hands-free:** latched with an amber ring and a stop button (clickable, never takes focus). **Working:** the bars settle to dots with a sweeping bright dot.
  - **Text pills,** 1.6 s: Didn't catch that, No microphone › (opens the mic picker), Pasted without AI · reason (offline, slow, key invalid…), Copied. Press Ctrl+V (paste was blocked; the text is on the clipboard), Cancelled (Esc), Paused · Resume. Success shows nothing: the text appearing is the confirmation.
- **Idle:** after first run a near-invisible resting tick marks the spot; it turns into a small pill under the pointer and opens on click. The window is click-through then, so hover is detected in Rust (a 20 Hz cursor poll that only runs while the tick is shown).
- **Click-through rules:** click-through while dictating and at rest; the latched hands-free pill and action pills take clicks but never focus (`WS_EX_NOACTIVATE`); only the opened surface takes focus, and it hands focus back to the previous app when it closes.
- **Opened** (click the tick or the tray icon; Esc, × or click away closes): last dictation (click to copy), Hold to talk and Hands-free key caps (hover offers Change), Microphone, Words, AI cleanup with live status (On, Off, Key invalid, Offline), Recent, Pause (1 hour or until resumed) / Resume, Restart to update when one is ready. Rows drill in within the same surface.
  - **Rebind** in place: Press keys, live keys as you hold them, Saved, or refused ("Used by Windows", "Used by Hands-free") and listening again. Esc cancels.
  - **Microphone:** every input with a live level; an unplugged selected device shows as Unplugged. Below the list, a **Sounds** row switches the earcons on or off (the existing `playSoundOnComplete` setting, saved through `update_settings`; off by default). It lives here rather than on Home to keep Home calm.
  - **Words:** one list, "you say → Chirp types", over vocabulary (single terms: hotwords, find/replace, LLM dictionary) and snippets (anything with punctuation or more than three words). Add, edit, delete in place; Enter saves, Esc cancels, leaving the row saves.
  - **AI cleanup:** optional. "Use Claude" is disabled until a key validates. Paste a key, Checking, Connected (Remove), Key not accepted, offline. The key is stored in **Windows Credential Manager** (generic credential `Chirp` / `anthropic-api-key`), validated with the `max_tokens: 1` prewarm request; `ANTHROPIC_API_KEY` stays a developer override and is never stored.
  - **Recent:** the last 20; click to copy, or Add to Words.
- **First run:** no wizard. Getting ready (model download with progress and an estimate) → Windows is blocking the mic (Open settings) if needed → "Hold [Right Ctrl] and talk" → after the first dictation, "You're set. Click me anytime." and the resting tick appears.
- **Paused:** hotkeys stop triggering; pressing one shows the Paused pill instead of failing silently. The tray icon dims and gains a pause badge.
- **Tray:** left click opens the surface. Right click is the native Windows menu: Restart to update (only when an update has downloaded), Open, Pause/Resume, Quit. Paused and update-ready icon variants are drawn in memory over the existing tray icon; brand files are untouched.
- **Updates** download quietly (production builds only) and wait for a restart the user chooses.
- **Keyboard:** opening focuses the first row; arrows move, Enter acts, Tab reaches ×; Esc cancels the innermost thing (key capture, then an edit, then a drill-in), then closes. Focus ring: 2 px amber, offset 2.
- **Motion** (frame 12): pill appears 120 ms; text pills morph 180 ms; pill to surface 240 ms, all `cubic-bezier(0.2, 0, 0, 1)`; content fades in 120 ms after 80 ms; drill-ins slide 16 px in 160 ms; the paste fade is 140 ms to 0.96. Reduced motion turns morphs into 120 ms crossfades.
- **Color:** amber only for meaning: level bars, toggles, focus, the latched ring, "Saved", Resume (`#F0B723` on dark, `#C9900A` / text `#8F6400` on light). Everything else is neutral. Surfaces follow the Windows theme (`#FFFFFF` / `#1A1917`). Type: Segoe UI Variable, 13 px rows, 15 px last dictation, 17 px first-run titles.
- **Sound:** three soft synthesized sweeps, each under 90 ms and around -14 dBFS: **start** rises 1.5 to 2.9 kHz over 70 ms when capture starts, **done** falls 2.9 to 1.9 kHz over 60 ms when text is pasted, and **pasted without AI** is a quieter double chirp. `scripts/generate-sounds.mjs` writes them deterministically to `src-tauri/sounds/`. They are embedded in the binary and played by `play_sound` on a background thread that keeps the output device open for 30 s after the last cue. Playback never blocks capture or paste. The old completion chirp is gone.

Deviations from the Paper file, on purpose:
- The tray icon stays the existing amber bird (`src-tauri/icons/tray-icon.png`); the design's waveform tray glyph is not a brand asset we have.
- "Not connected" microphones are not listed: Windows only enumerates present inputs. Only the selected-but-missing device shows (Unplugged).
- The Words list shows vocabulary before snippets rather than one interleaved order.

## Brand north stars (first pass, 2026-10-24)

What stays is the bird mark (white bird on amber `#F0B723`), the lowercase "chirp" wordmark and amber. What we're after is software whose brand *is* its speed: confident, minimal, a single accent color, keyboard-first.

- **Superhuman** — speed as the brand promise. Shortcuts are first-class UI (key caps shown everywhere), monochrome chrome, color reserved for meaning. Model for the settings and hotkey UI.
- **Linear** — dark, precise, restrained. One gradient moment per page, everything else quiet. Model for the marketing site and README tone.
- **Snapchat** — one bold yellow, used with total confidence on the first screen. Proof that a single saturated brand color plus a simple white mark can carry an entire identity. Model for the onboarding first frame and the app icon.
- **Grok / ElevenLabs voice input** — the voice UI is a thin pill with a live waveform and nothing else. Model for the overlay: smaller and calmer than v1.
- **Monologue (Every)** — the strongest brand among the dictation competitors: tactile, a physical-object metaphor, editorial type. Shows that dictation can have a personality rather than a generic waveform.

**Anti-references:**

- Wispr Flow and Aqua: generic serif-plus-pastel "AI calm".
- Mascot-heavy consumer apps (Finch, Keeta): too cute for a tool.

## Open questions

None currently. CPU support was decided on 2026-10-24 (see below).

## Shipping decision: CPU is supported (2026-10-24)

Users without an NVIDIA GPU get Parakeet v3 int8 on CPU, with up to six threads
(measured synthetic ASR p50 317.2 ms). NVIDIA users get v3 FP32/CUDA (98.6 ms).
CPU is supported, not best-effort. The base installer ships the CPU runtime;
the app installs int8 and VAD without manual setup. After CPU is ready, a usable
NVIDIA driver triggers background, resumable, hash-pinned CUDA runtime and FP32
downloads. GPU activates on the next restart/model load: Windows cannot safely
replace an already-loaded ONNX runtime. A child probes CUDA; initialization and
warmup failures persist CPU fallback until manual retry. Engine/progress IPC
is available for the UI. Release steps: [RELEASING.md](RELEASING.md).

Installer provisioning uses `scripts/setup-windows.ps1 -Bundle` in local builds
and CI. CRT files are required only by the explicit `tauri.bundle.conf.json`
configuration; base builds do not produce installers. This keeps development
independent of installer resources without allowing an incomplete default bundle.
