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
    AccessibilityRequired,
}

/// How the global hotkey controls recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    Hold,
    Tap,
}

impl Default for HotkeyMode {
    fn default() -> Self {
        Self::Hold
    }
}

/// User-facing app settings, persisted as JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    #[serde(default)]
    pub hotkey_mode: HotkeyMode,
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
    pub overlay_position: serde_json::Value,
    #[serde(default = "default_true")]
    pub show_passive_overlay: bool,
    #[serde(default = "default_tone_mode")]
    pub tone_mode: String,
    #[serde(default)]
    pub history_retention_days: i64,
    #[serde(default)]
    pub help_improve: bool,
    #[serde(default)]
    pub beam_search: bool,
    #[serde(default = "default_cleanup_model")]
    pub cleanup_model: String,
    #[serde(default)]
    pub dark_mode: bool,
}

fn default_cleanup_model() -> String {
    "chirp-v2".into()
}

fn default_overlay_position() -> serde_json::Value {
    serde_json::Value::String("bottom".into())
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
            hotkey: if cfg!(target_os = "macos") { "MetaLeft+ShiftLeft+Space" } else { "ControlLeft+ShiftLeft+Space" }.into(),
            hotkey_mode: HotkeyMode::Hold,
            launch_at_login: true,
            play_sound_on_complete: false,
            auto_dismiss_overlay: true,
            smart_formatting: true,
            input_device: "default".into(),
            model: "parakeet-tdt-0.6b".into(),
            onboarding_complete: false,
            ai_cleanup: true,
            overlay_position: serde_json::Value::String("bottom".into()),
            show_passive_overlay: true,
            tone_mode: "message".into(),
            history_retention_days: 0,
            help_improve: false,
            beam_search: false,
            cleanup_model: "chirp-v2".into(),
            dark_mode: false,
        }
    }
}

/// Snippet entry for text expansion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnippetEntry {
    pub trigger: String,
    pub expansion: String,
}

/// A vocabulary entry: a canonical term plus optional list of mishearings
/// to find/replace toward this term.
///
/// `term` is what the ASR is biased toward (sherpa-onnx hotwords) and what
/// every `replaces` entry is corrected TO during the post-ASR find/replace
/// pass. The `replaces` list is purely for deterministic text substitution
/// — it never touches the ASR or LLM.
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

/// Main application state shared across commands
pub struct AppState {
    pub settings: Settings,
    pub vocabulary: Vec<VocabEntry>,
    pub snippets: Vec<SnippetEntry>,
    pub history: Vec<TranscriptionEntry>,
    pub recording_state: RecordingState,
    pub recording_generation: u64,
    /// True if the VAD receiver thread was successfully spawned for the
    /// current/most-recent recording. Set in start_recording, checked in
    /// stop_recording to decide whether to use VAD output or the chunked
    /// fallback. Never decide based on vad_texts.is_empty() — a transient
    /// receiver hiccup could leave it empty and cause the fallback to
    /// re-transcribe the whole buffer, producing duplicated output.
    pub vad_was_active: bool,
    pub hotkey_status: HotkeyStatus,
    /// Recognizer is in its own Arc so transcription can proceed without holding
    /// the main state lock. The sherpa C API is thread-safe (Send+Sync).
    pub recognizer: Option<Arc<SherpaRecognizer>>,
    /// Set when the user updates vocabulary. The recognizer is rebuilt lazily
    /// at the start of the next idle recording, NOT inline in update_vocabulary.
    /// Inline rebuilds caused heap corruption when the frontend fired multiple
    /// rapid update_vocabulary calls — sherpa-onnx can't handle rapid
    /// create/destroy of hotword-enabled recognizers.
    pub recognizer_dirty: bool,
    pub llm_process: Option<tokio::process::Child>,
    pub llm_port: Option<u16>,
    /// Shared HTTP client for LLM/T5 requests (connection pooling)
    pub http_client: reqwest::Client,
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
            vad_was_active: false,
            hotkey_status: HotkeyStatus::Idle,
            recognizer: None,
            recognizer_dirty: false,
            llm_process: None,
            llm_port: None,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .pool_max_idle_per_host(1)
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

/// Thread-safe wrapper for AppState
pub type SharedState = Arc<Mutex<AppState>>;

/// Separate audio buffer to avoid blocking cpal callback on main state lock
pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;

/// Accumulated transcripts from VAD segments, filled by receiver thread
pub type VadTranscripts = Arc<std::sync::Mutex<Vec<String>>>;

/// Accumulated per-segment CLEANED transcripts from VAD segments, filled by
/// the receiver thread after it runs the full cleanup pipeline
/// (regex + vocab + snippets + optional LLM) on each segment as it arrives.
/// Joined in stop_recording and injected directly.
///
/// Newtype wrapper (not a type alias) so Tauri's state manager can
/// distinguish it from `VadTranscripts` — both wrap the same inner type
/// and Tauri keys state by the outer Rust type.
#[derive(Default)]
pub struct VadCleanedTranscripts(pub Arc<std::sync::Mutex<Vec<String>>>);

/// Handle to the VAD receiver thread (joined on stop_recording)
pub struct VadReceiverHandle(pub std::sync::Mutex<Option<std::thread::JoinHandle<()>>>);

/// Sender for the VAD segment channel (needed to send poison pill on stop)
pub struct VadSender(pub std::sync::Mutex<Option<crossbeam_channel::Sender<Vec<f32>>>>);

/// Handle to flush VAD on stop (kept separate from audio callback's copy)
pub struct VadFlushHandle(pub std::sync::Mutex<Option<Arc<std::sync::Mutex<crate::audio::VadState>>>>);
