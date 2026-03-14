import { BirdMark } from '../shared/BirdMark'
import { Waveform } from '../shared/Waveform'
import { useAppStore } from '../../stores/appStore'

export function Listening() {
  const amplitudes = useAppStore((s) => s.amplitudes)
  const hotkey = useAppStore((s) => s.hotkey)

  const hotkeyDisplay = hotkey
    .replace('CmdOrCtrl', navigator.platform.includes('Mac') ? '\u2318' : 'Ctrl')
    .replace('Shift', navigator.platform.includes('Mac') ? '\u21E7' : 'Shift')
    .replace(/\+/g, '')

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <BirdMark size={16} />
        <div className="relative h-1.5 w-1.5">
          <div className="absolute inset-0 rounded-full bg-chirp-success animate-pulse" />
          <div className="h-1.5 w-1.5 rounded-full bg-chirp-success" />
        </div>
        <span className="font-body font-medium text-sm text-chirp-stone-900">
          Listening...
        </span>
      </div>

      <Waveform amplitudes={amplitudes} />

      <p className="font-body text-[11px] text-chirp-stone-500 text-center">
        {hotkeyDisplay} to stop · Esc to cancel
      </p>
    </div>
  )
}
