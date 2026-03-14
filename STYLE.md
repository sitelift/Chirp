# STYLE.md — Chirp Design System

> **Every visual decision for Chirp lives here.**
> Claude Code: follow this file exactly. Do not improvise colors, fonts, spacing, radii, shadows, or any visual property. If something is not specified here, ask — do not guess.

---

## 1. Brand Identity

**Name:** Chirp (always lowercase in wordmark: "chirp")
**Domain:** trychirp.app
**Tagline:** "Speak freely."
**Secondary taglines:** "Talk. Type. Done." / "Your voice, your device, your text." / "Free, local voice-to-text for everyone."
**Logo:** Yellow bird silhouette — SVG provided in `/assets/brand/chirp-mark-primary.svg`. Do not redraw, trace, or simplify. Use the exact path data.

---

## 2. Color System

### 2.1 Tokens (use these names in Tailwind config)

```javascript
// tailwind.config.ts
module.exports = {
  theme: {
    extend: {
      colors: {
        chirp: {
          yellow: '#F0B723',      // Bird mark, primary brand accent
          amber: {
            50:  '#FFFBEB',       // Lightest background tint
            100: '#FEF3C7',       // Light section backgrounds
            200: '#FDE68A',       // Hover highlight
            300: '#FCD34D',       // Active/pressed state on yellow
            400: '#FBBF24',       // CTAs, app icon bg, buttons
            500: '#F59E0B',       // Darker accent (sparingly)
          },
          stone: {
            100: '#F5F5F4',       // App chrome, sidebar bg
            200: '#E7E5E4',       // Borders, dividers, subtle outlines
            300: '#D6D3D1',       // Inactive elements, disabled borders
            500: '#78716C',       // Secondary text, captions, hints
            700: '#44403C',       // Body text
            900: '#1C1917',       // Headings, primary text, dark backgrounds
          },
          white: '#FFFFFF',       // Card surfaces, content areas, inputs
          success: '#16A34A',     // Recording indicator, success states
          error: '#DC2626',       // Error states, destructive actions
          info: '#2563EB',        // Links, informational highlights
        }
      }
    }
  }
}
```

### 2.2 Color Application Rules

**Backgrounds:**
- App chrome / window frame: `stone-100`
- Content areas, cards, modals: `white`
- Sidebar: `stone-100`
- Light sections (landing page): `amber-50`
- Dark sections (footer only): `stone-900`
- NEVER use `amber-400` or `chirp-yellow` as a large background in the app
- Landing page hero may use `amber-50` as background, never full yellow

**Text:**
- Primary headings: `stone-900`
- Body text: `stone-700`
- Secondary / caption / hint: `stone-500`
- Text on yellow buttons: `stone-900`
- Text on dark backgrounds: `white`
- Links: `info` (#2563EB), underline on hover only

**Borders:**
- Default: `stone-200` at 1px
- Focused inputs: `amber-400` at 2px
- Cards: `stone-200` at 1px OR no border with shadow
- Dividers / separators: `stone-200` at 1px

**Brand yellow usage:**
- Bird logo mark
- Primary CTA buttons (bg `amber-400`, text `stone-900`)
- Active/selected indicators
- Checkbox/toggle fill when checked
- Waveform bars
- Step indicator dots (active state)
- Progress bars (fill color)
- NEVER for body text (fails contrast on white)
- NEVER for large surface areas in the app

**Semantic colors:**
- `success` (#16A34A): recording active dot, success confirmations, checkmarks in comparison tables
- `error` (#DC2626): error icons, error text, destructive button hover
- `info` (#2563EB): links only

---

## 3. Typography

### 3.1 Font Stack

```css
/* Import from Google Fonts */
@import url('https://fonts.googleapis.com/css2?family=Nunito:wght@600;700;800&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap');
```

**Display font:** Nunito
- Wordmark: 800 (ExtraBold)
- Headings: 700 (Bold)
- Subheadings: 600 (SemiBold)
- Usage: logo wordmark, all headings, button labels, navigation items, section titles

**Body font:** Inter
- Body: 400 (Regular)
- Labels: 500 (Medium)
- Emphasis: 600 (SemiBold)
- Usage: all body text, descriptions, UI labels, form inputs, captions, tooltips

**Monospace font:** JetBrains Mono
- Code: 400 (Regular)
- Code emphasis: 500 (Medium)
- Usage: hotkey badges, version numbers, URLs, technical values, code references, file paths

### 3.2 Type Scale — Desktop App

| Element | Font | Weight | Size | Line Height | Color | Letter Spacing |
|---------|------|--------|------|-------------|-------|---------------|
| App branding (sidebar) | Nunito | 800 | 18px | 1.2 | stone-900 | -0.5px |
| Section header | Nunito | 700 | 16px | 1.3 | stone-900 | 0 |
| Card/group title | Nunito | 600 | 14px | 1.3 | stone-900 | 0 |
| Body text | Inter | 400 | 14px | 1.5 | stone-700 | 0 |
| UI label | Inter | 500 | 14px | 1.4 | stone-700 | 0 |
| Button text | Nunito | 700 | 14px | 1 | varies | 0 |
| Secondary text | Inter | 400 | 13px | 1.5 | stone-500 | 0 |
| Caption / hint | Inter | 400 | 12px | 1.4 | stone-500 | 0 |
| Group label (floating) | Inter | 600 | 12px | 1 | stone-500 | 0.5px |
| Hotkey badge | JetBrains Mono | 500 | 12px | 1 | stone-700 | 0 |
| Version / URL | JetBrains Mono | 400 | 13px | 1.4 | stone-500 | 0 |
| Tiny label | Inter | 500 | 11px | 1.3 | stone-500 | 0.3px |

### 3.3 Type Scale — Landing Page

| Element | Font | Weight | Size | Line Height | Color |
|---------|------|--------|------|-------------|-------|
| Hero headline | Nunito | 800 | 60px | 1.1 | stone-900 |
| Section title | Nunito | 700 | 36px | 1.2 | stone-900 |
| Card title | Nunito | 700 | 20px | 1.3 | stone-900 |
| Hero subhead | Inter | 400 | 18px | 1.7 | stone-700 |
| Body | Inter | 400 | 17px | 1.7 | stone-700 |
| Nav links | Inter | 500 | 14px | 1 | stone-700 |
| Small print | Inter | 400 | 13px | 1.5 | stone-500 |
| Code / URL | JetBrains Mono | 400 | 15px | 1.5 | amber-500 |

---

## 4. Spacing System

Use Tailwind's default spacing scale. These are the most commonly used values:

| Token | Value | Usage |
|-------|-------|-------|
| `p-1` / `gap-1` | 4px | Tight internal spacing (icon-to-text) |
| `p-2` / `gap-2` | 8px | Internal padding (badges, small chips) |
| `p-3` / `gap-3` | 12px | Compact card padding, input padding |
| `p-4` / `gap-4` | 16px | Default element spacing |
| `p-5` / `gap-5` | 20px | Card internal padding |
| `p-6` / `gap-6` | 24px | Section padding (inside cards/groups) |
| `p-8` / `gap-8` | 32px | Major section spacing |
| `p-10` / `gap-10` | 40px | Landing page section padding |
| `p-12` / `gap-12` | 48px | Landing page section gaps |
| `p-16` / `gap-16` | 64px | Hero vertical padding |

**Rules:**
- Consistent vertical rhythm: elements within a group use `gap-3` or `gap-4`
- Groups/cards separated by `gap-5` or `gap-6`
- Sections separated by `gap-8` or larger
- Landing page sections: `py-16` or `py-20`
- Never mix spacing values randomly — pick a rhythm and stick to it

---

## 5. Border Radius

| Element | Radius | Tailwind Class |
|---------|--------|---------------|
| Main window (overlay) | 20px | `rounded-[20px]` |
| Cards, modals, grouped settings | 12px | `rounded-xl` |
| Buttons, inputs, dropdowns | 8px | `rounded-lg` |
| Checkboxes, toggles | 6px | `rounded-md` |
| Badges, key caps, tags | 6px | `rounded-md` |
| Progress bars | 4px | `rounded` |
| App icon background | 112px on 512px | `rounded-[22%]` |
| Avatar / circular elements | 9999px | `rounded-full` |

**Rule: no sharp corners anywhere in the entire app or landing page.** Everything has radius. The minimum radius for any visible element is 4px.

---

## 6. Shadows

| Level | Value | Usage |
|-------|-------|-------|
| None | — | Flat elements, inline items |
| Subtle | `0 1px 2px rgba(0,0,0,0.05)` | Active sidebar item, selected states |
| Card | `0 1px 3px rgba(0,0,0,0.08)` | Cards, setting groups, dropdown menus |
| Elevated | `0 4px 12px rgba(0,0,0,0.12)` | Modals, popovers, floating elements |
| Overlay | `0 8px 32px rgba(0,0,0,0.16)` | Floating overlay window only |

```javascript
// tailwind.config.ts
boxShadow: {
  subtle: '0 1px 2px rgba(0,0,0,0.05)',
  card: '0 1px 3px rgba(0,0,0,0.08)',
  elevated: '0 4px 12px rgba(0,0,0,0.12)',
  overlay: '0 8px 32px rgba(0,0,0,0.16)',
}
```

**Rules:**
- Cards can use EITHER a `stone-200` border OR `shadow-card`, not both
- The overlay always uses `shadow-overlay`
- Dark backgrounds never have shadows
- Shadows are always black/neutral, never colored

---

## 7. Component Specs

### 7.1 Buttons

**Primary button:**
```
Background: amber-400
Text: stone-900, Nunito 700, 14px
Height: 36px (app) / 44-48px (landing page)
Padding: 0 16px (app) / 0 24px (landing)
Radius: rounded-lg (8px)
Hover: amber-300
Active/pressed: amber-500
Disabled: amber-200, stone-400 text, cursor-not-allowed
Transition: 150ms ease-out on background-color
```

**Secondary button:**
```
Background: white
Border: 1px solid stone-200
Text: stone-700, Inter 500, 14px
Height: 36px (app) / 44-48px (landing)
Padding: 0 16px (app) / 0 24px (landing)
Radius: rounded-lg (8px)
Hover: stone-100 background
Active/pressed: stone-200 background
Transition: 150ms ease-out on background-color
```

**Ghost button (text only):**
```
Background: transparent
Text: stone-500, Inter 500, 13px
Hover: stone-700 text
No border, no padding beyond text
```

**Icon button:**
```
Width/height: 32px
Radius: rounded-lg (8px)
Background: transparent
Hover: stone-100 background
Icon: 20px, stone-500
Hover icon: stone-700
```

### 7.2 Inputs

**Text input / dropdown:**
```
Height: 40px
Background: white
Border: 1px solid stone-200
Radius: rounded-lg (8px)
Padding: 0 12px
Font: Inter 400, 14px, stone-700
Placeholder: Inter 400, 14px, stone-500 (italic)
Focus: border becomes 2px solid amber-400, no outline
Transition: 150ms ease-out on border-color
```

**Dropdown (select):**
- Same as text input styling
- Chevron icon (Lucide `ChevronDown`, 16px, stone-500) on right, 12px padding
- Dropdown menu: white bg, shadow-elevated, rounded-xl, 4px padding
- Menu items: 36px height, rounded-lg on hover (stone-100 bg)

### 7.3 Checkboxes

```
Size: 18px × 18px
Unchecked: white bg, 1px solid stone-300, rounded-md (6px)
Checked: amber-400 bg, white checkmark icon (Lucide Check, 12px, 2px stroke)
Hover (unchecked): stone-200 border
Focus: 2px amber-400 ring with 2px offset
Transition: 150ms ease-out
```

### 7.4 Toggle / Switch

```
Track: 40px × 22px, rounded-full
Off: stone-300 track, white thumb
On: amber-400 track, white thumb
Thumb: 18px circle, white, shadow-subtle
Thumb position: 2px inset from track edge
Transition: 200ms ease-out on background-color and transform
```

### 7.5 Key Badges (hotkey display)

```
Background: white
Border: 1px solid stone-200
Shadow: 0 1px 2px rgba(0,0,0,0.06)
Radius: rounded-md (6px)
Padding: 2px 8px
Font: JetBrains Mono 500, 12px, stone-700
Min-width: 28px (center-aligned text)
Display: inline-flex, items-center, justify-center
```

### 7.6 Cards / Setting Groups

```
Background: white
Border: 1px solid stone-200
Radius: rounded-xl (12px)
Padding: 20px
No shadow (border is sufficient)
```

**Floating group label:**
```
Position: absolute, top -10px, left 16px
Background: white
Padding: 0 8px
Font: Inter 600, 12px, stone-500
Text-transform: uppercase
Letter-spacing: 0.5px
```

### 7.7 Sidebar (Settings)

```
Width: 160px
Background: stone-100
Padding: 16px 8px
Border-right: 1px solid stone-200
```

**Logo area:**
```
Padding: 0 8px 16px 8px
Border-bottom: 1px solid stone-200
Margin-bottom: 16px
Bird mark: 24px height, chirp-yellow color
Wordmark: Nunito 800, 16px, stone-900, 8px gap from bird
```

**Nav items:**
```
Height: 36px
Padding: 0 12px
Radius: rounded-lg (8px)
Font: Inter 500, 14px

Inactive: stone-500 text, transparent bg
Hover: stone-200 bg
Active: stone-900 text, white bg, shadow-subtle
```

### 7.8 Waveform Visualization

```
Container: full width of overlay content area, 40px height
Bars: 3px wide, 3px gap between bars
Bar count: fills container width (~48 bars at 420px overlay)
Color: amber-400
Bar min-height: 3px (rounded-full caps)
Bar max-height: 36px
Animation: amplitude values interpolated over 80ms
Corner: rounded-full on each bar (pill shape)
```

### 7.9 Progress Bar

```
Track: full width, 4px height, stone-200, rounded-full
Fill: amber-400, rounded-full
Shimmer variant: amber-100 track, amber-400 shimmer sweeping left to right
Shimmer animation: 1.5s ease-in-out infinite
```

### 7.10 Recording Pulse Dot

```
Size: 8px
Color: success (#16A34A)
Shape: circle (rounded-full)
Animation: scale 1→1.4→1, opacity 1→0.6→1, 1.5s ease-in-out infinite
```

### 7.11 Step Indicator Dots (Onboarding)

```
Size: 8px
Gap: 8px between dots
Active: amber-400
Inactive: stone-300
Shape: rounded-full
Transition: 200ms ease-out on background-color
```

---

## 8. Iconography

**Library:** Lucide React (https://lucide.dev)
**Default size:** 20px
**Default stroke-width:** 1.5px
**Default color:** stone-500
**Active/selected color:** stone-900
**On yellow background:** stone-900
**On dark background:** white

**Commonly used icons:**
| Context | Icon Name |
|---------|-----------|
| Microphone | `Mic` |
| Settings | `Settings` |
| Keyboard/hotkey | `Keyboard` |
| Download | `Download` |
| Check/success | `Check` |
| Error/warning | `AlertTriangle` |
| Close | `X` |
| Privacy/security | `Shield` |
| Speed | `Zap` |
| Free/heart | `Heart` |
| Audio/volume | `Volume2` |
| Chevron down | `ChevronDown` |
| External link | `ExternalLink` |
| GitHub | `Github` |
| Info | `Info` |
| Search | `Search` |
| Plus (add) | `Plus` |
| Minus/delete | `Trash2` |
| Edit | `Pencil` |

---

## 9. Animation & Transitions

### 9.1 General Rules

- All interactive state changes: `150ms ease-out`
- Layout shifts / size changes: `200ms ease-out`
- Page/view transitions: `200ms ease-out` on opacity
- Never use `ease-in` for UI transitions (feels sluggish)
- Never animate anything longer than 300ms (feels slow)
- Prefer `transform` and `opacity` for 60fps animations
- Use `will-change` sparingly and only on animated elements

### 9.2 Specific Animations

**Overlay appear:**
```css
animation: overlayIn 150ms ease-out;
@keyframes overlayIn {
  from { opacity: 0; transform: translateY(-8px); }
  to { opacity: 1; transform: translateY(0); }
}
```

**Overlay dismiss:**
```css
animation: overlayOut 150ms ease-in;
@keyframes overlayOut {
  from { opacity: 1; }
  to { opacity: 0; }
}
```

**Overlay state transitions (listening → processing → done):**
```css
/* Crossfade between states */
transition: opacity 200ms ease-out;
```

**Shimmer loader:**
```css
animation: shimmer 1.5s ease-in-out infinite;
@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}
/* Applied to a pseudo-element over the progress track */
```

**Recording pulse:**
```css
animation: pulse 1.5s ease-in-out infinite;
@keyframes pulse {
  0%, 100% { transform: scale(1); opacity: 1; }
  50% { transform: scale(1.4); opacity: 0.6; }
}
```

**Waveform bars:**
```css
transition: height 80ms linear;
/* Each bar's height updates from amplitude data */
```

**Button hover:**
```css
transition: background-color 150ms ease-out;
```

**Focus ring (inputs, checkboxes):**
```css
transition: box-shadow 150ms ease-out;
/* Focus: ring-2 ring-amber-400 ring-offset-2 */
```

---

## 10. Layout Rules

### 10.1 Overlay

- Fixed width: 420px
- Height: varies by state (auto, content-driven)
- Max height: 200px
- Position: centered horizontally, top 20% of screen
- Background: white
- Border: 1px solid stone-200
- Radius: 20px
- Shadow: overlay (0 8px 32px)
- Internal padding: 24px

### 10.2 Settings Window

- Default: 640px × 520px
- Min: 560px × 440px
- Sidebar: 160px fixed width, left side
- Content area: remaining width, scrollable vertically
- Content padding: 32px
- Gap between setting groups: 24px

### 10.3 Onboarding

- Same dimensions as settings: 640px × 520px
- No sidebar
- All content centered horizontally
- Max content width: 400px
- Generous vertical padding: 48px top

### 10.4 Landing Page

- Max content width: 1120px, centered
- Nav height: 64px
- Section vertical padding: 80px (py-20)
- Card grid: 3 columns, 24px gap
- Hero: 2-column layout (text left, bird right) above 768px, stacked below
- Footer height: 80px

---

## 11. Responsive Behavior (Landing Page Only)

The desktop app is fixed-size and not responsive. The landing page is.

| Breakpoint | Behavior |
|-----------|----------|
| < 640px (mobile) | Single column, hero stacked, cards stacked, nav hamburger |
| 640-1024px (tablet) | 2-column card grid, hero stacked, nav visible |
| > 1024px (desktop) | Full 3-column grid, hero side-by-side, full nav |

Hero headline scales: 36px mobile → 48px tablet → 60px desktop
Section titles scale: 28px mobile → 32px tablet → 36px desktop

---

## 12. Logo Usage in App

**Sidebar (settings):**
- Bird mark: 24px height, `chirp-yellow` fill
- Gap: 8px
- "chirp" text: Nunito 800, 16px, stone-900
- Horizontal layout, vertically centered

**About page:**
- Bird mark: 64px height, `chirp-yellow` fill, centered
- "chirp" text: Nunito 800, 28px, stone-900, centered below, 8px gap
- Version below: JetBrains Mono 400, 13px, stone-500

**Overlay:**
- Bird mark: 20px height, `chirp-yellow` fill
- Inline with status text ("Listening...", "Processing...")
- 8px gap between bird and text

**System tray:**
- Bird silhouette only, no text
- 16px on Windows, 22px on macOS menu bar
- Idle: monochrome (platform-appropriate)
- Listening: success green (#16A34A)
- Processing: chirp-yellow (#F0B723)
- Error: error red (#DC2626)

**Landing page nav:**
- Bird mark: 28px height, `chirp-yellow` fill
- "chirp" text: Nunito 800, 18px, stone-900
- 8px gap

---

## 13. Dark Mode

**Chirp v1 does not have dark mode.** The app is light-only. Do not implement dark mode, do not add dark mode toggles, do not add `dark:` Tailwind variants. The brand is built around yellow-on-white. Dark mode is a v2 consideration at the earliest.

The only dark surface is the landing page footer (stone-900 background), which uses white text and the white bird variant.

---

## 14. Do Not

- Do not use Inter for headings
- Do not use Nunito for body text
- Do not use any color outside the defined palette
- Do not use gradients anywhere (flat colors only)
- Do not use border-radius: 0 on any visible element
- Do not use shadows on dark backgrounds
- Do not use more than 3 brand colors in a single component
- Do not use amber-400 for text on white backgrounds
- Do not add dark mode
- Do not add custom fonts beyond Nunito, Inter, JetBrains Mono
- Do not use icon libraries other than Lucide
- Do not use opacity for disabled states on text (use stone-400 color instead)
- Do not animate anything longer than 300ms
- Do not use `ease-in` for UI transitions
- Do not use sharp corners (0 radius) anywhere
- Do not use colored shadows
- Do not mix border + shadow on the same card (pick one)
