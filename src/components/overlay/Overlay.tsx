import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../../stores/appStore'
import { useAudio } from '../../hooks/useAudio'
import { useRecording } from '../../hooks/useRecording'
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
  useEffect(() => {
    const win = getCurrentWindow()
    if (status !== 'idle') {
      win.show()
    } else {
      win.hide()
    }
  }, [status])

  // Auto-dismiss after done state
  useEffect(() => {
    if (status === 'done' && autoDismiss) {
      const timer = setTimeout(() => {
        setDismissing(true)
        setTimeout(() => {
          setStatus('idle')
          setDismissing(false)
        }, 150)
      }, 800)
      return () => clearTimeout(timer)
    }
  }, [status, autoDismiss, setStatus])

  if (status === 'idle') return null

  return (
    <div className="flex h-screen w-screen items-end justify-center pb-4">
      <div
        className={`w-[320px] rounded-2xl border border-chirp-stone-200 bg-white px-4 py-3 shadow-overlay ${
          dismissing ? 'animate-overlay-out' : 'animate-overlay-in'
        }`}
        style={{ transition: 'height 200ms ease-out' }}
      >
        {status === 'listening' && <Listening />}
        {status === 'processing' && <Processing />}
        {status === 'done' && <Done />}
        {status === 'error' && <Error />}
      </div>
    </div>
  )
}
