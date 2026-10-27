use serde::{Deserialize, Serialize};
use sherpa_onnx::OfflineRecognizer;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Thread-safe wrapper for sherpa-onnx OfflineRecognizer.
/// SAFETY: sherpa-onnx's C API is internally thread-safe (all state is behind
/// mutexes in the C++ implementation). We additionally wrap in Arc and only
/// call from spawn_blocking tasks. See: k2-fsa/sherpa-onnx c-api.h
pub struct SherpaRecognizer(pub OfflineRecognizer);
unsafe impl Send for SherpaRecognizer {}
unsafe impl Sync for SherpaRecognizer {}

/// Recording lifecycle state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

/// Hotkey listener lifecycle state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyStatus {
    Idle,
    Retrying,
    Active,
    Failed,
}

/// User-facing app settings, persisted as JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "crate::hotkey::default_hold")]
    pub hold_binding: Option<crate::hotkey::Binding>,
    #[serde(default = "crate::hotkey::default_toggle")]
    pub toggle_binding: Option<crate::hotkey::Binding>,
    pub launch_at_login: bool,
    pub play_sound_on_complete: bool,
    pub auto_dismiss_overlay: bool,
    /// Unused since the deterministic cleanup was removed; kept so settings
    /// files keep round-tripping.
    pub smart_formatting: bool,
    #[serde(default, alias = "capture_dictations")]
    pub capture_dictations: bool,
    #[serde(default, alias = "speculative_cleanup")]
    pub speculative_cleanup: bool,
    #[serde(default = "default_capture_retention_days", alias = "capture_retention_days")]
    pub capture_retention_days: u64,
    #[serde(default = "default_capture_max_bytes", alias = "capture_max_bytes")]
    pub capture_max_bytes: u64,
    pub input_device: String,
    #[serde(alias = "whisperModel")]
    pub model: String,
    pub onboarding_complete: bool,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: serde_json::Value,
    #[serde(default = "default_true")]
    pub show_passive_overlay: bool,
    #[serde(default)]
    pub history_retention_days: i64,
    #[serde(default)]
    pub beam_search: bool,
    #[serde(default)]
    pub dark_mode: bool,
    /// Legacy "Use Claude" switch. Superseded by `provider`; read only to migrate
    /// settings written before it existed (see `Settings::provider()`).
    #[serde(default = "default_true")]
    pub ai_cleanup: bool,
    /// Which cloud model cleans up dictations. `None` in files written before
    /// providers existed; resolve with `Settings::provider()`.
    #[serde(default)]
    pub provider: Option<Provider>,
    /// Surface theme: follow Windows, or always light / dark.
    #[serde(default)]
    pub appearance: Appearance,
    /// Cancels the dictation in progress and discards it. Esc by default.
    #[serde(default = "crate::hotkey::default_cancel")]
    pub cancel_binding: Option<crate::hotkey::Binding>,
    /// What happens to other apps' audio while recording (media.rs).
    #[serde(default)]
    pub media_while_dictating: MediaMode,
    /// With `Lower`: how much quieter, in percent of each app's volume.
    #[serde(default = "default_media_lower_by")]
    pub media_lower_by: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaMode {
    Off,
    #[default]
    Lower,
    Pause,
}

pub fn default_media_lower_by() -> u8 { 80 }

/// The cloud model used for cleanup. `Off` pastes the recognizer's text with
/// only spoken commands ("new line", "comma") converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Off,
    Anthropic,
    Openai,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Settings {
    /// The provider in effect, migrating the legacy `ai_cleanup` switch.
    pub fn provider(&self) -> Provider {
        self.provider.unwrap_or(if self.ai_cleanup { Provider::Anthropic } else { Provider::Off })
    }
}

fn default_overlay_position() -> serde_json::Value {
    serde_json::Value::String("bottom".into())
}

fn default_true() -> bool {
    true
}

pub fn default_capture_retention_days() -> u64 { 14 }
pub fn default_capture_max_bytes() -> u64 { 2 * 1024 * 1024 * 1024 }

impl Default for Settings {
    fn default() -> Self {
        Self {
            hold_binding: crate::hotkey::default_hold(),
            toggle_binding: crate::hotkey::default_toggle(),
            launch_at_login: true,
            play_sound_on_complete: false,
            auto_dismiss_overlay: true,
            smart_formatting: true,
            capture_dictations: false,
            speculative_cleanup: false,
            capture_retention_days: default_capture_retention_days(),
            capture_max_bytes: default_capture_max_bytes(),
            input_device: "default".into(),
            model: "parakeet-tdt-0.6b".into(),
            onboarding_complete: false,
            overlay_position: serde_json::Value::String("bottom".into()),
            show_passive_overlay: true,
            history_retention_days: 0,
            beam_search: false,
            dark_mode: false,
            ai_cleanup: true,
            provider: Some(Provider::Openai),
            appearance: Appearance::System,
            cancel_binding: crate::hotkey::default_cancel(),
            media_while_dictating: MediaMode::Lower,
            media_lower_by: default_media_lower_by(),
        }
    }
}

/// Snippet entry for text expansion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnippetEntry {
    pub trigger: String,
    pub expansion: String,
}

/// A Words entry: a spelling the cleanup model receives in its dictionary.
///
/// `replaces` (mishearings for the old find/replace pass) stays on disk for
/// compatibility and is ignored everywhere.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabEntry {
    pub term: String,
    #[serde(default)]
    pub replaces: Vec<String>,
}

/// Wire format that accepts BOTH the legacy Vec<String> shape and the new
/// Vec<VocabEntry> shape, so existing vocabulary.json files keep loading
/// without any migration. Strings get widened to VocabEntry { term, replaces: [] }.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VocabEntryWire {
    /// Legacy: just a canonical term, no replacements.
    Bare(String),
    /// New: canonical term plus optional list of mishearings.
    Full(VocabEntry),
}

impl From<VocabEntryWire> for VocabEntry {
    fn from(w: VocabEntryWire) -> Self {
        match w {
            VocabEntryWire::Bare(term) => VocabEntry { term, replaces: Vec::new() },
            VocabEntryWire::Full(e) => e,
        }
    }
}

/// Audio device info sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub id: String,
}

/// Transcription result sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub word_count: usize,
    pub duration_ms: u64,
    #[serde(default)]
    pub was_cleaned_up: bool,
    /// Set when AI cleanup was wanted but the deterministic text was pasted
    /// instead ("offline", "slow", "key invalid", ...).
    #[serde(default)]
    pub fallback_reason: Option<String>,
}

/// Persisted transcription history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionEntry {
    #[serde(default)]
    pub timings: crate::performance::Breakdown,
    pub text: String,
    pub timestamp: String,
    pub word_count: usize,
    pub duration_ms: u64,
    #[serde(default)]
    pub speech_duration_ms: u64,
    #[serde(default)]
    pub was_cleaned_up: bool,
}

/// Model download/presence status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub model: String,
    pub downloaded: bool,
    pub size_bytes: u64,
}

/// Amplitude data event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplitudeData {
    pub bars: Vec<f32>,
}

/// Main application state shared across commands
pub struct AppState {
    pub settings: Settings,
    pub vocabulary: Vec<VocabEntry>,
    pub snippets: Vec<SnippetEntry>,
    pub history: Vec<TranscriptionEntry>,
    pub recording_state: RecordingState,
    pub recording_generation: u64,
    pub hotkey_status: HotkeyStatus,
    /// Recognizer is in its own Arc so transcription can proceed without holding
    /// the main state lock. The sherpa C API is thread-safe (Send+Sync).
    pub recognizer: Option<Arc<SherpaRecognizer>>,
}

impl AppState {
    pub fn new(settings: Settings, vocabulary: Vec<VocabEntry>, snippets: Vec<SnippetEntry>, history: Vec<TranscriptionEntry>) -> Self {
        Self {
            settings,
            vocabulary,
            snippets,
            history,
            recording_state: RecordingState::Idle,
            recording_generation: 0,
            hotkey_status: HotkeyStatus::Idle,
            recognizer: None,
        }
    }
}

/// Thread-safe wrapper for AppState
pub type SharedState = Arc<Mutex<AppState>>;

/// Separate audio buffer to avoid blocking cpal callback on main state lock
pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;

/// Accumulated transcripts from VAD segments, filled by receiver thread
pub type VadTranscripts = Arc<std::sync::Mutex<Vec<String>>>;

/// Handle to the VAD receiver thread (joined on stop_recording)
pub struct VadReceiverHandle(pub std::sync::Mutex<Option<std::thread::JoinHandle<()>>>);

/// Sender for the VAD segment channel (needed to send poison pill on stop)
pub struct VadSender(pub std::sync::Mutex<Option<crossbeam_channel::Sender<crate::audio::Segment>>>);

/// Handle to flush VAD on stop (kept separate from audio callback's copy)
pub struct VadFlushHandle(pub std::sync::Mutex<Option<Arc<std::sync::Mutex<crate::audio::VadState>>>>);
