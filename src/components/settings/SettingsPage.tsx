import { useState, useEffect, useRef } from 'react'
import { emit } from '@tauri-apps/api/event'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { BindingSettings } from '../shared/BindingSettings'
import type { AudioDevice } from '../../hooks/useTauri'
import { STT_MODELS } from '../../lib/constants'
import { Toggle } from '../shared/Toggle'
import { Select } from '../shared/Select'
import { Button } from '../shared/Button'


function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] font-semibold text-dm-secondary uppercase tracking-[0.8px] mb-2 pl-0.5">
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
      className={`flex items-center justify-between px-[18px] py-[14px] transition-colors hover:bg-card-hover ${
        last ? '' : 'border-b border-dm-row-sep'
      }`}
    >
      {children}
    </div>
  )
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-card rounded-card border border-card-border overflow-hidden">
      {children}
    </div>
  )
}

function describePosition(pos: unknown): string {
  if (typeof pos === 'string') {
    return pos === 'top' ? 'top center' : 'bottom center'
  }
  if (pos && typeof pos === 'object' && 'x' in pos && 'y' in pos) {
    return 'custom position'
  }
  return 'bottom center'
}

export function SettingsPage() {
  const store = useAppStore()
  const tauri = useTauri()

  // Audio state
  const [devices, setDevices] = useState<AudioDevice[]>([])
  const [devicesLoading, setDevicesLoading] = useState(true)
  const [inputLevel, setInputLevel] = useState(0)
  const [testState, setTestState] = useState<'idle' | 'recording' | 'playing'>('idle')
  const [testCountdown, setTestCountdown] = useState(3)
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const testIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Model download state
  const [downloadError, setDownloadError] = useState<string | null>(null)

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isDownloaded = store.modelDownloaded[store.model]

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
        <SectionLabel>Hotkeys</SectionLabel>
        <Card><BindingSettings /></Card>
      </div>

      {/* ── AUDIO ──────────────────────────────────── */}
      <div className="animate-slide-up stagger-2">
        <SectionLabel>Audio</SectionLabel>
        <Card>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Input device</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Microphone used for recording</div>
            </div>
            <div className="w-[220px]">
              {devicesLoading ? (
                <span className="text-[12px] text-dm-secondary">Loading...</span>
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
                <div className="text-[13px] font-medium text-dm-primary">Input level</div>
                <button
                  onClick={handleTest}
                  disabled={testState !== 'idle'}
                  className="text-[11px] text-chirp-amber-500 hover:underline disabled:opacity-50 disabled:no-underline"
                >
                  {testLinkText}
                </button>
              </div>
              <div className="mt-2 h-[6px] w-full overflow-hidden rounded-full bg-surface">
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
              <div className="text-[13px] font-medium text-dm-primary">Smart formatting</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Automatically format lists, paragraphs, and structure</div>
            </div>
            <Toggle
              checked={store.smartFormatting}
              onChange={(v) => store.updateSettings({ smartFormatting: v })}
            />
          </Row>

          <Row last>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Enhanced Recognition</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Better accuracy with accents and noise, slightly slower</div>
            </div>
            <Toggle
              checked={store.beamSearch}
              onChange={(v) => store.updateSettings({ beamSearch: v })}
            />
          </Row>
        </Card>
      </div>

      {/* ── BEHAVIOR ───────────────────────────────── */}
      <div className="animate-slide-up stagger-4">
        <SectionLabel>Behavior</SectionLabel>
        <Card>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Launch at login</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Start Chirp when you sign in</div>
            </div>
            <Toggle
              checked={store.launchAtLogin}
              onChange={(v) => store.updateSettings({ launchAtLogin: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Auto-dismiss overlay</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Hide the overlay after text is injected</div>
            </div>
            <Toggle
              checked={store.autoDismissOverlay}
              onChange={(v) => store.updateSettings({ autoDismissOverlay: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Completion sound</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Play a sound when transcription finishes</div>
            </div>
            <Toggle
              checked={store.playSoundOnComplete}
              onChange={(v) => store.updateSettings({ playSoundOnComplete: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Passive indicator</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Small icon on screen when Chirp is ready</div>
            </div>
            <Toggle
              checked={store.showPassiveOverlay}
              onChange={(v) => store.updateSettings({ showPassiveOverlay: v })}
            />
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Overlay position</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">
                Currently: {describePosition(store.overlayPosition)}
              </div>
            </div>
            <Button
              onClick={() => emit('enter-reposition-mode')}
              variant="secondary"
            >
              Reposition
            </Button>
          </Row>
          <Row>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">History retention</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Automatically delete old transcriptions</div>
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
          <Row last>
            <div>
              <div className="text-[13px] font-medium text-dm-primary">Dark mode</div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Use a dark theme for the content area</div>
            </div>
            <Toggle
              checked={store.darkMode}
              onChange={(v) => store.updateSettings({ darkMode: v })}
            />
          </Row>
        </Card>
      </div>

      {/* ── MODELS ─────────────────────────────────── */}
      <div className="animate-slide-up stagger-5">
        <SectionLabel>Models</SectionLabel>
        <Card>
          {/* Speech model */}
          <Row last>
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <div className="text-[13px] font-medium text-dm-primary">{currentModel?.name}</div>
                <span className="text-[11px] text-dm-secondary">{currentModel?.size}</span>
              </div>
              <div className="text-[11px] text-dm-secondary mt-0.5">Speech recognition model</div>
              {store.modelDownloadProgress !== null && (
                <div className="mt-2">
                  <div className="flex items-center gap-2">
                    <div className="h-1 flex-1 overflow-hidden rounded-full bg-surface">
                      <div
                        className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200"
                        style={{ width: `${store.modelDownloadProgress}%` }}
                      />
                    </div>
                    <span className="text-[11px] text-dm-secondary">{store.modelDownloadProgress}%</span>
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
                  <span className="text-[12px] text-dm-muted">Ready</span>
                </div>
              ) : (
                <Button onClick={handleDownload} disabled={store.modelDownloadProgress !== null}>
                  Download
                </Button>
              )}
            </div>
          </Row>
        </Card>
      </div>
    </div>
  )
}
