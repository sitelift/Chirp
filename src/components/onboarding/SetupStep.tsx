import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { KeyBadge } from '../shared/KeyBadge'
import { Button } from '../shared/Button'

interface SetupStepProps {
  onNext: () => void
}

export function SetupStep({ onNext }: SetupStepProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [capturing, setCapturing] = useState(false)

  const canContinue = store.hotkeyKeycode > 0

  const handleCaptureKey = async () => {
    setCapturing(true)
    try {
      const result = await tauri.captureHotkeyKey()
      if (result.keycode >= 0) {
        store.updateSettings({
          hotkeyMode: 'dedicated_key',
          hotkeyKeycode: result.keycode,
          hotkeyKeyName: result.name,
        })
      }
    } catch {
      // capture cancelled or failed
    } finally {
      setCapturing(false)
    }
  }

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Set your hotkey
      </h1>
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        Pick a key to hold while you speak. Release to transcribe.
      </p>

      {/* Hotkey capture area */}
      <button
        onClick={handleCaptureKey}
        disabled={capturing}
        className={`mt-5 flex h-20 w-full flex-col items-center justify-center rounded-xl transition-all duration-150 ${
          capturing
            ? 'border-2 border-solid border-chirp-yellow bg-chirp-amber-50/50 shadow-[0_0_0_4px_rgba(240,183,35,0.15)]'
            : 'border-2 border-dashed border-chirp-stone-300 bg-chirp-stone-50'
        }`}
      >
        {capturing ? (
          <span className="font-body text-sm text-chirp-stone-500">
            Press any key...
          </span>
        ) : (
          <>
            <div className="flex items-center gap-2">
              <KeyBadge keyLabel={canContinue ? store.hotkeyKeyName : 'Not set'} />
            </div>
            <span className="mt-2 font-body text-xs text-chirp-stone-400">
              {canContinue ? 'Click to change' : 'Click to set hotkey'}
            </span>
          </>
        )}
      </button>

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
