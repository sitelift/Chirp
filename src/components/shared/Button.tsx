import { type ButtonHTMLAttributes, type ReactNode } from 'react'

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'icon'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: 'app' | 'onboarding'
  children: ReactNode
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    'bg-[#1a1a1a] text-white font-display font-bold text-sm hover:bg-[#333] active:bg-[#000] disabled:bg-[#ccc] disabled:text-[#888] disabled:cursor-not-allowed',
  secondary:
    'bg-chirp-white border border-chirp-stone-200 text-chirp-stone-700 font-body font-medium text-sm hover:bg-chirp-stone-100 active:bg-chirp-stone-200',
  ghost:
    'bg-transparent text-chirp-stone-500 font-body font-medium text-[13px] hover:text-chirp-stone-700',
  icon:
    'w-8 h-8 bg-transparent hover:bg-chirp-stone-100 flex items-center justify-center text-chirp-stone-500 hover:text-chirp-stone-700',
}

export function Button({
  variant = 'primary',
  size = 'app',
  className = '',
  children,
  ...props
}: ButtonProps) {
  const heightClass =
    variant === 'ghost' || variant === 'icon'
      ? ''
      : size === 'onboarding'
        ? 'h-11'
        : 'h-9'

  const paddingClass =
    variant === 'ghost'
      ? ''
      : variant === 'icon'
        ? ''
        : size === 'onboarding'
          ? 'px-6'
          : 'px-4'

  return (
    <button
      className={`inline-flex items-center justify-center rounded-lg transition-colors duration-150 ease-out ${heightClass} ${paddingClass} ${variantStyles[variant]} ${className}`}
      {...props}
    >
      {children}
    </button>
  )
}
