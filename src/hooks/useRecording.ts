import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../stores/appStore'
import { useTauri } from './useTauri'
import { playCompletionSound } from '../lib/sounds'

/**
 * Hold-to-record: hotkey press starts recording, release stops and processes.
 * Also supports toggle via tray menu.
 * Only runs in the overlay window.
 */
export function useRecording() {
  const setStatus = useAppStore((s) => s.setStatus)
  const setError = useAppStore((s) => s.setError)
  const setWordCount = useAppStore((s) => s.setWordCount)
  const tauri = useTauri()
  const busyRef = useRef(false)
  const busyTimestampRef = useRef(0)
  const pendingStopRef = useRef(false)

  useEffect(() => {
    // Only handle recording in the overlay window
    const windowLabel = getCurrentWindow().label
    if (windowLabel !== 'overlay') return

    const unlisteners: Array<() => void> = []

    // Safety: if busyRef has been stuck for >5s, force-reset it
    const checkBusyStale = () => {
      if (busyRef.current && busyTimestampRef.current > 0) {
        if (Date.now() - busyTimestampRef.current > 5000) {
          console.warn('busyRef stuck for >5s, force-resetting')
          busyRef.current = false
          busyTimestampRef.current = 0
        }
      }
    }

    const setBusy = (busy: boolean) => {
      busyRef.current = busy
      busyTimestampRef.current = busy ? Date.now() : 0
    }

    // --- Hold-to-record: press to start ---
    listen('hotkey-pressed', async () => {
      checkBusyStale()
      if (busyRef.current) return
      const status = useAppStore.getState().status
      // Allow restarting from error/done states (don't wait for auto-dismiss)
      if (status !== 'idle' && status !== 'error' && status !== 'done') return
      if (status === 'error' || status === 'done') setStatus('idle')

      setBusy(true)
      pendingStopRef.current = false
      try {
        setStatus('listening')
        await tauri.startRecording()
      } catch (e) {
        handleStartError(e)
        return
      } finally {
        setBusy(false)
      }

      // If user released the hotkey while startRecording was awaiting, stop now
      if (pendingStopRef.current) {
        pendingStopRef.current = false
        setBusy(true)
        try {
          await stopAndProcess()
        } finally {
          setBusy(false)
        }
      }
    }).then((fn) => unlisteners.push(fn))

    // --- Hold-to-record: release to stop ---
    listen('hotkey-released', async () => {
      // If start is still in progress, queue the stop for when it finishes
      if (busyRef.current) {
        pendingStopRef.current = true
        return
      }
      const status = useAppStore.getState().status
      if (status !== 'listening') return

      setBusy(true)
      try {
        await stopAndProcess()
      } finally {
        setBusy(false)
      }
    }).then((fn) => unlisteners.push(fn))

    // --- Tray menu toggle ---
    listen('toggle-recording', async () => {
      checkBusyStale()
      if (busyRef.current) return
      const status = useAppStore.getState().status

      setBusy(true)
      try {
        if (status === 'idle' || status === 'error' || status === 'done') {
          if (status === 'error' || status === 'done') setStatus('idle')
          setStatus('listening')
          await tauri.startRecording()
        } else if (status === 'listening') {
          await stopAndProcess()
        }
      } catch (e) {
        handleStartError(e)
      } finally {
        setBusy(false)
      }
    }).then((fn) => unlisteners.push(fn))

    async function stopAndProcess() {
      try {
        setStatus('processing')
        const result = await tauri.stopRecording()
        setWordCount(result.wordCount)
        setStatus('done')
        if (useAppStore.getState().playSoundOnComplete) {
          playCompletionSound()
        }
      } catch (e) {
        const errMsg = String(e)
        if (errMsg.includes('transcription_failed')) {
          setError('transcription_failed')
        } else if (errMsg.includes('injection_failed')) {
          setError('injection_failed')
        } else {
          setError('unknown')
        }
      }
    }

    function handleStartError(e: unknown) {
      const errMsg = String(e)
      if (errMsg.includes('model_not_loaded')) {
        setError('model_not_loaded')
      } else if (errMsg.includes('mic_not_found')) {
        setError('mic_not_found')
      } else if (errMsg.includes('mic_permission')) {
        setError('mic_permission')
      } else {
        setError('unknown')
      }
    }

    // Listen for polishing state from backend
    listen<string>('recording-state', (event) => {
      if (event.payload === 'polishing') {
        setStatus('polishing')
      }
    }).then((fn) => unlisteners.push(fn))

    // Escape key to cancel/dismiss
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        const status = useAppStore.getState().status
        if (status === 'listening') {
          await tauri.cancelRecording()
          setStatus('idle')
        } else if (status === 'error' || status === 'done') {
          setStatus('idle')
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)

    return () => {
      unlisteners.forEach((fn) => fn())
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [])
}
