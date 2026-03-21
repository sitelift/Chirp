import { invoke } from '@tauri-apps/api/core'

export async function playCompletionSound() {
  try {
    await invoke('play_completion_sound')
  } catch {
    // Sound playback is best-effort — failure is not critical
  }
}
