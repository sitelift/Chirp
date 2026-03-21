import { Button } from '../shared/Button'

interface WelcomeProps {
  onNext: () => void
}

export function Welcome({ onNext }: WelcomeProps) {
  const valueProps = [
    'Free, local voice-to-text for everyone',
    'Your voice never leaves your device',
    'No accounts, no cloud, no subscriptions',
  ]

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Welcome to Chirp
      </h1>

      <div className="flex flex-col gap-3 mt-4">
        {valueProps.map((text) => (
          <div key={text} className="flex items-start gap-2.5">
            <div className="w-1.5 h-1.5 rounded-full bg-chirp-amber-400 mt-2 shrink-0" />
            <span className="font-body text-sm text-chirp-stone-600">
              {text}
            </span>
          </div>
        ))}
      </div>

      <div className="mt-6">
        <Button size="onboarding" className="min-w-[160px] text-base" onClick={onNext}>
          Get Started
        </Button>
      </div>
    </div>
  )
}
