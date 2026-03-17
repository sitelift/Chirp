import { useState, useEffect } from 'react'
import { Sparkles, CheckCircle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { CLEANUP_EXAMPLE, LLM_MODEL } from '../../lib/constants'
import { Button } from '../shared/Button'

interface SmartCleanupProps {
  onNext: () => void
}

export function SmartCleanup({ onNext }: SmartCleanupProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [state, setState] = useState<'pre' | 'downloading' | 'starting' | 'complete' | 'error'>('pre')
  const [error, setError] = useState<string | null>(null)

  // Auto-advance 2s after completion
  useEffect(() => {
    if (state === 'complete') {
      const timer = setTimeout(onNext, 2000)
      return () => clearTimeout(timer)
    }
  }, [state, onNext])

  const handleEnable = async () => {
    store.updateSettings({ aiCleanup: true })
    setState('downloading')
    setError(null)
    store.setLlmDownloadProgress(0)
    try {
      await tauri.downloadLlm((progress) => {
        store.setLlmDownloadProgress(progress)
      })
      store.setLlmDownloadProgress(null)

      // Auto-start server
      setState('starting')
      try {
        await tauri.startLlm()
        store.setLlmReady(true)
      } catch {}

      setState('complete')
    } catch {
      setError('Download failed. Please check your internet connection.')
      store.setLlmDownloadProgress(null)
      setState('error')
    }
  }

  return (
    <div className="flex flex-col animate-fade-in">
      <span className="inline-flex items-center self-start rounded-full bg-chirp-amber-50 border border-chirp-amber-200 px-3 py-1 font-body text-xs text-chirp-amber-500 font-medium">
        STEP 6 OF 6
      </span>

      {/* Icon card */}
      <div
        className={`w-20 h-20 rounded-2xl flex items-center justify-center mt-6 border transition-colors duration-300 ${
          state === 'complete'
            ? 'bg-green-50 border-green-200'
            : state === 'downloading' || state === 'starting'
              ? 'bg-chirp-amber-50 border-chirp-amber-200 animate-pulse-gentle'
              : 'bg-chirp-amber-50 border-chirp-amber-200'
        }`}
      >
        {state === 'complete' ? (
          <CheckCircle size={32} className="text-chirp-success" strokeWidth={1.5} />
        ) : (
          <Sparkles size={32} className="text-chirp-amber-500" strokeWidth={1.5} />
        )}
      </div>

      {state === 'complete' ? (
        <>
          <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
            You're all set!
          </h1>
          <p className="mt-2 font-body text-[15px] text-chirp-stone-500">
            Smart Cleanup is ready. Your transcriptions will be polished automatically.
          </p>
          <div className="mt-8">
            <Button size="onboarding" className="w-full text-base" onClick={onNext}>
              Start Using Chirp
            </Button>
          </div>
        </>
      ) : (
        <>
          <h1 className="mt-6 font-display font-extrabold text-3xl text-chirp-stone-900">
            Make your text sound polished
          </h1>
          <p className="mt-2 font-body text-[15px] text-chirp-stone-500">
            Chirp can automatically clean up grammar, filler words, and messy sentences.
          </p>

          {/* Before/after example */}
          <div className="rounded-xl border border-chirp-stone-200 bg-chirp-stone-50 p-4 mt-5 flex flex-col gap-3">
            <div>
              <span className="font-body text-[10px] font-semibold uppercase tracking-[0.5px] text-chirp-stone-400">Before</span>
              <p className="font-body text-sm text-chirp-stone-500 mt-1 italic">
                "{CLEANUP_EXAMPLE.before}"
              </p>
            </div>
            <div className="border-t border-chirp-stone-200 pt-3">
              <span className="font-body text-[10px] font-semibold uppercase tracking-[0.5px] text-chirp-amber-500">After</span>
              <p className="font-body text-sm text-chirp-stone-900 mt-1">
                "{CLEANUP_EXAMPLE.after}"
              </p>
            </div>
          </div>

          {/* Model size note */}
          <p className="font-body text-xs text-chirp-stone-400 mt-3">
            {LLM_MODEL.friendlySize} download · runs entirely on your device
          </p>

          {/* Progress bar (downloading state) */}
          {(state === 'downloading' || state === 'starting') && store.llmDownloadProgress !== null && (
            <div className="mt-6">
              <div className="h-3 rounded-full bg-chirp-stone-200 overflow-hidden">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200 relative overflow-hidden"
                  style={{ width: `${store.llmDownloadProgress}%` }}
                >
                  <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer" />
                </div>
              </div>
              <div className="flex justify-between mt-2">
                <span className="font-body text-sm text-chirp-stone-500">
                  {store.llmDownloadProgress < 96
                    ? 'Downloading Smart Cleanup...'
                    : 'Extracting files...'}
                </span>
                <span className="font-mono text-sm font-medium text-chirp-stone-700">
                  {store.llmDownloadProgress}%
                </span>
              </div>
            </div>
          )}

          {state === 'starting' && store.llmDownloadProgress === null && (
            <div className="mt-6 flex items-center gap-2">
              <div className="h-2 w-2 rounded-full bg-chirp-amber-400 animate-pulse" />
              <span className="font-body text-sm text-chirp-stone-500">Starting Smart Cleanup...</span>
            </div>
          )}

          {/* Error state */}
          {state === 'error' && error && (
            <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
              <p className="font-body text-sm text-red-700">{error}</p>
            </div>
          )}

          {/* Action buttons */}
          {(state === 'pre' || state === 'error') && (
            <div className="mt-8 flex flex-col gap-3">
              <Button size="onboarding" className="w-full text-base" onClick={handleEnable}>
                <Sparkles size={16} className="mr-2" />
                {state === 'error' ? 'Retry Setup' : 'Turn on Smart Cleanup'}
              </Button>
              <button
                onClick={onNext}
                className="font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
              >
                Skip for now
              </button>
            </div>
          )}
        </>
      )}
    </div>
  )
}
