export const DEFAULT_SETTINGS = {
  hotkey: 'CmdOrCtrl+Shift+Space',
  launchAtLogin: true,
  playSoundOnComplete: false,
  autoDismissOverlay: true,
  smartFormatting: true,
  inputDevice: 'default',
  model: 'parakeet-tdt-0.6b' as const,
  onboardingComplete: false,
  aiCleanup: true,
  overlayPosition: 'bottom' as 'bottom' | 'top',
  showPassiveOverlay: true,
  toneMode: 'message',
  historyRetentionDays: 0,
}

export const TONE_MODES = [
  { id: 'message', label: 'Message', description: 'Natural conversational tone' },
  { id: 'email', label: 'Email', description: 'Formatted with greeting and sign-off' },
  { id: 'formal', label: 'Formal', description: 'Professional, no contractions' },
  { id: 'casual', label: 'Casual', description: 'Short and conversational' },
] as const

export const STT_MODELS = [
  { id: 'parakeet-tdt-0.6b' as const, name: 'Parakeet TDT 0.6B', size: '465 MB', description: 'Best accuracy · 25 languages · fast on any PC', recommended: true },
]

export const LLM_MODEL = {
  name: 'Qwen 2.5 1.5B',
  displayName: 'Smart Cleanup',
  size: '1.1 GB',
  friendlySize: 'About 1 GB',
  description: 'Fast, local AI cleanup on any PC.',
}

export const CLEANUP_EXAMPLE = {
  before: "so um basically I was thinking that we should like probably move the meeting to uh Thursday if that works",
  after: "I was thinking we should probably move the meeting to Thursday, if that works.",
}

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
