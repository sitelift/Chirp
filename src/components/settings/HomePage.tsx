import { useState, useMemo } from 'react'
import { Search, Download, Trash2, Copy, Lightbulb, Cpu } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { useCountUp } from '../../hooks/useCountUp'
import { STT_MODELS } from '../../lib/constants'
import {
  formatRelativeTime,
  getWeekBarChartData,
  getYesterdayComparison,
  estimateTimeSaved,
  groupHistoryByDay,
  formatDayLabel,
} from '../../lib/utils'
import { ContextCard } from '../shared/ContextCard'

function getGreeting(): string {
  const hour = new Date().getHours()
  if (hour < 12) return 'Good morning'
  if (hour < 18) return 'Good afternoon'
  return 'Good evening'
}

const QUICK_TIPS = [
  'Try adding a snippet for text you type often, like your email signature',
  'You can change your hotkey anytime in Settings',
  'Dictionary rules auto-replace words as you dictate',
  'Smart Cleanup polishes grammar and removes filler words automatically',
  'Hold the hotkey to talk, release to transcribe',
]

export function HomePage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [llmDownloaded] = useLlmDownloaded()
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [copiedTimestamp, setCopiedTimestamp] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isDownloaded = store.modelDownloaded[store.model]

  // Stats
  const totalWords = store.history.reduce((sum, e) => sum + e.wordCount, 0)
  const today = new Date().toDateString()
  const todayEntries = store.history.filter((e) => new Date(e.timestamp).toDateString() === today)
  const todayWords = todayEntries.reduce((sum, e) => sum + e.wordCount, 0)
  const todaySessions = todayEntries.length

  // Avg WPM
  const entriesWithSpeech = store.history.filter((e) => e.speechDurationMs > 0)
  const avgWpm = entriesWithSpeech.length > 0
    ? Math.round(entriesWithSpeech.reduce((sum, e) => sum + (e.wordCount / (e.speechDurationMs / 60000)), 0) / entriesWithSpeech.length)
    : 0

  // Animated numbers
  const animatedTodayWords = useCountUp(todayWords)
  const animatedSessions = useCountUp(todaySessions)
  const animatedTotalWords = useCountUp(totalWords)
  const animatedAvgWpm = useCountUp(avgWpm)

  // Week data
  const weekData = getWeekBarChartData(store.history)
  const trend = getYesterdayComparison(store.history)
  const timeSaved = estimateTimeSaved(totalWords)

  // Deduplicated history (filter by unique timestamp)
  const deduplicatedHistory = useMemo(() => {
    const seen = new Set<string>()
    return [...store.history]
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
      .filter((e) => {
        if (seen.has(e.timestamp)) return false
        seen.add(e.timestamp)
        return true
      })
  }, [store.history])

  // Search filter
  const filteredHistory = searchQuery.trim()
    ? deduplicatedHistory.filter((e) =>
        e.text.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : deduplicatedHistory

  // Group by day
  const grouped = groupHistoryByDay(filteredHistory)

  const tipIndex = useMemo(() => Math.floor(Date.now() / 86400000) % QUICK_TIPS.length, [])

  // Formatted date
  const dateStr = new Date().toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
  })

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

  const handleCopy = async (text: string, timestamp: string) => {
    await navigator.clipboard.writeText(text)
    setCopiedTimestamp(timestamp)
    setTimeout(() => setCopiedTimestamp(null), 1500)
  }

  const handleDelete = async (timestamp: string) => {
    await tauri.deleteHistoryEntry(timestamp)
    store.removeHistoryEntry(timestamp)
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

  const handleClearHistory = async () => {
    await tauri.clearHistory()
    store.setHistory([])
  }

  return (
    <div className="flex flex-col gap-[10px] animate-fade-in">
      {/* Hero Stats Block */}
      <div className="bg-sidebar rounded-[18px] relative overflow-hidden">
        {/* Floating orbs */}
        <div className="absolute top-[-30px] left-[-20px] w-[180px] h-[140px] rounded-full bg-gradient-to-br from-chirp-yellow/20 to-chirp-yellow/5 blur-[40px] animate-float-1 pointer-events-none" />
        <div className="absolute bottom-[-20px] right-[40px] w-[120px] h-[100px] rounded-full bg-gradient-to-br from-chirp-amber-400/15 to-chirp-amber-600/5 blur-[30px] animate-float-2 pointer-events-none" />

        <div className="relative z-10 p-6">
          {/* Top row: greeting + status pill */}
          <div className="flex items-center justify-between mb-6">
            <div>
              <h1 className="font-display font-[800] text-[22px] text-white leading-tight">
                {getGreeting()}
              </h1>
              <span className="text-[12px] text-white/40 font-body">{dateStr}</span>
            </div>
            <div className="bg-white/[0.08] backdrop-blur-sm border border-white/[0.08] rounded-full px-3 py-[5px] flex items-center gap-2">
              <div className={`w-[7px] h-[7px] rounded-full ${
                isDownloaded
                  ? 'bg-green-400 shadow-status-glow animate-glow-pulse'
                  : 'bg-chirp-amber-400'
              }`} />
              <span className="text-[11px] text-white/60 font-body font-medium">
                {isDownloaded ? 'Ready' : 'Setup needed'}
              </span>
            </div>
          </div>

          {/* Four stat columns — all with label on top, number below */}
          <div className="flex">
            {/* Words today */}
            <div className="flex-1 border-r border-white/[0.06] pr-5">
              <div className="text-[11px] text-white/30 font-body font-medium uppercase tracking-wider mb-1">
                Words today
              </div>
              <div className="font-display font-[900] text-[40px] text-gradient-white leading-none animate-count-up">
                {animatedTodayWords.toLocaleString()}
              </div>
              {trend !== null && (
                <div className="text-[11px] font-body font-medium mt-1.5 text-chirp-yellow">
                  {trend > 0 ? '+' : ''}{trend}% vs yesterday
                </div>
              )}
              {trend === null && todayWords > 0 && (
                <div className="text-[11px] font-body font-medium mt-1.5 text-white/20">
                  No data yesterday
                </div>
              )}
            </div>

            {/* Sessions */}
            <div className="flex-1 border-r border-white/[0.06] px-5">
              <div className="text-[11px] text-white/30 font-body font-medium uppercase tracking-wider mb-1">
                Sessions
              </div>
              <div className="font-display font-[900] text-[40px] text-gradient-white leading-none animate-count-up">
                {animatedSessions.toLocaleString()}
              </div>
              {/* Weekly bar chart */}
              <div className="flex gap-[5px] items-end h-10 mt-2">
                {weekData.map((words, i) => {
                  const maxWords = Math.max(...weekData, 1)
                  const height = Math.max(4, (words / maxWords) * 40)
                  const isToday = i === 6
                  const dayLabels = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
                  return (
                    <div key={i} className="flex flex-col items-center gap-1">
                      <div
                        className={`w-5 rounded ${isToday ? 'bg-chirp-yellow shadow-[0_0_12px_rgba(240,183,35,0.4)]' : words > 0 ? 'bg-chirp-yellow/40 hover:bg-chirp-yellow/70' : 'bg-white/[0.06]'} animate-bar-grow`}
                        style={{ height: `${height}px`, animationDelay: `${500 + i * 50}ms`, transformOrigin: 'bottom' }}
                      />
                      <span className={`text-[9px] font-medium ${isToday ? 'text-chirp-yellow' : 'text-white/20'}`}>{dayLabels[i]}</span>
                    </div>
                  )
                })}
              </div>
            </div>

            {/* Avg WPM — label on top, number below (consistent with others) */}
            <div className="flex-1 border-r border-white/[0.06] px-5">
              <div className="text-[11px] text-white/30 font-body font-medium uppercase tracking-wider mb-1">
                Avg WPM
              </div>
              <div className="font-display font-[900] text-[40px] text-gradient-white leading-none animate-count-up">
                {animatedAvgWpm.toLocaleString()}
              </div>
            </div>

            {/* All time */}
            <div className="flex-1 pl-5">
              <div className="text-[11px] text-white/30 font-body font-medium uppercase tracking-wider mb-1">
                All time
              </div>
              <div className="font-display font-[900] text-[40px] text-gradient-white leading-none animate-count-up">
                {animatedTotalWords.toLocaleString()}
              </div>
              <div className="text-[11px] font-body font-medium mt-1.5 text-white/30">
                ~{timeSaved} saved
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Contextual Cards Row — always both visible */}
      <div className="flex gap-[10px]">
        <ContextCard
          icon={<Lightbulb size={16} />}
          title="Quick tip"
          description={QUICK_TIPS[tipIndex]}
          variant="suggestion"
        />
        <ContextCard
          icon={<Cpu size={16} />}
          title="Models"
          description={
            isDownloaded && llmDownloaded
              ? 'Speech and cleanup models ready. Ready to transcribe.'
              : isDownloaded
                ? 'Speech model ready. Cleanup model not downloaded.'
                : 'Speech model needs to be downloaded.'
          }
          variant="default"
          actions={
            !isDownloaded
              ? [
                  {
                    label: store.modelDownloadProgress !== null
                      ? `Downloading... ${store.modelDownloadProgress}%`
                      : `Download (${currentModel?.size})`,
                    onClick: handleDownload,
                  },
                ]
              : undefined
          }
        />
      </div>

      {/* Download progress + error */}
      {!isDownloaded && store.modelDownloadProgress !== null && (
        <div className="rounded-card border border-card-border bg-white p-4">
          <div className="flex items-center gap-2">
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-chirp-stone-200">
              <div
                className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200 relative overflow-hidden"
                style={{ width: `${store.modelDownloadProgress}%` }}
              >
                <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer" />
              </div>
            </div>
            <span className="font-mono text-xs text-[#888] w-10 text-right">
              {store.modelDownloadProgress}%
            </span>
          </div>
          <p className="font-body text-xs text-[#888] mt-1">
            {store.modelDownloadProgress < 96
              ? `Downloading ${currentModel?.name}...`
              : 'Extracting model...'}
          </p>
        </div>
      )}
      {downloadError && (
        <p className="font-body text-xs text-chirp-error">{downloadError}</p>
      )}

      {/* History Section */}
      <div className="mt-2">
        {/* Search + Export bar */}
        <div className="flex items-center gap-3 mb-4">
          <h2 className="font-display font-bold text-[15px] text-[#1a1a1a] flex-shrink-0">History</h2>
          <div className="flex-1 relative">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-[#bbb] pointer-events-none" />
            <input
              type="text"
              placeholder="Search transcriptions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full h-[36px] rounded-[10px] border border-card-border bg-white pl-8 pr-3 font-body text-[13px] text-[#333] placeholder:text-[#bbb] focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
            />
          </div>
          {store.history.length > 0 && (
            <button
              onClick={handleExport}
              className="h-[36px] px-3 rounded-[10px] border border-card-border bg-white font-body text-[12px] text-[#888] flex items-center gap-1.5 hover:bg-[#FAFAF8] transition-colors flex-shrink-0"
            >
              <Download size={13} />
              Export
            </button>
          )}
        </div>

        {/* Day-grouped history */}
        {store.history.length === 0 ? (
          <div className="flex items-center justify-center rounded-card border border-card-border bg-white px-6 py-12">
            <p className="font-body text-sm text-[#bbb] text-center">
              Your transcriptions will appear here
            </p>
          </div>
        ) : filteredHistory.length === 0 ? (
          <div className="flex items-center justify-center rounded-card border border-card-border bg-white px-6 py-12">
            <p className="font-body text-sm text-[#bbb] text-center">
              No transcriptions match &ldquo;{searchQuery}&rdquo;
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-6">
            {Array.from(grouped.entries()).map(([dayKey, entries]) => {
              const dayWords = entries.reduce((sum, e) => sum + e.wordCount, 0)
              const daySessions = entries.length

              return (
                <div key={dayKey}>
                  {/* Day header */}
                  <div className="flex items-center justify-between mb-3">
                    <h3 className="font-display font-bold text-[13px] text-[#888] uppercase tracking-wide">
                      {formatDayLabel(dayKey)}
                    </h3>
                    <span className="text-[11px] text-[#bbb] font-body">
                      {dayWords.toLocaleString()} words &middot; {daySessions} session{daySessions !== 1 ? 's' : ''}
                    </span>
                  </div>

                  {/* Entries for this day */}
                  <div className="flex flex-col gap-2">
                    {entries.map((entry) => {
                      const wpm = entry.speechDurationMs > 0
                        ? Math.round(entry.wordCount / (entry.speechDurationMs / 60000))
                        : null
                      const isCopied = copiedTimestamp === entry.timestamp

                      return (
                        <div
                          key={entry.timestamp}
                          className="p-[14px_16px] bg-white rounded-card border border-card-border flex items-start gap-3 hover-lift cursor-default group"
                        >
                          {/* Left indicator bar */}
                          <div className={`w-1 min-h-[36px] rounded-sm flex-shrink-0 mt-0.5 ${
                            entry.wasCleanedUp ? 'bg-gradient-to-b from-chirp-yellow to-[#F7D86C]' : 'bg-[#e5e5e5]'
                          }`} />

                          {/* Content */}
                          <div className="flex-1 min-w-0">
                            <div className="text-[13px] text-[#333] leading-relaxed line-clamp-3">{entry.text}</div>
                            <div className="flex gap-2 mt-[6px] items-center flex-wrap">
                              <span className="text-[11px] text-[#bbb]">{entry.wordCount} words</span>
                              <span className="text-[11px] text-[#e5e5e5]">&middot;</span>
                              <span className="text-[11px] text-[#bbb]">{(entry.speechDurationMs / 1000).toFixed(0)}s</span>
                              {wpm && <>
                                <span className="text-[11px] text-[#e5e5e5]">&middot;</span>
                                <span className="text-[10px] text-[#888] font-medium bg-[#F5F4F0] px-2 py-[1px] rounded">{wpm} WPM</span>
                              </>}
                              {entry.wasCleanedUp && <>
                                <span className="text-[11px] text-[#e5e5e5]">&middot;</span>
                                <span className="text-[10px] text-[#D4A020] font-semibold bg-gradient-to-r from-[#FFF9E5] to-[#FEF3C7] px-2 py-[1px] rounded">Polished</span>
                              </>}
                            </div>
                          </div>

                          {/* Hover actions */}
                          <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                handleCopy(entry.text, entry.timestamp)
                              }}
                              className="w-[30px] h-[30px] rounded-[7px] bg-[#F5F4F0] border border-card-border flex items-center justify-center text-[#aaa] hover:bg-[#eee] hover:text-[#555] transition-all"
                              title="Copy"
                            >
                              {isCopied ? (
                                <span className="text-[10px] text-chirp-success font-semibold">OK</span>
                              ) : (
                                <Copy size={13} />
                              )}
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                handleDelete(entry.timestamp)
                              }}
                              className="w-[30px] h-[30px] rounded-[7px] bg-[#F5F4F0] border border-card-border flex items-center justify-center text-[#aaa] hover:bg-red-50 hover:border-red-200 hover:text-red-400 transition-all"
                              title="Delete"
                            >
                              <Trash2 size={13} />
                            </button>
                          </div>

                          {/* Time */}
                          <div className="text-[11px] text-[#ccc] whitespace-nowrap flex-shrink-0">
                            {formatRelativeTime(entry.timestamp)}
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              )
            })}

            {/* Clear history */}
            {store.history.length > 0 && (
              <div className="flex justify-end">
                <button
                  onClick={handleClearHistory}
                  className="flex items-center gap-1.5 text-[12px] text-[#bbb] hover:text-red-400 transition-colors font-body"
                >
                  <Trash2 size={12} />
                  Clear all history
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
