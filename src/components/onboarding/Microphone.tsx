import { useState } from 'react'
import { Mic, Lock } from 'lucide-react'
import { open } from '@tauri-apps/plugin-shell'
import { Button } from '../shared/Button'

interface MicrophoneProps {
  onNext: () => void
}

export function Microphone({ onNext }: MicrophoneProps) {
  const [denied, setDenied] = useState(false)

  const handleAllow = async () => {
    try {
      await navigator.mediaDevices.getUserMedia({ audio: true })
      onNext()
    } catch {
      setDenied(true)
    }
  }

  return (
    <div className="flex flex-col animate-fade-in">
      <span className="inline-flex items-center self-start rounded-full bg-chirp-amber-50 border border-chirp-amber-200 px-3 py-1 font-body text-xs text-chirp-amber-500 font-medium">
        STEP 2 OF 6
      </span>

      {/* Styled mic card */}
      <div className="w-20 h-20 rounded-2xl bg-chirp-amber-50 border border-chirp-amber-200 flex items-center justify-center mt-6">
        <Mic size={32} className="text-chirp-amber-500" strokeWidth={1.5} />
      </div>

      <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
        Microphone access
      </h1>

      {denied ? (
        <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
          <p className="font-body text-sm text-red-700">
            Microphone access was denied. You can enable it in your system settings.
          </p>
        </div>
      ) : (
        <div className="rounded-lg bg-chirp-stone-50 border border-chirp-stone-200 p-3 mt-4 flex items-start gap-2.5">
          <Lock size={16} className="text-chirp-stone-400 mt-0.5 shrink-0" />
          <p className="font-body text-sm text-chirp-stone-600">
            We'll verify your mic works. Chirp only listens when you press your hotkey — nothing runs in the background.
          </p>
        </div>
      )}

      <div className="mt-8 flex flex-col gap-3">
        <Button size="onboarding" className="min-w-[180px] text-base" onClick={handleAllow}>
          Verify Microphone →
        </Button>
        {denied && (
          <Button
            variant="secondary"
            size="onboarding"
            onClick={() => {
              open('ms-settings:privacy-microphone')
            }}
          >
            Open System Settings
          </Button>
        )}
      </div>
    </div>
  )
}
