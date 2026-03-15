import { create } from 'zustand'
import { DEFAULT_SETTINGS, type ErrorType } from '../lib/constants'

export type AppStatus = 'idle' | 'listening' | 'processing' | 'done' | 'error'
export type SttModel = 'parakeet-tdt-0.6b'

export interface DictionaryEntry {
  from: string
  to: string
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
  silenceTimeout: number
  language: string
  smartFormatting: boolean

  // Audio
  inputDevice: string
  inputLevel: number
  noiseSuppression: boolean

  // Model
  model: SttModel
  modelDownloaded: Record<string, boolean>
  modelDownloadProgress: number | null

  // Dictionary
  dictionary: DictionaryEntry[]

  // Onboarding
  onboardingComplete: boolean

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
  setSettingsPage: (page: string) => void
  setOnboardingComplete: (complete: boolean) => void
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
  silenceTimeout: DEFAULT_SETTINGS.silenceTimeout,
  language: DEFAULT_SETTINGS.language,
  smartFormatting: DEFAULT_SETTINGS.smartFormatting,

  // Audio
  inputDevice: DEFAULT_SETTINGS.inputDevice,
  inputLevel: 0,
  noiseSuppression: DEFAULT_SETTINGS.noiseSuppression,

  // Model
  model: DEFAULT_SETTINGS.model,
  modelDownloaded: {},
  modelDownloadProgress: null,

  // Dictionary
  dictionary: [],

  // Onboarding
  onboardingComplete: DEFAULT_SETTINGS.onboardingComplete,

  // Settings page
  settingsPage: 'general',

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
  setSettingsPage: (settingsPage) => set({ settingsPage }),
  setOnboardingComplete: (onboardingComplete) => set({ onboardingComplete }),
}))
