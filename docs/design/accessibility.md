---
title: Accessibility
description: Contrast, touch size, text size, screen readers and motion.
order: 9
---

# Accessibility

Everyone should be able to use AssistX by touch, voice, keyboard or screen reader.

## Rules

| Area | Rule |
|---|---|
| Contrast | Text 4.5:1, big text (≥ 22 or bold ≥ 17) 3:1, icons and edges that matter 3:1. Check light and dark. |
| Colour | Never the only sign. Pair state colour with an icon or word. |
| Touch | Tap areas at least 44 × 44 (48 on Android), 8 apart. |
| Text size | Sizes in sp / rem. Works at 200%: wraps, no cut text, no lost buttons. |
| Screen readers | Every icon button has a label. Chat answers are read when finished, not word by word. Approval cards are announced. |
| Focus | Visible 2 px ring (`:focus-visible`). Order follows the screen. Sheets trap focus and give it back on close. |
| Keyboard | On desktop: Tab moves, Enter sends, Esc closes, `/` jumps to the composer. |
| Motion | Follow reduced motion. See [Motion](motion.md). |
| Sound and haptics | Always optional, and never the only signal. |
| Voice | Spoken answers are also shown as text. |
| Language | Plain, short words, the same as these docs. |

## Check before a change ships

1. Light and dark contrast of new colours.
2. TalkBack on Android: every control has a name.
3. 200% text size on a small phone.
4. Reduced motion on.
5. Keyboard only on desktop.

## Gaps

- Light `mute`, `ok`, `warn` and `bad` are below 4.5:1 in the page today. [Color](color.md#contrast) gives new values and the ratios.
- The page turns off text selection for most of the screen (`user-select:none`); only bubbles and inputs can be selected. Fine for touch, but desktop users cannot copy row text.
- The page uses px sizes, so it may not follow the phone's text size inside the WebView.
- Native screens are not checked with TalkBack.
- No keyboard shortcuts yet.
