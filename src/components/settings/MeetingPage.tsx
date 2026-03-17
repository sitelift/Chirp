import { useState, useEffect, useRef, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'

interface MeetingResult {
  transcript: string
  summary: string
  durationSecs: number
  wordCount: number
  timestamp: string
}

interface MeetingProgress {
  phase: string
  progress: number
}

type AudioSource = 'microphone' | 'system' | 'both'

export function MeetingPage() {
  const [source, setSource] = useState<AudioSource>('both')
  const [recording, setRecording] = useState(false)
  const [processing, setProcessing] = useState(false)
  const [phase, setPhase] = useState<string | null>(null)
  const [progress, setProgress] = useState(0)
  const [elapsed, setElapsed] = useState(0)
  const [result, setResult] = useState<MeetingResult | null>(null)
  const [history, setHistory] = useState<MeetingResult[]>([])
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState<'transcript' | 'summary' | null>(null)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Load meeting history on mount
  useEffect(() => {
    invoke<MeetingResult[]>('get_meeting_history').then(setHistory).catch(() => {})
  }, [])

  // Elapsed time counter
  useEffect(() => {
    if (recording) {
      setElapsed(0)
      timerRef.current = setInterval(() => setElapsed((e) => e + 1), 1000)
    } else {
      if (timerRef.current) clearInterval(timerRef.current)
    }
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [recording])

  const formatTime = (secs: number) => {
    const m = Math.floor(secs / 60).toString().padStart(2, '0')
    const s = (secs % 60).toString().padStart(2, '0')
    return `${m}:${s}`
  }

  const handleStart = async () => {
    setError(null)
    setResult(null)
    try {
      await invoke('start_meeting', { source })
      setRecording(true)
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Failed to start recording')
    }
  }

  const handleStop = useCallback(async () => {
    setRecording(false)
    setProcessing(true)
    setPhase('transcribing')

    const unlisten = await listen<MeetingProgress>('meeting-progress', (event) => {
      setPhase(event.payload.phase)
      setProgress(event.payload.progress)
    })

    try {
      const duration = await invoke<number>('stop_meeting')
      const transcript = await invoke<string>('transcribe_meeting')
      const meetingResult = await invoke<MeetingResult>('summarize_meeting', { transcript })
      meetingResult.durationSecs = duration
      setResult(meetingResult)

      // Refresh history
      const h = await invoke<MeetingResult[]>('get_meeting_history')
      setHistory(h)
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Meeting processing failed')
    } finally {
      unlisten()
      setProcessing(false)
      setPhase(null)
      setProgress(0)
    }
  }, [])

  const handleCopy = async (field: 'transcript' | 'summary') => {
    if (!result) return
    const text = field === 'transcript' ? result.transcript : result.summary
    await navigator.clipboard.writeText(text)
    setCopied(field)
    setTimeout(() => setCopied(null), 1500)
  }

  const handleDelete = async (timestamp: string) => {
    await invoke('delete_meeting', { timestamp })
    setHistory((h) => h.filter((m) => m.timestamp !== timestamp))
    if (result?.timestamp === timestamp) setResult(null)
  }

  const sourceOptions: { value: AudioSource; label: string; description: string }[] = [
    { value: 'microphone', label: 'Microphone', description: 'Record from your mic only' },
    { value: 'system', label: 'System Audio', description: 'Record what you hear only' },
    { value: 'both', label: 'Both', description: 'Mic + system audio combined' },
  ]

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">Meeting</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">
          Record, transcribe, and summarize meetings locally.
        </p>
      </div>

      <SettingGroup label="Audio Source">
        <div className="flex gap-2">
          {sourceOptions.map((opt) => (
            <button
              key={opt.value}
              onClick={() => !recording && setSource(opt.value)}
              disabled={recording}
              className={`flex-1 rounded-xl border p-3 text-left transition-colors ${
                source === opt.value
                  ? 'border-chirp-amber-400 bg-chirp-amber-50'
                  : 'border-chirp-stone-200 bg-white hover:bg-chirp-stone-50'
              }`}
            >
              <span className="font-body text-sm font-medium text-chirp-stone-900">{opt.label}</span>
              <p className="font-body text-xs text-chirp-stone-500 mt-0.5">{opt.description}</p>
            </button>
          ))}
        </div>
      </SettingGroup>

      <SettingGroup label="Recording">
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-4">
            {!recording && !processing ? (
              <Button onClick={handleStart}>Start Recording</Button>
            ) : recording ? (
              <Button onClick={handleStop} variant="secondary">
                Stop Recording
              </Button>
            ) : null}

            {recording && (
              <div className="flex items-center gap-2">
                <div className="h-3 w-3 rounded-full bg-red-500 animate-pulse" />
                <span className="font-mono text-lg text-chirp-stone-900">{formatTime(elapsed)}</span>
              </div>
            )}
          </div>

          {processing && (
            <div className="flex flex-col gap-2">
              <div className="h-2 w-full overflow-hidden rounded-full bg-chirp-stone-200">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-300"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <span className="font-body text-xs text-chirp-stone-500">
                {phase === 'transcribing' && 'Transcribing...'}
                {phase === 'summarizing' && 'Summarizing...'}
              </span>
            </div>
          )}

          {error && (
            <p className="font-body text-sm text-red-500">{error}</p>
          )}
        </div>
      </SettingGroup>

      {result && (
        <>
          <SettingGroup label="Transcript">
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-4 text-xs font-body text-chirp-stone-500">
                <span>{result.wordCount} words</span>
              </div>
              <textarea
                readOnly
                value={result.transcript}
                className="h-48 w-full resize-y rounded-xl border border-chirp-stone-200 bg-white p-3 font-body text-sm text-chirp-stone-900 focus:outline-none"
              />
              <Button variant="secondary" onClick={() => handleCopy('transcript')}>
                {copied === 'transcript' ? 'Copied!' : 'Copy Transcript'}
              </Button>
            </div>
          </SettingGroup>

          <SettingGroup label="Summary">
            <div className="flex flex-col gap-3">
              <div className="whitespace-pre-wrap rounded-xl border border-chirp-stone-200 bg-white p-3 font-body text-sm text-chirp-stone-900">
                {result.summary}
              </div>
              <Button variant="secondary" onClick={() => handleCopy('summary')}>
                {copied === 'summary' ? 'Copied!' : 'Copy Summary'}
              </Button>
            </div>
          </SettingGroup>
        </>
      )}

      {history.length > 0 && (
        <SettingGroup label="History">
          <div className="flex flex-col gap-2">
            {history.slice().reverse().map((meeting) => (
              <div
                key={meeting.timestamp}
                className="flex items-center justify-between rounded-xl border border-chirp-stone-200 bg-white p-3"
              >
                <div className="flex flex-col">
                  <span className="font-body text-sm text-chirp-stone-900">
                    {new Date(meeting.timestamp).toLocaleDateString()} {new Date(meeting.timestamp).toLocaleTimeString()}
                  </span>
                  <span className="font-body text-xs text-chirp-stone-500">
                    {meeting.wordCount} words
                  </span>
                </div>
                <div className="flex gap-2">
                  <Button
                    variant="secondary"
                    onClick={() => setResult(meeting)}
                  >
                    View
                  </Button>
                  <button
                    onClick={() => handleDelete(meeting.timestamp)}
                    className="font-body text-xs text-chirp-stone-400 hover:text-red-500"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        </SettingGroup>
      )}
    </div>
  )
}
