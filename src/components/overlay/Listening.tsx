import { Waveform } from '../shared/Waveform'
import { useAppStore } from '../../stores/appStore'

export function Listening() {
  const amplitudes = useAppStore((s) => s.amplitudes)
  const liveText = useAppStore((s) => s.liveTranscription)

  return (
    <div className="flex items-center gap-3">
      <Waveform amplitudes={amplitudes} />
      {liveText && (
        <span className="font-body text-sm text-chirp-stone-500 max-w-[200px] truncate">
          {liveText}
        </span>
      )}
    </div>
  )
}
