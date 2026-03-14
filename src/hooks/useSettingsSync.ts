import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../stores/appStore'
import { useTauri } from './useTauri'

// Settings keys that should be synced to the backend
const SYNCED_KEYS = [
  'hotkey', 'launchAtLogin', 'showInMenuBar', 'playSoundOnComplete',
  'autoDismissOverlay', 'silenceTimeout', 'language', 'smartFormatting',
  'inputDevice', 'noiseSuppression', 'whisperModel', 'onboardingComplete',
] as const

/**
 * Loads settings from the Rust backend on mount and syncs changes back.
 */
export function useSettingsSync() {
  const tauri = useTauri()
  const updateSettings = useAppStore((s) => s.updateSettings)
  const loaded = useRef(false)

  useEffect(() => {
    if (loaded.current) return
    loaded.current = true

    // Load settings from backend
    tauri.getSettings().then((settings) => {
      if (settings && Object.keys(settings).length > 0) {
        updateSettings(settings as Partial<ReturnType<typeof useAppStore.getState>>)
      }
    }).catch((e) => {
      console.warn('Failed to load settings:', e)
    })

    // Check model download status for all models
    for (const model of ['tiny', 'base', 'small', 'medium']) {
      tauri.getModelStatus(model).then((status) => {
        if (status.downloaded) {
          updateSettings({
            modelDownloaded: {
              ...useAppStore.getState().modelDownloaded,
              [status.model]: true,
            },
          })
        }
      }).catch(() => {})
    }

    // Subscribe to store changes and sync settings + dictionary to backend
    const unsub = useAppStore.subscribe((state, prevState) => {
      const changed: Record<string, unknown> = {}
      for (const key of SYNCED_KEYS) {
        if (state[key] !== prevState[key]) {
          changed[key] = state[key]
        }
      }
      if (Object.keys(changed).length > 0) {
        invoke('update_settings', { partial: changed }).catch((e) => {
          console.warn('Failed to sync settings:', e)
        })
      }

      // Sync dictionary changes
      if (state.dictionary !== prevState.dictionary) {
        invoke('update_dictionary', { entries: state.dictionary }).catch((e) => {
          console.warn('Failed to sync dictionary:', e)
        })
      }
    })

    return () => unsub()
  }, [])
}
