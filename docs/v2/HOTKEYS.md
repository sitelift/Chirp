# Hotkey v2 implementation and verification

One Windows message thread in `hotkey_input.rs` owns the keyboard and XBUTTON hooks, held keys, matching, capture, and suppression. A separate channel consumer calls Tauri and records dispatch latency. Callbacks never call app IPC, lock app state, log, or wait for a consumer. Settings changes go to the existing thread without reinstalling hooks.

Bindings contain `{ keys: [{ scan, extended, vk }], label, suppress }`. `holdBinding` and `toggleBinding` are independently nullable. Keyboard identity is scan + extended; VK supplies modifier classification, async-state queries, and the identity fallback for scan-less media/mouse events. Defaults are Right Ctrl for hold and Left Ctrl + Left Win for toggle. Hardware Fn keys that produce no Windows event cannot be captured.

Matching requires every binding key and excludes extra modifiers; unrelated held keys are ignored. Rising match edges fire once; auto-repeat does not repeatedly toggle. Hold emits release on a falling edge. The overlay tracks which binding started recording, so a hold release cannot stop a toggle-started recording. A release or toggle during recording startup queues the stop.

Suppression consumes a non-modifier down that completes a matching binding, its repeats, and its eventual up. Earlier chord prefixes pass through: arbitrary combinations cannot be predicted before completion without delaying/replaying typing. Modifiers always pass through; modifier-only bindings never suppress. Swallowed-down bookkeeping survives release-order and configuration changes. A dummy VK 0xFF pair masks Win on activation and immediately before Win-up, including Win-last chords. Injected input does not activate bindings.

A hidden zero-size top-level window receives WTS session and power notifications. The input loop notices foreground changes within 20 ms and reconciles with GetAsyncKeyState. It also reconciles after repeated downs and at 250 ms intervals. Suppressed downs are absent from Windows async state, so ordinary reconciliation retains their release bookkeeping; session/focus reconciliation can clear their held state. Resync can stop an orphaned hold but cannot manufacture a press. Hooks and the window are removed on shutdown.

Capture uses the same hooks, suspends actions, and gathers keys until all captured keys are released. Esc, explicit cancellation, a 15-second deadline, session changes, or settings changes end capture. Cancelled captures inhibit actions until held keys are released. Duplicate bindings and common Windows shortcuts are rejected after capture and before saving. Secure-attention sequences such as Ctrl+Alt+Del cannot be intercepted by an ordinary desktop app; session interruption/timeout recovers capture.

`hotkey_migration.rs` is the only remaining string-name mapping. It reads legacy rdev/browser/Tauri/VK spellings and writes the new schema. Unknown legacy bindings fall back to defaults with a warning. Tap migrates to toggle; hold migrates to hold. Explicit null remains disabled. Labels are regenerated on load/save using GetKeyNameTextW. The rdev dependency and old Raw Input/suppression modules are removed.

Win32 references: [hook timing and async state](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc), [native key labels](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeynametextw).

## Automated checks

Use the environment in AGENTS.md with `CARGO_TARGET_DIR=C:/tmp/chirp-target-hotkey`. Gitignored sherpa DLLs/import libraries were copied from the main checkout.

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo build --manifest-path src-tauri/Cargo.toml`
- `npm.cmd run build`
- `npm.cmd run lint`

Tests cover matching, identity, sides, labels, F13/F24/media/OEM, capture/cancellation, repeat, release ordering, Win masking, resync, and migration, alongside existing application tests.

Verified on 2026-10-24: all four commands pass; 82 Rust tests pass (22 hotkey tests). Existing warnings remain for the deprecated tray-menu method, unused `vad_model_exists`, and outdated Browserslist data. No live dictation or physical-key verification was performed.

## Maintainer manual checklist

1. Quit other Chirp instances. Hold/release Right Ctrl; toggle with Left Ctrl + Left Win twice. Try fast taps during microphone startup and releasing hold during a toggle recording.
2. Record both sides of modifiers, modifier-only chords, F13/F24, media/OEM keys, keypad Enter, and Mouse 4/5. Verify labels and activation with an unrelated letter held, then an extra modifier held.
3. Toggle suppression in Notepad. Test repeats and both release orders. Test Win-first and Win-last chords: Start must not pop after a suppressed chord. Modifier-only suppression must stay disabled.
4. Cancel capture with Esc and the button; let it time out; repeat several times. Leftover capture keys must not start recording. Try duplicate/reserved shortcuts and verify the error.
5. Hold across focus changes, lock/unlock, UAC, and sleep/resume; release on the other desktop. Verify recovery and no stuck recording. Test multiple keyboards and AltGr.
6. Disable each binding and both together; restart. With a backup, test migrating old hold/tap settings, then restore the original settings.
7. Dictate into a real app; verify paste and input/end-to-end timings. Physical device behavior, hook latency, Start-menu masking, secure desktop transitions, and actual dictation/paste require human validation.
