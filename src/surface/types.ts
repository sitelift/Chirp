export interface BindingKey {
  scan: number
  extended: boolean
  vk: number
}
export interface Binding {
  keys: BindingKey[]
  label: string
  suppress: boolean
}
export type BindingTarget = 'hold' | 'toggle' | 'cancel'

export interface Settings {
  holdBinding: Binding | null
  toggleBinding: Binding | null
  inputDevice: string
  onboardingComplete: boolean
  /** Legacy switch; `provider` wins when set. */
  aiCleanup: boolean
  /** null in settings written before providers existed: treat as aiCleanup ? 'anthropic' : 'off'. */
  provider: Provider | null
  appearance: Appearance
  cancelBinding: Binding | null
  playSoundOnComplete: boolean
  model: string
  mediaWhileDictating: MediaMode
  /** With 'lower': percent quieter, 10..100. */
  mediaLowerBy: number
}

/** Which cloud model cleans up dictations; 'off' pastes the recognizer text with spoken commands converted. */
export type Provider = 'off' | 'anthropic' | 'openai'
export type KeyProvider = Exclude<Provider, 'off'>
export type Appearance = 'system' | 'light' | 'dark'

/** What other apps' audio does while recording. */
export type MediaMode = 'off' | 'lower' | 'pause'

export interface VocabEntry {
  term: string
  /** Kept on disk for compatibility; ignored, and written as []. */
  replaces: string[]
}
export interface SnippetEntry {
  trigger: string
  expansion: string
}
export interface HistoryEntry {
  text: string
  timestamp: string
  wordCount: number
  wasCleanedUp?: boolean
}
export interface AudioDevice {
  name: string
  id: string
}

export type KeyState = 'none' | 'checking' | 'connected' | 'invalid' | 'offline'
export interface AiStatus {
  state: KeyState
  keyHint: string | null
  fromEnv: boolean
}
/** One status per provider; each keeps its own key. */
export type AiStatuses = Record<KeyProvider, AiStatus>

export interface PauseState {
  paused: boolean
  until: number | null
}

export type Theme = 'light' | 'dark'

/** What the one window is showing. */
export type Phase = 'hidden' | 'tick' | 'pill' | 'surface' | 'firstrun'

export type Pill =
  | { kind: 'listening' }
  | { kind: 'handsfree' }
  | { kind: 'working' }
  | { kind: 'paused' }
  /** Text pills. `action` makes them clickable; `check` adds the amber check. */
  | { kind: 'message'; text: string; action?: 'microphone'; check?: boolean }

export type View = 'home' | 'recent' | 'words' | 'snippets' | 'settings'
export type Section = 'chirp' | 'shortcuts' | 'microphone' | 'sound' | 'ai'
export type FirstRunStep = 'downloading' | 'mic-blocked' | 'ready'

export type RebindPhase = 'listening' | 'saved' | 'conflict'
export interface RebindState {
  target: BindingTarget
  phase: RebindPhase
  /** Labels of keys held so far (live), or the refused combination. */
  keys: string[]
  message?: string
}

/** Transient presentation state; kept in the store so the preview can set it. */
export interface UiState {
  copied: string | null
  rebind: RebindState | null
  /** The API key being typed. */
  aiDraft: string
  /** Preview only: draw the keyboard focus ring on this element. */
  forceFocus?: string
  /** Preview only: draw this element in its hover state. */
  forceHover?: string
  /** Preview only: draw this element pressed. */
  forcePress?: string
}
