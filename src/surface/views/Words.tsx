import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Header } from '../parts'
import { drill, saveVocabulary } from '../actions'
import { animateRow } from '../motion'
import { IconAdd, IconRemove } from '../icons'

/** A list row that grows in when it is new and folds away when removed. */
export function ListRow({ id, fresh, tall, onRemove, children }: { id: string; fresh: boolean; tall?: boolean; onRemove: (el: HTMLElement) => void; children: ReactNode }) {
  const el = useRef<HTMLDivElement>(null)
  const forced = useForced(id)
  // Only the row just added grows in, once, when it mounts.
  const [growIn] = useState(fresh)
  useLayoutEffect(() => {
    if (growIn && el.current) animateRow(el.current, 'in')
  }, [growIn])
  return (
    <div
      ref={el}
      className={tall ? 'row tall wrow' : 'row wrow'}
      {...forced}
    >
      {children}
      <button type="button" data-nav="" className="nav del" aria-label="Remove" title="Remove" onClick={() => el.current && onRemove(el.current)}>
        <IconRemove size={14} />
      </button>
    </div>
  )
}

/** Names and words Chirp should spell exactly as written. */
export function Words() {
  const ctx = useSurfaceContext()
  const vocabulary = useSurface((s) => s.vocabulary)
  const focus = useForced('word-input')
  const [draft, setDraft] = useState('')
  const [fresh, setFresh] = useState<string | null>(null)
  const leaving = useRef(new Set<string>())

  const terms = () => ctx.store.getState().vocabulary.map((v) => v.term)
  const add = () => {
    const word = draft.trim()
    if (!word) return
    setDraft('')
    const list = terms()
    const i = list.findIndex((t) => t.toLowerCase() === word.toLowerCase())
    if (i >= 0) {
      if (list[i] !== word) saveVocabulary(ctx, list.map((t, j) => (j === i ? word : t)))
      return
    }
    setFresh(word)
    saveVocabulary(ctx, [word, ...list])
  }
  const remove = (term: string, el: HTMLElement) => {
    if (leaving.current.has(term)) return
    leaving.current.add(term)
    animateRow(el, 'out').then(() => {
      leaving.current.delete(term)
      saveVocabulary(
        ctx,
        terms().filter((t) => t !== term),
      )
    })
  }

  return (
    <>
      <Header title="Words" onBack={() => drill(ctx, 'home')} />
      <div className="addrow" {...focus}>
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && add()}
          placeholder="Add a name or word"
          aria-label="Add a name or word"
          spellCheck={false}
          autoComplete="off"
        />
        <button type="button" className="addbtn" aria-label="Add" title="Add" onClick={add}>
          <IconAdd />
        </button>
      </div>
      <div className="scroll">
        {vocabulary.map((v) => (
          <ListRow key={v.term} id={`word-${v.term}`} fresh={v.term === fresh} onRemove={(el) => remove(v.term, el)}>
            <span className="clamp">{v.term}</span>
          </ListRow>
        ))}
      </div>
      <div className="hint">Chirp spells these exactly as written.</div>
    </>
  )
}
