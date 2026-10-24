import { DEFAULT_SETTINGS, IDLE_BARS, type SurfaceState } from '../surface/store'
import type { Theme, UiState } from '../surface/types'

export interface Scenario {
  label: string
  height?: number
  state: (theme: Theme) => Partial<Omit<SurfaceState, 'ui'>> & { ui?: Partial<UiState> }
}
export interface Section {
  id: string
  title: string
  note: string
  width: number
  height: number
  scenarios: Scenario[]
}

const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString()

const HISTORY = [
  { text: "Dinner at seven works, I'll book the place on the corner.", timestamp: ago(2 * 24 * 60), wordCount: 11 },
  { text: 'Looks good to me, merge it.', timestamp: ago(26 * 60), wordCount: 6 },
  { text: 'Remind me to call the landlord about the heating.', timestamp: ago(25 * 60), wordCount: 9 },
  { text: 'The CUDA build is about 40 percent faster on the 4080.', timestamp: ago(3 * 60), wordCount: 11 },
  { text: 'Can you add Anouk to the invite?', timestamp: ago(65), wordCount: 7 },
  { text: "Let's push the review to Thursday morning.", timestamp: ago(62), wordCount: 7 },
  { text: 'Hi Mark, thanks for sending this over. The total should be €4,250.', timestamp: ago(30), wordCount: 12 },
  { text: 'Yeah, we can ship it Friday. The resync thing is done, just waiting on the last review.', timestamp: ago(2), wordCount: 17 },
]
const RECENT_HOVER = HISTORY[6].timestamp

const VOCABULARY = [
  { term: 'SiteLift', replaces: ['sitelift'] },
  { term: 'Kubernetes', replaces: [] },
  { term: 'Anouk', replaces: [] },
  { term: 'PR', replaces: ['p r'] },
  { term: 'Parakeet', replaces: ['parakit'] },
  { term: 'Tauri', replaces: ['tory'] },
  { term: 'WebView2', replaces: ['web view two'] },
  { term: 'Pieter', replaces: ['peter'] },
]
const SNIPPETS = [
  { trigger: 'my sign off', expansion: 'Best, Pieter' },
  { trigger: 'office address', expansion: 'Keizersgracht 12, 1015 Amsterdam' },
  { trigger: 'my email', expansion: 'pieter@example.com' },
  { trigger: 'standup link', expansion: 'https://meet.example.com/standup' },
]
// Design order: the first six rows match the Paper frame.
const WORDS_VOCAB = [VOCABULARY[0], VOCABULARY[1], VOCABULARY[2]]
const WORDS_SNIPPETS = [SNIPPETS[0], SNIPPETS[1]]

const DEVICES = [
  { id: 'default', name: 'Default' },
  { id: 'Microphone (Brio 500)', name: 'Microphone (Brio 500)' },
  { id: 'Microphone Array (Laptop microphone)', name: 'Microphone Array (Laptop microphone)' },
]

/** Everything a populated install has. */
const home = (extra: Partial<SurfaceState> = {}): Partial<SurfaceState> => ({
  phase: 'surface',
  view: 'home',
  history: HISTORY,
  vocabulary: VOCABULARY,
  snippets: SNIPPETS,
  devices: DEVICES,
  settings: { ...DEFAULT_SETTINGS, onboardingComplete: true, inputDevice: 'Microphone (Brio 500)' },
  ai: { state: 'connected', keyHint: 'sk-ant-••••••••••3fA9', fromEnv: false },
  ...extra,
})

const pill = (p: SurfaceState['pill'], extra: Partial<SurfaceState> = {}): Partial<SurfaceState> => ({
  phase: 'pill',
  pill: p,
  bars: IDLE_BARS,
  settings: { ...DEFAULT_SETTINGS, onboardingComplete: true },
  ...extra,
})

const words = (ui?: Partial<UiState>, empty = false) => ({
  ...home({ view: 'words', vocabulary: empty ? [] : WORDS_VOCAB.concat([{ term: 'PR', replaces: ['p r'] }]), snippets: empty ? [] : WORDS_SNIPPETS }),
  ui,
})

// Rows the Words view shows in order: sitelift, kubernetes, anouk, p r, my sign off, office address.
// The design lists "p r" last; ours lists vocabulary before snippets.

export const SECTIONS: Section[] = [
  {
    id: 'ov',
    title: 'Overlay / v2',
    note: 'The ink pill states.',
    width: 300,
    height: 120,
    scenarios: [
      { label: 'Listening', state: () => pill({ kind: 'listening' }) },
      { label: 'Working', state: () => pill({ kind: 'working' }) },
      { label: 'No mic', state: () => pill({ kind: 'message', text: 'No microphone' }) },
      { label: 'Nothing heard', state: () => pill({ kind: 'message', text: "Didn't catch that" }) },
    ],
  },
  {
    id: '01',
    title: '01 Idle',
    note: 'After first run a faint tick marks the spot; it becomes the pill under the pointer.',
    width: 300,
    height: 200,
    scenarios: [
      { label: 'Resting', state: () => pill(null, { phase: 'tick' }) },
      { label: 'Hover', state: () => pill(null, { phase: 'tick', tickHover: true }) },
    ],
  },
  {
    id: '02',
    title: '02 Dictation',
    note: 'Hold to talk. Tap the hands-free combo to latch; tap again or Esc to stop. Success shows nothing.',
    width: 300,
    height: 200,
    scenarios: [
      { label: 'Hold', state: () => pill({ kind: 'listening' }) },
      { label: 'Hands-free', state: () => pill({ kind: 'handsfree' }) },
      { label: 'Working', state: () => pill({ kind: 'working' }) },
      { label: 'Pasted without AI', state: () => pill({ kind: 'message', text: 'Pasted without AI · offline' }) },
      { label: 'Nothing heard', state: () => pill({ kind: 'message', text: "Didn't catch that" }) },
      { label: 'No mic', state: () => pill({ kind: 'message', text: 'No microphone', action: 'microphone' }) },
      { label: 'Esc', state: () => pill({ kind: 'message', text: 'Cancelled' }) },
      { label: 'Paste blocked', state: () => pill({ kind: 'message', text: 'Copied. Press Ctrl+V' }) },
    ],
  },
  {
    id: '03',
    title: '03 Opened',
    note: 'Click the pill or the tray icon. Esc, × or click away closes. Click the last dictation to copy it.',
    width: 360,
    height: 500,
    scenarios: [
      { label: 'Home', state: () => home() },
      { label: 'Hover', state: () => ({ ...home(), ui: { forceHover: 'microphone' } }) },
      { label: 'Pressed', state: () => ({ ...home(), ui: { forcePress: 'words' } }) },
      { label: 'Hover dictation', state: () => ({ ...home(), ui: { forceHover: 'last' } }) },
      { label: 'Copied', state: () => ({ ...home(), ui: { copied: 'last' } }) },
    ],
  },
  {
    id: '04',
    title: '04 Rebind hotkey',
    note: 'Hover a hotkey row and it offers Change. Click to listen; it saves on release and refuses combos Windows owns.',
    width: 360,
    height: 500,
    scenarios: [
      { label: 'Hover', state: () => ({ ...home(), ui: { forceHover: 'key-hold' } }) },
      { label: 'Listening', state: () => ({ ...home(), ui: { rebind: { target: 'hold', phase: 'listening', keys: [] } } }) },
      { label: 'Holding', state: () => ({ ...home(), ui: { rebind: { target: 'hold', phase: 'listening', keys: ['Ctrl', 'Shift'] } } }) },
      { label: 'Saved', state: () => ({ ...home({ settings: { ...DEFAULT_SETTINGS, onboardingComplete: true, inputDevice: 'Microphone (Brio 500)' } }), ui: { rebind: { target: 'hold', phase: 'saved', keys: ['Ctrl', 'Shift', 'Space'] } } }) },
      { label: 'Conflict', state: () => ({ ...home(), ui: { rebind: { target: 'hold', phase: 'conflict', keys: ['Win', 'H'], message: 'Used by Windows' } } }) },
    ],
  },
  {
    id: '05',
    title: '05 Microphone',
    note: 'Every input shows a live level. The same list opens from the No microphone pill.',
    width: 360,
    height: 340,
    scenarios: [
      { label: 'Picker', state: () => home({ view: 'microphone', levels: { 'Microphone (Brio 500)': 1 } }) },
      { label: 'Hover', state: () => ({ ...home({ view: 'microphone', levels: { 'Microphone (Brio 500)': 1 } }), ui: { forceHover: 'mic-Microphone Array (Laptop microphone)' } }) },
      {
        label: 'From No mic',
        state: () => ({ ...home({ view: 'microphone', devices: [DEVICES[0], DEVICES[2]] }), ui: { forceHover: 'mic-Microphone Array (Laptop microphone)' } }),
      },
    ],
  },
  {
    id: '06',
    title: '06 Words',
    note: 'Say this, get that. One list for spellings and expansions. Enter saves, Esc cancels.',
    width: 360,
    height: 440,
    scenarios: [
      { label: 'List', state: () => words() },
      { label: 'Add', state: () => words({ words: { mode: 'add', say: 'chirpy', get: '' } }) },
      { label: 'Edit', state: () => words({ words: { mode: 'edit', index: 4, say: 'my sign off', get: 'Best, Pieter' } }) },
      { label: 'Empty', state: () => words(undefined, true) },
    ],
  },
  {
    id: '07',
    title: '07 AI cleanup',
    note: 'Optional. Your own Anthropic key. Without one, Chirp pastes what you said, cleaned up locally.',
    width: 360,
    height: 340,
    scenarios: [
      { label: 'Off', state: () => home({ view: 'ai', settings: { ...DEFAULT_SETTINGS, onboardingComplete: true, aiCleanup: false } }) },
      { label: 'Add key', state: () => home({ view: 'ai', ai: { state: 'none', keyHint: null, fromEnv: false } }) },
      { label: 'Checking', state: () => home({ view: 'ai', ai: { state: 'checking', keyHint: 'sk-ant-••••••••••3fA9', fromEnv: false } }) },
      { label: 'Connected', state: () => home({ view: 'ai' }) },
      { label: 'Invalid', state: () => home({ view: 'ai', ai: { state: 'invalid', keyHint: 'sk-ant-••••••••••x7Qe', fromEnv: false } }) },
      { label: 'Home: key invalid', height: 500, state: () => home({ ai: { state: 'invalid', keyHint: 'sk-ant-••••••••••x7Qe', fromEnv: false } }) },
      { label: 'Home: offline', height: 500, state: () => home({ ai: { state: 'offline', keyHint: 'sk-ant-••••••••••3fA9', fromEnv: false } }) },
    ],
  },
  {
    id: '08',
    title: '08 Recent',
    note: 'The last 20 dictations, kept on this PC. Click one to copy it, or send a misheard word to Words.',
    width: 360,
    height: 480,
    scenarios: [
      { label: 'Hover', state: () => ({ ...home({ view: 'recent' }), ui: { forceHover: `recent-${RECENT_HOVER}` } }) },
      { label: 'Copied', state: () => ({ ...home({ view: 'recent' }), ui: { copied: RECENT_HOVER } }) },
    ],
  },
  {
    id: '09',
    title: '09 Paused',
    note: 'Hotkeys stop listening. The tray icon shows it, and pressing a hotkey says so instead of failing silently.',
    width: 360,
    height: 540,
    scenarios: [
      { label: 'Pause options', state: () => ({ ...home(), ui: { pauseMenu: true, forceHover: 'pause-hour' } }) },
      { label: 'Opened', state: () => home({ pause: { paused: true, until: null } }) },
      { label: 'Hotkey pressed', height: 200, state: () => pill({ kind: 'paused' }) },
    ],
  },
  {
    id: '10',
    title: '10 First run',
    note: 'No wizard. The pill teaches the hotkey once the model is ready, then teaches that it can be clicked.',
    width: 360,
    height: 260,
    scenarios: [
      { label: 'Getting ready', state: () => ({ phase: 'firstrun', firstRun: 'downloading', download: { progress: 18, etaSec: 240 } }) },
      { label: 'Mic blocked', state: () => ({ phase: 'firstrun', firstRun: 'mic-blocked' }) },
      { label: 'Ready', state: () => ({ phase: 'firstrun', firstRun: 'ready' }) },
      { label: 'First words', state: () => pill({ kind: 'listening' }) },
      { label: 'Done', state: () => pill({ kind: 'message', text: "You're set. Click me anytime.", check: true }) },
    ],
  },
  {
    id: '11',
    title: '11 Tray menu and updates',
    note: 'The tray menu is the native Windows menu (see the running app). Updates wait for a restart you choose.',
    width: 360,
    height: 500,
    scenarios: [{ label: 'Update in app', state: () => home({ update: '2.0.1' }) }],
  },
  {
    id: '13',
    title: '13 Keyboard',
    note: 'Focus starts on the first row; arrows move, Enter acts, Tab reaches ×, Esc goes back.',
    width: 360,
    height: 500,
    scenarios: [
      { label: 'Focus on Home', state: () => ({ ...home(), ui: { forceFocus: 'microphone' } }) },
      { label: 'Focus in a drill-in', height: 340, state: () => ({ ...home({ view: 'microphone', levels: { 'Microphone (Brio 500)': 1 } }), ui: { forceFocus: 'mic-Microphone Array (Laptop microphone)' } }) },
    ],
  },
]
