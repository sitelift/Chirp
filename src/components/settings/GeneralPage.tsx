import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { TONE_MODES } from '../../lib/constants'
import { formatHotkey } from '../../lib/utils'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'
import { Checkbox } from '../shared/Checkbox'
import { Select } from '../shared/Select'
import { Toggle } from '../shared/Toggle'
import { KeyBadge } from '../shared/KeyBadge'

export function GeneralPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [capturing, setCapturing] = useState(false)
  const [llmDownloaded] = useLlmDownloaded()
  const [cleanupStarting, setCleanupStarting] = useState(false)

  const handleCleanupToggle = async (enabled: boolean) => {
    store.updateSettings({ aiCleanup: enabled })
    if (enabled && llmDownloaded && !store.llmReady) {
      setCleanupStarting(true)
      try {
        await tauri.startLlm()
        store.setLlmReady(true)
      } catch (e) { console.debug('Failed to start LLM:', e) }
      setCleanupStarting(false)
    } else if (!enabled && store.llmReady) {
      try {
        await tauri.stopLlm()
        store.setLlmReady(false)
      } catch (e) { console.debug('Failed to stop LLM:', e) }
    }
  }

  const hotkeyParts = formatHotkey(store.hotkey)

  const handleCaptureKey = async () => {
    setCapturing(true)
    try {
      const result = await tauri.captureHotkeyKey()
      if (result.keycode >= 0) {
        store.updateSettings({
          hotkeyKeycode: result.keycode,
          hotkeyKeyName: result.name,
        })
      }
    } catch (e) {
      console.debug('Key capture failed:', e)
    }
    setCapturing(false)
  }

  const handleCaptureShortcut = () => {
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
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Hotkey mode</p>
          <Select
            options={[
              { value: 'dedicated_key', label: 'Dedicated key (recommended)' },
              { value: 'custom', label: 'Custom shortcut' },
            ]}
            value={store.hotkeyMode}
            onChange={(v) => store.updateSettings({ hotkeyMode: v as 'dedicated_key' | 'custom' })}
          />
          {store.hotkeyMode === 'dedicated_key' ? (
            <div className="mt-3 space-y-3">
              {capturing ? (
                <div className="flex h-12 items-center justify-center rounded-xl border-2 border-dashed border-chirp-amber-400 bg-chirp-stone-100">
                  <span className="font-body text-sm text-chirp-stone-500">
                    Press any key...
                  </span>
                </div>
              ) : (
                <div className="flex items-center justify-between">
                  <KeyBadge keyLabel={store.hotkeyKeycode > 0 ? store.hotkeyKeyName : 'Not set'} />
                  <Button variant="secondary" onClick={handleCaptureKey}>
                    {store.hotkeyKeycode > 0 ? 'Change' : 'Set key'}
                  </Button>
                </div>
              )}
              {store.hotkeyKeycode > 0 && (
                <div className="space-y-1.5">
                  <div className="flex items-center gap-2">
                    <span className="font-body text-sm text-chirp-stone-500">Hold for push-to-talk</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="font-body text-sm text-chirp-stone-500">Double-tap for hands-free</span>
                  </div>
                </div>
              )}
              {store.hotkeyKeycode > 0 && store.hotkeyStatus === 'active' && (
                <div className="flex items-center gap-1.5">
                  <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                  <span className="font-body text-[13px] text-chirp-stone-500">Hotkey active</span>
                </div>
              )}
              {store.hotkeyStatus === 'retrying' && (
                <div className="flex items-center gap-1.5">
                  <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400 animate-pulse" />
                  <span className="font-body text-[13px] text-chirp-stone-500">Setting up hotkey...</span>
                </div>
              )}
              {store.hotkeyStatus === 'failed' && (
                <div className="space-y-2">
                  <div className="flex items-center gap-1.5">
                    <div className="h-1.5 w-1.5 rounded-full bg-red-400" />
                    <span className="font-body text-[13px] text-red-500">Input monitoring permission needed</span>
                  </div>
                  <p className="font-body text-[13px] text-chirp-stone-400">
                    Open System Settings → Privacy & Security → Input Monitoring, find Chirp, toggle off then on.
                  </p>
                  <Button variant="secondary" onClick={() => tauri.restartHotkeyListener()}>
                    Retry
                  </Button>
                </div>
              )}
            </div>
          ) : (
            <div className="mt-3">
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
                  <Button variant="secondary" onClick={handleCaptureShortcut}>
                    Change
                  </Button>
                </div>
              )}
              <p className="font-body text-[13px] text-chirp-stone-400 mt-2">
                Hold to talk, release to transcribe.
              </p>
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
        <Checkbox
          checked={store.playSoundOnComplete}
          onChange={(v) => store.updateSettings({ playSoundOnComplete: v })}
          label="Play sound on completion"
        />
        <Checkbox
          checked={store.showPassiveOverlay}
          onChange={(v) => store.updateSettings({ showPassiveOverlay: v })}
          label="Show passive overlay indicator"
          description="Small icon on screen when Chirp is ready"
        />
        <div>
          <p className="font-body text-sm text-chirp-stone-700 mb-2">Overlay position</p>
          <Select
            options={[
              { value: 'bottom', label: 'Bottom center' },
              { value: 'top', label: 'Top center' },
            ]}
            value={store.overlayPosition}
            onChange={(v) => store.updateSettings({ overlayPosition: v as 'bottom' | 'top' })}
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
        <div className="flex items-center justify-between">
          <div className="flex flex-col flex-1 mr-3">
            <div className="flex items-center gap-2">
              <span className="font-body text-sm text-chirp-stone-700">Smart Cleanup</span>
              {store.aiCleanup && (
                <span className="flex items-center gap-1">
                  {cleanupStarting ? (
                    <>
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400 animate-pulse" />
                      <span className="font-body text-[11px] text-chirp-stone-400">Getting ready...</span>
                    </>
                  ) : store.llmReady ? (
                    <>
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                      <span className="font-body text-[11px] text-chirp-stone-400">Active</span>
                    </>
                  ) : !llmDownloaded ? (
                    <button
                      onClick={() => store.setSettingsPage('model')}
                      className="flex items-center gap-1 group"
                    >
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400" />
                      <span className="font-body text-[11px] text-chirp-amber-500 group-hover:underline">Setup needed</span>
                    </button>
                  ) : null}
                </span>
              )}
            </div>
            <span className="font-body text-[13px] text-chirp-stone-500 mt-0.5">
              Polish grammar and filler words automatically
            </span>
          </div>
          <Toggle
            checked={store.aiCleanup}
            onChange={handleCleanupToggle}
            disabled={cleanupStarting}
          />
        </div>
        {store.aiCleanup && (
          <div>
            <p className="font-body text-sm text-chirp-stone-700 mb-2">Tone mode</p>
            <Select
              options={TONE_MODES.map(m => ({ value: m.id, label: m.label }))}
              value={store.toneMode}
              onChange={(v) => store.updateSettings({ toneMode: String(v) })}
            />
            <p className="font-body text-[13px] text-chirp-stone-500 mt-1.5">
              {TONE_MODES.find(m => m.id === store.toneMode)?.description}
            </p>
          </div>
        )}
      </SettingGroup>
    </div>
  )
}
