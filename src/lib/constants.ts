export const DEFAULT_HOTKEY = 'CmdOrCtrl+Shift+Space'

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
  model: 'parakeet-tdt-0.6b' as const,
  onboardingComplete: false,
}

export const STT_MODELS = [
  { id: 'parakeet-tdt-0.6b' as const, name: 'Parakeet TDT 0.6B', size: '465 MB', description: 'Best accuracy, fast on any PC', recommended: true },
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
