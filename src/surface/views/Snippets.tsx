import { useRef, useState } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Header } from '../parts'
import { drill, saveSnippets } from '../actions'
import { animateRow } from '../motion'
import { IconAdd } from '../icons'
import { ListRow } from './Words'

/** Say a phrase, Chirp types the rest. */
export function Snippets() {
  const ctx = useSurfaceContext()
  const snippets = useSurface((s) => s.snippets)
  const focus = useForced('snippet-form')
  const [say, setSay] = useState('')
  const [type, setType] = useState('')
  const [fresh, setFresh] = useState<string | null>(null)
  const sayRef = useRef<HTMLInputElement>(null)
  const typeRef = useRef<HTMLInputElement>(null)
  const leaving = useRef(new Set<string>())

  const add = () => {
    const trigger = say.trim()
    const expansion = type.trim()
    if (!trigger || !expansion) return
    const rest = ctx.store.getState().snippets.filter((s) => s.trigger.toLowerCase() !== trigger.toLowerCase())
    setSay('')
    setType('')
    setFresh(trigger)
    saveSnippets(ctx, [{ trigger, expansion }, ...rest])
    sayRef.current?.focus({ preventScroll: true })
  }
  const remove = (trigger: string, el: HTMLElement) => {
    if (leaving.current.has(trigger)) return
    leaving.current.add(trigger)
    animateRow(el, 'out').then(() => {
      leaving.current.delete(trigger)
      saveSnippets(
        ctx,
        ctx.store.getState().snippets.filter((s) => s.trigger !== trigger),
      )
    })
  }

  return (
    <>
      <Header title="Snippets" onBack={() => drill(ctx, 'home')} />
      <div className="snipform" {...focus}>
        <div className="line">
          <input
            ref={sayRef}
            value={say}
            onChange={(e) => setSay(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && typeRef.current?.focus()}
            placeholder="When I say"
            aria-label="When I say"
            spellCheck={false}
            autoComplete="off"
          />
        </div>
        <div className="line">
          <input
            ref={typeRef}
            value={type}
            onChange={(e) => setType(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && add()}
            placeholder="Type this"
            aria-label="Type this"
            spellCheck={false}
            autoComplete="off"
          />
          <button type="button" className="addbtn" aria-label="Add" title="Add" onClick={add}>
            <IconAdd />
          </button>
        </div>
      </div>
      <div className="scroll">
        {snippets.map((s) => (
          <ListRow key={s.trigger} id={`snippet-${s.trigger}`} tall fresh={s.trigger === fresh} onRemove={(el) => remove(s.trigger, el)}>
            <span className="word clamp">
              <span className="clamp">{s.trigger}</span>
              <small className="clamp">{s.expansion}</small>
            </span>
          </ListRow>
        ))}
      </div>
      <div className="hint">Say the phrase and Chirp types the rest.</div>
    </>
  )
}
