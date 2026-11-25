---
title: Principles
description: The few rules every AssistX screen follows.
order: 1
---

# Principles

AssistX is a buddy who runs your life. The screen should feel calm, quick and personal. Not like a dashboard. Not like a default template.

## The rules

1. **Content first.** Your words, your plans and the assistant's answers are the loudest thing on screen. Frames, lines and labels stay quiet.
2. **Little colour.** The base is warm-cool greys, black and white. Colour means something: an action, a state, or the assistant being alive. If a colour means nothing, take it out. See [Color](color.md).
3. **Compact, not cramped.** Show more on one screen with tight, even spacing. Never shrink touch areas to do it. See [Spacing and layout](spacing-and-layout.md).
4. **One shape family.** Soft, large corners and round line ends everywhere: cards, bubbles, buttons, icons. See [Icons](icons.md).
5. **Motion explains.** Things move to show where they came from and where they went. Nothing moves just to look busy. Every move can be turned off. See [Motion](motion.md).
6. **Fluid across sizes.** One design that flows from a small phone to a wide desktop. More room shows more panes, not bigger buttons.
7. **Smart, but never surprising.** The screen may reorder or suggest based on context. It never moves what is under your finger, and it can always say why. See [Smart UI](smart-ui.md).
8. **Feel, not just look.** Light haptics and short sounds confirm actions. They are always optional. See [Haptics](haptics.md) and [App structure](../android/app-structure.md#sound-effects).
9. **Made by hand.** No stock gradients, no glowing purple, no emoji as icons, no generic card grids. Every value comes from [Tokens](tokens.md).
10. **Everyone can use it.** Contrast, text size, screen readers and reduced motion are part of the design, not extras. See [Accessibility](accessibility.md).
11. **One display name.** Screens never hard-code a name. They show one display name the user sets (default "Assistant"), read from one place. The project is AssistX; the app on screen is just "the assistant". See [Code style](../contributing/code-style.md) rule 2.

## Things we do not do

| Not this | Do this |
|---|---|
| Bright gradients and neon accents | Black, white and flat greys; no accent hue |
| Many font sizes and weights | Seven sizes (plus one for big numbers), four weights |
| Spinners for everything | Show the part that is ready; a calm typing dot for the rest |
| Pop-ups that block | Sheets and inline cards you can swipe away |
| A phone screen stretched on desktop | More panes side by side |
| Icons from mixed sets | One set, one stroke, one corner style |

## Gaps

- On desktop the page is a phone-sized frame in the middle of the screen. Rule 6 asks for real wide layouts.
- Native screens (`RingActivity.kt`, `AssistActivity.kt`) use their own hard-coded colours and sizes, not the tokens. Rule 9.
