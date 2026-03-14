interface KeyBadgeProps {
  keyLabel: string
}

export function KeyBadge({ keyLabel }: KeyBadgeProps) {
  return (
    <span className="inline-flex min-w-[28px] items-center justify-center rounded-md border border-chirp-stone-200 bg-white px-2 py-0.5 font-mono text-xs font-medium text-chirp-stone-700 shadow-[0_1px_2px_rgba(0,0,0,0.06)]">
      {keyLabel}
    </span>
  )
}
