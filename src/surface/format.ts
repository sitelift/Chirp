import type { Binding } from './types'

/** One key label as shown on a key cap: "Left Ctrl" -> "Ctrl", "Right Ctrl" stays. */
export function capLabel(label: string): string {
  return label
    .trim()
    .replace(/^Left /, '')
    .replace(/Windows/i, 'Win')
    .replace(/Control/i, 'Ctrl')
}

const MODIFIER_ORDER = [/ctrl/i, /shift/i, /alt/i, /win/i]
function capRank(label: string): number {
  const i = MODIFIER_ORDER.findIndex((re) => re.test(label))
  return i < 0 ? MODIFIER_ORDER.length : i
}

/** Key caps in the usual reading order: Ctrl, Shift, Alt, Win, then the key. */
export function sortCaps(labels: string[]): string[] {
  return [...labels].sort((a, b) => capRank(a) - capRank(b))
}

export function bindingCaps(binding: Binding | null): string[] {
  if (!binding || !binding.label) return []
  return sortCaps(binding.label.split(' + ').map(capLabel))
}

/** The hold key as written in "Hold [Right Ctrl] and talk". */
export function holdKeyLabel(hold: Binding | null, toggle: Binding | null): string {
  return bindingCaps(hold ?? toggle).join(' + ') || 'your hotkey'
}

export function relativeLong(iso: string, now = Date.now()): string {
  const s = Math.max(0, Math.floor((now - new Date(iso).getTime()) / 1000))
  if (s < 60) return 'just now'
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min ago`
  const h = Math.floor(m / 60)
  if (h < 24) return h === 1 ? '1 hour ago' : `${h} hours ago`
  const d = Math.floor(h / 24)
  if (d === 1) return 'Yesterday'
  return `${d} days ago`
}

export function relativeShort(iso: string, now = Date.now()): string {
  const s = Math.max(0, Math.floor((now - new Date(iso).getTime()) / 1000))
  if (s < 60) return 'now'
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h`
  const d = Math.floor(h / 24)
  if (d < 7) return `${d}d`
  return `${Math.floor(d / 7)}w`
}

export function etaLabel(etaSec: number | null): string {
  if (etaSec === null) return 'Starting'
  if (etaSec < 50) return 'Less than a minute'
  return `About ${Math.max(1, Math.round(etaSec / 60))} min`
}

/**
 * Short device name: Windows calls inputs "Microphone (Brio 500)"; show
 * "Brio 500", unless another device would get the same short name.
 */
export function deviceLabel(id: string, all: string[] = []): string {
  if (id === 'default') return 'Default'
  const short = (name: string) => {
    const open = name.indexOf('(')
    const close = name.lastIndexOf(')')
    return open > 0 && close > open ? name.slice(open + 1, close).trim() : name
  }
  const s = short(id)
  return all.some((other) => other !== id && short(other) === s) ? id : s
}
