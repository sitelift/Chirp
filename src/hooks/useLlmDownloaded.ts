import { useEffect } from 'react'
import { useAppStore } from '../stores/appStore'
import { useTauri } from './useTauri'

/**
 * Returns [llmDownloaded, setLlmDownloaded] backed by the Zustand store so
 * every consumer sees the same value. On first mount we query the backend
 * once to seed the store; afterwards the modal/settings flows update it
 * synchronously on download completion.
 */
export function useLlmDownloaded() {
  const tauri = useTauri()
  const llmDownloaded = useAppStore((s) => s.llmDownloaded)
  const setLlmDownloaded = useAppStore((s) => s.setLlmDownloaded)

  useEffect(() => {
    tauri.getLlmStatus().then((status) => {
      setLlmDownloaded(status.binaryDownloaded && status.modelDownloaded)
    }).catch((e) => {
      if (import.meta.env.DEV) console.error('Failed to get LLM status:', e)
    })
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- one-time init

  return [llmDownloaded, setLlmDownloaded] as const
}
