import { useState } from 'react'
import { Mic } from 'lucide-react'
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
    <div className="flex flex-col items-center">
      <Mic size={48} className="text-chirp-stone-700" strokeWidth={1.5} />

      <h1 className="mt-6 font-display font-extrabold text-2xl text-chirp-stone-900">
        Chirp needs your microphone
      </h1>

      <p className="mt-4 max-w-[360px] text-center font-body text-[15px] leading-[1.7] text-chirp-stone-700">
        {denied
          ? 'Microphone access was denied. You can enable it in your system settings.'
          : "We only listen when you press your hotkey. That's it. Nothing runs in the background."}
      </p>

      <div className="mt-8 flex flex-col items-center gap-3">
        <Button size="onboarding" className="min-w-[180px]" onClick={handleAllow}>
          Allow Microphone →
        </Button>
        {denied && (
          <Button
            variant="secondary"
            size="onboarding"
            onClick={() => {
              // Will open OS settings when Tauri backend is wired
              console.log('Open system settings')
            }}
          >
            Open System Settings
          </Button>
        )}
      </div>
    </div>
  )
}
