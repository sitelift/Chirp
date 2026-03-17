import type { ReactNode } from 'react'

interface SettingGroupProps {
  label: string
  children: ReactNode
}

export function SettingGroup({ label, children }: SettingGroupProps) {
  return (
    <div className="relative rounded-xl border border-chirp-stone-200 bg-white p-5 shadow-card hover:shadow-card-hover transition-shadow duration-200">
      <span className="absolute -top-2.5 left-4 bg-white px-2 font-body text-xs font-semibold uppercase tracking-[0.5px] text-chirp-stone-500">
        {label}
      </span>
      <div className="flex flex-col gap-4">{children}</div>
    </div>
  )
}
