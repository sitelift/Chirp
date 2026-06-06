import { useState } from 'react'
import { Sparkles, Download } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { LLM_MODEL } from '../../lib/constants'
import { BirdMark } from './BirdMark'
import { Button } from './Button'

export function UpgradeModal() {
  const store = useAppStore()
  const tauri = useTauri()
  const upgradeModalOpen = useAppStore((s) => s.upgradeModalOpen)
  const setUpgradeModalOpen = useAppStore((s) => s.setUpgradeModalOpen)
  const [, setLlmDownloaded] = useLlmDownloaded()

  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  if (!upgradeModalOpen) return null

  const handleDownload = async () => {
    setDownloading(true)
    setError(null)
    setProgress(0)
    try {
      await tauri.downloadLlm((p) => setProgress(p))
      setLlmDownloaded(true)

      try {
        await tauri.startLlm()
        store.setLlmReady(true)
      } catch {
        // Non-fatal
      }

      store.updateSettings({ aiCleanup: true })
      setUpgradeModalOpen(false)
    } catch {
      setError('Download failed. Check your internet connection and try again.')
    } finally {
      setDownloading(false)
      setProgress(null)
    }
  }

  const handleSkip = () => {
    store.updateSettings({ aiCleanup: false })
    setUpgradeModalOpen(false)
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={handleSkip}
    >
      <div
        className="relative bg-card rounded-[20px] shadow-xl max-w-[380px] w-full mx-4 overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Warm gradient header */}
        <div className="relative bg-gradient-to-b from-chirp-amber-400/8 to-transparent pt-10 pb-6 flex flex-col items-center">
          <BirdMark size={52} />

          <div className="mt-5 flex items-center gap-2">
            <Sparkles size={14} className="text-chirp-amber-500" />
            <span className="font-display font-extrabold text-[11px] tracking-widest uppercase text-chirp-amber-500">
              New in Chirp
            </span>
          </div>

          <h2 className="mt-3 font-display font-extrabold text-[24px] text-dm-primary leading-tight text-center px-8">
            Enable
            <br />
            Smart Cleanup
          </h2>
        </div>

        <div className="px-8 pb-8">
          <div className="flex flex-col items-center text-center">
            <p className="font-body text-[14px] text-dm-muted leading-relaxed">
              Smart Cleanup uses a small on-device language model to remove
              filler words, fix stutters, and resolve self-corrections in
              your dictations — all without sending audio anywhere.
            </p>

            <p className="mt-6 font-body text-[12px] text-dm-secondary">
              One-time download · {LLM_MODEL.friendlySize} · 100% on-device
            </p>

            {/* Progress bar */}
            {downloading && progress !== null && (
              <div className="mt-5 w-full">
                <div className="h-1.5 w-full bg-surface rounded-full overflow-hidden">
                  <div
                    className="h-full bg-chirp-amber-400 rounded-full transition-all duration-300"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <p className="mt-2 font-mono text-[12px] text-dm-muted">
                  {progress}%
                </p>
              </div>
            )}

            {error && (
              <p className="mt-3 font-body text-[13px] text-red-500">{error}</p>
            )}

            {/* Actions */}
            <div className="mt-5 flex flex-col gap-2.5 w-full">
              <Button
                onClick={handleDownload}
                disabled={downloading}
                className="w-full gap-2"
              >
                {downloading ? (
                  'Downloading...'
                ) : (
                  <>
                    <Download size={14} />
                    Upgrade Smart Cleanup
                  </>
                )}
              </Button>
              <button
                onClick={handleSkip}
                disabled={downloading}
                className="font-body text-[13px] text-dm-secondary hover:text-dm-primary transition-colors disabled:opacity-50"
              >
                Maybe later
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
