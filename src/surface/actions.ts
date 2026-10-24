import { setUi, type SurfaceContextValue, type SurfaceState } from './store'
import type { Binding, BindingTarget, Settings, View } from './types'
import { capLabel } from './format'
import type { WordStores } from './words'

type Ctx = Pick<SurfaceContextValue, 'store' | 'api'>

export const TIMING = {
  textPillHold: 1600,
  pasteFade: 140,
  closeFade: 80,
  closeCollapse: 160,
  copiedHold: 1600,
  savedHold: 1600,
  conflictHold: 1600,
  firstRunDoneHold: 3000,
}

const timers = new Map<string, number>()
/** One named timer at a time; re-arming replaces it. */
export function later(name: string, ms: number, fn: () => void) {
  const old = timers.get(name)
  if (old) window.clearTimeout(old)
  timers.set(
    name,
    window.setTimeout(() => {
      timers.delete(name)
      fn()
    }, ms),
  )
}
export function cancelTimer(name: string) {
  const t = timers.get(name)
  if (t) window.clearTimeout(t)
  timers.delete(name)
}

/** Where the window rests when nothing is happening. */
export function restPhase(s: SurfaceState): SurfaceState['phase'] {
  return s.settings.onboardingComplete ? 'tick' : 'firstrun'
}

export function saveSettings({ store, api }: Ctx, partial: Partial<Settings>) {
  store.setState((s) => ({ settings: { ...s.settings, ...partial } }))
  return api.call('update_settings', { partial })
}

export async function saveWords({ store, api }: Ctx, next: WordStores) {
  const prev = store.getState()
  store.setState({ vocabulary: next.vocabulary, snippets: next.snippets })
  if (next.vocabulary !== prev.vocabulary) await api.call('update_vocabulary', { entries: next.vocabulary })
  if (next.snippets !== prev.snippets) await api.call('update_snippets', { entries: next.snippets })
}

const freshUi = { pauseMenu: false, words: null, rebind: null, aiDraft: '' }

export function openSurface({ store }: Ctx, view: View = 'home') {
  const s = store.getState()
  if (s.phase === 'pill' && s.pill && s.pill.kind !== 'message' && s.pill.kind !== 'paused') return
  cancelTimer('pill')
  cancelTimer('close')
  store.setState((st) => ({ phase: 'surface', view, drill: 1, leaving: false, pill: null, tickHover: false, ui: { ...st.ui, ...freshUi } }))
}

export function closeSurface(ctx: Ctx) {
  const { store, api } = ctx
  if (store.getState().phase !== 'surface') return
  if (store.getState().ui.rebind?.phase === 'listening') api.call('cancel_record_binding').catch(() => {})
  store.setState({ leaving: 'surface' })
  later('close', TIMING.closeFade, () => {
    store.setState((s) => ({ phase: restPhase(s), leaving: false, view: 'home', ui: { ...s.ui, ...freshUi, copied: null } }))
  })
}

export function drill({ store }: Ctx, view: View) {
  store.setState((s) => ({ view, drill: view === 'home' ? -1 : 1, ui: { ...s.ui, words: null, pauseMenu: false } }))
}

/** Esc: the innermost thing first (frame 12). Edits handle their own Esc. */
export function escape(ctx: Ctx) {
  const s = ctx.store.getState()
  if (s.ui.pauseMenu) return setUi(ctx.store, { pauseMenu: false })
  if (s.ui.words) return setUi(ctx.store, { words: null })
  if (s.view !== 'home') return drill(ctx, 'home')
  closeSurface(ctx)
}

export function showMessage({ store }: Ctx, text: string, opts: { action?: 'microphone'; check?: boolean; hold?: number } = {}) {
  store.setState({ phase: 'pill', leaving: false, pill: { kind: 'message', text, action: opts.action, check: opts.check } })
  later('pill', opts.hold ?? TIMING.textPillHold, () => dismissPill({ store }))
}

/** Fade the pill out (140 ms, scale 0.96), then rest. */
export function dismissPill({ store }: Pick<Ctx, 'store'>) {
  if (store.getState().phase !== 'pill') return
  store.setState({ leaving: 'pill' })
  later('pill', TIMING.pasteFade, () => store.setState((s) => ({ phase: restPhase(s), pill: null, leaving: false })))
}

export async function copyText({ store, api }: Ctx, text: string, key: string) {
  await api.call('copy_text', { text })
  setUi(store, { copied: key })
  later('copied', TIMING.copiedHold, () => setUi(store, { copied: null }))
}

function rebindMessage(error: string, target: BindingTarget): string | null {
  if (/cancel|timed out|stopped|interrupted/i.test(error)) return null
  if (/reserved system shortcut/i.test(error)) return 'Used by Windows'
  if (/other binding/i.test(error)) return target === 'hold' ? 'Used by Hands-free' : 'Used by Hold to talk'
  return "Can't use that key"
}

export async function startRebind(ctx: Ctx, target: BindingTarget) {
  const { store, api } = ctx
  cancelTimer('rebind')
  setUi(store, { rebind: { target, phase: 'listening', keys: [] }, pauseMenu: false })
  try {
    const binding = await api.call<Binding>('record_binding', { target })
    await saveSettings(ctx, target === 'hold' ? { holdBinding: binding } : { toggleBinding: binding })
    setUi(store, { rebind: { target, phase: 'saved', keys: binding.label.split(' + ').map(capLabel) } })
    later('rebind', TIMING.savedHold, () => setUi(store, { rebind: null }))
  } catch (e) {
    const current = store.getState().ui.rebind
    if (!current || current.target !== target) return
    const message = rebindMessage(String(e), target)
    if (!message) return setUi(store, { rebind: null })
    setUi(store, { rebind: { target, phase: 'conflict', keys: current.keys, message } })
    // Refused: show why, then listen again. Esc still cancels.
    later('rebind', TIMING.conflictHold, () => {
      if (store.getState().ui.rebind?.phase === 'conflict' && store.getState().phase === 'surface') startRebind(ctx, target)
      else setUi(store, { rebind: null })
    })
  }
}

export async function pauseHotkeys({ store, api }: Ctx, paused: boolean, minutes: number | null = null) {
  const pause = await api.call<SurfaceState['pause']>('pause_hotkeys', { paused, minutes })
  store.setState((s) => ({ pause: pause ?? { paused, until: null }, ui: { ...s.ui, pauseMenu: false } }))
}

export async function setApiKey({ store, api }: Ctx, key: string) {
  const trimmed = key.trim()
  if (!trimmed) return
  store.setState((s) => ({ ai: { ...s.ai, state: 'checking' }, ui: { ...s.ui, aiDraft: '' } }))
  try {
    const status = await api.call<SurfaceState['ai']>('set_api_key', { key: trimmed })
    if (status) store.setState({ ai: status })
  } catch {
    store.setState((s) => ({ ai: { ...s.ai, state: 'invalid' } }))
  }
}

export async function removeApiKey({ store, api }: Ctx) {
  const status = await api.call<SurfaceState['ai']>('remove_api_key')
  store.setState({ ai: status ?? { state: 'none', keyHint: null, fromEnv: false } })
}
