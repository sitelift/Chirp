import { useState } from 'react'
import { useAppStore } from '../stores/appStore'
import { useTauri } from './useTauri'
import { useLlmDownloaded } from './useLlmDownloaded'

/**
 * Shared logic for toggling Smart Cleanup on/off.
 * Used by both HomePage and SettingsPage to avoid duplication.
 */
export function useCleanupToggle() {
  const store = useAppStore()
  const tauri = useTauri()
  const [llmDownloaded] = useLlmDownloaded()
  const [cleanupStarting, setCleanupStarting] = useState(false)

  const handleCleanupToggle = async (enabled: boolean) => {
    if (enabled && !llmDownloaded) {
      // Model not downloaded — show upgrade modal to trigger download
      store.setUpgradeModalOpen(true)
      return
    }

    store.updateSettings({ aiCleanup: enabled })
    if (enabled && llmDownloaded && !store.llmReady) {
      setCleanupStarting(true)
      try {
        await tauri.startLlm()
        store.setLlmReady(true)
      } catch (e) {
        if (import.meta.env.DEV) console.error('Failed to start LLM:', e)
      }
      setCleanupStarting(false)
    } else if (!enabled && store.llmReady) {
      try {
        await tauri.stopLlm()
        store.setLlmReady(false)
      } catch (e) {
        if (import.meta.env.DEV) console.error('Failed to stop LLM:', e)
      }
    }
  }

  return { handleCleanupToggle, cleanupStarting, llmDownloaded }
}
