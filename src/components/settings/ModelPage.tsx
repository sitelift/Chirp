import { useState } from 'react'
import { Check, Download } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { STT_MODELS, LLM_MODEL } from '../../lib/constants'
import { SettingGroup } from './SettingGroup'
import { Button } from '../shared/Button'

export function ModelPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [llmDownloadError, setLlmDownloadError] = useState<string | null>(null)
  const [llmDownloaded, setLlmDownloaded] = useLlmDownloaded()

  const currentModel = STT_MODELS.find((m) => m.id === store.model)
  const isDownloaded = store.modelDownloaded[store.model]

  const handleLlmDownload = async () => {
    setLlmDownloadError(null)
    store.setLlmDownloadProgress(0)
    try {
      await tauri.downloadLlm((progress) => {
        store.setLlmDownloadProgress(progress)
      })
      setLlmDownloaded(true)

      // Auto-start server if cleanup is enabled
      if (store.aiCleanup) {
        try {
          await tauri.startLlm()
          store.setLlmReady(true)
        } catch (e) { console.debug('Failed to auto-start LLM:', e) }
      }
    } catch {
      setLlmDownloadError('Download failed. Check your internet connection and try again.')
    } finally {
      store.setLlmDownloadProgress(null)
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

  return (
    <div className="flex flex-col gap-6">
      <div className="mb-2">
        <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">Speech Model</h1>
        <p className="font-body text-sm text-chirp-stone-500 mt-1">Manage the AI model used for transcription.</p>
      </div>

      <SettingGroup label="Speech Model">
        <div className="flex items-center gap-3 rounded-xl border border-chirp-stone-200 bg-white p-4">
          <div className="flex-1">
            <div className="flex items-center gap-2">
              <span className="font-body text-sm font-medium text-chirp-stone-900">
                {currentModel?.name}
              </span>
              <span className="font-body text-xs text-chirp-stone-500">
                {currentModel?.size}
              </span>
            </div>
            <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
              {currentModel?.description}
            </p>
          </div>
          {isDownloaded ? (
            <div className="flex items-center gap-1.5">
              <Check size={16} className="text-chirp-success" />
              <span className="font-body text-sm text-chirp-stone-700">Ready</span>
            </div>
          ) : (
            <Button onClick={handleDownload} disabled={store.modelDownloadProgress !== null}>
              <Download size={14} className="mr-1.5" />
              Download
            </Button>
          )}
        </div>

        {store.modelDownloadProgress !== null && (
          <div className="mt-2">
            <div className="flex items-center gap-2">
              <div className="h-1 flex-1 overflow-hidden rounded-full bg-chirp-stone-200">
                <div
                  className="h-full rounded-full bg-chirp-amber-400 transition-all duration-200"
                  style={{ width: `${store.modelDownloadProgress}%` }}
                />
              </div>
              <span className="font-body text-xs text-chirp-stone-500">
                {store.modelDownloadProgress}%
              </span>
            </div>
            <p className="font-body text-xs text-chirp-stone-500 mt-1">
              {store.modelDownloadProgress < 96
                ? `Downloading ${currentModel?.name}...`
                : 'Extracting model...'}
            </p>
          </div>
        )}

        {downloadError && (
          <p className="mt-2 font-body text-xs text-chirp-error">{downloadError}</p>
        )}

        {isDownloaded && (
          <Button variant="ghost" onClick={handleDownload} className="self-start mt-1">
            Re-download model
          </Button>
        )}
      </SettingGroup>

      <SettingGroup label="Smart Cleanup Engine">
        <div className="flex items-center gap-3 rounded-xl border border-chirp-stone-200 bg-white p-4">
          <div className="flex-1">
            <div className="flex items-center gap-2">
              <span className="font-body text-sm font-medium text-chirp-stone-900">
                {LLM_MODEL.displayName} engine
              </span>
              <span className="font-body text-xs text-chirp-stone-500">
                {LLM_MODEL.friendlySize}
              </span>
            </div>
            <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
              Polishes grammar and sentences locally
            </p>
          </div>
          {llmDownloaded ? (
            <div className="flex items-center gap-1.5">
              <Check size={16} className="text-chirp-success" />
              <span className="font-body text-sm text-chirp-stone-700">Ready</span>
            </div>
          ) : (
            <Button onClick={handleLlmDownload} disabled={store.llmDownloadProgress !== null}>
              <Download size={14} className="mr-1.5" />
              Download
            </Button>
          )}
        </div>

        {store.llmDownloadProgress !== null && (
          <div className="mt-2">
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
            <p className="font-body text-xs text-chirp-stone-500 mt-1">
              {store.llmDownloadProgress < 96
                ? 'Downloading Smart Cleanup engine...'
                : 'Extracting model...'}
            </p>
          </div>
        )}

        {llmDownloadError && (
          <p className="mt-2 font-body text-xs text-chirp-error">{llmDownloadError}</p>
        )}

        {llmDownloaded && (
          <Button variant="ghost" onClick={handleLlmDownload} className="self-start mt-1">
            Re-download model
          </Button>
        )}
      </SettingGroup>
    </div>
  )
}
