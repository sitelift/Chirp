import { Check } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'

export function Done() {
  const wordCount = useAppStore((s) => s.wordCount)

  return (
    <div className="flex items-center gap-2">
      <Check size={20} className="text-chirp-success" />
      <span className="font-body font-medium text-sm text-chirp-stone-700">
        {wordCount === 0
          ? 'No speech detected'
          : `Inserted ${wordCount} word${wordCount !== 1 ? 's' : ''}`}
      </span>
    </div>
  )
}
