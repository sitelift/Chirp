import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import {
  addKeyToCapture,
  addSystemKeyToCapture,
  buildHotkeyString,
  captureIsValid,
  captureIsModifierOnly,
  createStickyCapture,
  getCaptureLabels,
  type CapturedHotkey,
  type StickyCapture,
} from '../lib/hotkeyCapture'

export function useHotkeyRecorder() {
  const [capturing, setCapturing] = useState(false)
  const [pendingHotkey, setPendingHotkey] = useState<CapturedHotkey | null>(null)
  const [previewLabels, setPreviewLabels] = useState<string[]>([])
  const [systemCapturing, setSystemCapturing] = useState(false)
  const [showSystemHint, setShowSystemHint] = useState(false)
  const captureRef = useRef<StickyCapture>(createStickyCapture())
  const hintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (!capturing) return

    hintTimerRef.current = setTimeout(() => {
      if (captureRef.current.keys.size === 0) {
        setShowSystemHint(true)
      }
    }, 3000)

    const handleKeyDown = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()

      if (event.key === 'Escape') {
        setCapturing(false)
        setPreviewLabels([])
        setPendingHotkey(null)
        setShowSystemHint(false)
        captureRef.current = createStickyCapture()
        return
      }

      if (hintTimerRef.current) {
        clearTimeout(hintTimerRef.current)
        hintTimerRef.current = null
      }
      setShowSystemHint(false)

      const next = addKeyToCapture(captureRef.current, event)
      captureRef.current = next
      setPreviewLabels(getCaptureLabels(next))
    }

    const handleKeyUp = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()
    }

    window.addEventListener('keydown', handleKeyDown, true)
    window.addEventListener('keyup', handleKeyUp, true)

    return () => {
      window.removeEventListener('keydown', handleKeyDown, true)
      window.removeEventListener('keyup', handleKeyUp, true)
      if (hintTimerRef.current) {
        clearTimeout(hintTimerRef.current)
        hintTimerRef.current = null
      }
    }
  }, [capturing])

  const startCapture = () => {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur()
    }
    captureRef.current = createStickyCapture()
    setCapturing(true)
    setPreviewLabels([])
    setPendingHotkey(null)
    setShowSystemHint(false)
  }

  const startSystemCapture = async () => {
    setSystemCapturing(true)
    setShowSystemHint(false)
    try {
      const result = await invoke<{ code: string; label: string }>('capture_next_key')
      const next = addSystemKeyToCapture(captureRef.current, result.code)
      captureRef.current = next
      setPreviewLabels(getCaptureLabels(next))
    } catch {
      // Timeout or error — just go back to normal capture
    }
    setSystemCapturing(false)
  }

  const confirmCapture = (): CapturedHotkey | null => {
    const result = buildHotkeyString(captureRef.current)
    if (result) {
      setPendingHotkey(result)
    }
    setCapturing(false)
    setShowSystemHint(false)
    return result
  }

  const cancelCapture = () => {
    setCapturing(false)
    setPreviewLabels([])
    setPendingHotkey(null)
    setShowSystemHint(false)
    captureRef.current = createStickyCapture()
  }

  const clearPending = () => {
    setCapturing(false)
    setPreviewLabels([])
    setPendingHotkey(null)
    setShowSystemHint(false)
    captureRef.current = createStickyCapture()
  }

  const canConfirm = captureIsValid(captureRef.current)
  const isModifierOnly = captureIsModifierOnly(captureRef.current)

  return {
    capturing,
    pendingHotkey,
    previewLabels,
    canConfirm,
    isModifierOnly,
    showSystemHint,
    systemCapturing,
    startCapture,
    startSystemCapture,
    confirmCapture,
    cancelCapture,
    clearPending,
  }
}
