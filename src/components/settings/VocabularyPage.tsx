import { useEffect, useState } from 'react'
import { trackEvent } from '@aptabase/tauri'
import { useAppStore, type VocabEntry } from '../../stores/appStore'
import { Button } from '../shared/Button'

/**
 * Parse a comma-separated input string into a list of trimmed, non-empty
 * replacement terms.
 */
function parseReplaces(input: string): string[] {
  return input
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
}

/**
 * Format a list of replacement terms back into a comma-separated string for
 * the input field. Round-trips with parseReplaces (modulo whitespace).
 */
function formatReplaces(replaces: string[]): string {
  return replaces.join(', ')
}

interface VocabRowProps {
  index: number
  entry: VocabEntry
  onUpdate: (index: number, entry: VocabEntry) => void
  onRemove: (index: number) => void
}

/**
 * One row in the vocabulary table. Owns its own raw-string local state for
 * the replaces input so a partially-typed comma doesn't get parsed away
 * mid-keystroke. Commits to the parent store on blur.
 *
 * The term column is a plain controlled string with no parse/format
 * roundtrip, so it can sync to the parent on every keystroke without losing
 * characters.
 */
function VocabRow({ index, entry, onUpdate, onRemove }: VocabRowProps) {
  // Local raw-string state for the replaces input. Initialized from the
  // entry's parsed list, updated freely on each keystroke (so the user can
  // type commas, spaces, partial words), committed to the parent store on
  // blur via parseReplaces.
  const [rawReplaces, setRawReplaces] = useState(() => formatReplaces(entry.replaces))

  // If the entry's replaces list changes externally (cross-window sync,
  // bulk edits, etc.) AND the user is not actively editing this field,
  // re-derive the raw string. We detect "not editing" by comparing the
  // current local string's parsed form to the new external list — if they
  // already match semantically, no resync is needed; if they differ, the
  // external change wins. This avoids stomping on the user's in-progress
  // typing during a settings-changed event.
  useEffect(() => {
    const currentParsed = parseReplaces(rawReplaces)
    const externalParsed = entry.replaces
    const sameLength = currentParsed.length === externalParsed.length
    const sameContent = sameLength && currentParsed.every((v, i) => v === externalParsed[i])
    if (!sameContent) {
      setRawReplaces(formatReplaces(externalParsed))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentionally only react to external entry changes
  }, [entry.replaces])

  const handleTermChange = (term: string) => {
    onUpdate(index, { ...entry, term })
  }

  const handleReplacesBlur = () => {
    const parsed = parseReplaces(rawReplaces)
    // Only commit if the parsed list actually differs from what's already in
    // the store, to avoid spurious update_vocabulary calls (which would mark
    // the recognizer dirty and trigger a needless rebuild on next dictation).
    const same =
      parsed.length === entry.replaces.length &&
      parsed.every((v, i) => v === entry.replaces[i])
    if (!same) {
      onUpdate(index, { ...entry, replaces: parsed })
    }
    // Normalize the displayed string after blur (collapses extra commas/whitespace)
    setRawReplaces(formatReplaces(parsed))
  }

  return (
    <div
      className={`flex items-center gap-3 px-[18px] py-2 border-b border-dm-row-sep last:border-b-0 transition-colors hover:bg-card-hover group animate-slide-up ${
        index % 2 === 0 ? 'bg-card' : 'bg-card-hover/50'
      }`}
      style={{ animationDelay: `${index * 30}ms` }}
    >
      <input
        type="text"
        value={entry.term}
        onChange={(e) => handleTermChange(e.target.value)}
        className="w-[36%] h-8 rounded-md border border-transparent bg-transparent px-2 text-[13px] text-dm-primary hover:border-card-border focus:border-chirp-yellow focus:bg-card focus:outline-none transition-colors"
      />
      <input
        type="text"
        value={rawReplaces}
        onChange={(e) => setRawReplaces(e.target.value)}
        onBlur={handleReplacesBlur}
        placeholder="e.g. Peter, peter"
        className="flex-1 h-8 rounded-md border border-transparent bg-transparent px-2 text-[13px] text-dm-primary placeholder:text-dm-tertiary hover:border-card-border focus:border-chirp-yellow focus:bg-card focus:outline-none transition-colors"
      />
      <button
        onClick={() => onRemove(index)}
        className="flex h-8 w-10 items-center justify-center text-dm-tertiary hover:text-chirp-error transition-colors duration-150 opacity-0 group-hover:opacity-100"
        aria-label="Remove entry"
      >
        ✕
      </button>
    </div>
  )
}

export function VocabularyPage() {
  const vocabulary = useAppStore((s) => s.vocabulary)
  const addEntry = useAppStore((s) => s.addVocabularyEntry)
  const updateEntry = useAppStore((s) => s.updateVocabularyEntry)
  const removeEntry = useAppStore((s) => s.removeVocabularyEntry)

  const [newTerm, setNewTerm] = useState('')

  const handleAdd = () => {
    const term = newTerm.trim()
    if (!term) return
    addEntry(term)
    setNewTerm('')
    try {
      trackEvent('feature_used', { feature: 'vocabulary_add' })
    } catch {
      // Aptabase may not be ready on first paint — silently ignore
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleAdd()
  }

  return (
    <div className="flex flex-col gap-5 animate-slide-up">
      <div className="mb-1">
        <h1 className="font-display font-extrabold text-2xl text-dm-primary tracking-[-0.5px]">
          Vocabulary
        </h1>
        <p className="text-[13px] text-dm-secondary mt-1">
          Names and terms Chirp should recognize. The first column is the canonical spelling
          (used to bias the speech model). The second column is a comma-separated list of common
          mishearings to auto-correct toward the canonical spelling.
        </p>
      </div>

      {vocabulary.length > 0 ? (
        <div className="overflow-hidden rounded-card border border-card-border">
          {/* Header */}
          <div className="flex items-center bg-card-hover px-[18px] py-2.5">
            <span className="w-[36%] text-[11px] font-semibold uppercase tracking-[0.5px] text-dm-secondary">
              Word / Phrase
            </span>
            <span className="flex-1 text-[11px] font-semibold uppercase tracking-[0.5px] text-dm-secondary">
              Also correct from <span className="font-normal normal-case text-dm-tertiary">(comma-separated)</span>
            </span>
            <span className="w-10" />
          </div>

          {/* Rows */}
          {vocabulary.map((entry, i) => (
            <VocabRow
              key={i}
              index={i}
              entry={entry}
              onUpdate={updateEntry}
              onRemove={removeEntry}
            />
          ))}
        </div>
      ) : (
        <div className="flex items-center justify-center rounded-card border border-dashed border-card-border bg-card-hover px-6 py-10">
          <p className="text-[13px] text-dm-secondary text-center">
            No vocabulary yet. Add names, jargon, or technical terms that Chirp should recognize.
          </p>
        </div>
      )}

      {/* Add row */}
      <div className="flex items-center gap-3">
        <input
          type="text"
          value={newTerm}
          onChange={(e) => setNewTerm(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Add a word or phrase..."
          className="flex-1 h-10 rounded-lg border border-card-border bg-card px-3 text-[13px] text-dm-primary placeholder:text-dm-tertiary focus:border-chirp-yellow focus:shadow-[0_0_0_3px_rgba(240,183,35,0.1)] focus:outline-none transition-all duration-150"
        />
        <Button onClick={handleAdd} disabled={!newTerm.trim() || vocabulary.length >= 500}>
          Add
        </Button>
      </div>

      {vocabulary.length >= 450 && (
        <p className="text-xs text-chirp-error">
          You're approaching the maximum of 500 entries ({vocabulary.length}/500).
        </p>
      )}
    </div>
  )
}
