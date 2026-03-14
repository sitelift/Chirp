import { useState } from 'react'
import { Trash2 } from 'lucide-react'
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
    <div className="flex flex-col gap-6">
      <div>
        <h2 className="font-display font-bold text-lg text-chirp-stone-900">
          Personal Dictionary
        </h2>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">
          Words and phrases Chirp should always spell or format a specific way.
        </p>
      </div>

      {dictionary.length > 0 ? (
        <div className="overflow-hidden rounded-xl border border-chirp-stone-200">
          {/* Header */}
          <div className="flex bg-chirp-stone-100 px-4 py-2.5">
            <span className="flex-1 font-body text-xs font-semibold uppercase tracking-[0.5px] text-chirp-stone-500">
              Heard
            </span>
            <span className="flex-1 font-body text-xs font-semibold uppercase tracking-[0.5px] text-chirp-stone-500">
              Replace with
            </span>
            <span className="w-10" />
          </div>

          {/* Rows */}
          {dictionary.map((entry, i) => (
            <div
              key={i}
              className={`flex items-center px-4 h-11 ${
                i % 2 === 0 ? 'bg-white' : 'bg-chirp-stone-100/50'
              }`}
            >
              <span className="flex-1 font-body text-sm text-chirp-stone-700">
                {entry.from}
              </span>
              <span className="flex-1 font-body text-sm text-chirp-stone-700">
                {entry.to}
              </span>
              <button
                onClick={() => removeEntry(i)}
                className="flex h-8 w-10 items-center justify-center text-chirp-stone-400 hover:text-chirp-error transition-colors duration-150"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="flex items-center justify-center rounded-xl border border-chirp-stone-200 bg-white px-6 py-10">
          <p className="font-body text-sm text-chirp-stone-500 text-center">
            No entries yet. Add words and phrases Chirp should always format
            correctly.
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
          className="flex-1 h-10 rounded-lg border border-chirp-stone-200 bg-white px-3 font-body text-sm text-chirp-stone-700 placeholder:text-chirp-stone-500 placeholder:italic focus:border-2 focus:border-chirp-amber-400 focus:outline-none transition-colors duration-150"
        />
        <input
          type="text"
          value={newTo}
          onChange={(e) => setNewTo(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Replaced by..."
          className="flex-1 h-10 rounded-lg border border-chirp-stone-200 bg-white px-3 font-body text-sm text-chirp-stone-700 placeholder:text-chirp-stone-500 placeholder:italic focus:border-2 focus:border-chirp-amber-400 focus:outline-none transition-colors duration-150"
        />
        <Button onClick={handleAdd} disabled={!newFrom.trim() || !newTo.trim()}>
          Add
        </Button>
      </div>

      {dictionary.length >= 450 && (
        <p className="font-body text-xs text-chirp-error">
          You're approaching the maximum of 500 entries ({dictionary.length}/500).
        </p>
      )}
    </div>
  )
}
