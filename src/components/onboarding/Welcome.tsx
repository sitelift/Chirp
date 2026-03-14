import { BirdMark } from '../shared/BirdMark'
import { Button } from '../shared/Button'

interface WelcomeProps {
  onNext: () => void
}

export function Welcome({ onNext }: WelcomeProps) {
  return (
    <div className="flex flex-col items-center">
      <BirdMark size={80} />

      <h1 className="mt-6 font-display font-extrabold text-[28px] text-chirp-stone-900">
        Welcome to Chirp
      </h1>

      <p className="mt-4 max-w-[360px] text-center font-body text-[15px] leading-[1.7] text-chirp-stone-700">
        Free, local voice-to-text for everyone.
      </p>
      <p className="mt-2 max-w-[360px] text-center font-body text-[15px] leading-[1.7] text-chirp-stone-700">
        Your voice never leaves your device. No accounts. No cloud. No subscriptions.
      </p>

      <div className="mt-8">
        <Button size="onboarding" className="min-w-[180px]" onClick={onNext}>
          Get Started →
        </Button>
      </div>
    </div>
  )
}
