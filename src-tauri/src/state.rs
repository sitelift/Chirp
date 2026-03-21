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
    pub hotkey: String,
    pub launch_at_login: bool,
    pub play_sound_on_complete: bool,
    pub auto_dismiss_overlay: bool,
    pub smart_formatting: bool,
    pub input_device: String,
    #[serde(alias = "whisperModel")]
    pub model: String,
    pub onboarding_complete: bool,
    #[serde(default)]
    pub ai_cleanup: bool,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: String,
    #[serde(default = "default_true")]
    pub show_passive_overlay: bool,
    #[serde(default = "default_tone_mode")]
    pub tone_mode: String,
    #[serde(default)]
    pub history_retention_days: i64,
}

fn default_overlay_position() -> String {
    "bottom".into()
}

fn default_true() -> bool {
    true
}

fn default_tone_mode() -> String {
    "message".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "CmdOrCtrl+Shift+Space".into(),
            launch_at_login: true,
            play_sound_on_complete: false,
            auto_dismiss_overlay: true,
            smart_formatting: true,
            input_device: "default".into(),
            model: "parakeet-tdt-0.6b".into(),
            onboarding_complete: false,
            ai_cleanup: true,
            overlay_position: "bottom".into(),
            show_passive_overlay: true,
            tone_mode: "message".into(),
            history_retention_days: 0,
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
    #[serde(default)]
    pub was_cleaned_up: bool,
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

/// File transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTranscriptionResult {
    pub text: String,
    pub duration_secs: f32,
    pub word_count: usize,
    pub chunks: usize,
}

/// Main application state shared across commands
pub struct AppState {
    pub settings: Settings,
    pub dictionary: Vec<DictionaryEntry>,
    pub snippets: Vec<SnippetEntry>,
    pub history: Vec<TranscriptionEntry>,
    pub recording_state: RecordingState,
    pub recording_generation: u64,
    pub hotkey_status: HotkeyStatus,
    /// Recognizer is in its own Arc so transcription can proceed without holding
    /// the main state lock. The sherpa C API is thread-safe (Send+Sync).
    pub recognizer: Option<Arc<SherpaRecognizer>>,
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
            recording_generation: 0,
            hotkey_status: HotkeyStatus::Idle,
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
