---
title: Typography
description: The font, the type scale and how text is set.
order: 3
---

# Typography

One font family: **Inter**. It is clear at small sizes, has tabular numbers for times, and has a calm, even shape. Code and machine text use the system monospace.

| Role | Family |
|---|---|
| Sans | `Inter`, then `ui-sans-serif, system-ui, -apple-system, sans-serif` |
| Mono | `ui-monospace, Menlo, monospace` |

## Scale

Seven sizes. Sizes are in px on the web and sp on Android (they grow with the phone's text size).

| Token | Size / line | Weight | Use |
|---|---|---|---|
| `type-display` | 34 / 37 | 700 | Screen title (Today, Plan, Chat, You) |
| `type-title` | 22 / 28 | 600 | Card title, sheet title |
| `type-headline` | 17 / 24 | 600 | Row title, section title |
| `type-body` | 16 / 23 | 400 | Chat text, normal text |
| `type-small` | 14 / 20 | 400 | Second line of a row, chips, buttons in tight places |
| `type-caption` | 12 / 16 | 500 | Tab labels, times, meta |
| `type-mono` | 13 / 19 | 400 | Code, tool steps, tokens |

Four weights: 400 normal, 500 for row titles and captions, 600 for headings and buttons, 700 for screen titles and the active tab.

## Rules

- **Tight titles.** Display and title use letter spacing `-0.02em`. Body uses `0`. Caption uses `+0.01em`.
- **Numbers line up.** Times, counts and money use tabular numbers (`font-variant-numeric: tabular-nums`).
- **Short lines.** Reading text stays under about 68 characters wide, even on desktop.
- **Weight before size.** To stand out, change weight or colour (`mute`) first. Add a new size last.
- **Sentence case** for every label and button. No all-caps blocks.
- **Large text.** Layouts must work at 200% text size: text wraps, nothing is cut. See [Accessibility](accessibility.md).
- Big numbers (timers, the alarm clock face) use `type-number` (44 / 48, 500). It is the only size outside the seven.

## Gaps

- Inter is named but not bundled. On the phone the page falls back to the system font, so the look differs per phone. The page also names `Space Grotesk`, which is not loaded.
- The page uses 16 font sizes (11 to 48 px). They map to the scale above, apart from clock faces.
- Native screens set sizes by hand (`AssistActivity.kt`: 28 and 18).
