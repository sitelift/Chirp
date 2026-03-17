import { invoke } from '@tauri-apps/api/core'

export async function playCompletionSound() {
  try {
    await invoke('play_completion_sound')
  } catch (e) {
    console.warn('Failed to play completion sound:', e)
  }
}
