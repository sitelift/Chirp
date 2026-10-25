import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from 'react'
import { themeOf, useSurface, useSurfaceContext, type SurfaceStore } from './store'
import { HoverPill, PillContent } from './Pill'
import { Home } from './views/Home'
import { Recent } from './views/Recent'
import { Words } from './views/Words'
import { Snippets } from './views/Snippets'
import { Settings } from './views/Settings'
import { FirstRun } from './views/FirstRun'
import { closeSurface, dismissPill, escape, openSurface, pauseHotkeys } from './actions'
import { dictation } from './dictation'
import { anim, cancelAll, done, DUR, EASE_CLOSE, PILL_AT, reducedMotion, rowMotion } from './motion'
import type { Phase, View } from './types'
import { ENVELOPE, GAP, PILL, TICK } from './geometry'

/** The backend makes the window click-through outside `inset`, the one interactive thing on screen. */
/** The tick sits on the centre line of the slot the hover pill fills; the panel lands there. */
const TICK_LIFT = (PILL.h - TICK.h) / 2

type Mode = 'hidden' | 'tick' | 'passive' | 'interactive' | 'focused'
type InkKind = 'none' | 'tick' | 'hover' | 'pill'
export interface Size {
  w: number
  h: number
}

const isCard = (p: Phase) => p === 'surface' || p === 'firstrun'
const size = (el: HTMLElement): Size => ({ w: el.offsetWidth, h: el.offsetHeight })

/** Keep focus inside the panel without a ring after a mouse open; arrows bring it back. */
function focusFirst(sheet: HTMLElement | undefined, home: boolean) {
  const nav = Array.from(sheet?.querySelectorAll<HTMLElement>('[data-nav]') ?? [])
  const first = home ? nav[0] : (nav.find((el) => el.hasAttribute('data-back')) ?? nav[0])
  first?.focus({ preventScroll: true, focusVisible: false } as FocusOptions)
}

// ── Window fit ─────────────────────────────────────────────────────────

/** One fixed envelope; only the mode, the hit rect and (on re-anchor) the monitor change. */
function useWindowFit(mode: Mode, hit: Size, anchorKey: string) {
  const { api, still } = useSurfaceContext()
  const applied = useRef({ key: '', anchorKey: '' })
  useEffect(() => {
    if (still) return
    const w = Math.min(ENVELOPE.width, Math.ceil(hit.w))
    const h = Math.min(ENVELOPE.height - GAP, Math.ceil(hit.h))
    const side = (ENVELOPE.width - w) / 2
    const inset = [ENVELOPE.height - GAP - h, side, GAP, side]
    const key = `${mode}:${inset.join(',')}`
    const a = applied.current
    if (a.key === key && a.anchorKey === anchorKey) return
    const anchor = a.anchorKey !== anchorKey ? anchorKey.split(':')[0] || null : null
    applied.current = { key, anchorKey }
    api.call('surface_update', { width: ENVELOPE.width, height: ENVELOPE.height, mode, anchor, inset }).catch(() => {})
  }, [api, still, mode, hit.w, hit.h, anchorKey])
}

/** Measure content at its natural size; re-measure when it changes. */
function useNaturalSize(key: string) {
  const ref = useRef<HTMLDivElement>(null)
  const [natural, setNatural] = useState<Size>({ w: 0, h: 0 })
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const read = () => {
      const r = size(el)
      setNatural((s) => (s.w === r.w && s.h === r.h ? s : r))
    }
    read()
    const ro = new ResizeObserver(read)
    ro.observe(el)
    return () => ro.disconnect()
  }, [key])
  return { ref, natural }
}

/**
 * Open: snapshot the ink (hover pill or a clickable message pill) the moment
 * the store says "surface", before React redraws it, and let the copy fade
 * and swell on top while the panel grows out from under it.
 */
function useInkHandOff(store: SurfaceStore, slot: RefObject<HTMLDivElement | null>, dock: RefObject<HTMLDivElement | null>, origin: RefObject<Size>, still: boolean) {
  useEffect(() => {
    if (still) return
    return store.subscribe((s, prev) => {
      if (!isCard(s.phase) || isCard(prev.phase)) return
      const el = slot.current
      const host = dock.current
      const ink = el?.firstElementChild as HTMLElement | null
      if (!el || !host || !ink) return
      const inkSize = size(ink)
      origin.current = { w: Math.max(PILL.w, inkSize.w), h: PILL.h }
      if (prev.phase === 'hidden' || reducedMotion()) return
      const r = el.getBoundingClientRect()
      const d = host.getBoundingClientRect()
      const ghost = el.cloneNode(true) as HTMLElement
      ghost.removeAttribute('id')
      ghost.setAttribute('aria-hidden', 'true')
      ghost.querySelectorAll('[data-kind]').forEach((n) => {
        const k = n as HTMLElement
        k.style.transition = 'none'
        k.style.width = `${inkSize.w}px`
        k.style.height = `${inkSize.h}px`
      })
      Object.assign(ghost.style, { left: `${r.left - d.left}px`, bottom: `${d.bottom - r.bottom}px`, width: `${r.width}px`, transform: 'none', zIndex: '3', pointerEvents: 'none' })
      host.appendChild(ghost)
      const a = anim(
        ghost,
        [
          { opacity: 1, transform: 'scale(1)' },
          { opacity: 0, transform: 'scale(1.12)' },
        ],
        { duration: DUR.inkFade, easing: 'linear' },
      )
      done(a).then(() => ghost.remove())
    })
  }, [store, slot, dock, origin, still])
}

// ── The panel ──────────────────────────────────────────────────────────

const ORDER: View[] = ['home', 'recent', 'words', 'snippets', 'settings']
const VIEWS: Record<View, () => ReactNode> = {
  home: () => <Home />,
  recent: () => <Recent />,
  words: () => <Words />,
  snippets: () => <Snippets />,
  settings: () => <Settings />,
}

interface CardProps {
  open: boolean
  /** Where the panel goes when it closes: back into the tick, or a quick fade under a dictation pill. */
  exitTo: 'tick' | 'fade'
  slot: RefObject<HTMLDivElement | null>
  origin: RefObject<Size>
  onHit: (s: Size) => void
  onClosed: () => void
}

/**
 * The panel: real width and height animate (never clip-path). Sheets of
 * paper slide edge to edge inside it; the panel follows the active sheet.
 */
function Card({ open, exitTo, slot, origin, onHit, onClosed }: CardProps) {
  const ctx = useSurfaceContext()
  const { still } = ctx
  const phase = useSurface((s) => s.phase)
  const view = useSurface((s) => s.view)

  // What the panel shows stays put while it closes (the phase has moved on by then).
  const [content, setContent] = useState<'panel' | 'firstrun'>(phase === 'firstrun' ? 'firstrun' : 'panel')
  const wanted = phase === 'firstrun' ? 'firstrun' : 'panel'
  if (open && wanted !== content) setContent(wanted)

  // The sheets on show: the current one, plus the one leaving during a push.
  const [pair, setPair] = useState<{ from: View | null; to: View }>({ from: null, to: view })
  const [wasOpen, setWasOpen] = useState(open)
  if (open !== wasOpen) setWasOpen(open)
  if (view !== pair.to) setPair({ from: open && wasOpen && content === 'panel' ? pair.to : null, to: view })

  const cardRef = useRef<HTMLDivElement>(null)
  const clipRef = useRef<HTMLDivElement>(null)
  const sheets = useRef(new Map<string, HTMLElement>())
  /** Bumped by every open, close and push; a stale animation never cleans up over a newer one. */
  const gen = useRef(0)
  const busy = useRef(false)
  const pairRef = useRef(pair)
  useLayoutEffect(() => {
    pairRef.current = pair
  })

  const activeKey = content === 'firstrun' ? 'firstrun' : pair.to
  const sheetEl = useCallback((k: string) => sheets.current.get(k), [])

  // Open and close.
  useLayoutEffect(() => {
    const card = cardRef.current
    const clip = clipRef.current
    const sheet = sheetEl(content === 'firstrun' ? 'firstrun' : pairRef.current.to)
    if (!card || !clip || !sheet) return
    const token = ++gen.current
    const all = Array.from(sheets.current.values())

    if (still) {
      const r = size(sheet)
      Object.assign(card.style, { width: `${r.w}px`, height: `${r.h}px`, visibility: 'visible' })
      return
    }

    if (open) {
      busy.current = true
      cancelAll(card, clip, slot.current, ...all)
      if (pairRef.current.from) setPair({ from: null, to: pairRef.current.to })
      sheet.style.minHeight = ''
      sheet.style.visibility = ''
      const { w, h } = size(sheet)
      Object.assign(card.style, { width: `${w}px`, height: `${h}px`, visibility: 'visible' })
      onHit({ w, h })
      focusFirst(sheet, pairRef.current.to === 'home')
      const finish = (...as: Animation[]) =>
        done(...as).then(() => {
          if (gen.current !== token) return
          as.forEach((a) => a.cancel())
          busy.current = false
        })
      if (reducedMotion()) {
        finish(anim(card, [{ opacity: 0 }, { opacity: 1 }], { duration: DUR.reduced, easing: 'linear' }))
        return
      }
      const o = origin.current ?? PILL
      const grow = anim(card, [{ width: `${o.w}px`, height: `${o.h}px` }, { width: `${w}px`, height: `${h}px` }], { duration: DUR.open })
      const appear = anim(card, [{ opacity: 0 }, { opacity: 1 }], { duration: DUR.cardAppear, easing: 'linear' })
      const rise = anim(sheet, [{ opacity: 0, transform: 'translateY(8px)' }, { opacity: 1, transform: 'translateY(0px)' }], { duration: DUR.content, delay: DUR.contentDelay })
      // Added on top of the rise, so the content stays centred while the panel widens.
      const centre = anim(sheet, [{ transform: `translateX(${(o.w - w) / 2}px)` }, { transform: 'translateX(0px)' }], { duration: DUR.open, composite: 'add' })
      finish(grow, appear, rise, centre)
      return
    }

    // Closing.
    busy.current = true
    const { w, h } = size(card)
    cancelAll(card, clip)
    Object.assign(card.style, { width: `${w}px`, height: `${h}px` })
    const closed = (...as: (Animation | null)[]) =>
      done(...as).then(() => {
        if (gen.current !== token) return
        busy.current = false
        card.style.visibility = 'hidden'
        as.forEach((a) => a?.cancel())
        onClosed()
      })
    if (exitTo === 'fade' || reducedMotion()) {
      closed(anim(card, [{ opacity: 1 }, { opacity: 0 }], { duration: reducedMotion() ? DUR.reduced : DUR.exitFade, easing: 'linear' }))
      return
    }
    const D = DUR.close
    const fades = all.map((s) => anim(s, [{ opacity: 1 }, { opacity: 0 }], { duration: DUR.closeFade, easing: 'linear' }))
    // The content follows the panel's centre while it narrows, so it never slides off to one side.
    const centres = all.map((s) =>
      anim(
        s,
        [{ transform: 'translateX(0px)' }, { transform: `translateX(${(PILL.w - w) / 2}px)`, offset: PILL_AT }, { transform: `translateX(${(TICK.w - w) / 2}px)` }],
        { duration: D, easing: EASE_CLOSE, composite: 'add' },
      ),
    )
    // Solid all the way: the pill's shape at 72%, then the tick, lifted to the tick's centre line.
    const shrink = anim(
      card,
      [
        { width: `${w}px`, height: `${h}px`, transform: 'translateX(-50%) translateY(0px)' },
        { width: `${PILL.w}px`, height: `${PILL.h}px`, transform: 'translateX(-50%) translateY(0px)', offset: PILL_AT },
        { width: `${TICK.w}px`, height: `${TICK.h}px`, transform: `translateX(-50%) translateY(${-TICK_LIFT}px)` },
      ],
      { duration: D, easing: EASE_CLOSE },
    )
    const corners = anim(clip, [{ borderRadius: '18px' }, { borderRadius: '18px', offset: PILL_AT }, { borderRadius: '3px' }], { duration: D, easing: EASE_CLOSE })
    const handOver = anim(card, [{ opacity: 1 }, { opacity: 1, offset: 0.7 }, { opacity: 0 }], { duration: D, easing: 'linear' })
    const tick = slot.current ? anim(slot.current, [{ opacity: 0 }, { opacity: 1 }], { duration: DUR.tickIn, delay: D - DUR.tickIn, easing: 'linear' }) : null
    closed(shrink, corners, handOver, tick, ...fades, ...centres)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- runs on open / close only
  }, [open])

  // Push between screens: both sheets locked edge to edge on one curve, as tall as the taller.
  useLayoutEffect(() => {
    const card = cardRef.current
    if (!pair.from || !card || still) return
    const from = sheetEl(pair.from)
    const next = sheetEl(pair.to)
    if (!from || !next) return
    const token = ++gen.current
    busy.current = true
    const r0 = size(card)
    cancelAll(card, from, next)
    Object.assign(card.style, { width: `${r0.w}px`, height: `${r0.h}px` })
    for (const s of [from, next]) {
      s.style.minHeight = ''
      s.style.visibility = ''
    }
    const r1 = size(next)
    const tall = `${Math.max(r0.h, r1.h)}px`
    from.style.minHeight = tall
    next.style.minHeight = tall
    onHit({ w: Math.max(r0.w, r1.w), h: Math.max(r0.h, r1.h) })
    focusFirst(next, pair.to === 'home')
    const forward = pair.to !== 'home'
    let as: Animation[]
    if (reducedMotion()) {
      Object.assign(card.style, { width: `${r1.w}px`, height: `${r1.h}px` })
      as = [anim(from, [{ opacity: 1 }, { opacity: 0 }], { duration: DUR.reduced, easing: 'linear' }), anim(next, [{ opacity: 0 }, { opacity: 1 }], { duration: DUR.reduced, easing: 'linear' })]
    } else {
      const d = DUR.push
      const resize = anim(card, [{ width: `${r0.w}px`, height: `${r0.h}px` }, { width: `${r1.w}px`, height: `${r1.h}px` }], { duration: d })
      // The detail sheet lies on Home, so its left edge casts a soft shadow.
      const edge = '-12px 0 20px -12px var(--shadow)'
      const flat = '0 0 0 0 transparent'
      const a = forward
        ? anim(from, [{ transform: 'translateX(0px)', boxShadow: flat }, { transform: `translateX(${-r0.w}px)`, boxShadow: flat }], { duration: d })
        : anim(from, [{ transform: 'translateX(0px)', boxShadow: edge }, { transform: `translateX(${r1.w}px)`, boxShadow: edge }], { duration: d })
      const b = forward
        ? anim(next, [{ transform: `translateX(${r0.w}px)`, boxShadow: edge }, { transform: 'translateX(0px)', boxShadow: edge }], { duration: d })
        : anim(next, [{ transform: `translateX(${-r1.w}px)`, boxShadow: flat }, { transform: 'translateX(0px)', boxShadow: flat }], { duration: d })
      as = [resize, a, b]
    }
    done(...as).then(() => {
      if (gen.current !== token) return
      from.style.visibility = 'hidden'
      from.style.minHeight = ''
      next.style.minHeight = ''
      const r = size(next)
      Object.assign(card.style, { width: `${r.w}px`, height: `${r.h}px` })
      as.forEach((x) => x.cancel())
      busy.current = false
      onHit(r)
      setPair((p) => (p.from === pair.from && p.to === pair.to ? { from: null, to: p.to } : p))
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps -- one push per pair
  }, [pair])

  // Between big moves the panel follows the active sheet: frame by frame while rows
  // animate their own height, otherwise easing to the new size.
  useLayoutEffect(() => {
    const card = cardRef.current
    const sheet = sheetEl(activeKey)
    if (!card || !sheet) return
    const ro = new ResizeObserver(() => {
      if (busy.current || !open) return
      const r = size(sheet)
      const c = size(card)
      if (r.w === c.w && r.h === c.h) return
      onHit(r)
      card.getAnimations().forEach((a) => a.cancel())
      Object.assign(card.style, { width: `${r.w}px`, height: `${r.h}px` })
      if (still || rowMotion.moving > 0 || reducedMotion()) return
      const a = anim(card, [{ width: `${c.w}px`, height: `${c.h}px` }, { width: `${r.w}px`, height: `${r.h}px` }], { duration: DUR.resize })
      done(a).then(() => a.playState === 'finished' && a.cancel())
    })
    ro.observe(sheet)
    return () => ro.disconnect()
  }, [activeKey, open, still, sheetEl, onHit])

  const onKeyDown = (e: KeyboardEvent) => {
    const target = e.target as HTMLInputElement
    const inField = target.tagName === 'INPUT' && target.type !== 'range'
    if (e.key === 'Escape') {
      e.preventDefault()
      escape(ctx)
      return
    }
    if (inField || (e.key !== 'ArrowDown' && e.key !== 'ArrowUp')) return
    const sheet = sheetEl(activeKey)
    const nav = Array.from(sheet?.querySelectorAll<HTMLElement>('[data-nav]:not(:disabled)') ?? [])
    const i = nav.indexOf(document.activeElement as HTMLElement)
    const next = nav[Math.max(0, Math.min(nav.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))]
    if (next) {
      e.preventDefault()
      next.focus()
    }
  }

  const ref = (k: string) => (el: HTMLElement | null) => {
    if (el) sheets.current.set(k, el)
    else sheets.current.delete(k)
  }

  return (
    <div ref={cardRef} className="card" style={{ visibility: 'hidden' }} onKeyDown={onKeyDown}>
      <div ref={clipRef} className="clip">
        {content === 'firstrun' ? (
          <div ref={ref('firstrun')} className="sheet bare">
            <FirstRun />
          </div>
        ) : (
          ORDER.filter((v) => v === pair.to || v === pair.from).map((v) => (
            <div key={v} ref={ref(v)} data-view={v} className={v === 'settings' ? 'sheet wide' : 'sheet'} inert={v !== pair.to}>
              {VIEWS[v]()}
            </div>
          ))
        )}
      </div>
    </div>
  )
}

// ── The stage ──────────────────────────────────────────────────────────

function useInk(): { kind: InkKind; key: string; mode: Mode; content: ReactNode; interactive: boolean } {
  const ctx = useSurfaceContext()
  const phase = useSurface((s) => s.phase)
  const pill = useSurface((s) => s.pill)
  const hover = useSurface((s) => s.tickHover)
  const step = useSurface((s) => s.firstRun)

  switch (phase) {
    case 'hidden':
      return { kind: 'none', key: 'tick', mode: 'hidden', content: null, interactive: false }
    case 'tick':
      return { kind: hover ? 'hover' : 'tick', key: 'tick', mode: 'tick', content: <HoverPill on={hover} />, interactive: true }
    case 'pill': {
      if (!pill) return { kind: 'none', key: 'tick', mode: 'hidden', content: null, interactive: false }
      const interactive = pill.kind === 'handsfree' || pill.kind === 'paused' || (pill.kind === 'message' && !!pill.action)
      return {
        kind: 'pill',
        key: pill.kind === 'message' ? `msg-${pill.text}` : pill.kind,
        mode: interactive ? 'interactive' : 'passive',
        interactive,
        content: (
          <PillContent
            pill={pill}
            onStop={() => dictation.stop()}
            onAction={() => openSurface(ctx, 'settings', 'microphone')}
            onResume={() => {
              pauseHotkeys(ctx, false)
              dismissPill(ctx)
            }}
          />
        ),
      }
    }
    case 'surface':
      return { kind: 'tick', key: 'tick', mode: 'focused', content: <HoverPill on={false} />, interactive: false }
    case 'firstrun':
      return { kind: 'none', key: 'tick', mode: step === 'mic-blocked' ? 'interactive' : 'passive', content: null, interactive: false }
  }
}

/** Follow the appearance setting; switching crossfades the whole page (View Transitions, 220 ms). */
function useTheme() {
  const root = useRef<HTMLDivElement>(null)
  const { still } = useSurfaceContext()
  const theme = useSurface(themeOf)
  const shown = useRef<string | null>(null)
  useLayoutEffect(() => {
    const el = root.current
    if (!el) return
    const swap = () => {
      el.dataset.theme = theme
    }
    const first = shown.current === null
    shown.current = theme
    const doc = document as Document & { startViewTransition?: (cb: () => void) => { ready: Promise<void>; finished: Promise<void> } }
    if (first || still || !doc.startViewTransition || reducedMotion()) swap()
    else {
      // A newer switch skips the running crossfade; its promises reject, which is expected.
      const t = doc.startViewTransition(swap)
      t.ready.catch(() => {})
      t.finished.catch(() => {})
    }
  }, [theme, still])
  return root
}

export function Stage({ className = 'fixed inset-0' }: { className?: string }) {
  const ctx = useSurfaceContext()
  const { store, api, still } = ctx
  const phase = useSurface((s) => s.phase)
  const leaving = useSurface((s) => s.leaving)
  const latched = useSurface((s) => s.pill?.kind === 'handsfree')
  const anchorKey = useSurface((s) => `${s.anchor.to}:${s.anchor.n}`)
  const ink = useInk()
  const { ref: inkContent, natural } = useNaturalSize(ink.key)

  const rootRef = useTheme()
  const dockRef = useRef<HTMLDivElement>(null)
  const slotRef = useRef<HTMLDivElement>(null)
  const origin = useRef<Size>(PILL)
  useInkHandOff(store, slotRef, dockRef, origin, still)

  // The panel stays mounted through its close animation.
  const cardWanted = isCard(phase)
  const [cardMounted, setCardMounted] = useState(cardWanted)
  if (cardWanted && !cardMounted) setCardMounted(true)
  const [panel, setPanel] = useState<Size>(PILL)
  const onClosed = useCallback(() => {
    if (!isCard(store.getState().phase)) setCardMounted(false)
  }, [store])
  const onHit = useCallback((s: Size) => setPanel((p) => (p.w === s.w && p.h === s.h ? p : s)), [])

  const pillSize = { w: natural.w, h: PILL.h }
  const hit = cardWanted ? panel : phase === 'pill' ? pillSize : PILL
  useWindowFit(ink.mode, hit, anchorKey)

  const hideInk = cardWanted
  return (
    <div
      ref={rootRef}
      className={className}
      onMouseDown={(e) => {
        // Clicking the transparent margin is clicking away.
        if (e.target === e.currentTarget && phase === 'surface') closeSurface(ctx)
      }}
    >
      <div ref={dockRef} className="absolute left-1/2 h-0 w-0" style={{ bottom: GAP }}>
        <div
          ref={slotRef}
          className="absolute bottom-0 left-0 flex h-[36px] -translate-x-1/2 items-center justify-center"
          style={{ minWidth: PILL.w, visibility: hideInk ? 'hidden' : undefined }}
          onMouseEnter={() => !api.live && store.getState().phase === 'tick' && store.setState({ tickHover: true })}
          onMouseLeave={() => !api.live && store.getState().tickHover && store.setState({ tickHover: false })}
        >
          <div
            className="ink"
            data-kind={ink.kind}
            data-latched={latched ? '' : undefined}
            data-leaving={leaving === 'pill' ? '' : undefined}
            style={ink.kind === 'pill' ? { width: natural.w } : undefined}
            onClick={phase === 'tick' ? () => openSurface(ctx) : undefined}
            role={phase === 'tick' ? 'button' : undefined}
            aria-label={phase === 'tick' ? 'Open Chirp' : undefined}
          >
            <div ref={inkContent} key={ink.key} className="absolute left-1/2 top-1/2 w-max -translate-x-1/2 -translate-y-1/2">
              {ink.content}
            </div>
          </div>
        </div>
        {cardMounted && <Card open={cardWanted} exitTo={phase === 'tick' ? 'tick' : 'fade'} slot={slotRef} origin={origin} onHit={onHit} onClosed={onClosed} />}
      </div>
    </div>
  )
}
