import { useEffect, useMemo, useState } from 'react'
import { Stage } from '../surface/Surface'
import { ENVELOPE } from '../surface/geometry'
import { SurfaceContext, createSurfaceStore, initialState } from '../surface/store'
import { useLiveBackend } from '../surface/dictation'
import { setMotionSpeed } from '../surface/motion'
import { createMockBackend } from './mockBackend'

/**
 * Dev only: the real surface over an in-memory backend, on a fake desktop:
 * http://localhost:5173/?play=1 (add &slow=4 for 4x slower scripted motion,
 * &theme=dark for the dark Windows theme, &hit=1 to outline the hit rect).
 */
export default function Playground() {
  const params = new URLSearchParams(location.search)
  const dark = params.get('theme') === 'dark'
  const showHit = params.has('hit')
  const api = useMemo(() => createMockBackend(), [])
  const value = useMemo(() => ({ store: createSurfaceStore(initialState(dark ? 'dark' : 'light')), api, still: false }), [api, dark])
  const [hit, setHit] = useState<{ mode: string; inset: number[] } | null>(null)

  useEffect(() => {
    setMotionSpeed(Number(params.get('slow')) || 1)
    api.onSurface = setHit
    // The playground stands in for the backend's Windows theme.
    const id = window.setTimeout(() => value.store.setState({ theme: dark ? 'dark' : 'light' }), 0)
    ;(window as unknown as Record<string, unknown>).chirp = { store: value.store, api }
    return () => window.clearTimeout(id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, value])

  useLiveBackend(value)

  const press = (mode: 'hold' | 'toggle') => {
    if (value.store.getState().pause.paused) return api.emit('hotkey-paused')
    api.emit('hotkey-pressed', mode)
    if (mode === 'hold') window.setTimeout(() => api.emit('hotkey-released'), 1400)
  }

  return (
    <div
      className="fixed inset-0 flex flex-col"
      style={{ background: dark ? 'linear-gradient(165deg, #23272e, #121418)' : 'linear-gradient(165deg, #eef1f6, #dde2ea)' }}
      onMouseDown={(e) => {
        // A click on the desktop is a click into another app: the window blurs.
        if (e.target === e.currentTarget || (e.target as HTMLElement).dataset.desktop !== undefined) api.emit('surface-blur')
      }}
    >
      <div className="flex gap-[8px] p-[12px] text-[13px]" data-desktop="">
        <button type="button" className="rounded-[8px] bg-white px-[10px] py-[4px] shadow" onClick={() => press('hold')}>
          Hold to talk (1.4 s)
        </button>
        <button type="button" className="rounded-[8px] bg-white px-[10px] py-[4px] shadow" onClick={() => press('toggle')}>
          Hands-free
        </button>
        <button type="button" className="rounded-[8px] bg-white px-[10px] py-[4px] shadow" onClick={() => api.emit('surface-open', 'primary')}>
          Tray: Open
        </button>
      </div>
      <div className="relative flex-1" data-desktop="">
        <div id="envelope" className="absolute bottom-0 left-1/2 -translate-x-1/2" style={{ width: ENVELOPE.width, height: ENVELOPE.height, outline: showHit ? '1px dashed #0006' : undefined }}>
          <SurfaceContext.Provider value={value}>
            <Stage className="absolute inset-0" />
          </SurfaceContext.Provider>
          {showHit && hit && (
            <div
              className="pointer-events-none absolute"
              style={{ top: hit.inset[0], right: hit.inset[1], bottom: hit.inset[2], left: hit.inset[3], outline: '1px solid #e0347f' }}
              data-mode={hit.mode}
            />
          )}
        </div>
      </div>
      <div className="h-[48px] shrink-0" style={{ background: dark ? '#1c1c1ce6' : '#f3f3f3e6', borderTop: '1px solid #0000000f' }} data-desktop="" />
    </div>
  )
}
