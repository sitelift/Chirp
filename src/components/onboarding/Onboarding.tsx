import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from '../shared/BirdMark'
import { Welcome } from './Welcome'
import { SetupStep } from './SetupStep'
import { ModelDownload } from './ModelDownload'
import { SmartCleanup } from './SmartCleanup'

const STEPS = 4

export function Onboarding() {
  const [step, setStep] = useState(0)
  const setOnboardingComplete = useAppStore((s) => s.setOnboardingComplete)

  const handleFinish = () => {
    setOnboardingComplete(true)
  }

  return (
    <div className="h-screen overflow-hidden bg-surface flex items-center justify-center">
      <div className="max-w-lg w-full mx-auto bg-white rounded-[18px] shadow-lg p-8">
        {/* Logo */}
        <div className="flex items-center gap-2 mb-6">
          <BirdMark size={24} />
          <span className="font-display font-black text-lg text-chirp-stone-900">
            chirp
          </span>
        </div>

        {/* Step content */}
        {step === 0 && <Welcome onNext={() => setStep(1)} />}
        {step === 1 && <SetupStep onNext={() => setStep(2)} />}
        {step === 2 && <ModelDownload onFinish={() => setStep(3)} />}
        {step === 3 && <SmartCleanup onNext={handleFinish} />}

        {/* Progress dots */}
        <div className="flex justify-center gap-2 mt-6">
          {Array.from({ length: STEPS }, (_, i) => (
            <div
              key={i}
              className={`rounded-full transition-all duration-300 ease-out h-1.5 ${
                i === step
                  ? 'w-6 bg-chirp-amber-400'
                  : i < step
                    ? 'w-1.5 bg-chirp-amber-300'
                    : 'w-1.5 bg-chirp-stone-200'
              }`}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
