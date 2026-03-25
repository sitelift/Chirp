import { useAppStore } from '../../stores/appStore'
import { ERROR_MESSAGES } from '../../lib/constants'
import { open } from '@tauri-apps/plugin-shell'

function openOsSettings() {
  const isMac = navigator.platform.includes('Mac')
  if (isMac) {
    open('x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone')
  } else {
    open('ms-settings:privacy-microphone')
  }
}

export function Error() {
  const errorType = useAppStore((s) => s.errorType)
  const msg = errorType ? ERROR_MESSAGES[errorType] : ERROR_MESSAGES.unknown

  const handleAction = () => {
    if (!msg.action) return
    if (msg.action.type === 'os_settings') {
      openOsSettings()
    }
  }

  const isClickable = msg.action?.type === 'os_settings'

  return (
    <span
      className={`font-body text-[11px] font-medium text-chirp-error ${isClickable ? 'pointer-events-auto cursor-pointer underline' : ''}`}
      onClick={isClickable ? handleAction : undefined}
    >
      {msg.title}
    </span>
  )
}
