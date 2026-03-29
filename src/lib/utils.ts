import type { TranscriptionEntry } from '../stores/appStore'

export function formatHotkey(hotkey: string): string[] {
  const IS_MAC = navigator.platform.includes('Mac')
  return hotkey.split('+').map(c => {
    c = c.trim()
    if (c === 'ControlLeft' || c === 'ControlRight') return 'Ctrl'
    if (c === 'ShiftLeft' || c === 'ShiftRight') return 'Shift'
    if (c === 'AltLeft' || c === 'AltRight') return IS_MAC ? 'Option' : 'Alt'
    if (c === 'MetaLeft' || c === 'MetaRight') return IS_MAC ? '\u2318' : 'Win'
    if (c === 'Fn') return 'fn'
    if (c.startsWith('Key') && c.length === 4) return c[3]
    if (c.startsWith('Digit') && c.length === 6) return c[5]
    if (c === 'ArrowUp') return 'Up'
    if (c === 'ArrowDown') return 'Down'
    if (c === 'ArrowLeft') return 'Left'
    if (c === 'ArrowRight') return 'Right'
    // Legacy format fallback
    if (c === 'CmdOrCtrl') return IS_MAC ? '\u2318' : 'Ctrl'
    return c
  })
}

export function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  const diffMin = Math.floor(diffSec / 60)
  const diffHr = Math.floor(diffMin / 60)
  const diffDay = Math.floor(diffHr / 24)

  if (diffSec < 60) return 'just now'
  if (diffMin < 60) return `${diffMin} min ago`
  if (diffHr < 24) return `${diffHr} hour${diffHr > 1 ? 's' : ''} ago`
  if (diffDay < 7) return `${diffDay} day${diffDay > 1 ? 's' : ''} ago`

  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export function getWeekBarChartData(history: TranscriptionEntry[]): number[] {
  const now = new Date()
  const days: number[] = []
  for (let i = 6; i >= 0; i--) {
    const date = new Date(now)
    date.setDate(date.getDate() - i)
    const dayStr = date.toDateString()
    const words = history
      .filter((e) => new Date(e.timestamp).toDateString() === dayStr)
      .reduce((sum, e) => sum + e.wordCount, 0)
    days.push(words)
  }
  return days
}

export function getYesterdayComparison(history: TranscriptionEntry[]): number | null {
  const now = new Date()
  const today = now.toDateString()
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  const yesterdayStr = yesterday.toDateString()

  const todayWords = history
    .filter((e) => new Date(e.timestamp).toDateString() === today)
    .reduce((sum, e) => sum + e.wordCount, 0)
  const yesterdayWords = history
    .filter((e) => new Date(e.timestamp).toDateString() === yesterdayStr)
    .reduce((sum, e) => sum + e.wordCount, 0)

  if (yesterdayWords === 0) return null
  return Math.round(((todayWords - yesterdayWords) / yesterdayWords) * 100)
}

export function estimateTimeSaved(totalWords: number): string {
  // Assume 40 WPM typing vs 150 WPM speaking
  const typingMinutes = totalWords / 40
  const speakingMinutes = totalWords / 150
  const savedMinutes = typingMinutes - speakingMinutes
  if (savedMinutes < 60) return `${Math.round(savedMinutes)} min`
  const hours = savedMinutes / 60
  return `${hours.toFixed(1)} hrs`
}

export function groupHistoryByDay(history: TranscriptionEntry[]): Map<string, TranscriptionEntry[]> {
  const groups = new Map<string, TranscriptionEntry[]>()
  const sorted = [...history].sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
  for (const entry of sorted) {
    const dayKey = new Date(entry.timestamp).toDateString()
    const group = groups.get(dayKey) ?? []
    group.push(entry)
    groups.set(dayKey, group)
  }
  return groups
}

export function formatDayLabel(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const today = now.toDateString()
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)

  if (dateStr === today) return `Today — ${date.toLocaleDateString('en-US', { month: 'long', day: 'numeric' })}`
  if (dateStr === yesterday.toDateString()) return `Yesterday — ${date.toLocaleDateString('en-US', { month: 'long', day: 'numeric' })}`
  return date.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' })
}
