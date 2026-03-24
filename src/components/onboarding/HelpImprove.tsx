import { useState } from 'react'
import { BarChart3 } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { Toggle } from '../shared/Toggle'
import { Button } from '../shared/Button'

interface HelpImproveProps {
  onNext: () => void
}

export function HelpImprove({ onNext }: HelpImproveProps) {
  const store = useAppStore()
  const [opted, setOpted] = useState(false)

  const handleToggle = (value: boolean) => {
    setOpted(value)
    store.updateSettings({ helpImprove: value })
  }

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Help Improve Chirp
      </h1>
      {/* PLACEHOLDER COPY — needs human rewrite */}
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        Share anonymous usage stats and crash reports to help us make Chirp better.
      </p>

      <div className="rounded-lg border border-card-border bg-chirp-stone-50 p-4 mt-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <BarChart3 size={18} className="text-chirp-stone-400" />
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Anonymous analytics</div>
              <div className="text-[11px] text-chirp-stone-400 mt-0.5">
                No audio, no text, no personal info
              </div>
            </div>
          </div>
          <Toggle checked={opted} onChange={handleToggle} />
        </div>
      </div>

      <p className="font-body text-xs text-chirp-stone-400 mt-3">
        You can change this anytime in Settings.
      </p>

      <div className="mt-6 flex flex-col gap-2">
        <Button size="onboarding" className="min-w-[160px] text-base" onClick={onNext}>
          {opted ? 'Continue' : 'Skip'}
        </Button>
      </div>
    </div>
  )
}
