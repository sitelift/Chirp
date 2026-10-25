import { DEFAULT_SETTINGS, IDLE_BARS, type SurfaceState } from '../surface/store'
import type { AiStatus, Section, Settings, Theme, UiState } from '../surface/types'
import { AUDIO_DEVICES, HISTORY, SNIPPETS, VOCABULARY } from './data'

export interface Scenario {
  label: string
  height?: number
  state: (theme: Theme) => Partial<Omit<SurfaceState, 'ui'>> & { ui?: Partial<UiState> }
}
export interface Section_ {
  id: string
  title: string
  note: string
  width: number
  height: number
  scenarios: Scenario[]
}

const CONNECTED: AiStatus = { state: 'connected', keyHint: 'sk-ant-••••••••••4f2a', fromEnv: false }
const NONE: AiStatus = { state: 'none', keyHint: null, fromEnv: false }
const SETTINGS: Settings = { ...DEFAULT_SETTINGS, onboardingComplete: true, inputDevice: AUDIO_DEVICES[1].id, mediaWhileDictating: 'off', mediaLowerBy: 60 }

/** Everything a populated install has. */
const open = (extra: Partial<SurfaceState> = {}, settings: Partial<Settings> = {}): Partial<SurfaceState> => ({
  phase: 'surface',
  view: 'home',
  history: HISTORY,
  vocabulary: VOCABULARY,
  snippets: SNIPPETS,
  devices: AUDIO_DEVICES,
  settings: { ...SETTINGS, ...settings },
  ai: { anthropic: CONNECTED, openai: NONE },
  ...extra,
})
const settings = (section: Section, extra: Partial<SurfaceState> = {}, s: Partial<Settings> = {}) => open({ view: 'settings', section, ...extra }, s)
const ui = (state: Partial<SurfaceState>, patch: Partial<UiState>) => ({ ...state, ui: patch })

const pill = (p: SurfaceState['pill'], extra: Partial<SurfaceState> = {}): Partial<SurfaceState> => ({
  phase: 'pill',
  pill: p,
  bars: IDLE_BARS,
  settings: SETTINGS,
  ...extra,
})

const LEVELS = { [AUDIO_DEVICES[1].id]: 0.8, [AUDIO_DEVICES[2].id]: 0.15 }
const WIDE = { width: 700, height: 500 }

export const SECTIONS: Section_[] = [
  {
    id: '01',
    title: '01 Idle',
    note: 'A faint tick marks the spot; it becomes the pill under the pointer.',
    width: 300,
    height: 160,
    scenarios: [
      { label: 'Resting', state: () => pill(null, { phase: 'tick' }) },
      { label: 'Hover', state: () => pill(null, { phase: 'tick', tickHover: true }) },
    ],
  },
  {
    id: '02',
    title: '02 Dictation',
    note: 'Hold to talk. Tap the hands-free combo to latch; tap again or press Cancel to stop. Success shows nothing.',
    width: 300,
    height: 160,
    scenarios: [
      { label: 'Hold', state: () => pill({ kind: 'listening' }) },
      { label: 'Hands-free', state: () => pill({ kind: 'handsfree' }) },
      { label: 'Working', state: () => pill({ kind: 'working' }) },
      { label: 'Pasted without AI', state: () => pill({ kind: 'message', text: 'Pasted without AI · offline' }) },
      { label: 'Nothing heard', state: () => pill({ kind: 'message', text: "Didn't catch that" }) },
      { label: 'No mic', state: () => pill({ kind: 'message', text: 'No microphone', action: 'microphone' }) },
      { label: 'Cancelled', state: () => pill({ kind: 'message', text: 'Cancelled' }) },
      { label: 'Paste blocked', state: () => pill({ kind: 'message', text: 'Copied. Press Ctrl+V' }) },
      { label: 'Paused', state: () => pill({ kind: 'paused' }) },
    ],
  },
  {
    id: '03',
    title: '03 Home',
    note: 'The last dictation (click copies), Recent, Words, Snippets; Pause and Settings. Esc or click away closes.',
    width: 360,
    height: 380,
    scenarios: [
      { label: 'Home', state: () => open() },
      { label: 'Hover row', state: () => ui(open(), { forceHover: 'words' }) },
      { label: 'Pressed', state: () => ui(open(), { forcePress: 'snippets' }) },
      { label: 'Hover dictation', state: () => ui(open(), { forceHover: 'last' }) },
      { label: 'Copied', state: () => ui(open(), { copied: 'last' }) },
      { label: 'Paused', state: () => open({ pause: { paused: true, until: null } }) },
      { label: 'Nothing dictated yet', state: () => open({ history: [], vocabulary: [], snippets: [] }) },
      { label: 'Keyboard focus', state: () => ui(open(), { forceFocus: 'recent' }) },
    ],
  },
  {
    id: '04',
    title: '04 Recent',
    note: 'The last 20 dictations, kept on this PC. Click one to copy it.',
    width: 360,
    height: 380,
    scenarios: [
      { label: 'List', state: () => open({ view: 'recent' }) },
      { label: 'Hover', state: () => ui(open({ view: 'recent' }), { forceHover: `recent-${HISTORY[3].timestamp}` }) },
      { label: 'Copied', state: () => ui(open({ view: 'recent' }), { copied: HISTORY[3].timestamp }) },
      { label: 'Empty', state: () => open({ view: 'recent', history: [] }) },
    ],
  },
  {
    id: '05',
    title: '05 Words',
    note: 'Names and words Chirp spells exactly as written. Enter or + adds; hover a row to remove it.',
    width: 360,
    height: 440,
    scenarios: [
      { label: 'List', state: () => open({ view: 'words' }) },
      { label: 'Hover row', state: () => ui(open({ view: 'words' }), { forceHover: 'word-Parakeet' }) },
      { label: 'Typing', state: () => ui(open({ view: 'words' }), { forceFocus: 'word-input' }) },
      { label: 'Empty', state: () => open({ view: 'words', vocabulary: [] }) },
    ],
  },
  {
    id: '06',
    title: '06 Snippets',
    note: 'Say the phrase and Chirp types the rest.',
    width: 360,
    height: 440,
    scenarios: [
      { label: 'List', state: () => open({ view: 'snippets' }) },
      { label: 'Hover row', state: () => ui(open({ view: 'snippets' }), { forceHover: 'snippet-sig' }) },
      { label: 'Typing', state: () => ui(open({ view: 'snippets' }), { forceFocus: 'snippet-form' }) },
      { label: 'Empty', state: () => open({ view: 'snippets', snippets: [] }) },
    ],
  },
  {
    id: '07',
    title: '07 Settings: Chirp',
    note: 'Appearance follows Windows unless set. Switching crossfades the page.',
    ...WIDE,
    scenarios: [
      { label: 'System', state: () => settings('chirp') },
      { label: 'Always dark', state: () => settings('chirp', {}, { appearance: 'dark' }) },
      { label: 'Update ready', state: () => settings('chirp', { update: '2.0.1' }) },
    ],
  },
  {
    id: '08',
    title: '08 Settings: Shortcuts',
    note: 'Click the keys, press the new combination. Clicking elsewhere keeps the old keys.',
    ...WIDE,
    scenarios: [
      { label: 'Shortcuts', state: () => settings('shortcuts') },
      { label: 'Hover', state: () => ui(settings('shortcuts'), { forceHover: 'bind-toggle' }) },
      { label: 'Press keys', state: () => ui(settings('shortcuts'), { rebind: { target: 'cancel', phase: 'listening', keys: [] } }) },
      { label: 'Holding', state: () => ui(settings('shortcuts'), { rebind: { target: 'hold', phase: 'listening', keys: ['Ctrl', 'Shift'] } }) },
      { label: 'Saved', state: () => ui(settings('shortcuts', {}, { cancelBinding: { keys: [], label: 'F8', suppress: false } }), { rebind: { target: 'cancel', phase: 'saved', keys: ['F8'] } }) },
      { label: 'Conflict', state: () => ui(settings('shortcuts'), { rebind: { target: 'hold', phase: 'conflict', keys: ['Win', 'H'], message: 'Used by Windows' } }) },
    ],
  },
  {
    id: '09',
    title: '09 Settings: Microphone',
    note: 'Say something. The one that moves is the one that hears you.',
    ...WIDE,
    scenarios: [
      { label: 'Picker', state: () => settings('microphone', { levels: LEVELS }) },
      { label: 'Hover', state: () => ui(settings('microphone', { levels: LEVELS }), { forceHover: `mic-${AUDIO_DEVICES[3].id}` }) },
      { label: 'Unplugged', state: () => settings('microphone', { devices: [AUDIO_DEVICES[0], AUDIO_DEVICES[3]] }) },
    ],
  },
  {
    id: '10',
    title: '10 Settings: Sound',
    note: 'Sounds are off by default. Lower reveals how much quieter.',
    ...WIDE,
    scenarios: [
      { label: 'Default', state: () => settings('sound') },
      { label: 'Lower', state: () => settings('sound', {}, { mediaWhileDictating: 'lower', playSoundOnComplete: true }) },
      { label: 'Pause', state: () => settings('sound', {}, { mediaWhileDictating: 'pause' }) },
    ],
  },
  {
    id: '11',
    title: '11 Settings: AI cleanup',
    note: 'Off, Anthropic or OpenAI, each with its own key. Paste a key and it is checked and stored in Windows Credential Manager.',
    ...WIDE,
    scenarios: [
      { label: 'Connected', state: () => settings('ai') },
      { label: 'Off', state: () => settings('ai', {}, { provider: 'off' }) },
      { label: 'Add key', state: () => settings('ai', {}, { provider: 'openai' }) },
      { label: 'Checking', state: () => settings('ai', { ai: { anthropic: CONNECTED, openai: { state: 'checking', keyHint: 'sk-••••••••••9c1e', fromEnv: false } } }, { provider: 'openai' }) },
      { label: 'Not accepted', state: () => settings('ai', { ai: { anthropic: { state: 'invalid', keyHint: 'sk-ant-••••••••••x7Qe', fromEnv: false }, openai: NONE } }) },
      { label: 'Offline', state: () => settings('ai', { ai: { anthropic: { ...CONNECTED, state: 'offline' }, openai: NONE } }) },
      { label: 'From environment', state: () => settings('ai', { ai: { anthropic: { ...CONNECTED, fromEnv: true }, openai: NONE } }) },
      { label: 'Legacy (provider unset)', state: () => settings('ai', {}, { provider: null, aiCleanup: false }) },
    ],
  },
  {
    id: '12',
    title: '12 First run',
    note: 'No wizard. The pill teaches the hotkey once the model is ready, then teaches that it can be clicked.',
    width: 360,
    height: 220,
    scenarios: [
      { label: 'Getting ready', state: () => ({ phase: 'firstrun', firstRun: 'downloading', download: { progress: 18, etaSec: 240 } }) },
      { label: 'Mic blocked', state: () => ({ phase: 'firstrun', firstRun: 'mic-blocked' }) },
      { label: 'Ready', state: () => ({ phase: 'firstrun', firstRun: 'ready' }) },
      { label: 'Done', state: () => pill({ kind: 'message', text: "You're set. Click me anytime.", check: true }) },
    ],
  },
]
