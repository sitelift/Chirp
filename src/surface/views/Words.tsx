import { useEffect, useRef, type KeyboardEvent } from 'react'
import { ArrowRight, Plus, Trash2 } from 'lucide-react'
import { useForced } from '../forced'
import { setUi, useSurface, useSurfaceContext } from '../store'
import { Header, IconButton } from '../parts'
import { drill, saveWords } from '../actions'
import { addItem, editItem, removeItem, wordItems, type WordItem } from '../words'

const fieldClass =
  'h-[32px] min-w-0 rounded-field border-b-2 border-transparent bg-field px-[10px] text-row text-t1 outline-none placeholder:text-t4 focus:border-accent'
const ring = { boxShadow: '0 0 0 1px var(--field-ring)' }

function Arrow() {
  return <ArrowRight size={14} strokeWidth={2} className="shrink-0 text-arrow" />
}

/** The add / edit row: two fields, Enter saves, Esc cancels, leaving the row saves. */
function EditRow({ item }: { item?: WordItem }) {
  const ctx = useSurfaceContext()
  const edit = useSurface((s) => s.ui.words) ?? { mode: 'add' as const, say: '', get: '' }
  const rowRef = useRef<HTMLDivElement>(null)
  const sayRef = useRef<HTMLInputElement>(null)
  const getRef = useRef<HTMLInputElement>(null)
  const editing = item !== undefined

  // Focus once when the row opens: "You say" when adding, "Chirp types" when editing.
  useEffect(() => {
    if (ctx.still) return
    const target = editing ? getRef.current : sayRef.current
    target?.focus()
    if (editing && target) target.setSelectionRange(target.value.length, target.value.length)
  }, [editing, ctx.still])

  const commit = () => {
    const s = ctx.store.getState()
    const cur = s.ui.words
    if (!cur) return // already cancelled or saved
    const stores = { vocabulary: s.vocabulary, snippets: s.snippets }
    const next = item ? editItem(stores, item, cur.say, cur.get) : addItem(stores, cur.say, cur.get)
    if (next !== stores) saveWords(ctx, next)
    setUi(ctx.store, { words: null })
  }
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      commit()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      setUi(ctx.store, { words: null })
    }
  }
  const set = (patch: Partial<typeof edit>) => setUi(ctx.store, { words: { ...edit, ...patch } })

  return (
    <div
      ref={rowRef}
      className={`flex h-[48px] shrink-0 items-center gap-[8px] rounded-row bg-hover ${item ? 'pl-[6px] pr-[4px]' : 'px-[6px]'}`}
      onBlur={(e) => {
        if (!ctx.still && !rowRef.current?.contains(e.relatedTarget as Node)) commit()
      }}
    >
      <input ref={sayRef} value={edit.say} onChange={(e) => set({ say: e.target.value })} onKeyDown={onKey} placeholder="You say" aria-label="You say" className={`${fieldClass} w-[102px] shrink-0`} style={ring} spellCheck={false} />
      <Arrow />
      <input ref={getRef} value={edit.get} onChange={(e) => set({ get: e.target.value })} onKeyDown={onKey} placeholder="Chirp types" aria-label="Chirp types" className={`${fieldClass} flex-1`} style={ring} spellCheck={false} />
      {item && (
        <IconButton
          label="Delete"
          size={28}
          className="rounded-field text-t2"
          onClick={() => {
            const s = ctx.store.getState()
            saveWords(ctx, removeItem({ vocabulary: s.vocabulary, snippets: s.snippets }, item))
            setUi(ctx.store, { words: null })
          }}
        >
          <Trash2 size={15} strokeWidth={1.8} />
        </IconButton>
      )}
    </div>
  )
}

function WordRow({ item, index }: { item: WordItem; index: number }) {
  const ctx = useSurfaceContext()
  const forced = useForced(`word-${index}`)
  return (
    <button
      type="button"
      data-nav=""
      onClick={() => setUi(ctx.store, { words: { mode: 'edit', index, say: item.say, get: item.get } })}
      className="nav flex h-[40px] w-full shrink-0 items-center gap-[8px] rounded-row px-[12px] text-left text-row text-t1 hover:bg-hover active:bg-press data-[force-hover]:bg-hover"
      {...forced}
    >
      <span className="w-[96px] shrink-0 truncate">{item.say}</span>
      <Arrow />
      <span className="min-w-0 flex-1 truncate">{item.get}</span>
    </button>
  )
}

export function Words() {
  const ctx = useSurfaceContext()
  const vocabulary = useSurface((s) => s.vocabulary)
  const snippets = useSurface((s) => s.snippets)
  const edit = useSurface((s) => s.ui.words)
  const items = wordItems(vocabulary, snippets)
  const empty = items.length === 0

  // Empty: the add row is the whole view.
  return (
    <div className="flex flex-col">
      <Header
        title="Words"
        onBack={() => drill(ctx, 'home')}
        action={
          !empty && (
            <IconButton label="Add a word" onClick={() => setUi(ctx.store, { words: { mode: 'add', say: '', get: '' } })} className="text-t1">
              <Plus size={16} strokeWidth={2} />
            </IconButton>
          )
        }
      />
      {(edit?.mode === 'add' || empty) && <EditRow />}
      <div className="-m-[4px] flex max-h-[296px] flex-col overflow-y-auto p-[4px]">
        {items.map((item, i) =>
          edit?.mode === 'edit' && edit.index === i ? <EditRow key={`edit-${i}`} item={item} /> : <WordRow key={`${item.kind}-${item.say}-${i}`} item={item} index={i} />,
        )}
      </div>
    </div>
  )
}
