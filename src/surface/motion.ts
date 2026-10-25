/** Motion numbers from docs/v2/design/SPEC.md (and the mockup they came from). */
export const EASE = 'cubic-bezier(0.32, 0.72, 0, 1)'
export const EASE_CLOSE = 'cubic-bezier(0.4, 0, 0.2, 1)'
/** Big moves (open, push) run 10% slower than reactions. */
const PACE = 1.1

export const DUR = {
  open: 300 * PACE,
  push: 300 * PACE,
  close: 300,
  /** Open: content rises 8 px and fades in. */
  content: 220 * PACE,
  contentDelay: 60,
  inkFade: 90,
  cardAppear: 110,
  /** Close: the sheet fades in 70 ms; the tick fades in over the last 90. */
  closeFade: 70,
  tickIn: 90,
  /** Card gone to a dictation pill (hotkey pressed while open). */
  exitFade: 120,
  paneOut: 120,
  paneIn: 240,
  rowIn: 260,
  rowOut: 220,
  /** Content height changes that are not row animations. */
  resize: 220,
  /** Reduced motion: everything is a short fade. */
  reduced: 150,
}

/** Close keyframe at which the panel is pill-sized. */
export const PILL_AT = 0.72

/** Dev only: `?slow=4` in the playground runs every scripted animation 4x slower. */
let speed = 1
export function setMotionSpeed(n: number) {
  speed = n > 0 ? n : 1
}

export function reducedMotion(): boolean {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

export function anim(el: Element, frames: Keyframe[], opts: KeyframeAnimationOptions & { duration: number }): Animation {
  return el.animate(frames, {
    fill: 'both',
    easing: EASE,
    ...opts,
    duration: opts.duration * speed,
    delay: ((opts.delay as number | undefined) ?? 0) * speed,
  })
}

export const done = (...as: (Animation | null | undefined)[]) => Promise.all(as.map((a) => a?.finished.catch(() => {})))

export function cancelAll(...els: (Element | null | undefined)[]) {
  for (const el of els) el?.getAnimations().forEach((a) => a.cancel())
}

/**
 * Rows animating their own height (Words, Snippets). While any are moving the
 * panel follows the sheet frame by frame instead of easing to the new size.
 */
export const rowMotion = { moving: 0 }

/** Height 0 to the row's height with a fade (add) or back (remove). */
export function animateRow(el: HTMLElement, dir: 'in' | 'out'): Promise<unknown> {
  if (reducedMotion()) {
    return dir === 'in' ? Promise.resolve() : done(anim(el, [{ opacity: 1 }, { opacity: 0 }], { duration: DUR.reduced, easing: 'linear' }))
  }
  // Measure the row's own height, not a frame of an animation already on it.
  el.getAnimations().forEach((a) => a.cancel())
  const h = el.offsetHeight
  const frames = [
    { height: '0px', opacity: 0 },
    { height: `${h}px`, opacity: 1 },
  ]
  rowMotion.moving++
  const a = anim(el, dir === 'in' ? frames : frames.reverse(), { duration: dir === 'in' ? DUR.rowIn : DUR.rowOut })
  return done(a).then(() => {
    rowMotion.moving--
    if (dir === 'in') a.cancel()
  })
}
