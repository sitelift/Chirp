# Overlay Free Position — Design Spec

## Context

Users are stuck with the overlay at bottom-center or top-center of the primary monitor. People are complaining about the fixed position. This change lets users drag the overlay to any position on screen via a full-screen reposition mode triggered from settings.

## Overview

Replace the top/bottom segmented control in settings with a "Reposition overlay" button. Clicking it enters a full-screen drag mode where the user sees their desktop through a dark semi-transparent backdrop with the overlay pill rendered at its current position. They drag the pill wherever they want, drop it, click "Done" to confirm (or Escape to cancel). Position is saved as x/y percentages and persists across sessions.

## Data Model

### Before
```typescript
overlayPosition: 'bottom' | 'top'
```

### After
```typescript
overlayPosition: { x: number; y: number }  // percentages 0-100
```

- `x: 0` = left edge, `x: 100` = right edge, `x: 50` = centered horizontally
- `y: 0` = top edge, `y: 100` = bottom edge, `y: 90` = near bottom (current default)
- Default: `{ x: 50, y: 90 }` (matches current bottom-center behavior)

### Migration
When reading `overlayPosition` from stored settings:
- `'bottom'` (string) → `{ x: 50, y: 90 }`
- `'top'` (string) → `{ x: 50, y: 10 }`
- `{ x, y }` (object) → use as-is

Migration happens in `Overlay.tsx` position calculation and in the settings UI. No Rust changes needed — settings are arbitrary JSON passed through `update_settings`.

## Reposition Mode Flow

### Entry
1. User clicks "Reposition overlay" button in Settings page
2. Settings window emits Tauri event `enter-reposition-mode` to the overlay window
3. Overlay window enters reposition mode

### Reposition Mode (in Overlay window)
1. Overlay window resizes to fullscreen (entire primary monitor)
2. Background renders as `bg-black/60` — dark but see-through
3. `setIgnoreCursorEvents(false)` — window becomes interactive
4. The pill renders at its current position, styled as draggable (grab cursor, subtle glow/border to indicate draggability)
5. Instructional text at top: "Drag the pill to reposition" with "Press Escape to cancel"
6. User drags the pill via mouse (standard React drag: `onMouseDown` → track delta → update position)
7. On drop: pill stays at new position, a small "Done" button appears near the pill (below or beside it)
8. User clicks "Done":
   - Calculate x/y percentages from pixel position relative to screen size
   - Save via `updateSettings({ overlayPosition: { x, y } })`
   - Exit reposition mode (shrink window back, re-enable cursor passthrough)
   - Emit `reposition-complete` event so settings window can update its display
9. User presses Escape at any point: cancel, restore original position, exit reposition mode

### Exit
1. Window resizes back to `WIN_W x WIN_H` at the new position
2. `setIgnoreCursorEvents(true)` re-enabled
3. Normal overlay behavior resumes

## Settings UI

### Before
```
Overlay position          [Bottom] [Top]
```

### After
```
Overlay position          [Reposition]
  Currently: bottom center
```

- "Reposition" is a button styled like existing controls (dark bg, small)
- "Currently: bottom center" (or "top left", "center", etc.) — computed label from x/y values using named zones:
  - x < 25 = "left", 25-75 = "center", > 75 = "right"
  - y < 25 = "top", 25-75 = "middle", > 75 = "bottom"
  - Combined: "bottom center", "top left", "middle right", etc.

## Position Calculation (Overlay.tsx)

Current code:
```typescript
const x = Math.round((screenW - WIN_W) / 2)
const y = position === 'top' ? OFFSET : Math.round(screenH - OFFSET - WIN_H)
```

New code:
```typescript
const pos = normalizePosition(position) // handles migration
const x = Math.round((screenW - WIN_W) * (pos.x / 100))
const y = Math.round((screenH - WIN_H) * (pos.y / 100))
```

Where `normalizePosition` handles the `'top'`/`'bottom'` string → object migration.

## Files to Modify

1. **`src/lib/constants.ts`** — Update `DEFAULT_SETTINGS.overlayPosition` to `{ x: 50, y: 90 }`
2. **`src/stores/appStore.ts`** — Update `overlayPosition` type to `{ x: number; y: number }`, add `repositionMode: boolean` state + setter
3. **`src/components/overlay/Overlay.tsx`** — Reposition mode (fullscreen backdrop, draggable pill, done/cancel), updated position calculation with migration
4. **`src/components/settings/SettingsPage.tsx`** — Replace SegmentedControl with Reposition button + position label, emit `enter-reposition-mode` event
5. **`src/hooks/useSettingsSync.ts`** — Listen for `reposition-complete` event if needed (may not be necessary since settings sync already handles `overlayPosition` changes)

## Edge Cases

- **Recording starts during reposition mode**: Exit reposition mode immediately, let recording proceed normally. The overlay should never block dictation.
- **Settings window closed during reposition**: Listen for settings window close, cancel reposition mode.
- **Position out of bounds after monitor change**: Clamp x/y to 0-100 range on read. If a user disconnects a monitor and the overlay would be offscreen, the percentage-based system handles it naturally since it's relative to whatever monitor is primary.

## Verification

1. Open settings → see "Reposition" button where the old top/bottom toggle was
2. Click "Reposition" → screen goes dark with semi-transparent backdrop, pill visible at current position
3. Drag pill to top-right corner → pill follows mouse smoothly
4. "Done" button appears after drop → click it → backdrop disappears, overlay stays at new position
5. Press Escape instead of Done → overlay returns to original position
6. Close and reopen app → overlay appears at saved position
7. Upgrade from old version with `overlayPosition: 'bottom'` stored → overlay appears at bottom-center, settings shows "bottom center"
8. Start recording while in reposition mode → reposition cancels, recording proceeds normally
