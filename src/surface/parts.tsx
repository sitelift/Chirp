import type { CSSProperties, ReactNode } from 'react'
import { IconBack } from './icons'

/** The sub-screen header: back (8 radius, 10 px from both edges) and the title. */
export function Header({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <div className="head">
      <button type="button" data-nav="" data-back="" aria-label="Back" onClick={onBack} className="nav back">
        <IconBack />
      </button>
      <span className="title">{title}</span>
    </div>
  )
}

export function Divider() {
  return (
    <div className="divider" aria-hidden>
      <i />
    </div>
  )
}

export function Caps({ keys }: { keys: string[] }) {
  return (
    <span className="caps">
      {keys.map((k) => (
        <span key={k} className="cap">
          {k}
        </span>
      ))}
    </span>
  )
}

/** The 40 x 20 switch; the row around it is the button. */
export function Switch() {
  return <span className="toggle" aria-hidden />
}

/** Off / Lower / Pause style picker with a sliding thumb (6 radius in an 8 track). */
export function Segmented<T extends string>({
  options,
  value,
  onChange,
  width,
  label,
}: {
  options: { value: T; label: string }[]
  value: T
  onChange: (v: T) => void
  /** Width of one segment. */
  width: number
  label: string
}) {
  const i = Math.max(0, options.findIndex((o) => o.value === value))
  return (
    <span className="segs" role="group" aria-label={label} style={{ '--w': `${width}px` } as CSSProperties}>
      <span className="thumb" style={{ transform: `translateX(${i * width}px)` }} />
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          aria-pressed={o.value === value}
          data-nav=""
          className="nav"
          onClick={() => o.value !== value && onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </span>
  )
}

/** Grows to its content (grid rows 0fr to 1fr) and fades. */
export function Reveal({ open, children }: { open: boolean; children: ReactNode }) {
  return (
    <div className="reveal" data-open={open ? '' : undefined} inert={!open}>
      <div>{children}</div>
    </div>
  )
}

const LEVEL_SHAPE = [0.55, 0.85, 1, 0.75, 0.5]

/** Five bars that follow a device's input level. */
export function LevelBars({ level }: { level: number }) {
  const l = Math.min(1, Math.max(0, level))
  return (
    <span className="levels" aria-hidden>
      {LEVEL_SHAPE.map((s, i) => (
        <i key={i} style={{ height: Math.round(4 + 10 * s * l) }} />
      ))}
    </span>
  )
}

export function Spinner() {
  return <span className="spin" aria-hidden />
}
