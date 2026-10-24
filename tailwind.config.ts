import type { Config } from 'tailwindcss'

/**
 * Design tokens from the Paper file "Chirp v2". Themed colors are CSS
 * variables set per `[data-theme]` in globals.css; the ink pill is the same in
 * both themes.
 */
const v = (name: string) => `var(--${name})`

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        ink: '#1A1917',
        amber: '#F0B723',
        surface: v('surface'),
        t1: v('t1'),
        t2: v('t2'),
        t3: v('t3'),
        t4: v('t4'),
        chev: v('chev'),
        arrow: v('arrow'),
        divider: v('divider'),
        hover: v('hover'),
        press: v('press'),
        accent: v('accent'),
        'accent-text': v('accent-text'),
        'accent-soft': v('accent-soft'),
        danger: v('danger'),
        'danger-soft': v('danger-soft'),
        field: v('field'),
        'field-ro': v('field-ro'),
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
        key: '6px',
        field: '7px',
        btn: '8px',
        row: '10px',
        surface: '18px',
      },
      boxShadow: {
        surface: v('surface-shadow'),
        pill: '0 6px 20px #00000073, 0 0 0 1px #FFFFFF1F',
        latched: '0 6px 20px #00000059, 0 0 0 2px #F0B723, 0 0 0 6px #F0B72338',
      },
      transitionTimingFunction: {
        out: 'cubic-bezier(0.2, 0, 0, 1)',
        in: 'cubic-bezier(0.4, 0, 1, 1)',
      },
    },
  },
  plugins: [],
} satisfies Config
