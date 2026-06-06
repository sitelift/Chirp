# Chirp iOS — Incremental Build Plan

## Context

Chirp is a Tauri 2 desktop dictation app for Windows and macOS. It captures audio through cpal, transcribes with NVIDIA Parakeet TDT 0.6B v3 via sherpa-onnx, runs a regex cleanup pass, optionally runs a fine-tuned 0.6B `chirp-cleanup-v2` LLM through `llama-server`, and injects the result at the cursor via clipboard + `Ctrl/Cmd+V`. The user wants a native iOS port that preserves the speech-to-text pipeline and ships in the App Store.

The full feasibility analysis is at `~/.claude/plans/hidden-whistling-honey.md`. Read it first if you have not. Key conclusions from that doc:

- iOS keyboard extensions cannot directly access the microphone (Apple sandbox restriction, error 561145187, still in force on iOS 18). Recording must happen in the containing app.
- The shipped pattern (used by Wispr Flow): containing app declares `UIBackgroundModes: ["audio"]`, stays alive in the background holding an active `AVAudioSession`, and is controlled by the keyboard extension over Darwin notifications + an App Group container. The user only sees an app-switch once per "session" (default ~5 minutes of inactivity), not once per dictation.
- ASR runtime: [FluidAudio](https://github.com/FluidInference/FluidAudio) — Apache 2.0 Swift package that ships the same Parakeet TDT 0.6B v3 model Chirp uses on desktop, pre-converted to Core ML and ANE-accelerated. Published benchmark: 128.6× RTFx median on iPhone 16 Pro Max, WER 2.1% on LibriSpeech test-clean. A 5-second utterance transcribes in ~40 ms.
- LLM cleanup runtime: [mlx-swift](https://github.com/ml-explore/mlx-swift) (Apple-maintained), with model loading via [mlx-swift-examples](https://github.com/ml-explore/mlx-swift-examples) `MLXLLM` package. ~20–40 tok/s for 0.6B-class models on iPhone 15 Pro.
- The single biggest ongoing cost is maintaining the Swift port of `cleanup.rs` in lockstep with the Rust desktop version via a shared golden-fixture corpus.

## Approach: Incremental Real Build with Verification Gates

This is **not a spike**. We are building the real iOS app. Each increment is small enough to land in one or two sittings, ends with an explicit verification gate that must pass on a real iPhone, and produces production-quality code that the next increment builds on. We do not advance until the current gate passes. If a gate fails, we either fix the problem in place or revisit the architecture before adding more layers on top.

The order is risk-first: the increments that could kill the project (ASR perf, mic-in-background, keyboard ↔ app IPC) come early. Polish, onboarding, and store submission come last.

## Locked Architectural Decisions (do not revisit during build)

These are decided. Do not relitigate them in the middle of an increment.

| Decision | Choice |
|---|---|
| Repo | New separate `chirp-ios` repo, NOT inside the existing `chirp` Tauri repo |
| Language | Pure Swift, no Rust on iOS |
| UI framework | SwiftUI |
| Minimum iOS | 17.0 (FluidAudio requirement, also where ANE perf matured) |
| Lowest target device | iPhone 12 (the user noted this "might be a little low" — flagged in Open Questions) |
| Bundle ID, app | `com.chirp.ios` (placeholder — user to confirm against their existing App Store account; rename in increment 0 if wrong) |
| Bundle ID, keyboard | `com.chirp.ios.keyboard` |
| App Group identifier | `group.com.chirp.shared` |
| Apple Developer account | Paid (already enrolled) |
| ASR runtime | FluidAudio (Parakeet TDT 0.6B v3 Core ML) |
| LLM runtime | mlx-swift + mlx-swift-examples MLXLLM |
| Audio format | 16 kHz mono Float32 PCM, FluidAudio's expected input |
| Settings/vocab/history persistence | App Group container, JSON files mirroring desktop schemas |
| Keyboard ↔ app IPC | Darwin notifications (`CFNotificationCenter`) + App Group shared files. URL scheme `chirp://wake` only as cold-session fallback. |
| Background mic | `UIBackgroundModes: ["audio"]` on the containing app, holding an active `AVAudioSession.playAndRecord` |

## Increment Roadmap

| # | Increment | Sittings | Verification gate |
|---|---|---|---|
| 0 | Project foundation: repo, Xcode project, two targets, App Group, package deps, empty SwiftUI shell | 1 | Builds and runs on iPhone 12 from a fresh `git clone` |
| 1 | Audio capture in containing app: AVAudioEngine, format conversion, mic permission flow | 1 | Tap button, speak 5 s, see sample count + dB meter; permission prompt fires once |
| 2 | FluidAudio Parakeet ASR: load on launch, transcribe captured audio, display result + timing | 1–2 | Transcript appears, latency ≤200 ms on iPhone 12, visually matches desktop on a shared test sentence |
| 3 | Background session: `UIBackgroundModes: ["audio"]`, keep `AVAudioSession` alive across backgrounding | 1 | Background app for 5 min, return, mic still works without reload, model still loaded |
| 4 | Swift port of `src-tauri/src/cleanup.rs` with shared golden-fixture corpus | 2 | Swift port passes the same JSON fixture corpus as Rust version |
| 5 | mlx-swift LLM cleanup with stand-in 0.6B model (Qwen2.5/3 Instruct from mlx-community) | 1–2 | End-to-end pipeline: record → ASR → regex → LLM → display, total ≤4 s post-stop on iPhone 12 |
| 6 | App Group container + settings/vocabulary/history persistence (mirror desktop JSON schemas) | 1–2 | Settings survive launch and reboot, vocabulary file is read by both app and (eventually) extension |
| 7 | Hotword/contextual biasing wired into FluidAudio's recognizer config | 1 | Custom vocab word (e.g. proper noun) transcribes correctly when added to vocabulary |
| 8 | Keyboard extension target: thin SwiftUI keyboard, Darwin-notification IPC to containing app | 2–3 | Tap mic in keyboard inside Notes.app → text appears, no app switch (steady-state path) |
| 9 | Cold-session bounce: `chirp://wake` URL scheme, fallback when containing app is jetsamed or session expired | 1 | Reboot phone, tap mic, app wakes, dictation completes, returns to host app |
| 10 | Onboarding flow: install keyboard, Full Access prompt, mic grant, model downloads | 2 | Fresh install runs end-to-end through onboarding to first dictation in Notes |
| → | Re-quantize `chirp-cleanup-v2` to MLX format and replace stand-in model | separate task | Output parity with desktop on a shared test corpus |
| → | TestFlight build, beta, App Store submission | separate | App approved |

The user makes the go/no-go call at every gate. If gate 2 (ASR perf) fails, the rest of the plan is blocked. If gate 5 (LLM perf) fails, we ship v1 regex-only and defer the LLM. If gate 8 (keyboard IPC) fails, we revisit the architecture before doing 9–10.

## Foundation Conventions

These apply to every increment. Set them up correctly in increment 0; do not deviate later.

- **File organization**: one type per file. Group by feature, not by layer (e.g. `Audio/`, `ASR/`, `Cleanup/`, `KeyboardIPC/`, `UI/`), not `Models/`, `Views/`, `Controllers/`.
- **Concurrency**: `async`/`await` everywhere, `@MainActor` on classes that touch UI, `actor` for anything holding shared mutable state across tasks. No completion handlers, no GCD primitives unless calling into a C API that requires them.
- **Errors**: typed Swift errors per subsystem (e.g. `enum AudioCaptureError: Error`, `enum ASRError: Error`). Surface them in UI as a status string, never as a fatal alert during development. Never `try!` outside of test code.
- **Logging**: `os.Logger` with subsystem `com.chirp.ios` and a per-feature category (`audio`, `asr`, `cleanup`, `llm`, `ipc`, `ui`). No `print()` in committed code. Log levels matter — debug for internal state, info for user-visible events, error for things that should never happen.
- **Testing**: every increment that introduces logic adds an `XCTest` target test for that logic. The Swift port of `cleanup.rs` must run the shared golden-fixture corpus as parameterized tests. Pure-perf increments (like increment 2's ASR latency) don't need unit tests but should have a documented manual measurement procedure.
- **No force-unwraps** in app code. Optional binding or `guard let`.
- **No SwiftUI previews are required**, but if you write one, it must compile in CI (which means no live ASR loading in the preview body).
- **Commits**: one increment per branch, branched from `main`, merged after the verification gate passes. PR title format: `incN: <one-line description>`. CHANGELOG.md updated in each PR (mirroring desktop chirp's discipline).
- **No emojis in code, comments, commit messages, or UI** (matches desktop Chirp convention; use SF Symbols for icons).
- **Color and typography**: defer the design system until we have something working. For now use system defaults. Don't bikeshed colors before increment 5.

## Increment 0: Project Foundation

**Goal:** A `chirp-ios` git repo containing an Xcode project with two correctly-configured targets (containing app + keyboard extension), all package dependencies resolved, App Group entitlement provisioned, and an empty SwiftUI shell that builds and runs on a connected iPhone 12.

**Out of scope for this increment:** any actual functionality, audio code, ASR code, IPC code, design system. We're building the slab the rest of the project sits on.

### Steps

1. **Create the repo**
   - On the Mac Mini: `mkdir ~/chirp-ios && cd ~/chirp-ios && git init`
   - Create `.gitignore` covering Xcode (`xcuserdata/`, `*.xcodeproj/project.xcworkspace/xcuserdata/`, `DerivedData/`, `.swiftpm/xcode/`, `*.xcuserstate`, `Pods/`, `.DS_Store`)
   - Add empty `README.md` and `CHANGELOG.md`. CHANGELOG starts with `## [Unreleased]` heading.
   - Initial commit.

2. **Verify Xcode environment**
   - `xcodebuild -version` reports Xcode 15.4 or later.
   - User is signed into Xcode with their paid Apple Developer team. Verify by opening Xcode → Settings → Accounts.
   - Connected iPhone 12 visible in Window → Devices and Simulators. Developer Mode enabled on the device.
   - **If any of these fail, stop and report.**

3. **Create the Xcode project**
   - File → New → Project → iOS → App
   - Product name: `Chirp`
   - Team: the user's paid team (whichever appears in the dropdown)
   - Organization identifier: derived from bundle ID — if `com.chirp.ios` works, use `com.chirp`; if the user reports their existing App Store apps use a different prefix (e.g. `com.acme.chirp`), use `com.acme` and update the locked-decisions table at the top of this spec.
   - Interface: SwiftUI
   - Language: Swift
   - Storage: None
   - Include Tests: yes (we use the test target from increment 4 onward)
   - Save inside `~/chirp-ios/`. The Xcode project ends up at `~/chirp-ios/Chirp.xcodeproj` with the source under `~/chirp-ios/Chirp/`.

4. **Set deployment target and devices**
   - Target `Chirp` → General → Minimum Deployments → iOS 17.0
   - Supported destinations: iPhone only (uncheck iPad, Mac Designed for iPad, Vision)
   - Device orientation: Portrait only

5. **Add the keyboard extension target**
   - File → New → Target → iOS → Custom Keyboard Extension
   - Product name: `ChirpKeyboard`
   - Bundle identifier: `com.chirp.ios.keyboard` (matches main app + `.keyboard`)
   - Embed in Application: `Chirp`
   - Language: Swift
   - This creates `ChirpKeyboard/` next to `Chirp/`. Xcode auto-generates a `KeyboardViewController` stub. **Leave the stub alone for now** — increment 8 builds it out.

6. **Configure App Group entitlement**
   - In Apple Developer portal (developer.apple.com → Certificates, Identifiers & Profiles → Identifiers → App Groups), create the group `group.com.chirp.shared` if it does not exist.
   - In Xcode, both `Chirp` and `ChirpKeyboard` targets → Signing & Capabilities → + Capability → App Groups → check `group.com.chirp.shared`.
   - Verify the entitlements file for each target now contains the `com.apple.security.application-groups` array with that ID.

7. **Add Info.plist entries on the main app target**
   - `NSMicrophoneUsageDescription`: `Chirp uses your microphone for voice dictation. Audio is processed entirely on your device and never sent to a server.`
   - `UIBackgroundModes`: array containing `audio` (this is the load-bearing entitlement; without it, increment 3 fails)
   - `LSApplicationQueriesSchemes`: not needed yet — added in increment 9
   - URL scheme `chirp` registered under `CFBundleURLTypes`: not needed yet — added in increment 9

8. **Add Swift Package dependencies**

   In Xcode: File → Add Package Dependencies. Add each of the following with the version rule "Up to next major version" from the latest stable release, then link the listed products to the listed targets.

   | Package URL | Products | Linked to |
   |---|---|---|
   | `https://github.com/FluidInference/FluidAudio` | `FluidAudio` | `Chirp` (not the extension) |
   | `https://github.com/ml-explore/mlx-swift` | `MLX`, `MLXNN`, `MLXOptimizers`, `MLXRandom` | `Chirp` |
   | `https://github.com/ml-explore/mlx-swift-examples` | `MLXLLM`, `MLXLMCommon` | `Chirp` |
   | `https://github.com/apple/swift-log` | `Logging` | `Chirp`, `ChirpKeyboard` |

   Notes:
   - **None of these packages are linked to the keyboard extension target.** The extension stays small. It only depends on `Logging` and (later) the App Group bridge code that we add directly to the keyboard target.
   - mlx-swift-examples is a moving target. If product names have changed, mirror whatever the current `LLMEval` example in the mlx-swift-examples repo links to.
   - First package resolution will take several minutes. mlx-swift compiles a lot of code on first build.

9. **Create the source folder layout**

   Inside `Chirp/`:

   ```
   Chirp/
   ├── ChirpApp.swift              # @main, just renders RootView()
   ├── UI/
   │   └── RootView.swift          # Empty SwiftUI view: Text("Chirp"), nothing else for now
   ├── Audio/                      # populated in increment 1
   ├── ASR/                        # populated in increment 2
   ├── Cleanup/                    # populated in increment 4
   ├── LLM/                        # populated in increment 5
   ├── Storage/                    # populated in increment 6
   ├── KeyboardIPC/                # populated in increment 8
   ├── Logging/
   │   └── Loggers.swift           # Centralized os.Logger instances per category
   └── Resources/
       └── Assets.xcassets         # Auto-generated, leave as-is
   ```

   Empty folders should contain a `.gitkeep` so they survive `git add`.

10. **Loggers.swift content** (the only "real" code in increment 0)

    ```swift
    import os.log

    enum Loggers {
        static let subsystem = "com.chirp.ios"
        static let audio    = Logger(subsystem: subsystem, category: "audio")
        static let asr      = Logger(subsystem: subsystem, category: "asr")
        static let cleanup  = Logger(subsystem: subsystem, category: "cleanup")
        static let llm      = Logger(subsystem: subsystem, category: "llm")
        static let ipc      = Logger(subsystem: subsystem, category: "ipc")
        static let ui       = Logger(subsystem: subsystem, category: "ui")
    }
    ```

11. **ChirpApp.swift content**

    ```swift
    import SwiftUI

    @main
    struct ChirpApp: App {
        var body: some Scene {
            WindowGroup { RootView() }
        }
    }
    ```

12. **RootView.swift content** (placeholder, replaced in every subsequent increment)

    ```swift
    import SwiftUI

    struct RootView: View {
        var body: some View {
            VStack(spacing: 16) {
                Text("Chirp").font(.largeTitle).bold()
                Text("Increment 0: foundation only").foregroundStyle(.secondary)
            }
            .padding()
        }
    }
    ```

13. **First commit**: branch `inc0-foundation`, commit all of the above with a CHANGELOG entry.

### Verification gate (Increment 0)

All of the following must pass:

- [ ] `git clone` the repo to a clean folder, open `Chirp.xcodeproj`, hit ⌘B — builds without errors or warnings.
- [ ] Connect iPhone 12, select it as run destination, hit ⌘R — app launches on the device, shows "Chirp / Increment 0: foundation only".
- [ ] App icon is the default Xcode placeholder (not a concern yet, just verifying Xcode didn't fail to bundle).
- [ ] Xcode → Window → Devices and Simulators → select iPhone 12 → Installed Apps shows both `Chirp` and `ChirpKeyboard` (the keyboard appears in the device's Settings → General → Keyboard → Keyboards → Add New Keyboard list).
- [ ] Both targets' entitlements files contain `group.com.chirp.shared` under `com.apple.security.application-groups`.
- [ ] `Loggers.audio.info("...")` shows up in Console.app filtered by subsystem `com.chirp.ios` when triggered (you can add a one-line trigger in `RootView.onAppear` to verify, then remove it before merging).

If anything in this list fails, stop and fix before moving to increment 1.

### Branch and PR

- Branch: `inc0-foundation`
- PR title: `inc0: project foundation, two targets, app group, package deps`
- Merge to `main` only after the gate passes.

---

## Increment 1: Audio Capture

**Goal:** A button on `RootView` that, when tapped, records 5 seconds of audio from the iPhone's microphone, converts it to 16 kHz mono Float32 PCM, and displays the resulting sample count and a peak dB meter. The mic permission prompt fires the first time and is handled gracefully.

**Why this comes before ASR:** if the audio pipeline is broken, the ASR pipeline will look broken too and we won't know which one to debug. Get audio capture nailed first, then add ASR on top.

### New files

- `Audio/AudioCaptureError.swift` — typed errors (`permissionDenied`, `engineFailedToStart`, `formatConversionFailed`)
- `Audio/AudioRecorder.swift` — the actual recorder, an `actor` or `@MainActor class`
- `Audio/AudioFormatConverter.swift` — wraps `AVAudioConverter` for the input → 16 kHz mono Float32 conversion
- `UI/RootView.swift` — replaced: now has a "Record 5 seconds" button, status text, sample count, peak dB

### `AudioRecorder` responsibilities

- `func recordFiveSeconds() async throws -> [Float]` — does the following:
  1. Requests mic permission via `AVAudioApplication.requestRecordPermission` (iOS 17+ API). If denied, throws `permissionDenied`.
  2. Configures `AVAudioSession.sharedInstance()` with category `.record`, mode `.measurement`, `try setActive(true)`.
  3. Creates an `AVAudioEngine`, gets `engine.inputNode`, reads its `outputFormat(forBus: 0)`.
  4. Installs a tap on the input node at the native format (commonly 48 kHz stereo on iPhone). The tap handler appends incoming `AVAudioPCMBuffer` instances to an internal array.
  5. Starts the engine.
  6. After exactly 5 seconds (`try await Task.sleep(for: .seconds(5))`), removes the tap, stops the engine.
  7. Concatenates the captured buffers, converts to a single 16 kHz mono Float32 `[Float]` array via `AudioFormatConverter`, returns it.
- On any error, deactivates the audio session and rethrows.

### `AudioFormatConverter` responsibilities

- `func convert(_ buffers: [AVAudioPCMBuffer], from inputFormat: AVAudioFormat, to targetFormat: AVAudioFormat) throws -> [Float]`
- Uses `AVAudioConverter`. Output format is hardcoded for now to `AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16000, channels: 1, interleaved: false)!`.
- Must handle the case where the input is stereo by averaging channels (or taking channel 0) before downsampling.
- Validates that the output sample count is approximately `inputDurationSeconds * 16000`. If it's wildly off, throw `formatConversionFailed`.

### `RootView` updates

- A `@State` for the recorder's status (`idle | requestingPermission | recording | converting | done(samples: Int, peakDB: Float) | error(String)`)
- A "Record 5 seconds" button — disabled while not idle
- Status text reflecting the current state
- When `done`: show "Captured N samples (X seconds at 16 kHz), peak Y dB"
- When `error`: show the error message

### Tests

- `AudioFormatConverterTests`: feed it a synthetic `AVAudioPCMBuffer` of known content (e.g. 48 kHz stereo sine wave), verify the output is 16 kHz mono with the expected sample count and a non-zero peak. This test runs on the simulator without needing the mic.
- No test for `AudioRecorder` itself — it requires real hardware. Manual gate covers it.

### Verification gate (Increment 1)

- [ ] App launches on iPhone 12, shows the new RootView with "Record 5 seconds" button.
- [ ] First tap of the button triggers the iOS mic permission prompt. Accept it.
- [ ] After accepting, button proceeds to record. Status shows "recording" for 5 seconds.
- [ ] After 5 seconds, status shows "Captured ~80000 samples (5.0 seconds at 16 kHz), peak X dB".
- [ ] Sample count is between 79000 and 81000 (allows for buffer alignment slop).
- [ ] Peak dB is non-zero when speaking, near zero in silence.
- [ ] Tap the button a second time without speaking → records, peak dB is very low.
- [ ] Force-quit the app, delete it, reinstall, verify the permission prompt fires again on first tap of the new install.
- [ ] Deny the permission once on a fresh install → button shows the `permissionDenied` error, does not crash.
- [ ] Unit test `AudioFormatConverterTests` passes in the simulator.

### Branch and PR

- Branch: `inc1-audio-capture`
- PR title: `inc1: AVAudioEngine 5-second capture with 16 kHz mono Float32 conversion`

---

## Increment 2: FluidAudio Parakeet ASR

**Goal:** After the user records 5 seconds, the app transcribes the captured audio with FluidAudio's Parakeet TDT 0.6B v3 Core ML model and displays the transcript along with the inference time. Model is loaded on app launch and stays warm. Cold-load time is measured and displayed.

**Why this is the highest-stakes increment:** this is the gate that decides whether the iOS port is feasible at all. If FluidAudio's published 128× RTFx doesn't hold on the iPhone 12, or if the model exceeds the device's memory budget, or if FluidAudio's Swift API has drifted in a way we can't work around, the project pauses here and we re-evaluate.

### New files

- `ASR/ASRError.swift` — typed errors
- `ASR/ASRRunner.swift` — `@MainActor class ASRRunner: ObservableObject`
- `ASR/ASRStatus.swift` — published status enum (`notLoaded | loading | ready(coldMs: Double, warmMs: Double) | transcribing | error(String)`)
- `UI/RootView.swift` — replaced again: now has a status header showing ASR readiness, the record button (now disabled until ASR is ready), and a transcript display

### `ASRRunner` responsibilities

- Holds an `AsrManager` instance for its lifetime (do not recreate per call).
- `func loadModel() async` — kicks off in `RootView.onAppear` or in a `Task` from `ChirpApp.init()`. Internally:
  1. Calls `AsrModels.downloadAndLoad()` (or whatever the current FluidAudio API is — read the package's README at the version pinned in `Package.resolved` to confirm). Times the wall clock with `ContinuousClock`.
  2. Constructs `AsrManager(config: .default)`.
  3. Calls `try await manager.initialize(models:)`.
  4. Records the elapsed as `coldLoadMs`.
  5. Immediately runs a throwaway transcription on `[Float](repeating: 0, count: 8000)` (0.5 s of silence) to force any lazy compile.
  6. Records that as `warmLoadMs`.
  7. Sets status to `.ready`.
- `func transcribe(_ samples: [Float]) async throws -> (text: String, latencyMs: Double)`:
  1. Sets status to `.transcribing`.
  2. Calls `try await manager.transcribe(samples, source: .system)` (or current API).
  3. Returns `(result.text, elapsedMs)`.
  4. Sets status back to `.ready`.
- On `deinit`, calls `manager.cleanup()` if FluidAudio's API requires it.

### Pre-warming on launch

In `ChirpApp`:

```swift
@main
struct ChirpApp: App {
    @StateObject private var asr = ASRRunner()
    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(asr)
                .task { await asr.loadModel() }
        }
    }
}
```

The `task` modifier runs the load on first appearance and cancels on disappearance. For increment 2 this is fine — increment 3 will move it to a longer-lived holder when we add background mode.

### `RootView` updates

- Top: ASR status header. While loading, show a `ProgressView` and "Loading Parakeet…". When ready, show "Parakeet ready (cold X ms, warm Y ms)".
- Middle: the record button from increment 1, now disabled unless ASR status is `.ready`.
- Below the record button: the dB meter / sample count from increment 1.
- Below that: the transcript (an empty `Text` until the first transcription completes), labeled "Transcript:". Below the transcript: "ASR latency: X ms".

### Logging

- `Loggers.asr.info("ASR cold load \(coldMs) ms")`
- `Loggers.asr.info("ASR warm load \(warmMs) ms")`
- `Loggers.asr.info("ASR inference \(latencyMs) ms for \(samples.count) samples")`
- `Loggers.asr.error("ASR error: \(error)")`

### Tests

- No automated tests for ASR — it requires real hardware and a real model. Manual measurement procedure documented in the verification gate.

### Verification gate (Increment 2)

This is the project-go/no-go gate. Capture the numbers in the PR description.

- [ ] App launches on iPhone 12. The first launch shows "Loading Parakeet…" for some seconds (model download + cold load). The download is several hundred MB; the first run requires Wi-Fi.
- [ ] After loading, status shows "Parakeet ready (cold X ms, warm Y ms)". Record both numbers.
- [ ] Force-quit and re-launch (warm-cache scenario). New cold load time is much lower than the first one because the encoder `.mlmodelc` is cached. Record this.
- [ ] Tap the record button, speak: *"Hey Chirp, this is a test sentence with twenty three numbers and a comma, period."*
- [ ] Transcript appears, ASR latency displayed. Record the latency.
- [ ] Repeat 3 more times with different sentences. Average the latency.
- [ ] **Pass criterion**: mean ASR inference latency on a 5 s utterance is **≤ 200 ms** on iPhone 12.
- [ ] **Pass criterion**: warm load is **≤ 300 ms**.
- [ ] **Pass criterion**: post-cache cold load is **≤ 5 s**.
- [ ] **Quality check**: take one of the test sentences, run the same audio through desktop Chirp (record on Mac, save WAV, run through `chirp` desktop app), compare transcripts. They should be visually identical or differ only in trivial whitespace/punctuation.
- [ ] Open Xcode → Debug Navigator → Memory while the app is running. Note the peak memory after model load and after one transcription. **Pass criterion**: peak ≤ 1 GB.

If any pass criterion fails:
- Latency 200–500 ms: still feasible, note it and move on; revisit if total pipeline becomes uncomfortable.
- Latency > 500 ms: investigate. Try the streaming API instead. If still slow, consider WhisperKit as a fallback.
- Memory > 1 GB on iPhone 12: investigate FluidAudio config. May need to drop iPhone 12 from supported devices.
- Quality differs significantly from desktop: investigate model variant — confirm FluidAudio is using `parakeet-tdt-0.6b-v3-coreml` and not an older version.
- **If two or more pass criteria fail by >2x: stop the project and bring numbers to the user.**

### Branch and PR

- Branch: `inc2-fluidaudio-asr`
- PR title: `inc2: FluidAudio Parakeet ASR with cold/warm load timing`
- PR description must include the four numbers (cold, warm, mean inference, peak memory) and the desktop comparison transcript.

---

## Increments 3–10 (sketched, to be detailed before each one starts)

Each of the remaining increments gets its own detailed section appended to this spec when its turn comes. For now, just goal + gate so the roadmap is concrete.

### Increment 3: Background session
- **Goal**: containing app stays alive in the background with `AVAudioSession` active. `AsrManager` stays loaded across foreground/background transitions.
- **Gate**: launch app, foreground for 30 s, switch to Notes for 5 minutes, switch back, tap record — recording works without re-loading the model. `Loggers.audio` shows the session was never deactivated.

### Increment 4: Swift port of cleanup.rs
- **Goal**: a `Cleanup/CleanupPipeline.swift` that mirrors the Rust pipeline (filler removal, spoken-punctuation expansion, number-word conversion, vocabulary find/replace) and a shared JSON fixture corpus committed to *both* `chirp` (Rust tests) and `chirp-ios` (Swift tests).
- **Gate**: the Swift implementation passes every fixture in the corpus. The Rust implementation also passes every fixture in the corpus. CI on both repos enforces this.
- **Note**: this is the highest-discipline increment because the corpus must be authoritative going forward. Spend time on the corpus design.

### Increment 5: mlx-swift LLM cleanup
- **Goal**: optional second-pass cleanup using a stand-in 0.6B-class model from mlx-community (e.g. `mlx-community/Qwen2.5-0.5B-Instruct-4bit`). Toggleable in `RootView`.
- **Gate**: end-to-end pipeline (record → ASR → regex → LLM → display) completes in **≤ 4 s post-stop** on iPhone 12 for an 80-token output. Memory peak ≤ 1.5 GB. Output is sensible English (this is a smoke test, not a quality test — true quality test happens after `chirp-cleanup-v2` is requantized to MLX).

### Increment 6: App Group persistence
- **Goal**: `Storage/SettingsStore.swift`, `Storage/VocabularyStore.swift`, `Storage/HistoryStore.swift` that read and write JSON files in the App Group container at `group.com.chirp.shared`. Schemas mirror desktop Chirp's `settings.json`, `vocabulary.json`, `history.json`.
- **Gate**: settings persist across launches and reboots. Files visible in the App Group container via Xcode → Devices → Container.

### Increment 7: Hotword/contextual biasing
- **Goal**: vocabulary entries from `VocabularyStore` are passed to FluidAudio's recognizer config for contextual biasing on every transcription.
- **Gate**: add a custom proper noun (e.g. "Cyrillus") to vocabulary, speak it, transcript contains the exact spelling.

### Increment 8: Keyboard extension target
- **Goal**: `ChirpKeyboard` extension becomes functional. It posts a Darwin notification on mic tap; the containing app (already alive in background from increment 3) receives it, captures audio, transcribes, writes result to App Group, posts a "done" notification; the keyboard reads the result and calls `UITextDocumentProxy.insertText(_:)`.
- **Gate**: open Notes.app, switch to Chirp keyboard, tap mic, speak, text appears at cursor without leaving Notes. End-to-end post-stop latency (steady state, session active) ≤ 1 s in regex-only mode.

### Increment 9: Cold-session bounce
- **Goal**: handle the case where the containing app has been jetsamed or the configured "Flow Session" timeout has expired. Keyboard falls back to opening `chirp://wake`, which brings the containing app to the foreground, activates the audio session, then auto-bounces back to the host app.
- **Gate**: reboot the iPhone, immediately open Notes, tap mic on Chirp keyboard, see brief Chirp app flash, return to Notes, dictate, see text appear. Subsequent dictations within the session hit the steady-state path (no app switch).

### Increment 10: Onboarding flow
- **Goal**: a one-time onboarding screen on first launch that walks the user through: install Chirp keyboard in Settings, enable Full Access, grant mic permission, download models. After onboarding, the app is fully functional.
- **Gate**: factory-reset (or fresh install) the app, run through onboarding, end at a successful first dictation in Notes.

---

## How to use this spec from a Claude Code session on the Mac Mini

Open a Claude Code session in `~/chirp-ios` (or `~/` if the repo doesn't exist yet). Use a prompt like:

> Read the spec at `~/chirp/docs/superpowers/specs/2026-04-07-ios-phase0-spike-spec.md`. Execute Increment 0 (Project Foundation). Stop at the verification gate and report which checks passed and failed. Do not start Increment 1 until I confirm the gate.

For each subsequent increment, repeat with the next increment number. The session executing the work should:

1. Read the locked architectural decisions and the relevant increment section.
2. Verify prerequisites (iPhone connected, signed in, etc.) before writing code.
3. Implement the increment.
4. Run the verification gate and report results.
5. Stop and wait for user approval before moving to the next increment.

Claude Code sessions executing this spec should treat the locked architectural decisions table as immutable. If a real obstacle requires changing one of those decisions, stop and bring it back to the user.

## Open Questions / Deferred Decisions

- **Bundle ID confirmation.** The spec uses `com.chirp.ios` as a placeholder. The user will look up their existing App Store team's bundle ID prefix and confirm or correct in increment 0.
- **iPhone 12 floor.** The user noted this "might be a little low." We hold the floor through increments 0–2 and re-evaluate after the increment 2 perf gate. If iPhone 12 is comfortable, keep it. If it's marginal, consider iPhone 13.
- **Re-quantizing `chirp-cleanup-v2` to MLX format.** Tracked as a separate task, not blocking. Increments 5 and earlier use a stand-in 0.6B model. The real cleanup model gets swapped in once requantization is done. Tokenizer parity and prompt parity must be tested against the desktop GGUF version at that point.
- **Streaming ASR mode.** FluidAudio supports streaming with 320 ms chunks at lower accuracy. We default to batch in increments 2–10. If batch latency is uncomfortable in the keyboard flow, increment 8 may revisit this.
- **Vocabulary biasing format.** FluidAudio's hotword API needs to be checked against the existing desktop `hotwords.txt` format. Increment 7 will reconcile.
- **Design system, dark mode, app icon, colors.** All deferred until after increment 8. Spec deliberately avoids any visual design discussion until the architecture is proven.
- **TestFlight, beta, App Store submission.** Tracked as separate tasks after increment 10. Not part of this spec.

## Files in the existing chirp repo to reference

When detailing increments 4 (cleanup port), 5 (LLM prompt), 6 (storage schemas), or 7 (vocabulary), the executing Claude session should read the corresponding desktop sources at `~/chirp/`:

- `src-tauri/src/cleanup.rs` — the Rust pipeline that increment 4 ports to Swift
- `src-tauri/src/llm.rs` — the few-shot prompt template, max_tokens, temperature, stop sequences for increment 5
- `src-tauri/src/settings.rs` — JSON schema for `settings.json`
- `src-tauri/src/history.rs` — JSON schema for `history.json` and the retention/pruning behavior
- `src-tauri/src/transcribe.rs` — confirms the exact Parakeet variant desktop uses (`sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`) so we match it
- `src-tauri/src/audio.rs` — VAD endpointing rules (relevant only when we move past the fixed 5-second capture in a later increment)

These are *read-only* references. The iOS port re-implements the logic in Swift, it does not vendor or link the Rust code.

## Definition of "real iOS app"

We are done with this spec when:

1. All ten increments have passed their verification gates on a real iPhone 12.
2. The Swift port of `cleanup.rs` is in lockstep with the Rust version via the shared fixture corpus.
3. The full keyboard-extension dictation flow works end-to-end in third-party host apps (Notes, Messages, Safari, ChatGPT iOS).
4. Onboarding handles a fresh install gracefully.
5. A TestFlight build is shippable.

After that, the next phases (re-quantize cleanup model, design system, App Store submission, marketing site, pricing/paywall integration) are tracked separately.
