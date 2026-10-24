import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/** The backend as the surface sees it. The preview harness swaps in a mock. */
export interface Api {
  live: boolean
  call<T = void>(command: string, args?: Record<string, unknown>): Promise<T>
  /** Subscribe to a backend event; returns an unsubscribe function. */
  on<T>(event: string, handler: (payload: T) => void): () => void
}

export const tauriApi: Api = {
  live: true,
  call: (command, args) => invoke(command, args),
  on: (event, handler) => {
    let off: (() => void) | null = null
    let cancelled = false
    listen(event, (e) => handler(e.payload as never)).then((fn) => {
      if (cancelled) fn()
      else off = fn
    })
    return () => {
      cancelled = true
      off?.()
    }
  },
}

export const mockApi: Api = {
  live: false,
  call: async () => undefined as never,
  on: () => () => {},
}
