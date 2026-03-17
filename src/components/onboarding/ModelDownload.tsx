import { useState, useEffect } from 'react'
import { Download, CheckCircle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { STT_MODELS } from '../../lib/constants'
import { Button } from '../shared/Button'

interface ModelDownloadProps {
  onFinish: () => void
}

export function ModelDownload({ onFinish }: ModelDownloadProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [state, setState] = useState<'pre' | 'downloading' | 'complete' | 'error'>('pre')
  const [error, setError] = useState<string | null>(null)

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
      <span className="inline-flex items-center self-start rounded-full bg-chirp-amber-50 border border-chirp-amber-200 px-3 py-1 font-body text-xs text-chirp-amber-500 font-medium">
        STEP 5 OF 6
      </span>

      {/* Icon card */}
      <div
        className={`w-20 h-20 rounded-2xl flex items-center justify-center mt-6 border transition-colors duration-300 ${
          state === 'complete'
            ? 'bg-green-50 border-green-200'
            : state === 'downloading'
              ? 'bg-chirp-amber-50 border-chirp-amber-200 animate-pulse-gentle'
              : 'bg-chirp-amber-50 border-chirp-amber-200'
        }`}
      >
        {state === 'complete' ? (
          <CheckCircle size={32} className="text-chirp-success" strokeWidth={1.5} />
        ) : (
          <Download size={32} className="text-chirp-amber-500" strokeWidth={1.5} />
        )}
      </div>

      {state === 'complete' ? (
        <>
          <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
            Speech model ready!
          </h1>
          <p className="mt-2 font-body text-[15px] text-chirp-stone-500">
            Chirp can now transcribe your voice. One more optional step...
          </p>
          <div className="mt-8">
            <Button size="onboarding" className="w-full text-base" onClick={onFinish}>
              Continue
            </Button>
          </div>
        </>
      ) : (
        <>
          <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
            Download Speech Model
          </h1>

          {/* Model info card */}
          <div className="rounded-lg bg-chirp-stone-50 border border-chirp-stone-200 p-4 mt-4">
            <div className="flex items-center justify-between">
              <span className="font-mono text-sm font-medium text-chirp-stone-900">
                {currentModel?.name}
              </span>
              <span className="font-body text-sm text-chirp-stone-500">
                {currentModel?.size}
              </span>
            </div>
            <p className="font-body text-xs text-chirp-stone-400 mt-1">
              One-time download · runs entirely on your device
            </p>
          </div>

          {/* Progress bar (downloading state) */}
          {state === 'downloading' && store.modelDownloadProgress !== null && (
            <div className="mt-6">
              <div className="h-3 rounded-full bg-chirp-stone-200 overflow-hidden">
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
                    ? 'Downloading model...'
                    : 'Extracting files...'}
                </span>
                <span className="font-mono text-sm font-medium text-chirp-stone-700">
                  {store.modelDownloadProgress}%
                </span>
              </div>
              <p className="font-body text-xs text-chirp-stone-400 mt-2">
                This may take a few minutes
              </p>
            </div>
          )}

          {/* Error state */}
          {state === 'error' && error && (
            <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
              <p className="font-body text-sm text-red-700">{error}</p>
              <p className="font-body text-xs text-red-500 mt-1">Check your internet connection</p>
            </div>
          )}

          {/* Download / Retry button */}
          {(state === 'pre' || state === 'error') && (
            <div className="mt-8">
              <Button size="onboarding" className="w-full text-base" onClick={handleDownload}>
                {state === 'error' ? 'Retry Download' : 'Download Model'}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
