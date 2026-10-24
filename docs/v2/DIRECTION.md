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
- LLM providers other than Anthropic. Users bring their own Anthropic API key.
- Live transcript preview while speaking.
- "Local-only" as a principle. Speech recognition stays local because it's fastest, not on principle.
- Tone modes. The model infers register and format from the active app; the user never switches anything.
- Telemetry and extras. Sentry, Aptabase, the Discord feedback webhook, announcements, and the Chirp Pro page are removed.

## Pipeline and latency budget

Every stage is timed and logged on every transcript from day one. Targets are revised once the benchmark harness produces real numbers.

| Stage | Approach | Target |
|---|---|---|
| Hotkey → recording | Win32 low-level hook (port from v1.3.4). Mic stream kept warm so pressing the key never waits on device open. | <20ms |
| On key press (parallel) | Pre-open the TLS/HTTP connection to the Anthropic API so the handshake is done before release. | off the critical path |
| Speech → text | sherpa-onnx with the CUDA provider. Start with offline Parakeet TDT over the full recording at release. Move to a streaming model (Nemotron Speech Streaming) only if the measurements say we need it. | <50ms for typical dictation |
| Deterministic pass | Regex cleanup, dictionary replacements, snippets. Unit-tested. | <5ms |
| Router | Decide whether the LLM is needed (see below). | <1ms |
| LLM cleanup | Claude Haiku 5.5 (`claude-haiku-5-5`), thinking disabled, low effort, cached system prompt, active-app context. Hard timeout → paste the deterministic result instead. | measure; aim p50 <1s |
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

### Prompt and safety

- The system prompt is a style guide plus examples, kept stable so it caches.
- The transcript is wrapped as data, never as instructions.
- An output-length guard rejects runaway responses.
- On any error, timeout or refusal, fall back to the deterministic text. Dictation must never fail just because the API did.

## Measurement first: the bench harness

Before any UI work, build a headless harness: WAV in → per-stage timings + output text out.

- **Corpus.** 30–50 real dictations recorded by the maintainer: Slack replies, emails, AI prompts, lists, rambles, self-corrections. Each has a hand-written ideal output.
- **Speed.** Speech-model latency on the 4080 (Parakeet offline vs. Nemotron streaming), plus Haiku 5.5 latency (time to first token, total; cold vs. cached prompt).
- **Quality.** Scored against the ideal outputs, so every prompt change is measured, not eyeballed.

It runs on the Windows machine. The Linux devbox has no GPU and can only run the CPU and API parts.

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

- API key storage: Windows Credential Manager (preferred) vs. settings file.
- CPU fallback for contributors without an NVIDIA GPU: supported, best-effort, or unsupported?
