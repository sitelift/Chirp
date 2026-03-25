import { useState, useEffect, useRef, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { CheckCircle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { STT_MODELS, LLM_MODEL } from '../../lib/constants'
import { Button } from '../shared/Button'

interface ModelDownloadProps {
  onFinish: () => void
}

function formatElapsed(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${m}:${s.toString().padStart(2, '0')}`
}

type Phase = 'checking' | 'stt' | 'stt-done' | 'llm' | 'llm-starting' | 'complete' | 'error'

export function ModelDownload({ onFinish }: ModelDownloadProps) {
  const store = useAppStore()
  const tauri = useTauri()
  const [phase, setPhase] = useState<Phase>('checking')
  const [error, setError] = useState<string | null>(null)
  const [elapsed, setElapsed] = useState(0)
  const [unifiedProgress, setUnifiedProgress] = useState(0)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const startedRef = useRef(false)

  const currentModel = STT_MODELS.find((m) => m.id === store.model)

  // Elapsed time counter
  useEffect(() => {
    if (phase === 'stt' || phase === 'llm' || phase === 'llm-starting') {
      if (!timerRef.current) {
        setElapsed(0)
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
  }, [phase])

  // Auto-advance 2s after full completion
  useEffect(() => {
    if (phase === 'complete') {
      const timer = setTimeout(onFinish, 2000)
      return () => clearTimeout(timer)
    }
  }, [phase, onFinish])

  const startLlmDownload = useCallback(async () => {
    setPhase('llm')
    try {
      // Listen for LLM download progress
      const unlisten = await listen<number>('llm-download-progress', (event) => {
        const llmProgress = event.payload
        setUnifiedProgress(Math.round(18 + llmProgress * 0.82))
      })

      try {
        await tauri.downloadLlm()
      } finally {
        unlisten()
      }

      // LLM downloaded, start server
      setPhase('llm-starting')
      setUnifiedProgress(100)
      store.updateSettings({ aiCleanup: true })

      try {
        const startPromise = tauri.startLlm()
        const timeoutPromise = new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('LLM start timed out')), 30000)
        )
        await Promise.race([startPromise, timeoutPromise])
        store.setLlmReady(true)
      } catch {
        // Non-fatal: LLM can be started later
        try { await tauri.stopLlm() } catch { /* ignore */ }
      }

      setPhase('complete')
    } catch {
      setError('Smart Cleanup download failed. You can retry or continue without it.')
      setPhase('stt-done')
    }
  }, [tauri, store])

  // Main download flow
  useEffect(() => {
    if (startedRef.current) return
    startedRef.current = true

    const run = async () => {
      try {
        // Check STT model status
        const sttDownloaded = store.modelDownloaded[store.model]

        // Check LLM status
        const llmStatus = await tauri.getLlmStatus()
        const llmReady = llmStatus.binaryDownloaded && llmStatus.modelDownloaded

        if (sttDownloaded && llmReady) {
          setUnifiedProgress(100)
          setPhase('complete')
          return
        }

        if (sttDownloaded) {
          // Skip STT, go to LLM
          setUnifiedProgress(18)
          setPhase('stt-done')
          await startLlmDownload()
          return
        }

        // Download STT first
        setPhase('stt')
        setUnifiedProgress(0)

        const unlisten = await listen<number>('model-download-progress', (event) => {
          const sttProgress = event.payload
          setUnifiedProgress(Math.round(sttProgress * 0.18))
        })

        try {
          await tauri.downloadModel(store.model)
        } finally {
          unlisten()
        }

        store.updateSettings({
          modelDownloaded: { ...store.modelDownloaded, [store.model]: true },
        })

        setUnifiedProgress(18)
        setPhase('stt-done')

        // Start LLM download immediately
        await startLlmDownload()
      } catch {
        setError('Download failed. Please check your internet connection.')
        setPhase('error')
      }
    }

    run()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- one-time init

  const canContinue = phase === 'stt-done' || phase === 'llm' || phase === 'llm-starting' || phase === 'complete'

  return (
    <div className="flex flex-col animate-fade-in">
      {phase === 'complete' ? (
        <>
          <div className="flex items-center gap-2">
            <CheckCircle size={20} className="text-chirp-success" strokeWidth={1.5} />
            <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
              All models ready
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
            Download Models
          </h1>

          {/* Model info */}
          <div className="mt-3 px-1">
            <div className="flex items-center justify-between">
              <span className="font-mono text-sm font-medium text-chirp-stone-700">
                {currentModel?.name}
              </span>
              <span className="font-body text-sm text-chirp-stone-400">
                {currentModel?.size}
              </span>
            </div>
            <div className="flex items-center justify-between mt-1">
              <span className="font-mono text-sm font-medium text-chirp-stone-700">
                {LLM_MODEL.displayName} ({LLM_MODEL.name})
              </span>
              <span className="font-body text-sm text-chirp-stone-400">
                {LLM_MODEL.friendlySize}
              </span>
            </div>
          </div>
          <p className="font-body text-xs text-chirp-stone-400 mt-2 px-1">
            One-time download, everything runs on your device
          </p>

          {/* Progress bar */}
          {(phase === 'checking' || phase === 'stt' || phase === 'stt-done' || phase === 'llm' || phase === 'llm-starting') && (
            <div className="mt-5">
              <div className="h-2 rounded-full bg-chirp-stone-200 overflow-hidden">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200 relative overflow-hidden"
                  style={{ width: `${unifiedProgress}%` }}
                >
                  <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer" />
                </div>
              </div>
              <div className="flex justify-between mt-2">
                <span className="font-body text-sm text-chirp-stone-500">
                  {phase === 'checking'
                    ? 'Checking models...'
                    : phase === 'stt'
                      ? `Downloading speech model... ${unifiedProgress}%`
                      : phase === 'stt-done'
                        ? 'Basic transcription is ready!'
                        : phase === 'llm'
                          ? `Downloading Smart Cleanup... ${unifiedProgress}%`
                          : phase === 'llm-starting'
                            ? 'Starting Smart Cleanup...'
                            : `${unifiedProgress}%`}
                </span>
                {(phase === 'stt' || phase === 'llm' || phase === 'llm-starting') && (
                  <span className="font-mono text-sm text-chirp-stone-400">
                    {formatElapsed(elapsed)} elapsed
                  </span>
                )}
              </div>

              {/* Ready message + continue button after STT */}
              {canContinue && (
                <div className="mt-4">
                  {phase === 'stt-done' && (
                    <p className="font-body text-sm text-chirp-success mb-3">
                      Basic transcription is ready!
                    </p>
                  )}
                  <Button size="onboarding" className="min-w-[160px] text-base" onClick={onFinish}>
                    Start using Chirp
                  </Button>
                </div>
              )}

              {/* Skip link */}
              {!canContinue && (
                <button
                  onClick={onFinish}
                  className="mt-3 font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
                >
                  Skip
                </button>
              )}
            </div>
          )}

          {/* Error state */}
          {phase === 'error' && error && (
            <div className="rounded-lg bg-red-50 border border-red-200 p-3 mt-4">
              <p className="font-body text-sm text-red-700">{error}</p>
              <div className="mt-3">
                <Button
                  size="onboarding"
                  className="min-w-[160px] text-base"
                  onClick={() => {
                    startedRef.current = false
                    setError(null)
                    setPhase('checking')
                    setUnifiedProgress(0)
                    startedRef.current = false
                    // Re-trigger by forcing re-mount isn't possible, so just call run inline
                    window.location.reload()
                  }}
                >
                  Retry
                </Button>
                <button
                  onClick={onFinish}
                  className="ml-4 font-body text-sm text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors"
                >
                  Skip
                </button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
