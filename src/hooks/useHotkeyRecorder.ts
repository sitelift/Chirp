import { useCallback, useEffect, useRef, useState } from 'react'
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
  const [canConfirm, setCanConfirm] = useState(false)
  const [isModifierOnly, setIsModifierOnly] = useState(false)
  const [systemCapturing, setSystemCapturing] = useState(false)
  const [showSystemHint, setShowSystemHint] = useState(false)
  const captureRef = useRef<StickyCapture>(createStickyCapture())
  const hintTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const setCapture = useCallback((capture: StickyCapture) => {
    captureRef.current = capture
    setPreviewLabels(getCaptureLabels(capture))
    setCanConfirm(captureIsValid(capture))
    setIsModifierOnly(captureIsModifierOnly(capture))
  }, [])

  const resetCapture = useCallback(() => {
    setCapture(createStickyCapture())
  }, [setCapture])

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
        setPendingHotkey(null)
        setShowSystemHint(false)
        resetCapture()
        return
      }

      if (hintTimerRef.current) {
        clearTimeout(hintTimerRef.current)
        hintTimerRef.current = null
      }
      setShowSystemHint(false)

      const next = addKeyToCapture(captureRef.current, event)
      setCapture(next)
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
  }, [capturing, resetCapture, setCapture])

  const startCapture = () => {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur()
    }
    resetCapture()
    setCapturing(true)
    setPendingHotkey(null)
    setShowSystemHint(false)
  }

  const startSystemCapture = async () => {
    setSystemCapturing(true)
    setShowSystemHint(false)
    try {
      const result = await invoke<{ code: string; label: string }>('capture_next_key')
      const next = addSystemKeyToCapture(captureRef.current, result.code)
      setCapture(next)
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
    setPendingHotkey(null)
    setShowSystemHint(false)
    resetCapture()
  }

  const clearPending = () => {
    setCapturing(false)
    setPendingHotkey(null)
    setShowSystemHint(false)
    resetCapture()
  }

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
