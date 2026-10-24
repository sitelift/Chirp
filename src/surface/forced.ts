import { useSurface } from './store'

/** Hover / press / focus that the preview can force on a named element. */
export function useForced(id: string | undefined) {
  const ui = useSurface((s) => s.ui)
  if (!id) return {}
  return {
    'data-force-focus': ui.forceFocus === id ? '' : undefined,
    'data-force-hover': ui.forceHover === id ? '' : undefined,
    'data-force-press': ui.forcePress === id ? '' : undefined,
  }
}
