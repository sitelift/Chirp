import { createContext, useContext } from 'react'
import { createStore, useStore, type StoreApi } from 'zustand'
import type {
  AiStatus,
  AiStatuses,
  AudioDevice,
  FirstRunStep,
  HistoryEntry,
  PauseState,
  Phase,
  Pill,
  Provider,
  Section,
  Settings,
  SnippetEntry,
  Theme,
  UiState,
  View,
  VocabEntry,
} from './types'
import type { Api } from './api'

export interface SurfaceState {
  /** The Windows theme; `settings.appearance` may override it (see `themeOf`). */
  theme: Theme
  settings: Settings
  vocabulary: VocabEntry[]
  snippets: SnippetEntry[]
  history: HistoryEntry[]
  devices: AudioDevice[]
  levels: Record<string, number>
  /** 11 bar heights, 0..1, while listening. */
  bars: number[]
  ai: AiStatuses
  pause: PauseState
  update: string | null
  download: { progress: number; etaSec: number | null }

  phase: Phase
  /** The pill fading out after a paste (fade + scale). */
  leaving: false | 'pill'
  /** Bump `n` to re-anchor the window on another monitor. */
  anchor: { to: 'foreground' | 'cursor' | 'primary'; n: number }
  pill: Pill | null
  tickHover: boolean
  view: View
  /** The Settings section on show. */
  section: Section
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

export const NO_KEY: AiStatus = { state: 'none', keyHint: null, fromEnv: false }

/** Accepts the per-provider statuses, or a single (Anthropic) status from an older backend. */
export function aiStatuses(raw: unknown, prev: AiStatuses): AiStatuses {
  if (!raw || typeof raw !== 'object') return prev
  const r = raw as Partial<AiStatuses> & Partial<AiStatus>
  if (typeof r.state === 'string') return { ...prev, anthropic: r as AiStatus }
  return { anthropic: r.anthropic ?? prev.anthropic, openai: r.openai ?? prev.openai }
}

/** `provider: null` comes from settings written before providers existed. */
export function providerOf(settings: Settings): Provider {
  return settings.provider ?? (settings.aiCleanup ? 'anthropic' : 'off')
}

export function themeOf(s: Pick<SurfaceState, 'theme' | 'settings'>): Theme {
  return s.settings.appearance === 'light' || s.settings.appearance === 'dark' ? s.settings.appearance : s.theme
}

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
    ai: { anthropic: NO_KEY, openai: NO_KEY },
    pause: { paused: false, until: null },
    update: null,
    download: { progress: 0, etaSec: null },
    phase: 'hidden',
    leaving: false,
    anchor: { to: 'primary', n: 0 },
    pill: null,
    tickHover: false,
    view: 'home',
    section: 'chirp',
    firstRun: 'ready',
    ui: { copied: null, rebind: null, aiDraft: '' },
  }
}

export type SurfaceStore = StoreApi<SurfaceState>

export function createSurfaceStore(state: SurfaceState): SurfaceStore {
  return createStore<SurfaceState>(() => state)
}

export interface SurfaceContextValue {
  store: SurfaceStore
  api: Api
  /** Static rendering for the preview harness: no window calls, no timers, no motion. */
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
