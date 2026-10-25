import { Check, Copy, Pause, Play, X } from 'lucide-react'
import { useForced } from '../forced'
import { useSurface, useSurfaceContext, setUi } from '../store'
import { Caps, Divider, IconButton, NavRow, Row } from '../parts'
import { bindingCaps, deviceLabel, relativeLong } from '../format'
import { closeSurface, copyText, drill, pauseHotkeys, startRebind } from '../actions'
import { wordItems } from '../words'
import type { BindingTarget } from '../types'
import { installUpdate } from '../updates'

function LastDictation() {
  const ctx = useSurfaceContext()
  const last = useSurface((s) => s.history[s.history.length - 1])
  const copied = useSurface((s) => s.ui.copied === 'last')
  const forced = useForced('last')
  if (!last) return null
  return (
    <>
      <button
        type="button"
        data-nav=""
        onClick={() => copyText(ctx, last.text, 'last')}
        className="nav group flex w-full items-start gap-[8px] rounded-row py-[10px] pl-[12px] pr-[8px] text-left hover:bg-hover active:bg-press data-[force-hover]:bg-hover"
        style={copied ? { background: 'var(--hover)' } : undefined}
        title="Copy"
        {...forced}
      >
        <span className="flex min-w-0 flex-1 flex-col gap-[2px]">
          <span className="line-clamp-2 font-display text-body text-t1">{last.text}</span>
          <span className={`text-meta ${copied ? 'text-t1' : 'text-t3'}`}>{copied ? 'Copied' : relativeLong(last.timestamp)}</span>
        </span>
        <span className="flex h-[28px] w-[28px] shrink-0 items-center justify-center rounded-btn text-t1">
          {copied ? (
            <Check size={16} strokeWidth={2} />
          ) : (
            <Copy size={16} strokeWidth={1.8} className="opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100 group-data-[force-hover]:opacity-100" />
          )}
        </span>
      </button>
      <Divider />
    </>
  )
}

const LABEL: Record<BindingTarget, string> = { hold: 'Hold to talk', toggle: 'Hands-free', cancel: 'Cancel' }

function HotkeyRow({ target }: { target: BindingTarget }) {
  const ctx = useSurfaceContext()
  const binding = useSurface((s) => (target === 'hold' ? s.settings.holdBinding : s.settings.toggleBinding))
  const rebind = useSurface((s) => (s.ui.rebind?.target === target ? s.ui.rebind : null))
  const paused = useSurface((s) => s.pause.paused)
  const id = `key-${target}`
  const caps = bindingCaps(binding)

  if (rebind) {
    const tone = rebind.phase === 'conflict' ? 'danger' : rebind.phase === 'listening' ? 'accent' : 'normal'
    const bg = rebind.phase === 'conflict' ? 'var(--danger-soft)' : rebind.phase === 'listening' ? 'var(--hover)' : undefined
    return (
      <div className="flex h-[56px] shrink-0 items-center justify-between rounded-row pl-[12px] pr-[14px]" style={{ background: bg }} aria-live="polite">
        <div className="flex flex-col gap-[2px]">
          <span className="text-row text-t1">{LABEL[target]}</span>
          {rebind.phase === 'listening' && <span className="text-meta text-t3">Esc to cancel</span>}
          {rebind.phase === 'saved' && (
            <span className="flex items-center gap-[4px] text-meta text-accent-text">
              <Check size={12} strokeWidth={2.5} />
              Saved
            </span>
          )}
          {rebind.phase === 'conflict' && <span className="text-meta text-danger">{rebind.message}</span>}
        </div>
        <span className="flex items-center gap-[4px]">
          {rebind.keys.length > 0 && <Caps keys={rebind.keys} tone={tone} />}
          {rebind.phase === 'listening' &&
            (rebind.keys.length === 0 ? (
              <span className="flex h-[24px] items-center rounded-key border border-dashed border-accent bg-field px-[10px] text-meta font-semibold text-accent-text">Press keys</span>
            ) : (
              <span className="flex h-[24px] items-center rounded-key border border-dashed border-accent px-[8px] text-meta font-medium text-accent-text">…</span>
            ))}
        </span>
      </div>
    )
  }

  return (
    <Row id={id} tight={false} disabled={paused} onClick={() => startRebind(ctx, target)} className={paused ? 'opacity-40' : ''} aria-label={`${LABEL[target]}: ${caps.join(' + ') || 'not set'}. Change`}>
      <span>{LABEL[target]}</span>
      <span className="flex items-center gap-[8px]">
        {!paused && <span className="hidden text-meta text-t2 group-hover:inline group-focus-visible:inline group-data-[force-hover]:inline">Change</span>}
        {caps.length ? <Caps keys={caps} hoverable={!paused} /> : <span className="text-row text-t2">Not set</span>}
      </span>
    </Row>
  )
}

function aiValue(state: string, enabled: boolean) {
  if (state === 'invalid')
    return (
      <span className="flex items-center gap-[6px] text-danger">
        <span className="h-[6px] w-[6px] rounded-full bg-danger" />
        Key invalid
      </span>
    )
  if (state === 'offline') return <span className="text-t3">Offline</span>
  if (state === 'none' || !enabled) return 'Off'
  return 'On'
}

function BottomRow() {
  const ctx = useSurfaceContext()
  const paused = useSurface((s) => s.pause.paused)
  const menu = useSurface((s) => s.ui.pauseMenu)
  const update = useSurface((s) => s.update)
  const forcedPause = useForced('pause-hour')

  if (paused) {
    return (
      <div className="flex h-[40px] shrink-0 items-center rounded-row bg-accent-soft pl-[12px] pr-[6px]">
        <button type="button" data-nav="" onClick={() => pauseHotkeys(ctx, false)} className="nav flex h-full flex-1 items-center gap-[10px] rounded-row text-row font-semibold text-accent-text">
          <Play size={16} fill="currentColor" strokeWidth={0} />
          Resume
        </button>
        <IconButton label="Close" size={28} onClick={() => closeSurface(ctx)} className="text-accent-text hover:bg-transparent">
          <X size={14} strokeWidth={2} />
        </IconButton>
      </div>
    )
  }

  if (menu) {
    return (
      <div className="flex flex-col">
        <button type="button" data-nav="" onClick={() => pauseHotkeys(ctx, true, 60)} className="nav flex h-[40px] shrink-0 items-center gap-[10px] rounded-row px-[12px] text-row text-t1 hover:bg-hover active:bg-press data-[force-hover]:bg-hover" {...forcedPause}>
          <Pause size={16} strokeWidth={2} className="text-t2" />
          Pause for 1 hour
        </button>
        <button type="button" data-nav="" onClick={() => pauseHotkeys(ctx, true, null)} className="nav flex h-[40px] shrink-0 items-center gap-[10px] rounded-row px-[12px] text-row text-t1 hover:bg-hover active:bg-press">
          <span className="w-[16px] shrink-0" />
          Pause until I resume
        </button>
      </div>
    )
  }

  return (
    <div className="flex h-[40px] shrink-0 items-center gap-[10px] rounded-row pr-[6px]">
      <button type="button" data-nav="" onClick={() => setUi(ctx.store, { pauseMenu: true })} className="nav flex h-full items-center gap-[10px] rounded-row pl-[12px] pr-[8px] text-row text-t1">
        <Pause size={16} strokeWidth={2} className="text-t2" />
        Pause
      </button>
      <div className="flex flex-1 items-center justify-end gap-[6px]">
        {update && (
          <button type="button" data-nav="" onClick={() => installUpdate(ctx.api)} className="nav flex h-[28px] items-center gap-[6px] rounded-btn bg-accent-soft px-[10px] text-meta font-semibold text-accent-text">
            <span className="h-[6px] w-[6px] rounded-full bg-accent" />
            Restart to update
          </button>
        )}
        <IconButton label="Close" size={28} onClick={() => closeSurface(ctx)} className="text-t2">
          <X size={14} strokeWidth={2} />
        </IconButton>
      </div>
    </div>
  )
}

export function Home() {
  const ctx = useSurfaceContext()
  const device = useSurface((s) => s.settings.inputDevice)
  const devices = useSurface((s) => s.devices)
  const count = useSurface((s) => wordItems(s.vocabulary, s.snippets).length)
  const ai = useSurface((s) => s.ai.state)
  const aiEnabled = useSurface((s) => s.settings.aiCleanup)
  return (
    <div className="flex flex-col">
      <LastDictation />
      <HotkeyRow target="hold" />
      <HotkeyRow target="toggle" />
      <NavRow id="microphone" label="Microphone" value={deviceLabel(device, devices.map((d) => d.id))} onClick={() => drill(ctx, 'microphone')} />
      <Divider />
      <NavRow id="words" label="Words" value={count} onClick={() => drill(ctx, 'words')} />
      <NavRow id="ai" label="AI cleanup" value={aiValue(ai, aiEnabled)} onClick={() => drill(ctx, 'ai')} />
      <NavRow id="recent" label="Recent" onClick={() => drill(ctx, 'recent')} />
      <NavRow id="settings" label="Settings" onClick={() => drill(ctx, 'settings')} />
      <Divider />
      <BottomRow />
    </div>
  )
}
