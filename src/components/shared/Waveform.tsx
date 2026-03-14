interface WaveformProps {
  amplitudes: number[]
}

export function Waveform({ amplitudes }: WaveformProps) {
  const barCount = amplitudes.length || 48

  return (
    <div className="flex h-10 w-full items-end justify-center gap-[3px]">
      {Array.from({ length: barCount }, (_, i) => {
        const amp = amplitudes[i] ?? 0.08
        const height = Math.max(3, amp * 36)
        return (
          <div
            key={i}
            className="w-[3px] rounded-full bg-chirp-amber-400"
            style={{
              height: `${height}px`,
              transition: 'height 80ms linear',
            }}
          />
        )
      })}
    </div>
  )
}
