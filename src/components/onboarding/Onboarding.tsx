import { useState } from 'react'
import { trackEvent } from '@aptabase/tauri'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from '../shared/BirdMark'
import { Welcome } from './Welcome'
import { SetupStep } from './SetupStep'
import { ModelDownload } from './ModelDownload'
import { HelpImprove } from './HelpImprove'

const STEPS = 4

export function Onboarding() {
  const [step, setStep] = useState(0)
  const setOnboardingComplete = useAppStore((s) => s.setOnboardingComplete)

  const handleFinish = () => {
    trackEvent('onboarding_completed', { steps_completed: String(STEPS) })
    setOnboardingComplete(true)
  }

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Left branded panel */}
      <div className="w-[40%] bg-sidebar flex flex-col items-center justify-center relative overflow-hidden">
        <div className="absolute w-72 h-72 rounded-full blur-3xl opacity-20 bg-gradient-to-br from-chirp-yellow via-chirp-amber-400 to-chirp-amber-600 animate-float-1" />

        <div className="relative">
          <div className="absolute inset-0 w-[120px] h-[120px] rounded-full blur-2xl opacity-40 bg-chirp-yellow" />
          <BirdMark size={120} className="relative" />
        </div>

        <span className="font-display font-black text-4xl text-white mt-4 relative">
          chirp
        </span>
        <span className="font-body text-lg text-white/40 mt-2 relative">
          Speak freely.
        </span>

        <div className="flex gap-2 mt-12 relative">
          {Array.from({ length: STEPS }, (_, i) => (
            <div
              key={i}
              className={`rounded-full transition-all duration-300 ease-out h-2 ${
                i === step
                  ? 'w-8 bg-chirp-yellow'
                  : i < step
                    ? 'w-2 bg-chirp-yellow/50'
                    : 'w-2 bg-white/20'
              }`}
            />
          ))}
        </div>
      </div>

      {/* Right content panel */}
      <div className="w-[60%] bg-surface flex items-center justify-center px-16 overflow-hidden">
        <div className="max-w-[480px] w-full">
          {step === 0 && <Welcome onNext={() => setStep(1)} />}
          {step === 1 && <SetupStep onNext={() => setStep(2)} />}
          {step === 2 && <ModelDownload onFinish={() => setStep(3)} />}
          {step === 3 && <HelpImprove onNext={handleFinish} />}
        </div>
      </div>
    </div>
  )
}
