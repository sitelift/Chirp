import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/appStore'
import type { TranscriptionEntry } from '../stores/appStore'
import { useTauri } from './useTauri'

// Settings keys that should be synced to the backend
const SYNCED_KEYS = [
  'hotkey', 'launchAtLogin', 'showInMenuBar', 'playSoundOnComplete',
  'autoDismissOverlay', 'smartFormatting',
  'inputDevice', 'noiseSuppression', 'model', 'onboardingComplete',
  'aiCleanup',
  'toneMode',
  'overlayPosition',
  'showPassiveOverlay',
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
      useAppStore.getState().setSettingsLoaded()
    }).catch((e) => {
      console.debug('Failed to load settings:', e)
      useAppStore.getState().setSettingsLoaded()
    })

    // Load transcription history
    tauri.getHistory().then((entries) => {
      useAppStore.getState().setHistory(entries)
    }).catch((e) => {
      console.debug('Failed to load history:', e)
    })

    // Load snippets
    tauri.getSnippets().then((entries) => {
      useAppStore.getState().setSnippets(entries)
    }).catch((e) => {
      console.debug('Failed to load snippets:', e)
    })

    // Check model download status
    for (const model of ['parakeet-tdt-0.6b']) {
      tauri.getModelStatus(model).then((status) => {
        if (status.downloaded) {
          updateSettings({
            modelDownloaded: {
              ...useAppStore.getState().modelDownloaded,
              [status.model]: true,
            },
          })
        }
      }).catch((e) => {
        console.debug('Failed to check model status:', e)
      })
    }
  }, [])

  // Always-active: sync store changes back to backend + listen for events.
  // This is intentionally in a separate useEffect WITHOUT the loaded guard
  // so that React StrictMode's remount correctly recreates the subscription.
  useEffect(() => {
    const unlisteners: Array<() => void> = []

    // Listen for new transcription entries from the backend (cross-window)
    listen<TranscriptionEntry>('history-updated', (event) => {
      const state = useAppStore.getState()
      state.setHistory([...state.history, event.payload])
    }).then((fn) => unlisteners.push(fn))

    // Subscribe to store changes and sync settings + dictionary to backend
    // Guard: don't sync until backend settings have been loaded to avoid
    // overwriting saved values with zustand defaults during startup.
    const unsub = useAppStore.subscribe((state, prevState) => {
      if (!state.settingsLoaded) return

      const changed: Record<string, unknown> = {}
      for (const key of SYNCED_KEYS) {
        if (state[key] !== prevState[key]) {
          changed[key] = state[key]
        }
      }
      if (Object.keys(changed).length > 0) {
        invoke('update_settings', { partial: changed }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => {
          console.debug('Failed to sync settings:', e)
        })
      }

      // Sync dictionary changes
      if (state.dictionary !== prevState.dictionary) {
        invoke('update_dictionary', { entries: state.dictionary }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => {
          console.debug('Failed to sync dictionary:', e)
        })
      }

      // Sync snippet changes
      if (state.snippets !== prevState.snippets) {
        invoke('update_snippets', { entries: state.snippets }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => {
          console.debug('Failed to sync snippets:', e)
        })
      }
    })

    return () => {
      unsub()
      unlisteners.forEach((fn) => fn())
    }
  }, [])
}
