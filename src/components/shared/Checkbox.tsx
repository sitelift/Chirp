import { Check } from 'lucide-react'

interface CheckboxProps {
  checked: boolean
  onChange: (checked: boolean) => void
  label: string
  description?: string
}

export function Checkbox({ checked, onChange, label, description }: CheckboxProps) {
  return (
    <label className="flex items-start gap-3 cursor-pointer group">
      <button
        role="checkbox"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`mt-0.5 flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-md border transition-all duration-150 ease-out focus:ring-2 focus:ring-chirp-amber-400 focus:ring-offset-2 ${
          checked
            ? 'bg-chirp-amber-400 border-chirp-amber-400'
            : 'bg-white border-chirp-stone-300 group-hover:border-chirp-stone-200'
        }`}
      >
        {checked && <Check size={12} strokeWidth={2} className="text-white" />}
      </button>
      <div className="flex flex-col">
        <span className="font-body text-sm text-chirp-stone-700 leading-snug">{label}</span>
        {description && (
          <span className="font-body text-[13px] text-chirp-stone-500 leading-snug mt-0.5">
            {description}
          </span>
        )}
      </div>
    </label>
  )
}
