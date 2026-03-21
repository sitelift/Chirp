import { useAppStore } from '../../stores/appStore'
import { useHotkeyRecorder } from '../../hooks/useHotkeyRecorder'
import { formatHotkey } from '../../lib/utils'
import { KeyBadge } from '../shared/KeyBadge'
import { Button } from '../shared/Button'

interface SetupStepProps {
  onNext: () => void
}

export function SetupStep({ onNext }: SetupStepProps) {
  const store = useAppStore()
  const { capturing, pendingHotkey, previewLabels, canConfirm, startCapture, confirmCapture, cancelCapture, clearPending } = useHotkeyRecorder()

  const confirmed = Boolean(store.hotkey)
  const currentHotkeyLabels = formatHotkey(store.hotkey)

  const handleRecord = () => {
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

  return (
    <div className="flex flex-col animate-fade-in">
      <h1 className="font-display font-extrabold text-2xl text-chirp-stone-900">
        Set your hotkey
      </h1>
      <p className="mt-1 font-body text-sm text-chirp-stone-500">
        Choose a shortcut to start and stop dictation. Hold it to talk, release to transcribe.
      </p>

      {/* Capture zone */}
      <div
        className={`mt-6 flex h-28 w-full items-center justify-center rounded-xl transition-all duration-200 ${
          capturing
            ? 'border-2 border-dashed border-chirp-amber-400 bg-chirp-amber-50/30 shadow-[0_0_0_4px_rgba(240,183,35,0.1)]'
            : pendingHotkey
              ? 'border-2 border-solid border-chirp-amber-400 bg-chirp-amber-50/30'
              : confirmed
                ? 'border border-solid border-card-border bg-white'
                : 'border-2 border-dashed border-chirp-stone-300 bg-chirp-stone-50'
        }`}
      >
        {capturing ? (
          previewLabels.length > 0 ? (
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
            <span className="font-body text-sm text-chirp-stone-400 animate-pulse">
              Press keys one at a time...
            </span>
          )
        ) : pendingHotkey ? (
          <div className="flex items-center gap-3">
            {pendingHotkey.labels.map((label, i) => (
              <div key={label} className="flex items-center gap-3">
                {i > 0 && <span className="text-[13px] text-chirp-stone-300 font-medium select-none">+</span>}
                <KeyBadge keyLabel={label} />
              </div>
            ))}
          </div>
        ) : confirmed ? (
          <div className="flex items-center gap-3">
            {currentHotkeyLabels.map((label, i) => (
              <div key={label} className="flex items-center gap-3">
                {i > 0 && <span className="text-[13px] text-chirp-stone-300 font-medium select-none">+</span>}
                <KeyBadge keyLabel={label} />
              </div>
            ))}
          </div>
        ) : (
          <span className="font-body text-sm text-chirp-stone-400">No hotkey set</span>
        )}
      </div>

      {/* Actions */}
      <div className="mt-4 flex items-center gap-3">
        {capturing ? (
          <>
            {canConfirm && (
              <Button
                size="onboarding"
                className="min-w-[120px] text-base"
                onClick={handleConfirmCapture}
              >
                Set hotkey
              </Button>
            )}
            <button
              onClick={cancelCapture}
              className="font-body text-xs text-chirp-stone-400 hover:text-chirp-stone-600 hover:underline transition-colors"
            >
              Cancel
            </button>
          </>
        ) : pendingHotkey ? (
          <>
            <Button
              size="onboarding"
              className="min-w-[120px] text-base"
              onClick={handleConfirmPending}
            >
              Set hotkey
            </Button>
            <button
              onClick={cancelCapture}
              className="font-body text-xs text-chirp-stone-400 hover:text-chirp-stone-600 hover:underline transition-colors"
            >
              Cancel
            </button>
          </>
        ) : (
          <div className="flex items-center gap-3">
            <Button
              size="onboarding"
              className="min-w-[140px] text-base"
              onClick={handleRecord}
            >
              {confirmed ? 'Change shortcut' : 'Set hotkey'}
            </Button>
            {confirmed && (
              <Button
                size="onboarding"
                className="min-w-[140px] text-base"
                onClick={onNext}
              >
                Continue
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
