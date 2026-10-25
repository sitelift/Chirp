import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Header, Reveal, Segmented, Switch } from '../parts'
import { drill, later, saveSettings, showSection } from '../actions'
import { anim, cancelAll, done, DUR, reducedMotion } from '../motion'
import { IconAiCleanup, IconChirp, IconMicrophone, IconShortcuts, IconSound, type IconProps } from '../icons'
import { installUpdate } from '../updates'
import type { Appearance, MediaMode, Section } from '../types'
import { Shortcuts } from './Shortcuts'
import { Microphone } from './Microphone'
import { AiCleanup } from './AiCleanup'

const SECTIONS: { id: Section; label: string; Icon: (p: IconProps) => ReactNode }[] = [
  { id: 'chirp', label: 'Chirp', Icon: IconChirp },
  { id: 'shortcuts', label: 'Shortcuts', Icon: IconShortcuts },
  { id: 'microphone', label: 'Microphone', Icon: IconMicrophone },
  { id: 'sound', label: 'Sound', Icon: IconSound },
  { id: 'ai', label: 'AI cleanup', Icon: IconAiCleanup },
]
/** First nav's top in the sidebar: 6 padding + 40 header + 8 gap; each nav is 34. */
const NAV_TOP = 54
const NAV_H = 34

const APPEARANCE_NOTE: Record<Appearance, string> = { system: 'Follows Windows', light: 'Always light', dark: 'Always dark' }

function ChirpPane() {
  const ctx = useSurfaceContext()
  const appearance = useSurface((s) => s.settings.appearance)
  const update = useSurface((s) => s.update)
  return (
    <>
      <h2>Chirp</h2>
      <p className="lede">How Chirp looks on your PC.</p>
      <div>
        <div className="item">
          <div className="txt">
            <b>Appearance</b>
            <small>{APPEARANCE_NOTE[appearance] ?? APPEARANCE_NOTE.system}</small>
          </div>
          <Segmented<Appearance>
            label="Appearance"
            width={72}
            value={appearance}
            onChange={(v) => saveSettings(ctx, { appearance: v })}
            options={[
              { value: 'system', label: 'System' },
              { value: 'light', label: 'Light' },
              { value: 'dark', label: 'Dark' },
            ]}
          />
        </div>
      </div>
      <div className="note">
        Chirp {__APP_VERSION__}
        {update && (
          <>
            {' · '}
            <button type="button" data-nav="" className="nav" onClick={() => installUpdate(ctx.api)}>
              Restart to update to {update}
            </button>
          </>
        )}
      </div>
    </>
  )
}

/** How much quieter. The store follows the drag; the save waits for it to settle. */
function LowerBy() {
  const ctx = useSurfaceContext()
  const value = useSurface((s) => s.settings.mediaLowerBy)
  const set = (v: number) => {
    ctx.store.setState((s) => ({ settings: { ...s.settings, mediaLowerBy: v } }))
    later('media-lower-by', 300, () => saveSettings(ctx, { mediaLowerBy: v }))
  }
  return (
    <div className="item">
      <div className="txt">
        <b>Quieter by</b>
      </div>
      <input
        type="range"
        data-nav=""
        min={10}
        max={100}
        step={5}
        value={value}
        aria-label="Quieter by"
        aria-valuetext={`${value} percent`}
        onChange={(e) => set(Number(e.target.value))}
        className="nav slider"
        style={{ '--pct': `${((value - 10) / 90) * 100}%` } as CSSProperties}
      />
      <span className="sval">{value}%</span>
    </div>
  )
}

function SoundPane() {
  const ctx = useSurfaceContext()
  const sounds = useSurface((s) => s.settings.playSoundOnComplete)
  const media = useSurface((s) => s.settings.mediaWhileDictating)
  const forced = useForced('sounds')
  return (
    <>
      <h2>Sound</h2>
      <p className="lede">What you hear while you dictate.</p>
      <div>
        <button
          type="button"
          data-nav=""
          role="switch"
          aria-checked={sounds}
          className={sounds ? 'nav item on-t' : 'nav item'}
          onClick={() => saveSettings(ctx, { playSoundOnComplete: !sounds })}
          {...forced}
        >
          <div className="txt">
            <b>Sounds</b>
            <small>A soft tone when recording starts and stops.</small>
          </div>
          <Switch />
        </button>
        <div className="item">
          <div className="txt">
            <b>Music while dictating</b>
            <small>Turns other apps down.</small>
          </div>
          <Segmented<MediaMode>
            label="Music while dictating"
            width={64}
            value={media}
            onChange={(v) => saveSettings(ctx, { mediaWhileDictating: v })}
            options={[
              { value: 'off', label: 'Off' },
              { value: 'lower', label: 'Lower' },
              { value: 'pause', label: 'Pause' },
            ]}
          />
        </div>
        <Reveal open={media === 'lower'}>
          <LowerBy />
        </Reveal>
      </div>
    </>
  )
}

const PANES: Record<Section, () => ReactNode> = {
  chirp: () => <ChirpPane />,
  shortcuts: () => <Shortcuts />,
  microphone: () => <Microphone />,
  sound: () => <SoundPane />,
  ai: () => <AiCleanup />,
}

/** Settings: a sidebar with a sliding marker; the panes crossfade with a small lift. */
export function Settings() {
  const ctx = useSurfaceContext()
  const section = useSurface((s) => s.section)
  const [panes, setPanes] = useState<{ from: Section | null; to: Section }>({ from: null, to: section })
  if (section !== panes.to) setPanes({ from: ctx.still ? null : panes.to, to: section })
  const refs = useRef(new Map<Section, HTMLElement>())

  useLayoutEffect(() => {
    if (!panes.from) return
    const old = refs.current.get(panes.from)
    const pane = refs.current.get(panes.to)
    if (!old || !pane) return
    cancelAll(old, pane)
    const reduced = reducedMotion()
    const fade = anim(old, [{ opacity: 1 }, { opacity: 0 }], { duration: reduced ? DUR.reduced : DUR.paneOut, easing: 'linear' })
    const rise = reduced
      ? anim(pane, [{ opacity: 0 }, { opacity: 1 }], { duration: DUR.reduced, easing: 'linear' })
      : anim(pane, [{ opacity: 0, transform: 'translateY(6px)' }, { opacity: 1, transform: 'none' }], { duration: DUR.paneIn })
    done(fade).then(() => setPanes((p) => (p.from === panes.from && p.to === panes.to ? { from: null, to: p.to } : p)))
    done(rise).then(() => rise.playState === 'finished' && rise.cancel())
  }, [panes])

  const i = SECTIONS.findIndex((s) => s.id === section)
  return (
    <>
      <div className="side">
        <div className="marker" style={{ transform: `translateY(${NAV_TOP + i * NAV_H}px)` }} />
        <Header title="Settings" onBack={() => drill(ctx, 'home')} />
        {SECTIONS.map(({ id, label, Icon }) => (
          <button key={id} type="button" data-nav="" className="nav side-nav" aria-current={id === section ? 'true' : undefined} onClick={() => showSection(ctx, id)}>
            <Icon />
            {label}
          </button>
        ))}
      </div>
      <div className="content">
        {SECTIONS.filter((s) => s.id === panes.to || s.id === panes.from).map(({ id }) => (
          <div
            key={id}
            className="pane"
            inert={id !== panes.to}
            ref={(el) => {
              if (el) refs.current.set(id, el)
              else refs.current.delete(id)
            }}
          >
            {PANES[id]()}
          </div>
        ))}
      </div>
    </>
  )
}
