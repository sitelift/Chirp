import { aiStatuses, NO_KEY, setUi, type SurfaceContextValue, type SurfaceState } from './store'
import type { AiStatuses, Binding, BindingTarget, KeyProvider, Section, Settings, SnippetEntry, View, VocabEntry } from './types'
import { capLabel, sortCaps } from './format'

type Ctx = Pick<SurfaceContextValue, 'store' | 'api'>

export const TIMING = {
  textPillHold: 1600,
  pasteFade: 140,
  copiedHold: 1600,
  /** The accent border on freshly saved keys (mockup: 900 ms). */
  savedHold: 900,
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

/** Words are spellings only: `replaces` is written as []. */
export async function saveVocabulary({ store, api }: Ctx, terms: string[]) {
  const entries: VocabEntry[] = terms.map((term) => ({ term, replaces: [] }))
  store.setState({ vocabulary: entries })
  await api.call('update_vocabulary', { entries })
}

export async function saveSnippets({ store, api }: Ctx, entries: SnippetEntry[]) {
  store.setState({ snippets: entries })
  await api.call('update_snippets', { entries })
}

export function openSurface({ store }: Ctx, view: View = 'home', section?: Section) {
  const s = store.getState()
  if (s.phase === 'pill' && s.pill && s.pill.kind !== 'message' && s.pill.kind !== 'paused') return
  cancelTimer('pill')
  store.setState((st) => ({
    phase: 'surface',
    view,
    section: section ?? st.section,
    leaving: false,
    pill: null,
    tickHover: false,
    ui: { ...st.ui, rebind: null, aiDraft: '', copied: null },
  }))
}

/** The panel animates itself back into the tick (Surface.tsx); the window rests at once. */
export function closeSurface(ctx: Ctx) {
  const { store, api } = ctx
  if (store.getState().phase !== 'surface') return
  if (store.getState().ui.rebind?.phase === 'listening') api.call('cancel_record_binding').catch(() => {})
  cancelTimer('rebind')
  store.setState((s) => ({ phase: restPhase(s), tickHover: false, ui: { ...s.ui, rebind: null, aiDraft: '' } }))
}

export function drill(ctx: Ctx, view: View) {
  const { store } = ctx
  if (store.getState().ui.rebind?.phase === 'listening') cancelRebind(ctx)
  store.setState({ view })
}

export function showSection(ctx: Ctx, section: Section) {
  if (ctx.store.getState().ui.rebind?.phase === 'listening') cancelRebind(ctx)
  ctx.store.setState({ section })
}

/** Esc: back to Home, then close. While a shortcut is being recorded Esc belongs to it. */
export function escape(ctx: Ctx) {
  const s = ctx.store.getState()
  if (s.ui.rebind?.phase === 'listening') return
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

function rebindMessage(error: string): string | null {
  if (/cancel|timed out|stopped|interrupted/i.test(error)) return null
  if (/reserved system shortcut/i.test(error)) return 'Used by Windows'
  if (/already assigned|other (binding|shortcut)/i.test(error)) return 'Used by another shortcut'
  return "Can't use that key"
}

const BINDING_FIELD: Record<BindingTarget, 'holdBinding' | 'toggleBinding' | 'cancelBinding'> = {
  hold: 'holdBinding',
  toggle: 'toggleBinding',
  cancel: 'cancelBinding',
}

export async function startRebind(ctx: Ctx, target: BindingTarget) {
  const { store, api } = ctx
  const current = store.getState().ui.rebind
  if (current?.phase === 'listening') {
    if (current.target === target) return
    await api.call('cancel_record_binding').catch(() => {})
  }
  cancelTimer('rebind')
  setUi(store, { rebind: { target, phase: 'listening', keys: [] } })
  try {
    const binding = await api.call<Binding>('record_binding', { target })
    if (store.getState().ui.rebind?.target !== target) return
    await saveSettings(ctx, { [BINDING_FIELD[target]]: binding })
    setUi(store, { rebind: { target, phase: 'saved', keys: sortCaps(binding.label.split(' + ').map(capLabel)) } })
    later('rebind', TIMING.savedHold, () => setUi(store, { rebind: null }))
  } catch (e) {
    const now = store.getState().ui.rebind
    if (!now || now.target !== target || now.phase !== 'listening') return
    const message = rebindMessage(String(e))
    if (!message) return setUi(store, { rebind: null })
    setUi(store, { rebind: { target, phase: 'conflict', keys: now.keys, message } })
    later('rebind', TIMING.conflictHold, () => {
      if (store.getState().ui.rebind?.phase === 'conflict') setUi(store, { rebind: null })
    })
  }
}

/** Stop listening and keep the old keys (click elsewhere, leave the section). */
export function cancelRebind({ store, api }: Ctx) {
  if (store.getState().ui.rebind?.phase !== 'listening') return
  setUi(store, { rebind: null })
  api.call('cancel_record_binding').catch(() => {})
}

export async function pauseHotkeys({ store, api }: Ctx, paused: boolean, minutes: number | null = null) {
  const pause = await api.call<SurfaceState['pause']>('pause_hotkeys', { paused, minutes })
  store.setState({ pause: pause ?? { paused, until: null } })
}

export async function setApiKey({ store, api }: Ctx, provider: KeyProvider, key: string) {
  const trimmed = key.trim()
  if (!trimmed) return
  store.setState((s) => ({ ai: { ...s.ai, [provider]: { ...s.ai[provider], state: 'checking' } }, ui: { ...s.ui, aiDraft: '' } }))
  try {
    const status = await api.call<AiStatuses>('set_api_key', { provider, key: trimmed })
    store.setState((s) => ({ ai: aiStatuses(status, s.ai) }))
  } catch {
    store.setState((s) => ({ ai: { ...s.ai, [provider]: { ...s.ai[provider], state: 'invalid' } } }))
  }
}

export async function removeApiKey({ store, api }: Ctx, provider: KeyProvider) {
  const status = await api.call<AiStatuses>('remove_api_key', { provider })
  store.setState((s) => ({ ai: status ? aiStatuses(status, s.ai) : { ...s.ai, [provider]: NO_KEY } }))
}
