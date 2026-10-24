import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { Binding, BindingTarget } from '../lib/hotkeyCapture'

export function useHotkeyRecorder() {
  const [capturing, setCapturing] = useState<BindingTarget | null>(null)
  const [error, setError] = useState<string | null>(null)
  const generation = useRef(0)
  const active = useRef(false)
  useEffect(() => () => {
    generation.current++
    if (active.current) void invoke('cancel_record_binding').catch(() => {})
  }, [])
  const cancelCapture = () => {
    generation.current++
    active.current = false
    setCapturing(null)
    void invoke('cancel_record_binding').catch(() => {})
  }
  const record = async (target: BindingTarget): Promise<Binding | null> => {
    if (active.current) return null
    active.current = true
    const id = ++generation.current
    setCapturing(target)
    setError(null)
    try {
      const result = await invoke<Binding>('record_binding', { target })
      return id === generation.current ? result : null
    } catch (e) {
      if (id === generation.current) setError(String(e))
      return null
    } finally {
      if (id === generation.current) { active.current = false; setCapturing(null) }
    }
  }
  return { capturing, error, record, cancelCapture }
}
