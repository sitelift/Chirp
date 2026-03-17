import { useState, useEffect, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useTauri } from '../../hooks/useTauri'
import { BirdMark } from '../shared/BirdMark'
import { Button } from '../shared/Button'

type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'current' | 'error'

export function AboutPage() {
  const tauri = useTauri()
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState<string | null>(null)
  const [doRelaunch, setDoRelaunch] = useState<(() => Promise<void>) | null>(null)

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

  return (
    <div className="flex flex-col">
      <div className="mb-8">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">About Chirp</h1>
      </div>

      <div className="flex flex-col items-center pt-4">
        <BirdMark size={64} />

        <h2 className="mt-3 font-display font-extrabold text-[28px] text-chirp-stone-900">
          chirp
        </h2>

        <span className="mt-1 font-mono text-[13px] text-chirp-stone-500">v1.0.0</span>

        <p className="mt-3 font-body text-sm text-chirp-stone-500 italic text-center">
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
          <p className="mt-2 font-body text-sm text-chirp-stone-700">
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
          <p className="mt-2 font-body text-sm text-chirp-stone-500">
            Couldn't check for updates.
          </p>
        )}

        {/* Credits */}
        <div className="mt-8 w-full max-w-sm rounded-xl bg-chirp-stone-100 p-4">
          <div className="flex flex-col items-center gap-1.5">
            <p className="font-body text-[13px] text-chirp-stone-500">
              Made by Pieter de Bruijn
            </p>
            <p className="font-body text-[13px] text-chirp-stone-500">
              Speech recognition: Parakeet TDT (sherpa-onnx)
            </p>
            <p className="font-body text-[13px] text-chirp-stone-500">
              Built with Tauri + React
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
