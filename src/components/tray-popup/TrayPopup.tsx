import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Copy, Settings, LogOut, Check } from 'lucide-react'
import { useAppStore, type TranscriptionEntry } from '../../stores/appStore'
import { useOverlaySync } from '../../hooks/useOverlaySync'
import { useCleanupToggle } from '../../hooks/useCleanupToggle'
import { formatHotkey, formatRelativeTime } from '../../lib/utils'
import { BirdMark } from '../shared/BirdMark'
import { Toggle } from '../shared/Toggle'
import { KeyBadge } from '../shared/KeyBadge'

export function TrayPopup() {
  useOverlaySync()

  // Reload all state fresh from backend each time the popup is shown
  useEffect(() => {
    const refresh = async () => {
      try {
        const [settings, history, modelStatus, llmStatus] = await Promise.all([
          invoke('get_settings'),
          invoke('get_history'),
          invoke<{ model: string; downloaded: boolean }>('get_model_status', { model: 'parakeet-tdt-0.6b' }),
          invoke<{ binaryDownloaded: boolean; modelDownloaded: boolean; serverRunning: boolean }>('get_llm_status'),
        ])
        if (settings && typeof settings === 'object') {
          useAppStore.getState().updateSettings(settings as Partial<ReturnType<typeof useAppStore.getState>>)
        }
        if (Array.isArray(history)) {
          useAppStore.getState().setHistory(history as TranscriptionEntry[])
        }
        if (modelStatus?.downloaded) {
          useAppStore.getState().updateSettings({
            modelDownloaded: {
              ...useAppStore.getState().modelDownloaded,
              [modelStatus.model]: true,
            },
          })
        }
        if (llmStatus) {
          useAppStore.getState().setLlmDownloaded(llmStatus.binaryDownloaded && llmStatus.modelDownloaded)
          useAppStore.getState().setLlmReady(llmStatus.serverRunning)
        }
      } catch { /* backend unavailable */ }
    }
    refresh()
    const unlisten = listen('tray-popup-shown', refresh)
    return () => { unlisten.then((f) => f()) }
  }, [])

  const darkMode = useAppStore((s) => s.darkMode)
  const hotkey = useAppStore((s) => s.hotkey)
  const aiCleanup = useAppStore((s) => s.aiCleanup)
  const llmReady = useAppStore((s) => s.llmReady)
  const hotkeyStatus = useAppStore((s) => s.hotkeyStatus)
  const history = useAppStore((s) => s.history)
  const modelDownloaded = useAppStore((s) => s.modelDownloaded)
  const settingsLoaded = useAppStore((s) => s.settingsLoaded)
  const { handleCleanupToggle, cleanupStarting } = useCleanupToggle()

  const [copied, setCopied] = useState(false)

  const hotkeyLabels = formatHotkey(hotkey)
  const isModelReady = modelDownloaded['parakeet-tdt-0.6b']

  // Today stats
  const today = new Date().toDateString()
  const todayEntries = history.filter((e) => new Date(e.timestamp).toDateString() === today)
  const todayWords = todayEntries.reduce((sum, e) => sum + e.wordCount, 0)
  const todaySessions = todayEntries.length

  // Last transcription
  const lastEntry = history.length > 0
    ? [...history].sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())[0]
    : null

  // Hide on Escape (dismiss is handled by Rust via WindowEvent::Focused)
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') getCurrentWindow().hide()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const handleCopy = async () => {
    if (!lastEntry) return
    try {
      await navigator.clipboard.writeText(lastEntry.text)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch { /* clipboard access denied */ }
  }

  const handleOpenSettings = async () => {
    await invoke('show_settings')
    getCurrentWindow().hide()
  }

  const handleQuit = async () => {
    await invoke('quit_app')
  }

  // Status display
  const statusDot = isModelReady
    ? 'bg-chirp-success animate-glow-pulse'
    : 'bg-chirp-amber-400'
  const statusLabel = isModelReady ? 'Ready' : 'Model needed'

  if (!settingsLoaded) return null

  return (
    <div className={`h-full w-full bg-card overflow-hidden ${darkMode ? 'dark' : ''}`}>
      <div className="animate-popup-in">
        {/* Header */}
        <div className="flex items-center justify-between px-5 pt-5 pb-3">
          <div className="flex items-center gap-2.5">
            <BirdMark size={24} />
            <span className="font-display font-[800] text-[17px] text-dm-primary tracking-[-0.3px]">
              chirp
            </span>
          </div>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${statusDot}`} />
            <span className="text-[12px] font-medium text-dm-secondary">{statusLabel}</span>
          </div>
        </div>

        {/* Hotkey display */}
        <div className="px-5 pb-3">
          <div className="flex items-center gap-1.5 px-3 py-2.5 rounded-[10px] bg-surface border border-card-border">
            <span className="text-[11px] text-dm-muted font-medium mr-auto">Hotkey</span>
            <div className="flex items-center gap-1">
              {hotkeyLabels.map((label) => (
                <KeyBadge key={label} keyLabel={label} />
              ))}
            </div>
            {hotkeyStatus === 'active' && (
              <div className="w-1.5 h-1.5 rounded-full bg-chirp-success ml-1" />
            )}
            {hotkeyStatus === 'failed' && (
              <div className="w-1.5 h-1.5 rounded-full bg-red-400 ml-1" />
            )}
          </div>
        </div>

        {/* Smart Cleanup toggle */}
        <div className="flex items-center justify-between px-5 py-3 border-t border-card-border">
          <div>
            <div className="text-[13px] font-medium text-dm-primary">Smart Cleanup</div>
            <div className="text-[11px] text-dm-secondary mt-0.5">
              {aiCleanup
                ? cleanupStarting
                  ? 'Starting...'
                  : llmReady
                    ? 'Active'
                    : 'Model needed'
                : 'Off'}
            </div>
          </div>
          <Toggle
            checked={aiCleanup}
            onChange={handleCleanupToggle}
            disabled={cleanupStarting}
          />
        </div>

        {/* Last transcription */}
        {lastEntry && (
          <button
            onClick={handleCopy}
            className="w-full text-left px-5 py-3 border-t border-card-border hover:bg-card-hover transition-colors group"
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-[11px] font-medium text-dm-muted uppercase tracking-wide">Latest</span>
              <span className="text-[11px] text-dm-secondary flex items-center gap-1">
                {copied ? (
                  <><Check size={11} className="text-chirp-success" /> Copied</>
                ) : (
                  <><Copy size={11} className="opacity-0 group-hover:opacity-100 transition-opacity" /> {formatRelativeTime(lastEntry.timestamp)}</>
                )}
              </span>
            </div>
            <p className="text-[13px] text-dm-primary leading-relaxed line-clamp-2">
              {lastEntry.text}
            </p>
            <span className="text-[11px] text-dm-secondary mt-1 block">
              {lastEntry.wordCount} words
            </span>
          </button>
        )}

        {/* Today stats */}
        <div className="flex border-t border-card-border">
          <div className="flex-1 px-5 py-3 border-r border-card-border">
            <div className="text-[11px] text-dm-muted font-medium uppercase tracking-wide mb-0.5">Today</div>
            <div className="font-display font-[800] text-[22px] text-dm-primary leading-none">
              {todayWords.toLocaleString()}
            </div>
            <div className="text-[11px] text-dm-secondary mt-0.5">words</div>
          </div>
          <div className="flex-1 px-5 py-3">
            <div className="text-[11px] text-dm-muted font-medium uppercase tracking-wide mb-0.5">Sessions</div>
            <div className="font-display font-[800] text-[22px] text-dm-primary leading-none">
              {todaySessions}
            </div>
            <div className="text-[11px] text-dm-secondary mt-0.5">today</div>
          </div>
        </div>

        {/* Action buttons */}
        <div className="flex gap-2 px-5 py-4 border-t border-card-border">
          <button
            onClick={handleOpenSettings}
            className="flex-1 h-[34px] rounded-[8px] bg-surface border border-card-border flex items-center justify-center gap-1.5 text-[12px] font-medium text-dm-primary hover:bg-card-hover transition-colors"
          >
            <Settings size={13} />
            Settings
          </button>
          <button
            onClick={handleQuit}
            className="h-[34px] px-4 rounded-[8px] bg-surface border border-card-border flex items-center justify-center gap-1.5 text-[12px] font-medium text-dm-secondary hover:bg-red-500/10 hover:text-red-400 hover:border-red-400/20 transition-colors"
          >
            <LogOut size={13} />
            Quit
          </button>
        </div>
      </div>
    </div>
  )
}
