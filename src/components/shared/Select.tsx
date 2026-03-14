import { useState, useRef, useEffect } from 'react'
import { ChevronDown } from 'lucide-react'

interface SelectOption {
  value: string | number
  label: string
}

interface SelectProps {
  options: SelectOption[]
  value: string | number
  onChange: (value: string | number) => void
  placeholder?: string
}

export function Select({ options, value, onChange, placeholder }: SelectProps) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  const selected = options.find((o) => o.value === value)

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex h-10 w-full items-center justify-between rounded-lg border border-chirp-stone-200 bg-white px-3 font-body text-sm text-chirp-stone-700 transition-colors duration-150 ease-out hover:border-chirp-stone-300 focus:border-chirp-amber-400 focus:border-2 focus:outline-none"
      >
        <span className={selected ? '' : 'text-chirp-stone-500 italic'}>
          {selected?.label ?? placeholder ?? 'Select...'}
        </span>
        <ChevronDown size={16} className="text-chirp-stone-500 ml-2" />
      </button>

      {open && (
        <div className="absolute z-50 mt-1 w-full rounded-xl bg-white p-1 shadow-elevated border border-chirp-stone-200">
          {options.map((option) => (
            <button
              key={option.value}
              onClick={() => {
                onChange(option.value)
                setOpen(false)
              }}
              className={`flex w-full items-center h-9 px-3 rounded-lg text-sm font-body transition-colors duration-150 ease-out ${
                option.value === value
                  ? 'bg-chirp-stone-100 text-chirp-stone-900'
                  : 'text-chirp-stone-700 hover:bg-chirp-stone-100'
              }`}
            >
              {option.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
