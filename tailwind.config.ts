import type { Config } from 'tailwindcss'

/**
 * Design tokens from docs/v2/design/motion.html. Themed colours are CSS
 * variables set per `[data-theme]` in globals.css, the ink pill included.
 */
const v = (name: string) => `var(--${name})`

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        ink: v('ink'),
        'ink-text': v('ink-text'),
        'ink-muted': v('ink-muted'),
        'ink-accent': v('ink-accent'),
        'ink-btn': v('ink-btn'),
        'ink-btn-hover': v('ink-btn-hover'),
        surface: v('surface'),
        t1: v('t1'),
        t2: v('t2'),
        t3: v('t3'),
        t4: v('t4'),
        chev: v('chev'),
        divider: v('divider'),
        hover: v('hover'),
        press: v('press'),
        selected: v('selected'),
        accent: v('accent'),
        'accent-text': v('accent-text'),
        'accent-soft': v('accent-soft'),
        danger: v('danger'),
        'danger-soft': v('danger-soft'),
        field: v('field'),
        seg: v('seg'),
        'level-idle': v('level-idle'),
        track: v('track'),
        fill: v('fill'),
        btn: v('btn'),
        'btn-text': v('btn-text'),
      },
      fontFamily: {
        ui: ['"Segoe UI Variable Text"', '"Segoe UI Variable"', '"Segoe UI"', 'system-ui', 'sans-serif'],
        display: ['"Segoe UI Variable Display"', '"Segoe UI Variable"', '"Segoe UI"', 'system-ui', 'sans-serif'],
      },
      fontSize: {
        meta: ['12px', '16px'],
        row: ['13px', '18px'],
        body: ['15px', '22px'],
        title: ['17px', '24px'],
      },
      borderRadius: {
        key: v('r-key'),
        btn: v('r-btn'),
        row: v('r-row'),
        card: v('r-card'),
      },
      boxShadow: {
        pill: '0 6px 20px var(--ink-shadow), 0 0 0 1px var(--ink-ring)',
      },
      transitionTimingFunction: {
        out: v('ease'),
        in: v('ease-in'),
      },
    },
  },
  plugins: [],
} satisfies Config
