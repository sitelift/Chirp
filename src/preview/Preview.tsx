import { useEffect, useMemo } from 'react'
import { Stage } from '../surface/Surface'
import { SurfaceContext, createSurfaceStore, initialState, type SurfaceState } from '../surface/store'
import { mockApi } from '../surface/api'
import type { Theme } from '../surface/types'
import { SECTIONS, type Scenario } from './scenarios'

/**
 * Dev-only gallery of every surface state in both themes, laid out like the
 * Paper artboards for side-by-side review: http://localhost:5173/?preview=1
 * Add `&section=03` to show one artboard.
 */
const SCREEN: Record<Theme, { background: string; taskbar: string; border: string }> = {
  light: { background: 'linear-gradient(in oklab 165deg, #E9EEF3 0%, #D2DBE4 100%)', taskbar: '#F3F3F3', border: '#00000012' },
  dark: { background: 'linear-gradient(in oklab 165deg, #2B3038 0%, #15171B 100%)', taskbar: '#1C1C1C', border: '#FFFFFF0F' },
}

function Taskbar({ theme }: { theme: Theme }) {
  const s = SCREEN[theme]
  const tile = theme === 'light' ? '#C9CFD6' : '#3A3D42'
  const win = theme === 'light' ? '#0067C0' : '#4CC2FF'
  const fg = theme === 'light' ? '#1A1A1A' : '#FFFFFF'
  return (
    <div className="flex h-[48px] shrink-0 items-center px-[12px]" style={{ background: s.taskbar, borderTop: `1px solid ${s.border}` }}>
      <div className="flex-1" />
      <div className="flex items-center gap-[8px]">
        <svg width="18" height="18" viewBox="0 0 18 18">
          {[0, 9.6].flatMap((x) => [0, 9.6].map((y) => <rect key={`${x}-${y}`} x={x} y={y} width="8.4" height="8.4" rx="1" fill={win} />))}
        </svg>
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-[20px] w-[20px] rounded-[5px]" style={{ background: tile }} />
        ))}
      </div>
      <div className="flex flex-1 items-center justify-end gap-[6px] text-[12px]" style={{ color: fg }}>
        <svg width="16" height="16" viewBox="0 0 16 16">
          <rect x="2.5" y="6" width="2" height="4" rx="1" fill={fg} />
          <rect x="7" y="3" width="2" height="10" rx="1" fill={fg} />
          <rect x="11.5" y="5" width="2" height="6" rx="1" fill={fg} />
        </svg>
        14:32
      </div>
    </div>
  )
}

function Frame({ scenario, theme, width, height }: { scenario: Scenario; theme: Theme; width: number; height: number }) {
  const value = useMemo(() => {
    const base = initialState(theme)
    const patch = scenario.state(theme)
    const state: SurfaceState = { ...base, ...patch, ui: { ...base.ui, ...(patch.ui ?? {}) } }
    return { store: createSurfaceStore(state), api: mockApi, still: true }
  }, [scenario, theme])
  return (
    <div className="flex shrink-0 flex-col gap-[10px]" style={{ width }}>
      <div className="font-display text-[13px] font-semibold leading-[18px] text-[#0E0D0B]">{scenario.label}</div>
      <div className="flex flex-col overflow-clip rounded-[10px]" style={{ background: SCREEN[theme].background, height }}>
        <div className="relative min-h-0 flex-1">
          <SurfaceContext.Provider value={value}>
            <Stage className="absolute inset-0" />
          </SurfaceContext.Provider>
        </div>
        <Taskbar theme={theme} />
      </div>
    </div>
  )
}

export default function Preview() {
  const only = new URLSearchParams(location.search).get('section')
  useEffect(() => {
    document.body.style.overflow = 'auto'
    document.body.style.background = '#F4F4F3'
  }, [])
  return (
    <div className="flex flex-col gap-[64px] p-[48px]">
      {SECTIONS.filter((s) => !only || s.id === only).map((section) => (
        <section key={section.id} id={`s${section.id}`} className="flex flex-col gap-[24px]">
          <div className="flex flex-col gap-[6px]">
            <h1 className="font-display text-[28px] font-bold leading-[34px] tracking-[-0.02em] text-[#0E0D0B]">{section.title}</h1>
            <p className="text-[14px] text-[#6B6B6B]">{section.note}</p>
          </div>
          {(['light', 'dark'] as const).map((theme) => (
            <div key={theme} className="flex flex-col gap-[12px]">
              <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[#8A8A8A]">{theme}</div>
              <div className="flex flex-wrap items-end gap-[24px]">
                {section.scenarios.map((sc) => (
                  <Frame key={sc.label} scenario={sc} theme={theme} width={section.width} height={sc.height ?? section.height} />
                ))}
              </div>
            </div>
          ))}
        </section>
      ))}
    </div>
  )
}
