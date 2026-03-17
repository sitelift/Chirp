import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        chirp: {
          yellow: '#F0B723',
          amber: {
            50: '#FFFBEB',
            100: '#FEF3C7',
            200: '#FDE68A',
            300: '#FCD34D',
            400: '#FBBF24',
            500: '#F59E0B',
            600: '#D97706',
          },
          stone: {
            50: '#FAFAF9',
            100: '#F5F5F4',
            200: '#E7E5E4',
            300: '#D6D3D1',
            400: '#A8A29E',
            500: '#78716C',
            600: '#57534E',
            700: '#44403C',
            800: '#292524',
            900: '#1C1917',
          },
          white: '#FFFFFF',
          success: '#16A34A',
          error: '#DC2626',
          info: '#2563EB',
        },
      },
      fontFamily: {
        display: ['Nunito', 'sans-serif'],
        body: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      boxShadow: {
        subtle: '0 1px 2px rgba(0,0,0,0.05)',
        card: '0 1px 3px rgba(0,0,0,0.08)',
        'card-hover': '0 4px 16px rgba(0,0,0,0.10)',
        stat: '0 1px 4px rgba(245,158,11,0.12)',
        elevated: '0 4px 12px rgba(0,0,0,0.12)',
        overlay: '0 2px 12px rgba(0,0,0,0.08)',
      },
      keyframes: {
        overlayIn: {
          from: { opacity: '0', transform: 'translateY(8px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
        overlayOut: {
          from: { opacity: '1', transform: 'translateY(0)' },
          to: { opacity: '0', transform: 'translateY(4px)' },
        },
        shimmer: {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(100%)' },
        },
        fadeIn: {
          from: { opacity: '0', transform: 'translateY(4px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
        'pulse-gentle': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
      },
      animation: {
        'overlay-in': 'overlayIn 120ms cubic-bezier(0.16, 1, 0.3, 1)',
        'overlay-out': 'overlayOut 150ms ease-in',
        shimmer: 'shimmer 1.5s ease-in-out infinite',
        'fade-in': 'fadeIn 200ms ease-out',
        'pulse-gentle': 'pulse-gentle 2s ease-in-out infinite',
      },
    },
  },
  plugins: [],
} satisfies Config
