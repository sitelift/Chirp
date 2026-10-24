import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useForced } from './forced'

const rowBase =
  'nav group flex w-full shrink-0 items-center rounded-row text-left text-row text-t1 transition-colors duration-100 ' +
  'enabled:hover:bg-hover enabled:active:bg-press data-[force-hover]:bg-hover data-[force-press]:bg-press disabled:cursor-default'

interface RowProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  id?: string
  /** Right padding: 8 next to a chevron, 14 next to key caps (design). */
  tight?: boolean
  tall?: boolean
}

export function Row({ id, tight = true, tall = false, className = '', children, ...rest }: RowProps) {
  const forced = useForced(id)
  return (
    <button
      type="button"
      data-nav=""
      className={`${rowBase} ${tall ? 'h-[56px]' : 'h-[40px]'} pl-[12px] ${tight ? 'pr-[8px]' : 'pr-[14px]'} justify-between ${className}`}
      {...forced}
      {...rest}
    >
      {children}
    </button>
  )
}

export function Chevron() {
  return <ChevronRight size={16} strokeWidth={2} className="shrink-0 text-chev" />
}

/** "Label ......... value >" */
export function NavRow({ id, label, value, onClick, disabled }: { id: string; label: string; value?: ReactNode; onClick?: () => void; disabled?: boolean }) {
  return (
    <Row id={id} onClick={onClick} disabled={disabled}>
      <span>{label}</span>
      <span className="flex items-center gap-[4px]">
        {value !== undefined && value !== null && <span className="text-row text-t2">{value}</span>}
        <Chevron />
      </span>
    </Row>
  )
}

export function Divider() {
  return (
    <div className="flex flex-col px-[12px] py-[6px]">
      <div className="h-px shrink-0 bg-divider" />
    </div>
  )
}

export type CapTone = 'normal' | 'hover' | 'accent' | 'danger'

export function Caps({ keys, tone = 'normal', hoverable = false }: { keys: string[]; tone?: CapTone; hoverable?: boolean }) {
  const toneStyle =
    tone === 'accent'
      ? { borderColor: 'var(--accent)', background: 'var(--field)' }
      : tone === 'danger'
        ? { borderColor: 'var(--danger)', background: 'var(--field)', color: 'var(--danger)' }
        : tone === 'hover'
          ? { borderColor: 'var(--key-border-hover)', background: 'var(--key-bg-hover)' }
          : undefined
  return (
    <span className="flex gap-[4px]">
      {keys.map((k) => (
        <span key={k} className={`keycap ${hoverable ? 'keycap-hover' : ''}`} style={toneStyle}>
          {k}
        </span>
      ))}
    </span>
  )
}

/** The view header: back, title, optional trailing action. */
export function Header({ title, onBack, action }: { title: string; onBack: () => void; action?: ReactNode }) {
  return (
    <div className="flex h-[40px] shrink-0 items-center gap-[4px]">
      <button
        type="button"
        data-nav=""
        aria-label="Back"
        onClick={onBack}
        className="nav flex h-[32px] w-[32px] shrink-0 items-center justify-center rounded-btn text-t1 hover:bg-hover active:bg-press"
      >
        <ChevronLeft size={16} strokeWidth={2} />
      </button>
      <div className="min-w-0 flex-1 font-display text-row font-semibold text-t1">{title}</div>
      {action}
    </div>
  )
}

export function IconButton({ label, onClick, children, size = 32, className = '', id }: { label: string; onClick: () => void; children: ReactNode; size?: number; className?: string; id?: string }) {
  const forced = useForced(id)
  return (
    <button
      type="button"
      data-nav=""
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
      className={`nav flex shrink-0 items-center justify-center rounded-btn hover:bg-hover active:bg-press ${className}`}
      style={{ width: size, height: size }}
      {...forced}
    >
      {children}
    </button>
  )
}

export type ToggleState = 'off' | 'on' | 'disabled'

/** The switch itself, without a button: for rows that are the button (Sounds). */
export function SwitchTrack({ state }: { state: ToggleState }) {
  const on = state === 'on'
  return (
    <span
      aria-hidden
      className="flex h-[20px] w-[40px] shrink-0 items-center rounded-[10px] px-[3px] transition-colors duration-150"
      style={
        on
          ? { background: 'var(--accent)', justifyContent: 'flex-end' }
          : { border: `1px solid ${state === 'disabled' ? 'var(--toggle-disabled)' : 'var(--toggle-border)'}` }
      }
    >
      <span
        className="block shrink-0 rounded-full"
        style={
          on
            ? { width: 14, height: 14, background: 'var(--toggle-on-thumb)' }
            : { width: 12, height: 12, background: state === 'disabled' ? 'var(--toggle-disabled-thumb)' : 'var(--toggle-thumb)' }
        }
      />
    </span>
  )
}

export function Toggle({ state, onChange, label }: { state: ToggleState; onChange?: (on: boolean) => void; label: string }) {
  const on = state === 'on'
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      data-nav=""
      disabled={state === 'disabled'}
      onClick={() => onChange?.(!on)}
      className="nav flex shrink-0 rounded-[10px]"
    >
      <SwitchTrack state={state} />
    </button>
  )
}

const LEVEL_SHAPE = [4, 9, 13, 7, 3]
const IDLE_SHAPE = [3, 5, 7, 4, 2]

/** Five tiny bars that follow a device's input level. */
export function LevelBars({ level, active }: { level: number; active: boolean }) {
  const l = Math.min(1, Math.max(0, level))
  return (
    <span className="flex h-[14px] w-[18px] shrink-0 items-center gap-[2px]" aria-hidden>
      {LEVEL_SHAPE.map((h, i) => (
        <span
          key={i}
          className="w-[2px] rounded-[1px] transition-[height] duration-75"
          style={{ height: Math.round(IDLE_SHAPE[i] + (h - IDLE_SHAPE[i]) * l), background: active ? 'var(--accent)' : 'var(--level-idle)' }}
        />
      ))}
    </span>
  )
}

export function Spinner() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" className="spin shrink-0" aria-hidden>
      <circle cx="12" cy="12" r="9" fill="none" stroke="var(--spinner-track)" strokeWidth="2.5" />
      <path d="M12 3a9 9 0 0 1 9 9" fill="none" stroke="var(--t1)" strokeWidth="2.5" strokeLinecap="round" />
    </svg>
  )
}

/** The three-bar glyph used on the hover pill. */
export function Glyph({ color = '#F0B723', size = 16 }: { color?: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden>
      <rect x="2.5" y="6" width="2" height="4" rx="1" fill={color} />
      <rect x="7" y="3" width="2" height="10" rx="1" fill={color} />
      <rect x="11.5" y="5" width="2" height="6" rx="1" fill={color} />
    </svg>
  )
}
