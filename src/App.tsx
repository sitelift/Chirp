import { useMemo } from 'react'
import { Stage } from './surface/Surface'
import { SurfaceContext, createSurfaceStore, initialState } from './surface/store'
import { tauriApi } from './surface/api'
import { useLiveBackend } from './surface/dictation'
import { ErrorBoundary } from './surface/ErrorBoundary'

/** The one window: pill, resting tick and every expanded surface. */
export function App() {
  const value = useMemo(() => ({ store: createSurfaceStore(initialState('light')), api: tauriApi, still: false }), [])
  useLiveBackend(value)
  return (
    <ErrorBoundary>
      <SurfaceContext.Provider value={value}>
        <Stage />
      </SurfaceContext.Provider>
    </ErrorBoundary>
  )
}
