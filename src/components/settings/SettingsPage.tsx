import { useState, useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useHotkeyRecorder } from '../../hooks/useHotkeyRecorder'
import type { AudioDevice } from '../../hooks/useTauri'
import { useCleanupToggle } from '../../hooks/useCleanupToggle'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { TONE_MODES, STT_MODELS, LLM_MODEL } from '../../lib/constants'
import { formatHotkey } from '../../lib/utils'
import { Toggle } from '../shared/Toggle'
import { Select } from '../shared/Select'
import { SegmentedControl } from '../shared/SegmentedControl'
import { KeyBadge } from '../shared/KeyBadge'
import { Button } from '../shared/Button'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] font-semibold text-[#aaa] uppercase tracking-[0.8px] mb-2 pl-0.5">
      {children}
    </div>
  )
}

function Row({
  children,
  last = false,
}: {
  children: React.ReactNode
  last?: boolean
}) {
  return (
    <div
      className={`flex items-center justify-between px-[18px] py-[14px] transition-colors hover:bg-[#FAFAF8] ${
        last ? '' : 'border-b border-[#F5F4F0]'
      }`}
    >
      {children}
    </div>
  )
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-white rounded-card border border-card-border overflow-hidden">
      {children}
    </div>
  )
}

function FeedbackSection() {
  const [expanded, setExpanded] = useState(false)
  const [text, setText] = useState('')
  const [status, setStatus] = useState<'idle' | 'sending' | 'sent' | 'error'>('idle')
  const [errorMsg, setErrorMsg] = useState('')

  const handleSend = async () => {
    setStatus('sending')
    try {
      await invoke('send_feedback', { text })
      setStatus('sent')
      setText('')
      setTimeout(() => {
        setStatus('idle')
        setExpanded(false)
      }, 3000)
    } catch (e) {
      setStatus('error')
      setErrorMsg(String(e))
    }
  }

  if (!expanded) {
    return (
      <button
        onClick={() => setExpanded(true)}
        className="text-[13px] font-medium text-[#1a1a1a] hover:text-chirp-amber-500 transition-colors w-full text-left"
      >
        Send Feedback
      </button>
    )
  }

  return (
    <div className="w-full">
      <div className="text-[13px] font-medium text-[#1a1a1a] mb-2">Send Feedback</div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Tell us what you think..."
        maxLength={2000}
        className="w-full h-24 rounded-lg border border-card-border bg-white p-3 text-sm font-body text-chirp-stone-900 resize-none focus:outline-none focus:border-chirp-amber-400 transition-colors"
      />
      <div className="flex items-center justify-between mt-2">
        <span className="text-[11px] text-chirp-stone-400">
          {status === 'sent' ? 'Sent!' : status === 'error' ? errorMsg : `${text.length}/2000`}
        </span>
        <div className="flex gap-2">
          <button
            onClick={() => { setExpanded(false); setText(''); setStatus('idle') }}
            className="text-[12px] text-chirp-stone-400 hover:text-chirp-stone-600"
          >
            Cancel
          </button>
          <Button
            onClick={handleSend}
            disabled={text.trim().length === 0 || status === 'sending' || status === 'sent'}
          >
            {status === 'sending' ? 'Sending...' : status === 'sent' ? 'Sent!' : 'Send'}
          </Button>
        </div>
      </div>
    </div>
  )
}

export function SettingsPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const { capturing, pendingHotkey, previewLabels, canConfirm, isModifierOnly, showSystemHint, systemCapturing, startCapture, startSystemCapture, confirmCapture, cancelCapture, clearPending } = useHotkeyRecorder()

  // Audio state
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [devicesLoading, setDevicesLoading] = useState(true)
  const [inputLevel, setInputLevel] = useState(0)
  const [testState, setTestState] = useState<'idle' | 'recording' | 'playing'>('idle')
  const [testCountdown, setTestCountdown] = useState(3)
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const testIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // AI / LLM state
  const { handleCleanupToggle, cleanupStarting, llmDownloaded } = useCleanupToggle()
  const [, setLlmDownloaded] = useLlmDownloaded()

  // Model download state
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [llmDownloadError, setLlmDownloadError] = useState<string | null>(null)

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isDownloaded = store.modelDownloaded[store.model]
  const currentHotkeyLabels = formatHotkey(store.hotkey)

  // Load audio devices
  useEffect(() => {
    tauri.getAudioDevices().then(setDevices).finally(() => setDevicesLoading(false))
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- one-time init

  // Live input level polling (~15fps)
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const level = await tauri.getInputLevel()
        setInputLevel(level)
      } catch { /* input level polling — device may be unavailable */ }
    }, 67)
    return () => clearInterval(interval)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps -- continuous polling

  // ── Handlers ──────────────────────────────────────────────

  const handleCaptureKey = () => {
    startCapture()
  }

  const handleConfirmCapture = () => {
    const result = confirmCapture()
    if (result) {
      store.updateSettings({ hotkey: result.hotkey })
      clearPending()
    }
  }

  const handleConfirmPending = () => {
    if (pendingHotkey) {
      store.updateSettings({ hotkey: pendingHotkey.hotkey })
      clearPending()
    }
  }


  const handleTest = async () => {
    // Clear any stale interval from a previous test
    if (testIntervalRef.current) clearInterval(testIntervalRef.current)
    setTestState('recording')
    setTestCountdown(3)
    testIntervalRef.current = setInterval(() => {
      setTestCountdown((c) => {
        if (c <= 1) {
          if (testIntervalRef.current) clearInterval(testIntervalRef.current)
          testIntervalRef.current = null
          return 0
        }
        return c - 1
      })
    }, 1000)

    try {
      const wavBytes = await tauri.testMicrophone()
      if (testIntervalRef.current) clearInterval(testIntervalRef.current)
      testIntervalRef.current = null

      const uint8 = new Uint8Array(wavBytes)
      const blob = new Blob([uint8], { type: 'audio/wav' })
      const url = URL.createObjectURL(blob)

      setTestState('playing')
      const audio = new Audio(url)
      audioRef.current = audio
      audio.onended = () => {
        setTestState('idle')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.onerror = () => {
        setTestState('idle')
        URL.revokeObjectURL(url)
        audioRef.current = null
      }
      audio.play().catch(() => {
        setTestState('idle')
        URL.revokeObjectURL(url)
        audioRef.current = null
      })
    } catch {
      if (testIntervalRef.current) clearInterval(testIntervalRef.current)
      testIntervalRef.current = null
      setTestState('idle')
    }
  }

  const handleDownload = async () => {
    setDownloadError(null)
    store.setModelDownloadProgress(0)
    try {
      await tauri.downloadModel(store.model, (progress) => {
        store.setModelDownloadProgress(progress)
      })
      store.updateSettings({
        modelDownloaded: { ...store.modelDownloaded, [store.model]: true },
      })
    } catch {
      setDownloadError('Download failed. Check your internet connection and try again.')
    } finally {
      store.setModelDownloadProgress(null)
    }
  }

  const handleLlmDownload = async () => {
    setLlmDownloadError(null)
    store.setLlmDownloadProgress(0)
    try {
      await tauri.downloadLlm((progress) => {
        store.setLlmDownloadProgress(progress)
      })
      setLlmDownloaded(true)

      if (store.aiCleanup) {
        try {
          await tauri.startLlm()
          store.setLlmReady(true)
        } catch (e) {
          console.error('Failed to start LLM after download:', e)
        }
      }
    } catch (e) {
      console.error('LLM download failed:', e)
      setLlmDownloadError('Download failed. Check your internet connection and try again.')
    } finally {
      store.setLlmDownloadProgress(null)
    }
  }

  const testLinkText =
    testState === 'recording'
      ? `Recording... (${testCountdown}s)`
      : testState === 'playing'
        ? 'Playing back...'
        : 'Test mic'

  // ── Render ────────────────────────────────────────────────

  return (
    <div className="flex flex-col gap-6 pb-8">
      {/* ── HOTKEY ─────────────────────────────────── */}
      <div className="animate-slide-up stagger-1">
        <SectionLabel>Hotkey</SectionLabel>
        <Card>
          {capturing ? (
            /* ── CAPTURING STATE: big prominent capture zone ── */
            <div className="p-5">
              <div className="flex items-center justify-between mb-4">
                <div>
                  <div className="text-[13px] font-medium text-[#1a1a1a]">Recording shortcut</div>
                  <div className="text-[11px] text-[#aaa] mt-0.5">Press keys one at a time — they&apos;ll stick</div>
                </div>
                <button
                  onClick={cancelCapture}
                  className="text-[12px] text-[#aaa] hover:text-[#666] hover:underline whitespace-nowrap transition-colors"
                >
                  Cancel
                </button>
              </div>

              {/* Large capture zone */}
              <div className="flex h-24 items-center justify-center rounded-xl border-2 border-dashed border-chirp-amber-400 bg-chirp-amber-50/30 transition-all duration-200">
                {previewLabels.length > 0 ? (
                  <div className="flex items-center gap-3">
                    {previewLabels.map((label, i) => (
                      <div key={label} className="flex items-center gap-3">
                        {i > 0 && <span className="text-[13px] text-chirp-stone-300 font-medium select-none">+</span>}
                        <span className="inline-flex min-w-[36px] items-center justify-center rounded-lg border border-card-border bg-white px-3 py-2 font-mono text-sm font-medium text-[#333] shadow-[0_2px_4px_rgba(0,0,0,0.06)]">
                          {label}
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <span className="text-sm text-chirp-stone-400 animate-pulse">Press any key or combo...</span>
                )}
              </div>

              {/* System capture hint */}
              {showSystemHint && !systemCapturing && (
                <button
                  onClick={startSystemCapture}
                  className="mt-1 text-[11px] text-chirp-amber-500 hover:underline"
                >
                  Key not detected? Capture via system
                </button>
              )}
              {systemCapturing && (
                <div className="mt-1 text-[11px] text-chirp-stone-400 animate-pulse">
                  Press any key on your keyboard...
                </div>
              )}

              {/* Modifier-only warning */}
              {canConfirm && isModifierOnly && (
                <div className="mt-1 text-[11px] text-chirp-amber-600">
                  Using a modifier key alone may conflict with other shortcuts.
                </div>
              )}

              {/* Confirm button */}
              {canConfirm && (
                <div className="mt-4 flex justify-end">
                  <button
                    onClick={handleConfirmCapture}
                    className="rounded-lg bg-[#1a1a1a] px-5 py-2 text-[13px] font-medium text-white hover:bg-[#333] transition-colors"
                  >
                    Set hotkey
                  </button>
                </div>
              )}
            </div>
          ) : (
            /* ── IDLE / PENDING STATE: compact row ── */
            <Row last>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <div className="text-[13px] font-medium text-[#1a1a1a]">Hotkey</div>
                  {store.hotkeyStatus === 'active' && (
                    <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                  )}
                  {store.hotkeyStatus === 'retrying' && (
                    <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400 animate-pulse" />
                  )}
                  {store.hotkeyStatus === 'failed' && (
                    <div className="h-1.5 w-1.5 rounded-full bg-red-400" />
                  )}
                </div>
                <div className="text-[11px] text-[#aaa] mt-0.5">
                  {store.hotkeyStatus === 'failed'
                    ? 'Hotkey unavailable — try a different shortcut'
                    : store.hotkeyStatus === 'retrying'
                      ? 'Setting up hotkey...'
                      : 'Hold to talk, release to transcribe'}
                </div>
              </div>

              <div className="flex items-center gap-3 ml-4">
                {pendingHotkey ? (
                  <>
                    <div className="flex items-center gap-1.5">
                      {pendingHotkey.labels.map((label) => (
                        <KeyBadge key={label} keyLabel={label} />
                      ))}
                    </div>
                    <button
                      onClick={handleConfirmPending}
                      className="text-[12px] font-medium text-chirp-amber-500 hover:underline whitespace-nowrap"
                    >
                      Set hotkey
                    </button>
                    <button
                      onClick={cancelCapture}
                      className="text-[12px] text-[#aaa] hover:underline whitespace-nowrap"
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <>
                    <div className="flex items-center gap-1.5">
                      {currentHotkeyLabels.map((label) => (
                        <KeyBadge key={label} keyLabel={label} />
                      ))}
                    </div>
                    <button
                      onClick={handleCaptureKey}
                      className="text-[12px] text-chirp-amber-500 hover:underline whitespace-nowrap"
                    >
                      Change
                    </button>
                  </>
                )}
              </div>
            </Row>
          )}
        </Card>
      </div>

      {/* ── AUDIO ──────────────────────────────────── */}
      <div className="animate-slide-up stagger-2">
        <SectionLabel>Audio</SectionLabel>
        <Card>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Input device</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Microphone used for recording</div>
            </div>
            <div className="w-[220px]">
              {devicesLoading ? (
                <span className="text-[12px] text-[#aaa]">Loading...</span>
              ) : (
                <Select
                  options={devices.map((d) => ({ value: d.id, label: d.name }))}
                  value={store.inputDevice}
                  onChange={(v) => store.updateSettings({ inputDevice: v as string })}
                />
              )}
            </div>
          </Row>

          <Row last>
            <div className="flex-1 mr-4">
              <div className="flex items-center gap-2">
                <div className="text-[13px] font-medium text-[#1a1a1a]">Input level</div>
                <button
                  onClick={handleTest}
                  disabled={testState !== 'idle'}
                  className="text-[11px] text-chirp-amber-500 hover:underline disabled:opacity-50 disabled:no-underline"
                >
                  {testLinkText}
                </button>
              </div>
              <div className="mt-2 h-[6px] w-full overflow-hidden rounded-full bg-[#F5F4F0]">
                <div
                  className="h-full rounded-full bg-chirp-success transition-all duration-100"
                  style={{ width: `${Math.min(100, inputLevel * 100)}%` }}
                />
              </div>
            </div>
          </Row>

        </Card>
      </div>

      {/* ── AI & OUTPUT ────────────────────────────── */}
      <div className="animate-slide-up stagger-3">
        <SectionLabel>AI &amp; Output</SectionLabel>
        <Card>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Smart formatting</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Automatically format lists, paragraphs, and structure</div>
            </div>
            <Toggle
              checked={store.smartFormatting}
              onChange={(v) => store.updateSettings({ smartFormatting: v })}
            />
          </Row>

          <Row>
            <div>
              <div className="flex items-center gap-2">
                <div className="text-[13px] font-medium text-[#1a1a1a]">Smart Cleanup</div>
                {store.aiCleanup && (
                  <span className="flex items-center gap-1">
                    {cleanupStarting ? (
                      <>
                        <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400 animate-pulse" />
                        <span className="text-[11px] text-[#aaa]">Getting ready...</span>
                      </>
                    ) : store.llmReady ? (
                      <>
                        <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                        <span className="text-[11px] text-[#aaa]">Active</span>
                      </>
                    ) : !llmDownloaded ? (
                      <>
                        <div className="h-1.5 w-1.5 rounded-full bg-chirp-amber-400" />
                        <span className="text-[11px] text-chirp-amber-500">Model needed</span>
                      </>
                    ) : null}
                  </span>
                )}
              </div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Polish grammar and filler words with local AI</div>
            </div>
            <Toggle
              checked={store.aiCleanup}
              onChange={handleCleanupToggle}
              disabled={cleanupStarting}
            />
          </Row>

          {store.aiCleanup && (
            <Row last>
              <div>
                <div className="text-[13px] font-medium text-[#1a1a1a]">Tone</div>
                <div className="text-[11px] text-[#aaa] mt-0.5">
                  {TONE_MODES.find(m => m.id === store.toneMode)?.description}
                </div>
              </div>
              <div className="w-[180px]">
                <Select
                  options={TONE_MODES.map(m => ({ value: m.id, label: m.label }))}
                  value={store.toneMode}
                  onChange={(v) => store.updateSettings({ toneMode: String(v) })}
                />
              </div>
            </Row>
          )}

          {!store.aiCleanup && (
            /* Invisible row closer — the cleanup row is last when tone is hidden */
            <div className="hidden" />
          )}
        </Card>
      </div>

      {/* ── BEHAVIOR ───────────────────────────────── */}
      <div className="animate-slide-up stagger-4">
        <SectionLabel>Behavior</SectionLabel>
        <Card>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Launch at login</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Start Chirp when you sign in</div>
            </div>
            <Toggle
              checked={store.launchAtLogin}
              onChange={(v) => store.updateSettings({ launchAtLogin: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Auto-dismiss overlay</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Hide the overlay after text is injected</div>
            </div>
            <Toggle
              checked={store.autoDismissOverlay}
              onChange={(v) => store.updateSettings({ autoDismissOverlay: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Completion sound</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Play a sound when transcription finishes</div>
            </div>
            <Toggle
              checked={store.playSoundOnComplete}
              onChange={(v) => store.updateSettings({ playSoundOnComplete: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Passive indicator</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Small icon on screen when Chirp is ready</div>
            </div>
            <Toggle
              checked={store.showPassiveOverlay}
              onChange={(v) => store.updateSettings({ showPassiveOverlay: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">Overlay position</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Where the recording pill appears</div>
            </div>
            <SegmentedControl
              options={[
                { value: 'bottom', label: 'Bottom' },
                { value: 'top', label: 'Top' },
              ]}
              value={store.overlayPosition}
              onChange={(v) => store.updateSettings({ overlayPosition: v as 'bottom' | 'top' })}
            />
          </Row>
          <Row last>
            <div>
              <div className="text-[13px] font-medium text-[#1a1a1a]">History retention</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Automatically delete old transcriptions</div>
            </div>
            <div className="w-[140px]">
              <Select
                options={[
                  { value: 0, label: 'Forever' },
                  { value: 7, label: '7 days' },
                  { value: 30, label: '30 days' },
                  { value: 90, label: '90 days' },
                ]}
                value={store.historyRetentionDays}
                onChange={(v) => store.updateSettings({ historyRetentionDays: Number(v) })}
              />
            </div>
          </Row>
        </Card>
      </div>

      {/* ── MODELS ─────────────────────────────────── */}
      <div className="animate-slide-up stagger-5">
        <SectionLabel>Models</SectionLabel>
        <Card>
          {/* Speech model */}
          <Row>
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <div className="text-[13px] font-medium text-[#1a1a1a]">{currentModel?.name}</div>
                <span className="text-[11px] text-[#aaa]">{currentModel?.size}</span>
              </div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Speech recognition model</div>
              {store.modelDownloadProgress !== null && (
                <div className="mt-2">
                  <div className="flex items-center gap-2">
                    <div className="h-1 flex-1 overflow-hidden rounded-full bg-[#F5F4F0]">
                      <div
                        className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200"
                        style={{ width: `${store.modelDownloadProgress}%` }}
                      />
                    </div>
                    <span className="text-[11px] text-[#aaa]">{store.modelDownloadProgress}%</span>
                  </div>
                </div>
              )}
              {downloadError && (
                <p className="mt-1 text-[11px] text-chirp-error">{downloadError}</p>
              )}
            </div>
            <div className="ml-4">
              {isDownloaded ? (
                <div className="flex items-center gap-1.5">
                  <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                  <span className="text-[12px] text-[#888]">Ready</span>
                </div>
              ) : (
                <Button onClick={handleDownload} disabled={store.modelDownloadProgress !== null}>
                  Download
                </Button>
              )}
            </div>
          </Row>

          {/* LLM model */}
          <Row last>
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <div className="text-[13px] font-medium text-[#1a1a1a]">{LLM_MODEL.displayName} engine</div>
                <span className="text-[11px] text-[#aaa]">{LLM_MODEL.friendlySize}</span>
              </div>
              <div className="text-[11px] text-[#aaa] mt-0.5">Polishes grammar and sentences locally</div>
              <div className="text-[10px] text-chirp-stone-400 mt-0.5">{LLM_MODEL.attribution}</div>
              {store.llmDownloadProgress !== null && (
                <div className="mt-2">
                  <div className="flex items-center gap-2">
                    <div className="h-1 flex-1 overflow-hidden rounded-full bg-[#F5F4F0]">
                      <div
                        className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200"
                        style={{ width: `${store.llmDownloadProgress}%` }}
                      />
                    </div>
                    <span className="text-[11px] text-[#aaa]">{store.llmDownloadProgress}%</span>
                  </div>
                </div>
              )}
              {llmDownloadError && (
                <p className="mt-1 text-[11px] text-chirp-error">{llmDownloadError}</p>
              )}
            </div>
            <div className="ml-4">
              {llmDownloaded ? (
                <div className="flex items-center gap-1.5">
                  <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
                  <span className="text-[12px] text-[#888]">Ready</span>
                </div>
              ) : (
                <Button onClick={handleLlmDownload} disabled={store.llmDownloadProgress !== null}>
                  Download
                </Button>
              )}
            </div>
          </Row>
        </Card>
      </div>

      {/* ── PRIVACY & FEEDBACK ───────────────────────── */}
      <div className="mt-6">
        <SectionLabel>Privacy & Feedback</SectionLabel>
        <Card>
          <Row>
            <div className="flex-1">
              <div className="text-[13px] font-medium text-[#1a1a1a]">Help improve Chirp</div>
              <div className="text-[11px] text-[#aaa] mt-0.5">
                Share anonymous usage stats and crash reports
              </div>
              <div className="text-[10px] text-chirp-stone-400 mt-1">
                Changes take effect on restart
              </div>
            </div>
            <Toggle
              checked={store.helpImprove}
              onChange={(v) => store.updateSettings({ helpImprove: v })}
            />
          </Row>
          <Row last>
            <FeedbackSection />
          </Row>
        </Card>
      </div>
    </div>
  )
}
