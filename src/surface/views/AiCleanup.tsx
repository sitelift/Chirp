import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { open } from '@tauri-apps/plugin-shell'
import { providerOf, setUi, useSurface, useSurfaceContext } from '../store'
import { Reveal, Segmented, Spinner } from '../parts'
import { removeApiKey, saveSettings, setApiKey } from '../actions'
import { anim } from '../motion'
import { IconOpen } from '../icons'
import type { KeyProvider, Provider } from '../types'

const PROVIDERS: Record<KeyProvider, { name: string; model: string; mask: string; env: string; keysUrl: string }> = {
  anthropic: { name: 'Anthropic', model: 'Claude Haiku 5.5', mask: 'sk-ant-••••••••••', env: 'ANTHROPIC_API_KEY', keysUrl: 'https://console.anthropic.com/settings/keys' },
  openai: { name: 'OpenAI', model: 'GPT-6 Luna', mask: 'sk-••••••••••', env: 'OPENAI_API_KEY', keysUrl: 'https://platform.openai.com/api-keys' },
}

function Status({ tone, children }: { tone?: 'danger' | 'quiet'; children: ReactNode }) {
  return (
    <span className="status" data-tone={tone}>
      <i />
      {children}
    </span>
  )
}

/** The key field and a same-height button: Remove, or Get a key. */
function KeyRow({ provider }: { provider: KeyProvider }) {
  const ctx = useSurfaceContext()
  const status = useSurface((s) => s.ai[provider])
  const draft = useSurface((s) => s.ui.aiDraft)
  const p = PROVIDERS[provider]
  const fieldRef = useRef<HTMLDivElement>(null)
  const shown = useRef(provider)

  // Another provider's key: a quick fade so the change reads.
  useLayoutEffect(() => {
    if (shown.current === provider) return
    shown.current = provider
    if (!ctx.still && fieldRef.current) anim(fieldRef.current, [{ opacity: 0.4 }, { opacity: 1 }], { duration: 160, easing: 'linear' })
  }, [provider, ctx.still])

  const hint = <span className="keytext">{status.keyHint ?? p.mask}</span>
  let field: ReactNode
  switch (status.state) {
    case 'checking':
      field = (
        <>
          {hint}
          <span className="status">
            <Spinner />
            Checking
          </span>
        </>
      )
      break
    case 'connected':
      field = (
        <>
          {hint}
          <Status>Connected</Status>
        </>
      )
      break
    case 'offline':
      field = (
        <>
          {hint}
          <Status tone="quiet">Offline</Status>
        </>
      )
      break
    case 'invalid':
      field = (
        <>
          {hint}
          <Status tone="danger">Not accepted</Status>
        </>
      )
      break
    default:
      field = (
        <input
          type="password"
          value={draft}
          autoComplete="off"
          spellCheck={false}
          aria-label={`${p.name} API key`}
          placeholder={`Paste your ${p.name} key`}
          onChange={(e) => setUi(ctx.store, { aiDraft: e.target.value })}
          onPaste={(e) => {
            const text = e.clipboardData.getData('text')
            if (text.trim()) {
              e.preventDefault()
              setApiKey(ctx, provider, text)
            }
          }}
          onKeyDown={(e) => e.key === 'Enter' && setApiKey(ctx, provider, draft)}
        />
      )
  }

  let action: ReactNode
  if (status.state === 'none') {
    action = (
      <button type="button" data-nav="" className="nav quiet" onClick={() => ctx.api.live && open(p.keysUrl)}>
        Get a key
        <IconOpen size={12} />
      </button>
    )
  } else if (status.fromEnv) {
    action = <span className="quiet" title={`Set by ${p.env}`}>From {p.env}</span>
  } else {
    action = (
      <button type="button" data-nav="" className="nav quiet" onClick={() => removeApiKey(ctx, provider)}>
        Remove
      </button>
    )
  }

  return (
    <div className="keyrow">
      <div ref={fieldRef} className="keyfield">
        {field}
      </div>
      {action}
    </div>
  )
}

const MODEL: Record<Provider, string> = { off: 'No cleanup', anthropic: PROVIDERS.anthropic.model, openai: PROVIDERS.openai.model }

export function AiCleanup() {
  const ctx = useSurfaceContext()
  const provider = useSurface((s) => providerOf(s.settings))
  // The key block keeps the last provider's key while it folds away under Off.
  const [keyFor, setKeyFor] = useState<KeyProvider>(provider === 'off' ? 'anthropic' : provider)
  if (provider !== 'off' && provider !== keyFor) setKeyFor(provider)

  const choose = (v: Provider) => {
    setUi(ctx.store, { aiDraft: '' })
    saveSettings(ctx, { provider: v })
  }
  return (
    <>
      <h2>AI cleanup</h2>
      <p className="lede">Removes filler words, fixes punctuation and formats text for the app you&apos;re in.</p>
      <div>
        <div className="item">
          <div className="txt">
            <b>Provider</b>
            <small>{MODEL[provider]}</small>
          </div>
          <Segmented<Provider>
            label="Provider"
            width={80}
            value={provider}
            onChange={choose}
            options={[
              { value: 'off', label: 'Off' },
              { value: 'anthropic', label: 'Anthropic' },
              { value: 'openai', label: 'OpenAI' },
            ]}
          />
        </div>
        <Reveal open={provider !== 'off'}>
          <div className="item col">
            <div className="txt">
              <b>API key</b>
            </div>
            <KeyRow provider={keyFor} />
          </div>
        </Reveal>
      </div>
      <div className="note">{provider === 'off' ? 'Your dictation stays on this PC.' : `Your dictation is sent to ${PROVIDERS[provider].name} to be cleaned up.`}</div>
    </>
  )
}
