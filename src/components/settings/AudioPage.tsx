import { useState, useEffect } from 'react'
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
  const [testCountdown, setTestCountdown] = useState(5)
  const [inputLevel, setInputLevel] = useState(0)

  useEffect(() => {
    tauri.getAudioDevices().then(setDevices)
  }, [])

  // Live input level polling
  useEffect(() => {
    let running = true
    const poll = async () => {
      if (!running) return
      const level = await tauri.getInputLevel()
      setInputLevel(level)
      if (running) requestAnimationFrame(poll)
    }
    poll()
    return () => { running = false }
  }, [])

  const handleTest = async () => {
    setTestState('recording')
    setTestCountdown(5)
    const interval = setInterval(() => {
      setTestCountdown((c) => {
        if (c <= 1) {
          clearInterval(interval)
          return 0
        }
        return c - 1
      })
    }, 1000)

    await new Promise((r) => setTimeout(r, 5000))
    clearInterval(interval)
    setTestState('playing')
    await new Promise((r) => setTimeout(r, 2000))
    setTestState('idle')
  }

  const testButtonText =
    testState === 'recording'
      ? `Recording... (${testCountdown}s)`
      : testState === 'playing'
        ? 'Playing back...'
        : 'Test Microphone'

  return (
    <div className="flex flex-col gap-6">
      <SettingGroup label="Input Device">
        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Microphone</p>
          <Select
            options={devices.map((d) => ({ value: d.id, label: d.name }))}
            value={store.inputDevice}
            onChange={(v) => store.updateSettings({ inputDevice: v as string })}
          />
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

        <Button
          variant="secondary"
          onClick={handleTest}
          disabled={testState !== 'idle'}
        >
          {testButtonText}
        </Button>
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
