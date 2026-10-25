import { StrictMode, lazy, Suspense } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './styles/globals.css'

// No context menu in production builds.
if (import.meta.env.PROD) document.addEventListener('contextmenu', (e) => e.preventDefault())

// Dev only: the state gallery (?preview=1) and the interactive playground (?play=1).
const params = new URLSearchParams(location.search)
const preview = import.meta.env.DEV && params.has('preview')
const play = import.meta.env.DEV && params.has('play')
const Preview = import.meta.env.DEV ? lazy(() => import('./preview/Preview')) : null
const Playground = import.meta.env.DEV ? lazy(() => import('./preview/Playground')) : null

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {preview && Preview ? (
      <Suspense fallback={null}>
        <Preview />
      </Suspense>
    ) : play && Playground ? (
      <Suspense fallback={null}>
        <Playground />
      </Suspense>
    ) : (
      <App />
    )}
  </StrictMode>,
)
