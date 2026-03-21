import { useState, useEffect, useRef } from 'react'
import { Sparkles, CheckCircle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { CLEANUP_EXAMPLE, LLM_MODEL } from '../../lib/constants'
import { Button } from '../shared/Button'

interface SmartCleanupProps {
  onNext: () => void
}

function formatElapsed(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

export function SmartCleanup({ onNext }: SmartCleanupProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [state, setState] = useState<'pre' | 'downloading' | 'starting' | 'complete' | 'error'>('pre')
  const [error, setError] = useState<string | null>(null)
  const [elapsed, setElapsed] = useState(0)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Auto-advance 2s after completion
  useEffect(() => {
    if (state === 'complete') {
      const timer = setTimeout(onNext, 2000)
      return () => clearTimeout(timer)
    }
  }, [state, onNext])

  // Elapsed time counter
  useEffect(() => {
    if (state === 'downloading' || state === 'starting') {
      if (!timerRef.current) {
        setElapsed(0) // eslint-disable-line react-hooks/set-state-in-effect -- reset timer on state change
        timerRef.current = setInterval(() => setElapsed((e) => e + 1), 1000)
      }
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

      // Auto-start server with 30s timeout
      setState('starting')
      try {
        const startPromise = tauri.startLlm()
        const timeoutPromise = new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('LLM start timed out')), 30000)
        )
        await Promise.race([startPromise, timeoutPromise])
        store.setLlmReady(true)
        setState('complete')
      } catch {
        // Kill any partially-started server to prevent orphan processes
        try { await tauri.stopLlm() } catch { /* ignore */ }
        setError('Smart Cleanup failed to start. You can retry or skip and enable it later in Settings.')
        setState('error')
      }
    } catch {
      setError('Download failed. Please check your internet connection.')
      store.setLlmDownloadProgress(null)
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
              You're all set!
            </h1>
          </div>
          <p className="mt-2 font-body text-sm text-chirp-stone-500">
            Your transcriptions will be polished automatically.
          </p>
          <div className="mt-6">
            <Button size="onboarding" className="min-w-[160px] text-base" onClick={onNext}>
              Start Using Chirp
            </Button>
          </div>
        </>
      ) : (
        <>
          <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
            Smart Cleanup
          </h1>
          <p className="mt-1 font-body text-sm text-chirp-stone-500">
            Automatically clean up grammar and filler words.
          </p>

          {/* Compact before/after */}
          <div className="rounded-lg border border-card-border bg-chirp-stone-50 p-3 mt-4 text-sm">
            <div className="flex gap-2">
              <span className="font-body text-[10px] font-semibold uppercase tracking-wide text-chirp-stone-400 shrink-0 mt-0.5">Before</span>
              <p className="font-body text-chirp-stone-500 italic">"{CLEANUP_EXAMPLE.before}"</p>
            </div>
            <div className="flex gap-2 mt-2 pt-2 border-t border-card-border">
              <span className="font-body text-[10px] font-semibold uppercase tracking-wide text-chirp-amber-500 shrink-0 mt-0.5">After</span>
              <p className="font-body text-chirp-stone-800">"{CLEANUP_EXAMPLE.after}"</p>
            </div>
          </div>

          <p className="font-body text-xs text-chirp-stone-400 mt-2">
            {LLM_MODEL.friendlySize} download, runs on your device
          </p>

          {/* Progress bar (downloading state) */}
          {(state === 'downloading' || state === 'starting') && store.llmDownloadProgress !== null && (
            <div className="mt-4">
              <div className="h-2 rounded-full bg-chirp-stone-200 overflow-hidden">
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
                    ? `Downloading... ${store.llmDownloadProgress}%`
                    : 'Extracting files...'}
                </span>
                <span className="font-mono text-sm text-chirp-stone-400">
                  {formatElapsed(elapsed)} elapsed
                </span>
              </div>
              <button
                onClick={onNext}
                className="mt-3 font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
              >
                Skip
              </button>
            </div>
          )}

          {state === 'starting' && store.llmDownloadProgress === null && (
            <div className="mt-4">
              <div className="flex items-center gap-2">
                <div className="h-2 w-2 rounded-full bg-chirp-amber-400 animate-pulse" />
                <span className="font-body text-sm text-chirp-stone-500">Starting Smart Cleanup...</span>
                <span className="font-mono text-sm text-chirp-stone-400 ml-auto">
                  {formatElapsed(elapsed)} elapsed
                </span>
              </div>
              <button
                onClick={onNext}
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

          {/* Action buttons */}
          {(state === 'pre' || state === 'error') && (
            <div className="mt-6 flex flex-col gap-2">
              <Button size="onboarding" className="min-w-[160px] text-base" onClick={handleEnable}>
                <Sparkles size={16} className="mr-2" />
                {state === 'error' ? 'Retry' : 'Turn on Smart Cleanup'}
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
