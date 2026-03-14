import { AlertTriangle } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { ERROR_MESSAGES } from '../../lib/constants'

export function Error() {
  const errorType = useAppStore((s) => s.errorType)
  const error = ERROR_MESSAGES[errorType ?? 'unknown']

  const handleAction = () => {
    if (!error.action) return
    // Actions will be wired up when Tauri backend is ready
    console.log('Error action:', error.action.type)
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <AlertTriangle size={20} className="text-chirp-error" />
        <span className="font-body font-medium text-sm text-chirp-stone-900">
          {error.title}
        </span>
      </div>
      <p className="font-body text-[13px] text-chirp-stone-500">{error.help}</p>
      {error.action && (
        <button
          onClick={handleAction}
          className="self-start font-body font-medium text-[13px] text-chirp-info hover:underline transition-colors duration-150"
        >
          {error.action.label}
        </button>
      )}
    </div>
  )
}
