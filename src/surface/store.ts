import { createContext, useContext } from 'react'
import { createStore, useStore, type StoreApi } from 'zustand'
import type {
  AiStatus,
  AudioDevice,
  FirstRunStep,
  HistoryEntry,
  PauseState,
  Phase,
  Pill,
  Settings,
  SnippetEntry,
  Theme,
  UiState,
  View,
  VocabEntry,
} from './types'
import type { Api } from './api'

export interface SurfaceState {
  theme: Theme
  settings: Settings
  vocabulary: VocabEntry[]
  snippets: SnippetEntry[]
  history: HistoryEntry[]
  devices: AudioDevice[]
  levels: Record<string, number>
  /** 11 bar heights, 0..1, while listening. */
  bars: number[]
  ai: AiStatus
  pause: PauseState
  update: string | null
  download: { progress: number; etaSec: number | null }

  phase: Phase
  /** Fading out: the pill after a paste (fade + scale), or the surface content before it collapses. */
  leaving: false | 'pill' | 'surface'
  /** Bump `n` to re-anchor the window on another monitor. */
  anchor: { to: 'foreground' | 'cursor' | 'primary'; n: number }
  pill: Pill | null
  tickHover: boolean
  view: View
  /** Direction of the last drill: 1 into a view, -1 back. */
  drill: 1 | -1
  firstRun: FirstRunStep
  ui: UiState
}

export const DEFAULT_SETTINGS: Settings = {
  holdBinding: { keys: [{ scan: 29, extended: true, vk: 163 }], label: 'Right Ctrl', suppress: false },
  toggleBinding: {
    keys: [
      { scan: 29, extended: false, vk: 162 },
      { scan: 91, extended: true, vk: 91 },
    ],
    label: 'Left Ctrl + Left Win',
    suppress: false,
  },
  inputDevice: 'default',
  onboardingComplete: false,
  aiCleanup: true,
  provider: 'anthropic',
  appearance: 'system',
  cancelBinding: { keys: [{ scan: 1, extended: false, vk: 27 }], label: 'Esc', suppress: false },
  playSoundOnComplete: false,
  model: 'parakeet-tdt-0.6b',
  mediaWhileDictating: 'lower',
  mediaLowerBy: 80,
}

export const IDLE_BARS = [4, 8, 14, 10, 18, 12, 16, 7, 11, 5, 4].map((h) => (h - 3) / 15)

export function initialState(theme: Theme): SurfaceState {
  return {
    theme,
    settings: DEFAULT_SETTINGS,
    vocabulary: [],
    snippets: [],
    history: [],
    devices: [],
    levels: {},
    bars: IDLE_BARS.map(() => 0),
    ai: { state: 'none', keyHint: null, fromEnv: false },
    pause: { paused: false, until: null },
    update: null,
    download: { progress: 0, etaSec: null },
    phase: 'hidden',
    leaving: false,
    anchor: { to: 'primary', n: 0 },
    pill: null,
    tickHover: false,
    view: 'home',
    drill: 1,
    firstRun: 'ready',
    ui: { copied: null, rebind: null, pauseMenu: false, words: null, aiDraft: '' },
  }
}

export type SurfaceStore = StoreApi<SurfaceState>

export function createSurfaceStore(state: SurfaceState): SurfaceStore {
  return createStore<SurfaceState>(() => state)
}

export interface SurfaceContextValue {
  store: SurfaceStore
  api: Api
  /** Static rendering for the preview harness: no window calls, no timers. */
  still: boolean
}

export const SurfaceContext = createContext<SurfaceContextValue | null>(null)

export function useSurfaceContext(): SurfaceContextValue {
  const ctx = useContext(SurfaceContext)
  if (!ctx) throw new Error('SurfaceContext missing')
  return ctx
}

export function useSurface<T>(selector: (s: SurfaceState) => T): T {
  return useStore(useSurfaceContext().store, selector)
}

export function setUi(store: SurfaceStore, patch: Partial<UiState>) {
  store.setState((s) => ({ ui: { ...s.ui, ...patch } }))
}
