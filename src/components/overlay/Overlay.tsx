import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { primaryMonitor } from '@tauri-apps/api/window'
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi'
import { useAppStore } from '../../stores/appStore'
import { useAudio } from '../../hooks/useAudio'
import { useRecording } from '../../hooks/useRecording'
import { BirdMark } from '../shared/BirdMark'
import { Listening } from './Listening'
import { Processing } from './Processing'
import { Polishing } from './Polishing'
import { Done } from './Done'
import { Error } from './Error'

// Window stays the same size for both states — no jarring resize
const WIN_W = 600
const WIN_H = 56
const OFFSET = 80

export function Overlay() {
  const status = useAppStore((s) => s.status)
  const autoDismiss = useAppStore((s) => s.autoDismissOverlay)
  const setStatus = useAppStore((s) => s.setStatus)
  const position = useAppStore((s) => s.overlayPosition)
  const showPassive = useAppStore((s) => s.showPassiveOverlay)
  const [dismissing, setDismissing] = useState(false)

  useAudio()
  useRecording()

  const isActive = status !== 'idle'
  const shouldShow = isActive || showPassive

  // Position the window once — same size for passive and active
  useEffect(() => {
    const win = getCurrentWindow()

    if (!shouldShow) {
      win.hide()
      return
    }

    primaryMonitor().then(async (monitor) => {
      if (!monitor) return
      const sf = monitor.scaleFactor
      const screenW = monitor.size.width / sf
      const screenH = monitor.size.height / sf

      const x = Math.round((screenW - WIN_W) / 2)
      const y = position === 'top'
        ? OFFSET
        : Math.round(screenH - OFFSET - WIN_H)

      await win.setSize(new LogicalSize(WIN_W, WIN_H))
      await win.setPosition(new LogicalPosition(x, y))
      await win.show()
    })
  }, [shouldShow, position])

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

  if (!shouldShow) return null

  return (
    <div className="flex h-screen w-screen items-center justify-center">
      {/* Single pill that morphs between passive and active */}
      <div
        className={`flex items-center rounded-full transition-all duration-200 ease-[cubic-bezier(0.16,1,0.3,1)] ${
          isActive
            ? 'h-12 gap-3 border border-chirp-stone-200 bg-white px-4 shadow-overlay'
            : 'h-9 border border-chirp-stone-200/50 bg-white/70 px-2.5 shadow-sm backdrop-blur-sm'
        } ${dismissing ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}
      >
        <BirdMark size={isActive ? 22 : 16} />
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
