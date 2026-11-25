---
title: Color
description: Colour tokens for light and dark, and how colour is used.
order: 2
---

# Color

AssistX is monochrome: black, white and greys. There is no brand hue. The strongest colour is **ink**: near-black in light mode, near-white in dark mode. It marks the main action, your own chat bubbles and the assistant being alive. Green, amber and red are kept only to mean a state, never for decoration.

Light and dark follow the phone's setting, with a manual choice in Settings (`data-theme` on the page: `system`, `light`, `dark`). The docs site and the home page share one choice: change it on one page and every page follows.

## Tokens

All values come from [Tokens](tokens.md); this page explains them. Names match the CSS variables the page already uses (`--page`, `--card` ...). Values marked **changed** moved to the monochrome palette; the old value is in brackets.

### Surfaces

| Token | Light | Dark | Use |
|---|---|---|---|
| `page` **changed** | `#FAFAFA` (was `#E9E9ED`) | `#000000` | The back of every screen |
| `card` **changed** | `#FFFFFF` | `#111111` (was `#141417`) | Cards, rows, tab bar, menus |
| `raised` **changed** | `#FFFFFF` | `#1A1A1A` (was `#1C1C21`) | Sheets and menus above cards |
| `field` **changed** | `#F1F1F1` (was `#F2F2F5`) | `#1A1A1A` (was `#1C1C21`) | Inputs, segmented controls, chips, tracks |
| `bubble` **changed** | `#EDEDED` (was `#E7E7EB`) | `#1C1C1C` (was `#1F1F24`) | The assistant's chat bubble |
| `code` **changed** | `#F4F4F4` (was `#F0F0F4`) | `#0A0A0A` (was `#0B0B0E`) | Code blocks |
| `line` **changed** | `#E4E4E4` (was `#E3E3E8`) | `#262626` (was `#26262B`) | Thin dividers and outlines |
| `scrim` | `rgba(10,10,10,.32)` | `rgba(0,0,0,.56)` | Behind a sheet |

The greys are neutral: no blue or warm tint.

### Text and ink

| Token | Light | Dark | Use |
|---|---|---|---|
| `text` **changed** | `#0A0A0A` (was `#0C0C10`) | `#EDEDED` (was `#F3F3F6`) | Main text |
| `mute` **changed** | `#666666` (was `#5E5E68`) | `#8F8F8F` (was `#9B9BA5`) | Labels, times, hints |
| `ink` **changed** | `#0A0A0A` (was `#17171C`) | `#EDEDED` (was `#F3F3F6`) | Main buttons, your bubble, switches on |
| `on-ink` **changed** | `#FFFFFF` | `#0A0A0A` (was `#0C0C10`) | Text on ink |

### Accent

| Token | Light | Dark | Use |
|---|---|---|---|
| `accent` **changed** | `#0A0A0A` (was `#4B57D6`) | `#EDEDED` (was `#9AA3FF`) | Same as `ink`. Kept as a name so old code still works |
| `on-accent` **changed** | `#FFFFFF` | `#0A0A0A` | Text on accent |
| `accent-soft` **changed** | `#0A0A0A0F` | `#EDEDED1A` | Quiet fills: a suggested chip, a selected row |

There is no coloured accent. "The assistant is doing something" is shown with motion (a breathing dot, a moving wave), not with a hue.

### State

| Token | Light | Dark | Use |
|---|---|---|---|
| `ok` | `#157A42` | `#43C076` | Done, good, connected |
| `warn` | `#8A5D00` | `#E2A826` | Needs a look soon |
| `bad` | `#B8291F` | `#FF6B63` | Error, danger, live recording, badge dot |

These are the only colours in the product, and they only ever mean a state.

### Depth

| Token | Light | Dark |
|---|---|---|
| `shadow` **changed** | `0 0 0 1px #E4E4E4` (a hairline, no blur) | none (use `line` as a 1px edge) |
| `shadow-raised` **changed** | `0 0 0 1px #E4E4E4, 0 12px 32px rgba(0,0,0,.08)` | `0 0 0 1px #262626` |

Depth comes from hairlines first. Only things that float over the page (sheets, menus) get a soft shadow, and only in light mode.

## Rules

- Main action: `ink` fill with `on-ink` text. One per screen part. Second actions are outlined with `line` and turn to `ink` on hover or press.
- No colour for decoration. No gradients, no glows, no tinted icon tiles.
- State colours are for small things: a dot, a bar, an icon, short text. Always pair them with an icon or word, never colour alone.
- Pure black `page` in dark mode is kept: it saves power on OLED phones and makes cards float.
- Text must reach 4.5:1 on the surface it sits on. The tables below show that every text token passes on every surface.

## Contrast

WCAG ratios.

| Token (light) | `page` | `card` | `field` | `bubble` |
|---|---|---|---|---|
| `text` | 19.0 | 19.8 | 17.5 | 16.9 |
| `mute` | 5.50 | 5.74 | 5.08 | 4.90 |
| `ok` | 5.17 | 5.39 | 4.77 | 4.61 |
| `warn` | 5.52 | 5.76 | 5.10 | 4.92 |
| `bad` | 5.96 | 6.22 | 5.51 | 5.31 |

| Token (dark) | `page` | `card` | `field` | `bubble` |
|---|---|---|---|---|
| `text` | 17.9 | 16.1 | 14.9 | 14.6 |
| `mute` | 6.49 | 5.84 | 5.38 | 5.27 |
| `ok` | 9.04 | 8.13 | 7.49 | 7.34 |
| `warn` | 9.86 | 8.87 | 8.17 | 8.00 |
| `bad` | 7.54 | 6.78 | 6.25 | 6.12 |

`on-ink` on `ink` is 19.8:1 light and 16.9:1 dark.

## Gaps

- The app page (`server/examples/dummy-client/index.html`), the docs site and the native colours (`colors.xml`, `MainActivity.kt`, `RingActivity.kt`) use these values.
- Native screens still write their colours in code (`RingActivity.kt`, `AssistActivity.kt`) instead of reading `@color/ax_*` resources. See the [Design language rollout](../plans/design-language-rollout.md).
- `res/values/colors.xml` has only `page` and `icon_bg`; the other tokens are not Android resources yet.
