import { useEffect, useRef } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Caps } from '../parts'
import { bindingCaps } from '../format'
import { cancelRebind, startRebind } from '../actions'
import type { BindingTarget } from '../types'

const ROWS: { target: BindingTarget; label: string; hint: string }[] = [
  { target: 'hold', label: 'Hold to talk', hint: 'Hold, speak, let go.' },
  { target: 'toggle', label: 'Hands-free', hint: 'Tap on, tap off.' },
  { target: 'cancel', label: 'Cancel', hint: 'Stops dictating and throws it away.' },
]

/**
 * The keys are the button: click, then press the new combination. Clicking
 * anywhere else stops listening and keeps the old keys. (Esc is a key you can
 * record for Cancel, so only the mouse aborts that one; the other two also
 * stop on Esc in the backend.)
 */
function Bind({ target, label }: { target: BindingTarget; label: string }) {
  const ctx = useSurfaceContext()
  const field = target === 'hold' ? 'holdBinding' : target === 'toggle' ? 'toggleBinding' : 'cancelBinding'
  const binding = useSurface((s) => s.settings[field])
  const rebind = useSurface((s) => (s.ui.rebind?.target === target ? s.ui.rebind : null))
  const forced = useForced(`bind-${target}`)
  const ref = useRef<HTMLButtonElement>(null)
  const listening = rebind?.phase === 'listening'

  useEffect(() => {
    if (!listening || ctx.still) return
    const away = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) cancelRebind(ctx)
    }
    document.addEventListener('mousedown', away, true)
    return () => document.removeEventListener('mousedown', away, true)
  }, [listening, ctx])

  const caps = rebind && rebind.phase !== 'listening' ? rebind.keys : bindingCaps(binding)
  let body
  if (listening) body = rebind.keys.length ? <Caps keys={rebind.keys} /> : <span className="press">Press keys</span>
  else body = caps.length ? <Caps keys={caps} /> : <span className="press">Not set</span>

  return (
    <button
      ref={ref}
      type="button"
      data-nav=""
      className="nav bind"
      data-state={rebind?.phase}
      aria-label={`${label}: ${caps.join(' + ') || 'not set'}. Click to change`}
      onClick={() => !listening && startRebind(ctx, target)}
      {...forced}
    >
      {body}
    </button>
  )
}

export function Shortcuts() {
  const rebind = useSurface((s) => s.ui.rebind)
  return (
    <>
      <h2>Shortcuts</h2>
      <p className="lede">Click a shortcut to change it.</p>
      <div>
        {ROWS.map((r) => {
          const conflict = rebind?.target === r.target && rebind.phase === 'conflict'
          return (
            <div key={r.target} className="item">
              <div className="txt">
                <b>{r.label}</b>
                <small style={conflict ? { color: 'var(--danger)' } : undefined} aria-live="polite">
                  {conflict ? rebind.message : r.hint}
                </small>
              </div>
              <Bind target={r.target} label={r.label} />
            </div>
          )
        })}
      </div>
    </>
  )
}
