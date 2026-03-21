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
        sidebar: '#1a1917',
        surface: '#F5F4F0',
        'card-border': '#EDECE8',
      },
      fontFamily: {
        display: ['Nunito', 'sans-serif'],
        body: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      borderRadius: {
        card: '14px',
      },
      boxShadow: {
        subtle: '0 1px 2px rgba(0,0,0,0.05)',
        card: '0 1px 3px rgba(0,0,0,0.04)',
        'card-hover': '0 8px 24px rgba(0,0,0,0.08)',
        'nav-active': '0 1px 3px rgba(0,0,0,0.06), 0 0 0 1px rgba(0,0,0,0.04)',
        stat: '0 1px 4px rgba(245,158,11,0.12)',
        elevated: '0 4px 12px rgba(0,0,0,0.12)',
        overlay: '0 2px 12px rgba(0,0,0,0.08)',
        'logo-glow': '0 0 20px rgba(240,183,35,0.3)',
        'status-glow': '0 0 8px rgba(34,197,94,0.5)',
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
        slideUp: {
          from: { opacity: '0', transform: 'translateY(12px)' },
          to: { opacity: '1', transform: 'translateY(0)' },
        },
        countUp: {
          from: { opacity: '0', transform: 'translateY(20px)', filter: 'blur(4px)' },
          to: { opacity: '1', transform: 'translateY(0)', filter: 'blur(0)' },
        },
        barGrow: {
          from: { transform: 'scaleY(0)' },
          to: { transform: 'scaleY(1)' },
        },
        glowPulse: {
          '0%, 100%': { boxShadow: '0 0 8px rgba(34,197,94,0.5)' },
          '50%': { boxShadow: '0 0 16px rgba(34,197,94,0.8)' },
        },
        float1: {
          '0%, 100%': { transform: 'translate(0, 0)' },
          '50%': { transform: 'translate(-20px, 15px)' },
        },
        float2: {
          '0%, 100%': { transform: 'translate(0, 0)' },
          '50%': { transform: 'translate(15px, -10px)' },
        },
        savedPop: {
          from: { opacity: '0', transform: 'translateY(10px) scale(0.95)' },
          to: { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
      },
      animation: {
        'overlay-in': 'overlayIn 120ms cubic-bezier(0.16, 1, 0.3, 1)',
        'overlay-out': 'overlayOut 150ms ease-in',
        shimmer: 'shimmer 1.5s ease-in-out infinite',
        'fade-in': 'fadeIn 200ms ease-out',
        'pulse-gentle': 'pulse-gentle 2s ease-in-out infinite',
        'slide-up': 'slideUp 400ms ease-out both',
        'count-up': 'countUp 600ms cubic-bezier(0.34, 1.56, 0.64, 1) both',
        'bar-grow': 'barGrow 600ms cubic-bezier(0.34, 1.56, 0.64, 1) both',
        'glow-pulse': 'glowPulse 2s ease-in-out infinite',
        'float-1': 'float1 6s ease-in-out infinite',
        'float-2': 'float2 8s ease-in-out infinite',
        'saved-pop': 'savedPop 300ms cubic-bezier(0.34, 1.56, 0.64, 1)',
      },
    },
  },
  plugins: [],
} satisfies Config
