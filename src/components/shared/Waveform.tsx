interface WaveformProps {
  amplitudes: number[]
}

const DISPLAY_BARS = 16

export function Waveform({ amplitudes }: WaveformProps) {
  const sourceLen = amplitudes.length || 48
  const bars = Array.from({ length: DISPLAY_BARS }, (_, i) => {
    const srcIdx = Math.round((i / DISPLAY_BARS) * sourceLen)
    return amplitudes[srcIdx] ?? 0.06
  })

  return (
    <div className="flex h-6 items-center gap-[1.5px]">
      {bars.map((amp, i) => {
        const height = Math.max(3, Math.sqrt(amp) * 24)
        return (
          <div
            key={i}
            className="w-[2px] rounded-full bg-chirp-amber-400"
            style={{
              height: `${height}px`,
              transition: 'height 50ms ease-out',
            }}
          />
        )
      })}
    </div>
  )
}
