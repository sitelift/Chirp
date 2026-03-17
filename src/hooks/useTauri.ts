/**
 * Typed wrappers for Tauri commands.
 * Calls the real Rust backend via invoke().
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export interface AudioDevice {
  name: string
  id: string
}

export interface TranscriptionResult {
  text: string
  wordCount: number
  durationMs: number
}

export interface TranscriptionEntry {
  text: string
  timestamp: string
  wordCount: number
  durationMs: number
  speechDurationMs: number
}

export interface ModelStatus {
  model: string
  downloaded: boolean
  sizeBytes: number
}

export function useTauri() {
  const startRecording = async (): Promise<void> => {
    await invoke('start_recording')
  }

  const stopRecording = async (): Promise<TranscriptionResult> => {
    return await invoke<TranscriptionResult>('stop_recording')
  }

  const cancelRecording = async (): Promise<void> => {
    await invoke('cancel_recording')
  }

  const getAudioDevices = async (): Promise<AudioDevice[]> => {
    return await invoke<AudioDevice[]>('get_audio_devices')
  }

  const getInputLevel = async (): Promise<number> => {
    return await invoke<number>('get_input_level')
  }

  const downloadModel = async (
    model: string,
    onProgress?: (progress: number) => void
  ): Promise<void> => {
    // Listen for progress events before starting download
    let unlisten: (() => void) | undefined
    if (onProgress) {
      unlisten = await listen<number>('model-download-progress', (event) => {
        onProgress(event.payload)
      })
    }

    try {
      await invoke('download_model', { model })
    } finally {
      unlisten?.()
    }
  }

  const getModelStatus = async (model: string): Promise<ModelStatus> => {
    return await invoke<ModelStatus>('get_model_status', { model })
  }

  const updateSettings = async (settings: Record<string, unknown>): Promise<void> => {
    await invoke('update_settings', { partial: settings })
  }

  const getSettings = async (): Promise<Record<string, unknown>> => {
    return await invoke<Record<string, unknown>>('get_settings')
  }

  const updateDictionary = async (
    entries: Array<{ from: string; to: string }>
  ): Promise<void> => {
    await invoke('update_dictionary', { entries })
  }

  const getHistory = async (): Promise<TranscriptionEntry[]> => {
    return await invoke<TranscriptionEntry[]>('get_history')
  }

  const clearHistory = async (): Promise<void> => {
    await invoke('clear_history')
  }

  const deleteHistoryEntry = async (timestamp: string): Promise<void> => {
    await invoke('delete_history_entry', { timestamp })
  }

  const checkForUpdates = async (onProgress?: (downloaded: number, total: number | null) => void) => {
    const update = await check()
    if (!update) {
      return { available: false as const }
    }
    return {
      available: true as const,
      version: update.version,
      date: update.date,
      download: async () => {
        let downloaded = 0
        let contentLength: number | null = null
        await update.downloadAndInstall((event) => {
          if (event.event === 'Started') {
            contentLength = event.data.contentLength ?? null
          } else if (event.event === 'Progress') {
            downloaded += event.data.chunkLength
            onProgress?.(downloaded, contentLength)
          }
        })
      },
      relaunch,
    }
  }

  return {
    startRecording,
    stopRecording,
    cancelRecording,
    getAudioDevices,
    getInputLevel,
    downloadModel,
    getModelStatus,
    updateSettings,
    getSettings,
    updateDictionary,
    getHistory,
    clearHistory,
    deleteHistoryEntry,
    checkForUpdates,
  }
}
