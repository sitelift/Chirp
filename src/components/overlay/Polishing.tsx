export function Polishing() {
  return (
    <div className="flex items-center gap-2">
      <div className="relative h-[3px] w-16 overflow-hidden rounded-full bg-chirp-amber-100">
        <div className="absolute inset-0 animate-shimmer">
          <div className="h-full w-1/2 rounded-full bg-chirp-amber-400" />
        </div>
      </div>
    </div>
  )
}
