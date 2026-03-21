import { useState, useEffect, useCallback } from 'react'
import { useAppStore } from '../../stores/appStore'
import { formatHotkey } from '../../lib/utils'
import { Button } from '../shared/Button'

interface SetupStepProps {
  onNext: () => void
}

export function SetupStep({ onNext }: SetupStepProps) {
  const hotkey = useAppStore((s) => s.hotkey)
  const updateSettings = useAppStore((s) => s.updateSettings)

  const [capturing, setCapturing] = useState(false)
  const hotkeyParts = formatHotkey(hotkey)

  const canContinue = hotkey && hotkey.includes('+')

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
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Set your hotkey
      </h1>
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        This shortcut starts and stops dictation.
      </p>

      {/* Hotkey capture area */}
      <button
        onClick={() => setCapturing(true)}
        className={`mt-5 flex h-20 w-full flex-col items-center justify-center rounded-xl transition-all duration-150 ${
          capturing
            ? 'border-2 border-solid border-chirp-yellow bg-chirp-amber-50/50 shadow-[0_0_0_4px_rgba(240,183,35,0.15)]'
            : 'border-2 border-dashed border-chirp-stone-300 bg-chirp-stone-50'
        }`}
      >
        <div className="flex items-center gap-2">
          {hotkeyParts.map((part, i) => (
            <span
              key={i}
              className="inline-flex items-center justify-center rounded-lg border border-chirp-stone-300 bg-white px-3 py-1.5 font-mono text-base font-medium text-chirp-stone-700 shadow-subtle"
            >
              {part}
            </span>
          ))}
        </div>
        <span className="mt-2 font-body text-xs text-chirp-stone-400">
          {capturing ? 'Press your shortcut...' : 'Click to change'}
        </span>
      </button>

      <p className="mt-2 font-body text-xs text-chirp-stone-400">
        Tip: Use a shortcut with 2+ modifier keys
      </p>

      <div className="mt-6">
        <Button
          size="onboarding"
          className="min-w-[160px] text-base"
          onClick={onNext}
          disabled={!canContinue}
        >
          Continue
        </Button>
      </div>
    </div>
  )
}
