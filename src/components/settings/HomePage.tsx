import { useState, useEffect } from 'react'
import { Copy, Download, FileText, Search, Sparkles, Trash2, X } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { STT_MODELS } from '../../lib/constants'
import { formatHotkey, formatRelativeTime } from '../../lib/utils'
import { Button } from '../shared/Button'
import { KeyBadge } from '../shared/KeyBadge'

function getGreeting(): string {
  const hour = new Date().getHours()
  if (hour < 12) return 'Good morning'
  if (hour < 18) return 'Good afternoon'
  return 'Good evening'
}

function formatFullDate(isoString: string): string {
  return new Date(isoString).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  })
}

export function HomePage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [expandedTimestamp, setExpandedTimestamp] = useState<string | null>(null)
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null)
  const [copiedTimestamp, setCopiedTimestamp] = useState<string | null>(null)

  const [searchQuery, setSearchQuery] = useState('')

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isDownloaded = store.modelDownloaded[store.model]

  const hotkeyParts = formatHotkey(store.hotkey)

  // Stats
  const totalWords = store.history.reduce((sum, e) => sum + e.wordCount, 0)
  const totalSessions = store.history.length
  const today = new Date().toDateString()
  const todayWords = store.history
    .filter((e) => new Date(e.timestamp).toDateString() === today)
    .reduce((sum, e) => sum + e.wordCount, 0)

  // Reset confirmingDelete when expanded entry changes
  useEffect(() => {
    setConfirmingDelete(null)
  }, [expandedTimestamp])

  const handleDownload = async () => {
    setDownloadError(null)
    store.setModelDownloadProgress(0)
    try {
      await tauri.downloadModel(store.model, (progress) => {
        store.setModelDownloadProgress(progress)
      })
      store.updateSettings({
        modelDownloaded: { ...store.modelDownloaded, [store.model]: true },
      })
    } catch {
      setDownloadError('Download failed. Check your internet connection and try again.')
    } finally {
      store.setModelDownloadProgress(null)
    }
  }

  const handleClearHistory = async () => {
    await tauri.clearHistory()
    store.setHistory([])
    setExpandedTimestamp(null)
  }

  const handleExport = () => {
    const content = store.history
      .map(
        (e) =>
          `[${new Date(e.timestamp).toLocaleString()}] (${e.wordCount} words)\n${e.text}\n`
      )
      .join('\n---\n\n')
    const blob = new Blob([content], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `chirp-history-${new Date().toISOString().slice(0, 10)}.txt`
    a.click()
    URL.revokeObjectURL(url)
  }

  const handleCopy = async (text: string, timestamp: string) => {
    await navigator.clipboard.writeText(text)
    setCopiedTimestamp(timestamp)
    setTimeout(() => setCopiedTimestamp(null), 1500)
  }

  const handleDelete = async (timestamp: string) => {
    if (confirmingDelete === timestamp) {
      await tauri.deleteHistoryEntry(timestamp)
      store.removeHistoryEntry(timestamp)
      setExpandedTimestamp(null)
      setConfirmingDelete(null)
    } else {
      setConfirmingDelete(timestamp)
    }
  }

  const toggleExpand = (timestamp: string) => {
    setExpandedTimestamp((prev) => (prev === timestamp ? null : timestamp))
  }

  return (
    <div className="flex flex-col animate-fade-in">
      {/* Greeting bar */}
      <div className="flex items-center justify-between">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
          {getGreeting()}
        </h1>
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${isDownloaded ? 'bg-chirp-success' : 'bg-chirp-amber-400'}`} />
          <span className="font-body text-sm text-chirp-stone-500">
            {isDownloaded ? 'Ready' : 'Model needed'}
          </span>
        </div>
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-3 gap-4 mt-6">
        {[
          { value: totalWords.toLocaleString(), label: 'Total Words' },
          { value: totalSessions.toLocaleString(), label: 'Sessions' },
          { value: todayWords.toLocaleString(), label: 'Today' },
        ].map(({ value, label }) => (
          <div
            key={label}
            className="rounded-xl border border-chirp-stone-200 bg-white p-5 shadow-stat border-t-2 border-t-chirp-amber-400"
          >
            <div className="font-display font-extrabold text-3xl text-chirp-stone-900">
              {value}
            </div>
            <div className="font-body text-sm text-chirp-stone-500 mt-1">{label}</div>
          </div>
        ))}
      </div>

      {/* Model download banner (only if not downloaded) */}
      {!isDownloaded && (
        <div className="mt-6 rounded-xl border border-chirp-stone-200 bg-white p-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Download size={16} className="text-chirp-stone-400" />
              <span className="font-body text-sm text-chirp-stone-700">
                Speech model needed
              </span>
            </div>
            {store.modelDownloadProgress === null && (
              <Button onClick={handleDownload}>
                Download ({currentModel?.size})
              </Button>
            )}
          </div>
          {store.modelDownloadProgress !== null && (
            <div className="mt-3">
              <div className="flex items-center gap-2">
                <div className="h-2 flex-1 overflow-hidden rounded-full bg-chirp-stone-200">
                  <div
                    className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200 relative overflow-hidden"
                    style={{ width: `${store.modelDownloadProgress}%` }}
                  >
                    <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer" />
                  </div>
                </div>
                <span className="font-mono text-xs text-chirp-stone-500 w-10 text-right">
                  {store.modelDownloadProgress}%
                </span>
              </div>
              <p className="font-body text-xs text-chirp-stone-500 mt-1">
                {store.modelDownloadProgress < 96
                  ? `Downloading ${currentModel?.name}...`
                  : 'Extracting model...'}
              </p>
            </div>
          )}
          {downloadError && (
            <p className="font-body text-xs text-chirp-error mt-2">{downloadError}</p>
          )}
        </div>
      )}

      {/* Hotkey reminder */}
      <div className="flex items-center gap-3 mt-6">
        <div className="flex items-center gap-1.5">
          {hotkeyParts.map((part, i) => (
            <KeyBadge key={i} keyLabel={part} />
          ))}
        </div>
        <span className="font-body text-sm text-chirp-stone-500">
          Hold to dictate
        </span>
      </div>

      {/* Recent transcriptions */}
      <div className="mt-8">
        <div className="flex items-center justify-between">
          <h2 className="font-display font-bold text-lg text-chirp-stone-900">Recent</h2>
          {store.history.length > 0 && (
            <div className="flex items-center gap-1">
              <Button variant="ghost" onClick={handleExport}>
                <FileText size={14} className="mr-1" />
                Export
              </Button>
              <Button variant="ghost" onClick={handleClearHistory}>
                Clear
              </Button>
            </div>
          )}
        </div>

        {store.history.length > 0 && (
          <div className="relative mt-3">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-chirp-stone-400" />
            <input
              type="text"
              placeholder="Search transcriptions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full h-9 rounded-lg border border-chirp-stone-200 bg-white pl-9 pr-3 font-body text-sm text-chirp-stone-700 placeholder:text-chirp-stone-400 focus:border-2 focus:border-chirp-amber-400 focus:outline-none transition-colors duration-150"
            />
          </div>
        )}

        {store.history.length > 0 ? (
          <div className="max-h-[400px] overflow-y-auto flex flex-col gap-2 mt-3">
            {[...store.history]
              .reverse()
              .filter(
                (entry) =>
                  !searchQuery.trim() ||
                  entry.text.toLowerCase().includes(searchQuery.toLowerCase())
              )
              .map((entry) => {
              const isExpanded = expandedTimestamp === entry.timestamp
              const wpm = entry.speechDurationMs > 0
                ? Math.round(entry.wordCount / (entry.speechDurationMs / 60000))
                : null

              return (
                <div
                  key={entry.timestamp}
                  className={`rounded-lg border border-chirp-stone-200 bg-white px-4 py-3 transition-all duration-200 cursor-pointer ${
                    isExpanded
                      ? 'border-l-2 border-l-chirp-amber-400 shadow-card-hover'
                      : 'hover:shadow-card-hover'
                  }`}
                  onClick={() => toggleExpand(entry.timestamp)}
                >
                  {isExpanded ? (
                    <div className="animate-fade-in" onClick={(e) => e.stopPropagation()}>
                      <p className="text-sm text-chirp-stone-700 whitespace-pre-wrap">
                        {entry.text}
                      </p>
                      <div className="flex flex-col gap-0.5 mt-2 text-xs text-chirp-stone-400">
                        <span>
                          {entry.wordCount} words{wpm ? ` · ${wpm} wpm` : ''} · {(entry.speechDurationMs / 1000).toFixed(1)}s audio
                        </span>
                        <span className="flex items-center gap-1.5">
                          Processed in {(entry.durationMs / 1000).toFixed(1)}s · {formatFullDate(entry.timestamp)}
                          {entry.wasCleanedUp && (
                            <span className="inline-flex items-center gap-0.5 text-chirp-amber-500">
                              <Sparkles size={10} />
                              Polished
                            </span>
                          )}
                        </span>
                      </div>
                      <div className="flex items-center gap-1 mt-3 border-t border-chirp-stone-100 pt-2">
                        <Button
                          variant="ghost"
                          onClick={() => handleCopy(entry.text, entry.timestamp)}
                        >
                          <Copy size={14} className="mr-1" />
                          {copiedTimestamp === entry.timestamp ? 'Copied!' : 'Copy'}
                        </Button>
                        <Button
                          variant="ghost"
                          onClick={() => handleDelete(entry.timestamp)}
                          className={confirmingDelete === entry.timestamp ? 'text-chirp-error' : 'hover:text-chirp-error'}
                        >
                          <Trash2 size={14} className="mr-1 text-chirp-error" />
                          {confirmingDelete === entry.timestamp ? 'Are you sure?' : 'Delete'}
                        </Button>
                        <div className="flex-1" />
                        <Button
                          variant="ghost"
                          onClick={() => setExpandedTimestamp(null)}
                        >
                          <X size={14} />
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <p className="text-sm text-chirp-stone-700 line-clamp-2">
                        {entry.text.slice(0, 120)}
                        {entry.text.length > 120 ? '...' : ''}
                      </p>
                      <div className="flex items-center gap-3 mt-1.5 text-xs text-chirp-stone-400">
                        <span>{formatRelativeTime(entry.timestamp)}</span>
                        <span>{entry.wordCount} words</span>
                        {wpm && <span>{wpm} wpm</span>}
                        <span>processed in {(entry.durationMs / 1000).toFixed(1)}s</span>
                        {entry.wasCleanedUp && (
                          <span className="inline-flex items-center gap-0.5 text-chirp-amber-500">
                            <Sparkles size={10} />
                            Polished
                          </span>
                        )}
                      </div>
                    </>
                  )}
                </div>
              )
            })}
          </div>
        ) : (
          <div className="flex items-center justify-center rounded-xl border border-chirp-stone-200 bg-white px-6 py-12 mt-3">
            <p className="font-body text-sm text-chirp-stone-400 text-center">
              Your transcriptions will appear here
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
