import { Waveform } from '../shared/Waveform'
import { useAppStore } from '../../stores/appStore'

export function Listening() {
  const amplitudes = useAppStore((s) => s.amplitudes)

  return (
    <div className="flex items-center h-4 overflow-hidden">
      <Waveform amplitudes={amplitudes} />
    </div>
  )
}
