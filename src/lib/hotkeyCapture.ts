const IS_MAC = navigator.platform.includes('Mac')

const MODIFIER_EVENTS = new Set(['Control', 'Shift', 'Alt', 'Meta'])

/** Normalize a DOM key name into a display label */
function keyToLabel(key: string): string | null {
  if (MODIFIER_EVENTS.has(key)) {
    switch (key) {
      case 'Control': return IS_MAC ? '\u2318' : 'Ctrl'
      case 'Meta': return IS_MAC ? '\u2318' : 'Ctrl'
      case 'Alt': return IS_MAC ? 'Option' : 'Alt'
      case 'Shift': return 'Shift'
    }
  }

  if (key.length === 1) {
    if (key === ' ') return 'Space'
    return key.toUpperCase()
  }

  switch (key) {
    case ' ':
    case 'Spacebar':
      return 'Space'
    case 'ArrowUp': return 'Up'
    case 'ArrowDown': return 'Down'
    case 'ArrowLeft': return 'Left'
    case 'ArrowRight': return 'Right'
    case 'Escape':
    case 'Enter':
    case 'Tab':
    case 'Backspace':
    case 'Delete':
    case 'Insert':
    case 'Home':
    case 'End':
    case 'PageUp':
    case 'PageDown':
      return key
    default:
      if (/^F\d{1,2}$/i.test(key)) {
        return key.toUpperCase()
      }
      return null
  }
}

/** Normalize a DOM key name into a Tauri shortcut string part */
function keyToShortcutPart(key: string): string | null {
  if (MODIFIER_EVENTS.has(key)) {
    switch (key) {
      case 'Control': return 'CmdOrCtrl'
      case 'Meta': return 'CmdOrCtrl'
      case 'Alt': return 'Alt'
      case 'Shift': return 'Shift'
    }
  }

  // Non-modifier — same normalization as labels
  return keyToLabel(key)
}

export interface CapturedHotkey {
  hotkey: string
  labels: string[]
}

/** Ordering: modifiers first (Ctrl, Alt, Shift), then regular key */
const MODIFIER_ORDER = ['CmdOrCtrl', 'Alt', 'Shift'] as const

/**
 * Accumulated key state for sticky capture.
 * Keys are added on press and stay until confirmed or cancelled.
 */
export interface StickyCapture {
  /** Modifier shortcut parts accumulated (e.g., 'CmdOrCtrl', 'Shift') */
  modifiers: Set<string>
  /** Display labels for modifiers */
  modifierLabels: Map<string, string>
  /** The main (non-modifier) key shortcut part, if pressed */
  mainKey: string | null
  /** Display label for the main key */
  mainKeyLabel: string | null
}

export function createStickyCapture(): StickyCapture {
  return {
    modifiers: new Set(),
    modifierLabels: new Map(),
    mainKey: null,
    mainKeyLabel: null,
  }
}

/** Add a keydown event to the sticky capture. Returns false if key is unrecognized. */
export function addKeyToCapture(capture: StickyCapture, event: KeyboardEvent): StickyCapture | null {
  const key = event.key

  if (MODIFIER_EVENTS.has(key)) {
    const part = keyToShortcutPart(key)
    const label = keyToLabel(key)
    if (!part || !label) return null

    const next = {
      ...capture,
      modifiers: new Set(capture.modifiers),
      modifierLabels: new Map(capture.modifierLabels),
    }
    next.modifiers.add(part)
    next.modifierLabels.set(part, label)
    return next
  }

  // Non-modifier key
  const part = keyToShortcutPart(key)
  const label = keyToLabel(key)
  if (!part || !label) return null

  return {
    ...capture,
    mainKey: part,
    mainKeyLabel: label,
  }
}

/** Get ordered display labels from a sticky capture */
export function getCaptureLabels(capture: StickyCapture): string[] {
  const labels: string[] = []
  for (const mod of MODIFIER_ORDER) {
    const label = capture.modifierLabels.get(mod)
    if (label) labels.push(label)
  }
  if (capture.mainKeyLabel) {
    labels.push(capture.mainKeyLabel)
  }
  return labels
}

/** Build the Tauri shortcut string from a sticky capture, or null if incomplete.
 *  Requires at least one non-modifier key (modifier-only combos are not supported). */
export function buildHotkeyString(capture: StickyCapture): CapturedHotkey | null {
  if (!capture.mainKey) return null

  const parts: string[] = []
  for (const mod of MODIFIER_ORDER) {
    if (capture.modifiers.has(mod)) parts.push(mod)
  }
  parts.push(capture.mainKey)

  return {
    hotkey: parts.join('+'),
    labels: getCaptureLabels(capture),
  }
}

/** Check if the capture has a valid (confirmable) shortcut — requires a non-modifier key */
export function captureIsValid(capture: StickyCapture): boolean {
  return capture.mainKey !== null
}

/** Check if the capture has any keys at all (for showing preview) */
export function captureHasKeys(capture: StickyCapture): boolean {
  return capture.modifiers.size > 0 || capture.mainKey !== null
}
