import { useState, useEffect } from 'react'
import { Check, Download, Sparkles } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { LLM_MODEL } from '../../lib/constants'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'

export function AiCleanupPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [testInput, setTestInput] = useState('')
  const [testOutput, setTestOutput] = useState<string | null>(null)
  const [testing, setTesting] = useState(false)
  const [llmDownloaded, setLlmDownloaded] = useState(false)
  const [serverRunning, setServerRunning] = useState(false)
  const [starting, setStarting] = useState(false)

  useEffect(() => {
    tauri.getLlmStatus().then((status) => {
      setLlmDownloaded(status.binaryDownloaded && status.modelDownloaded)
      setServerRunning(status.serverRunning)
      store.setLlmReady(status.serverRunning)
    }).catch(() => {})
  }, [])

  const handleToggle = async () => {
    const newValue = !store.aiCleanup
    store.updateSettings({ aiCleanup: newValue })

    if (newValue && llmDownloaded && !serverRunning) {
      setStarting(true)
      try {
        await tauri.startLlm()
        setServerRunning(true)
        store.setLlmReady(true)
      } catch {}
      setStarting(false)
    } else if (!newValue && serverRunning) {
      try {
        await tauri.stopLlm()
        setServerRunning(false)
        store.setLlmReady(false)
      } catch {}
    }
  }

  const handleDownload = async () => {
    setDownloadError(null)
    store.setLlmDownloadProgress(0)
    try {
      await tauri.downloadLlm((progress) => {
        store.setLlmDownloadProgress(progress)
      })
      setLlmDownloaded(true)

      // Auto-start server after download if toggle is on
      if (store.aiCleanup) {
        setStarting(true)
        try {
          await tauri.startLlm()
          setServerRunning(true)
          store.setLlmReady(true)
        } catch {}
        setStarting(false)
      }
    } catch {
      setDownloadError('Download failed. Check your internet connection and try again.')
    } finally {
      store.setLlmDownloadProgress(null)
    }
  }

  const handleTest = async () => {
    if (!testInput.trim()) return
    setTesting(true)
    setTestOutput(null)
    try {
      const result = await tauri.testLlmCleanup(testInput)
      setTestOutput(result)
    } catch (e) {
      setTestOutput(`Error: ${e}`)
    } finally {
      setTesting(false)
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">AI Cleanup</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">
          Polish transcriptions with a local AI model — fix grammar, restructure sentences, and clean up formatting.
        </p>
      </div>

      <SettingGroup label="AI Cleanup">
        <div className="flex items-center justify-between">
          <div>
            <span className="font-body text-sm font-medium text-chirp-stone-900">Enable AI cleanup</span>
            <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
              Runs a small language model after transcription to improve text quality.
            </p>
          </div>
          <button
            onClick={handleToggle}
            disabled={starting}
            className={`relative h-6 w-11 rounded-full transition-colors duration-200 ${
              store.aiCleanup ? 'bg-chirp-amber-400' : 'bg-chirp-stone-300'
            }`}
          >
            <div
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform duration-200 ${
                store.aiCleanup ? 'translate-x-[22px]' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        {/* Status line */}
        {store.aiCleanup && (
          <div className="flex items-center gap-2">
            {starting ? (
              <>
                <div className="h-2 w-2 rounded-full bg-chirp-amber-400 animate-pulse" />
                <span className="font-body text-xs text-chirp-stone-500">Starting...</span>
              </>
            ) : serverRunning ? (
              <>
                <div className="h-2 w-2 rounded-full bg-chirp-success" />
                <span className="font-body text-xs text-chirp-stone-500">Running</span>
              </>
            ) : !llmDownloaded ? (
              <>
                <div className="h-2 w-2 rounded-full bg-chirp-amber-400" />
                <span className="font-body text-xs text-chirp-stone-500">Model download required</span>
              </>
            ) : (
              <>
                <div className="h-2 w-2 rounded-full bg-chirp-stone-300" />
                <span className="font-body text-xs text-chirp-stone-500">Stopped</span>
              </>
            )}
          </div>
        )}
      </SettingGroup>

      {/* Download section - only show when needed */}
      {store.aiCleanup && !llmDownloaded && (
        <SettingGroup label="Download">
          <div className="flex items-center justify-between">
            <div>
              <span className="font-body text-sm font-medium text-chirp-stone-900">{LLM_MODEL.name}</span>
              <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
                {LLM_MODEL.size} download. {LLM_MODEL.description}
              </p>
            </div>
            <Button onClick={handleDownload} disabled={store.llmDownloadProgress !== null}>
              <Download size={14} className="mr-1.5" />
              Download
            </Button>
          </div>

          {store.llmDownloadProgress !== null && (
            <div className="mt-1">
              <div className="flex items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-chirp-stone-200">
                  <div
                    className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200"
                    style={{ width: `${store.llmDownloadProgress}%` }}
                  />
                </div>
                <span className="font-body text-xs text-chirp-stone-500">
                  {store.llmDownloadProgress}%
                </span>
              </div>
            </div>
          )}

          {downloadError && (
            <p className="mt-1 font-body text-xs text-chirp-error">{downloadError}</p>
          )}
        </SettingGroup>
      )}

      {/* Model info when downloaded */}
      {store.aiCleanup && llmDownloaded && (
        <SettingGroup label="Model">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Check size={16} className="text-chirp-success" />
              <span className="font-body text-sm text-chirp-stone-700">{LLM_MODEL.name}</span>
              <span className="font-body text-xs text-chirp-stone-500">{LLM_MODEL.size}</span>
            </div>
            <Button variant="ghost" onClick={handleDownload}>
              Re-download
            </Button>
          </div>
        </SettingGroup>
      )}

      {/* Test area */}
      {serverRunning && (
        <SettingGroup label="Try it">
          <div className="flex flex-col gap-3">
            <textarea
              value={testInput}
              onChange={(e) => setTestInput(e.target.value)}
              placeholder="Paste a transcription to test cleanup..."
              className="h-20 w-full resize-none rounded-lg border border-chirp-stone-200 bg-chirp-stone-50 px-3 py-2 font-body text-sm text-chirp-stone-900 placeholder:text-chirp-stone-400 focus:border-chirp-amber-400 focus:outline-none"
            />
            <Button onClick={handleTest} disabled={testing || !testInput.trim()} className="self-start">
              <Sparkles size={14} className="mr-1.5" />
              {testing ? 'Cleaning up...' : 'Clean up'}
            </Button>
            {testOutput !== null && (
              <div className="rounded-lg border border-chirp-stone-200 bg-white p-3">
                <span className="font-body text-[10px] font-semibold uppercase tracking-[0.5px] text-chirp-stone-400">
                  Result
                </span>
                <p className="font-body text-sm text-chirp-stone-900 mt-1 whitespace-pre-wrap">
                  {testOutput}
                </p>
              </div>
            )}
          </div>
        </SettingGroup>
      )}
    </div>
  )
}
