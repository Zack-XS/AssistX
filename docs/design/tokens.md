---
title: Tokens
description: One table of design values for the app page, Android and the docs site.
order: 11
---

# Tokens

The one list of design values. This table wins over every other page and over `server/docs/16-app-design.md`. The app page, the native Android screens and the docs site all use these names. Change a value here and in the token file together.

## Naming

| Place | Pattern | Example |
|---|---|---|
| Page and docs site (CSS) | `--name` | `--page`, `--space-6`, `--ease-out` |
| Android resources | `ax_name` (dots and dashes become `_`) | `@color/ax_page` in `values/` and `values-night/`, `@dimen/ax_space_6` |
| Kotlin / Compose (if used) | `Ax.name` in camel case | `Ax.page`, `Ax.space6` |

Plan: one source file, `design-tokens/tokens.json` (outside `docs/`), makes the CSS file, the Android `colors.xml` / `dimens.xml`, and this table. A small Rust program does it. See [Design language rollout](../plans/design-language-rollout.md). Until then, this table is the source.

## All tokens

Colour cells are light / dark. Sizes are px on the web (rem for type: px ÷ 16), dp on Android, sp for type on Android.

| Group | Token | Value | Use |
|---|---|---|---|
| Surface | `page` | `#FAFAFA` / `#000000` | Back of every screen |
| Surface | `card` | `#FFFFFF` / `#111111` | Cards, rows, tab bar |
| Surface | `raised` | `#FFFFFF` / `#1A1A1A` | Sheets and menus above cards |
| Surface | `field` | `#F1F1F1` / `#1A1A1A` | Inputs, segments, chips, tracks |
| Surface | `bubble` | `#EDEDED` / `#1C1C1C` | The assistant's chat bubble |
| Surface | `code` | `#F4F4F4` / `#0A0A0A` | Code blocks |
| Surface | `line` | `#E4E4E4` / `#262626` | Dividers and outlines |
| Surface | `scrim` | `rgba(10,10,10,.32)` / `rgba(0,0,0,.56)` | Behind a sheet |
| Text | `text` | `#0A0A0A` / `#EDEDED` | Main text |
| Text | `mute` | `#666666` / `#8F8F8F` | Labels, times, hints |
| Text | `ink` | `#0A0A0A` / `#EDEDED` | Main buttons, your bubble |
| Text | `on-ink` | `#FFFFFF` / `#0A0A0A` | Text on `ink` |
| Accent | `accent` | `#0A0A0A` / `#EDEDED` | Same as `ink`; no coloured accent |
| Accent | `on-accent` | `#FFFFFF` / `#0A0A0A` | Text on `accent` |
| Accent | `accent-soft` | `#0A0A0A0F` / `#EDEDED1A` | Quiet fills |
| State | `ok` | `#157A42` / `#43C076` | Done, good, connected |
| State | `warn` | `#8A5D00` / `#E2A826` | Needs a look soon |
| State | `bad` | `#B8291F` / `#FF6B63` | Error, danger, live, badge dot |
| Depth | `shadow` | see [Color](color.md#depth) | Card lift |
| Depth | `shadow-raised` | see [Color](color.md#depth) | Sheets, menus |
| Space | `space-1` … `space-10` | 2, 4, 6, 8, 12, 16, 20, 24, 32, 48 | See [Spacing](spacing-and-layout.md) |
| Radius | `radius-xs`, `-sm`, `-md`, `-lg`, `-xl`, `-2xl`, `-full` | 6, 10, 14, 20, 22, 30, 999 | See [Corners](spacing-and-layout.md#corners) |
| Size | `touch-min` | 44 (48 Android) | Smallest tap area |
| Size | `control-h`, `row-h`, `read-max` | 40, 52, 680 | Buttons, rows, reading width |
| Icon | `icon-xs`, `-sm`, `-md`, `-lg` | 16, 20, 22, 28 | `icon-md` is the default |
| Icon | `icon-stroke`, `icon-stroke-active` | 1.9, 2.4 | Line width |
| Type | `type-display` | 34/37 700 | Screen title |
| Type | `type-title` | 22/28 600 | Card and sheet title |
| Type | `type-headline` | 17/24 600 | Row and section title |
| Type | `type-body` | 16/23 400 | Normal text |
| Type | `type-small` | 14/20 400 | Second lines, tight buttons |
| Type | `type-caption` | 12/16 500 | Tab labels, times |
| Haptic | `haptic-tick`, `-click`, `-confirm`, `-reject`, `-heavy`, `-attention` | Android constants, see [Haptics](haptics.md#the-haptic-set) | One meaning each; page calls `hap(name)` |
| Type | `type-mono` | 13/19 400 | Code, tool steps |
| Type | `type-number` | 44/48 500 | Big numbers: timers, clock face |
| Font | `font-sans`, `font-mono` | Inter + system; ui-monospace, Menlo | |
| Duration | `dur-instant`, `-quick`, `-base`, `-slow`, `-page` | 80, 150, 240, 320, 420 ms | See [Motion](motion.md) |
| Easing | `ease-out`, `ease-in`, `ease-in-out`, `ease-spring` | (.2,.8,.2,1), (.4,0,1,1), (.7,0,.2,1), (.3,1.6,.5,1) | |
| Spring | `spring-snappy`, `-smooth`, `-bouncy` | 700/0.85, 380/1.0, 500/0.6 | Stiffness / damping ratio |
| Breakpoint | `bp-medium`, `bp-expanded`, `bp-large` | 600, 840, 1200 | See [Breakpoints](spacing-and-layout.md#breakpoints) |

Weights: 400, 500, 600, 700. Four, no more.

## Settled against doc 16

`server/docs/16-app-design.md` had other values. These are the ones that hold now.

| Thing | Doc 16 | Now |
|---|---|---|
| `mute`, `ok`, `warn`, `bad` (light) | `#5e5e68`, `#157a42`, `#8a5d00`, `#b8291f` | Same: 16's darker values (they fix contrast) |
| Accent | none | none: `accent` equals `ink` (monochrome) |
| Space | 4 8 12 16 20 24 32 | `space-*` (adds 2, 6, 48) |
| Radius | 8 12 16 22 999 | `radius-*` (6 10 14 20 22 30 999) |
| Type | adds 18 and 44 | 17 for headlines; 44 is `type-number` |
| Icon size | 24 | 22 (`icon-md`), drawn on a 24 grid |
| Sheet motion | 300 ms in, 220 ms out | `dur-slow` in, `dur-base` out |
| Tabs | Today, Plan, Chat, You | Same |

## Gaps

- No token file exists. The page has 13 colour variables in `:root` (plus `--shadow`); Android has only `page` and `icon_bg` in `res/values/colors.xml`.
- The app has no Compose. Native screens are plain Kotlin views, so the Android side of tokens is resources first.
