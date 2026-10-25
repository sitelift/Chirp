import { Check, ChevronRight } from 'lucide-react'
import { useSurface, useSurfaceContext } from './store'
import chirpMark from '../../assets/brand/chirp-mark-primary.svg?no-inline'
import type { Pill as PillModel } from './types'

const WORKING_OPACITY = [0.25, 0.25, 0.25, 0.45, 0.7, 1, 0.7, 0.45, 0.25, 0.25, 0.25]

/** 11 amber bars; heights follow the mic (60 fps, 80 ms smoothing). */
function Bars() {
  const bars = useSurface((s) => s.bars)
  return (
    <span className="flex items-center gap-[3px]" aria-hidden>
      {bars.map((v, i) => (
        <span
          key={i}
          className="w-[3px] shrink-0 rounded-[2px] bg-amber transition-[height] duration-[80ms] ease-out"
          style={{ height: Math.round(3 + Math.min(1, Math.max(0, v)) * 15), opacity: i === 0 || i === bars.length - 1 ? 0.5 : 1 }}
        />
      ))}
    </span>
  )
}

function Dots() {
  const { still } = useSurfaceContext()
  return (
    <span className="flex items-center gap-[3px]" aria-hidden>
      {WORKING_OPACITY.map((o, i) => (
        <span
          key={i}
          className={`h-[3px] w-[3px] shrink-0 rounded-[2px] bg-amber ${still ? '' : 'sweep'}`}
          style={still ? { opacity: o } : { animationDelay: `${(i - 5) * (900 / 11) - 900}ms` }}
        />
      ))}
    </span>
  )
}

const text = 'whitespace-nowrap font-display text-row font-semibold text-white'

export function PillContent({ pill, onStop, onAction, onResume }: { pill: PillModel; onStop?: () => void; onAction?: () => void; onResume?: () => void }) {
  switch (pill.kind) {
    case 'listening':
      return (
        <div className="flex h-[36px] w-[132px] items-center justify-center">
          <Bars />
        </div>
      )
    case 'working':
      return (
        <div className="flex h-[36px] w-[132px] items-center justify-center" aria-label="Working">
          <Dots />
        </div>
      )
    case 'handsfree':
      return (
        <div className="flex h-[36px] items-center gap-[12px] pl-[18px] pr-[6px]">
          <Bars />
          <button
            type="button"
            aria-label="Stop"
            onClick={onStop}
            className="flex h-[24px] w-[24px] shrink-0 items-center justify-center rounded-[12px] bg-[#FFFFFF1F] hover:bg-[#FFFFFF33]"
          >
            <span className="h-[8px] w-[8px] rounded-[2px] bg-white" />
          </button>
        </div>
      )
    case 'paused':
      return (
        <div className="flex h-[36px] items-center gap-[10px] pl-[14px] pr-[6px]">
          <span className="flex items-center gap-[6px]">
            <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden>
              <rect x="3.5" y="2.5" width="3" height="11" rx="1" fill="#A3A29E" />
              <rect x="9.5" y="2.5" width="3" height="11" rx="1" fill="#A3A29E" />
            </svg>
            <span className={text}>Paused</span>
          </span>
          <button type="button" onClick={onResume} className="flex h-[24px] items-center rounded-[12px] bg-[#FFFFFF1F] px-[10px] text-meta font-semibold text-white hover:bg-[#FFFFFF33]">
            Resume
          </button>
        </div>
      )
    case 'message':
      if (pill.action) {
        return (
          <button type="button" onClick={onAction} className="flex h-[36px] items-center justify-center gap-[4px] pl-[16px] pr-[11px]">
            <span className={text}>{pill.text}</span>
            <ChevronRight size={14} strokeWidth={2.2} color="#A3A29E" />
          </button>
        )
      }
      if (pill.check) {
        return (
          <div className="flex h-[36px] items-center gap-[6px] pl-[13px] pr-[16px]">
            <Check size={14} strokeWidth={2.6} color="#F0B723" />
            <span className={text}>{pill.text}</span>
          </div>
        )
      }
      return (
        <div className="flex h-[36px] items-center justify-center px-[16px]">
          <span className={text}>{pill.text}</span>
        </div>
      )
  }
}

/** The pill the resting tick turns into under the pointer. */
export function HoverPill() {
  return (
    <div className="flex h-[36px] w-[56px] items-center justify-center">
      <img src={chirpMark} width={20} height={20} alt="" draggable={false} />
    </div>
  )
}
