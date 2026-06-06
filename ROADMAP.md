# Roadmap

## Planned Features

- **Hotkey reliability rework** — Investigate and rework how hotkeys are loaded/unloaded and registered. Currently breaking for some users. Needs deep exploration of the lifecycle and edge cases.
- **Tray icon quick settings redesign** — Redesign the system tray popup to look polished and surface relevant information (status, current settings, recent transcriptions, etc.).
- **Overlay on active monitor** — Recording pill should appear on the monitor being used, not always primary. Use `cursorPosition()` + `availableMonitors()` with DPI-aware positioning. Configurable in settings.
- **Draggable overlay positioning** — Instead of fixed positions, add a drag mode where users can freely position the recording pill anywhere on screen.
- **Dark mode for content area** — Currently only sidebar is dark.
- **Multiple paste methods** — Let users choose Ctrl+Shift+V (terminals), Shift+Insert (legacy), or direct typing (no clipboard) instead of just Ctrl+V.
- **Transcription coordinator** — Move recording lifecycle from frontend to a Rust mpsc channel state machine. Serializes hotkey events, prevents race conditions from rapid presses, adds stuck-state recovery.
- **Tap-to-talk mode** — Tap hotkey to start, tap to stop. RMS-based SmoothedVad with onset/hangover/prefill auto-stops recording after sustained silence.
- **Linux support** — Bring Chirp to Linux with full feature parity.
- **Cmd+Enter to submit feedback** — Keyboard shortcut for the feedback form.
- **LLM download progress after onboarding** — Show model download status on Home page when skipping onboarding.

## Completed

- **F13+ function key support** — Extended function keys as hotkey options.

## Won't Do

- **Arabic language support** — Parakeet ASR does not support Arabic.
- **Auto-learn corrections** — Not feasible.
