import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'
import { Button } from '../shared/Button'

interface PermissionsStepProps {
  onNext: () => void
}

export function PermissionsStep({ onNext }: PermissionsStepProps) {
  const [accessibilityGranted, setAccessibilityGranted] = useState(false)
  const [micGranted, setMicGranted] = useState(false)

  useEffect(() => {
    let active = true
    const poll = async () => {
      while (active) {
        try {
          const acc = await invoke<boolean>('check_accessibility_permission')
          if (active) setAccessibilityGranted(acc)
        } catch { /* ignore */ }
        try {
          const mic = await invoke<boolean>('request_mic_permission')
          if (active) setMicGranted(mic)
        } catch { /* ignore */ }
        await new Promise(r => setTimeout(r, 1000))
      }
    }
    poll()
    return () => { active = false }
  }, [])

  const handleOpenAccessibility = async () => {
    try {
      await invoke('request_accessibility_permission')
    } catch { /* ignore */ }
    await open('x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility')
  }

  const handleRequestMic = async () => {
    try {
      const result = await invoke<boolean>('request_mic_permission')
      setMicGranted(result)
    } catch { /* ignore */ }
  }

  const bothGranted = accessibilityGranted && micGranted

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Grant permissions
      </h1>
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        Chirp needs two permissions to work.
      </p>

      <div className="mt-6 space-y-3">
        {/* Accessibility */}
        <div className="flex items-center justify-between rounded-xl border border-card-border bg-white px-4 py-3">
          <div className="flex items-center gap-3">
            <div className={`h-2.5 w-2.5 rounded-full ${accessibilityGranted ? 'bg-chirp-success' : 'bg-chirp-stone-300'}`} />
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Accessibility</div>
              <div className="text-[11px] text-chirp-stone-500">Required to detect your hotkey and paste text</div>
            </div>
          </div>
          {!accessibilityGranted && (
            <button
              onClick={handleOpenAccessibility}
              className="rounded-lg bg-[#1a1a1a] px-3 py-1.5 text-[11px] font-medium text-white hover:bg-[#333] transition-colors"
            >
              Open System Settings
            </button>
          )}
        </div>

        {/* Microphone */}
        <div className="flex items-center justify-between rounded-xl border border-card-border bg-white px-4 py-3">
          <div className="flex items-center gap-3">
            <div className={`h-2.5 w-2.5 rounded-full ${micGranted ? 'bg-chirp-success' : 'bg-chirp-stone-300'}`} />
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Microphone</div>
              <div className="text-[11px] text-chirp-stone-500">Required to hear your voice</div>
            </div>
          </div>
          {!micGranted && (
            <button
              onClick={handleRequestMic}
              className="rounded-lg bg-[#1a1a1a] px-3 py-1.5 text-[11px] font-medium text-white hover:bg-[#333] transition-colors"
            >
              Request Access
            </button>
          )}
        </div>
      </div>

      <div className="mt-6 flex items-center gap-3">
        <Button
          size="onboarding"
          className="min-w-[140px] text-base"
          onClick={onNext}
          disabled={!bothGranted}
        >
          Continue
        </Button>
        <button
          onClick={onNext}
          className="font-body text-xs text-chirp-stone-400 hover:text-chirp-stone-600 hover:underline transition-colors"
        >
          Skip for now
        </button>
      </div>
    </div>
  )
}
