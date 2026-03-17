import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { SILENCE_TIMEOUT_OPTIONS } from '../../lib/constants'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'
import { Checkbox } from '../shared/Checkbox'
import { Select } from '../shared/Select'
import { Toggle } from '../shared/Toggle'
import { KeyBadge } from '../shared/KeyBadge'

export function GeneralPage() {
  const store = useAppStore()
  const [capturing, setCapturing] = useState(false)

  const hotkeyParts = store.hotkey
    .replace('CmdOrCtrl', navigator.platform.includes('Mac') ? '\u2318' : 'Ctrl')
    .split('+')

  const handleCapture = () => {
    setCapturing(true)
    const handler = (e: KeyboardEvent) => {
      e.preventDefault()
      if (e.key === 'Escape') {
        setCapturing(false)
        window.removeEventListener('keydown', handler)
        return
      }
      const parts: string[] = []
      if (e.ctrlKey || e.metaKey) parts.push('CmdOrCtrl')
      if (e.shiftKey) parts.push('Shift')
      if (e.altKey) parts.push('Alt')
      if (!['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
        parts.push(e.key.toUpperCase())
      }
      if (parts.length > 1) {
        store.updateSettings({ hotkey: parts.join('+') })
        setCapturing(false)
        window.removeEventListener('keydown', handler)
      }
    }
    window.addEventListener('keydown', handler)
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">General</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">Hotkey, behavior, and output preferences.</p>
      </div>

      <SettingGroup label="Hotkey">
        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-3">Dictation toggle</p>
          {capturing ? (
            <div className="flex h-12 items-center justify-center rounded-xl border-2 border-dashed border-chirp-amber-400 bg-chirp-stone-100">
              <span className="font-body text-sm text-chirp-stone-500">
                Press new shortcut...
              </span>
            </div>
          ) : (
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5">
                {hotkeyParts.map((part, i) => (
                  <KeyBadge key={i} keyLabel={part} />
                ))}
              </div>
              <Button variant="secondary" onClick={handleCapture}>
                Change
              </Button>
            </div>
          )}
        </div>
      </SettingGroup>

      <SettingGroup label="Behavior">
        <Checkbox
          checked={store.launchAtLogin}
          onChange={(v) => store.updateSettings({ launchAtLogin: v })}
          label="Launch at login"
        />
        <Checkbox
          checked={store.showInMenuBar}
          onChange={(v) => store.updateSettings({ showInMenuBar: v })}
          label="Show in menu bar"
        />
        <Checkbox
          checked={store.autoDismissOverlay}
          onChange={(v) => store.updateSettings({ autoDismissOverlay: v })}
          label="Auto-dismiss overlay"
        />
        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Silence timeout</p>
          <Select
            options={SILENCE_TIMEOUT_OPTIONS.map((o) => ({
              value: o.value,
              label: o.label,
            }))}
            value={store.silenceTimeout}
            onChange={(v) => store.updateSettings({ silenceTimeout: v as number })}
          />
        </div>
      </SettingGroup>

      <SettingGroup label="Output">
        <div className="flex items-center justify-between">
          <div className="flex flex-col">
            <span className="font-body text-sm text-chirp-stone-700">Smart formatting</span>
            <span className="font-body text-[13px] text-chirp-stone-500 mt-0.5">
              Automatically format lists, paragraphs, and structure
            </span>
          </div>
          <Toggle
            checked={store.smartFormatting}
            onChange={(v) => store.updateSettings({ smartFormatting: v })}
          />
        </div>
      </SettingGroup>
    </div>
  )
}
