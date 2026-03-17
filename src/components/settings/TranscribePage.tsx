import { useState, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'

interface FileTranscriptionResult {
  text: string
  durationSecs: number
  wordCount: number
  chunks: number
}

interface ProgressEvent {
  phase: string
  progress: number
  chunk?: number
  totalChunks?: number
}

export function TranscribePage() {
  const [filePath, setFilePath] = useState<string | null>(null)
  const [fileName, setFileName] = useState<string | null>(null)
  const [transcribing, setTranscribing] = useState(false)
  const [progress, setProgress] = useState<ProgressEvent | null>(null)
  const [result, setResult] = useState<FileTranscriptionResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  const handlePickFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Audio Files',
        extensions: ['wav', 'mp3', 'm4a', 'flac', 'ogg', 'mp4', 'aac'],
      }],
    })
    if (selected) {
      const path = typeof selected === 'string' ? selected : selected
      setFilePath(path)
      setFileName(path.split(/[\\/]/).pop() ?? path)
      setResult(null)
      setError(null)
    }
  }

  const handleTranscribe = useCallback(async () => {
    if (!filePath) return
    setTranscribing(true)
    setError(null)
    setProgress({ phase: 'decoding', progress: 0 })

    const unlisten = await listen<ProgressEvent>('file-transcribe-progress', (event) => {
      setProgress(event.payload)
    })

    try {
      const res = await invoke<FileTranscriptionResult>('transcribe_file', { path: filePath })
      setResult(res)
    } catch (e) {
      setError(typeof e === 'string' ? e : 'Transcription failed')
    } finally {
      unlisten()
      setTranscribing(false)
      setProgress(null)
    }
  }, [filePath])

  const handleCopy = async () => {
    if (!result) return
    await navigator.clipboard.writeText(result.text)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  const handleClear = () => {
    setFilePath(null)
    setFileName(null)
    setResult(null)
    setError(null)
    setProgress(null)
  }

  const formatDuration = (secs: number) => {
    const m = Math.floor(secs / 60)
    const s = Math.round(secs % 60)
    return m > 0 ? `${m}m ${s}s` : `${s}s`
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">Transcribe File</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">
          Transcribe audio files locally. Supports WAV, MP3, M4A, FLAC, OGG, and MP4.
        </p>
      </div>

      <SettingGroup label="Audio File">
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <Button variant="secondary" onClick={handlePickFile} disabled={transcribing}>
              Choose File
            </Button>
            {fileName && (
              <span className="font-body text-sm text-chirp-stone-700 truncate max-w-[300px]">
                {fileName}
              </span>
            )}
          </div>

          {filePath && !transcribing && !result && (
            <Button onClick={handleTranscribe}>
              Transcribe
            </Button>
          )}

          {transcribing && progress && (
            <div className="flex flex-col gap-2">
              <div className="h-2 w-full overflow-hidden rounded-full bg-chirp-stone-200">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-300"
                  style={{ width: `${progress.progress}%` }}
                />
              </div>
              <span className="font-body text-xs text-chirp-stone-500">
                {progress.phase === 'decoding' && 'Decoding audio...'}
                {progress.phase === 'transcribing' && (
                  progress.totalChunks && progress.totalChunks > 1
                    ? `Transcribing chunk ${progress.chunk} of ${progress.totalChunks}...`
                    : 'Transcribing...'
                )}
              </span>
            </div>
          )}

          {error && (
            <p className="font-body text-sm text-red-500">{error}</p>
          )}
        </div>
      </SettingGroup>

      {result && (
        <SettingGroup label="Result">
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-4 text-xs font-body text-chirp-stone-500">
              <span>{result.wordCount} words</span>
              <span>{formatDuration(result.durationSecs)} audio</span>
              <span>{result.chunks} chunk{result.chunks !== 1 ? 's' : ''}</span>
            </div>
            <textarea
              readOnly
              value={result.text}
              className="h-64 w-full resize-y rounded-xl border border-chirp-stone-200 bg-white p-3 font-body text-sm text-chirp-stone-900 focus:outline-none"
            />
            <div className="flex gap-2">
              <Button variant="secondary" onClick={handleCopy}>
                {copied ? 'Copied!' : 'Copy to Clipboard'}
              </Button>
              <Button variant="secondary" onClick={handleClear}>
                Clear
              </Button>
            </div>
          </div>
        </SettingGroup>
      )}
    </div>
  )
}
