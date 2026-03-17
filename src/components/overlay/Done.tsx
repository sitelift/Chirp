import { useAppStore } from '../../stores/appStore'

export function Done() {
  const wordCount = useAppStore((s) => s.wordCount)

  return (
    <div className="flex items-center gap-2">
      <div className="h-2 w-2 rounded-full bg-chirp-success" />
      <span className="font-body text-sm text-chirp-stone-700">
        Inserted {wordCount} {wordCount === 1 ? 'word' : 'words'}
      </span>
    </div>
  )
}
