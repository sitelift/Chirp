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
import { Done } from './Done'
import { Error } from './Error'

export function Overlay() {
  const status = useAppStore((s) => s.status)
  const autoDismiss = useAppStore((s) => s.autoDismissOverlay)
  const setStatus = useAppStore((s) => s.setStatus)
  const [dismissing, setDismissing] = useState(false)

  useAudio()
  useRecording()

  // Show/hide the overlay window based on recording state
  // IMPORTANT: never call setFocus() — the target text field must keep focus for Ctrl+V injection
  // Position the window to span the full screen width so CSS centering works
  useEffect(() => {
    const win = getCurrentWindow()
    if (status !== 'idle') {
      primaryMonitor().then(async (monitor) => {
        if (monitor) {
          const sf = monitor.scaleFactor
          const w = monitor.size.width / sf
          const h = monitor.size.height / sf
          await win.setSize(new LogicalSize(w, h))
          await win.setPosition(new LogicalPosition(0, 0))
        }
        await win.show()
      })
    } else {
      win.hide()
    }
  }, [status])

  // Auto-dismiss after done/error state
  useEffect(() => {
    const delay = status === 'done' && autoDismiss ? 600 : status === 'error' ? 2000 : null
    if (delay === null) return

    const timer = setTimeout(() => {
      setDismissing(true)
      setTimeout(() => {
        setStatus('idle')
        setDismissing(false)
      }, 80)
    }, delay)
    return () => clearTimeout(timer)
  }, [status, autoDismiss, setStatus])

  if (status === 'idle') return null

  return (
    <div className="flex h-screen w-screen items-end justify-center pb-[80px]">
      <div
        className={`flex h-12 items-center gap-3 rounded-full border border-chirp-stone-200 bg-white px-4 shadow-overlay ${
          dismissing ? 'animate-overlay-out' : 'animate-overlay-in'
        }`}
      >
        <BirdMark size={22} />
        {status === 'listening' && <Listening />}
        {status === 'processing' && <Processing />}
        {status === 'done' && <Done />}
        {status === 'error' && <Error />}
      </div>
    </div>
  )
}
