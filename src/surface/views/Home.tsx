import type { ReactNode } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Divider } from '../parts'
import { relativeLong } from '../format'
import { copyText, drill, pauseHotkeys } from '../actions'
import { IconCopy, IconNext, IconPause, IconRecent, IconSettings, IconSnippets, IconWords } from '../icons'
import type { View } from '../types'

/** The last dictation; click copies it. */
function LastDictation() {
  const ctx = useSurfaceContext()
  const last = useSurface((s) => s.history[s.history.length - 1])
  const copied = useSurface((s) => s.ui.copied === 'last')
  const forced = useForced('last')
  if (!last) return null
  return (
    <button type="button" data-nav="" className="nav last" title="Copy" onClick={() => copyText(ctx, last.text, 'last')} {...forced}>
      <b>{last.text}</b>
      <span>
        <span className={copied ? 'text-t1' : undefined}>{copied ? 'Copied' : relativeLong(last.timestamp)}</span>
        <IconCopy size={14} />
      </span>
    </button>
  )
}

function NavRow({ to, icon, label, value }: { to: View; icon: ReactNode; label: string; value?: number }) {
  const ctx = useSurfaceContext()
  const forced = useForced(to)
  return (
    <button type="button" data-nav="" className="nav row" onClick={() => drill(ctx, to)} {...forced}>
      <span className="lead">
        {icon}
        {label}
      </span>
      <span className="val">
        {value !== undefined && <span>{value}</span>}
        <IconNext className="chev" />
      </span>
    </button>
  )
}

function Bar() {
  const ctx = useSurfaceContext()
  const paused = useSurface((s) => s.pause.paused)
  const forcedPause = useForced('pause')
  const forcedSettings = useForced('settings')
  return (
    <div className="bar">
      <button type="button" data-nav="" className="nav btn" data-on={paused ? '' : undefined} onClick={() => pauseHotkeys(ctx, !paused)} {...forcedPause}>
        <IconPause />
        {paused ? 'Resume' : 'Pause'}
      </button>
      <button type="button" data-nav="" className="nav btn" onClick={() => drill(ctx, 'settings')} {...forcedSettings}>
        <IconSettings />
        Settings
      </button>
    </div>
  )
}

export function Home() {
  const words = useSurface((s) => s.vocabulary.length)
  const snippets = useSurface((s) => s.snippets.length)
  return (
    <>
      <LastDictation />
      <NavRow to="recent" icon={<IconRecent />} label="Recent" />
      <NavRow to="words" icon={<IconWords />} label="Words" value={words} />
      <NavRow to="snippets" icon={<IconSnippets />} label="Snippets" value={snippets} />
      <Divider />
      <Bar />
    </>
  )
}
