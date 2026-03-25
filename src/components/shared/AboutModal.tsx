import { useState, useCallback, useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'
import { open } from '@tauri-apps/plugin-shell'
import { Heart } from 'lucide-react'
import { useTauri } from '../../hooks/useTauri'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from './BirdMark'
import { Button } from './Button'

type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'current' | 'error'

export function AboutModal() {
  const tauri = useTauri()
  const aboutModalOpen = useAppStore((s) => s.aboutModalOpen)
  const setAboutModalOpen = useAppStore((s) => s.setAboutModalOpen)

  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState<string | null>(null)
  const [doRelaunch, setDoRelaunch] = useState<(() => Promise<void>) | null>(null)
  const [appVersion, setAppVersion] = useState('...')

  useEffect(() => { getVersion().then(setAppVersion) }, [])

  const handleCheckUpdates = useCallback(async () => {
    setUpdateStatus('checking')
    try {
      const result = await tauri.checkForUpdates((downloaded, total) => {
        if (total) {
          const pct = Math.round((downloaded / total) * 100)
          setDownloadProgress(`${pct}%`)
        } else {
          setDownloadProgress(`${(downloaded / 1024 / 1024).toFixed(1)} MB`)
        }
      })

      if (!result.available) {
        setUpdateStatus('current')
        setTimeout(() => setUpdateStatus('idle'), 3000)
        return
      }

      setUpdateVersion(result.version)
      setUpdateStatus('available')
      setDoRelaunch(() => result.relaunch)

      // Auto-download
      setUpdateStatus('downloading')
      await result.download()
      setUpdateStatus('ready')
    } catch {
      setUpdateStatus('error')
    }
  }, [tauri])

  useEffect(() => {
    const unlisten = listen('check-for-updates', () => {
      handleCheckUpdates()
    })
    return () => { unlisten.then((f) => f()) }
  }, [handleCheckUpdates])

  const handleRelaunch = async () => {
    if (doRelaunch) await doRelaunch()
  }

  if (!aboutModalOpen) return null

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={() => setAboutModalOpen(false)}
    >
      <div
        className="relative bg-white rounded-2xl shadow-xl max-w-md w-full mx-4 p-8"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close button */}
        <button
          onClick={() => setAboutModalOpen(false)}
          className="absolute top-4 right-4 w-8 h-8 flex items-center justify-center rounded-lg text-[#888] hover:text-[#555] hover:bg-[#F5F4F0] transition-colors"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M1 1L13 13M1 13L13 1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>

        <div className="flex flex-col items-center">
          <BirdMark size={64} />

          <h2 className="mt-3 font-display font-extrabold text-[28px] text-[#1a1a1a]">
            chirp
          </h2>

          <span className="mt-1 font-mono text-[13px] text-[#888]">v{appVersion}</span>

          <p className="mt-3 font-body text-sm text-[#888] italic text-center">
            Free, local voice-to-text{'\n'}for everyone.
          </p>

          <a
            href="https://trychirp.app"
            target="_blank"
            rel="noopener noreferrer"
            className="mt-2 font-mono text-[13px] text-chirp-info hover:underline"
          >
            trychirp.app
          </a>

          <div className="mt-4">
            {updateStatus === 'ready' ? (
              <Button onClick={handleRelaunch}>
                Restart to Update
              </Button>
            ) : (
              <Button
                variant="secondary"
                onClick={handleCheckUpdates}
                disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
              >
                {updateStatus === 'checking'
                  ? 'Checking...'
                  : updateStatus === 'downloading'
                    ? `Downloading... ${downloadProgress ?? ''}`
                    : 'Check for Updates'}
              </Button>
            )}
          </div>

          {updateStatus === 'available' && (
            <p className="mt-2 font-body text-sm text-[#1a1a1a]">
              Update available!{' '}
              <span className="font-mono text-[13px]">v{updateVersion}</span>
            </p>
          )}
          {updateStatus === 'current' && (
            <p className="mt-2 font-body text-sm text-chirp-success">
              You're on the latest version.
            </p>
          )}
          {updateStatus === 'error' && (
            <p className="mt-2 font-body text-sm text-[#888]">
              Couldn't check for updates.
            </p>
          )}

          {/* Support Chirp */}
          <button
            onClick={() => open('https://buymeacoffee.com/chirpapp')}
            className="mt-4 inline-flex items-center gap-1.5 font-body text-[13px] text-chirp-stone-400 hover:text-chirp-amber-500 transition-colors"
          >
            <Heart size={14} strokeWidth={1.5} />
            Support Chirp
          </button>

          {/* Credits */}
          <div className="mt-6 w-full rounded-xl bg-[#F5F4F0] p-4">
            <div className="flex flex-col items-center gap-1.5">
              <p className="font-body text-[13px] text-[#888]">
                Made by Pieter de Bruijn
              </p>
              <p className="font-body text-[13px] text-[#888]">
                Speech recognition: Parakeet TDT — NVIDIA (sherpa-onnx)
              </p>
              <p className="font-body text-[13px] text-[#888]">
                Smart Cleanup: Qwen 2.5 — Alibaba
              </p>
              <p className="font-body text-[13px] text-[#888]">
                Built with Tauri + React
              </p>
              <p className="font-body text-[12px] text-[#aaa] mt-2 text-center">
                All processing happens on your device. Your voice and text never leave your machine.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
