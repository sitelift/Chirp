import { useState } from 'react'
import { useAppStore } from '../../stores/appStore'
import { Button } from '../shared/Button'

export function DictionaryPage() {
  const dictionary = useAppStore((s) => s.dictionary)
  const addEntry = useAppStore((s) => s.addDictionaryEntry)
  const removeEntry = useAppStore((s) => s.removeDictionaryEntry)

  const [newFrom, setNewFrom] = useState('')
  const [newTo, setNewTo] = useState('')

  const handleAdd = () => {
    const from = newFrom.trim()
    const to = newTo.trim()
    if (!from || !to) return
    addEntry(from, to)
    setNewFrom('')
    setNewTo('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleAdd()
  }

  return (
    <div className="flex flex-col gap-5 animate-slide-up">
      <div className="mb-1">
        <h1 className="font-display font-extrabold text-2xl text-[#1a1a1a] tracking-[-0.5px]">
          Dictionary
        </h1>
        <p className="text-[13px] text-[#aaa] mt-1">
          Words and phrases Chirp should always spell or format a specific way.
        </p>
      </div>

      {dictionary.length > 0 ? (
        <div className="overflow-hidden rounded-card border border-card-border">
          {/* Header */}
          <div className="flex bg-[#FAFAF8] px-[18px] py-2.5">
            <span className="flex-1 text-[11px] font-semibold uppercase tracking-[0.5px] text-[#aaa]">
              Heard
            </span>
            <span className="flex-1 text-[11px] font-semibold uppercase tracking-[0.5px] text-[#aaa]">
              Replace with
            </span>
            <span className="w-10" />
          </div>

          {/* Rows */}
          {dictionary.map((entry, i) => (
            <div
              key={i}
              className={`flex items-center px-[18px] h-11 border-b border-[#F5F4F0] last:border-b-0 transition-colors hover:bg-[#FAFAF8] group animate-slide-up ${
                i % 2 === 0 ? 'bg-white' : 'bg-[#FAFAF8]/50'
              }`}
              style={{ animationDelay: `${i * 30}ms` }}
            >
              <span className="flex-1 text-[13px] text-[#333]">
                {entry.from}
              </span>
              <span className="flex-1 text-[13px] text-[#333]">
                {entry.to}
              </span>
              <button
                onClick={() => removeEntry(i)}
                className="flex h-8 w-10 items-center justify-center text-[#ccc] hover:text-chirp-error transition-colors duration-150 opacity-0 group-hover:opacity-100"
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="flex items-center justify-center rounded-card border border-dashed border-card-border bg-[#FAFAF8] px-6 py-10">
          <p className="text-[13px] text-[#aaa] text-center">
            No entries yet. Add words and phrases Chirp should always format correctly.
          </p>
        </div>
      )}

      {/* Add row */}
      <div className="flex items-center gap-3">
        <input
          type="text"
          value={newFrom}
          onChange={(e) => setNewFrom(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="New phrase..."
          className="flex-1 h-10 rounded-lg border border-card-border bg-white px-3 text-[13px] text-[#333] placeholder:text-[#ccc] focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
        />
        <input
          type="text"
          value={newTo}
          onChange={(e) => setNewTo(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Replaced by..."
          className="flex-1 h-10 rounded-lg border border-card-border bg-white px-3 text-[13px] text-[#333] placeholder:text-[#ccc] focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
        />
        <Button onClick={handleAdd} disabled={!newFrom.trim() || !newTo.trim() || dictionary.length >= 500}>
          Add
        </Button>
      </div>

      {dictionary.length >= 450 && (
        <p className="text-xs text-chirp-error">
          You're approaching the maximum of 500 entries ({dictionary.length}/500).
        </p>
      )}
    </div>
  )
}
