import { useAppStore } from '../../stores/appStore'

export function Done() {
  const wordCount = useAppStore((s) => s.wordCount)

  return (
    <span className="font-mono text-[12px] font-medium text-chirp-success flex items-center gap-2">
      ✓ Inserted {wordCount} {wordCount === 1 ? 'word' : 'words'}
    </span>
  )
}
