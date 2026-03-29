const IS_MAC = navigator.platform.includes('Mac')

const MODIFIER_CODES = new Set([
  'ControlLeft', 'ControlRight',
  'ShiftLeft', 'ShiftRight',
  'AltLeft', 'AltRight',
  'MetaLeft', 'MetaRight',
])

const MODIFIER_ORDER = [
  'ControlLeft', 'ControlRight',
  'AltLeft', 'AltRight',
  'ShiftLeft', 'ShiftRight',
  'MetaLeft', 'MetaRight',
]

export function codeToLabel(code: string): string {
  if (code === 'ControlLeft' || code === 'ControlRight') return 'Ctrl'
  if (code === 'ShiftLeft' || code === 'ShiftRight') return 'Shift'
  if (code === 'AltLeft' || code === 'AltRight') return IS_MAC ? 'Option' : 'Alt'
  if (code === 'MetaLeft' || code === 'MetaRight') return IS_MAC ? '\u2318' : 'Win'
  if (code === 'Fn') return 'fn'
  if (code.startsWith('Key') && code.length === 4) return code[3]
  if (code.startsWith('Digit') && code.length === 6) return code[5]
  if (code === 'ArrowUp') return 'Up'
  if (code === 'ArrowDown') return 'Down'
  if (code === 'ArrowLeft') return 'Left'
  if (code === 'ArrowRight') return 'Right'
  if (code === 'Backquote') return '`'
  if (code === 'Minus') return '-'
  if (code === 'Equal') return '='
  if (code === 'BracketLeft') return '['
  if (code === 'BracketRight') return ']'
  if (code === 'Semicolon') return ';'
  if (code === 'Quote') return "'"
  if (code === 'Backslash' || code === 'IntlBackslash') return '\\'
  if (code === 'Comma') return ','
  if (code === 'Period') return '.'
  if (code === 'Slash') return '/'
  return code
}

export interface CapturedHotkey {
  hotkey: string
  labels: string[]
}

export interface StickyCapture {
  keys: Set<string>
}

export function createStickyCapture(): StickyCapture {
  return { keys: new Set() }
}

export function addKeyToCapture(capture: StickyCapture, event: KeyboardEvent): StickyCapture {
  const code = event.code
  if (!code) return capture
  const next = { keys: new Set(capture.keys) }
  next.keys.add(code)
  return next
}

export function addSystemKeyToCapture(capture: StickyCapture, code: string): StickyCapture {
  const next = { keys: new Set(capture.keys) }
  next.keys.add(code)
  return next
}

export function getCaptureLabels(capture: StickyCapture): string[] {
  const mods: string[] = []
  const rest: string[] = []
  for (const code of capture.keys) {
    if (MODIFIER_CODES.has(code)) {
      mods.push(code)
    } else {
      rest.push(code)
    }
  }
  mods.sort((a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b))
  rest.sort()
  return [...mods, ...rest].map(codeToLabel)
}

function getOrderedCodes(capture: StickyCapture): string[] {
  const mods: string[] = []
  const rest: string[] = []
  for (const code of capture.keys) {
    if (MODIFIER_CODES.has(code)) {
      mods.push(code)
    } else {
      rest.push(code)
    }
  }
  mods.sort((a, b) => MODIFIER_ORDER.indexOf(a) - MODIFIER_ORDER.indexOf(b))
  rest.sort()
  return [...mods, ...rest]
}

export function buildHotkeyString(capture: StickyCapture): CapturedHotkey | null {
  if (capture.keys.size === 0) return null
  const codes = getOrderedCodes(capture)
  return {
    hotkey: codes.join('+'),
    labels: codes.map(codeToLabel),
  }
}

export function captureIsValid(capture: StickyCapture): boolean {
  return capture.keys.size > 0
}

export function captureIsModifierOnly(capture: StickyCapture): boolean {
  if (capture.keys.size === 0) return false
  for (const code of capture.keys) {
    if (!MODIFIER_CODES.has(code)) return false
  }
  return true
}
