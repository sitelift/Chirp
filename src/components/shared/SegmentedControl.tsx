interface SegmentedControlProps {
  options: { value: string; label: string }[]
  value: string
  onChange: (value: string) => void
}

export function SegmentedControl({ options, value, onChange }: SegmentedControlProps) {
  return (
    <div className="flex rounded-lg border border-card-border bg-[#F5F4F0] p-[3px]">
      {options.map((option) => (
        <button
          key={option.value}
          onClick={() => onChange(option.value)}
          className={`px-4 py-[6px] rounded-md text-xs font-medium transition-all duration-200 ${
            option.value === value
              ? 'bg-white text-[#1a1a1a] shadow-[0_1px_3px_rgba(0,0,0,0.06)]'
              : 'text-[#888] hover:text-[#555]'
          }`}
          style={{ transitionTimingFunction: option.value === value ? 'cubic-bezier(0.34, 1.56, 0.64, 1)' : undefined }}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}
