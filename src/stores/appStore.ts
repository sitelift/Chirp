import { create } from 'zustand'
import { DEFAULT_SETTINGS, type ErrorType } from '../lib/constants'

export type AppStatus = 'idle' | 'listening' | 'processing' | 'polishing' | 'done' | 'error'
export type SttModel = 'parakeet-tdt-0.6b'

export interface DictionaryEntry {
  from: string
  to: string
}

export interface SnippetEntry {
  trigger: string
  expansion: string
}

export interface TranscriptionEntry {
  text: string
  timestamp: string
  wordCount: number
  durationMs: number
  speechDurationMs: number
  wasCleanedUp?: boolean
}

export interface AppState {
  // Recording state
  status: AppStatus
  errorType: ErrorType | null
  wordCount: number | null
  amplitudes: number[]

  // Settings
  hotkey: string
  launchAtLogin: boolean
  showInMenuBar: boolean
  playSoundOnComplete: boolean
  autoDismissOverlay: boolean
  smartFormatting: boolean

  // Audio
  inputDevice: string
  inputLevel: number
  noiseSuppression: boolean

  // Model
  model: SttModel
  modelDownloaded: Record<string, boolean>
  modelDownloadProgress: number | null

  // AI Cleanup
  aiCleanup: boolean
  llmReady: boolean
  llmDownloadProgress: number | null

  // Dictionary
  dictionary: DictionaryEntry[]

  // Snippets
  snippets: SnippetEntry[]

  // History
  history: TranscriptionEntry[]

  // Onboarding
  onboardingComplete: boolean

  // Tone
  toneMode: string

  // Overlay
  overlayPosition: 'bottom' | 'top'
  showPassiveOverlay: boolean

  // Settings saved indicator
  settingsSaved: boolean

  // Loading
  settingsLoaded: boolean

  // Settings page
  settingsPage: string

  // Actions
  setStatus: (status: AppStatus) => void
  setError: (errorType: ErrorType) => void
  setAmplitudes: (data: number[]) => void
  setWordCount: (count: number) => void
  setInputLevel: (level: number) => void
  setModelDownloadProgress: (progress: number | null) => void
  setLlmDownloadProgress: (progress: number | null) => void
  setLlmReady: (ready: boolean) => void
  updateSettings: (partial: Partial<AppState>) => void
  addDictionaryEntry: (from: string, to: string) => void
  removeDictionaryEntry: (index: number) => void
  setSnippets: (snippets: SnippetEntry[]) => void
  addSnippet: (trigger: string, expansion: string) => void
  updateSnippet: (index: number, trigger: string, expansion: string) => void
  removeSnippet: (index: number) => void
  setSettingsLoaded: () => void
  setHistory: (history: TranscriptionEntry[]) => void
  removeHistoryEntry: (timestamp: string) => void
  setSettingsPage: (page: string) => void
  setOnboardingComplete: (complete: boolean) => void
  setSettingsSaved: (saved: boolean) => void
}

export const useAppStore = create<AppState>((set) => ({
  // Recording state
  status: 'idle',
  errorType: null,
  wordCount: null,
  amplitudes: [],

  // Settings (from defaults)
  hotkey: DEFAULT_SETTINGS.hotkey,
  launchAtLogin: DEFAULT_SETTINGS.launchAtLogin,
  showInMenuBar: DEFAULT_SETTINGS.showInMenuBar,
  playSoundOnComplete: DEFAULT_SETTINGS.playSoundOnComplete,
  autoDismissOverlay: DEFAULT_SETTINGS.autoDismissOverlay,
  smartFormatting: DEFAULT_SETTINGS.smartFormatting,

  // Audio
  inputDevice: DEFAULT_SETTINGS.inputDevice,
  inputLevel: 0,
  noiseSuppression: DEFAULT_SETTINGS.noiseSuppression,

  // Model
  model: DEFAULT_SETTINGS.model,
  modelDownloaded: {},
  modelDownloadProgress: null,

  // AI Cleanup
  aiCleanup: DEFAULT_SETTINGS.aiCleanup,
  llmReady: false,
  llmDownloadProgress: null,

  // Dictionary
  dictionary: [],

  // Snippets
  snippets: [],

  // History
  history: [],

  // Onboarding
  onboardingComplete: DEFAULT_SETTINGS.onboardingComplete,

  // Tone
  toneMode: DEFAULT_SETTINGS.toneMode,

  // Overlay
  overlayPosition: DEFAULT_SETTINGS.overlayPosition,
  showPassiveOverlay: DEFAULT_SETTINGS.showPassiveOverlay,

  // Settings saved indicator
  settingsSaved: false,

  // Loading
  settingsLoaded: false,

  // Settings page
  settingsPage: 'home',

  // Actions
  setStatus: (status) => set({ status, errorType: status !== 'error' ? null : undefined }),
  setError: (errorType) => set({ status: 'error', errorType }),
  setAmplitudes: (amplitudes) => set({ amplitudes }),
  setWordCount: (wordCount) => set({ wordCount }),
  setInputLevel: (inputLevel) => set({ inputLevel }),
  setModelDownloadProgress: (modelDownloadProgress) => set({ modelDownloadProgress }),
  setLlmDownloadProgress: (llmDownloadProgress) => set({ llmDownloadProgress }),
  setLlmReady: (llmReady) => set({ llmReady }),
  updateSettings: (partial) => set(partial),
  addDictionaryEntry: (from, to) =>
    set((state) => ({ dictionary: [...state.dictionary, { from, to }] })),
  removeDictionaryEntry: (index) =>
    set((state) => ({ dictionary: state.dictionary.filter((_, i) => i !== index) })),
  setSnippets: (snippets) => set({ snippets }),
  addSnippet: (trigger, expansion) =>
    set((state) => ({ snippets: [...state.snippets, { trigger, expansion }] })),
  updateSnippet: (index, trigger, expansion) =>
    set((state) => ({
      snippets: state.snippets.map((s, i) => (i === index ? { trigger, expansion } : s)),
    })),
  removeSnippet: (index) =>
    set((state) => ({ snippets: state.snippets.filter((_, i) => i !== index) })),
  setSettingsLoaded: () => set({ settingsLoaded: true }),
  setHistory: (history) => set({ history }),
  removeHistoryEntry: (timestamp) =>
    set((state) => ({ history: state.history.filter((e) => e.timestamp !== timestamp) })),
  setSettingsPage: (settingsPage) => set({ settingsPage }),
  setOnboardingComplete: (onboardingComplete) => set({ onboardingComplete }),
  setSettingsSaved: (settingsSaved) => set({ settingsSaved }),
}))
