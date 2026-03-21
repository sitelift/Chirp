import { useState, useRef, useEffect, useCallback } from 'react'
import { createPortal } from 'react-dom'
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
  const buttonRef = useRef<HTMLButtonElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState({ top: 0, left: 0, width: 0 })

  const updatePosition = useCallback(() => {
    if (!buttonRef.current) return
    const rect = buttonRef.current.getBoundingClientRect()
    setPos({
      top: rect.bottom + 4,
      left: rect.left,
      width: rect.width,
    })
  }, [])

  useEffect(() => {
    if (!open) return
    updatePosition()

    const handleClick = (e: MouseEvent) => {
      if (
        buttonRef.current?.contains(e.target as Node) ||
        dropdownRef.current?.contains(e.target as Node)
      ) return
      setOpen(false)
    }
    const handleScroll = () => updatePosition()

    document.addEventListener('mousedown', handleClick)
    document.addEventListener('scroll', handleScroll, true)
    return () => {
      document.removeEventListener('mousedown', handleClick)
      document.removeEventListener('scroll', handleScroll, true)
    }
  }, [open, updatePosition])

  const selected = options.find((o) => o.value === value)

  return (
    <>
      <button
        ref={buttonRef}
        onClick={() => setOpen(!open)}
        className="flex h-10 w-full items-center justify-between rounded-lg border border-card-border bg-white px-3 font-body text-sm text-chirp-stone-700 transition-colors duration-150 ease-out hover:border-chirp-stone-300 focus:border-chirp-yellow focus:border-2 focus:outline-none"
      >
        <span className={selected ? '' : 'text-chirp-stone-500 italic'}>
          {selected?.label ?? placeholder ?? 'Select...'}
        </span>
        <ChevronDown size={16} className="text-chirp-stone-500 ml-2" />
      </button>

      {open && createPortal(
        <div
          ref={dropdownRef}
          className="fixed rounded-xl bg-white p-1 shadow-elevated border border-card-border"
          style={{
            top: pos.top,
            left: pos.left,
            width: pos.width,
            zIndex: 9999,
          }}
        >
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
        </div>,
        document.body
      )}
    </>
  )
}
