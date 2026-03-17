import { useState, useEffect, useRef } from 'react'
import { Mic, Play, RotateCcw } from 'lucide-react'
import { useTauri } from '../../hooks/useTauri'
import { Button } from '../shared/Button'

interface MicTestProps {
  onNext: () => void
}

export function MicTest({ onNext }: MicTestProps) {
  const tauri = useTauri()
  const [state, setState] = useState<'idle' | 'recording' | 'playing' | 'done'>('idle')
  const [countdown, setCountdown] = useState(3)
  const [inputLevel, setInputLevel] = useState(0)
  const audioRef = useRef<HTMLAudioElement | null>(null)

  // Live input level polling during recording
  useEffect(() => {
    if (state !== 'recording') return
    const interval = setInterval(async () => {
      try {
        const level = await tauri.getInputLevel()
        setInputLevel(level)
      } catch {}
    }, 67)
    return () => clearInterval(interval)
  }, [state])

  const handleRecord = async () => {
    setState('recording')
    setCountdown(3)
    const interval = setInterval(() => {
      setCountdown((c) => {
        if (c <= 1) {
          clearInterval(interval)
          return 0
        }
        return c - 1
      })
    }, 1000)

    try {
      const wavBytes = await tauri.testMicrophone()
      clearInterval(interval)

      const uint8 = new Uint8Array(wavBytes)
      const blob = new Blob([uint8], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      setState('playing')
      const audio = new Audio(url)
      audioRef.current = audio
      audio.onended = () => {
        setState('done')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.onerror = () => {
        setState('done')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.play()
    } catch {
      clearInterval(interval)
      setState('idle')
    }
  }

  return (
    <div className="animate-fade-in">
      <h2 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Test your microphone
      </h2>
      <p className="font-body text-sm text-chirp-stone-500 mt-2">
        Let's make sure your microphone works. Record a short clip and listen back.
      </p>

      <div className="mt-8 flex flex-col items-center gap-6">
        {/* Level meter */}
        {state === 'recording' && (
          <div className="w-full">
            <div className="h-3 w-full overflow-hidden rounded-full bg-chirp-stone-200">
              <div
                className="h-full rounded-full bg-chirp-success transition-all duration-100"
                style={{ width: `${Math.min(100, inputLevel * 100)}%` }}
              />
            </div>
            <p className="font-body text-sm text-chirp-stone-500 mt-2 text-center">
              Recording... ({countdown}s)
            </p>
          </div>
        )}

        {state === 'playing' && (
          <div className="flex items-center gap-2">
            <Play size={18} className="text-chirp-amber-500 animate-pulse" />
            <p className="font-body text-sm text-chirp-stone-500">Playing back...</p>
          </div>
        )}

        {state === 'idle' && (
          <Button onClick={handleRecord} className="gap-2">
            <Mic size={18} />
            Record a test clip
          </Button>
        )}

        {state === 'done' && (
          <div className="flex flex-col items-center gap-4">
            <p className="font-body text-sm text-chirp-stone-700">
              Did you hear yourself clearly?
            </p>
            <div className="flex items-center gap-3">
              <Button variant="secondary" onClick={handleRecord} className="gap-2">
                <RotateCcw size={16} />
                Try again
              </Button>
              <Button onClick={onNext}>
                Sounds good
              </Button>
            </div>
          </div>
        )}
      </div>

      <button
        onClick={onNext}
        className="mt-8 block mx-auto font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
      >
        Skip
      </button>
    </div>
  )
}
