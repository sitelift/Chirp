import type { CSSProperties } from 'react'
import { Check } from 'lucide-react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Header } from '../parts'
import { drill, later, saveSettings } from '../actions'
import type { MediaMode } from '../types'

const MODES: { mode: MediaMode; label: string }[] = [
  { mode: 'off', label: 'Off' },
  { mode: 'lower', label: 'Lower volume' },
  { mode: 'pause', label: 'Pause' },
]

function ModeRow({ mode, label, selected }: { mode: MediaMode; label: string; selected: boolean }) {
  const ctx = useSurfaceContext()
  const forced = useForced(`media-${mode}`)
  return (
    <button
      type="button"
      data-nav=""
      role="radio"
      aria-checked={selected}
      onClick={() => saveSettings(ctx, { mediaWhileDictating: mode })}
      className="nav flex h-[40px] w-full shrink-0 items-center gap-[8px] rounded-row pl-[8px] pr-[14px] text-left hover:bg-hover active:bg-press data-[force-hover]:bg-hover data-[force-press]:bg-press"
      {...forced}
    >
      <span className="flex w-[20px] shrink-0 justify-center text-t1">{selected && <Check size={14} strokeWidth={2.5} />}</span>
      <span className={`min-w-0 flex-1 truncate font-display text-row text-t1 ${selected ? 'font-semibold' : ''}`}>{label}</span>
    </button>
  )
}

/** How much quieter. The store follows the drag; the save waits for it to settle. */
function LowerBy() {
  const ctx = useSurfaceContext()
  const value = useSurface((s) => s.settings.mediaLowerBy)
  const forced = useForced('media-lower-by')
  const set = (v: number) => {
    ctx.store.setState((s) => ({ settings: { ...s.settings, mediaLowerBy: v } }))
    later('media-lower-by', 300, () => saveSettings(ctx, { mediaLowerBy: v }))
  }
  return (
    <div className="flex h-[40px] shrink-0 items-center gap-[12px] pl-[36px] pr-[14px]">
      <span className="shrink-0 text-row text-t2">Quieter by</span>
      <input
        type="range"
        data-nav=""
        min={10}
        max={100}
        step={5}
        value={value}
        aria-label="Quieter by"
        aria-valuetext={`${value} percent`}
        onChange={(e) => set(Number(e.target.value))}
        className="nav slider min-w-0 flex-1"
        style={{ '--pct': `${((value - 10) / 90) * 100}%` } as CSSProperties}
        {...forced}
      />
      <span className="w-[34px] shrink-0 text-right text-row tabular-nums text-t1">{value}%</span>
    </div>
  )
}

export function Settings() {
  const ctx = useSurfaceContext()
  const mode = useSurface((s) => s.settings.mediaWhileDictating)
  return (
    <div className="flex flex-col">
      <Header title="Settings" onBack={() => drill(ctx, 'home')} />
      <div className="flex flex-col gap-[2px] px-[12px] pb-[6px] pt-[4px]">
        <div className="text-row text-t1">Music while dictating</div>
        <div className="text-meta text-t3">Turn other apps down or pause them while you talk.</div>
      </div>
      <div role="radiogroup" aria-label="Music while dictating" className="flex flex-col">
        {MODES.map((m) => (
          <ModeRow key={m.mode} mode={m.mode} label={m.label} selected={mode === m.mode} />
        ))}
      </div>
      {mode === 'lower' && <LowerBy />}
    </div>
  )
}
