---
title: Icons
description: One soft, round icon set, sizes, states and animated icons.
order: 7
---

# Icons

## The set: Lucide

Use **[Lucide](https://lucide.dev)** for every icon. It is open (ISC licence), large, and drawn on a 24 grid with round ends and round joins. It matches our soft corners. The page already draws Lucide shapes in `ICONS` in `server/examples/dummy-client/index.html`, so nothing changes in look.

Do not mix sets. If Lucide lacks an icon, draw one on the same grid with the same stroke, and keep it with the others.

## Style

| Rule | Value |
|---|---|
| Grid | 24 × 24, 2 px safe edge |
| Stroke | 1.9 (2.4 when active) |
| Ends and joins | round |
| Fill | none, `currentColor` stroke; small dots may be filled |
| Colour | inherits text colour: `text`, `mute` or `ink` |

## Sizes

| Token | Size | Use |
|---|---|---|
| `icon-xs` | 16 | Inside chips and meta lines |
| `icon-sm` | 20 | Inside buttons and inputs |
| `icon-md` | 22 | Tab bar, list rows (the default) |
| `icon-lg` | 28 | Empty states, big actions |

The stroke stays 1.9 at every size, so small icons do not look thin.

## States

- Active tab: stroke 2.4, lifts 2 px and grows to 1.15 with `ease-spring`.
- Disabled: `mute` at 50%.
- An icon alone in a button always has a text label for screen readers.

## Meanings

One meaning per icon. One icon per meaning.

| Icon | Means | Icon | Means |
|---|---|---|---|
| `home` | Today; a place called Home | `calendar` | Calendar, events |
| `clock` | Alarms, time | `list` | Lists |
| `bolt` | Tasks | `chat` | Chat |
| `gear` | You | `bell` | Needs you, notices |
| `bulb` | Noticed | `check` | Done, Set up |
| `sliders` | Appearance | `mic` | Voice |
| `puzzle` | Skills | `brain` | Models |
| `folder` | Memory | `shield` | Privacy |
| `heart` | Health | `phone` | Access (phones, tokens) |
| `box` | Computers | `pin` | Location |
| `pulse` | Status | `map` | Memory map |

Action icons (`plus`, `trash`, `share`, `search`, `more`, `chev`, `back`, `x`, `camera`) mean the action they show. Icons live only in `ICONS`.

## Animated icons

Small, short, and only to show a change.

| Change | Motion |
|---|---|
| Mic → stop | morph shape, `dur-base` |
| Send appears | scale from 0.8 with `ease-spring` |
| Done | the check draws itself (stroke dash), `dur-slow` |
| Bell with a new notice | one small swing, then still |
| Menu → close | lines turn into an X |

How: on the web, animate the SVG `stroke-dashoffset`, `transform` or swap paths with a cross-fade. On Android, turn the same Lucide SVG into an `AnimatedVectorDrawable`. All icon motion stops under reduced motion.

## Gaps

- Icons are inline strings in the page. They are not shared with native screens; native buttons use text only.
- No animated icons yet, apart from the tab icon lift.
- The page still uses `folder` for Computers, `home` for Location, `bolt` for Status and `brain` for Memory map.
- App icon and notification icons are not checked against this style.
