import type { SnippetEntry, VocabEntry } from './types'

/**
 * Words is one list ("you say X, Chirp types Y") over two stores:
 * - vocabulary: a canonical `term` plus mishearings in `replaces` (drives ASR
 *   hotwords, the find/replace pass and the LLM dictionary);
 * - snippets: trigger -> expansion, expanded after cleanup.
 * Short single terms are vocabulary; anything with punctuation or more than a
 * few words is a snippet.
 */
export type WordItem =
  | { kind: 'vocab'; say: string; get: string; replace: string | null }
  | { kind: 'snippet'; say: string; get: string; index: number }

export function wordItems(vocabulary: VocabEntry[], snippets: SnippetEntry[]): WordItem[] {
  const items: WordItem[] = []
  for (const e of vocabulary) {
    if (e.replaces.length === 0) items.push({ kind: 'vocab', say: e.term.toLowerCase(), get: e.term, replace: null })
    for (const r of e.replaces) items.push({ kind: 'vocab', say: r, get: e.term, replace: r })
  }
  snippets.forEach((s, index) => items.push({ kind: 'snippet', say: s.trigger, get: s.expansion, index }))
  return items
}

export function isSnippet(get: string): boolean {
  const words = get.trim().split(/\s+/).length
  return /[,.;:!?\n@/]/.test(get) || words > 3 || get.length > 40
}

export interface WordStores {
  vocabulary: VocabEntry[]
  snippets: SnippetEntry[]
}

export function removeItem(stores: WordStores, item: WordItem): WordStores {
  if (item.kind === 'snippet') {
    return { ...stores, snippets: stores.snippets.filter((_, i) => i !== item.index) }
  }
  const vocabulary = stores.vocabulary
    .map((e) => {
      if (e.term !== item.get) return e
      if (item.replace === null) return null
      const replaces = e.replaces.filter((r) => r !== item.replace)
      return replaces.length ? { ...e, replaces } : null
    })
    .filter((e): e is VocabEntry => e !== null)
  return { ...stores, vocabulary }
}

export function addItem(stores: WordStores, sayRaw: string, getRaw: string): WordStores {
  const say = sayRaw.trim()
  const get = getRaw.trim()
  if (!say || !get) return stores
  if (isSnippet(get)) {
    const snippets = stores.snippets.filter((s) => s.trigger.toLowerCase() !== say.toLowerCase())
    return { ...stores, snippets: [...snippets, { trigger: say, expansion: get }] }
  }
  const vocabulary = [...stores.vocabulary]
  const i = vocabulary.findIndex((e) => e.term === get)
  const same = say.toLowerCase() === get.toLowerCase()
  if (i < 0) {
    vocabulary.push({ term: get, replaces: same ? [] : [say] })
  } else if (!same && !vocabulary[i].replaces.some((r) => r.toLowerCase() === say.toLowerCase())) {
    vocabulary[i] = { ...vocabulary[i], replaces: [...vocabulary[i].replaces, say] }
  }
  return { ...stores, vocabulary }
}

export function editItem(stores: WordStores, item: WordItem, say: string, get: string): WordStores {
  if (!say.trim() || !get.trim()) return stores
  return addItem(removeItem(stores, item), say, get)
}
