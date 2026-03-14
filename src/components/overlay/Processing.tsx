import { useState, useEffect } from 'react'
import { BirdMark } from '../shared/BirdMark'

export function Processing() {
  const [showStillWorking, setShowStillWorking] = useState(false)

  useEffect(() => {
    const timer = setTimeout(() => setShowStillWorking(true), 5000)
    return () => clearTimeout(timer)
  }, [])

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <BirdMark size={16} />
        <span className="font-body font-medium text-sm text-chirp-stone-900">
          Processing...
        </span>
      </div>

      {/* Shimmer progress bar */}
      <div className="relative h-1 w-full overflow-hidden rounded-full bg-chirp-amber-100">
        <div className="absolute inset-0 animate-shimmer">
          <div className="h-full w-1/2 rounded-full bg-chirp-amber-400" />
        </div>
      </div>

      {showStillWorking && (
        <p className="font-body text-[11px] text-chirp-stone-500 text-center">
          Still working...
        </p>
      )}
    </div>
  )
}
