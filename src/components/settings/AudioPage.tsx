import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import type { AudioDevice } from '../../hooks/useTauri'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'
import { Select } from '../shared/Select'
import { Checkbox } from '../shared/Checkbox'

export function AudioPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [testState, setTestState] = useState<'idle' | 'recording' | 'playing'>('idle')
  const [testCountdown, setTestCountdown] = useState(3)
  const [inputLevel, setInputLevel] = useState(0)
  const [devicesLoading, setDevicesLoading] = useState(true)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  useEffect(() => {
    tauri.getAudioDevices().then(setDevices).finally(() => setDevicesLoading(false))
  }, [])

  // Live input level polling (~15fps)
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const level = await tauri.getInputLevel()
        setInputLevel(level)
      } catch { /* device may be unavailable */ }
    }, 67)
    return () => clearInterval(interval)
  }, [])

  const handleTest = async () => {
    setTestState('recording')
    setTestCountdown(3)
    const interval = setInterval(() => {
      setTestCountdown((c) => {
        if (c <= 1) {
          clearInterval(interval)
          return 0
        }
        return c - 1
      })
    }, 1000)

    try {
      const wavBytes = await tauri.testMicrophone()
      clearInterval(interval)

      // Convert number array to WAV blob and play it back
      const uint8 = new Uint8Array(wavBytes)
      const blob = new Blob([uint8], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      setTestState('playing')
      const audio = new Audio(url)
      audioRef.current = audio
      audio.onended = () => {
        setTestState('idle')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.onerror = () => {
        setTestState('idle')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.play()
    } catch {
      clearInterval(interval)
      setTestState('idle')
    }
  }

  const testButtonText =
    testState === 'recording'
      ? `Recording... (${testCountdown}s)`
      : testState === 'playing'
        ? 'Playing back...'
        : 'Test Microphone'

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">Audio</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">Input device and audio processing.</p>
      </div>

      <SettingGroup label="Input Device">
        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Microphone</p>
          {devicesLoading ? (
            <p className="font-body text-sm text-chirp-stone-400">Loading devices...</p>
          ) : (
            <Select
              options={devices.map((d) => ({ value: d.id, label: d.name }))}
              value={store.inputDevice}
              onChange={(v) => store.updateSettings({ inputDevice: v as string })}
            />
          )}
        </div>

        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Input Level</p>
          <div className="h-3 w-full overflow-hidden rounded-full bg-chirp-stone-200">
            <div
              className="h-full rounded-full bg-chirp-success transition-all duration-100"
              style={{ width: `${Math.min(100, inputLevel * 100)}%` }}
            />
          </div>
        </div>

        <div>
          <Button
            variant="secondary"
            onClick={handleTest}
            disabled={testState !== 'idle'}
          >
            {testButtonText}
          </Button>
          <p className="font-body text-xs text-chirp-stone-400 mt-1.5">
            Records 3 seconds, then plays it back so you can hear yourself.
          </p>
        </div>
      </SettingGroup>

      <SettingGroup label="Processing">
        <Checkbox
          checked={store.noiseSuppression}
          onChange={(v) => store.updateSettings({ noiseSuppression: v })}
          label="Noise suppression"
          description="Reduces background noise before transcription"
        />
      </SettingGroup>
    </div>
  )
}
