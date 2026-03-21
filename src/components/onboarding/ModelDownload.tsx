import { useState, useEffect, useRef } from 'react'
import { CheckCircle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { STT_MODELS } from '../../lib/constants'
import { Button } from '../shared/Button'

interface ModelDownloadProps {
  onFinish: () => void
}

function formatElapsed(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

export function ModelDownload({ onFinish }: ModelDownloadProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [state, setState] = useState<'pre' | 'downloading' | 'complete' | 'error'>('pre')
  const [error, setError] = useState<string | null>(null)
  const [elapsed, setElapsed] = useState(0)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isAlreadyDownloaded = store.modelDownloaded[store.model]

  // If model is already downloaded, jump straight to complete
  useEffect(() => {
    if (isAlreadyDownloaded && state === 'pre') {
      setState('complete')
    }
  }, [isAlreadyDownloaded, state])

  // Auto-advance 2s after completion
  useEffect(() => {
    if (state === 'complete') {
      const timer = setTimeout(onFinish, 2000)
      return () => clearTimeout(timer)
    }
  }, [state, onFinish])

  // Elapsed time counter
  useEffect(() => {
    if (state === 'downloading') {
      setElapsed(0)
      timerRef.current = setInterval(() => setElapsed((e) => e + 1), 1000)
    } else {
      if (timerRef.current) {
        clearInterval(timerRef.current)
        timerRef.current = null
      }
    }
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [state])

  const handleDownload = async () => {
    setState('downloading')
    setError(null)
    store.setModelDownloadProgress(0)
    try {
      await tauri.downloadModel(store.model, (progress) => {
        store.setModelDownloadProgress(progress)
      })
      store.updateSettings({
        modelDownloaded: { ...store.modelDownloaded, [store.model]: true },
      })
      store.setModelDownloadProgress(null)
      setState('complete')
    } catch {
      setError('Download failed. Please check your internet connection.')
      store.setModelDownloadProgress(null)
      setState('error')
    }
  }

  return (
    <div className="flex flex-col animate-fade-in">
      {state === 'complete' ? (
        <>
          <div className="flex items-center gap-2">
            <CheckCircle size={20} className="text-chirp-success" strokeWidth={1.5} />
            <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
              Speech model ready
            </h1>
          </div>
          <p className="mt-2 font-body text-sm text-chirp-stone-500">
            One more optional step...
          </p>
          <div className="mt-6">
            <Button size="onboarding" className="min-w-[160px] text-base" onClick={onFinish}>
              Continue
            </Button>
          </div>
        </>
      ) : (
        <>
          <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
            Download Speech Model
          </h1>

          {/* Model info */}
          <div className="flex items-center justify-between mt-3 px-1">
            <span className="font-mono text-sm font-medium text-chirp-stone-700">
              {currentModel?.name}
            </span>
            <span className="font-body text-sm text-chirp-stone-400">
              {currentModel?.size}
            </span>
          </div>
          <p className="font-body text-xs text-chirp-stone-400 mt-1 px-1">
            One-time download, runs entirely on your device
          </p>

          {/* Progress bar */}
          {state === 'downloading' && store.modelDownloadProgress !== null && (
            <div className="mt-5">
              <div className="h-2 rounded-full bg-chirp-stone-200 overflow-hidden">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200 relative overflow-hidden"
                  style={{ width: `${store.modelDownloadProgress}%` }}
                >
                  <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer" />
                </div>
              </div>
              <div className="flex justify-between mt-2">
                <span className="font-body text-sm text-chirp-stone-500">
                  {store.modelDownloadProgress < 96
                    ? `Downloading... ${store.modelDownloadProgress}%`
                    : 'Extracting files...'}
                </span>
                <span className="font-mono text-sm text-chirp-stone-400">
                  {formatElapsed(elapsed)} elapsed
                </span>
              </div>
              <button
                onClick={onFinish}
                className="mt-3 font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
              >
                Skip
              </button>
            </div>
          )}

          {/* Error state */}
          {state === 'error' && error && (
            <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
              <p className="font-body text-sm text-red-700">{error}</p>
            </div>
          )}

          {/* Download / Retry button */}
          {(state === 'pre' || state === 'error') && (
            <div className="mt-6">
              <Button size="onboarding" className="min-w-[160px] text-base" onClick={handleDownload}>
                {state === 'error' ? 'Retry Download' : 'Download Model'}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
