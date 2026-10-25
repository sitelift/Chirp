# v2 redesign: build spec

The approved design is the working mockup in this folder, `motion.html`. Open it in a browser:
it is the visual and motion reference for every screen, state and transition, in both themes.
This file records the decisions and numbers behind it, and the contracts the build tasks share.
Where this file and the mockup disagree, ask; do not guess.

## Principles

- The pill is the app. One window, sized once, never resized during an animation.
- Motion is geometry, opacity and transform. Never animate `clip-path` (it did not repaint in
  one renderer we tested). Verify motion in real playback, not by seeking paused animations.
- Corner rule: anything nested inside a rounded shape sits the same distance from both edges,
  and its radius is the outer radius minus that distance. Card 18, rows 12 (6 inset), keycap
  highlight 10 around 6 px keys (4 inset), segmented thumb 6 in an 8 track (2 inset), back
  button 8 at 10 px from both edges.
- No grey: the dark ink pill never fades over the white panel. Light/dark swaps happen only at
  pill or tick size, or as a whole-page theme crossfade.

## Window (surface.rs + Surface.tsx)

- The overlay window is created once at the **envelope** size and only moves when the anchor
  monitor changes. Envelope = widest panel (600) and tallest panel (440) plus margins
  (40 top, 40 sides, 12 bottom): **680 x 492** logical px, bottom-centred on the work area.
- `WS_EX_LAYERED` is set once and never removed. Only `WS_EX_TRANSPARENT` (and
  `WS_EX_NOACTIVATE`) change, without `SWP_FRAMECHANGED`.
- **Hit rect.** The frontend sends `inset` (top, right, bottom, left, logical px from the
  window edges) describing the one interactive thing on screen: the hover pill in `tick`,
  the pill in `interactive`, the panel in `focused`. The backend polls the cursor at 60 Hz
  while a hit rect is active and makes the window click-through whenever the cursor is
  outside it (6 px slack once inside). `passive` is always click-through. `surface-hover`
  is still emitted in `tick` only. In `focused`, a click outside the panel therefore reaches
  the app underneath, the window blurs, and the panel closes.
- During a push between two panel sizes the frontend sends the union of both rects, then the
  final rect when the animation ends.

## Layout

Panel width 320 (Home, Recent, Words, Snippets); Settings 600 x 400. Panel padding 6.

- **Home**: last dictation (click copies), Recent >, Words (count) >, Snippets (count) >,
  divider, bottom bar: Pause (left), Settings (right). No close button.
- **Recent**: header (back + title), rows: text (ellipsis) + time column right-aligned, tabular.
- **Words**: header, add field ("Add a name or word", Enter or + adds), plain list of terms,
  hover shows remove (x). Hint: "Chirp spells these exactly as written."
- **Snippets**: header, two-line form ("When I say" / "Type this", divider inset 12), list of
  trigger + expansion, hover remove. Hint: "Say the phrase and Chirp types the rest."
- **Settings** (wide): sidebar with back + "Settings", sections Chirp, Shortcuts, Microphone,
  Sound, AI cleanup; sliding selection marker; selected icon in accent. Divider runs edge to edge.
  - Chirp: "How Chirp looks on your PC." Appearance System / Light / Dark ("Follows Windows",
    "Always light", "Always dark"). Note: "Chirp 2.0.0".
  - Shortcuts: "Click a shortcut to change it." Hold to talk ("Hold, speak, let go."),
    Hands-free ("Tap on, tap off."), Cancel ("Stops dictating and throws it away.", default Esc).
    Click the keys to rebind; "Press keys" dashed accent state.
  - Microphone: "Say something. The one that moves is the one that hears you." Radio rows with
    live level bars.
  - Sound: "What you hear while you dictate." Sounds toggle ("A soft tone when recording starts
    and stops.", off by default), Music while dictating Off / Lower / Pause ("Turns other apps
    down."), Quieter by slider revealed under Lower.
  - AI cleanup: "Removes filler words, fixes punctuation and formats text for the app you're in."
    Provider Off / Anthropic / OpenAI (model line: "No cleanup", "Claude Haiku 5.5",
    "GPT-6 Luna"). API key block hidden when Off: field (masked key + status "Checking" /
    "Connected", or paste input) and a same-height button (Remove / "Get a key"). Note:
    "Your dictation is sent to {Anthropic|OpenAI} to be cleaned up." / "Your dictation stays on
    this PC."

All copy is in the mockup verbatim.

## Motion (all numbers from the mockup)

- Ease out `cubic-bezier(0.32, 0.72, 0, 1)` for movement; linear for fades.
- Big moves run at PACE 1.1: open 330 ms, push 330 ms.
- Hover: tick 28 x 5 to pill 56 x 36 in 160 ms; bird fades in and scales from 0.4, -12 deg.
- **Open**: panel's real width/height grows from 56 x 36 to the panel size (330 ms); panel
  opacity 0 to 1 in 110 ms; ink pill fades out and scales to 1.12 in 90 ms; content rises 8 px
  and fades in (240 ms, 60 ms delay) and tracks the centre while the panel widens.
- **Close**: 300 ms, `cubic-bezier(0.4, 0, 0.2, 1)`. Content fades in 70 ms and tracks the
  centre. Panel size goes to the pill (56 x 36, radius 18) at 72%, then to the tick (28 x 5,
  radius 3, lifted 15.5 px). Panel opacity holds to 70%, then fades; the tick fades in over the
  last 90 ms.
- **Push**: both sheets locked edge to edge on one curve; the panel resizes to the next sheet;
  both sheets are as tall as the taller for the whole move; the detail sheet's left edge casts
  `-12px 0 20px -12px` shadow.
- Settings sections: marker slides 280 ms; old pane fades 120 ms; new pane fades in and rises
  6 px over 240 ms.
- Rows add/remove: height 0 to row height with fade, 260 / 220 ms; the panel follows.
- Theme change: 220 ms crossfade (View Transitions).

## Icons

Custom set, in `motion.html` as `<symbol>`s: 24 grid, 2 px stroke, round caps and joins, keylines
circle 20 / square 18 / landscape 20 x 16 / portrait 16 x 20. Chirp (bird outline, derived from
the brand path, fitted to the 20-unit live area), Recent (bubble + clock hands), Words (Aa),
Snippets (lines + bolt), Pause (circle + bars), Settings (sliders), Shortcuts (keyboard),
Microphone, Sound, AI cleanup (robot), Copy, Add, Remove, Open, Back, Next. Replace Lucide.

## Contracts

### Settings (done: `state.rs`, `types.ts`)

`provider: 'off' | 'anthropic' | 'openai' | null` (null = legacy; resolve with
`Settings::provider()`, i.e. `aiCleanup ? anthropic : off`), `appearance: 'system' | 'light' |
'dark'`, `cancelBinding: Binding | null` (default Esc). `aiCleanup` stays only for migration.

### Words and Snippets

- `VocabEntry` keeps `{ term, replaces }` on disk for compatibility; `replaces` is ignored and
  the frontend writes `[]`. Words are passed to the cleanup model as spellings only.
- Snippets are unchanged (`get_snippets` / `update_snippets`).

### AI providers (ai.rs, llm.rs)

- `get_ai_status() -> AiStatuses`, `set_api_key({ provider, key }) -> AiStatuses`,
  `remove_api_key({ provider }) -> AiStatuses`, event `ai-status` with `AiStatuses`.
- `AiStatuses = { anthropic: AiStatus, openai: AiStatus }`, `AiStatus = { state, keyHint,
  fromEnv }` as today. Hints: `sk-ant-••••••••••abcd` for Anthropic, `sk-••••••••••abcd` for
  OpenAI.
- Keys in Windows Credential Manager: service `Chirp`, accounts `anthropic-api-key` (existing)
  and `openai-api-key`. Env overrides `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`.
- OpenAI model `gpt-6-luna`, same system prompt and request shape, same timeout, refusal and
  fallback handling.

### Shortcuts

`record_binding({ target: 'hold' | 'toggle' | 'cancel' })`. The cancel binding replaces the
hard-coded Esc; it only acts while dictating and must not conflict with the other two.

## Pipeline

Parakeet is speech recognition only: no hotword biasing, greedy decoding allowed, no recognizer
rebuild on Words edits. The deterministic cleanup is removed except **spoken commands** ("new
line", "new paragraph", "comma", "period", "question mark", ...), which are always converted.
Raw text, with commands converted and snippets expanded, is the fallback and the result when the
provider is Off. The router decides skip vs cleanup on that text. The VAD lead-in, empty-segment
retry and dropped-segment recovery stay as they are.
