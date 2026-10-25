import type { CSSProperties, ReactNode } from 'react'

/**
 * Chirp's own icon set (docs/v2/design/motion.html): 24 grid, 2 px stroke,
 * round caps and joins; keylines circle 20, square 18, landscape 20 x 16,
 * portrait 16 x 20.
 */
export interface IconProps {
  size?: number
  className?: string
  style?: CSSProperties
}

function make(name: string, body: ReactNode, viewBox = '0 0 24 24') {
  function Icon({ size = 16, className, style }: IconProps) {
    return (
      <svg
        width={size}
        height={size}
        viewBox={viewBox}
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        className={className ? `shrink-0 ${className}` : 'shrink-0'}
        style={style}
        aria-hidden
        focusable="false"
      >
        {body}
      </svg>
    )
  }
  Icon.displayName = `Icon${name}`
  return Icon
}

export const IconNext = make('Next', <path d="M9 5l7 7-7 7" />)
export const IconBack = make('Back', <path d="M15 5l-7 7 7 7" />)
export const IconPause = make(
  'Pause',
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M10 9v6M14 9v6" />
  </>,
)
export const IconSettings = make(
  'Settings',
  <>
    <path d="M3 8h9.5M17.5 8H21M3 16h3.5M11.5 16H21" />
    <circle cx="15" cy="8" r="2.5" />
    <circle cx="9" cy="16" r="2.5" />
  </>,
)
export const IconWords = make(
  'Words',
  <>
    <path d="M3 19 8 5l5 14M4.8 14h6.4" />
    <circle cx="17.5" cy="15.5" r="3.5" />
    <path d="M21 12v7" />
  </>,
)
export const IconShortcuts = make(
  'Shortcuts',
  <>
    <rect x="2" y="5" width="20" height="14" rx="3" />
    <path d="M6 10h0M10 10h0M14 10h0M18 10h0M8 15h8" />
  </>,
)
export const IconMicrophone = make(
  'Microphone',
  <>
    <rect x="9" y="2" width="6" height="12" rx="3" />
    <path d="M5 10a7 7 0 0 0 14 0M12 17v4M8 21h8" />
  </>,
)
export const IconSound = make(
  'Sound',
  <>
    <path d="M3 9v6h4l5 4V5L7 9z" />
    <path d="M16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12" />
  </>,
)
export const IconAiCleanup = make(
  'AiCleanup',
  <>
    <rect x="5" y="8" width="14" height="12" rx="4" />
    <path d="M12 8V5M9.5 13v2M14.5 13v2M3 12.5v3M21 12.5v3" />
    <circle cx="12" cy="3.5" r="1.5" />
  </>,
)
/** The bird outline, derived from the brand path and fitted to the 20-unit live area. */
export const IconChirp = make(
  'Chirp',
  <path
    strokeWidth={400}
    d="M1531.604,2829.783c668.518,24.139 1171.926,-353.144 1045.89,-883.882c-0.56,-2.358 -17.658,-74.359 -23.385,-19.253c-0.995,9.575 69.958,358.895 -292.787,623.11c-524.721,382.194 -1516.421,61.554 -1543.52,-49.205c-11.643,-47.586 14.39,-42.864 628.22,-583.275c147.113,-129.517 646.449,-555.007 653.201,-572.264c34.69,-88.668 53.661,-462.329 467.34,-678.203c261.617,-136.522 720.888,-99.525 911.155,199.849c29.64,46.637 21.052,59.492 57.912,65.5c39.258,6.398 321.439,44.685 346.981,111.874c6.542,17.208 3.266,26.147 -189.861,130.03c-119.168,64.1 -121.172,70.126 -124.937,81.447c-10.473,31.492 -22.872,225.25 16.526,343.54c4.893,14.69 49.551,107.832 76.735,336.287c61.149,513.904 -236.456,1191.465 -944.642,1420.703c-412.05,133.38 -863.994,33.644 -1136.283,-126.848c-276.402,-162.916 -260.407,-240.238 -310.516,-201.362c-357.873,277.646 -414.605,298.476 -456.184,313.742c-37.345,13.712 -249.743,91.697 -493.694,-59.685c-90.71,-56.289 2.227,-101.049 53.22,-135.73c621.383,-422.616 620.158,-437.327 651.083,-428.394c149.547,43.196 321.948,101.022 607.547,112.018Z"
  />,
  '-418.5 -397.5 4804 4804',
)
export const IconRecent = make(
  'Recent',
  <>
    <path d="M8 17H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4v6a4 4 0 0 1-4 4h-5l-4 4z" />
    <path d="M12 6.5V10h3" />
  </>,
)
export const IconSnippets = make(
  'Snippets',
  <>
    <path d="M4 6h10M4 12h7M4 18h4" />
    <path d="M19.5 5 16 12h4.5L17 19" />
  </>,
)
export const IconAdd = make('Add', <path d="M12 5v14M5 12h14" />)
export const IconRemove = make('Remove', <path d="M6 6l12 12M18 6 6 18" />)
export const IconOpen = make('Open', <path d="M7 17 17 7M8 7h9v9" />)
export const IconCopy = make(
  'Copy',
  <>
    <rect x="8" y="8" width="13" height="13" rx="3" />
    <path d="M16 8V6a3 3 0 0 0-3-3H6a3 3 0 0 0-3 3v7a3 3 0 0 0 3 3h2" />
  </>,
)
/** Same grid and stroke as the set; used by the "You're set" pill. */
export const IconCheck = make('Check', <path d="M5 12.5l4.5 4.5L19 7.5" />)
