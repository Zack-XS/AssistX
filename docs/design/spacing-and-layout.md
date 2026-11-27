---
title: Spacing and layout
description: Roomy spacing, corner sizes, the grid and layouts from phone to desktop.
order: 4
---

# Spacing and layout

## Spacing

A 4 px base. Roomy, not compact: nothing should feel crammed. Most gaps are 12, 16 or 20.

The page uses these values (Nov 2026 spacing pass):

| Where | Value |
|---|---|
| Screen side margin | 20 |
| Card padding | 20 top and bottom, 22 sides |
| Gap between cards | 16 |
| One row in a card or list (toggle, value, item, link) | 16 above and below; settings rows at least 64 tall |
| Section label | 30 above, 10 below |
| Title to the line under it | 3 |
| Label to its input | 8; between fields 18 |
| Chat bubbles | 12 × 16 inside, 16 apart |
| Body line height | 1.5 |

| Token | Value | Use |
|---|---|---|
| `space-1` | 2 | Hairline gaps (segmented control) |
| `space-2` | 4 | Icon to label inside a chip |
| `space-3` | 6 | Gap between chips, menu items |
| `space-4` | 8 | Gap inside a row, between bubbles of one speaker |
| `space-5` | 12 | Gap between rows and cards |
| `space-6` | 16 | Card padding, screen side margin on phones |
| `space-7` | 20 | Gap between bubbles of different speakers |
| `space-8` | 24 | Screen side margin on wide screens |
| `space-9` | 32 | Between sections |
| `space-10` | 48 | Big empty states |

## Corners

Large, soft corners. Inner corners are smaller than outer ones (outer = inner + padding).

| Token | Value | Use |
|---|---|---|
| `radius-xs` | 6 | Inline code, bubble tail corner |
| `radius-sm` | 10 | Small buttons, selects, menu items |
| `radius-md` | 14 | Inputs, tiles, tool-step boxes |
| `radius-lg` | 20 | Chat bubbles, setting rows |
| `radius-xl` | 22 | Cards |
| `radius-2xl` | 30 | Tab bar, sheets (top corners) |
| `radius-full` | 999 | Pills, chips, switches, round buttons |

## Size

| Token | Value | Use |
|---|---|---|
| `touch-min` | 44 (48 on Android) | Smallest tap area, even if the drawn thing is smaller |
| `control-h` | 40 | Buttons and inputs in compact density |
| `row-h` | 52 | One-line list row |
| `read-max` | 680 | Widest reading column |

## Breakpoints

They match Android's window size classes, so the phone, a tablet, a folded phone and the desktop page all switch at the same widths.

| Name | Width | Layout |
|---|---|---|
| Compact | < 600 | One column. Floating tab bar at the bottom. Sheets come up from the bottom. |
| Medium | 600-839 | Nav rail on the left. One main column, max `read-max`. Sheets become centred dialogs. |
| Expanded | 840-1199 | Rail + two panes: list and detail (chats and chat; Plan and the open day). |
| Large | ≥ 1200 | Rail + main + side pane. The side pane shows context: today's plan, the open approval, what the assistant is doing. |

## Grid

- Compact: 4 columns, 16 side margin, 12 gutter.
- Medium and up: 8 or 12 columns, 24 side margin, 16 gutter.
- Panes snap to columns. The chat column never passes `read-max`.
- Content is placed in panes, not stretched. Extra width adds a pane.

## Rules

- Keep clear of the camera cutout, system bars and the keyboard (the app passes `--sat`, `--sab`, `--sal`, `--sar`).
- The composer and tab bar stay in thumb reach on phones.
- Changing width never loses state: an open chat stays open when the rail appears.

## Gaps

- The page has one layout: max width 480. At 520 px and up it shows a phone-shaped frame (corner 34). There is no rail, no second pane, no side pane.
- Spacing and corners are hand-set per rule (corner values 4, 8, 9, 10, 12, 14, 16, 20, 22, 30, 34). They fit the scale apart from 9 and 34.
