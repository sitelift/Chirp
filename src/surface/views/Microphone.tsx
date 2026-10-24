import { useEffect } from 'react'
import { Check } from 'lucide-react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext } from '../store'
import { Divider, Header, LevelBars, Row, SwitchTrack } from '../parts'
import { deviceLabel } from '../format'
import { drill, saveSettings } from '../actions'

function DeviceRow({ id, label, selected, missing, level }: { id: string; label: string; selected: boolean; missing: boolean; level: number }) {
  const ctx = useSurfaceContext()
  const forced = useForced(`mic-${id}`)
  return (
    <button
      type="button"
      data-nav=""
      disabled={missing}
      onClick={() => saveSettings(ctx, { inputDevice: id })}
      className="nav flex h-[40px] w-full shrink-0 items-center gap-[8px] rounded-row pl-[8px] pr-[14px] text-left enabled:hover:bg-hover enabled:active:bg-press data-[force-hover]:bg-hover"
      {...forced}
    >
      <span className="flex w-[20px] shrink-0 justify-center text-t1">{selected && !missing && <Check size={14} strokeWidth={2.5} />}</span>
      <span className={`min-w-0 flex-1 truncate font-display text-row ${selected ? 'font-semibold' : ''} ${missing ? 'text-t4' : 'text-t1'}`}>{label}</span>
      {missing ? <span className="text-meta text-t4">Unplugged</span> : <LevelBars level={level} active={selected} />}
    </button>
  )
}

/** Start, done and pasted-without-AI earcons. The row is the switch. */
function SoundsRow() {
  const ctx = useSurfaceContext()
  const on = useSurface((s) => s.settings.playSoundOnComplete)
  return (
    <Row id="sounds" tight={false} role="switch" aria-checked={on} onClick={() => saveSettings(ctx, { playSoundOnComplete: !on })}>
      <span>Sounds</span>
      <SwitchTrack state={on ? 'on' : 'off'} />
    </Row>
  )
}

export function Microphone() {
  const ctx = useSurfaceContext()
  const devices = useSurface((s) => s.devices)
  const selected = useSurface((s) => s.settings.inputDevice)
  const levels = useSurface((s) => s.levels)
  const { api, store, still } = ctx

  useEffect(() => {
    if (still) return
    api.call<typeof devices>('get_audio_devices').then((d) => d && store.setState({ devices: d })).catch(() => {})
    api.call('start_level_monitor').catch(() => {})
    const off = api.on<{ id: string; level: number }[]>('device-levels', (list) => {
      store.setState({ levels: Object.fromEntries(list.map((d) => [d.id, d.level])) })
    })
    return () => {
      off()
      api.call('stop_level_monitor').catch(() => {})
    }
  }, [api, store, still])

  const ids = devices.map((d) => d.id)
  const missing = selected !== 'default' && !ids.includes(selected)
  return (
    <div className="flex flex-col">
      <Header title="Microphone" onBack={() => drill(ctx, 'home')} />
      <div className="-m-[4px] flex max-h-[288px] flex-col overflow-y-auto p-[4px]">
        {missing && <DeviceRow id={selected} label={deviceLabel(selected)} selected missing level={0} />}
        {devices.map((d) => (
          <DeviceRow key={d.id} id={d.id} label={deviceLabel(d.id, ids)} selected={d.id === selected} missing={false} level={levels[d.id] ?? 0} />
        ))}
        {devices.length === 0 && <div className="flex h-[40px] items-center pl-[36px] text-row text-t4">No microphones found</div>}
      </div>
      <Divider />
      <SoundsRow />
    </div>
  )
}
