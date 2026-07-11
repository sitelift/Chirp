import { create } from 'zustand'
import { DEFAULT_SETTINGS, type ErrorType } from '../lib/constants'

export type AppStatus = 'idle' | 'listening' | 'processing' | 'done' | 'error'
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
  playSoundOnComplete: boolean
  autoDismissOverlay: boolean

  // Audio
  inputDevice: string
  inputLevel: number

  // Model
  model: SttModel
  modelDownloaded: Record<string, boolean>
  modelDownloadProgress: number | null

  // Beam Search
  beamSearch: boolean

  // Dictionary
  dictionary: DictionaryEntry[]

  // Snippets
  snippets: SnippetEntry[]

  // History
  history: TranscriptionEntry[]

  // Onboarding
  onboardingComplete: boolean

  // Overlay
  overlayPosition: 'bottom' | 'top'
  showPassiveOverlay: boolean

  // History retention
  historyRetentionDays: number

  // Telemetry
  helpImprove: boolean

  // Hotkey status
  hotkeyStatus: 'idle' | 'retrying' | 'active' | 'failed' | 'accessibility_required'

  // Settings saved indicator
  settingsSaved: boolean

  // About modal
  aboutModalOpen: boolean

  // Update availability
  updateAvailable: string | null  // version string, or null

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
  updateSettings: (partial: Partial<AppState>) => void
  addDictionaryEntry: (from: string, to: string) => void
  removeDictionaryEntry: (index: number) => void
  setDictionary: (dictionary: DictionaryEntry[]) => void
  setSnippets: (snippets: SnippetEntry[]) => void
  addSnippet: (trigger: string, expansion: string) => void
  updateSnippet: (index: number, trigger: string, expansion: string) => void
  removeSnippet: (index: number) => void
  setSettingsLoaded: () => void
  setHistory: (history: TranscriptionEntry[]) => void
  removeHistoryEntry: (timestamp: string) => void
  setSettingsPage: (page: string) => void
  setHotkeyStatus: (status: 'idle' | 'retrying' | 'active' | 'failed' | 'accessibility_required') => void
  setOnboardingComplete: (complete: boolean) => void
  setSettingsSaved: (saved: boolean) => void
  setAboutModalOpen: (open: boolean) => void
  setUpdateAvailable: (version: string | null) => void
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
  playSoundOnComplete: DEFAULT_SETTINGS.playSoundOnComplete,
  autoDismissOverlay: DEFAULT_SETTINGS.autoDismissOverlay,

  // Audio
  inputDevice: DEFAULT_SETTINGS.inputDevice,
  inputLevel: 0,

  // Model
  model: DEFAULT_SETTINGS.model,
  modelDownloaded: {},
  modelDownloadProgress: null,

  // Beam Search
  beamSearch: DEFAULT_SETTINGS.beamSearch,

  // Dictionary
  dictionary: [],

  // Snippets
  snippets: [],

  // History
  history: [],

  // Onboarding
  onboardingComplete: DEFAULT_SETTINGS.onboardingComplete,

  // Overlay
  overlayPosition: DEFAULT_SETTINGS.overlayPosition,
  showPassiveOverlay: DEFAULT_SETTINGS.showPassiveOverlay,

  // History retention
  historyRetentionDays: DEFAULT_SETTINGS.historyRetentionDays,

  // Telemetry
  helpImprove: DEFAULT_SETTINGS.helpImprove,

  // Hotkey status
  hotkeyStatus: 'idle',

  // Settings saved indicator
  settingsSaved: false,

  // About modal
  aboutModalOpen: false,

  // Update availability
  updateAvailable: null,

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
  updateSettings: (partial) => set(partial),
  addDictionaryEntry: (from, to) =>
    set((state) => ({ dictionary: [...state.dictionary, { from, to }] })),
  removeDictionaryEntry: (index) =>
    set((state) => ({ dictionary: state.dictionary.filter((_, i) => i !== index) })),
  setDictionary: (dictionary) => set({ dictionary }),
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
  setHotkeyStatus: (hotkeyStatus) => set({ hotkeyStatus }),
  setOnboardingComplete: (onboardingComplete) => set({ onboardingComplete }),
  setSettingsSaved: (settingsSaved) => set({ settingsSaved }),
  setAboutModalOpen: (aboutModalOpen) => set({ aboutModalOpen }),
  setUpdateAvailable: (updateAvailable) => set({ updateAvailable }),
}))
