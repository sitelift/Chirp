import { useAppStore } from '../../stores/appStore'

export function Done() {
  const wordCount = useAppStore((s) => s.wordCount)

  return (
    <span className="font-body text-[11px] font-medium text-chirp-stone-600">
      {wordCount} {wordCount === 1 ? 'word' : 'words'}
    </span>
  )
}
