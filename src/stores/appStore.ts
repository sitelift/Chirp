import { create } from 'zustand'
import { DEFAULT_SETTINGS, type ErrorType } from '../lib/constants'

export type AppStatus = 'idle' | 'listening' | 'processing' | 'polishing' | 'done' | 'error'
export type SttModel = 'parakeet-tdt-0.6b'
export type HotkeyMode = 'hold' | 'tap'

export interface SnippetEntry {
  trigger: string
  expansion: string
}

/**
 * A vocabulary entry: a canonical term plus optional list of mishearings
 * to find/replace toward this term. `term` drives ASR-side hotword biasing
 * (sherpa-onnx); `replaces` drives the post-ASR find/replace pass for
 * homophones and stable mishearings the ASR can't fix on its own.
 */
export interface VocabEntry {
  term: string
  replaces: string[]
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
  hotkeyMode: HotkeyMode
  launchAtLogin: boolean
  playSoundOnComplete: boolean
  autoDismissOverlay: boolean
  smartFormatting: boolean

  // Audio
  inputDevice: string
  inputLevel: number

  // Model
  model: SttModel
  modelDownloaded: Record<string, boolean>
  modelDownloadProgress: number | null

  // AI Cleanup
  aiCleanup: boolean
  cleanupModel: string
  llmReady: boolean
  llmDownloaded: boolean

  // Beam Search
  beamSearch: boolean
  llmDownloadProgress: number | null

  // Vocabulary
  vocabulary: VocabEntry[]

  // Snippets
  snippets: SnippetEntry[]

  // History
  history: TranscriptionEntry[]

  // Onboarding
  onboardingComplete: boolean

  // Tone
  toneMode: string

  // Overlay
  overlayPosition: string | { x: number; y: number }
  showPassiveOverlay: boolean
  repositionMode: boolean

  // History retention
  historyRetentionDays: number

  // Appearance
  darkMode: boolean

  // Telemetry
  helpImprove: boolean

  // Hotkey status
  hotkeyStatus: 'idle' | 'retrying' | 'active' | 'failed' | 'accessibility_required'

  // Settings saved indicator
  settingsSaved: boolean

  // About modal
  aboutModalOpen: boolean

  // Upgrade modal (shown when user needs the new cleanup model)
  upgradeModalOpen: boolean

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
  setLlmDownloadProgress: (progress: number | null) => void
  setLlmReady: (ready: boolean) => void
  setLlmDownloaded: (downloaded: boolean) => void
  updateSettings: (partial: Partial<AppState>) => void
  addVocabularyEntry: (term: string) => void
  updateVocabularyEntry: (index: number, entry: VocabEntry) => void
  removeVocabularyEntry: (index: number) => void
  setVocabulary: (vocabulary: VocabEntry[]) => void
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
  setUpgradeModalOpen: (open: boolean) => void
  setUpdateAvailable: (version: string | null) => void
  setRepositionMode: (mode: boolean) => void
}

export const useAppStore = create<AppState>((set) => ({
  // Recording state
  status: 'idle',
  errorType: null,
  wordCount: null,
  amplitudes: [],

  // Settings (from defaults)
  hotkey: DEFAULT_SETTINGS.hotkey,
  hotkeyMode: DEFAULT_SETTINGS.hotkeyMode,
  launchAtLogin: DEFAULT_SETTINGS.launchAtLogin,
  playSoundOnComplete: DEFAULT_SETTINGS.playSoundOnComplete,
  autoDismissOverlay: DEFAULT_SETTINGS.autoDismissOverlay,
  smartFormatting: DEFAULT_SETTINGS.smartFormatting,

  // Audio
  inputDevice: DEFAULT_SETTINGS.inputDevice,
  inputLevel: 0,

  // Model
  model: DEFAULT_SETTINGS.model,
  modelDownloaded: {},
  modelDownloadProgress: null,

  // AI Cleanup
  aiCleanup: DEFAULT_SETTINGS.aiCleanup,
  cleanupModel: DEFAULT_SETTINGS.cleanupModel,
  llmReady: false,
  llmDownloaded: false,
  llmDownloadProgress: null,

  // Beam Search
  beamSearch: DEFAULT_SETTINGS.beamSearch,

  // Vocabulary
  vocabulary: [],

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
  repositionMode: false,

  // History retention
  historyRetentionDays: DEFAULT_SETTINGS.historyRetentionDays,

  // Appearance
  darkMode: DEFAULT_SETTINGS.darkMode,

  // Telemetry
  helpImprove: DEFAULT_SETTINGS.helpImprove,

  // Hotkey status
  hotkeyStatus: 'idle',

  // Settings saved indicator
  settingsSaved: false,

  // About modal
  aboutModalOpen: false,
  upgradeModalOpen: false,

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
  setLlmDownloadProgress: (llmDownloadProgress) => set({ llmDownloadProgress }),
  setLlmReady: (llmReady) => set({ llmReady }),
  setLlmDownloaded: (llmDownloaded) => set({ llmDownloaded }),
  updateSettings: (partial) => set(partial),
  addVocabularyEntry: (term) =>
    set((state) => ({
      vocabulary: [...state.vocabulary, { term, replaces: [] }],
    })),
  updateVocabularyEntry: (index, entry) =>
    set((state) => ({
      vocabulary: state.vocabulary.map((e, i) => (i === index ? entry : e)),
    })),
  removeVocabularyEntry: (index) =>
    set((state) => ({ vocabulary: state.vocabulary.filter((_, i) => i !== index) })),
  setVocabulary: (vocabulary) => set({ vocabulary }),
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
  setUpgradeModalOpen: (upgradeModalOpen) => set({ upgradeModalOpen }),
  setUpdateAvailable: (updateAvailable) => set({ updateAvailable }),
  setRepositionMode: (repositionMode) => set({ repositionMode }),
}))
