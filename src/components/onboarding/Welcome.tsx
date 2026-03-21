import { useState } from 'react'
import { Lock } from 'lucide-react'
import { open } from '@tauri-apps/plugin-shell'
import { Button } from '../shared/Button'

interface WelcomeProps {
  onNext: () => void
}

export function Welcome({ onNext }: WelcomeProps) {
  const [denied, setDenied] = useState(false)

  const valueProps = [
    'Free, local voice-to-text for everyone',
    'Your voice never leaves your device',
    'No accounts, no cloud, no subscriptions',
  ]

  const handleGetStarted = async () => {
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
        STEP 1 OF 4
      </span>

      <h1 className="font-display font-extrabold text-3xl text-chirp-stone-900 mt-4">
        Welcome to Chirp
      </h1>

      <div className="flex flex-col gap-4 mt-6">
        {valueProps.map((text) => (
          <div key={text} className="flex items-start gap-3">
            <div className="w-2 h-2 rounded-full bg-chirp-yellow mt-2 shrink-0" />
            <span className="font-body text-[15px] leading-[1.7] text-chirp-stone-700">
              {text}
            </span>
          </div>
        ))}
      </div>

      {/* Privacy note */}
      <div className="rounded-lg bg-chirp-stone-50 border border-card-border p-3 mt-6 flex items-start gap-2.5">
        <Lock size={16} className="text-chirp-stone-400 mt-0.5 shrink-0" />
        <p className="font-body text-sm text-chirp-stone-600">
          We'll verify your mic works. Chirp only listens when you press your hotkey — nothing runs in the background.
        </p>
      </div>

      {denied && (
        <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
          <p className="font-body text-sm text-red-700">
            Microphone access was denied. You can enable it in your system settings.
          </p>
        </div>
      )}

      <div className="mt-8 flex flex-col gap-3">
        <Button size="onboarding" className="min-w-[180px] text-base" onClick={handleGetStarted}>
          Get Started →
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
