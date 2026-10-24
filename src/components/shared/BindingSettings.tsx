import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { useHotkeyRecorder } from '../../hooks/useHotkeyRecorder'
import { invoke } from '@tauri-apps/api/core'
import type { Binding, BindingTarget } from '../../lib/hotkeyCapture'

export function BindingSettings() {
  const store = useAppStore()
  const { capturing, error, record, cancelCapture } = useHotkeyRecorder()
  const [saveError, setSaveError] = useState<string | null>(null)
  const save = async (target: BindingTarget, binding: Binding | null) => {
    const partial = target === 'hold' ? { holdBinding: binding } : { toggleBinding: binding }
    try {
      await invoke('update_settings', { partial })
      setSaveError(null)
    } catch (e) { setSaveError(String(e)) }
  }
  return <div className="space-y-4 p-4">
    {(['hold', 'toggle'] as const).map(target => {
      const binding = target === 'hold' ? store.holdBinding : store.toggleBinding
      const modifierOnly = binding?.keys.every(k => [0xa0,0xa1,0xa2,0xa3,0xa4,0xa5,0x5b,0x5c].includes(k.vk))
      return <div key={target} className="space-y-2">
        <div className="flex items-center justify-between gap-3">
          <span>{target === 'hold' ? 'Hold to talk' : 'Toggle recording'}</span>
          <span>{binding?.label ?? 'Disabled'}</span>
          <button disabled={capturing !== null} onClick={async () => {
            const next = await record(target)
            if (next) await save(target, next)
          }}>Record</button>
          <button disabled={!binding || capturing !== null} onClick={() => void save(target, null)}>Disable</button>
        </div>
        {binding && <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" disabled={modifierOnly || capturing !== null} checked={!modifierOnly && binding.suppress}
            onChange={e => void save(target, { ...binding, suppress: e.target.checked })} />
          Suppress shortcut in other apps{modifierOnly ? ' (unavailable for modifier-only bindings)' : ''}
        </label>}
      </div>
    })}
    {capturing && <div role="status">Press a combination, then release all keys. Esc cancels. <button onClick={cancelCapture}>Cancel</button></div>}
    {(error || saveError) && <div role="alert">{error || saveError}</div>}
  </div>
}
