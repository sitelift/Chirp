import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import type { Api } from './api'

/**
 * Updates download quietly and wait for a restart the user chooses (frame 11):
 * the tray menu and the Home row then offer "Restart to update".
 */
let pending: Update | null = null

export async function checkForUpdate(api: Api, onReady: (version: string) => void) {
  if (!api.live || pending) return
  try {
    const update = await check()
    if (!update) return
    await update.download()
    pending = update
    await api.call('set_update_ready', { version: update.version })
    onReady(update.version)
  } catch {
    // Offline or no release feed: try again on the next check.
  }
}

export async function installUpdate(api: Api) {
  if (!api.live || !pending) return
  await pending.install()
  await relaunch()
}
