import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from '../shared/BirdMark'
import { Welcome } from './Welcome'
import { Microphone } from './Microphone'
import { Hotkey } from './Hotkey'
import { ModelDownload } from './ModelDownload'

const STEPS = 4

export function Onboarding() {
  const [step, setStep] = useState(0)
  const setOnboardingComplete = useAppStore((s) => s.setOnboardingComplete)

  const handleFinish = () => {
    setOnboardingComplete(true)
  }

  return (
    <div className="flex h-screen">
      {/* Left branded panel */}
      <div className="w-[40%] bg-gradient-to-br from-chirp-amber-50 via-chirp-amber-100/50 to-chirp-stone-100 flex flex-col items-center justify-center relative overflow-hidden">
        {/* Decorative blurs */}
        <div className="absolute top-[-100px] right-[-100px] w-[300px] h-[300px] bg-chirp-amber-200/20 rounded-full blur-3xl" />
        <div className="absolute bottom-[-80px] left-[-80px] w-[250px] h-[250px] bg-chirp-amber-200/20 rounded-full blur-3xl" />

        {/* Ambient glow behind bird */}
        <div className="absolute w-64 h-64 bg-chirp-amber-200 rounded-full blur-3xl opacity-30" />

        <BirdMark size={120} />
        <span className="font-display font-extrabold text-4xl text-chirp-stone-900 mt-4 relative">
          chirp
        </span>
        <span className="font-body text-lg text-chirp-stone-500 mt-2 relative">
          Speak freely.
        </span>

        {/* Step indicator dots */}
        <div className="flex gap-2 mt-12 relative">
          {Array.from({ length: STEPS }, (_, i) => (
            <div
              key={i}
              className={`rounded-full transition-all duration-300 ease-out ${
                i === step
                  ? 'w-8 h-2 bg-chirp-amber-400'
                  : 'w-2 h-2 bg-chirp-stone-300'
              }`}
            />
          ))}
        </div>
      </div>

      {/* Right content panel */}
      <div className="w-[60%] bg-white flex items-center justify-center px-16">
        <div className="max-w-[480px] w-full">
          {step === 0 && <Welcome onNext={() => setStep(1)} />}
          {step === 1 && <Microphone onNext={() => setStep(2)} />}
          {step === 2 && <Hotkey onNext={() => setStep(3)} />}
          {step === 3 && <ModelDownload onFinish={handleFinish} />}
        </div>
      </div>
    </div>
  )
}
