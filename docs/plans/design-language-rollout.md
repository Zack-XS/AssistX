---
title: Design language rollout
description: Steps to bring the Android app and its page onto the design language.
order: 3
---

# Design language rollout

Status: plan, mostly not built. Toasts (step 8) and a spacing pass (part of step 4, real values, not yet `space-*` variables) shipped in Nov 2026.

The app is the WebView page (`server/examples/dummy-client/index.html`, copied to `android/app/src/main/assets/index.html`) plus native Kotlin screens. There is no Compose. Most of the look lives in the page, so most steps are page steps. Each step is a small change that ships alone and keeps the app working. The gaps are listed at the end of each [Design](../design/README.md) page.

## Steps

1. **Token file.** Add `design-tokens/tokens.json` with every value in [Tokens](../design/tokens.md). A small Rust program (a binary in the repo, no Node or Python) makes `tokens.css`, `res/values/ax_tokens.xml`, `res/values-night/ax_tokens.xml` and `dimens.xml`. Keep today's names (`--page`, `--card` ...) so nothing breaks.
2. **Page colours.** Put the made CSS in the page's `:root` and dark block. Add `accent`, `raised`, `scrim`, `shadow-raised`. Apply the contrast fixes (`mute`, `ok`, `warn`). Make the update bar use `accent`.
3. **Font.** Bundle Inter (woff2, Latin subset) in the page. Drop `Space Grotesk`. Move font sizes to the seven type tokens, in rem, so the phone's text size works.
4. **Spacing and corners.** Replace hand-set values with `space-*` and `radius-*`. Remove the odd 9 and 34.
5. **Motion.** Replace hand-written curves and times with `dur-*` and `ease-*` variables. Check every animation is inside the reduced-motion block.
6. **Icons.** Keep `ICONS` but check each against Lucide and set sizes with `icon-*`. Add the animated icons: mic → stop, send, done check.
7. **Native screens.** Move `RingActivity.kt`, `AssistActivity.kt` and `VoiceGlow.kt` to `@color/ax_*` and `@dimen/ax_*`. Voice and live states use `accent`. Notification icons follow [Icons](../design/icons.md).
8. **Components.** Add sheets and swipe row actions to the page (toasts are in). Unify the approval card, ringing card and voice card buttons with [Components](../design/components.md).
9. **Wide layouts.** Replace the phone frame at 520 px with the breakpoints: rail at 600, list and detail at 840, side pane at 1200.
10. **Accessibility pass.** TalkBack on native screens, 200% text, keyboard on desktop, row text selectable on desktop.
11. **Smart UI.** Last, once the layout is stable: the "why this" card on Home, context cards and suggested replies, behind a Settings switch. See [Smart UI](../design/smart-ui.md).

## Per step

- Screenshots before and after, light and dark, phone and desktop width.
- The page contract stays the same (`NativeApp.*`, `window.__*`). See [App structure](../android/app-structure.md#the-page).
- Update the gap list on the design page that the step closes, and [Done](../roadmap/done.md).

## Later choice

If native screens grow, Compose could replace plain views. The token file can make an `Ax` Kotlin object for it. Not needed for these steps.
