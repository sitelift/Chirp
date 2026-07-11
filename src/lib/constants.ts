export const DEFAULT_SETTINGS = {
  hotkey: 'MetaLeft+ShiftLeft+Space',
  launchAtLogin: true,
  playSoundOnComplete: false,
  autoDismissOverlay: true,
  inputDevice: 'default',
  model: 'parakeet-tdt-0.6b' as const,
  onboardingComplete: false,
  overlayPosition: 'bottom' as 'bottom' | 'top',
  showPassiveOverlay: true,
  historyRetentionDays: 0,
  helpImprove: false,
  beamSearch: false,
}

export const STT_MODELS = [
  { id: 'parakeet-tdt-0.6b' as const, name: 'Parakeet TDT — NVIDIA', size: '465 MB', description: 'Best accuracy · 25 languages · fast on any PC', recommended: true },
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
  accessibility_denied: {
    title: 'Accessibility access needed',
    help: 'Enable Chirp in System Settings > Privacy > Accessibility',
    action: { label: 'Open Settings', type: 'os_settings' as const },
  },
  unknown: {
    title: 'Something went wrong',
    help: 'Please try again',
    action: { label: 'Try Again', type: 'retry' as const },
  },
} as const

export type ErrorType = keyof typeof ERROR_MESSAGES
