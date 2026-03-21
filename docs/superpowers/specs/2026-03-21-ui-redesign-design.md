# Chirp UI Redesign — Design Spec

## Context

Chirp is preparing for public launch as a free, local-only voice-to-text desktop app. The current UI works but feels like a prototype — too much white space, no micro-animations, generic settings layout, and nothing that makes a user say "wow." This redesign creates a distinctive, polished experience with craft, data visualization, and spatial depth throughout.

## Design Decisions

### Visual Direction: Clean & Minimal with Strong Identity
- **Not** warm/amber-heavy (too similar to current)
- **Not** dark mode (light mode only)
- Identity expressed through: unique UI patterns, typography craft, strategic accent color, branded sidebar

### UX Direction: Contextual Hub
- Home page surfaces useful information proactively (usage stats, dictionary suggestions, model status)
- Dashboard-first, not settings-first
- The app feels helpful and alive, not just a settings panel

### Wow Factor: All Three
- **Craft & polish**: Spring-eased hover animations, staggered entrance animations, count-up numbers
- **Data visualization**: Weekly bar charts, trend percentages, time-saved calculations
- **Spatial depth**: Cards lift on hover, layered shadows, floating gradient orbs, glass effects

---

## Design System

### Colors
| Token | Value | Usage |
|-------|-------|-------|
| `sidebar` | `#1a1917` | Dark sidebar background |
| `surface` | `#F5F4F0` | Content area background |
| `card-bg` | `#FFFFFF` | Card backgrounds |
| `card-border` | `#EDECE8` | Card/section borders |
| `text-primary` | `#1a1a1a` | Headings, active text, toggle-on |
| `text-secondary` | `#888888` | Body text, descriptions |
| `text-tertiary` | `#aaaaaa` | Labels, metadata |
| `text-muted` | `#cccccc` | Timestamps, separators |
| `accent` | `#F0B723` | Nav indicator, polished badges, trend stats, links |
| `accent-glow` | `rgba(240,183,35,0.3)` | Logo glow, orb fills |
| `success` | `#22c55e` | Ready status, model status |
| `error` | `#DC2626` | Error states, delete hover |

### Typography
| Role | Font | Weight | Size |
|------|------|--------|------|
| Display numbers | Nunito | 900 | 32-40px, letter-spacing -1.5px |
| Page titles | Nunito | 800 | 22-24px, letter-spacing -0.5px |
| Section titles | Nunito | 700 | 15px |
| Logo | Nunito | 900 | 20px |
| Body | Inter | 400-500 | 13px |
| Descriptions | Inter | 400 | 11-12px |
| Labels/uppercase | Inter | 500-600 | 10-11px, letter-spacing 0.5-1px |
| Keys/mono | JetBrains Mono | 500 | 11px |

### Spacing & Radius
- Cards: `14px` border-radius
- Buttons/inputs: `8px`
- Nav items: `8px`
- Key badges: `5px`
- Section gaps: `20px`
- Card padding: `16-20px`

### Shadows
| Name | Value | Usage |
|------|-------|-------|
| card-rest | `0 1px 3px rgba(0,0,0,0.04)` | Cards at rest |
| card-hover | `0 8px 24px rgba(0,0,0,0.08)` | Cards on hover |
| nav-active | `0 1px 3px rgba(0,0,0,0.06), 0 0 0 1px rgba(0,0,0,0.04)` | Active nav pill |
| logo-glow | `0 0 20px rgba(240,183,35,0.3)` | Logo mark |
| status-glow | `0 0 8px rgba(34,197,94,0.5)` | Ready status dot |

### Animations
| Name | Easing | Duration | Usage |
|------|--------|----------|-------|
| slideUp | `ease-out` | 400ms | Page entrance, staggered per element (+80ms) |
| countUp | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 600ms | Stat numbers (blur to clear) |
| barGrow | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 600ms | Chart bars (scaleY 0→1, stagger +50ms) |
| glowPulse | `ease-in-out` | 2s infinite | Status dot (shadow intensity) |
| float | `ease-in-out` | 6-8s infinite | Gradient orbs in hero |
| hover-lift | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 250ms | Cards translateY(-2px) + shadow |
| spring-toggle | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 200ms | Toggle thumb position |

---

## Sidebar

- **Background**: `#1a1917` with SVG noise texture at 3% opacity (pseudo-element)
- **Logo area**: BirdMark (32px, amber rounded square with glow shadow) + "chirp" in Nunito 900
- **Ambient glow**: Radial gradient from logo position, `rgba(240,183,35,0.12)`
- **Nav items** (5): Home, History, Dictionary, Snippets, Settings
  - Inactive: `rgba(255,255,255,0.4)` text
  - Hover: `rgba(255,255,255,0.7)` text, `rgba(255,255,255,0.04)` bg
  - Active: `#F0B723` text, `rgba(240,183,35,0.08)` bg, 3px amber bar on left edge
- **Badges**: History count badge in amber on nav item
- **Hotkey card** (bottom): Glass effect (`rgba(255,255,255,0.05)` bg, `rgba(255,255,255,0.06)` border), "HOLD TO DICTATE" label, key badges
- **Version**: `v1.0.0` centered at very bottom, very faint
- **About**: Not a nav item — linked from version text or a subtle link

---

## Pages

### Home (Dashboard)

**Hero Stats Block**
- Dark rounded card (`#1a1917`, 18px radius) spanning full width
- Two floating amber gradient orbs (`position: absolute`, `filter: blur(60px)`, animated with `float` keyframes at different speeds)
- Greeting: "Good morning/afternoon/evening" (Nunito 800 22px white) + date (12px, white 40% opacity)
- Status pill: top-right, glass bg, green glowing dot + "Ready"
- Three stat columns separated by `rgba(255,255,255,0.06)` borders:
  1. **Words today**: Nunito 900 40px, gradient text (white to white 70%), trend sub ("↑ 23% vs yesterday" in amber)
  2. **Sessions**: Nunito 900 40px, weekly bar chart below (7 amber bars, current day highlighted, staggered grow animation)
  3. **All time**: Nunito 900 40px, "~X.X hrs saved" sub-text

**Contextual Cards Row** (2 cards side-by-side)
- **Suggestion card**: Warm gradient bg (`#FFFDF5` → `#FFF8E5`), amber border, icon + title + description + "Add rule" dark button / "Dismiss" ghost button
  - Suggestions derived from: word frequency in history, empty dictionary, model not downloaded
- **Model status card**: White bg, green dot status rows for speech model + cleanup model

**Recent Transcriptions** (3 items)
- Section title (Nunito 700 15px) + "View all →" amber link
- Items: amber gradient left indicator bar (polished) or gray (raw), text + meta (words, duration, Polished badge), time, copy/delete on hover

### History

**Top bar**: Page title + search box (amber focus ring) + Export button
**List**: Day-grouped with headers ("Today — March 21" + "1,247 words · 12 sessions")
**Items**: Same as Home recent but with WPM badge, expanded hover actions
**Bottom stats bar**: Persistent, shows total words / sessions / hrs saved

### Dictionary

Same table structure as current, with new visual treatment:
- Card border `#EDECE8`, radius `14px`
- Row hover lift animation
- Staggered entrance
- Empty state with suggestion to try a phrase

### Snippets

Same as Dictionary treatment.

### Settings (consolidated)

Single scrollable page with 5 sections:

**Hotkey**: SegmentedControl (Custom shortcut / Dedicated key), key badge display, status indicator
**Audio**: Mic dropdown, input level bar + inline "Test mic" link, noise suppression toggle
**AI & Output**: Smart Cleanup toggle, Smart Formatting toggle, Tone dropdown
**Behavior**: Launch at login, play sound, auto-dismiss, passive overlay (all toggles), overlay position (SegmentedControl)
**Models**: Speech model + cleanup model status/download cards

Each section: uppercase label → white card with bordered rows, setting name + description on left, control on right.

### About (Modal)

Not a page — modal triggered from sidebar footer. Contains: logo, version, tagline, domain link, update checker, credits. Backdrop blur overlay.

---

## Onboarding (Simplified)

Reduced from 6 steps to 3-4:

1. **Welcome + Mic**: Welcome message + mic permission request. If denied, show error + system settings link.
2. **Setup**: Mic test (3s record + playback) + hotkey capture. Two sections on one page.
3. **Model Download**: Auto-starts download, progress bar, auto-advances on completion.
4. **Smart Cleanup** (optional): Before/after example, "Turn on" button or skip.

Same new design system: dark left panel with branded elements, warm right content panel.

---

## Removed Features

- **Transcribe File page** — killed, remove from nav and delete component
- **About page** — replaced by modal

---

## Overlay

**Not changed** — recently redesigned warm frosted glass pill is kept as-is.

---

## Polish Bugs

- Remove debug logging (console.log, verbose whisper output)
- Fix error state display consistency
- Clean up edge cases in recording flow

---

## Interactive Mockups

Reference mockups are in `.superpowers/brainstorm/2454-1774072614/`:
- `wow-home-v2.html` — Home page (approved design)
- `settings-page.html` — Settings page
- `history-page.html` — History page
