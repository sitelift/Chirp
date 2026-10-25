import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Header } from '../parts'
import { relativeShort } from '../format'
import { copyText, drill } from '../actions'
import type { HistoryEntry } from '../types'

function RecentRow({ entry }: { entry: HistoryEntry }) {
  const ctx = useSurfaceContext()
  const copied = useSurface((s) => s.ui.copied === entry.timestamp)
  const forced = useForced(`recent-${entry.timestamp}`)
  return (
    <button type="button" data-nav="" className="nav row pr-[12px]" title={entry.text} onClick={() => copyText(ctx, entry.text, entry.timestamp)} {...forced}>
      <span className="clamp">{entry.text}</span>
      <span className={copied ? 'when text-t1' : 'when'}>{copied ? 'Copied' : relativeShort(entry.timestamp)}</span>
    </button>
  )
}

/** The last 20 dictations; click one to copy it. */
export function Recent() {
  const ctx = useSurfaceContext()
  const history = useSurface((s) => s.history)
  const recent = history.slice(-20).reverse()
  return (
    <>
      <Header title="Recent" onBack={() => drill(ctx, 'home')} />
      <div className="scroll">
        {recent.map((e) => (
          <RecentRow key={e.timestamp} entry={e} />
        ))}
      </div>
      {recent.length === 0 && <div className="hint">Nothing yet. Dictations stay on this PC.</div>}
    </>
  )
}
