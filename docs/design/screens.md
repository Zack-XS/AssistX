---
title: Screens
description: The four tabs, where every screen lives, the states each screen shows, and how to add one.
order: 12
---

# Screens

Where things live in the app, and how to add a screen. This page is folded from `server/docs/16-app-design.md`; this page wins.

It covers the page `server/examples/dummy-client/index.html` and the native Android screens: the voice screen (`AssistActivity`, `VoiceGlow`), the ringing screen (`RingActivity`), quick tiles (`Tiles.kt`), notifications (`Notices.kt`, `RingService.kt`) and the rescue dialog (`MainActivity.showRescue`). It does not cover `examples/panel/`. The backend never depends on any of it.

## Rules

1. **One home per thing.** Each feature lives in one place. Other places may link to it; they never copy its screen.
2. **Four tabs, no more.** A new feature joins one of them.
3. **Tokens, not values.** No hand-typed `#hex` or `px` in screen code. See [Tokens](tokens.md).
4. **Same parts everywhere.** Build from the parts in [Components](components.md#parts-and-class-names). A new part needs a row there first.
5. **Every screen shows every state** (below).
6. **No inline style.** Screen code does not set colours, sizes, gaps or corners in `style`. Use a class. Values known only at run time are fine (a progress width, the app's inset values).

## The four tabs

| Tab | View id (frozen) | What it is for | Segments |
|---|---|---|---|
| Today | `home` | What matters now: next things, what needs a yes, what was noticed, what is broken | — |
| Plan | `plan` | Things with a time or a list | Calendar · Alarms · Lists · Tasks |
| Chat | `chat` | Talking to the assistant, past chats | — |
| You | `settings` | How the assistant works, what it knows, this phone, the app | — |

View ids stay as they are: they are stored (`startView`) and opened by the app. Only labels change.

Below a tab, a detail opens as a **sheet**: full screen, back button, title. Sheets may open sheets. Nothing else floats over a screen except the approval card, the chat menu and confirm dialogs.

## Where every screen goes

"Links from" means another place may point to the home, never copy it.

| Now (where) | New home | Links from |
|---|---|---|
| Home tab (`index.html` `#v-home`) | Today | — |
| "Needs your attention" (bell, `subInsights`) | Today, card "Needs you" (open sheet for all) | Today header bell |
| Update bar (`renderUpdBar`) | Today, top banner | — |
| Plan tab: Calendar segment | Plan · Calendar | Today "Upcoming" |
| Plan tab: Alarms segment | Plan · Alarms | Today "Upcoming" |
| Plan tab: Lists segment | Plan · Lists | Today (lists with open items, count) |
| Assistant tab (`#v-chat`) | Chat | Tile "Chat", shortcut `new-chat` |
| Chats history (`#hist`, `subHistory`) | Chat, header button → sheet "Chats" | — |
| Chat menu: model for this chat, Send log | Chat, menu | — |
| Connect / account (`#connect`, `showConnect`, `showAccount`, `showKey`, `showForgot`) | Full-screen "Connect" (before any tab) and You · App · Account | — |
| Settings: power card | You, top card (on/off for everything) | — |
| Settings: Set up (checklist) | You · App · Set up | Today card until done |
| Settings: General | You · App · Appearance and You · App · Connection (split by what it holds) | — |
| Settings: Import schedule | Plan, "+" menu → "Import schedule" | — |
| Settings: Status | You · App · Status | Today banner when something is down |
| Settings: Voice | You · Assistant · Voice | — |
| Settings: Integrations | You · Assistant · Skills (segment "Plugins") | — |
| Settings: Recipes | You · Assistant · Skills (segment "Recipes") | Chat (after a recipe card) |
| Settings: Intelligence | You · Assistant · Models | Chat menu ("model for this chat" stays per chat) |
| Settings: Memory | You · Memory (segment "Folders") | Today "Needs you" (unsure facts) |
| Settings: Mind map | You · Memory (segment "Map") | — |
| Settings: Automation (reminders) | Plan · Tasks | Today "Upcoming" rows |
| Settings: Automation (recent actions, undo) | Today, card "Done lately" → sheet "Activity" | Chat (a tool step's undo) |
| Settings: Alarms (jump to Plan) | **removed**: Plan · Alarms is its home | — |
| Settings: Calendar (jump to Plan) | **removed**: Plan · Calendar is its home | — |
| Settings: Lists (jump to Plan) | **removed**: Plan · Lists is its home | — |
| Settings: Location | You · Devices · Location | — |
| Settings: Noticing | You · App · Developer (only with developer mode on) | — |
| Settings: Health | You · Devices · Health | — |
| Settings: Morning | **removed** (Nov 2026): a brief is an automation, made in Plan · Tasks | Today (brief card) |
| Time to leave (was on Morning) | Plan · Tasks, "Built in" | — |
| Settings: Computers | You · Devices · Computers  | — |
| Settings: Devices (phones, tokens) | You · App · Access | — |
| Settings: Privacy (phone capture) | You · Devices · Privacy | Today (capture paused/on) |
| Sounds (from Alarms) | Plan · Alarms, "Sounds" row | — |
| Accounts (admin) | You · App · Account | — |
| Usage panel (`examples/panel/`) | not in the app (separate page) | — |

You is one list in four groups, in this order. Search stays at the top.

| Group | Rows |
|---|---|
| Assistant | Models, Voice, Skills |
| Memory | Memory (Folders · Map) |
| Devices | Privacy, Location, Health, Computers |
| App | Set up, Appearance, Connection, Status, Access, Account, Developer (hidden until on) |

Check before the redesign ships: the old Settings rows, 3 Plan segments, Chats, Connect, Account, "Needs your attention" and the update bar all appear above. Four rows are removed on purpose: three were second doors, and Morning became an automation.

### Settings today (before the redesign)

The page still has the old tabs. Settings is one list in cards (`GROUPS` in the page):

| Group | Rows |
|---|---|
| (top) | General, Voice |
| Assistant | Intelligence, Memory, Mind map, Automation, Recipes, Integrations |
| Phone | Location, Health, Computers, Privacy |
| (bottom) | Noticing, Developer: only with developer mode on (tap Version 7 times in General) |

Setup steps, Alarms, Calendar, Lists, Status and Devices have no row but are still found by search (`HIDDEN`).

**Developer mode.** Simple pages first. Numbers, counts and manual buttons show only with developer mode on. Use `devOn()` for anything technical.

**Search.** Each page has a word list of what is on it (`TAGS`). Search also checks plugin names, summaries and tool names; a plugin hit opens Integrations. Small typos match ("calander" finds Calendar), but only when an exact search finds nothing. Each result says why it matched. No match offers "Ask the assistant". A new page needs a `TAGS` entry.

### What the main pages hold

| Page | Normal | Developer mode adds |
|---|---|---|
| Intelligence | Model and cloud choice at top; keys and per-job models fold away | Memory runs, stats, manual sort |
| Privacy | One Listening switch, places to listen at (`listen_places`), keep-for menu and delete, in one card | Counts, manual processing |
| Health | Today in three tiles, a short week with a steps bar | Every metric |
| Memory | Search, an "About you" card, a folder tree; tap a fact to move or forget it; doubtful facts get a "Not sure" tag | — |
| Mind map | "You" in the middle; branches People, Groups, Places, Things; dashed lines for links memory found. Tap a name to fly to it and light its links; tap a branch to fold it. Drag, pinch or scroll; +, − and fit buttons. | — |
| Automation | Your automations, then "Built in" (Time to leave) | — |

### Plan · Alarms

- Top card: when the next alarm rings ("in 8 h"), then quick alarms.
- A ringing alarm shows first, with a shaking bell and big Snooze and Stop.
- Every alarm in one grouped card: big time, label, days, a switch each. "New alarm" is the last row.
- Phone settings that break alarms are rows with "Fix". Sounds is a row; its page splits Yours and Built in.
- Edit alarm: round day buttons, plus Once, Weekdays, Weekends, Every day. A live "Rings in 9 h 52 min · Tomorrow" under the time. Delete is the last row.

### Plan · Lists

- One grouped card: each list with a progress bar and its open count. "To-do" and "New list" are rows at the bottom.
- A list: one add box with a round + button. "2 milk" or "milk x2" fills How many.
- Round checks: a tick fills the circle, crosses the text out, then the row slides to Done.
- A progress line ("2 of 5 done"). Done folds away; Clear empties it. Share, Rename and Delete are rows in a "List" card.

### Changes show at once

Switches, ticks and similar small changes update the screen first. The server is told in the background. On failure the change flips back and a toast says so.

## States every screen shows

| State | When | Looks like |
|---|---|---|
| Loading | Waiting for the API | The card's own skeleton lines. Never a blank screen over 300 ms. |
| Empty | Nothing to show | An `.empty` line that says what will appear, and one action ("Nothing coming up." + "Add event") |
| Error | The call failed | The server's plain message + "Try again" |
| Offline | Server not reachable | `#net` banner on top. Messages wait in `outbox` and send later. |
| Signed out | No token, or 401 | The Connect screen, before any tab |
| Off | Power is off (`GET /v1/power`) | Today shows one card, "Everything is stopped", with the switch |

## Voice, approvals and notices

- **Voice screen** (`AssistActivity`): see-through, dark, white text, glow at the bottom. Shows what you said, then the answer as it is spoken. A tap outside stops it.
- **Live mode in Chat:** the send button turns `bad` and pulses while listening.
- **Approval card:** above the composer. The core's `question` and `details` first; the model's reason under them, labelled "Why". High risk shows the exact command in a code box.
- **Notices:** in the app, a notice is a row in Today "Needs you", never a pop-up. On the phone, three channels: normal, quiet, urgent. Tap opens its chat.
- **Alarms** ring in `RingActivity`: big time, label, Snooze and Stop, dark.
- **Rescue dialog:** "The app did not start", with "Update now" and "Reload". Native Android style.

## Layout and safe areas

- The app passes bar sizes as `--sat`, `--sab`, `--sal`, `--sar`. Views keep clear of them.
- Page header: page title (`type-display`) on the left, up to two icon buttons on the right.
- Tab bar: floating, bottom, `radius-2xl`.
- Chat: messages scroll. Approval cards, the offline banner and the composer stay at the bottom.
- Widths: see [Spacing and layout](spacing-and-layout.md#breakpoints).

## Words

- Short sentences. Common words. Say what happens: "Turn off to stop everything."
- Titles: one or two words, sentence case ("Needs you").
- Buttons: a verb ("Add event", "Try again", "Snooze").
- Errors: what went wrong and what to do. No codes on screen.
- Talk to the user as "you". The assistant says "I".
- No product name on screen. Use the display name ([Principles](principles.md) rule 11).

## Add a screen or a feature

1. **Pick its one home** from the table above, and add a row. If it fits nowhere, it goes in You.
2. **Name the routes** it calls ([API](../architecture/api.md)).
3. **Build it from parts.** No new colour, size or corner.
4. **Show every state.**
5. **Phone features:** add the bridge method on both sides and to the frozen list in [App structure](../android/app-structure.md#bridge-contract-frozen). Safety: [Safety rules](../security/safety-rules.md#android).
6. **Words:** title one or two words; one short hint line.
7. **Test** (below), and update this page in the same change.

## Test a page change

There are no page unit tests. Each change passes these by hand:

1. **It parses.** The release step checks the script.
2. **Bridge kept.** No name in the frozen lists may disappear:
   ```sh
   grep -o "NativeApp\.[a-zA-Z]*\|NA\.[a-zA-Z]*" examples/dummy-client/index.html | sed 's/.*\.//' | sort -u
   grep -o "window\.__[a-zA-Z]*" examples/dummy-client/index.html | sort -u
   ```
3. **In a browser** against a test server: every tab and sheet opens with no console error.
4. **On the phone:** no rescue dialog; tabs, a sheet, a chat with a tool call and an approval, a shortcut and a tile all work.
5. **Screenshots,** light and dark, each tab at phone width, before and after.
6. **Reduced motion** on: nothing slides.
7. **TalkBack:** every button reads a name.
8. **Safety grep:** the diff has no `innerHTML` with data, `eval`, `new Function`, `<iframe` or `<script src`.

## Not decided

- Whether Mind map stays its own row or becomes a segment of Memory.
- A skeleton style for loading cards.
- Whether `examples/panel/` folds into You · App · Status later.

## Gaps

- The page still has the old tabs: Home, Plan, Assistant, Settings.
- About 100 inline `style` uses remain in the page.
