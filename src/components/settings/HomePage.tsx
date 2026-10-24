import { useState, useMemo } from 'react'
import { trackEvent } from '@aptabase/tauri'
import { Search, Download, Trash2, Copy, BookText, Zap, ChevronDown, Clock, Mic, Type, Hash, AlertTriangle, Check } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useCleanupToggle } from '../../hooks/useCleanupToggle'
import { useCountUp } from '../../hooks/useCountUp'
import { TONE_MODES } from '../../lib/constants'
import {
  formatRelativeTime,
  getWeekBarChartData,
  getYesterdayComparison,
  estimateTimeSaved,
  groupHistoryByDay,
  formatDayLabel,
} from '../../lib/utils'
import { Toggle } from '../shared/Toggle'
import { Select } from '../shared/Select'
import { AnnouncementBanner } from './AnnouncementBanner'

function getGreeting(): string {
  const hour = new Date().getHours()
  if (hour < 12) return 'Good morning'
  if (hour < 18) return 'Good afternoon'
  return 'Good evening'
}

export function HomePage() {
  const store = useAppStore()
  const tauri = useTauri()
  const { handleCleanupToggle, cleanupStarting, llmDownloaded } = useCleanupToggle()
  const [copiedTimestamp, setCopiedTimestamp] = useState<string | null>(null)
  const [expandedTimestamp, setExpandedTimestamp] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [quickAddTab, setQuickAddTab] = useState<'vocabulary' | 'snippets'>('vocabulary')
  const [qaWord, setQaWord] = useState('')
  const [qaTrigger, setQaTrigger] = useState('')
  const [qaExpansion, setQaExpansion] = useState('')
  const [qaAdded, setQaAdded] = useState(false)

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

  // Formatted date
  const dateStr = new Date().toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
  })

  const handleQuickAddVocab = () => {
    const word = qaWord.trim()
    if (!word) return
    store.addVocabularyEntry(word)
    setQaWord('')
    setQaAdded(true)
    setTimeout(() => setQaAdded(false), 1200)
  }

  const handleQuickAddSnippet = () => {
    const trigger = qaTrigger.trim()
    const expansion = qaExpansion.trim()
    if (!trigger || !expansion) return
    store.addSnippet(trigger, expansion)
    setQaTrigger('')
    setQaExpansion('')
    setQaAdded(true)
    setTimeout(() => setQaAdded(false), 1200)
  }

  const handleCopy = async (text: string, timestamp: string) => {
    try {
      await navigator.clipboard.writeText(text)
      setCopiedTimestamp(timestamp)
      setTimeout(() => setCopiedTimestamp(null), 1500)
      trackEvent('feature_used', { feature: 'history_copy' })
    } catch { /* clipboard access denied — window may not be focused */ }
  }

  const handleDelete = async (timestamp: string) => {
    try {
      await tauri.deleteHistoryEntry(timestamp)
      store.removeHistoryEntry(timestamp)
    } catch { /* backend delete failed — entry stays in UI */ }
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
    setShowExportSuccess(true)
  }

  const [showClearConfirm, setShowClearConfirm] = useState(false)
  const [showExportSuccess, setShowExportSuccess] = useState(false)

  const handleClearHistory = async () => {
    setShowClearConfirm(false)
    await tauri.clearHistory()
    store.setHistory([])
  }

  const updateAvailable = store.updateAvailable
  const setAboutModalOpen = useAppStore((s) => s.setAboutModalOpen)

  return (
    <div className="flex flex-col gap-[10px] animate-fade-in">
      <AnnouncementBanner />

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
            <div className="flex items-center gap-2">
              {updateAvailable && (
                <button
                  onClick={() => setAboutModalOpen(true)}
                  className="bg-chirp-yellow/[0.15] backdrop-blur-sm border border-chirp-yellow/20 rounded-full px-3 py-[5px] flex items-center gap-2 hover:bg-chirp-yellow/[0.25] transition-colors"
                >
                  <div className="w-[7px] h-[7px] rounded-full bg-chirp-yellow animate-pulse" />
                  <span className="text-[11px] text-chirp-yellow font-body font-medium">
                    v{updateAvailable} available
                  </span>
                </button>
              )}
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

      {/* Contextual Cards Row */}
      <div className="flex gap-[10px]">
        {/* Smart Cleanup controls */}
        <div className="flex-1 rounded-card border border-card-border bg-card p-4 hover-lift">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <span className="text-[13px] font-semibold text-dm-primary">Smart Cleanup</span>
              {store.aiCleanup && (
                <span className="flex items-center gap-1">
                  {cleanupStarting ? (
                    <>
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400 animate-pulse" />
                      <span className="text-[11px] text-dm-secondary">Starting...</span>
                    </>
                  ) : store.llmReady ? (
                    <>
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                      <span className="text-[11px] text-dm-secondary">Active</span>
                    </>
                  ) : !llmDownloaded ? (
                    <>
                      <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400" />
                      <span className="text-[11px] text-chirp-amber-500">Model needed</span>
                    </>
                  ) : null}
                </span>
              )}
            </div>
            <Toggle
              checked={store.aiCleanup}
              onChange={handleCleanupToggle}
              disabled={cleanupStarting}
            />
          </div>
          <p className="text-[11px] text-dm-secondary mb-3">Polish grammar and filler words with local AI</p>
          {store.aiCleanup && (
            <div className="flex items-center justify-between pt-3 border-t border-dm-row-sep">
              <span className="text-[12px] text-dm-muted">Tone</span>
              <div className="w-[140px]">
                <Select
                  options={TONE_MODES.map(m => ({ value: m.id, label: m.label }))}
                  value={store.toneMode}
                  onChange={(v) => store.updateSettings({ toneMode: String(v) })}
                />
              </div>
            </div>
          )}
        </div>

        {/* Quick-add vocabulary/snippets */}
        <div className="flex-1 rounded-card border border-card-border bg-card p-4 hover-lift">
          {/* Tab header */}
          <div className="flex items-center gap-3 mb-3">
            <button
              onClick={() => setQuickAddTab('vocabulary')}
              className={`flex items-center gap-1.5 text-[13px] font-semibold transition-colors ${
                quickAddTab === 'vocabulary' ? 'text-dm-primary' : 'text-dm-tertiary hover:text-dm-muted'
              }`}
            >
              <BookText size={14} />
              Vocabulary
              <span className="text-[10px] font-normal text-dm-secondary">{store.vocabulary.length}</span>
            </button>
            <span className="text-dm-row-sep">|</span>
            <button
              onClick={() => setQuickAddTab('snippets')}
              className={`flex items-center gap-1.5 text-[13px] font-semibold transition-colors ${
                quickAddTab === 'snippets' ? 'text-dm-primary' : 'text-dm-tertiary hover:text-dm-muted'
              }`}
            >
              <Zap size={14} />
              Snippets
              <span className="text-[10px] font-normal text-dm-secondary">{store.snippets.length}</span>
            </button>
          </div>

          {qaAdded ? (
            <div className="flex items-center justify-center py-4 text-[12px] text-chirp-success font-medium animate-fade-in">
              Added
            </div>
          ) : quickAddTab === 'vocabulary' ? (
            <div className="flex gap-2">
              <input
                type="text"
                value={qaWord}
                onChange={(e) => setQaWord(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleQuickAddVocab()}
                placeholder="Add a word or name..."
                className="flex-1 h-[34px] rounded-lg border border-card-border bg-card-hover px-2.5 text-[12px] text-dm-primary placeholder:text-dm-tertiary focus:border-chirp-yellow focus:outline-none transition-colors"
              />
              <button
                onClick={handleQuickAddVocab}
                disabled={!qaWord.trim()}
                className="h-[34px] px-3 rounded-lg bg-dm-btn-bg text-dm-btn-text text-[12px] font-medium disabled:bg-dm-btn-disabled-bg transition-colors"
              >
                Add
              </button>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              <input
                type="text"
                value={qaTrigger}
                onChange={(e) => setQaTrigger(e.target.value)}
                placeholder="Trigger phrase..."
                className="h-[34px] rounded-lg border border-card-border bg-card-hover px-2.5 text-[12px] text-dm-primary placeholder:text-dm-tertiary focus:border-chirp-yellow focus:outline-none transition-colors"
              />
              <div className="flex gap-2">
                <input
                  type="text"
                  value={qaExpansion}
                  onChange={(e) => setQaExpansion(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleQuickAddSnippet()}
                  placeholder="Expands to..."
                  className="flex-1 h-[34px] rounded-lg border border-card-border bg-card-hover px-2.5 text-[12px] text-dm-primary placeholder:text-dm-tertiary focus:border-chirp-yellow focus:outline-none transition-colors"
                />
                <button
                  onClick={handleQuickAddSnippet}
                  disabled={!qaTrigger.trim() || !qaExpansion.trim()}
                  className="h-[34px] px-3 rounded-lg bg-dm-btn-bg text-dm-btn-text text-[12px] font-medium disabled:bg-dm-btn-disabled-bg transition-colors"
                >
                  Add
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* History Section */}
      <div className="mt-2">
        {/* Search + Export bar */}
        <div className="flex items-center gap-3 mb-4">
          <h2 className="font-display font-bold text-[15px] text-dm-primary flex-shrink-0">History</h2>
          <div className="flex-1 relative">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-dm-secondary pointer-events-none" />
            <input
              type="text"
              placeholder="Search transcriptions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full h-[36px] rounded-[10px] border border-card-border bg-dm-input pl-8 pr-3 font-body text-[13px] text-dm-primary placeholder:text-dm-secondary focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
            />
          </div>
          {store.history.length > 0 && (
            <>
              <button
                onClick={() => setShowClearConfirm(true)}
                className="h-[36px] px-3 rounded-[10px] border border-card-border bg-card font-body text-[12px] text-dm-muted flex items-center gap-1.5 hover:bg-red-50 hover:text-red-400 hover:border-red-200 transition-colors flex-shrink-0"
              >
                <Trash2 size={13} />
                Clear
              </button>
              <button
                onClick={handleExport}
                className="h-[36px] px-3 rounded-[10px] border border-card-border bg-card font-body text-[12px] text-dm-muted flex items-center gap-1.5 hover:bg-card-hover transition-colors flex-shrink-0"
              >
                <Download size={13} />
                Export
              </button>
            </>
          )}
        </div>

        {/* Day-grouped history */}
        {store.history.length === 0 ? (
          <div className="flex items-center justify-center rounded-card border border-card-border bg-card px-6 py-12">
            <p className="font-body text-sm text-dm-secondary text-center">
              Your transcriptions will appear here
            </p>
          </div>
        ) : filteredHistory.length === 0 ? (
          <div className="flex items-center justify-center rounded-card border border-card-border bg-card px-6 py-12">
            <p className="font-body text-sm text-dm-secondary text-center">
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
                    <h3 className="font-display font-bold text-[13px] text-dm-muted uppercase tracking-wide">
                      {formatDayLabel(dayKey)}
                    </h3>
                    <span className="text-[11px] text-dm-secondary font-body">
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
                      const isExpanded = expandedTimestamp === entry.timestamp
                      return (
                        <div
                          key={entry.timestamp}
                          onClick={() => setExpandedTimestamp(isExpanded ? null : entry.timestamp)}
                          className={`p-[14px_16px] bg-card rounded-card border border-card-border flex items-start gap-3 cursor-pointer group transition-all duration-200 ${isExpanded ? 'shadow-[0_2px_12px_rgba(0,0,0,0.06)]' : 'hover-lift'}`}
                        >
                          {/* Left indicator bar */}
                          <div className={`w-1 min-h-[36px] self-stretch rounded-sm flex-shrink-0 mt-0.5 ${
                            entry.wasCleanedUp ? 'bg-gradient-to-b from-chirp-yellow to-[#F7D86C]' : 'bg-card-border'
                          }`} />

                          {/* Content */}
                          <div className="flex-1 min-w-0">
                            <div className={`text-[13px] text-dm-primary leading-relaxed ${isExpanded ? '' : 'line-clamp-3'}`}>{entry.text}</div>

                            {/* Collapsed metadata */}
                            {!isExpanded && (
                              <div className="flex gap-2 mt-[6px] items-center flex-wrap">
                                <span className="text-[11px] text-dm-secondary">{entry.wordCount} words</span>
                                <span className="text-[11px] text-dm-row-sep">&middot;</span>
                                <span className="text-[11px] text-dm-secondary">{(entry.speechDurationMs / 1000).toFixed(0)}s</span>
                                {wpm && <>
                                  <span className="text-[11px] text-dm-row-sep">&middot;</span>
                                  <span className="text-[10px] text-dm-secondary font-medium bg-card-hover px-2 py-[1px] rounded">{wpm} WPM</span>
                                </>}
                                {entry.wasCleanedUp && <>
                                  <span className="text-[11px] text-dm-row-sep">&middot;</span>
                                  <span className="text-[10px] text-chirp-amber-500 font-semibold bg-chirp-amber-400/15 px-2 py-[1px] rounded">Polished</span>
                                </>}
                              </div>
                            )}

                            {/* Expanded stats */}
                            {isExpanded && (
                              <div className="mt-3 animate-fade-in">
                                <div className="flex items-center gap-4 p-3 bg-card-hover rounded-[10px] border border-card-border">
                                  <div className="flex items-center gap-2">
                                    <Mic size={12} className="text-dm-tertiary" />
                                    <div>
                                      <div className="text-[10px] text-dm-muted uppercase tracking-wide">Speech</div>
                                      <div className="text-[13px] text-dm-primary font-medium">{(entry.speechDurationMs / 1000).toFixed(1)}s</div>
                                    </div>
                                  </div>
                                  <div className="flex items-center gap-2">
                                    <Clock size={12} className="text-dm-tertiary" />
                                    <div>
                                      <div className="text-[10px] text-dm-muted uppercase tracking-wide">Processing</div>
                                      <div className="text-[13px] text-dm-primary font-medium">{(entry.durationMs / 1000).toFixed(1)}s</div>
                                    </div>
                                  </div>
                                  {wpm && (
                                    <div className="flex items-center gap-2">
                                      <Type size={12} className="text-dm-tertiary" />
                                      <div>
                                        <div className="text-[10px] text-dm-muted uppercase tracking-wide">Speed</div>
                                        <div className="text-[13px] text-dm-primary font-medium">{wpm} WPM</div>
                                      </div>
                                    </div>
                                  )}
                                  <div className="flex items-center gap-2">
                                    <Hash size={12} className="text-dm-tertiary" />
                                    <div>
                                      <div className="text-[10px] text-dm-muted uppercase tracking-wide">Words</div>
                                      <div className="text-[13px] text-dm-primary font-medium">{entry.wordCount}</div>
                                    </div>
                                  </div>
                                </div>

                                {/* Expanded actions + full timestamp */}
                                <div className="flex items-center justify-between mt-3">
                                  <span className="text-[11px] text-dm-muted">
                                    {new Date(entry.timestamp).toLocaleString(undefined, {
                                      weekday: 'short', month: 'short', day: 'numeric',
                                      year: 'numeric', hour: 'numeric', minute: '2-digit'
                                    })}
                                  </span>
                                  <div className="flex gap-1">
                                    <button
                                      onClick={(e) => {
                                        e.stopPropagation()
                                        handleCopy(entry.text, entry.timestamp)
                                      }}
                                      className="h-[28px] px-2.5 rounded-[7px] bg-surface border border-card-border flex items-center gap-1.5 text-dm-secondary hover:bg-card-hover hover:text-dm-primary transition-all text-[11px]"
                                    >
                                      {isCopied ? (
                                        <span className="text-chirp-success font-semibold">Copied</span>
                                      ) : (
                                        <><Copy size={12} /> Copy</>
                                      )}
                                    </button>
                                    <button
                                      onClick={(e) => {
                                        e.stopPropagation()
                                        handleDelete(entry.timestamp)
                                      }}
                                      className="h-[28px] px-2.5 rounded-[7px] bg-surface border border-card-border flex items-center gap-1.5 text-dm-secondary hover:bg-red-50 hover:border-red-200 hover:text-red-400 transition-all text-[11px]"
                                    >
                                      <Trash2 size={12} /> Delete
                                    </button>
                                  </div>
                                </div>
                              </div>
                            )}
                          </div>

                          {/* Hover actions (collapsed only) */}
                          {!isExpanded && (
                            <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation()
                                  handleCopy(entry.text, entry.timestamp)
                                }}
                                className="w-[30px] h-[30px] rounded-[7px] bg-surface border border-card-border flex items-center justify-center text-dm-secondary hover:bg-card-hover hover:text-dm-primary transition-all"
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
                                className="w-[30px] h-[30px] rounded-[7px] bg-surface border border-card-border flex items-center justify-center text-dm-secondary hover:bg-red-50 hover:border-red-200 hover:text-red-400 transition-all"
                                title="Delete"
                              >
                                <Trash2 size={13} />
                              </button>
                            </div>
                          )}

                          {/* Chevron + Time */}
                          <div className="flex flex-col items-end gap-1 flex-shrink-0">
                            <div className="text-[11px] text-dm-tertiary whitespace-nowrap">
                              {formatRelativeTime(entry.timestamp)}
                            </div>
                            <ChevronDown size={14} className={`text-dm-tertiary transition-transform duration-200 ${isExpanded ? 'rotate-180' : ''}`} />
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              )
            })}

          </div>
        )}
      </div>

      {/* Clear history confirmation modal */}
      {showClearConfirm && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
          onClick={() => setShowClearConfirm(false)}
        >
          <div
            className="bg-surface rounded-[16px] border border-card-border shadow-xl w-[360px] p-6 animate-fade-in"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex flex-col items-center text-center mb-5">
              <div className="w-12 h-12 rounded-full bg-red-50 flex items-center justify-center mb-3">
                <AlertTriangle size={24} className="text-red-400" />
              </div>
              <h3 className="font-display font-bold text-[15px] text-dm-primary mb-1">Clear all history</h3>
              <p className="font-body text-[13px] text-dm-muted">
                This will permanently delete all your transcription history. This cannot be undone.
              </p>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setShowClearConfirm(false)}
                className="flex-1 h-[38px] rounded-[10px] border border-card-border bg-card font-body text-[13px] text-dm-primary hover:bg-card-hover transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleClearHistory}
                className="flex-1 h-[38px] rounded-[10px] bg-red-500 font-body text-[13px] text-white hover:bg-red-600 transition-colors"
              >
                Delete all
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Export success modal */}
      {showExportSuccess && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
          onClick={() => setShowExportSuccess(false)}
        >
          <div
            className="bg-surface rounded-[16px] border border-card-border shadow-xl w-[360px] p-6 animate-fade-in"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex flex-col items-center text-center mb-5">
              <div className="w-12 h-12 rounded-full bg-green-50 flex items-center justify-center mb-3">
                <Check size={24} className="text-green-500" />
              </div>
              <h3 className="font-display font-bold text-[15px] text-dm-primary mb-1">History exported</h3>
              <p className="font-body text-[13px] text-dm-muted">
                {store.history.length} {store.history.length === 1 ? 'entry' : 'entries'} saved to your downloads folder.
              </p>
            </div>
            <button
              onClick={() => setShowExportSuccess(false)}
              className="w-full h-[38px] rounded-[10px] bg-dm-btn-bg font-body text-[13px] text-dm-btn-text hover:bg-dm-btn-hover transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
