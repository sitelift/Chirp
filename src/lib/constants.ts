export const DEFAULT_HOTKEY = 'CmdOrCtrl+Shift+C'

export const DEFAULT_SETTINGS = {
  hotkey: DEFAULT_HOTKEY,
  launchAtLogin: true,
  showInMenuBar: true,
  playSoundOnComplete: false,
  autoDismissOverlay: true,
  silenceTimeout: 3,
  language: 'auto',
  smartFormatting: true,
  inputDevice: 'default',
  noiseSuppression: true,
  whisperModel: 'base' as const,
  onboardingComplete: false,
}

export const WHISPER_MODELS = [
  { id: 'tiny' as const, name: 'Tiny (English)', size: '78 MB', description: 'Fastest — sub-second' },
  { id: 'base' as const, name: 'Base (English)', size: '148 MB', description: 'Fast & accurate', recommended: true },
  { id: 'small' as const, name: 'Small (English)', size: '488 MB', description: 'Most accurate, slower' },
  { id: 'medium' as const, name: 'Medium (English)', size: '1.5 GB', description: 'Best accuracy, slowest' },
]

export const SILENCE_TIMEOUT_OPTIONS = [
  { value: 2, label: '2 seconds' },
  { value: 3, label: '3 seconds' },
  { value: 5, label: '5 seconds' },
  { value: 10, label: '10 seconds' },
  { value: 0, label: 'Never (manual stop only)' },
]

export const LANGUAGES = [
  { value: 'auto', label: 'Auto-detect' },
  { value: 'en', label: 'English' },
  { value: 'es', label: 'Spanish' },
  { value: 'fr', label: 'French' },
  { value: 'de', label: 'German' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'it', label: 'Italian' },
  { value: 'ja', label: 'Japanese' },
  { value: 'zh', label: 'Chinese' },
  { value: 'ko', label: 'Korean' },
]

export const ERROR_MESSAGES = {
  mic_not_found: {
    title: 'No microphone detected',
    help: 'Connect a microphone and try again',
    action: null,
  },
  mic_permission: {
    title: "Couldn't access microphone",
    help: 'Check your system permissions',
    action: { label: 'Open Settings', type: 'os_settings' as const },
  },
  model_not_loaded: {
    title: 'Speech model not ready',
    help: 'Download a model in settings',
    action: { label: 'Open Settings', type: 'app_settings' as const },
  },
  transcription_failed: {
    title: "Couldn't process audio",
    help: 'Try speaking more clearly',
    action: { label: 'Try Again', type: 'retry' as const },
  },
  injection_failed: {
    title: "Couldn't paste text",
    help: 'Make sure a text field is focused',
    action: { label: 'Copy to Clipboard', type: 'copy' as const },
  },
  unknown: {
    title: 'Something went wrong',
    help: 'Please try again',
    action: { label: 'Try Again', type: 'retry' as const },
  },
} as const

export type ErrorType = keyof typeof ERROR_MESSAGES
