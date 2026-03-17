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
      <span className="inline-flex items-center self-start rounded-full bg-chirp-amber-50 border border-chirp-amber-200 px-3 py-1 font-body text-xs text-chirp-amber-500 font-medium">
        STEP 1 OF 4
      </span>

      <h1 className="font-display font-extrabold text-3xl text-chirp-stone-900 mt-4">
        Welcome to Chirp
      </h1>

      <div className="flex flex-col gap-4 mt-6">
        {valueProps.map((text) => (
          <div key={text} className="flex items-start gap-3">
            <div className="w-2 h-2 rounded-full bg-chirp-amber-400 mt-2 shrink-0" />
            <span className="font-body text-[15px] leading-[1.7] text-chirp-stone-700">
              {text}
            </span>
          </div>
        ))}
      </div>

      <div className="mt-8">
        <Button size="onboarding" className="min-w-[180px] text-base" onClick={onNext}>
          Get Started →
        </Button>
      </div>
    </div>
  )
}
