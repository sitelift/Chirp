import { useState, useCallback, useEffect } from 'react'
import { Keyboard } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { Button } from '../shared/Button'

interface HotkeyProps {
  onNext: () => void
}

export function Hotkey({ onNext }: HotkeyProps) {
  const hotkey = useAppStore((s) => s.hotkey)
  const updateSettings = useAppStore((s) => s.updateSettings)
  const [capturing, setCapturing] = useState(false)

  const hotkeyParts = hotkey
    .replace('CmdOrCtrl', navigator.platform.includes('Mac') ? '\u2318' : 'Ctrl')
    .split('+')

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
        STEP 3 OF 4
      </span>

      {/* Styled keyboard card */}
      <div className="w-20 h-20 rounded-2xl bg-chirp-amber-50 border border-chirp-amber-200 flex items-center justify-center mt-6">
        <Keyboard size={32} className="text-chirp-amber-500" strokeWidth={1.5} />
      </div>

      <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
        Set your hotkey
      </h1>

      <p className="mt-4 font-body text-[15px] leading-[1.7] text-chirp-stone-700">
        This is the shortcut you'll press to start and stop dictation.
      </p>

      {/* Hotkey capture area */}
      <button
        onClick={() => setCapturing(true)}
        className={`mt-6 flex h-24 w-full flex-col items-center justify-center rounded-2xl transition-all duration-150 ${
          capturing
            ? 'border-2 border-solid border-chirp-amber-400 bg-chirp-stone-100 shadow-[0_0_0_4px_rgba(251,191,36,0.2)]'
            : 'border-2 border-dashed border-chirp-stone-300 bg-chirp-stone-100'
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
        <span className="mt-2 font-body text-xs text-chirp-stone-500">
          {capturing ? 'Press your shortcut...' : 'Click to change'}
        </span>
      </button>

      <p className="mt-2 font-body text-xs text-chirp-stone-400">
        Tip: Use a shortcut with 2+ modifier keys
      </p>

      <div className="mt-8">
        <Button size="onboarding" className="min-w-[180px] text-base" onClick={onNext}>
          Continue →
        </Button>
      </div>
    </div>
  )
}
