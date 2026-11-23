---
title: Color
description: Colour tokens for light and dark, and how colour is used.
order: 2
---

# Color

AssistX is mostly grey, black and white. The strongest colour is **ink**: near-black in light mode, near-white in dark mode. It marks the main action and your own chat bubbles. One quiet **accent** is kept for the assistant being alive (listening, live, a suggestion). Green, amber and red only mean a state.

Light and dark follow the phone's setting, with a manual choice in Settings (`data-theme` on the page: `system`, `light`, `dark`).

## Tokens

All values come from [Tokens](tokens.md); this page explains them. Names match the CSS variables the page already uses (`--page`, `--card` ...). New tokens are marked **new**. Changed values are marked **changed** (for contrast; see [Accessibility](accessibility.md)).

### Surfaces

| Token | Light | Dark | Use |
|---|---|---|---|
| `page` | `#E9E9ED` | `#000000` | The back of every screen |
| `card` | `#FFFFFF` | `#141417` | Cards, rows, tab bar, menus |
| `raised` **new** | `#FFFFFF` | `#1C1C21` | Sheets and menus above cards |
| `field` | `#F2F2F5` | `#1C1C21` | Inputs, segmented controls, chips, tracks |
| `bubble` | `#E7E7EB` | `#1F1F24` | The assistant's chat bubble |
| `code` | `#F0F0F4` | `#0B0B0E` | Code blocks |
| `line` | `#E3E3E8` | `#26262B` | Thin dividers and outlines |
| `scrim` **new** | `rgba(12,12,16,.32)` | `rgba(0,0,0,.56)` | Behind a sheet |

### Text and ink

| Token | Light | Dark | Use |
|---|---|---|---|
| `text` | `#0C0C10` | `#F3F3F6` | Main text |
| `mute` **changed** | `#5E5E68` (was `#6B6B75`) | `#9B9BA5` | Labels, times, hints |
| `ink` | `#17171C` | `#F3F3F6` | Main buttons, your bubble, switches on |
| `on-ink` | `#FFFFFF` | `#0C0C10` | Text on ink |

### Accent

| Token | Light | Dark | Use |
|---|---|---|---|
| `accent` **new** | `#4B57D6` | `#9AA3FF` | The assistant is alive: listening, live mode, a suggestion, links in docs |
| `on-accent` **new** | `#FFFFFF` | `#0C0C10` | Text on accent |
| `accent-soft` **new** | `accent` at 12% | `accent` at 18% | Quiet fills: a suggested chip, a selected row |

One accent only. Never use it for a whole screen or a big block.

### State

| Token | Light | Dark | Use |
|---|---|---|---|
| `ok` **changed** | `#157A42` (was `#1E9B55`) | `#43C076` | Done, good, connected |
| `warn` **changed** | `#8A5D00` (was `#C58A0F`) | `#E2A826` | Needs a look soon |
| `bad` **changed** | `#B8291F` (was `#D1342F`) | `#FF6B63` | Error, danger, live recording, badge dot |

### Depth

| Token | Light | Dark |
|---|---|---|
| `shadow` | `0 1px 2px rgba(0,0,0,.05), 0 10px 28px rgba(0,0,0,.06)` | none (use `line` as a 1px edge) |
| `shadow-raised` **new** | `0 2px 6px rgba(0,0,0,.06), 0 18px 48px rgba(0,0,0,.14)` | `0 0 0 1px #26262B` |

Dark mode has no soft shadows. Depth there comes from lighter surfaces: `page` < `card` < `raised`.

## Rules

- Main action: `ink` fill with `on-ink` text. One per screen part.
- The assistant's voice and live states use `accent`, so "the assistant is doing something" always looks the same.
- State colours are for small things: a dot, a bar, an icon, short text. Always pair them with an icon or word, never colour alone.
- Pure black `page` in dark mode is kept: it saves power on OLED phones and makes cards float.
- Text must reach 4.5:1 on the surface it sits on. The table below shows which pairs pass.
- `ok` as text goes on `card`, `raised` or `field` only. On `page` and `bubble` it is 4.45:1 and 4.37:1, so there it is an icon or a dot, never text.

## Contrast

WCAG ratios for light text tokens. Dark tokens all reach 5.9:1 or more on every surface.

| Token (light) | `page` | `card` | `field` | `bubble` |
|---|---|---|---|---|
| `text` | 16.1 | 19.5 | 17.5 | 15.8 |
| `mute` | 5.29 | 6.41 | 5.74 | 5.20 |
| `accent` | 4.77 | 5.78 | 5.17 | 4.69 |
| `ok` | 4.45 (icon only) | 5.39 | 4.83 | 4.37 (icon only) |
| `warn` | 4.76 | 5.76 | 5.15 | 4.67 |
| `bad` | 5.14 | 6.22 | 5.57 | 5.05 |

`on-ink` on `ink` is 17.9:1. `on-accent` on `accent` is 5.78:1 light and 8.4:1 dark.

## Gaps

- `--accent` does not exist; only the update bar uses a fallback blue `#0A84FF` (white on it is 3.65:1, too low).
- The page still has the old values: light `mute` on `page` is 4.35:1, `ok` 3.57:1 and `warn` 2.99:1 on white, `bad` 4.09:1 on `page`. The new values fix this.
- `raised`, `scrim` and `shadow-raised` are missing; the chat menu uses `card` plus a dark-mode border instead.
- Native screens use their own colours: `RingActivity.kt` (`#2A2A30`, `#C8C8D0`), `AssistActivity.kt` (white with alpha). `res/values/colors.xml` has only `page` and `icon_bg`.
