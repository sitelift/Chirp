import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/appStore'

const SYNCED_KEYS = [
  'hotkey', 'hotkeyMode', 'launchAtLogin', 'playSoundOnComplete',
  'autoDismissOverlay', 'smartFormatting',
  'inputDevice', 'model', 'onboardingComplete',
  'aiCleanup', 'cleanupModel', 'beamSearch', 'toneMode',
  'overlayPosition', 'showPassiveOverlay',
  'historyRetentionDays', 'helpImprove', 'darkMode',
] as const

function isIpcNotReady(err: unknown): boolean {
  const s = String(err)
  return s.includes('__TAURI_IPC__') || s.includes('__TAURI_INTERNALS__')
}

/**
 * Lightweight settings sync for the overlay and tray-popup windows.
 *
 * Unlike useSettingsSync, this only hydrates settings from the backend —
 * no history / vocabulary / snippets / model-status / llm-status fetches.
 * The initial invoke retries until Tauri's IPC shim is injected, which
 * avoids the RUST-R crash ("window.__TAURI_IPC__ is not a function") that
 * shows up as "Something went wrong" on the overlay at cold start.
 */
export function useOverlaySync() {
  const updateSettings = useAppStore((s) => s.updateSettings)
  const loaded = useRef(false)
  const suppressCount = useRef(0)

  useEffect(() => {
    if (loaded.current) return
    loaded.current = true

    let cancelled = false

    async function loadSettings() {
      for (let attempt = 0; attempt < 20 && !cancelled; attempt++) {
        try {
          const settings = await invoke<Record<string, unknown>>('get_settings')
          if (settings && Object.keys(settings).length > 0) {
            updateSettings(settings as Partial<ReturnType<typeof useAppStore.getState>>)
          }
          useAppStore.getState().setSettingsLoaded()

          try {
            const status = await invoke<string>('get_hotkey_status')
            useAppStore.getState().setHotkeyStatus(
              status as 'idle' | 'retrying' | 'active' | 'failed'
            )
          } catch {
            // non-fatal
          }
          return
        } catch (e) {
          if (isIpcNotReady(e)) {
            await new Promise((r) => setTimeout(r, 100))
            continue
          }
          if (import.meta.env.DEV) console.error('Failed to load settings:', e)
          useAppStore.getState().setSettingsLoaded()
          return
        }
      }
      // Gave up after ~2s of retries — unblock UI anyway
      useAppStore.getState().setSettingsLoaded()
    }

    loadSettings()

    return () => {
      cancelled = true
    }
  }, [updateSettings])

  useEffect(() => {
    const unlisteners: Array<() => void> = []

    listen<string>('hotkey-status', (event) => {
      useAppStore.getState().setHotkeyStatus(
        event.payload as 'idle' | 'retrying' | 'active' | 'failed'
      )
    }).then((fn) => unlisteners.push(fn))

    listen<Record<string, unknown>>('settings-changed', (event) => {
      const partial = event.payload
      if (partial && typeof partial === 'object' && Object.keys(partial).length > 0) {
        suppressCount.current++
        useAppStore.getState().updateSettings(
          partial as Partial<ReturnType<typeof useAppStore.getState>>
        )
        setTimeout(() => { suppressCount.current-- }, 0)
      }
    }).then((fn) => unlisteners.push(fn))

    const unsub = useAppStore.subscribe((state, prevState) => {
      if (!state.settingsLoaded) return
      if (suppressCount.current > 0) return

      const changed: Record<string, unknown> = {}
      for (const key of SYNCED_KEYS) {
        if (state[key] !== prevState[key]) {
          changed[key] = state[key]
        }
      }
      if (Object.keys(changed).length > 0) {
        invoke('update_settings', { partial: changed }).catch((e) => {
          if (import.meta.env.DEV) console.error('Failed to sync settings:', e)
        })
      }
    })

    return () => {
      unsub()
      unlisteners.forEach((fn) => fn())
    }
  }, [])
}
