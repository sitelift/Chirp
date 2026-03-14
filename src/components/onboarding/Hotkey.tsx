import { useState, useCallback, useEffect } from 'react'
import { Keyboard } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { KeyBadge } from '../shared/KeyBadge'
import { Button } from '../shared/Button'

interface HotkeyProps {
  onFinish: () => void
}

export function Hotkey({ onFinish }: HotkeyProps) {
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
    <div className="flex flex-col items-center">
      <Keyboard size={48} className="text-chirp-stone-700" strokeWidth={1.5} />

      <h1 className="mt-6 font-display font-extrabold text-2xl text-chirp-stone-900">
        Set your hotkey
      </h1>

      <p className="mt-4 max-w-[360px] text-center font-body text-[15px] leading-[1.7] text-chirp-stone-700">
        This is the shortcut you'll press to start and stop dictation.
      </p>

      {/* Hotkey capture area */}
      <button
        onClick={() => setCapturing(true)}
        className={`mt-6 flex h-20 w-80 flex-col items-center justify-center rounded-xl ${
          capturing
            ? 'border-2 border-solid border-chirp-amber-400'
            : 'border-2 border-dashed border-chirp-stone-300'
        } bg-chirp-stone-100 transition-colors duration-150`}
      >
        <div className="flex items-center gap-1.5">
          {hotkeyParts.map((part, i) => (
            <KeyBadge key={i} keyLabel={part} />
          ))}
        </div>
        <span className="mt-2 font-body text-xs text-chirp-stone-500">
          {capturing ? 'Press your shortcut...' : 'Press keys to change...'}
        </span>
      </button>

      <div className="mt-8">
        <Button size="onboarding" className="min-w-[180px]" onClick={onFinish}>
          Start Using Chirp →
        </Button>
      </div>
    </div>
  )
}
