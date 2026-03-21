import { useState, useEffect, useRef, useCallback } from 'react'
import { Mic, Play, RotateCcw, Keyboard } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { formatHotkey } from '../../lib/utils'
import { Button } from '../shared/Button'

interface SetupStepProps {
  onNext: () => void
}

export function SetupStep({ onNext }: SetupStepProps) {
  const tauri = useTauri()
  const hotkey = useAppStore((s) => s.hotkey)
  const updateSettings = useAppStore((s) => s.updateSettings)

  // Mic test state
  const [micState, setMicState] = useState<'idle' | 'recording' | 'playing' | 'done'>('idle')
  const [countdown, setCountdown] = useState(3)
  const [inputLevel, setInputLevel] = useState(0)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  // Hotkey state
  const [capturing, setCapturing] = useState(false)
  const hotkeyParts = formatHotkey(hotkey)

  // Gate: must have a hotkey set (hotkey always has a default, so just check it exists)
  const canContinue = hotkey && hotkey.includes('+')

  // Live input level polling during recording
  useEffect(() => {
    if (micState !== 'recording') return
    const interval = setInterval(async () => {
      try {
        const level = await tauri.getInputLevel()
        setInputLevel(level)
      } catch {}
    }, 67)
    return () => clearInterval(interval)
  }, [micState])

  const handleRecord = async () => {
    setMicState('recording')
    setCountdown(3)
    const interval = setInterval(() => {
      setCountdown((c) => {
        if (c <= 1) {
          clearInterval(interval)
          return 0
        }
        return c - 1
      })
    }, 1000)

    try {
      const wavBytes = await tauri.testMicrophone()
      clearInterval(interval)

      const uint8 = new Uint8Array(wavBytes)
      const blob = new Blob([uint8], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      setMicState('playing')
      const audio = new Audio(url)
      audioRef.current = audio
      audio.onended = () => {
        setMicState('done')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.onerror = () => {
        setMicState('done')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.play()
    } catch {
      clearInterval(interval)
      setMicState('idle')
    }
  }

  // Hotkey capture
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!capturing) return
      e.preventDefault()
      if (e.key === 'Escape') {
        setCapturing(false)
        return
      }
      const parts: string[] = []
      if (e.ctrlKey || e.metaKey) parts.push('CmdOrCtrl')
      if (e.shiftKey) parts.push('Shift')
      if (e.altKey) parts.push('Alt')
      if (!['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
        parts.push(e.key.toUpperCase())
      }
      if (parts.length > 1) {
        updateSettings({ hotkey: parts.join('+') })
        setCapturing(false)
      }
    },
    [capturing, updateSettings]
  )

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  return (
    <div className="flex flex-col animate-fade-in">
      <span className="inline-flex items-center self-start rounded-full bg-chirp-amber-50 border border-chirp-amber-200 px-3 py-1 font-body text-xs text-chirp-amber-500 font-medium">
        STEP 2 OF 4
      </span>

      <h1 className="mt-4 font-display font-extrabold text-3xl text-chirp-stone-900">
        Setup
      </h1>

      {/* Section 1: Mic Test */}
      <div className="mt-6">
        <div className="flex items-center gap-2.5 mb-3">
          <div className="w-8 h-8 rounded-lg bg-chirp-amber-50 border border-chirp-amber-200 flex items-center justify-center">
            <Mic size={16} className="text-chirp-amber-500" strokeWidth={1.5} />
          </div>
          <h2 className="font-display font-bold text-lg text-chirp-stone-900">
            Test your microphone
          </h2>
        </div>
        <p className="font-body text-sm text-chirp-stone-500 mb-4">
          Record a short clip and listen back to make sure your mic works.
        </p>

        <div className="rounded-xl border border-card-border bg-white p-4">
          {micState === 'recording' && (
            <div className="w-full">
              <div className="h-3 w-full overflow-hidden rounded-full bg-chirp-stone-200">
                <div
                  className="h-full rounded-full bg-chirp-success transition-all duration-100"
                  style={{ width: `${Math.min(100, inputLevel * 100)}%` }}
                />
              </div>
              <p className="font-body text-sm text-chirp-stone-500 mt-2 text-center">
                Recording... ({countdown}s)
              </p>
            </div>
          )}

          {micState === 'playing' && (
            <div className="flex items-center justify-center gap-2">
              <Play size={18} className="text-chirp-amber-500 animate-pulse" />
              <p className="font-body text-sm text-chirp-stone-500">Playing back...</p>
            </div>
          )}

          {micState === 'idle' && (
            <div className="flex justify-center">
              <Button variant="secondary" onClick={handleRecord} className="gap-2">
                <Mic size={18} />
                Record a test clip
              </Button>
            </div>
          )}

          {micState === 'done' && (
            <div className="flex flex-col items-center gap-3">
              <p className="font-body text-sm text-chirp-stone-700">
                Did you hear yourself clearly?
              </p>
              <div className="flex items-center gap-3">
                <Button variant="secondary" onClick={handleRecord} className="gap-2">
                  <RotateCcw size={16} />
                  Try again
                </Button>
                <span className="font-body text-xs text-chirp-success font-medium">Sounds good!</span>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Section 2: Hotkey */}
      <div className="mt-8">
        <div className="flex items-center gap-2.5 mb-3">
          <div className="w-8 h-8 rounded-lg bg-chirp-amber-50 border border-chirp-amber-200 flex items-center justify-center">
            <Keyboard size={16} className="text-chirp-amber-500" strokeWidth={1.5} />
          </div>
          <h2 className="font-display font-bold text-lg text-chirp-stone-900">
            Set your hotkey
          </h2>
        </div>
        <p className="font-body text-sm text-chirp-stone-500 mb-4">
          This shortcut starts and stops dictation.
        </p>

        {/* Hotkey capture area */}
        <button
          onClick={() => setCapturing(true)}
          className={`flex h-20 w-full flex-col items-center justify-center rounded-xl transition-all duration-150 ${
            capturing
              ? 'border-2 border-solid border-chirp-yellow bg-white shadow-[0_0_0_4px_rgba(240,183,35,0.15)]'
              : 'border-2 border-dashed border-chirp-stone-300 bg-white'
          }`}
        >
          <div className="flex items-center gap-2">
            {hotkeyParts.map((part, i) => (
              <span
                key={i}
                className="inline-flex items-center justify-center rounded-lg border border-chirp-stone-300 bg-chirp-stone-50 px-3 py-1.5 font-mono text-base font-medium text-chirp-stone-700 shadow-subtle"
              >
                {part}
              </span>
            ))}
          </div>
          <span className="mt-2 font-body text-xs text-chirp-stone-500">
            {capturing ? 'Press your shortcut...' : 'Click to change'}
          </span>
        </button>

        <p className="mt-2 font-body text-xs text-chirp-stone-400">
          Tip: Use a shortcut with 2+ modifier keys
        </p>
      </div>

      {/* Continue button */}
      <div className="mt-8">
        <Button
          size="onboarding"
          className="min-w-[180px] text-base"
          onClick={onNext}
          disabled={!canContinue}
        >
          Continue →
        </Button>
      </div>
    </div>
  )
}
