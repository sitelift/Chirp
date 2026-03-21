import type { ReactNode } from 'react'

interface ContextCardProps {
  icon: ReactNode
  title: string
  description: string
  actions?: { label: string; onClick: () => void; variant?: 'primary' | 'ghost' }[]
  variant?: 'suggestion' | 'default'
}

export function ContextCard({ icon, title, description, actions, variant = 'default' }: ContextCardProps) {
  return (
    <div
      className={`flex-1 rounded-card border p-4 hover-lift cursor-default ${
        variant === 'suggestion'
          ? 'bg-gradient-to-br from-[#FFFDF5] to-[#FFF8E5] border-[#F0DFA0]'
          : 'bg-white border-card-border'
      }`}
    >
      <div className="flex items-center gap-[10px] mb-2">
        <div className={`w-8 h-8 rounded-[9px] flex items-center justify-center text-base ${
          variant === 'suggestion' ? 'bg-chirp-amber-100' : 'bg-[#F0FFF0]'
        }`}>
          {icon}
        </div>
        <span className="text-[13px] font-semibold text-[#1a1a1a]">{title}</span>
      </div>
      <p className="text-xs text-[#888] leading-relaxed mb-3">{description}</p>
      {actions && actions.length > 0 && (
        <div className="flex gap-2">
          {actions.map((action) => (
            <button
              key={action.label}
              onClick={action.onClick}
              className={`px-[14px] py-[6px] rounded-[7px] text-xs font-medium transition-all duration-150 ${
                action.variant === 'ghost'
                  ? 'text-[#888] hover:text-[#555]'
                  : 'bg-[#1a1a1a] text-white hover:bg-[#333]'
              }`}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
