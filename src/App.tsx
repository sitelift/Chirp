import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './stores/appStore'
import { Overlay } from './components/overlay/Overlay'
import { Settings } from './components/settings/Settings'
import { Onboarding } from './components/onboarding/Onboarding'
import { useSettingsSync } from './hooks/useSettingsSync'
import { ErrorBoundary } from './components/shared/ErrorBoundary'

/**
 * Routes to the appropriate component based on window label.
 * Tries Tauri API first, then URL query param, then defaults to settings.
 */
function getWindowLabel(): string {
  // Try Tauri API (works in Tauri webview context)
  try {
    const label = getCurrentWindow().label
    if (label) return label
  } catch {
    // Not in Tauri context
  }

  // Fallback: check URL query param (set in tauri.conf.json window url)
  try {
    const url = new URL(window.location.href)
    const param = url.searchParams.get('label')
    if (param) return param
  } catch {
    // Invalid URL
  }

  return 'settings'
}

export function App() {
  const onboardingComplete = useAppStore((s) => s.onboardingComplete)
  const settingsLoaded = useAppStore((s) => s.settingsLoaded)
  const windowLabel = getWindowLabel()

  useSettingsSync()

  // Overlay window — render immediately, no need to wait for settings
  if (windowLabel === 'overlay') {
    return (
      <ErrorBoundary fallback="overlay">
        <Overlay />
      </ErrorBoundary>
    )
  }

  // Wait for settings to load before deciding onboarding vs main app
  if (!settingsLoaded) {
    return null
  }

  // Show onboarding if not complete
  if (!onboardingComplete) {
    return (
      <ErrorBoundary fallback="settings">
        <Onboarding />
      </ErrorBoundary>
    )
  }

  // Settings window (default)
  return (
    <ErrorBoundary fallback="settings">
      <Settings />
    </ErrorBoundary>
  )
}
