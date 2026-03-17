import { useState } from 'react'
import { Trash2 } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { Button } from '../shared/Button'

export function SnippetsPage() {
  const snippets = useAppStore((s) => s.snippets)
  const addSnippet = useAppStore((s) => s.addSnippet)
  const removeSnippet = useAppStore((s) => s.removeSnippet)

  const [newTrigger, setNewTrigger] = useState('')
  const [newExpansion, setNewExpansion] = useState('')

  const handleAdd = () => {
    const trigger = newTrigger.trim()
    const expansion = newExpansion.trim()
    if (!trigger || !expansion) return
    addSnippet(trigger, expansion)
    setNewTrigger('')
    setNewExpansion('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) handleAdd()
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">Snippets</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">
          Voice-triggered text expansion. Say the trigger phrase during dictation and Chirp will replace it with the full text.
        </p>
      </div>

      {snippets.length > 0 ? (
        <div className="overflow-hidden rounded-xl border border-chirp-stone-200">
          {/* Header */}
          <div className="flex bg-chirp-stone-100 px-4 py-2.5">
            <span className="w-[35%] font-body text-xs font-semibold uppercase tracking-[0.5px] text-chirp-stone-500">
              Trigger
            </span>
            <span className="flex-1 font-body text-xs font-semibold uppercase tracking-[0.5px] text-chirp-stone-500">
              Expands to
            </span>
            <span className="w-10" />
          </div>

          {/* Rows */}
          {snippets.map((entry, i) => (
            <div
              key={i}
              className={`flex items-start px-4 py-2.5 ${
                i % 2 === 0 ? 'bg-white' : 'bg-chirp-stone-100/50'
              }`}
            >
              <span className="w-[35%] font-body text-sm text-chirp-stone-700 pr-3">
                {entry.trigger}
              </span>
              <span className="flex-1 font-body text-sm text-chirp-stone-700 whitespace-pre-wrap">
                {entry.expansion}
              </span>
              <button
                onClick={() => removeSnippet(i)}
                className="flex h-8 w-10 items-center justify-center text-chirp-stone-400 hover:text-chirp-error transition-colors duration-150 shrink-0"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="flex items-center justify-center rounded-xl border border-dashed border-chirp-stone-200 bg-chirp-stone-100/50 px-6 py-10">
          <p className="font-body text-sm text-chirp-stone-500 text-center">
            No snippets yet. Add trigger phrases that expand into longer text during dictation.
          </p>
        </div>
      )}

      {/* Add row */}
      <div className="flex flex-col gap-3">
        <input
          type="text"
          value={newTrigger}
          onChange={(e) => setNewTrigger(e.target.value.slice(0, 60))}
          onKeyDown={handleKeyDown}
          placeholder="Trigger phrase..."
          className="h-10 rounded-lg border border-chirp-stone-200 bg-white px-3 font-body text-sm text-chirp-stone-700 placeholder:text-chirp-stone-500 placeholder:italic focus:border-2 focus:border-chirp-amber-400 focus:outline-none transition-colors duration-150"
        />
        <textarea
          value={newExpansion}
          onChange={(e) => setNewExpansion(e.target.value.slice(0, 4000))}
          onKeyDown={handleKeyDown}
          placeholder="Expands to..."
          rows={3}
          className="rounded-lg border border-chirp-stone-200 bg-white px-3 py-2 font-body text-sm text-chirp-stone-700 placeholder:text-chirp-stone-500 placeholder:italic focus:border-2 focus:border-chirp-amber-400 focus:outline-none transition-colors duration-150 resize-none"
        />
        <Button onClick={handleAdd} disabled={!newTrigger.trim() || !newExpansion.trim()}>
          Add Snippet
        </Button>
      </div>

      <p className="font-body text-xs text-chirp-stone-400">
        Triggers are matched case-insensitively during dictation.
      </p>

      {snippets.length >= 90 && (
        <p className="font-body text-xs text-chirp-error">
          You're approaching the maximum of 100 entries ({snippets.length}/100).
        </p>
      )}
    </div>
  )
}
