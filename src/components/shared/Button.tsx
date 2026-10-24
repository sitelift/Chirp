import { type ButtonHTMLAttributes, type ReactNode } from 'react'

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'icon'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: 'app' | 'onboarding'
  children: ReactNode
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    'bg-dm-btn-bg text-dm-btn-text font-display font-bold text-sm hover:bg-dm-btn-hover active:bg-[#000] disabled:bg-dm-btn-disabled-bg disabled:text-dm-btn-disabled-text disabled:cursor-not-allowed',
  secondary:
    'bg-card border border-card-border text-dm-primary font-body font-medium text-sm hover:bg-card-hover active:bg-card-hover',
  ghost:
    'bg-transparent text-dm-secondary font-body font-medium text-[13px] hover:text-dm-primary',
  icon:
    'w-8 h-8 bg-transparent hover:bg-card-hover flex items-center justify-center text-dm-secondary hover:text-dm-primary',
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
