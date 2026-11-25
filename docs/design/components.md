---
title: Components
description: Buttons, cards, chat bubbles, lists, sheets, inputs and notices.
order: 8
---

# Components

Each part uses only [tokens](tokens.md). Class names in brackets are the page's names today.

## Buttons

| Kind | Look | Use |
|---|---|---|
| Primary | `ink` fill, `on-ink` text, `radius-full` | The one main action |
| Secondary (`.pill`) | `field` fill, `line` edge | Other actions |
| Ghost | no fill, `text` or `mute` | Low actions in menus and bars |
| Danger | `bad` text; `bad` fill only to confirm | Delete, stop |
| Icon | 40 round, icon `icon-sm`, hit area `touch-min` | Send, mic, more |

States: press scales to 0.98 (`dur-instant`). Disabled is 40% opacity. Focus shows a 2 px `ink` ring, 2 px away. Loading keeps the label and adds a small spinner, so the width does not jump.

## Cards (`.card`)

`card` fill, `radius-xl`, `space-6` padding, `shadow` (edge `line` in dark). A card holds one topic: next event, weather, a brief. Tapping the card opens it; small actions go on the right.

## Chat bubbles (`.me`, `.bot`)

| | You | Assistant |
|---|---|---|
| Fill | `ink` | `bubble` |
| Text | `on-ink` | `text` |
| Side | right | left |
| Corners | `radius-lg`, small tail corner bottom-right | `radius-lg`, tail bottom-left |
| Max width | 82% | 88% (full width for tables and code) |

- Bubbles of one speaker sit `space-4` apart; a change of speaker adds `space-7`.
- Tool steps (`.turn`) fold into a quiet one-line box with mono text. Open to see steps.
- Message actions (copy, retry) show on hover or long-press, in `mute`.

## Approval card

The assistant asks for a yes. `card` with `line` edge, a short title, what will happen, the risk in words, and two buttons: Primary "Yes" and Secondary "No". Plays the approval sound. Never auto-closes. See [Approvals](../agents/approvals.md).

## Lists (`.item`, `.setrow`)

- Row: `row-h` tall, icon `icon-md`, title `type-headline`, second line `type-small` in `mute`, chevron or value on the right.
- Grouped rows sit in one card with `line` dividers. Setting rows may be single cards.
- Swipe left for the main row action (delete, done), with a button for the same action.

## Sheets

From the bottom on compact screens, a centred dialog from medium up. `raised` fill, `radius-2xl` top corners, a small grab handle, `scrim` behind, `shadow-raised`. Drag down or tap the scrim to close. Use sheets for short tasks (new alarm, pick a model), not for full pages.

## Inputs

- Field: `field` fill, `radius-md`, `control-h` tall, label above in `type-caption`.
- Composer: a round field that grows to 6 lines; mic on the left, send on the right. Send turns `bad` and pulses while live.
- Switch (`.switch`): 50 × 30, on is `ink`. Segmented (`.seg`): `field` track, selected part is `card`.
- Errors: text in `bad` under the field, plus an icon. Never only a red edge.

## Notices

| Kind | Where | Look |
|---|---|---|
| Toast | Above the tab bar, 3-4 s | `ink` fill, one line, optional Undo |
| Banner | Top of a screen, until fixed | `card`, state icon, one action (update, offline) |
| Badge | On a tab icon | 9 px `bad` dot with `page` ring |
| Phone notification | System tray | Channels: normal, quiet, urgent. Monochrome small icon. See [App structure](../android/app-structure.md). |
| Ringing card | Full screen over the lock screen | Big time, two big buttons: Stop and Snooze |

## Tab bar and rail (`#tabs`)

Compact: a floating `card` pill, `radius-2xl`, four tabs (Today, Plan, Chat, You; view ids stay `home`, `plan`, `chat`, `settings`) with icon and caption. Medium and up: the same tabs as a left rail. See [Spacing and layout](spacing-and-layout.md).

## Parts and class names

One name per part. Code uses the class; docs and talk use the same name.

| Part | Class | Use for | States | Access |
|---|---|---|---|---|
| card | `.card` (+ `.cardhead`) | A group of related things | normal, loading, empty (`.empty`), error | `h3` heading inside |
| setrow | `.setrow` | A row that opens a sheet (You list) | normal, pressed | `button`; title, hint, chevron |
| item | `.item` | A row in a card (event, alarm, chat) | normal, pressed, done, off | `button` if it opens |
| pill | `.pill`, `.pill.solid`, `.pill.bad` | Actions; one `solid` per view | normal, pressed, disabled, busy | `button` with text |
| seg | `.seg` | 2-4 views of one thing | one `aria-pressed=true` | `role=group` + `aria-label` |
| toggle | `.tog` + `.switch` | On/off settings | on, off, disabled, saving | real checkbox with a label |
| field | `.field` | Text, number, time, select | normal, focus, error | `label` wraps the input |
| iconbtn | `.iconbtn` | Header actions | normal, pressed, badge | `aria-label` required |
| chip | `.chip` | Icon square before an item | — | `aria-hidden` |
| sheet | `#subpage` | Details, editors | opening, open, closing | Back has `aria-label="Back"`; title `h2` gets focus |
| approval card | `.apr`, `.apr.danger` | A yes the assistant needs | waiting, answering, done, void | `role=group` (`alertdialog` for danger) |
| bubble | `.me`, `.bot` | Chat messages | streaming, done, failed | `#log` is `role=log aria-live=polite` |
| composer | `#composer` | Write and send | idle, sending, live, offline | textarea has `aria-label` |
| banner | `#net`, `#updbar` | One line about the whole app | info, warn, bad | `role=status` |

- One `pill solid` per view: the main action.
- A danger action is `pill bad`. It asks first, or gives undo after.
- Lists over 50 rows load more on scroll.
- Model text in a bubble is drawn safely: see [Safety rules](../security/safety-rules.md#page).

## Gaps

- No sheets: the page uses full subpages and native dialogs.
- No toast; the update bar uses a blue fallback colour, not a token.
- Approval cards, ringing card and voice card each style buttons their own way. The ringing card buttons (`RingActivity.kt`) and voice card buttons (`AssistActivity.kt`) do not use these tokens.
- Swipe actions on rows are missing.
