import { useState } from 'react'
import { Pencil, Check, X } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { Button } from '../shared/Button'

export function SnippetsPage() {
  const snippets = useAppStore((s) => s.snippets)
  const addSnippet = useAppStore((s) => s.addSnippet)
  const updateSnippet = useAppStore((s) => s.updateSnippet)
  const removeSnippet = useAppStore((s) => s.removeSnippet)

  const [newTrigger, setNewTrigger] = useState('')
  const [newExpansion, setNewExpansion] = useState('')
  const [editIndex, setEditIndex] = useState<number | null>(null)
  const [editTrigger, setEditTrigger] = useState('')
  const [editExpansion, setEditExpansion] = useState('')

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

  const startEdit = (index: number) => {
    setEditIndex(index)
    setEditTrigger(snippets[index].trigger)
    setEditExpansion(snippets[index].expansion)
  }

  const cancelEdit = () => setEditIndex(null)

  const saveEdit = () => {
    if (editIndex === null) return
    const trigger = editTrigger.trim()
    const expansion = editExpansion.trim()
    if (!trigger || !expansion) return
    updateSnippet(editIndex, trigger, expansion)
    setEditIndex(null)
  }

  const handleEditKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) saveEdit()
    if (e.key === 'Escape') cancelEdit()
  }

  return (
    <div className="flex flex-col gap-5 animate-slide-up">
      <div className="mb-1">
        <h1 className="font-display font-extrabold text-2xl text-[#1a1a1a] tracking-[-0.5px]">
          Snippets
        </h1>
        <p className="text-[13px] text-[#aaa] mt-1">
          Voice-triggered text expansion. Say the trigger phrase during dictation and Chirp will replace it with the full text.
        </p>
      </div>

      {snippets.length > 0 ? (
        <div className="overflow-hidden rounded-card border border-card-border">
          {/* Header */}
          <div className="flex bg-[#FAFAF8] px-[18px] py-2.5">
            <span className="w-[35%] text-[11px] font-semibold uppercase tracking-[0.5px] text-[#aaa]">
              Trigger
            </span>
            <span className="flex-1 text-[11px] font-semibold uppercase tracking-[0.5px] text-[#aaa]">
              Expands to
            </span>
            <span className="w-20" />
          </div>

          {/* Rows */}
          {snippets.map((entry, i) => (
            <div
              key={i}
              className={`flex items-start px-[18px] py-2.5 border-b border-[#F5F4F0] last:border-b-0 transition-colors hover:bg-[#FAFAF8] group animate-slide-up ${
                i % 2 === 0 ? 'bg-white' : 'bg-[#FAFAF8]/50'
              }`}
              style={{ animationDelay: `${i * 30}ms` }}
            >
              {editIndex === i ? (
                <>
                  <div className="w-[35%] pr-3">
                    <input
                      type="text"
                      value={editTrigger}
                      onChange={(e) => setEditTrigger(e.target.value.slice(0, 60))}
                      onKeyDown={handleEditKeyDown}
                      autoFocus
                      className="w-full h-8 rounded-md border border-chirp-yellow bg-white px-2 text-[13px] text-[#333] focus:outline-none"
                    />
                  </div>
                  <div className="flex-1">
                    <textarea
                      value={editExpansion}
                      onChange={(e) => setEditExpansion(e.target.value.slice(0, 4000))}
                      onKeyDown={handleEditKeyDown}
                      rows={2}
                      className="w-full rounded-md border border-chirp-yellow bg-white px-2 py-1 text-[13px] text-[#333] focus:outline-none resize-none"
                    />
                  </div>
                  <div className="flex w-20 items-center justify-end gap-1 shrink-0">
                    <button
                      onClick={saveEdit}
                      disabled={!editTrigger.trim() || !editExpansion.trim()}
                      className="flex h-8 w-8 items-center justify-center text-chirp-success hover:text-chirp-success/80 disabled:text-[#ccc] transition-colors duration-150"
                    >
                      <Check size={16} />
                    </button>
                    <button
                      onClick={cancelEdit}
                      className="flex h-8 w-8 items-center justify-center text-[#aaa] hover:text-[#555] transition-colors duration-150"
                    >
                      <X size={16} />
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <span className="w-[35%] text-[13px] text-[#333] pr-3">
                    {entry.trigger}
                  </span>
                  <span className="flex-1 text-[13px] text-[#333] whitespace-pre-wrap">
                    {entry.expansion}
                  </span>
                  <div className="flex w-20 items-center justify-end gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={() => startEdit(i)}
                      className="flex h-8 w-8 items-center justify-center text-[#ccc] hover:text-chirp-yellow transition-colors duration-150"
                    >
                      <Pencil size={14} />
                    </button>
                    <button
                      onClick={() => removeSnippet(i)}
                      className="flex h-8 w-8 items-center justify-center text-[#ccc] hover:text-chirp-error transition-colors duration-150"
                    >
                      ✕
                    </button>
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      ) : (
        <div className="flex items-center justify-center rounded-card border border-dashed border-card-border bg-[#FAFAF8] px-6 py-10">
          <p className="text-[13px] text-[#aaa] text-center">
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
          className="h-10 rounded-lg border border-card-border bg-white px-3 text-[13px] text-[#333] placeholder:text-[#ccc] focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
        />
        <textarea
          value={newExpansion}
          onChange={(e) => setNewExpansion(e.target.value.slice(0, 4000))}
          onKeyDown={handleKeyDown}
          placeholder="Expands to..."
          rows={3}
          className="rounded-lg border border-card-border bg-white px-3 py-2 text-[13px] text-[#333] placeholder:text-[#ccc] focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150 resize-none"
        />
        <Button onClick={handleAdd} disabled={!newTrigger.trim() || !newExpansion.trim() || snippets.length >= 100}>
          Add Snippet
        </Button>
      </div>

      <p className="text-xs text-[#aaa]">
        Triggers are matched case-insensitively during dictation.
      </p>

      {snippets.length >= 90 && (
        <p className="text-xs text-chirp-error">
          You're approaching the maximum of 100 entries ({snippets.length}/100).
        </p>
      )}
    </div>
  )
}
