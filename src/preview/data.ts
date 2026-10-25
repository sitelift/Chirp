import type { AudioDevice, HistoryEntry, SnippetEntry, VocabEntry } from '../surface/types'

/** Fixtures for the preview and the playground: the mockup's own copy. */
const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000 - 5_000).toISOString()

export const HISTORY: HistoryEntry[] = [
  { text: 'Lunch at one?', timestamp: ago(24 * 60), wordCount: 3 },
  { text: 'Ship it once the installer check passes on a clean VM.', timestamp: ago(5 * 60), wordCount: 11 },
  { text: 'Remind me to renew the domain before the end of the month.', timestamp: ago(3 * 60), wordCount: 12 },
  { text: 'Parakeet', timestamp: ago(60), wordCount: 1 },
  { text: "Thanks, that works for me. I'll send the deck tonight.", timestamp: ago(18), wordCount: 10 },
  { text: 'Can we move the review to Thursday afternoon? Tuesday is packed.', timestamp: ago(2), wordCount: 11 },
]

export const VOCABULARY: VocabEntry[] = ['Sitelift', 'Pieter de Bruijn', 'Parakeet', 'Keizersgracht', 'Tauri', 'sherpa-onnx'].map((term) => ({ term, replaces: [] }))

export const SNIPPETS: SnippetEntry[] = [
  { trigger: 'my address', expansion: '12 Keizersgracht, Amsterdam' },
  { trigger: 'sig', expansion: 'Best, Pieter' },
  { trigger: 'standup', expansion: 'Yesterday: / Today: / Blockers:' },
  { trigger: 'zoom link', expansion: 'https://zoom.us/j/4815162342' },
]

export const AUDIO_DEVICES: AudioDevice[] = [
  { id: 'default', name: 'Default' },
  { id: 'Microphone (Brio 500)', name: 'Microphone (Brio 500)' },
  { id: 'Headset (WH-1000XM5)', name: 'Headset (WH-1000XM5)' },
  { id: 'Realtek Audio', name: 'Realtek Audio' },
]
