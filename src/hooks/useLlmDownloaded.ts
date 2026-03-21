import { useState, useEffect } from 'react'
import { useTauri } from './useTauri'

export function useLlmDownloaded() {
  const tauri = useTauri()
  const [llmDownloaded, setLlmDownloaded] = useState(false)

  useEffect(() => {
    tauri.getLlmStatus().then((status) => {
      setLlmDownloaded(status.binaryDownloaded && status.modelDownloaded)
    }).catch((e) => console.error('Failed to get LLM status:', e))
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- one-time init

  return [llmDownloaded, setLlmDownloaded] as const
}
