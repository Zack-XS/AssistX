---
title: calendar
description: The user's calendar - see, add, move and delete events, repeats, and reminders that can ring.
order: 1
---

# calendar

The user's calendar - see, add, move and delete events, repeats, and reminders that can ring.

- Backend: `core` (src/calendar.rs; events live in the database, reminders are scheduler jobs)
- Needs: nothing outside the core
- Quick-command words: ["calendar", "meeting", "meetings", "appointment", "appointments", "event", "events", "schedule"]
- Guide for the model: [guide.md](../../server/plugins/calendar/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/calendar/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `list_events` | reads | no |
| `find_events` | reads | no |
| `get_event` | reads | no |
| `add_event` | writes | no |
| `undo_add_event` | writes | no |
| `update_event` | writes | no |
| `skip_occurrence` | writes | no |
| `unskip_occurrence` | writes | no |
| `delete_event` | writes, deletes | yes |
| `restore_event` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Events with repeats and reminders (`event` jobs that follow the event, may ring). Next 24 h shown in the index.

## Routes

```
GET    /v1/calendar/events?from=&to=&q=     occurrences in a range (local time; default the next 7 days)
POST   /v1/calendar/events                  add: title, start, end?, all_day?, location?, notes?, repeat?, until?, remind?
GET    /v1/calendar/events/{id}             one event
PATCH  /v1/calendar/events/{id}             change only the fields given
DELETE /v1/calendar/events/{id}             delete it and its reminders
```

Times are local wall time (`2026-11-14 15:00`, or `2026-11-14` for all day) plus a zone. `repeat` is `none`, `day`, `week`, `weekdays`, `month` or `year`, with `until` a date. `remind` is minutes before:

`[10, 60]` or `[{"minutes": 10, "ring": true, "sound": "bell"}]`, up to 5. Each reminder is a job of kind `event` that follows its event: moving the event moves it, deleting the event cancels it, and the job itself cannot be changed directly.

The next 24 hours of events appear in the index, wrapped as outside content.
