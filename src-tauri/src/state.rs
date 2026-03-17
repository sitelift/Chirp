use serde::{Deserialize, Serialize};
use sherpa_onnx::OfflineRecognizer;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Thread-safe wrapper for sherpa-onnx OfflineRecognizer.
/// The C API is thread-safe so this is safe to Send+Sync.
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

/// User-facing app settings, persisted as JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub launch_at_login: bool,
    pub show_in_menu_bar: bool,
    pub play_sound_on_complete: bool,
    pub auto_dismiss_overlay: bool,
    pub silence_timeout: u32,
    pub language: String,
    pub smart_formatting: bool,
    pub input_device: String,
    pub noise_suppression: bool,
    #[serde(alias = "whisperModel")]
    pub model: String,
    pub onboarding_complete: bool,
    #[serde(default)]
    pub ai_cleanup: bool,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: String,
    #[serde(default = "default_true")]
    pub show_passive_overlay: bool,
}

fn default_overlay_position() -> String {
    "bottom".into()
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "CmdOrCtrl+Shift+Space".into(),
            launch_at_login: true,
            show_in_menu_bar: true,
            play_sound_on_complete: false,
            auto_dismiss_overlay: true,
            silence_timeout: 3,
            language: "auto".into(),
            smart_formatting: true,
            input_device: "default".into(),
            noise_suppression: true,
            model: "parakeet-tdt-0.6b".into(),
            onboarding_complete: false,
            ai_cleanup: false,
            overlay_position: "bottom".into(),
            show_passive_overlay: true,
        }
    }
}

/// Dictionary entry for word replacement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub from: String,
    pub to: String,
}

/// Snippet entry for text expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetEntry {
    pub trigger: String,
    pub expansion: String,
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
}

/// Persisted transcription history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionEntry {
    pub text: String,
    pub timestamp: String,
    pub word_count: usize,
    pub duration_ms: u64,
    #[serde(default)]
    pub speech_duration_ms: u64,
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

/// Error types matching the frontend's ErrorType
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChirpErrorType {
    MicNotFound,
    MicPermission,
    ModelNotLoaded,
    TranscriptionFailed,
    InjectionFailed,
    Unknown,
}

impl std::fmt::Display for ChirpErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MicNotFound => write!(f, "mic_not_found"),
            Self::MicPermission => write!(f, "mic_permission"),
            Self::ModelNotLoaded => write!(f, "model_not_loaded"),
            Self::TranscriptionFailed => write!(f, "transcription_failed"),
            Self::InjectionFailed => write!(f, "injection_failed"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Main application state shared across commands
pub struct AppState {
    pub settings: Settings,
    pub dictionary: Vec<DictionaryEntry>,
    pub snippets: Vec<SnippetEntry>,
    pub history: Vec<TranscriptionEntry>,
    pub recording_state: RecordingState,
    pub recognizer: Option<SherpaRecognizer>,
    pub llm_process: Option<tokio::process::Child>,
    pub llm_port: Option<u16>,
}

impl AppState {
    pub fn new(settings: Settings, dictionary: Vec<DictionaryEntry>, snippets: Vec<SnippetEntry>, history: Vec<TranscriptionEntry>) -> Self {
        Self {
            settings,
            dictionary,
            snippets,
            history,
            recording_state: RecordingState::Idle,
            recognizer: None,
            llm_process: None,
            llm_port: None,
        }
    }
}

/// Thread-safe wrapper for AppState
pub type SharedState = Arc<Mutex<AppState>>;

/// Separate audio buffer to avoid blocking cpal callback on main state lock
pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;
