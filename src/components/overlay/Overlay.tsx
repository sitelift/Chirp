import { useEffect, useState, useRef, useCallback } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { availableMonitors, primaryMonitor } from '@tauri-apps/api/window'
import type { Monitor } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi'
import { useAppStore } from '../../stores/appStore'
import { useAudio } from '../../hooks/useAudio'
import { useRecording } from '../../hooks/useRecording'
import { useOverlaySync } from '../../hooks/useOverlaySync'
import { BirdMark } from '../shared/BirdMark'
import { Listening } from './Listening'
import { Processing } from './Processing'
import { Polishing } from './Polishing'
import { Done } from './Done'
import { Error } from './Error'

const WIN_W = 300
const WIN_H = 64
const OFFSET = 80 // px from screen edge for default/snap positions

// Snap distance in pixels — pill snaps to nearest grid point within this radius
const SNAP_RADIUS = 40

// 3x3 snap grid as fractions of monitor work area
const SNAP_GRID = [
  { fx: 0.1, fy: 0, label: 'Top left' },
  { fx: 0.5, fy: 0, label: 'Top center' },
  { fx: 0.9, fy: 0, label: 'Top right' },
  { fx: 0.1, fy: 0.5, label: 'Middle left' },
  { fx: 0.5, fy: 0.5, label: 'Center' },
  { fx: 0.9, fy: 0.5, label: 'Middle right' },
  { fx: 0.1, fy: 1, label: 'Bottom left' },
  { fx: 0.5, fy: 1, label: 'Bottom center' },
  { fx: 0.9, fy: 1, label: 'Bottom right' },
] as const

/** Convert a monitor to logical bounds */
function monitorLogicalBounds(m: Monitor) {
  const sf = m.scaleFactor
  return {
    x: m.position.x / sf,
    y: m.position.y / sf,
    w: m.size.width / sf,
    h: m.size.height / sf,
  }
}

/** Compute snap point positions for a monitor (absolute logical coords) */
function monitorSnapPoints(m: Monitor) {
  const b = monitorLogicalBounds(m)
  return SNAP_GRID.map((s) => ({
    // Horizontal: fx maps across (b.w - WIN_W), offset from left
    x: b.x + (b.w - WIN_W) * s.fx,
    // Vertical: fy 0 = OFFSET from top, fy 1 = OFFSET from bottom, fy 0.5 = center
    y: b.y + OFFSET + (b.h - WIN_H - OFFSET * 2) * s.fy,
    label: s.label,
  }))
}

/** Calculate default position (bottom-center of primary monitor) */
function defaultPosition(primary: Monitor): { x: number; y: number } {
  const b = monitorLogicalBounds(primary)
  return {
    x: Math.round(b.x + (b.w - WIN_W) / 2),
    y: Math.round(b.y + b.h - OFFSET - WIN_H),
  }
}

/**
 * Migrate old formats and validate position is on a visible monitor.
 * Returns absolute logical pixel coordinates.
 */
async function resolvePosition(pos: unknown): Promise<{ x: number; y: number }> {
  const monitors = await availableMonitors()
  const primary = await primaryMonitor()
  const fallback = primary ? defaultPosition(primary) : { x: 100, y: 100 }

  // Migrate old string positions
  if (typeof pos === 'string') {
    if (!primary) return fallback
    const b = monitorLogicalBounds(primary)
    if (pos === 'top') {
      return { x: Math.round(b.x + (b.w - WIN_W) / 2), y: Math.round(b.y + OFFSET) }
    }
    return fallback // 'bottom' → default
  }

  // Validate { x, y } object
  if (pos && typeof pos === 'object' && 'x' in pos && 'y' in pos) {
    const { x, y } = pos as { x: number; y: number }
    // Check if position is on any monitor
    const pillCX = x + WIN_W / 2
    const pillCY = y + WIN_H / 2
    for (const m of monitors) {
      const b = monitorLogicalBounds(m)
      if (pillCX >= b.x && pillCX < b.x + b.w && pillCY >= b.y && pillCY < b.y + b.h) {
        return { x, y } // on a valid monitor
      }
    }
    // Position is off-screen (monitor disconnected?) — fall back
    return fallback
  }

  return fallback
}

/** Virtual screen bounding box from all monitors */
interface VirtualScreen {
  left: number
  top: number
  width: number
  height: number
  monitors: Monitor[]
}

function computeVirtualScreen(monitors: Monitor[]): VirtualScreen {
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity
  for (const m of monitors) {
    const b = monitorLogicalBounds(m)
    left = Math.min(left, b.x)
    top = Math.min(top, b.y)
    right = Math.max(right, b.x + b.w)
    bottom = Math.max(bottom, b.y + b.h)
  }
  return { left, top, width: right - left, height: bottom - top, monitors }
}

export function Overlay() {
  const status = useAppStore((s) => s.status)
  const autoDismiss = useAppStore((s) => s.autoDismissOverlay)
  const setStatus = useAppStore((s) => s.setStatus)
  const position = useAppStore((s) => s.overlayPosition)
  const showPassive = useAppStore((s) => s.showPassiveOverlay)
  const hotkeyStatus = useAppStore((s) => s.hotkeyStatus)
  const updateSettings = useAppStore((s) => s.updateSettings)
  const [dismissing, setDismissing] = useState(false)

  // Reposition mode state
  const [repositioning, setRepositioning] = useState(false)
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null)
  const [dropped, setDropped] = useState(false)
  const [dragging, setDragging] = useState(false)
  const [virtualScreen, setVirtualScreen] = useState<VirtualScreen | null>(null)
  const [snapPoints, setSnapPoints] = useState<Array<{ x: number; y: number; label: string }>>([])
  const draggingRef = useRef(false)
  const dragStartRef = useRef<{ mouseX: number; mouseY: number; pillX: number; pillY: number } | null>(null)
  const originalPositionRef = useRef<{ x: number; y: number } | null>(null)

  useOverlaySync()
  useAudio()
  useRecording()

  const isActive = status !== 'idle'
  const shouldShow = isActive || showPassive

  // Listen for reposition mode event from settings window
  useEffect(() => {
    const unlisten = listen('enter-reposition-mode', async () => {
      const win = getCurrentWindow()
      const monitors = await availableMonitors()
      if (monitors.length === 0) return

      const vs = computeVirtualScreen(monitors)
      const resolved = await resolvePosition(position)
      originalPositionRef.current = resolved

      // Compute snap points for all monitors
      const allSnaps: Array<{ x: number; y: number; label: string }> = []
      for (const m of monitors) {
        allSnaps.push(...monitorSnapPoints(m))
      }
      setSnapPoints(allSnaps)

      // dragPos is relative to the virtual screen origin (top-left of backdrop)
      setDragPos({ x: resolved.x - vs.left, y: resolved.y - vs.top })
      setVirtualScreen(vs)
      setDropped(false)
      setRepositioning(true)

      // Expand window to cover entire virtual screen
      await win.setIgnoreCursorEvents(false)
      await win.setSize(new LogicalSize(vs.width, vs.height))
      await win.setPosition(new LogicalPosition(vs.left, vs.top))
      await win.show()
    })

    return () => { unlisten.then((fn) => fn()) }
  }, [position])

  // Exit reposition mode
  const exitReposition = useCallback(async (savePosition: boolean) => {
    if (!virtualScreen) return

    const win = getCurrentWindow()

    // Convert drag position (relative to backdrop) back to absolute logical coords
    const finalPos = savePosition && dragPos
      ? {
          x: Math.round(dragPos.x + virtualScreen.left),
          y: Math.round(dragPos.y + virtualScreen.top),
        }
      : originalPositionRef.current || { x: 100, y: 100 }

    if (savePosition) {
      updateSettings({ overlayPosition: finalPos })
    }

    // Shrink back to pill size at final absolute position
    await win.setSize(new LogicalSize(WIN_W, WIN_H))
    await win.setPosition(new LogicalPosition(finalPos.x, finalPos.y))
    await win.setIgnoreCursorEvents(true)

    await win.emit('reposition-complete', finalPos)

    setRepositioning(false)
    setDragPos(null)
    setDropped(false)
    setDragging(false)
    setVirtualScreen(null)
    draggingRef.current = false
    dragStartRef.current = null
    originalPositionRef.current = null
    setSnapPoints([])
  }, [virtualScreen, dragPos, updateSettings])

  // Cancel reposition if recording starts
  useEffect(() => {
    if (!repositioning) return
    const unlisten = listen('hotkey-pressed', () => {
      exitReposition(false)
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [repositioning, exitReposition])

  // Escape to cancel, Enter to confirm
  useEffect(() => {
    if (!repositioning) return
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        exitReposition(false)
      } else if (e.key === 'Enter') {
        e.preventDefault()
        exitReposition(true)
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [repositioning, exitReposition])

  // Drag handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!dragPos) return
    e.preventDefault()
    draggingRef.current = true
    setDragging(true)
    setDropped(false)
    dragStartRef.current = {
      mouseX: e.clientX,
      mouseY: e.clientY,
      pillX: dragPos.x,
      pillY: dragPos.y,
    }
  }, [dragPos])

  useEffect(() => {
    if (!repositioning || !virtualScreen) return

    const handleMouseMove = (e: MouseEvent) => {
      if (!draggingRef.current || !dragStartRef.current) return
      const dx = e.clientX - dragStartRef.current.mouseX
      const dy = e.clientY - dragStartRef.current.mouseY
      let newX = Math.max(0, Math.min(virtualScreen.width - WIN_W, dragStartRef.current.pillX + dx))
      let newY = Math.max(0, Math.min(virtualScreen.height - WIN_H, dragStartRef.current.pillY + dy))

      // Magnetic snap — convert to absolute, check against snap points, convert back
      const absCX = newX + virtualScreen.left + WIN_W / 2
      const absCY = newY + virtualScreen.top + WIN_H / 2
      for (const snap of snapPoints) {
        const sx = snap.x + WIN_W / 2
        const sy = snap.y + WIN_H / 2
        const dist = Math.sqrt((absCX - sx) ** 2 + (absCY - sy) ** 2)
        if (dist < SNAP_RADIUS) {
          newX = snap.x - virtualScreen.left
          newY = snap.y - virtualScreen.top
          break
        }
      }

      setDragPos({ x: newX, y: newY })
    }

    const handleMouseUp = () => {
      if (!draggingRef.current) return
      draggingRef.current = false
      setDragging(false)
      dragStartRef.current = null
      setDropped(true)
    }

    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [repositioning, virtualScreen, snapPoints])

  // Normal positioning (when not in reposition mode)
  useEffect(() => {
    if (repositioning) return
    const win = getCurrentWindow()
    resolvePosition(position).then(async (pos) => {
      await win.setSize(new LogicalSize(WIN_W, WIN_H))
      await win.setPosition(new LogicalPosition(pos.x, pos.y))
      await win.setIgnoreCursorEvents(true)
    })
  }, [position, repositioning])

  // Show/hide separately
  useEffect(() => {
    if (repositioning) return
    const win = getCurrentWindow()
    if (shouldShow) {
      win.show()
    } else {
      win.hide()
    }
  }, [shouldShow, repositioning])

  // Auto-dismiss after done/error state
  useEffect(() => {
    const delay = status === 'done' && autoDismiss ? 1200 : status === 'error' ? 2000 : null
    if (delay === null) return

    const timer = setTimeout(() => {
      setDismissing(true)
      setTimeout(() => {
        setStatus('idle')
        setDismissing(false)
      }, 200)
    }, delay)
    return () => clearTimeout(timer)
  }, [status, autoDismiss, setStatus])

  // Reposition mode render
  if (repositioning && dragPos && virtualScreen) {
    // Compute snap dots relative to backdrop (virtual screen origin)
    const snapDots = snapPoints.map((s) => ({
      x: s.x - virtualScreen.left + WIN_W / 2,
      y: s.y - virtualScreen.top + WIN_H / 2,
      label: s.label,
      absX: s.x,
      absY: s.y,
    }))

    return (
      <div className="fixed inset-0 bg-black/60">
        {/* Instructions + Done button */}
        <div className="absolute top-8 left-0 right-0 flex items-start justify-center gap-4">
          <div className="flex flex-col items-center gap-2">
            <div className="rounded-lg bg-white/10 px-4 py-2 backdrop-blur-sm">
              <span className="text-[14px] font-medium text-white">
                Drag the pill to reposition
              </span>
            </div>
            <span className="text-[12px] text-white/60">
              Enter to confirm · Escape to cancel
            </span>
          </div>
          {dropped && (
            <button
              onClick={() => exitReposition(true)}
              className="animate-fade-in rounded-lg bg-white px-5 py-2 text-[13px] font-semibold text-[#1a1a1a] shadow-lg transition-all hover:bg-chirp-amber-50 hover:shadow-xl active:scale-95"
            >
              Done
            </button>
          )}
        </div>

        {/* Snap grid dots (all monitors) */}
        {snapDots.map((dot, i) => (
          <button
            key={i}
            className="group absolute flex flex-col items-center gap-1.5 -translate-x-1/2 -translate-y-1/2"
            style={{ left: dot.x, top: dot.y }}
            onClick={() => {
              setDragPos({ x: dot.absX - virtualScreen.left, y: dot.absY - virtualScreen.top })
              setDropped(true)
            }}
          >
            <div className="h-3 w-3 rounded-full border-2 border-white/30 bg-white/10 transition-all group-hover:scale-150 group-hover:border-white/60 group-hover:bg-white/20" />
            <span className="text-[10px] text-white/0 transition-all group-hover:text-white/50 select-none">
              {dot.label}
            </span>
          </button>
        ))}

        {/* Draggable pill */}
        <div
          style={{
            position: 'absolute',
            left: dragPos.x,
            top: dragPos.y,
            width: WIN_W,
            height: WIN_H,
            cursor: dragging ? 'grabbing' : 'grab',
          }}
          className="flex items-center justify-center"
          onMouseDown={handleMouseDown}
        >
          <div className="flex h-11 items-center gap-3 rounded-full border border-chirp-amber-200/30 bg-white/90 px-4 shadow-[0_2px_16px_rgba(245,158,11,0.08),0_0_0_3px_rgba(245,158,11,0.05)] backdrop-blur-xl">
            <BirdMark size={24} className="text-chirp-amber-500" />
            <span className="text-[13px] font-medium text-chirp-stone-600 select-none">Chirp</span>
          </div>
        </div>
      </div>
    )
  }

  if (!shouldShow) return null

  return (
    <div className="pointer-events-none flex h-screen w-screen items-center justify-center">
      <div
        className={`flex items-center rounded-full transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] backdrop-blur-xl ${
          isActive
            ? 'h-11 gap-3 border border-chirp-amber-200/30 bg-white/90 px-4 shadow-[0_2px_16px_rgba(245,158,11,0.08),0_0_0_3px_rgba(245,158,11,0.05)]'
            : 'h-9 gap-2 border border-chirp-amber-200/30 bg-white/90 px-2.5 shadow-[0_1px_4px_rgba(0,0,0,0.05)]'
        } ${dismissing ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}
      >
        <BirdMark size={isActive ? 24 : 18} className={
          isActive
            ? 'text-chirp-amber-500'
            : hotkeyStatus === 'failed'
              ? 'text-red-400'
              : hotkeyStatus === 'retrying'
                ? 'text-chirp-amber-400 animate-pulse'
                : 'text-chirp-stone-400'
        } />
        {isActive && (
          <div className="animate-fade-in flex items-center">
            {status === 'listening' && <Listening />}
            {status === 'processing' && <Processing />}
            {status === 'polishing' && <Polishing />}
            {status === 'done' && <Done />}
            {status === 'error' && <Error />}
          </div>
        )}
      </div>
    </div>
  )
}
