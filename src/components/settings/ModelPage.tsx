import { useState } from 'react'
import { Check, Download } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useTauri } from '../../hooks/useTauri'
import { STT_MODELS } from '../../lib/constants'
import { SettingGroup } from './SettingGroup'

export function ModelPage() {
  const store = useAppStore()
  const tauri = useTauri()
  const [downloadError, setDownloadError] = useState<string | null>(null)

  const currentModel = STT_MODELS.find((m) => m.id === store.model)

  const handleModelSelect = async (modelId: typeof store.model) => {
    store.updateSettings({ model: modelId })
    setDownloadError(null)

    if (!store.modelDownloaded[modelId]) {
      store.setModelDownloadProgress(0)
      try {
        await tauri.downloadModel(modelId, (progress) => {
          store.setModelDownloadProgress(progress)
        })
        store.updateSettings({
          modelDownloaded: { ...store.modelDownloaded, [modelId]: true },
        })
      } catch {
        setDownloadError('Download failed. Check your internet connection and try again.')
      } finally {
        store.setModelDownloadProgress(null)
      }
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <SettingGroup label="Speech Model">
        <div className="mb-2">
          <p className="font-body text-sm text-chirp-stone-700">
            Current: {currentModel?.name}
          </p>
          <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
            Size: {currentModel?.size}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          {STT_MODELS.map((model) => {
            const isSelected = store.model === model.id
            const isDownloaded = store.modelDownloaded[model.id]
            return (
              <button
                key={model.id}
                onClick={() => handleModelSelect(model.id)}
                className={`flex items-center gap-3 rounded-lg border p-3 text-left transition-colors duration-150 ease-out ${
                  isSelected
                    ? 'border-chirp-amber-400 bg-chirp-amber-50'
                    : 'border-chirp-stone-200 hover:bg-chirp-stone-100'
                }`}
              >
                <div
                  className={`flex h-5 w-5 items-center justify-center rounded-full border-2 ${
                    isSelected
                      ? 'border-chirp-amber-400'
                      : 'border-chirp-stone-300'
                  }`}
                >
                  {isSelected && (
                    <div className="h-2.5 w-2.5 rounded-full bg-chirp-amber-400" />
                  )}
                </div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-body text-sm font-medium text-chirp-stone-900">
                      {model.name}
                    </span>
                    <span className="font-body text-xs text-chirp-stone-500">
                      — {model.size}
                    </span>
                    {model.recommended && (
                      <span className="rounded-md bg-chirp-amber-100 px-1.5 py-0.5 font-body text-[11px] font-medium text-chirp-amber-500">
                        Recommended
                      </span>
                    )}
                  </div>
                  <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
                    {model.description}
                  </p>
                </div>
                {isDownloaded ? (
                  <Check size={16} className="text-chirp-success" />
                ) : (
                  <Download size={16} className="text-chirp-stone-400" />
                )}
              </button>
            )
          })}
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
      </SettingGroup>

      <SettingGroup label="Text Cleanup">
        <div>
          <p className="font-body text-sm text-chirp-stone-700">
            Model: Chirp Cleanup v1
          </p>
          <p className="font-body text-xs text-chirp-stone-500 mt-0.5">
            Size: 38 MB · Bundled
          </p>
        </div>
        <p className="font-body text-[13px] text-chirp-stone-500 leading-relaxed">
          This model runs locally to format your transcripts. It handles punctuation,
          lists, paragraphs, and filler word removal.
        </p>
      </SettingGroup>
    </div>
  )
}
