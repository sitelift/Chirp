/**
 * Typed wrappers for Tauri commands.
 * Calls the real Rust backend via invoke().
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface AudioDevice {
  name: string
  id: string
}

export interface TranscriptionResult {
  text: string
  wordCount: number
  durationMs: number
}

export interface ModelStatus {
  model: string
  downloaded: boolean
  sizeBytes: number
}

export interface UpdateInfo {
  available: boolean
  version: string | null
  url: string | null
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

  const checkForUpdates = async (): Promise<UpdateInfo> => {
    return await invoke<UpdateInfo>('check_for_updates')
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
    checkForUpdates,
  }
}
