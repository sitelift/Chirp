import { Waveform } from '../shared/Waveform'
import { useAppStore } from '../../stores/appStore'

export function Listening() {
  const amplitudes = useAppStore((s) => s.amplitudes)

  return <Waveform amplitudes={amplitudes} />
}
