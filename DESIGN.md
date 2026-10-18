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

Tailwind is not extended (`theme.extend` is empty), so tokens are applied through
inline `style` props as `var(--token)` rather than Tailwind colour classes.

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
| `--muted` | `#888888` | `#666666` | Secondary text, section headings, icons |

### Border

| Token | Dark | Light | Use |
|---|---|---|---|
| `--border` | `#333333` | `#e0e0e0` | Dividers, input and card outlines |

### Accent

| Token | Dark | Light | Use |
|---|---|---|---|
| `--accent` | `#3b82f6` | `#2563eb` | Primary buttons, active and selected states |
| `--accent-soft` | `#1f3a72` | `#dbeafe` | Low-emphasis accent fill (defined, not used in Synapt today) |

Tinted accent variants are produced with `color-mix(in srgb, var(--accent) N%, transparent)`.
Values in use: 15% (fill), 30% (border), 40% (border), 60% (icon).

### Semantic

| Token | Dark | Light | Use |
|---|---|---|---|
| `--danger` | `#ef4444` | `#dc2626` | Destructive actions, errors |
| `--success` | `#22c55e` | `#16a34a` | Completed states |
| `--warning` | `#f59e0b` | `#d97706` | Warning icon and text |
| `--warning-bg` | `#3a2e14` | `#fef3c7` | Warning banner fill |
| `--warning-border` | `#553c18` | `#fcd34d` | Warning banner border |
| `--warning-text` | `#fef08a` | `#78350f` | Text on warning banner |
| `--highlight-bg` | `#854d0e` | `#fde68a` | Search match highlight fill |
| `--highlight-text` | `#fef08a` | `#78350f` | Search match highlight text |

No token has the same value in dark and light.

### Hardcoded colours outside tokens

- `#fff` - text on accent-filled buttons (`InlineConfirm`, `DeviceMultiSelect`, `Overlay`, `Select`)
- `#f87171` - error text in `TransferCard`
- `rgba(0, 0, 0, 0.6)` - pairing dialog backdrop
- `rgba(0, 0, 0, 0.3)` - `Select` popup shadow
- `src/App.css` holds the unused Vite starter palette (`#0f0f0f`, `#f6f6f6`, `#646cff` and others). `App.tsx` and `App.css` are not imported anywhere.

## Typography

Font family: `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif`
Monospace: `monospace` (pairing and invite codes)

Scale:

- 10px - result kind badges, mode badge in the search bar, "connected" label in the header
- 11px - section headings, device picker header, transfer card metadata, indexing banner detail
- 12px (`text-xs`) - secondary text, Settings body text, buttons, inputs
- 13px - result names, device picker rows, confirmation message, Select options
- 14px (`text-sm`) - search input, empty-state messages
- 28px - invite code display in Settings (monospace)
- 32px - pairing verification code (monospace)

Font weights in use: 400 (default), 500 (`font-medium`; app result names, header title, headings in Settings and transfer history)

Line heights in use: Tailwind defaults only - 16px with `text-xs`, 20px with `text-sm`. No custom line heights.

Letter spacing in use: 0.05em (`tracking-wider`, Settings section headings), 0.06em (device multi-select header), 0.15em (invite code), 0.2em (pairing code)

## Spacing Scale

Base unit: 8px
Approved values: 4, 8, 12, 16, 24, 32, 48px
Do not use values outside this scale. If a value between these is needed, use the nearest approved value.

Off-scale values currently in use, flagged for cleanup in Prompt 2:

- 2px - `py-0.5`, `gap-0.5` (badges, text stacks)
- 3px - `padding: 3px 8px` (selected-device chip)
- 6px - `px-1.5`, `gap-1.5`, `marginTop: 6px`, `padding: 6px 16px` (indexing banner), `padding: 6px 12px` (Select option)
- 10px - `py-2.5` (result rows), `px-2.5`, `gap: 10px` (result icon gap, device picker, indexing banner), `padding: 10px 14px`
- 14px - `padding: 10px 14px` (device picker, transfer card)
- 20px - `gap-5` (Settings sections)
- 36px - `padding-left: 36px` (file action rows, aligns with result text)

## Border Radius

- 8px - window root, item cards in Settings, device picker, transfer card, pairing dialog; `0 0 8px 8px` for panels attached under a row
- 6px - inline confirmation container, Select popup
- 4px (`rounded`) - buttons, inputs, badges, chips, app icons, Select trigger
- 2px - progress bars
- 1px - underline loader
- 50% / `rounded-full` - 7px online/offline status dots (device picker, device multi-select, Settings bridge status)

Rule: 8px for containers that float or sit at window level, 4px for controls inside them,
6px only for small floating containers (confirmation, popup menu). Progress indicators use 1-2px.

## Elevation and Surfaces

Synapt uses background colour differentiation for elevation - no shadows, no borders between surface layers.

Surface hierarchy:

1. `--bg` - window root and header bar
2. `--surface` - search bar, cards, inputs, menus
3. `--surface-hover` - hovered and selected rows, badge fill

Current code differs:

- Layers are also separated by 1px `--border` lines: header bottom, search bar bottom, and between every result row.
- `Select` popup uses `box-shadow: 0 4px 12px rgba(0,0,0,0.3)`.
- Pairing dialog uses a `rgba(0,0,0,0.6)` backdrop.

## Interactive States

Every interactive element must implement these states consistently:

- Default: `--surface` fill or transparent, `--text` or `--muted` label
- Hover: `--surface-hover` fill
- Active/Pressed: `--surface-hover` fill, `--accent` label
- Focus: `--accent` - visible focus ring using accent colour at reduced opacity
- Disabled: opacity 0.4, no pointer events

Current code differs:

- List rows use selection-follows-hover: `onMouseEnter` moves the keyboard selection, and the selected row gets `--surface-hover`. There is no separate hover state.
- Buttons declare `transition-colors` but no hover colour, so they do not change on hover.
- No custom focus style exists. The overlay root and search input set `outline: none`; other controls fall back to the WebView default.
- Disabled buttons use `disabled:opacity-50` (0.5, not 0.4) and keep pointer events.

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

Currently in use:

- Panel fade-in: `opacity 120ms ease-out` (`.panel-root`)
- Transfer progress width: `200ms ease-out`
- Indexing progress width: `300ms ease-out`
- Underline loader: `underline-load 1.2s ease-in-out infinite`
- Search loading bar: `search-loading 1.2s linear infinite`
- Tailwind `transition-colors` (150ms, `cubic-bezier(0.4, 0, 0.2, 1)`) on rows and buttons

Current code differs:

- `motion` / Framer Motion is not a dependency yet.
- No transition uses the specified cubic-bezier; all use `ease-out`, `ease-in-out`, `linear`, or Tailwind's default curve.
- `prefers-reduced-motion` is not handled anywhere.
- The two looping loaders run 1.2s. They are indeterminate progress indicators, not transitions, but they still need a reduced-motion fallback.

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
- Scrollbar styling: thin (2px) using CSS scrollbar-width

Current code differs:

- `shadow` is not set in `tauri.conf.json` and is not disabled at runtime, so the Tauri default (shadow on) applies.
- Synapt has no custom scrollbar styling; result lists use the WebView default scrollbar.

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

Current code differs (`ResultList.tsx`):

- Padding is `px-3 py-2.5` (12px 10px), so rows are about 47px.
- Selected state is `--surface-hover` only, with no left accent border.
- Rows are separated by a 1px `--border` bottom border.
- Name is 13px (500 for apps), path is 12px `--muted`.
- File action rows use `padding: 8px 16px 8px 36px` and `--accent` text when selected.

### Section Heading

- Font size: 11px
- Font weight: 500
- Color: `var(--muted)`
- Text transform: uppercase
- Letter spacing: 0.06em
- Margin bottom: 8px

Current code differs (`Settings.tsx` `Section`): weight is 400, letter spacing is
`tracking-wider` (0.05em), spacing below is `gap-2` (8px).

### Button - Primary

- Background: `var(--accent)`
- Color: `#ffffff`
- Border radius: 6px
- Padding: 6px 14px
- Font size: 13px
- Font weight: 500
- Hover: opacity 0.85

Current code differs: accent-filled buttons (`InlineConfirm`, empty-state action) use
4px radius, `4px 12px` padding, 12px text, weight 400, and no hover. The Settings
`accentButton` style is not filled: `--surface-hover` fill with `--accent` text and an `--accent` border.

### Button - Secondary

- Background: transparent
- Color: `var(--text)`
- Border: 1px solid `var(--border)`
- Border radius: 6px
- Padding: 6px 14px
- Font size: 13px
- Hover: background `var(--surface-hover)`

Current code differs: Settings `subtleButton` uses `--surface` fill, 4px radius,
`px-2 py-1` (8px 4px), 12px text, and no hover.

### Button - Destructive

- Same as secondary but color: `var(--danger)`, border-color: `var(--danger)` at 0.4 opacity
- Hover: background `var(--danger)` at 0.1 opacity

Current code differs: Settings `dangerButton` keeps the `--border` colour on the border
and has no hover.

### Empty State

- Centered vertically and horizontally in the available space
- Primary text: `var(--text)`, 14px, font-weight 500
- Secondary text: `var(--muted)`, 13px, margin-top 4px
- No icons unless they directly aid comprehension
- No decorative elements

Current code differs (`Overlay.tsx`): primary text is 14px `--muted` at weight 400,
secondary is 12px, and the gap is `gap-1` (4px). Layout is `py-8 px-6`, centred.

### Input / Search Bar

- Background: `var(--surface)`
- Border: 1px solid `var(--border)`
- Border radius: 8px
- Padding: 10px 14px
- Font size: 14px
- Color: `var(--text)`
- Placeholder color: `var(--muted)`
- Focus: border-color `var(--accent)`, no box-shadow

Current code differs:

- The search bar is a full-width 48px strip (`px-3`, `gap-2`) with a bottom border only, no radius, and no focus state.
- Settings inputs use 4px radius, `px-2 py-1`, 12px text, and no focus state.

### Inline Confirmation

- Same visual weight as secondary button
- Confirm action: `var(--accent)` text
- Cancel action: `var(--muted)` text
- Keyboard: Enter confirms, Escape cancels

Current code differs (`InlineConfirm.tsx`): the container is `--surface` with a `--border`
outline, 6px radius, and `8px 12px` padding. Confirm is an accent-filled button with `#fff`
text. Arrow keys and Tab move focus between the two buttons.

## What Not To Do

- Do not use accent colour on card borders, section dividers, or status indicators
- Do not add decorative shadows to overlay cards or list items
- Do not use more than 2 font sizes within a single component
- Do not create new spacing values outside the approved scale
- Do not add border-radius values not listed in this document
- Do not use colour to convey information without a text label alongside it
- Do not add animations that exceed 300ms
- Do not use emojis anywhere in the UI

Existing violations to clean up:

- Status dots: Settings bridge status, device picker, and device multi-select (7px, `--accent` when active or online).
- Accent borders: result kind badges, mode badge, selected-device chip, Settings `accentButton`, file action panel border.
- Pill badges: result kind and mode badges (`text-[10px] px-1.5 py-0.5 rounded`).
- Animations over 300ms: the 1.2s loaders (see Animation System).
