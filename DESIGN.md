# Synapt Design System

This document is the single source of truth for all design decisions in Synapt.
All UI development must read this file before writing any component, CSS, or layout code.

Token values below were read from `src/index.css`, `tailwind.config.js`, and the
components under `src/`. Where the current code does not yet match the rule, the
gap is listed under "Current code differs" so it can be fixed, not copied.

## Guiding Principles

1. Functional over decorative - every visual element serves a purpose
2. Consistent over clever - use the system, do not invent exceptions
3. Platform-aware - the overlay must feel at home on macOS, Windows, and Linux
4. No decorative accents - the accent colour is reserved for interactive and active states only, never used for decoration, borders on cards, or status indicators
5. No status dots or pill badges - use text labels instead

## Colour Tokens

Defined in `src/index.css`. The theme is chosen by `data-theme` on `<html>`
(`dark`, `light`, or `system`). `system` follows `prefers-color-scheme` through two
media-query blocks. The bare `:root` block repeats the dark values as the default.

Tailwind extends only transition timing and duration (see Animation System). Colours
are not in the Tailwind theme, so tokens are applied through inline `style` props as
`var(--token)` rather than Tailwind colour classes.

### Background

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | `#1a1a1a` | `#f5f5f5` | Window root, header bar |

### Surface

| Token | Dark | Light | Use |
|---|---|---|---|
| `--surface` | `#242424` | `#ffffff` | Search bar, cards, inputs, menus |
| `--surface-hover` | `#2a2a2a` | `#ececec` | Hovered and selected rows, badge fill |

### Text

| Token | Dark | Light | Use |
|---|---|---|---|
| `--text` | `#ffffff` | `#111111` | Primary text |
| `--muted` | `#919191` | `#666666` | Secondary text, section headings, icons |

### Border

| Token | Dark | Light | Use |
|---|---|---|---|
| `--border` | `#333333` | `#e0e0e0` | Dividers, input and card outlines |

### Accent

| Token | Dark | Light | Use |
|---|---|---|---|
| `--accent` | `#4f8ff7` | `#2160eb` | Accent text, focus ring, active and selected states, progress fills |
| `--accent-fill` | `#2563eb` | `#2160eb` | Background behind `--on-accent` text: primary buttons |
| `--accent-soft` | `#1f3a72` | `#dbeafe` | Low-emphasis accent fill (defined, not used in Synapt today) |

Tinted variants use `color-mix(in srgb, var(--token) N%, transparent)`: `--accent` 60%
(focus ring), `--danger` 40% (destructive button border) and 10% (destructive hover).

### Semantic

| Token | Dark | Light | Use |
|---|---|---|---|
| `--danger` | `#f16161` | `#d02222` | Destructive actions, errors |
| `--danger-fill` | `#dc2626` | `#d02222` | Background behind `--on-accent` text (no filled danger button exists today) |
| `--success` | `#22c55e` | `#11813b` | Completed states |
| `--warning` | `#f59e0b` | `#aa5d05` | Warning icon and text |
| `--warning-bg` | `#3a2e14` | `#fef3c7` | Warning banner fill |
| `--warning-border` | `#553c18` | `#fcd34d` | Warning banner border |
| `--warning-text` | `#fef08a` | `#78350f` | Text on warning banner |
| `--highlight-bg` | `#854d0e` | `#fde68a` | Search match highlight fill |
| `--highlight-text` | `#fef08a` | `#78350f` | Search match highlight text |

### Contrast (WCAG 2.1 AA)

Every text token meets 4.5:1 on each surface it is used on (checked in v0.6.0; the
minimum lightness change that passes was applied to each failing token). Re-check
these pairs whenever a token value changes.

| Pair | Dark | Light |
|---|---|---|
| `--text` on `--bg` | 17.40 | 17.32 |
| `--text` on `--surface-hover` | 14.35 | 15.98 |
| `--muted` on `--bg` | 5.52 | 5.27 |
| `--muted` on `--surface` | 4.93 | 5.74 |
| `--muted` on `--surface-hover` | 4.55 | 4.86 |
| `--on-accent` on `--accent-fill` | 5.17 | 5.33 |
| `--on-accent` on `--danger-fill` | 4.83 | 5.34 |
| `--accent` on `--bg` | 5.47 | 4.89 |
| `--accent` on `--surface-hover` | 4.51 | 4.51 |
| `--danger` on `--bg` | 5.49 | 4.90 |
| `--danger` on `--surface` | 4.90 | 5.34 |
| `--danger` on `--surface-hover` | 4.53 | 4.52 |
| `--warning` on `--bg` | 8.10 | 4.50 |
| `--warning` on `--surface` | 7.23 | 4.91 |
| `--success` on `--bg` | 7.64 | 4.56 |
| `--success` on `--surface` | 6.81 | 4.97 |

Why there are fill tokens: no single blue can carry white text on top of it (needs a
dark blue) and also read as accent text on the dark `--surface-hover` (needs a light
blue). `--accent` is for text, borders and focus; `--accent-fill` is for backgrounds
behind `--on-accent` text. The same split applies to `--danger` and `--danger-fill`.
Muted text never sits on `--border`; badges that did so were removed.

### Theme-independent

Declared once in a separate `:root` block because the value is the same in both themes.

| Token | Value | Use |
|---|---|---|
| `--on-accent` | `#ffffff` | Text on accent- or danger-filled buttons |
| `--backdrop` | `rgba(0, 0, 0, 0.6)` | Pairing dialog backdrop |

All other tokens except the fills differ between dark and light.

### Hardcoded colours outside tokens

None in components. `src/App.css` holds the unused Vite starter palette (`#0f0f0f`,
`#f6f6f6`, `#646cff` and others); `App.tsx` and `App.css` are not imported anywhere.

## Typography

Font family: `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif`
Monospace: `monospace` (pairing and invite codes, paths in Settings)

Scale:

- 11px - section headings, Settings helper text
- 12px (`text-xs`) - secondary text, result paths and source labels, Settings body text, status bar, icon-button labels
- 13px - result names, device rows, buttons, confirmation message, Select options
- 14px (`text-sm`) - search input and other `.input` fields, empty-state titles
- 28px - invite code display in Settings (monospace)
- 32px - pairing verification code (monospace)

No component mixes more than two of these sizes.

Font weights in use: 400 (default), 500 (app result names, header title, section headings, primary buttons, empty-state titles, calculator result)

Line heights in use: Tailwind defaults (16px with `text-xs`, 20px with `text-sm`) plus 16px on buttons and headings and 20px on inputs and empty states.

Letter spacing in use: 0.06em (section headings), 0.15em (invite code), 0.2em (pairing code)

## Spacing Scale

Base unit: 8px
Approved values: 4, 8, 12, 16, 24, 32, 48px
Do not use values outside this scale. If a value between these is needed, use the nearest approved value.

Deliberate exceptions, both fixed by the Component Patterns below: buttons use `6px 14px`
padding and inputs use `10px 14px`.

Off-scale values left in the code: none in components in use. `src/components/PeerCard.tsx`
still has 2, 6 and 10px values, but nothing imports it.

## Border Radius

- 8px - window root, item cards in Settings, device picker, transfer card, pairing dialog, inputs and the search field; `0 0 8px 8px` for panels attached under a row
- 6px - buttons, inline confirmation container, Select popup
- 4px (`rounded`) - icon buttons, chips, app icons, Select trigger
- 2px - progress bars, scrollbar thumb
- 1px - underline loader
- 50% - the 6px separator dot in the indexing status bar (a separator, not a status indicator)

Rule: 8px for containers that float or sit at window level and for text fields, 6px for
buttons and small floating containers (confirmation, popup menu), 4px for icon buttons and
chips. Progress indicators use 1-2px.

## Elevation and Surfaces

Synapt uses background colour differentiation for elevation - no shadows, no borders between surface layers.

Surface hierarchy:

1. `--bg` - window root and header bar
2. `--surface` - search bar, cards, inputs, menus
3. `--surface-hover` - hovered and selected rows, badge fill

The overlay header and the search strip share `--bg` with no line between them; result
rows are separated by spacing only. No component uses a shadow.

Current code differs:

- Settings and Transfer History keep a 1px `--border` under their header, because their content scrolls beneath it.
- The pairing dialog dims the window with `var(--backdrop)`.

## Interactive States

Every interactive element must implement these states consistently:

- Default: `--surface` fill or transparent, `--text` or `--muted` label
- Hover: `--surface-hover` fill
- Active/Pressed: `--surface-hover` fill, `--accent` label
- Focus: `--accent` - visible focus ring using accent colour at reduced opacity
- Disabled: opacity 0.4, no pointer events

Implementation: shared classes in the `@layer components` block of `src/index.css`. Use
them instead of re-creating these styles inline.

| Class | Pattern |
|---|---|
| `.btn` + `.btn-primary` / `.btn-secondary` / `.btn-destructive` / `.btn-ghost` | Buttons (see Component Patterns); `.btn-ghost` is the borderless muted "Cancel" / "Later" action |
| `.icon-button` (+ `.icon-button-labeled`, `.icon-button-danger`) | Header and inline icon actions, optionally with a text label |
| `.input` | Input / Search Bar; also applied to a `<label>` wrapping an icon and a bare `<input>` (uses `:focus-within`) |
| `.list-row` with `data-selected` | Result and clip rows |
| `.section-heading` | Section Heading |
| `.empty-state`, `.empty-state-title`, `.empty-state-detail` | Empty State (via the `EmptyState` component) |
| `.error-text` | One-line plain-language error next to the action that failed |

A global `:focus-visible` rule draws a 2px `--accent` ring at 60% with a 1px offset, so
keyboard focus is always visible and mouse clicks never show it. Tailwind's default
`transition` timing is set to `--ease-standard` / 150ms in `tailwind.config.js`.

Remaining difference: list rows still use selection-follows-hover (`onMouseEnter` moves
the keyboard selection), so hover and selection look the same.

## Error Messages

Errors from Tauri commands are written for logs ("peer not online", "os error 2"). Never
show them raw. `src/utils/errors.ts` maps known causes to plain sentences that say what
happened and what to do (`describeError` in catch handlers, which also logs the raw error;
`plainError` when rendering), and falls back to a sentence written for the action that
failed. Show the result with `.error-text` (`role="alert"`) where the action lives, and
never let an action fail silently.

## Animation System

Principles:

- All animations use Apple-style critically damped spring or `cubic-bezier(0.25, 0.1, 0.25, 1.0)`
- No bounce - stiffness: 300, damping: 30 for spring animations
- Duration guidelines:
  - Micro-interactions (hover, focus): 100-150ms, `cubic-bezier(0.25, 0.1, 0.25, 1.0)`
  - Panel/overlay entrance: 200ms spring
  - Content transitions: 150ms `cubic-bezier(0.25, 0.1, 0.25, 1.0)`
  - Never exceed 300ms for any UI animation
- Respect `prefers-reduced-motion`: all animations must be disabled when this media query is set

Motion library: Framer Motion (`motion` package) for spring and presence animations.
CSS cubic-bezier for micro-interactions and hover states.

Tokens (in the stylesheet as custom properties, mirrored as Tailwind
`ease-*` / `duration-*` utilities):

| Token | Value | Tailwind |
|---|---|---|
| `--ease-standard` | `cubic-bezier(0.25, 0.1, 0.25, 1.0)` | `ease-standard` |
| `--ease-decelerate` | `cubic-bezier(0.0, 0.0, 0.2, 1.0)` | `ease-decelerate` |
| `--ease-accelerate` | `cubic-bezier(0.4, 0.0, 1.0, 1.0)` | `ease-accelerate` |
| `--duration-micro` | `100ms` | `duration-micro` |
| `--duration-short` | `150ms` | `duration-short` |
| `--duration-medium` | `200ms` | `duration-medium` |

Reduced motion: a global `prefers-reduced-motion: reduce` block collapses every CSS
animation and transition to one 0.01ms frame. Infinite loaders run once and hold a
static end frame instead of flickering. Elements marked `data-motion-timer` are exempt
because their animation is a functional countdown, not decoration.
Motion (JS) animations must check reduced motion themselves (`useReducedMotion()`).

Motion presets live in `src/animations.ts` (identical in Synapt and SynaptClip):
`SPRING`, `SPRING_GENTLE`, `OVERLAY_ENTER`, `PANEL_ENTER`, `RESULT_ITEM(index)`, `FADE_IN`,
`INLINE_CONFIRM`, `ITEM_EXIT`. Import from there; do not define one-off springs in components.

The window is shown and hidden from Rust, never unmounted, so the root entrance is
replayed on every window focus by `useEntranceOnShow` (`src/hooks/useEntranceOnShow.ts`)
through animation controls rather than by re-mounting. App roots are wrapped in
`<MotionConfig reducedMotion="user">`, which drops transform animation under reduced
motion; height animation (`INLINE_CONFIRM`) checks `useReducedMotion()` itself.

Currently in use:

- Overlay root: `OVERLAY_ENTER`, replayed on focus
- Result rows: `RESULT_ITEM`, stagger capped at the first 6 rows, keyed by source and path
- Inline confirmation: `INLINE_CONFIRM` expand and collapse
- Device picker: scale 0.95 to 1 with opacity, `SPRING_GENTLE`
- Search loading bar, transfer card, indexing banner: `FADE_IN` (cards add an 8px rise) with exit fades
- `.panel-root` fade (150ms `--ease-standard`) is defined in CSS but not used by any component
- Transfer progress width: `200ms ease-out`
- Indexing progress width: `300ms ease-out`
- Underline loader: `underline-load 1.2s ease-in-out infinite`
- Search loading bar: `search-loading 1.2s linear infinite`
- Tailwind `transition-colors` (150ms, `--ease-standard` by default) on rows and buttons

Current code differs:

- No transition uses the specified cubic-bezier; all use `ease-out`, `ease-in-out`, `linear`, or Tailwind's default curve.
- The two looping loaders run 1.2s. They are indeterminate progress indicators, not transitions; under reduced motion they hold a static segment.

## Z-Index

- 50 (`z-50`) - pairing dialog
- 100 - device picker
- 200 - inline confirmation
- 1000 - Select popup

## Platform Considerations

### macOS

- Window is transparent and frameless
- No Dock icon (NSApplication activation policy: Accessory)
- Tray icon in menu bar - single icon shared with SynaptClip
- Corner radius on the overlay window: 8px CSS matches system expectation
- System accent colour not used - app uses its own accent token

The overlay is 680x480, centred, always on top, not resizable. `macOSPrivateApi` is
enabled so the transparent webview corners render correctly.

### Windows

- Window is transparent and frameless
- Tray icon in system tray notification area
- No window shadow (disabled to avoid rendering artifacts)
- Scrollbar styling: thin (4px vertical, 2px horizontal) via `::-webkit-scrollbar`, with `scrollbar-width: thin` as the fallback; applied globally

Current code differs:

- `shadow` is not set in `tauri.conf.json` and is not disabled at runtime, so the Tauri default (shadow on) applies.

### Linux

- X11: standard window compositing
- Wayland GNOME: requires AppIndicator extension for tray
- Wayland wlroots: native tray support
- No window decorations

## Component Patterns

### Result Row / List Item

- Height: 48px minimum
- Padding: 12px 16px
- Selected state: background `var(--surface-hover)`, left border 2px `var(--accent)`
- Hover state: background `var(--surface-hover)`
- No card borders between list items - separation by spacing only

Implemented by `.list-row` with `data-selected`. Metadata is plain 12px text: no badges.

### Section Heading

- Font size: 11px
- Font weight: 500
- Color: `var(--muted)`
- Text transform: uppercase
- Letter spacing: 0.06em
- Margin bottom: 8px

Implemented by `.section-heading`. Inside a flex column with `gap-2`, add `mb-0` so the gap supplies the 8px.

### Button - Primary

- Background: `var(--accent-fill)`
- Color: `var(--on-accent)`
- Border radius: 6px
- Padding: 6px 14px
- Font size: 13px
- Font weight: 500
- Hover: opacity 0.85

Implemented by `.btn .btn-primary`. The fill is `--accent-fill`, not `--accent`, so white text passes AA in the dark theme.

### Button - Secondary

- Background: transparent
- Color: `var(--text)`
- Border: 1px solid `var(--border)`
- Border radius: 6px
- Padding: 6px 14px
- Font size: 13px
- Hover: background `var(--surface-hover)`

Implemented by `.btn .btn-secondary`. A toggle in its on state (segmented tabs, hotkey recorder) adds the Active style: `--surface-hover` fill, `--accent` label.

### Button - Destructive

- Same as secondary but color: `var(--danger)`, border-color: `var(--danger)` at 0.4 opacity
- Hover: background `var(--danger)` at 0.1 opacity

Implemented by `.btn .btn-destructive`.

### Empty State

- Centered vertically and horizontally in the available space
- Primary text: `var(--text)`, 14px, font-weight 500
- Secondary text: `var(--muted)`, 13px, margin-top 4px
- No icons unless they directly aid comprehension
- No decorative elements

Implemented by the `EmptyState` component (`title`, optional `detail`, optional `action`).

### Input / Search Bar

- Background: `var(--surface)`
- Border: 1px solid `var(--border)`
- Border radius: 8px
- Padding: 10px 14px
- Font size: 14px
- Color: `var(--text)`
- Placeholder color: `var(--muted)`
- Focus: border-color `var(--accent)`, no box-shadow

Implemented by `.input`. The search bar is the full-width `.input` inside a `--bg` strip (`px-3 pb-2`) directly under the header, so header and search read as one surface.

### Inline Confirmation

- Same visual weight as secondary button
- Confirm action: `var(--accent)` text
- Cancel action: `var(--muted)` text
- Keyboard: Enter confirms, Escape cancels

Implemented by `InlineConfirm` with `tone="danger"` (destructive confirm) or the default tone (accent confirm), and a `.btn-ghost` Cancel.

## What Not To Do

- Do not use accent colour on card borders, section dividers, or status indicators
- Do not add decorative shadows to overlay cards or list items
- Do not use more than 2 font sizes within a single component
- Do not create new spacing values outside the approved scale
- Do not add border-radius values not listed in this document
- Do not use colour to convey information without a text label alongside it
- Do not add animations that exceed 300ms
- Do not use emojis anywhere in the UI
- Do not show a raw backend error, and do not let an action fail silently (see Error Messages)

Existing violations to clean up:

- The two indeterminate loaders loop every 1.2s (see Animation System). They are progress indicators, not transitions, and hold still under reduced motion.
- `src/components/PeerCard.tsx` (unused) still has an accent border and a pill badge; delete it or bring it in line before using it.

