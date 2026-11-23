---
title: Motion
description: Durations, easing, springs, page moves and reduced motion.
order: 5
---

# Motion

Motion is quick and soft. Things come in fast and settle gently. Motion shows where something came from; it is never decoration.

## Durations

| Token | Value | Use |
|---|---|---|
| `dur-instant` | 80 ms | Press feedback (scale to 0.98) |
| `dur-quick` | 150 ms | Colour, opacity, switches, hover |
| `dur-base` | 240 ms | Small moves: chips, menus, icon changes |
| `dur-slow` | 320 ms | Tab change, subpage in, sheet up |
| `dur-page` | 420 ms | Content rising in on a fresh screen |

Exits are shorter than entries: a subpage slides in with `dur-slow` (320 ms) and out with `dur-base` (240 ms). Use only these tokens; no other times.

## Easing

| Token | Curve | Use |
|---|---|---|
| `ease-out` | `cubic-bezier(.2,.8,.2,1)` | Things coming in. The default. |
| `ease-in` | `cubic-bezier(.4,0,1,1)` | Things leaving |
| `ease-in-out` | `cubic-bezier(.7,0,.2,1)` | Theme change, big moves across the screen |
| `ease-spring` | `cubic-bezier(.3,1.6,.5,1)` | Small playful overshoot: active tab icon, a check appearing |

## Springs

For drag and fling (sheets, swipe to dismiss) use springs, so motion follows the finger.

| Token | Stiffness | Damping ratio | Use |
|---|---|---|---|
| `spring-snappy` | 700 | 0.85 | Chips, toggles, icon morphs |
| `spring-smooth` | 380 | 1.0 | Sheets, panes, page swipe |
| `spring-bouncy` | 500 | 0.6 | Only for delight: a done check, a new badge |

On Android these map to `SpringForce` (or Compose `spring()`). On the web, use the cubic curves above or a small spring helper.

## Patterns

- **Screen in:** fade + 0.985 scale + small sideways move, `dur-slow`, `ease-out`. Cards rise 16 px, each 40 ms after the last, up to 200 ms.
- **Subpage:** slides from the right in `dur-slow`; Back slides it out in `dur-base`.
- **Sheet:** rises from the bottom with `spring-smooth`; drag down to close.
- **Chat:** your bubble pops up from the composer. The assistant's text streams in; no per-word animation. The typing dots pulse in opacity only.
- **Live / listening:** a slow breath (1.4 s) on the accent. The voice glow follows sound level.
- **Theme change:** a cross-fade with view transitions, `ease-in-out`.
- **Haptics** go with motion: a light tick on press, a double buzz when an answer finishes.

## Reduced motion

When the system asks for less motion (`prefers-reduced-motion`, Android "Remove animations"):

- No moves, scales or overshoot. Use a fade of `dur-quick` or nothing.
- No loops (typing dots, live pulse, glow breath). Show a still state.
- Streaming text still appears; it just does not animate.

## Gaps

- The page uses these curves already, but writes them by hand in each rule. No duration or easing tokens.
- No springs: the page has no sheets or drag yet.
- Native screens only use the system fade (`VoiceWindowAnim`). `RingActivity.kt` has no motion tokens.
