import { Check, Copy, ListPlus } from 'lucide-react'
import { useForced } from '../forced'
import { setUi, useSurface, useSurfaceContext } from '../store'
import { Header } from '../parts'
import { relativeShort } from '../format'
import { copyText, drill } from '../actions'
import type { HistoryEntry } from '../types'

function RecentRow({ entry }: { entry: HistoryEntry }) {
  const ctx = useSurfaceContext()
  const copied = useSurface((s) => s.ui.copied === entry.timestamp)
  const forced = useForced(`recent-${entry.timestamp}`)
  const addToWords = () => {
    drill(ctx, 'words')
    // Short dictations are usually the misheard word itself.
    const say = entry.text.split(/\s+/).length <= 3 ? entry.text.replace(/[.,!?]+$/, '') : ''
    setUi(ctx.store, { words: { mode: 'add', say, get: '' } })
  }
  return (
    <div
      role="button"
      tabIndex={0}
      data-nav=""
      onClick={() => copyText(ctx, entry.text, entry.timestamp)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          copyText(ctx, entry.text, entry.timestamp)
        }
      }}
      className="nav group flex h-[40px] shrink-0 items-center gap-[12px] rounded-row px-[12px] hover:bg-hover active:bg-press data-[force-hover]:bg-hover"
      style={copied ? { background: 'var(--hover)' } : undefined}
      title={entry.text}
      {...forced}
    >
      <span className="min-w-0 flex-1 truncate font-display text-row text-t1">{entry.text}</span>
      {copied ? (
        <span className="flex shrink-0 items-center gap-[4px] text-meta text-t1">
          <Check size={13} strokeWidth={2.5} />
          Copied
        </span>
      ) : (
        <>
          <span className="w-[28px] shrink-0 text-right font-display text-meta text-t3 group-hover:hidden group-focus-within:hidden group-data-[force-hover]:hidden">{relativeShort(entry.timestamp)}</span>
          <span className="hidden shrink-0 items-center gap-[12px] text-t1 group-hover:flex group-focus-within:flex group-data-[force-hover]:flex">
            <button
              type="button"
              aria-label="Add to Words"
              title="Add to Words"
              onClick={(e) => {
                e.stopPropagation()
                addToWords()
              }}
              className="flex"
            >
              <ListPlus size={15} strokeWidth={1.8} />
            </button>
            <Copy size={15} strokeWidth={1.8} aria-hidden />
          </span>
        </>
      )}
    </div>
  )
}

export function Recent() {
  const ctx = useSurfaceContext()
  const history = useSurface((s) => s.history)
  const recent = history.slice(-20).reverse()
  return (
    <div className="flex flex-col">
      <Header title="Recent" onBack={() => drill(ctx, 'home')} />
      <div className="-m-[4px] flex max-h-[328px] flex-col overflow-y-auto p-[4px]">
        {recent.map((e) => (
          <RecentRow key={e.timestamp} entry={e} />
        ))}
        {recent.length === 0 && <div className="flex h-[40px] items-center px-[12px] text-row text-t4">Nothing yet. Dictations stay on this PC.</div>}
      </div>
    </div>
  )
}
