interface KeyBadgeProps {
  keyLabel: string
  variant?: 'default' | 'glass'
}

export function KeyBadge({ keyLabel, variant = 'default' }: KeyBadgeProps) {
  const styles = variant === 'glass'
    ? 'inline-flex min-w-[28px] items-center justify-center rounded-[5px] border border-white/10 bg-white/[0.08] px-2 py-1 font-mono text-[11px] font-medium text-white/60 shadow-[0_1px_2px_rgba(0,0,0,0.2)]'
    : 'inline-flex min-w-[28px] items-center justify-center rounded-md border border-card-border bg-[#F5F4F0] px-2 py-0.5 font-mono text-xs font-medium text-[#555] shadow-[0_1px_2px_rgba(0,0,0,0.04)]'
  return (
    <span className={styles}>
      {keyLabel}
    </span>
  )
}
