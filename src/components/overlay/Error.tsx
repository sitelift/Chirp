import { useAppStore } from '../../stores/appStore'
import { ERROR_MESSAGES } from '../../lib/constants'

export function Error() {
  const errorType = useAppStore((s) => s.errorType)
  const msg = errorType ? ERROR_MESSAGES[errorType] : ERROR_MESSAGES.unknown

  return (
    <span className="font-body text-[11px] font-medium text-chirp-error">{msg.title}</span>
  )
}
