import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/appStore'
import type { TranscriptionEntry } from '../stores/appStore'
import { useTauri } from './useTauri'

// Settings keys that should be synced to the backend
const SYNCED_KEYS = [
  'hotkey', 'launchAtLogin', 'playSoundOnComplete',
  'autoDismissOverlay', 'smartFormatting',
  'inputDevice', 'model', 'onboardingComplete',
  'aiCleanup',
  'toneMode',
  'overlayPosition',
  'showPassiveOverlay',
  'historyRetentionDays',
] as const

/**
 * Loads settings from the Rust backend on mount and syncs changes back.
 * Also listens for cross-window settings-changed events to keep all windows in sync.
 */
export function useSettingsSync() {
  const tauri = useTauri()
  const updateSettings = useAppStore((s) => s.updateSettings)
  const loaded = useRef(false)
  // Guard to prevent re-syncing changes that came from this window
  const suppressSync = useRef(false)

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
      console.error('Failed to load settings:', e)
      useAppStore.getState().setSettingsLoaded()
    })

    // Load transcription history
    tauri.getHistory().then((entries) => {
      useAppStore.getState().setHistory(entries)
    }).catch((e) => console.error('Failed to load history:', e))

    // Load snippets
    tauri.getSnippets().then((entries) => {
      useAppStore.getState().setSnippets(entries)
    }).catch((e) => console.error('Failed to load snippets:', e))

    // Load initial hotkey status
    tauri.getHotkeyStatus().then((status) => {
      useAppStore.getState().setHotkeyStatus(status as 'idle' | 'retrying' | 'active' | 'failed')
    }).catch((e) => console.error('Failed to get hotkey status:', e))

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
      }).catch((e) => console.error('Failed to get model status:', e))
    }

    // Initialize LLM ready state from backend
    tauri.getLlmStatus().then((status) => {
      if (status.serverRunning) {
        useAppStore.getState().setLlmReady(true)
      }
    }).catch((e) => console.error('Failed to get LLM status:', e))
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- one-time init

  // Always-active: sync store changes back to backend + listen for events.
  useEffect(() => {
    const unlisteners: Array<() => void> = []

    // Listen for hotkey status changes from the backend
    listen<string>('hotkey-status', (event) => {
      useAppStore.getState().setHotkeyStatus(event.payload as 'idle' | 'retrying' | 'active' | 'failed')
    }).then((fn) => unlisteners.push(fn))

    // Listen for new transcription entries from the backend (cross-window)
    listen<TranscriptionEntry>('history-updated', (event) => {
      const state = useAppStore.getState()
      state.setHistory([...state.history, event.payload])
    }).then((fn) => unlisteners.push(fn))

    // Listen for history pruning (e.g. retention change)
    listen('history-changed', () => {
      tauri.getHistory().then((entries) => {
        useAppStore.getState().setHistory(entries)
      }).catch((e) => console.error('Failed to reload history:', e))
    }).then((fn) => unlisteners.push(fn))

    // Listen for settings changes from other windows (cross-window sync)
    listen<Record<string, unknown>>('settings-changed', (event) => {
      const partial = event.payload
      if (partial && typeof partial === 'object' && Object.keys(partial).length > 0) {
        // Apply changes to this window's store without re-syncing back to Rust
        suppressSync.current = true
        useAppStore.getState().updateSettings(partial as Partial<ReturnType<typeof useAppStore.getState>>)
        // Reset suppress flag after a tick to allow future local changes to sync
        setTimeout(() => { suppressSync.current = false }, 0)
      }
    }).then((fn) => unlisteners.push(fn))

    // Subscribe to store changes and sync settings + dictionary to backend
    const unsub = useAppStore.subscribe((state, prevState) => {
      if (!state.settingsLoaded) return
      if (suppressSync.current) return

      const changed: Record<string, unknown> = {}
      for (const key of SYNCED_KEYS) {
        if (state[key] !== prevState[key]) {
          changed[key] = state[key]
        }
      }
      if (Object.keys(changed).length > 0) {
        invoke('update_settings', { partial: changed }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => console.error('Failed to sync settings:', e))
      }

      // Sync dictionary changes
      if (state.dictionary !== prevState.dictionary) {
        invoke('update_dictionary', { entries: state.dictionary }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => console.error('Failed to sync dictionary:', e))
      }

      // Sync snippet changes
      if (state.snippets !== prevState.snippets) {
        invoke('update_snippets', { entries: state.snippets }).then(() => {
          useAppStore.getState().setSettingsSaved(true)
        }).catch((e) => console.error('Failed to sync snippets:', e))
      }
    })

    return () => {
      unsub()
      unlisteners.forEach((fn) => fn())
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- event subscriptions registered once
}
