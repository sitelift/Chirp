import { useEffect } from 'react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { LevelBars } from '../parts'
import { saveSettings } from '../actions'
import type { AudioDevice } from '../types'

function DeviceItem({ id, name, selected, missing, level }: { id: string; name: string; selected: boolean; missing: boolean; level: number }) {
  const ctx = useSurfaceContext()
  const forced = useForced(`mic-${id}`)
  return (
    <button
      type="button"
      data-nav=""
      data-pick=""
      role="radio"
      aria-checked={selected}
      className={selected ? 'nav item sel' : 'nav item'}
      onClick={() => !selected && saveSettings(ctx, { inputDevice: id })}
      {...forced}
    >
      <span className="radio" />
      <div className="txt">
        <b className="clamp">{name}</b>
      </div>
      {missing ? <span className="text-meta text-t4">Unplugged</span> : <LevelBars level={level} />}
    </button>
  )
}

const deviceName = (d: AudioDevice) => (d.id === 'default' ? 'Default' : d.name || d.id)

/** Every input shows its live level: the one that moves is the one that hears you. */
export function Microphone() {
  const { api, store, still } = useSurfaceContext()
  const devices = useSurface((s) => s.devices)
  const selected = useSurface((s) => s.settings.inputDevice)
  const levels = useSurface((s) => s.levels)

  useEffect(() => {
    if (still) return
    api
      .call<AudioDevice[]>('get_audio_devices')
      .then((d) => d && store.setState({ devices: d }))
      .catch(() => {})
    api.call('start_level_monitor').catch(() => {})
    const off = api.on<{ id: string; level: number }[]>('device-levels', (list) => {
      store.setState({ levels: Object.fromEntries(list.map((d) => [d.id, d.level])) })
    })
    return () => {
      off()
      api.call('stop_level_monitor').catch(() => {})
    }
  }, [api, store, still])

  const missing = selected !== 'default' && !devices.some((d) => d.id === selected)
  return (
    <>
      <h2>Microphone</h2>
      <p className="lede">Say something. The one that moves is the one that hears you.</p>
      <div role="radiogroup" aria-label="Microphone">
        {missing && <DeviceItem id={selected} name={selected} selected missing level={0} />}
        {devices.map((d) => (
          <DeviceItem key={d.id} id={d.id} name={deviceName(d)} selected={d.id === selected} missing={false} level={levels[d.id] ?? 0} />
        ))}
        {devices.length === 0 && <div className="note">No microphones found.</div>}
      </div>
    </>
  )
}
