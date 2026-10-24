import { useEffect, useRef, type ReactNode } from 'react'
import { ArrowUpRight, Check } from 'lucide-react'
import { open } from '@tauri-apps/plugin-shell'
import { setUi, useSurface, useSurfaceContext } from '../store'
import { Header, Spinner, Toggle, type ToggleState } from '../parts'
import { drill, removeApiKey, saveSettings, setApiKey } from '../actions'

const KEYS_URL = 'https://console.anthropic.com/settings/keys'

function LinkButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button type="button" data-nav="" onClick={onClick} className="nav flex items-center gap-[2px] rounded-[4px] text-meta text-t1 hover:underline">
      {children}
    </button>
  )
}

function GetKey() {
  const { api } = useSurfaceContext()
  return (
    <LinkButton onClick={() => api.live && open(KEYS_URL)}>
      Get a key
      <ArrowUpRight size={12} strokeWidth={2} />
    </LinkButton>
  )
}

/** Paste or type a key; it is validated, then stored in Windows Credential Manager. */
function KeyInput({ invalidHint }: { invalidHint?: string }) {
  const ctx = useSurfaceContext()
  const draft = useSurface((s) => s.ui.aiDraft)
  const ref = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!ctx.still && !invalidHint) ref.current?.focus()
  }, [ctx.still, invalidHint])
  return (
    <input
      ref={ref}
      type="password"
      value={draft}
      autoComplete="off"
      spellCheck={false}
      aria-label="Anthropic API key"
      placeholder={invalidHint ?? 'Paste your Anthropic API key'}
      onChange={(e) => setUi(ctx.store, { aiDraft: e.target.value })}
      onPaste={(e) => {
        const text = e.clipboardData.getData('text')
        if (text.trim()) {
          e.preventDefault()
          setApiKey(ctx, text)
        }
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') setApiKey(ctx, draft)
      }}
      className={`h-[34px] w-full rounded-field border-b-2 bg-field px-[10px] text-row text-t1 outline-none ${
        invalidHint ? 'border-danger tracking-[0.02em] placeholder:text-t1' : 'border-transparent placeholder:text-t4 focus:border-accent'
      }`}
      style={{ boxShadow: '0 0 0 1px var(--field-ring)' }}
    />
  )
}

function KeyField({ hint, trailing, readOnly }: { hint: string; trailing?: ReactNode; readOnly?: boolean }) {
  return (
    <div
      className="flex h-[34px] shrink-0 items-center gap-[8px] rounded-field px-[10px]"
      style={readOnly ? { background: 'var(--field-ro)', boxShadow: '0 0 0 1px var(--field-ro-ring)' } : { background: 'var(--field)', boxShadow: '0 0 0 1px var(--field-ring)' }}
    >
      <span className="min-w-0 flex-1 truncate text-row tracking-[0.02em] text-t1">{hint}</span>
      {trailing}
    </div>
  )
}

export function AiCleanup() {
  const ctx = useSurfaceContext()
  const ai = useSurface((s) => s.ai)
  const enabled = useSurface((s) => s.settings.aiCleanup)
  const hasKey = ai.state === 'connected' || ai.state === 'offline'
  const toggle: ToggleState = !hasKey ? 'disabled' : enabled ? 'on' : 'off'
  const hint = ai.keyHint ?? ''

  let body: ReactNode
  if (hasKey && !enabled) {
    body = (
      <div className="px-[12px] pb-[10px]">
        <div className="text-meta text-t3">Local cleanup only.</div>
      </div>
    )
  } else {
    let field: ReactNode
    let left: ReactNode
    let right: ReactNode = null
    switch (ai.state) {
      case 'checking':
        field = <KeyField hint={hint} trailing={<Spinner />} />
        left = <span className="text-t3">Checking…</span>
        break
      case 'connected':
      case 'offline':
        field = <KeyField hint={hint} readOnly trailing={ai.state === 'connected' ? <Check size={14} strokeWidth={2.5} className="text-t1" /> : null} />
        left = <span className="text-t3">{ai.state === 'connected' ? 'Connected' : "Can't reach Anthropic"}</span>
        right = ai.fromEnv ? <span className="text-t3">From ANTHROPIC_API_KEY</span> : <LinkButton onClick={() => removeApiKey(ctx)}>Remove</LinkButton>
        break
      case 'invalid':
        field = <KeyInput invalidHint={hint} />
        left = <span className="text-danger">Key not accepted</span>
        right = <GetKey />
        break
      default:
        field = <KeyInput />
        left = <span className="text-t3">Stored on this PC only</span>
        right = <GetKey />
    }
    body = (
      <div className="flex flex-col gap-[6px] px-[12px] pb-[12px] pt-[4px]">
        {field}
        <div className="flex h-[16px] shrink-0 items-center justify-between text-meta">
          {left}
          {right}
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col">
      <Header title="AI cleanup" onBack={() => drill(ctx, 'home')} />
      <div className="flex items-center justify-between gap-[12px] rounded-row px-[12px] py-[8px]">
        <div className="flex flex-col gap-[2px]">
          <div className="text-row text-t1">Use Claude</div>
          <div className="text-meta text-t3">Removes filler, fixes punctuation. Sends your words and window title to Anthropic.</div>
        </div>
        <Toggle label="Use Claude" state={toggle} onChange={(on) => saveSettings(ctx, { aiCleanup: on })} />
      </div>
      {body}
    </div>
  )
}
