import type { Api } from '../surface/api'
import { DEFAULT_SETTINGS } from '../surface/store'
import type { AiStatuses, Binding, KeyProvider, Settings } from '../surface/types'
import { AUDIO_DEVICES, HISTORY, SNIPPETS, VOCABULARY } from './data'

/**
 * Dev only: an in-memory backend for the playground (?play=1), so the live
 * controller, open / close / push and every setting run in a plain browser.
 */
export interface MockBackend extends Api {
  emit: (event: string, payload?: unknown) => void
  /** Last `surface_update` arguments, for the hit-rect overlay. */
  lastSurface: { mode: string; inset: number[] } | null
  onSurface: ((s: { mode: string; inset: number[] }) => void) | null
}

const KEY_NAMES: Record<string, string> = { Control: 'Ctrl', Meta: 'Win', OS: 'Win', Alt: 'Alt', Shift: 'Shift', ' ': 'Space', Escape: 'Esc' }
const MODS = ['Control', 'Meta', 'OS', 'Alt', 'Shift']

export function createMockBackend(): MockBackend {
  const handlers = new Map<string, Set<(p: unknown) => void>>()
  let settings: Settings = { ...DEFAULT_SETTINGS, onboardingComplete: true, inputDevice: AUDIO_DEVICES[1].id }
  let vocabulary = VOCABULARY.map((v) => ({ ...v }))
  let snippets = SNIPPETS.map((s) => ({ ...s }))
  const history = HISTORY.map((h) => ({ ...h }))
  let ai: AiStatuses = {
    anthropic: { state: 'connected', keyHint: 'sk-ant-••••••••••4f2a', fromEnv: false },
    openai: { state: 'none', keyHint: null, fromEnv: false },
  }
  let paused = false
  let levelTimer = 0
  let recording: { reject: (e: Error) => void; stop: () => void } | null = null

  const emit = (event: string, payload?: unknown) => handlers.get(event)?.forEach((h) => h(payload))
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms))

  function recordBinding(): Promise<Binding> {
    recording?.reject(new Error('cancelled'))
    return new Promise((resolve, reject) => {
      const onKey = (e: KeyboardEvent) => {
        e.preventDefault()
        e.stopImmediatePropagation()
        const mods: string[] = []
        if (e.ctrlKey) mods.push('Ctrl')
        if (e.shiftKey) mods.push('Shift')
        if (e.altKey) mods.push('Alt')
        if (e.metaKey) mods.push('Win')
        emit('binding-capture-keys', mods)
        if (MODS.includes(e.key)) return
        const k = KEY_NAMES[e.key] ?? (e.key.length === 1 ? e.key.toUpperCase() : e.key)
        stop()
        resolve({ keys: [], label: [...mods, k].join(' + '), suppress: false })
      }
      const stop = () => {
        window.removeEventListener('keydown', onKey, true)
        recording = null
      }
      window.addEventListener('keydown', onKey, true)
      recording = {
        stop,
        reject: (err) => {
          stop()
          reject(err)
        },
      }
    })
  }

  const commands: Record<string, (args: Record<string, unknown>) => unknown> = {
    get_settings: () => settings,
    update_settings: (a) => {
      settings = { ...settings, ...(a.partial as Partial<Settings>) }
      emit('settings-changed', settings)
    },
    get_vocabulary: () => vocabulary,
    update_vocabulary: (a) => {
      vocabulary = a.entries as typeof vocabulary
    },
    get_snippets: () => snippets,
    update_snippets: (a) => {
      snippets = a.entries as typeof snippets
    },
    get_history: () => [...history],
    get_ai_status: () => ai,
    set_api_key: async (a) => {
      await wait(700)
      const p = a.provider as KeyProvider
      const key = String(a.key)
      ai = { ...ai, [p]: { state: key.length > 3 ? 'connected' : 'invalid', keyHint: `${p === 'anthropic' ? 'sk-ant-' : 'sk-'}••••••••••${key.slice(-4)}`, fromEnv: false } }
      return ai
    },
    remove_api_key: (a) => {
      ai = { ...ai, [a.provider as KeyProvider]: { state: 'none', keyHint: null, fromEnv: false } }
      return ai
    },
    get_pause_state: () => ({ paused, until: null }),
    pause_hotkeys: (a) => {
      paused = !!a.paused
      return { paused, until: null }
    },
    get_audio_devices: () => AUDIO_DEVICES,
    start_level_monitor: () => {
      window.clearInterval(levelTimer)
      let t = 0
      levelTimer = window.setInterval(() => {
        t += 1
        const speaking = Math.sin(t / 7) > 0
        emit(
          'device-levels',
          AUDIO_DEVICES.map((d, i) => ({ id: d.id, level: i === 1 && speaking ? 0.4 + Math.random() * 0.6 : i === 2 ? Math.random() * 0.12 : 0 })),
        )
      }, 60)
    },
    stop_level_monitor: () => window.clearInterval(levelTimer),
    get_model_status: () => ({ downloaded: true }),
    probe_microphone: () => 'ok',
    record_binding: () => recordBinding(),
    cancel_record_binding: () => recording?.reject(new Error('cancelled')),
    start_recording: async () => {
      if (paused) throw new Error('paused')
      await wait(40)
    },
    stop_recording: async () => {
      await wait(600)
      const entry = { text: 'Dictated from the playground just now.', timestamp: new Date().toISOString(), wordCount: 6 }
      history.push(entry)
      emit('history-updated', entry)
      return { fallbackReason: null }
    },
    cancel_recording: () => {},
    copy_text: () => {},
    play_sound: () => {},
  }

  const backend: MockBackend = {
    live: false,
    lastSurface: null,
    onSurface: null,
    emit,
    call: async <T,>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'surface_update') {
        backend.lastSurface = { mode: String(args.mode), inset: args.inset as number[] }
        backend.onSurface?.(backend.lastSurface)
        return undefined as T
      }
      const fn = commands[command]
      return (fn ? await fn(args) : undefined) as T
    },
    on: <T,>(event: string, handler: (p: T) => void) => {
      const set = handlers.get(event) ?? new Set()
      set.add(handler as (p: unknown) => void)
      handlers.set(event, set)
      return () => set.delete(handler as (p: unknown) => void)
    },
  }
  return backend
}
