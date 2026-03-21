import { useEffect, useRef, useState } from 'react'
import {
  addKeyToCapture,
  buildHotkeyString,
  captureIsValid,
  createStickyCapture,
  getCaptureLabels,
  type CapturedHotkey,
  type StickyCapture,
} from '../lib/hotkeyCapture'

export function useHotkeyRecorder() {
  const [capturing, setCapturing] = useState(false)
  const [pendingHotkey, setPendingHotkey] = useState<CapturedHotkey | null>(null)
  const captureRef = useRef<StickyCapture>(createStickyCapture())
  const [previewLabels, setPreviewLabels] = useState<string[]>([])

  useEffect(() => {
    if (!capturing) return

    const handleKeyDown = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()

      if (event.key === 'Escape') {
        setCapturing(false)
        setPreviewLabels([])
        setPendingHotkey(null)
        captureRef.current = createStickyCapture()
        return
      }

      const next = addKeyToCapture(captureRef.current, event)
      if (!next) return

      captureRef.current = next
      setPreviewLabels(getCaptureLabels(next))
    }

    // Swallow keyup to prevent other handlers from firing
    const handleKeyUp = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()
    }

    window.addEventListener('keydown', handleKeyDown, true)
    window.addEventListener('keyup', handleKeyUp, true)

    return () => {
      window.removeEventListener('keydown', handleKeyDown, true)
      window.removeEventListener('keyup', handleKeyUp, true)
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
  }

  const confirmCapture = (): CapturedHotkey | null => {
    const result = buildHotkeyString(captureRef.current)
    if (result) {
      setPendingHotkey(result)
    }
    setCapturing(false)
    return result
  }

  const cancelCapture = () => {
    setCapturing(false)
    setPreviewLabels([])
    setPendingHotkey(null)
    captureRef.current = createStickyCapture()
  }

  const clearPending = () => {
    setCapturing(false)
    setPreviewLabels([])
    setPendingHotkey(null)
    captureRef.current = createStickyCapture()
  }

  const canConfirm = captureIsValid(captureRef.current)

  return {
    capturing,
    pendingHotkey,
    previewLabels,
    canConfirm,
    startCapture,
    confirmCapture,
    cancelCapture,
    clearPending,
  }
}
