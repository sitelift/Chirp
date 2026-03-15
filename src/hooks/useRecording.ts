import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../stores/appStore'
import { useTauri } from './useTauri'

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
  const pendingStopRef = useRef(false)

  useEffect(() => {
    // Only handle recording in the overlay window
    const windowLabel = getCurrentWindow().label
    if (windowLabel !== 'overlay') return

    const unlisteners: Array<() => void> = []

    // --- Hold-to-record: press to start ---
    listen('hotkey-pressed', async () => {
      if (busyRef.current) return
      const status = useAppStore.getState().status
      if (status !== 'idle') return

      busyRef.current = true
      pendingStopRef.current = false
      try {
        setStatus('listening')
        await tauri.startRecording()
      } catch (e) {
        handleStartError(e)
        return
      } finally {
        busyRef.current = false
      }

      // If user released the hotkey while startRecording was awaiting, stop now
      if (pendingStopRef.current) {
        pendingStopRef.current = false
        busyRef.current = true
        try {
          await stopAndProcess()
        } finally {
          busyRef.current = false
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

      busyRef.current = true
      try {
        await stopAndProcess()
      } finally {
        busyRef.current = false
      }
    }).then((fn) => unlisteners.push(fn))

    // --- Tray menu toggle (fallback) ---
    listen('toggle-recording', async () => {
      if (busyRef.current) return
      const status = useAppStore.getState().status

      busyRef.current = true
      try {
        if (status === 'idle') {
          setStatus('listening')
          await tauri.startRecording()
        } else if (status === 'listening') {
          await stopAndProcess()
        }
      } catch (e) {
        handleStartError(e)
      } finally {
        busyRef.current = false
      }
    }).then((fn) => unlisteners.push(fn))

    async function stopAndProcess() {
      try {
        setStatus('processing')
        const result = await tauri.stopRecording()
        setWordCount(result.wordCount)
        setStatus('done')
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
