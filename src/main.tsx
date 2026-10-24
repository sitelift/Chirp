import { StrictMode, lazy, Suspense } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './styles/globals.css'

// No context menu in production builds.
if (import.meta.env.PROD) document.addEventListener('contextmenu', (e) => e.preventDefault())

// Dev-only state gallery for visual review: http://localhost:5173/?preview=1
const preview = import.meta.env.DEV && new URLSearchParams(location.search).has('preview')
const Preview = import.meta.env.DEV ? lazy(() => import('./preview/Preview')) : null

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {preview && Preview ? (
      <Suspense fallback={null}>
        <Preview />
      </Suspense>
    ) : (
      <App />
    )}
  </StrictMode>,
)
