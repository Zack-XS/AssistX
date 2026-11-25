---
title: Smart UI
description: How screens adapt to time, place and what you are doing.
order: 9
---

# Smart UI

Status: plan, not built. The page does not adapt yet beyond light and dark.

The assistant already knows the time, your plan, where you are and what you asked it to watch. The screen can use that to put the right thing first. It uses data the server already has: no new tracking.

## Signals

| Signal | Comes from |
|---|---|
| Time of day | The phone clock |
| Next events and alarms | [Calendar](../plugins/calendar.md), [Scheduler](../plugins/scheduler.md) |
| Place | [Location](../architecture/location.md) visits and places |
| Moving or still | Location speed (walking, driving) |
| Workout | [Health](../plugins/health.md), [Hevy](../plugins/hevy.md) |
| Open work | Pending approvals, running tasks |
| Device | Screen width, touch or mouse, keyboard open |
| Quiet | Phone on silent, night hours |

## What may adapt

| Context | Change |
|---|---|
| Morning | Today leads with the morning brief and the first event |
| An event soon | A "leaving in 12 min" card with directions on top |
| Driving | Big voice button, fewer words, no long lists |
| Workout | Timer and log first |
| Night | Dimmer, fewer notices, quiet sounds |
| Approval waiting | The approval sits at the top of Today and in the side pane |
| Desktop | Side pane shows context; keyboard shortcuts work |
| Composer | 2-3 suggested replies as `accent-soft` chips that fit the moment |

## Rules

- **Never move what is under the finger.** Reorder only when a screen opens, never while you look at it.
- **Same place for the same thing.** Tabs, the composer and Back never move.
- **Say why.** Each smart card has a small "why this" that names the signal ("you have Gym at 18:00").
- **Easy to undo.** Hide a card, or turn a kind of change off in You.
- **Few changes.** At most one smart card at the top of Today at a time.
- **No guessing on safety.** Smart UI never says yes to anything. Approvals stay manual.

## Gaps

- None of this is built. Today (now the Home tab) shows a fixed order of cards.
- Width only changes the frame, not the layout. See [Spacing and layout](spacing-and-layout.md).
