import { useEffect } from 'react'
import { IDLE_BARS, type SurfaceContextValue, type SurfaceState } from './store'
import {
  closeSurface,
  dismissPill,
  later,
  openSurface,
  saveSettings,
  showMessage,
  TIMING,
} from './actions'
import type { AiStatus, AudioDevice, HistoryEntry, PauseState, Settings, SnippetEntry, Theme, VocabEntry } from './types'
import { capLabel, sortCaps } from './format'
import { checkForUpdate, installUpdate } from './updates'

/** Lets the hands-free pill's stop button reach the controller. */
export const dictation = { stop: () => {} }

type Ctx = Pick<SurfaceContextValue, 'store' | 'api'>

const SETTING_KEYS: (keyof Settings)[] = ['holdBinding', 'toggleBinding', 'inputDevice', 'onboardingComplete', 'aiCleanup', 'playSoundOnComplete', 'model']
function pickSettings(raw: Record<string, unknown>, base: Settings): Settings {
  const next = { ...base } as Record<string, unknown>
  for (const k of SETTING_KEYS) if (k in raw) next[k] = raw[k]
  return next as unknown as Settings
}

function systemTheme(): Theme {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

/** 48 RMS slices of the last half second -> 11 bars shaped like the design. */
function barsFrom(slices: number[]): number[] {
  const tail = slices.slice(-22)
  return IDLE_BARS.map((shape, i) => {
    const v = Math.max(tail[i * 2] ?? 0, tail[i * 2 + 1] ?? 0)
    return shape * Math.min(1, v * 1.4)
  })
}

const STOP_ERRORS: [RegExp, string, 'microphone'?][] = [
  [/transcription_failed/, "Didn't catch that"],
  [/mic_disconnected|mic_not_found/, 'No microphone', 'microphone'],
  [/paste_blocked/, 'Copied. Press Ctrl+V'],
  [/injection_failed/, "Couldn't paste"],
  [/model_not_loaded/, 'Still getting ready'],
]

/** The live app: boot, backend events and the dictation flow. Not used by the preview. */
export function useLiveBackend(ctx: Ctx) {
  useEffect(() => {
    const { store, api } = ctx
    const get = () => store.getState()
    const set = (patch: Partial<SurfaceState>) => store.setState(patch)
    const offs: (() => void)[] = []
    let disposed = false

    // ── Dictation ───────────────────────────────────────────────
    let busy = false
    let recording = false
    let pendingStop = false
    let owner: 'hold' | 'toggle' | null = null

    const showPill = (kind: 'listening' | 'handsfree' | 'working') => set({ phase: 'pill', leaving: false, pill: { kind } })
    /** Earcons, when sounds are on. Fire and forget: playback never delays capture or paste. */
    const cue = (name: 'start' | 'done' | 'no-ai') => {
      if (get().settings.playSoundOnComplete) api.call('play_sound', { cue: name }).catch(() => {})
    }

    async function stop() {
      if (!recording) return
      recording = false
      owner = null
      showPill('working')
      try {
        const result = await api.call<{ fallbackReason?: string | null }>('stop_recording')
        cue(result?.fallbackReason ? 'no-ai' : 'done')
        if (!get().settings.onboardingComplete) {
          await saveSettings(ctx, { onboardingComplete: true })
          showMessage(ctx, "You're set. Click me anytime.", { check: true, hold: TIMING.firstRunDoneHold })
        } else if (result?.fallbackReason) {
          showMessage(ctx, `Pasted without AI · ${result.fallbackReason}`)
        } else {
          dismissPill(ctx)
        }
      } catch (e) {
        const hit = STOP_ERRORS.find(([re]) => re.test(String(e)))
        showMessage(ctx, hit ? hit[1] : 'Something went wrong', { action: hit?.[2] })
      }
    }
    dictation.stop = () => {
      if (busy) pendingStop = true
      else stop()
    }

    function startError(e: unknown) {
      recording = false
      owner = null
      const msg = String(e)
      if (/mic_permission/.test(msg)) {
        set({ phase: 'firstrun', firstRun: 'mic-blocked', pill: null })
        watchMicrophone()
      } else if (/mic_not_found/.test(msg)) showMessage(ctx, 'No microphone', { action: 'microphone' })
      else if (/model_not_loaded/.test(msg)) showMessage(ctx, 'Still getting ready')
      else showMessage(ctx, 'Something went wrong')
    }

    async function start(mode: 'hold' | 'toggle') {
      const s = get()
      if (s.phase === 'firstrun' && s.firstRun !== 'ready') return
      if (s.phase === 'surface') {
        // The surface hands focus back to the app before the paste lands.
        if (s.ui.rebind?.phase === 'listening') return
        closeSurface(ctx)
      }
      owner = mode
      pendingStop = false
      busy = true
      set({
        phase: 'pill',
        leaving: false,
        pill: { kind: mode === 'hold' ? 'listening' : 'handsfree' },
        bars: IDLE_BARS.map(() => 0),
        anchor: { to: 'foreground', n: s.anchor.n + 1 },
      })
      try {
        await api.call('start_recording', { hold: mode === 'hold' })
        recording = true
        cue('start')
      } catch (e) {
        busy = false
        return startError(e)
      }
      busy = false
      if (pendingStop) {
        pendingStop = false
        await stop()
      }
    }

    offs.push(
      api.on<string>('hotkey-pressed', (mode) => {
        if (mode === 'toggle' && (recording || busy)) return dictation.stop()
        if (recording || busy) return
        start(mode === 'toggle' ? 'toggle' : 'hold')
      }),
      api.on('hotkey-released', () => {
        if (owner !== 'hold') return
        if (busy) pendingStop = true
        else stop()
      }),
      api.on('dictation-escape', async () => {
        if (!recording && !busy) return
        recording = false
        owner = null
        await api.call('cancel_recording').catch(() => {})
        showMessage(ctx, 'Cancelled')
      }),
      api.on('hotkey-paused', () => {
        if (get().phase === 'surface') return
        set({ phase: 'pill', leaving: false, pill: { kind: 'paused' }, anchor: { to: 'foreground', n: get().anchor.n + 1 } })
        later('pill', 3000, () => dismissPill(ctx))
      }),
      api.on<{ bars: number[] }>('amplitude-data', (d) => {
        if (recording || busy) set({ bars: barsFrom(d.bars) })
      }),
      api.on<string>('recording-state', (st) => {
        if (st === 'processing' && get().pill?.kind !== 'working') showPill('working')
      }),
    )

    // ── Surface and system events ───────────────────────────────
    offs.push(
      api.on<string>('surface-open', (anchor) => {
        if (recording || busy) return
        set({ anchor: { to: anchor === 'cursor' ? 'cursor' : 'primary', n: get().anchor.n + 1 } })
        openSurface(ctx)
      }),
      api.on('surface-blur', () => closeSurface(ctx)),
      api.on<boolean>('surface-hover', (hover) => set({ tickHover: hover })),
      api.on<string[]>('binding-capture-keys', (labels) => {
        const r = get().ui.rebind
        if (r?.phase === 'listening') set({ ui: { ...get().ui, rebind: { ...r, keys: sortCaps(labels.map(capLabel)) } } })
      }),
      api.on<Record<string, unknown>>('settings-changed', (raw) => set({ settings: pickSettings(raw, get().settings) })),
      api.on<HistoryEntry>('history-updated', (entry) => set({ history: [...get().history, entry].slice(-1000) })),
      api.on<AiStatus>('ai-status', (ai) => set({ ai })),
      api.on<PauseState>('pause-changed', (pause) => set({ pause })),
      api.on('update-install', () => installUpdate(api)),
    )

    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const onTheme = () => set({ theme: systemTheme() })
    media.addEventListener('change', onTheme)
    offs.push(() => media.removeEventListener('change', onTheme))

    // ── First run ───────────────────────────────────────────────
    let micTimer = 0
    function watchMicrophone() {
      window.clearTimeout(micTimer)
      micTimer = window.setTimeout(async () => {
        const status = await api.call<string>('probe_microphone').catch(() => 'blocked')
        if (disposed) return
        if (status === 'blocked') return watchMicrophone()
        afterModel()
      }, 2000)
    }

    async function afterModel() {
      if (get().settings.onboardingComplete) return set({ phase: 'tick' })
      const status = await api.call<string>('probe_microphone').catch(() => 'ok')
      if (status === 'blocked') {
        set({ phase: 'firstrun', firstRun: 'mic-blocked' })
        return watchMicrophone()
      }
      set({ phase: 'firstrun', firstRun: 'ready' })
    }

    async function download(model: string) {
      set({ phase: 'firstrun', firstRun: 'downloading', download: { progress: 0, etaSec: null } })
      const started = Date.now()
      const off = api.on<number>('model-download-progress', (p) => {
        const elapsed = (Date.now() - started) / 1000
        set({ download: { progress: p, etaSec: p >= 3 && p < 100 ? (elapsed / p) * (100 - p) : null } })
      })
      try {
        await api.call('download_model', { model })
        off()
        afterModel()
      } catch {
        off()
        set({ download: { progress: 0, etaSec: null } })
        if (!disposed) window.setTimeout(() => download(model), 10_000)
      }
    }

    async function boot() {
      let raw: Record<string, unknown> | null = null
      for (let i = 0; i < 40 && !disposed && !raw; i++) {
        raw = await api.call<Record<string, unknown>>('get_settings').catch(() => null)
        if (!raw) await new Promise((r) => setTimeout(r, 100))
      }
      if (disposed || !raw) return
      set({ settings: pickSettings(raw, get().settings) })
      const [vocabulary, snippets, history, ai, pause, devices] = await Promise.all([
        api.call<VocabEntry[]>('get_vocabulary').catch(() => []),
        api.call<SnippetEntry[]>('get_snippets').catch(() => []),
        api.call<HistoryEntry[]>('get_history').catch(() => []),
        api.call<AiStatus>('get_ai_status').catch(() => get().ai),
        api.call<PauseState>('get_pause_state').catch(() => get().pause),
        api.call<AudioDevice[]>('get_audio_devices').catch(() => []),
      ])
      set({ vocabulary, snippets, history, ai, pause, devices })
      const model = get().settings.model
      const status = await api.call<{ downloaded: boolean }>('get_model_status', { model }).catch(() => ({ downloaded: true }))
      if (!status.downloaded) return download(model)
      afterModel()
    }

    set({ theme: systemTheme() })
    boot()
    if (import.meta.env.PROD) {
      const check = () => checkForUpdate(api, (version) => set({ update: version }))
      later('update-check', 30_000, check)
      const id = window.setInterval(check, 6 * 60 * 60 * 1000)
      offs.push(() => window.clearInterval(id))
    }

    return () => {
      disposed = true
      window.clearTimeout(micTimer)
      offs.forEach((off) => off())
    }
  }, [ctx])
}

