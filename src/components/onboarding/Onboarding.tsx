import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { Welcome } from './Welcome'
import { Microphone } from './Microphone'
import { Hotkey } from './Hotkey'

const STEPS = 3

export function Onboarding() {
  const [step, setStep] = useState(0)
  const setOnboardingComplete = useAppStore((s) => s.setOnboardingComplete)

  const handleFinish = () => {
    setOnboardingComplete(true)
  }

  return (
    <div className="flex h-screen flex-col items-center bg-white">
      {/* Content */}
      <div className="flex flex-1 items-center justify-center">
        <div className="max-w-[400px] w-full px-6">
          {step === 0 && <Welcome onNext={() => setStep(1)} />}
          {step === 1 && <Microphone onNext={() => setStep(2)} />}
          {step === 2 && <Hotkey onFinish={handleFinish} />}
        </div>
      </div>

      {/* Step dots */}
      <div className="flex items-center gap-2 pb-8">
        {Array.from({ length: STEPS }, (_, i) => (
          <div
            key={i}
            className={`h-2 w-2 rounded-full transition-colors duration-200 ease-out ${
              i === step ? 'bg-chirp-amber-400' : 'bg-chirp-stone-300'
            }`}
          />
        ))}
      </div>
    </div>
  )
}
