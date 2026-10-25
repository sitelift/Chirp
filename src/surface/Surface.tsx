import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from 'react'
import { useSurface, useSurfaceContext } from './store'
import { HoverPill, PillContent } from './Pill'
import { Home } from './views/Home'
import { Microphone } from './views/Microphone'
import { Words } from './views/Words'
import { AiCleanup } from './views/AiCleanup'
import { Recent } from './views/Recent'
import { FirstRun } from './views/FirstRun'
import { Settings } from './views/Settings'
import { dismissPill, escape, openSurface, pauseHotkeys, closeSurface } from './actions'
import { dictation } from './dictation'
import type { View } from './types'

/** Transparent room around the content for shadows; the bottom 12 px is the gap above the taskbar. */
const MARGIN = { top: 40, side: 40, bottom: 12 }
const HOVER_PILL = { width: 56, height: 36 }
const TICK = { width: 28, height: 5 }
/** The tick sits centred in the slot the hover pill fills (frame 01). */
const TICK_LIFT = (HOVER_PILL.height - TICK.height) / 2
/** Longest morph (pill to surface, 240 ms) plus a frame. */
const SHRINK_DELAY = 260

type Mode = 'hidden' | 'tick' | 'passive' | 'interactive' | 'focused'
type Kind = 'none' | 'tick' | 'hover' | 'pill' | 'surface'

const VIEWS: Record<View, () => ReactNode> = {
  home: () => <Home />,
  microphone: () => <Microphone />,
  words: () => <Words />,
  ai: () => <AiCleanup />,
  recent: () => <Recent />,
  settings: () => <Settings />,
}

function Panel() {
  const ctx = useSurfaceContext()
  const view = useSurface((s) => s.view)
  const dir = useSurface((s) => s.drill)
  const ref = useRef<HTMLDivElement>(null)

  // Focus the first row on open and after each drill (frame 13).
  useEffect(() => {
    if (ctx.still) return
    const nav = Array.from(ref.current?.querySelectorAll<HTMLElement>('[data-nav]') ?? [])
    const first = view === 'home' ? nav[0] : (nav.find((el) => el.getAttribute('aria-label') !== 'Back') ?? nav[0])
    // No ring when opened with the mouse; arrows bring it back.
    first?.focus({ preventScroll: true, focusVisible: false } as FocusOptions)
  }, [view, ctx.still])

  const onKeyDown = (e: KeyboardEvent) => {
    // Text fields keep the arrows; a slider takes left/right and leaves up/down to the list.
    const target = e.target as HTMLInputElement
    const inField = target.tagName === 'INPUT' && target.type !== 'range'
    if (e.key === 'Escape') {
      e.preventDefault()
      escape(ctx)
      return
    }
    if (inField || (e.key !== 'ArrowDown' && e.key !== 'ArrowUp')) return
    const nav = Array.from(ref.current?.querySelectorAll<HTMLElement>('[data-nav]:not(:disabled)') ?? [])
    const i = nav.indexOf(document.activeElement as HTMLElement)
    const next = nav[Math.max(0, Math.min(nav.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))]
    if (next) {
      e.preventDefault()
      next.focus()
    }
  }

  return (
    <div ref={ref} className="w-[320px] p-[6px]" onKeyDown={onKeyDown}>
      <div key={view} className={view === 'home' && dir === 1 ? 'content-in' : dir === 1 ? 'drill-in' : 'drill-back'}>
        {VIEWS[view]()}
      </div>
    </div>
  )
}

/** What to draw, how the morph looks, and how the window treats the mouse. */
function useScene(): { kind: Kind; key: string; mode: Mode; content: ReactNode } {
  const ctx = useSurfaceContext()
  const phase = useSurface((s) => s.phase)
  const pill = useSurface((s) => s.pill)
  const hover = useSurface((s) => s.tickHover)
  const step = useSurface((s) => s.firstRun)

  switch (phase) {
    case 'hidden':
      return { kind: 'none', key: 'none', mode: 'hidden', content: null }
    case 'tick':
      return hover
        ? { kind: 'hover', key: 'hover', mode: 'tick', content: <button type="button" aria-label="Open Chirp" onClick={() => openSurface(ctx)} className="block"><HoverPill /></button> }
        : { kind: 'tick', key: 'tick', mode: 'tick', content: null }
    case 'pill': {
      if (!pill) return { kind: 'none', key: 'none', mode: 'hidden', content: null }
      const interactive = pill.kind === 'handsfree' || pill.kind === 'paused' || (pill.kind === 'message' && !!pill.action)
      return {
        kind: 'pill',
        key: pill.kind === 'message' ? `msg-${pill.text}` : pill.kind,
        mode: interactive ? 'interactive' : 'passive',
        content: (
          <PillContent
            pill={pill}
            onStop={() => dictation.stop()}
            onAction={() => openSurface(ctx, 'microphone')}
            onResume={() => {
              pauseHotkeys(ctx, false)
              dismissPill(ctx)
            }}
          />
        ),
      }
    }
    case 'surface':
      return { kind: 'surface', key: `surface`, mode: 'focused', content: <Panel key="panel" /> }
    case 'firstrun':
      return { kind: 'surface', key: `first-${step}`, mode: step === 'mic-blocked' ? 'interactive' : 'passive', content: <FirstRun /> }
  }
}

function morphStyle(kind: Kind, latched: boolean, size: { width: number; height: number }): CSSProperties {
  if (kind === 'tick') return { ...TICK, marginBottom: TICK_LIFT, borderRadius: 3, background: '#1A1917', opacity: 'var(--tick-opacity)' as unknown as number, boxShadow: '0 0 0 1px var(--tick-ring)' }
  if (kind === 'none') return { ...TICK, marginBottom: TICK_LIFT, opacity: 0, borderRadius: 3, background: '#1A1917' }
  const surface = kind === 'surface'
  return {
    ...size,
    borderRadius: 18,
    background: surface ? 'var(--surface)' : '#1A1917',
    boxShadow: surface ? 'var(--surface-shadow)' : latched ? '0 6px 20px #00000059, 0 0 0 2px #F0B723, 0 0 0 6px #F0B72338' : '0 6px 20px #00000073, 0 0 0 1px #FFFFFF1F',
  }
}

/** Measure the content at its natural size; re-measure when it changes. */
function useNaturalSize(key: string) {
  const ref = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const read = () => {
      // Layout size, unaffected by transforms and zoom.
      const r = { width: el.offsetWidth, height: el.offsetHeight }
      setSize((s) => (s.width === r.width && s.height === r.height ? s : r))
    }
    read()
    const ro = new ResizeObserver(read)
    ro.observe(el)
    return () => ro.disconnect()
  }, [key])
  return { ref, size }
}

/**
 * Keep the window just big enough: grow immediately, shrink after the CSS
 * morph has finished, so the animation is never clipped.
 */
function useWindowFit(width: number, height: number, mode: Mode, anchorKey: string) {
  const { api, still } = useSurfaceContext()
  const applied = useRef({ width: 0, height: 0, mode: '' as Mode | '', anchorKey: '' })
  useEffect(() => {
    if (still || !api.live) return
    const W = Math.ceil(width) + MARGIN.side * 2
    const H = Math.ceil(height) + MARGIN.top + MARGIN.bottom
    const a = applied.current
    if (a.width === W && a.height === H && a.mode === mode) return
    const anchor = a.anchorKey !== anchorKey ? anchorKey.split(':')[0] || null : null
    const inset = [MARGIN.top, MARGIN.side, MARGIN.bottom, MARGIN.side]
    const send = (w: number, h: number) => api.call('surface_update', { width: w, height: h, mode, anchor, inset }).catch(() => {})
    const now = { width: Math.max(W, mode === 'hidden' ? W : a.width), height: Math.max(H, mode === 'hidden' ? H : a.height) }
    send(now.width, now.height)
    applied.current = { width: now.width, height: now.height, mode, anchorKey }
    if (now.width === W && now.height === H) return
    const t = window.setTimeout(() => {
      api.call('surface_update', { width: W, height: H, mode, anchor: null, inset }).catch(() => {})
      applied.current = { width: W, height: H, mode, anchorKey }
    }, SHRINK_DELAY)
    return () => window.clearTimeout(t)
  }, [api, still, width, height, mode, anchorKey])
}

export function Stage({ className = 'fixed inset-0' }: { className?: string }) {
  const ctx = useSurfaceContext()
  const theme = useSurface((s) => s.theme)
  const leaving = useSurface((s) => s.leaving)
  const phase = useSurface((s) => s.phase)
  const latched = useSurface((s) => s.pill?.kind === 'handsfree')
  const anchorKey = useSurface((s) => `${s.anchor.to}:${s.anchor.n}`)
  const scene = useScene()
  const { ref, size } = useNaturalSize(scene.key)

  const box = scene.kind === 'tick' || scene.kind === 'none' ? TICK : scene.kind === 'hover' ? HOVER_PILL : size
  const fit = scene.kind === 'tick' || scene.kind === 'hover' ? HOVER_PILL : box
  useWindowFit(fit.width, fit.height, scene.mode, anchorKey)

  return (
    <div
      data-theme={theme}
      className={`${className} flex items-end justify-center`}
      style={{ padding: `${MARGIN.top}px ${MARGIN.side}px ${MARGIN.bottom}px` }}
      onMouseDown={(e) => {
        // Clicking the transparent margin is clicking away.
        if (e.target === e.currentTarget && phase === 'surface') closeSurface(ctx)
      }}
    >
      <div
        className="morph relative shrink-0 overflow-hidden"
        data-kind={scene.kind}
        data-leaving={leaving === 'pill' ? '' : undefined}
        style={morphStyle(scene.kind, latched, box)}
      >
        <div
          ref={ref}
          key={scene.key}
          className={`absolute bottom-0 left-1/2 w-max -translate-x-1/2 ${scene.kind === 'pill' || scene.kind === 'hover' ? '' : 'content-in'}`}
          style={leaving === 'surface' ? { opacity: 0, transition: 'opacity 80ms linear' } : undefined}
        >
          {scene.content}
        </div>
      </div>
    </div>
  )
}
