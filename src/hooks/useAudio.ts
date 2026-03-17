import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../stores/appStore'

interface AmplitudeData {
  bars: number[]
}

/**
 * Subscribes to real amplitude data from the Rust backend.
 * The backend emits 'amplitude-data' events ~60fps during recording.
 * Also listens for live transcription interim results.
 */
export function useAudio() {
  const status = useAppStore((s) => s.status)
  const setAmplitudes = useAppStore((s) => s.setAmplitudes)
  const setInputLevel = useAppStore((s) => s.setInputLevel)
  const setLiveTranscription = useAppStore((s) => s.setLiveTranscription)

  useEffect(() => {
    if (status !== 'listening') {
      setAmplitudes([])
      setLiveTranscription('')
      return
    }

    const unlisteners: Array<() => void> = []

    listen<AmplitudeData>('amplitude-data', (event) => {
      const bars = event.payload.bars
      setAmplitudes(bars)
      if (bars.length > 0) {
        setInputLevel(bars.reduce((a, b) => a + b, 0) / bars.length)
      }
    }).then((fn) => unlisteners.push(fn))

    listen<string>('transcription-interim', (event) => {
      setLiveTranscription(event.payload)
    }).then((fn) => unlisteners.push(fn))

    return () => {
      unlisteners.forEach((fn) => fn())
    }
  }, [status, setAmplitudes, setInputLevel, setLiveTranscription])
}
