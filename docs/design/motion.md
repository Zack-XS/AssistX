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
- **Live / listening:** a slow breath (1.4 s) on `ink`. The voice ring follows sound level.
- **Theme change:** a cross-fade with view transitions, `ease-in-out`. The old look does not shrink, root images do not blend, and the phone's bars change halfway. Row-to-page animations are cancelled when each transition ends, so no edge of the other colour shows.
- **Row to page:** a tapped row grows into its page (Settings rows, a list). A page closed another way still slides out.
- **No flicker (`settle()`):** a screen swaps content only when it changed. A refresh with the same data does nothing. A changed refresh swaps with no entrance. A tab waits up to 300 ms for its content, then plays the entrance once. Home gathers its top cards before it draws, so nothing pops in later.
- **List tick:** the round check fills, the text is crossed out, then the row slides to Done.
- **Ringing alarm:** the bell shakes on the top card.
- **Mind map:** branches grow in when it opens. Tapping a name flies to it.
- **Toast:** rises in, fades out after about 3 s.
- **Docs site and home page:** sections rise 16 px and fade in as they scroll into view (`dur-page`, `ease-out`, 40 ms apart). Moving between pages fades the old page out in `dur-quick` and the new one in with `dur-base`.
- **Haptics** go with motion: a click on press, a confirm when an answer finishes. See [Haptics](haptics.md).

## Reduced motion

When the system asks for less motion (`prefers-reduced-motion`, Android "Remove animations"):

- No moves, scales or overshoot. Use a fade of `dur-quick` or nothing.
- No loops (typing dots, live pulse, glow breath, bell shake). Show a still state.
- The Mind map opens already grown.
- Streaming text still appears; it just does not animate.

## Gaps

- The page uses these curves already, but writes them by hand in each rule. No duration or easing tokens.
- No springs: the page has no sheets or drag yet.
- Native screens only use the system fade (`VoiceWindowAnim`). `RingActivity.kt` has no motion tokens.
