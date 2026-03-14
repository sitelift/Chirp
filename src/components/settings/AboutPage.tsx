import { useState } from 'react'
import { useTauri } from '../../hooks/useTauri'
import { BirdMark } from '../shared/BirdMark'
import { Button } from '../shared/Button'

export function AboutPage() {
  const tauri = useTauri()
  const [updateStatus, setUpdateStatus] = useState<
    'idle' | 'checking' | 'available' | 'current' | 'error'
  >('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)

  const handleCheckUpdates = async () => {
    setUpdateStatus('checking')
    try {
      const info = await tauri.checkForUpdates()
      if (info.available) {
        setUpdateStatus('available')
        setUpdateVersion(info.version)
      } else {
        setUpdateStatus('current')
        setTimeout(() => setUpdateStatus('idle'), 3000)
      }
    } catch {
      setUpdateStatus('error')
    }
  }

  return (
    <div className="flex flex-col items-center pt-8">
      <BirdMark size={64} />

      <h1 className="mt-3 font-display font-extrabold text-[28px] text-chirp-stone-900">
        chirp
      </h1>

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
        <Button variant="secondary" onClick={handleCheckUpdates} disabled={updateStatus === 'checking'}>
          {updateStatus === 'checking' ? 'Checking...' : 'Check for Updates'}
        </Button>
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

      {/* Divider */}
      <div className="mt-6 mb-6 w-full border-t border-chirp-stone-200" />

      <div className="flex flex-col items-center gap-1.5">
        <p className="font-body text-[13px] text-chirp-stone-500">
          Made by Pieter de Bruijn
        </p>
        <p className="font-body text-[13px] text-chirp-stone-500">
          Speech recognition: whisper.cpp
        </p>
        <p className="font-body text-[13px] text-chirp-stone-500">
          Text cleanup: Chirp Cleanup v1
        </p>
        <p className="font-body text-[13px] text-chirp-stone-500">
          Built with Tauri + React
        </p>
        <a
          href="https://github.com"
          target="_blank"
          rel="noopener noreferrer"
          className="mt-2 font-body text-[13px] font-medium text-chirp-info hover:underline"
        >
          Source code on GitHub →
        </a>
      </div>
    </div>
  )
}
