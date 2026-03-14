interface ToggleProps {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
}

export function Toggle({ checked, onChange, disabled }: ToggleProps) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-[22px] w-10 shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-out ${
        checked ? 'bg-chirp-amber-400' : 'bg-chirp-stone-300'
      } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
    >
      <span
        className={`pointer-events-none inline-block h-[18px] w-[18px] rounded-full bg-white shadow-subtle transition-transform duration-200 ease-out ${
          checked ? 'translate-x-[20px]' : 'translate-x-[2px]'
        } mt-[2px]`}
      />
    </button>
  )
}
