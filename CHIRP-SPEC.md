# CHIRP — Project Spec v1

> **This is the single source of truth for the entire Chirp project.**
> Claude Code: read this file completely before writing any code.
> Do not carry over any code, patterns, or decisions from previous iterations.

---

## 1. What Is Chirp?

Chirp is a free, local voice-to-text desktop app. The user presses a global hotkey, speaks, and their words appear as clean, formatted text at their cursor position in any application. All processing happens on-device. No accounts, no cloud, no subscriptions.

**Domain:** trychirp.app
**Tagline:** "Speak freely."
**Positioning:** The only truly 100% local voice-to-text app. Competitors like Wispr Flow and Voquill send text to cloud APIs for cleanup. Chirp doesn't.

---

## 2. Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| App framework | **Tauri 2** | Desktop shell, system tray, global hotkeys, window management |
| Frontend | **React + TypeScript + Vite** | All UI: overlay, settings, onboarding |
| Styling | **Tailwind CSS** | All styling, using custom theme tokens |
| State management | **Zustand** | Single global store |
| ASR engine | **whisper-rs** (wraps whisper.cpp) | Speech-to-text, runs locally |
| Text cleanup | **Custom T5 model via ort** (ONNX Runtime) | Transcript formatting and cleanup |
| Audio capture | **cpal** | Cross-platform microphone input |
| Clipboard | **arboard** | Read/write system clipboard |
| Key simulation | **enigo** | Simulate Ctrl+V / Cmd+V to paste |
| Global hotkey | **Tauri global-shortcut plugin** | System-wide hotkey listener |
| Icons | **Lucide React** | All UI icons |
| Fonts | **Nunito, Inter, JetBrains Mono** | Display, body, monospace |

### What we are NOT using
- No Python anywhere
- No Electron
- No cloud APIs
- No Firebase
- No user accounts or auth
- No analytics or telemetry
- No ONNX custom ops (T5 is vanilla transformer)

---

## 3. Architecture

```
┌─────────────────────────────────────────────┐
│                 React Frontend               │
│  (overlay, settings, onboarding)             │
│  Zustand store manages all UI state          │
├─────────────────────────────────────────────┤
│              Tauri IPC Bridge                │
│  Frontend calls Rust via invoke()            │
├─────────────────────────────────────────────┤
│               Rust Backend                   │
│                                             │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ Audio       │  │ ASR Pipeline         │  │
│  │ cpal capture│→ │ whisper-rs           │  │
│  │ 16kHz mono  │  │ → raw transcript     │  │
│  └─────────────┘  └──────┬───────────────┘  │
│                          │                   │
│                   ┌──────▼───────────────┐  │
│                   │ Text Cleanup         │  │
│                   │ T5 model via ort     │  │
│                   │ → formatted text     │  │
│                   └──────┬───────────────┘  │
│                          │                   │
│                   ┌──────▼───────────────┐  │
│                   │ Dictionary Pass      │  │
│                   │ String replacements  │  │
│                   │ from user config     │  │
│                   └──────┬───────────────┘  │
│                          │                   │
│                   ┌──────▼───────────────┐  │
│                   │ Text Injection       │  │
│                   │ arboard + enigo      │  │
│                   │ clipboard → paste    │  │
│                   └──────────────────────┘  │
└─────────────────────────────────────────────┘
```

### Data flow for a single dictation:

1. User presses global hotkey (default: Ctrl+Shift+C)
2. Tauri opens the floating overlay window
3. Rust begins capturing audio via cpal (16kHz, mono, f32)
4. Frontend shows waveform visualization from amplitude data
5. User presses hotkey again OR 3 seconds of silence detected
6. Audio buffer sent to whisper-rs for transcription
7. Raw transcript sent to T5 cleanup model via ort
8. Cleaned text run through dictionary replacements
9. Result written to clipboard via arboard
10. Ctrl+V (or Cmd+V on mac) simulated via enigo
11. Original clipboard contents restored
12. Overlay shows "Inserted X words" then dismisses

---

## 4. Rust Crate Dependencies

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "global-shortcut"] }
whisper-rs = "0.12"          # whisper.cpp bindings
ort = "2"                     # ONNX Runtime for T5 cleanup model
cpal = "0.15"                 # Audio capture
arboard = "3"                 # Clipboard
enigo = "0.2"                 # Key simulation
serde = { version = "1", features = ["derive"] }
serde_json = "1"
hound = "3.5"                 # WAV encoding for whisper input
rubato = "0.15"               # Audio resampling if needed
```

---

## 5. File Structure

```
chirp/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Tauri app entry, tray, windows
│   │   ├── audio.rs             # cpal mic capture, amplitude extraction
│   │   ├── transcribe.rs        # whisper-rs inference
│   │   ├── cleanup.rs           # T5 ONNX model inference via ort
│   │   ├── dictionary.rs        # User dictionary string replacements
│   │   ├── inject.rs            # Clipboard write + key simulation
│   │   ├── state.rs             # App state shared across commands
│   │   └── commands.rs          # All Tauri invoke handlers
│   ├── models/
│   │   ├── whisper-small.bin    # Whisper model (downloaded on first launch)
│   │   └── chirp-cleanup.onnx  # T5 cleanup model (bundled or downloaded)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── App.tsx                  # Router: overlay vs settings vs onboarding
│   ├── main.tsx                 # React entry
│   ├── stores/
│   │   └── appStore.ts          # Zustand store (single source of truth)
│   ├── components/
│   │   ├── overlay/
│   │   │   ├── Overlay.tsx      # Floating overlay container
│   │   │   ├── Listening.tsx    # Waveform + "Listening..." state
│   │   │   ├── Processing.tsx   # Shimmer loader state
│   │   │   ├── Done.tsx         # "Inserted X words" state
│   │   │   └── Error.tsx        # Error state with action
│   │   ├── settings/
│   │   │   ├── Settings.tsx     # Settings window layout with sidebar
│   │   │   ├── GeneralPage.tsx  # Hotkey, behavior, output settings
│   │   │   ├── AudioPage.tsx    # Input device, levels, noise suppression
│   │   │   ├── ModelPage.tsx    # Model selection, download, disk usage
│   │   │   ├── DictionaryPage.tsx # User dictionary editor
│   │   │   └── AboutPage.tsx    # Version, credits, update check
│   │   ├── onboarding/
│   │   │   ├── Onboarding.tsx   # 3-step first launch flow
│   │   │   ├── Welcome.tsx      # Step 1
│   │   │   ├── Microphone.tsx   # Step 2: mic permission
│   │   │   └── Hotkey.tsx       # Step 3: hotkey setup
│   │   └── shared/
│   │       ├── Button.tsx       # Primary/secondary button
│   │       ├── Toggle.tsx       # Checkbox/switch
│   │       ├── Select.tsx       # Dropdown
│   │       ├── KeyBadge.tsx     # Keyboard shortcut display
│   │       └── Waveform.tsx     # Audio waveform visualization
│   ├── hooks/
│   │   ├── useTauri.ts          # Tauri invoke wrappers
│   │   └── useAudio.ts          # Audio amplitude subscription
│   ├── lib/
│   │   └── constants.ts         # Default settings, keybinds
│   └── styles/
│       └── globals.css          # Tailwind config + custom properties
├── assets/
│   └── brand/
│       ├── chirp-mark-primary.svg
│       ├── chirp-mark-white.svg
│       └── chirp-appicon.svg
├── STYLE.md                     # Design system (see separate file)
├── package.json
├── tailwind.config.ts
├── tsconfig.json
├── vite.config.ts
└── index.html
```

---

## 6. Tauri Window Configuration

### Overlay Window
```json
{
  "label": "overlay",
  "title": "Chirp",
  "width": 420,
  "height": 160,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "center": true,
  "visible": false,
  "skipTaskbar": true,
  "shadow": false
}
```

### Settings Window
```json
{
  "label": "settings",
  "title": "Chirp Settings",
  "width": 640,
  "height": 520,
  "resizable": true,
  "minWidth": 560,
  "minHeight": 440,
  "decorations": true,
  "visible": false,
  "center": true
}
```

---

## 7. Zustand Store Shape

```typescript
interface AppState {
  // Recording state
  status: 'idle' | 'listening' | 'processing' | 'done' | 'error';
  errorMessage: string | null;
  wordCount: number | null;
  amplitudes: number[];  // current waveform data, 32-48 values

  // Settings
  hotkey: string;
  launchAtLogin: boolean;
  showInMenuBar: boolean;
  playSoundOnComplete: boolean;
  autoDismissOverlay: boolean;
  language: string;       // 'auto' | 'en' | 'es' | etc.
  removeFiller: boolean;
  autoPunctuate: boolean;

  // Audio
  inputDevice: string | null;
  inputLevel: number;
  noiseSuppression: boolean;

  // Model
  whisperModel: 'tiny' | 'base' | 'small' | 'medium';
  modelDownloaded: boolean;
  modelDownloadProgress: number;

  // Dictionary
  dictionary: Record<string, string>;

  // Onboarding
  onboardingComplete: boolean;

  // Actions
  setStatus: (status: AppState['status']) => void;
  setAmplitudes: (data: number[]) => void;
  updateSettings: (partial: Partial<AppState>) => void;
  addDictionaryEntry: (key: string, value: string) => void;
  removeDictionaryEntry: (key: string) => void;
}
```

---

## 8. Tauri Commands (Rust → TypeScript bridge)

```rust
#[tauri::command]
async fn start_recording(state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn stop_recording(state: State<'_, AppState>) -> Result<TranscriptionResult, String>

#[tauri::command]
async fn cancel_recording(state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn get_audio_devices() -> Result<Vec<AudioDevice>, String>

#[tauri::command]
async fn get_input_level(state: State<'_, AppState>) -> Result<f32, String>

#[tauri::command]
async fn download_model(model: String) -> Result<(), String>

#[tauri::command]
async fn get_model_status() -> Result<ModelStatus, String>

#[tauri::command]
async fn update_settings(settings: Settings) -> Result<(), String>

#[tauri::command]
async fn get_settings() -> Result<Settings, String>

#[tauri::command]
async fn update_dictionary(dictionary: HashMap<String, String>) -> Result<(), String>

#[tauri::command]
async fn check_for_updates() -> Result<UpdateInfo, String>
```

### TranscriptionResult
```rust
struct TranscriptionResult {
    text: String,
    word_count: u32,
    duration_ms: u64,
}
```

---

## 9. Settings Persistence

Settings stored as JSON at the platform-appropriate config directory:
- **Windows:** `%APPDATA%/com.chirp.app/settings.json`
- **macOS:** `~/Library/Application Support/com.chirp.app/settings.json`
- **Linux:** `~/.config/com.chirp.app/settings.json`

Dictionary stored separately as `dictionary.json` in the same directory.

Use Tauri's `app_config_dir()` to resolve paths. Read on launch, write on every change (debounced 500ms).

---

## 10. Model Management

### Whisper Model
- NOT bundled with the app (too large)
- Downloaded on first launch during onboarding or in Settings → Model
- Default: `whisper-small` (~500MB)
- Options: tiny (75MB), base (150MB), small (500MB), medium (1.5GB)
- Stored in app data directory under `models/`
- Show download progress in UI

### Cleanup Model (T5)
- Bundled with the app installer (~40MB after quantization)
- Located at `models/chirp-cleanup.onnx` within app resources
- No user-facing model management needed for this
- Loaded into memory on app start

---

## 11. Platform-Specific Notes

### Windows
- Text injection: clipboard write → Ctrl+V simulation
- Global hotkey: Ctrl+Shift+C default
- System tray: standard Windows notification area
- Installer: NSIS via Tauri bundler

### macOS
- Text injection: clipboard write → Cmd+V simulation
- Global hotkey: Cmd+Shift+C default
- System tray: macOS menu bar
- Requires Accessibility permission for key simulation
- Requires Microphone permission
- Installer: .dmg via Tauri bundler
- Notarization required for distribution

### Linux
- Text injection: clipboard write → Ctrl+V simulation
- System tray: varies by DE, use Tauri's tray API
- May need additional packages: libwebkit2gtk, libappindicator

---

## 12. Build & Release

### Development
```bash
npm install
cd src-tauri && cargo build
npm run dev          # Vite dev server + Tauri dev
```

### Production Build
```bash
npm run build        # Builds frontend
cd src-tauri && cargo tauri build   # Produces installer
```

### Release Process
1. Tag version in git
2. GitHub Actions builds for Windows + macOS + Linux
3. Binaries uploaded as GitHub Release assets
4. Landing page download links point to latest GitHub Release

---

## 13. Landing Page (trychirp.app)

Separate repo or `/web` directory. Next.js static export + Tailwind, deployed on Vercel.

**Pages:** Single page, sections anchor-linked from nav.

**Sections:**
1. Nav (sticky, logo + links + download button)
2. Hero ("Talk. Type. Done." + download buttons + bird mark)
3. Features (3 cards: Private, Fast, Free)
4. How It Works (3 steps: Download, Hotkey, Talk)
5. Privacy (comparison table vs Wispr Flow and Voquill)
6. Download CTA (repeat hero CTA)
7. Footer (logo, credit, GitHub link)

**Key copy points:**
- "100% local. Your voice never touches a server."
- "Not even the text cleanup. Unlike competitors, Chirp's entire pipeline runs on your device."
- "Free forever. No trials, no pro tier, no 'upgrade' popups."
- "Powered by [model name] — a custom-trained text formatting model built specifically for voice transcripts."

See STYLE.md for all visual design decisions.
