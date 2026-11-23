---
title: Mind and morning brief
description: The quiet task that thinks every 30 minutes, and the morning brief.
order: 17
---

# Mind and morning brief

Status: built. `src/mind.rs`.

The assistant thinks on its own, often, without being asked, and pieces things together across time.

## How it works

- A ready-made quiet task, **Mind**, runs every 30 minutes on the 9B model (`schedule: {every_minutes: 30, quiet: true, mind: true, model: "local"}`). It is made once (`mind.seeded`) and is an ordinary task: change its pace or model, or cancel it, in Automation. `AGENT_MIND=off` stops it being made.
- Each run starts with a **digest** of what changed since the last run, built by the core with no model call: where the user is and recent visits, the calendar for the next 24 hours, the latest health days, notifications sent (with the owner's ratings), what the user said to the assistant, what is coming up, unsorted notes, and its own recent journal.

  This keeps a run light; the model digs deeper with its normal tools only when something needs it.
- It has two extra tools, only in a Mind run: `journal_add {text, kind}` (observation, connection, question, plan, decision) and `journal_read {days?, limit?, kind?}`. The journal (`mind_journal`) is how a thought from one run reaches the next.
- It acts with its normal tools when clearly useful and safe (notes, reminders) and speaks only through `notify`, rarely. A quiet task sends no "Report ready" pointer.

## What stays enforced ([Threat model](../security/threat-model.md))

The mind has no extra powers. Tools that need a yes are not run unattended; outside content stays outside; urgent notifications from a run that read outside content are sent as normal; the daily notification cap and quiet hours hold.

Its prompt tells it never to spend, send, delete or guess. Everything it does is in the audit trail ([Audit](audit.md)): each run, its digest, the data it pulled, and its journal appear in the review pack under "The mind's journal".

## Tuning it

Give an agent the review pack (`assistantx-review`) and ask what the mind got right and wrong. Its instructions are `mind::GUIDE` and the task text in `mind::ensure`; its pace and model are the task's settings.

## Morning brief

`src/signals.rs`.

The phone sends `POST /v1/signals {kind: "woke", at, source, off_secs?}` (device, client or owner token) the first time each day it sees the user wake (`Woke.kt`): the first unlock after the screen was off 4 hours or more, between 03:00 and 12:00; dismissing an alarm between 03:00 and 12:00; or a Health Connect sleep session that ended this morning.

The server keeps every signal (`signals`, `GET /v1/signals`) and checks the same rule (`signals::is_wake`).

The first wake of a local day (`brief.last_day`) starts the task **Morning brief** (made when first needed, `schedule: {on: "wake", brief: true, quiet: true, model: "local"}`).

The core first fetches today's weather for the phone's last point from Open-Meteo (`api.open-meteo.com`, by coordinates), then gives the run today's facts: weather, today's calendar, sleep from the latest health day, alarms and reminders still to come today, and open to-do counts when a `list_items` table exists (read softly; any other shape is skipped).

Weather, calendar and job text are wrapped as outside content. The run's reply is sent by the core as a normal notice titled "Good morning" that opens the run's thread; when the model gives nothing, the core sends the facts in plain words.

Settings: `GET,PATCH /v1/brief {enabled, parts: {weather, calendar, sleep, reminders, lists}, fallback: "08:30" | null}` (on, all parts, no fixed time by default). With `fallback` the core timer sends the brief at that time (until noon) on days no wake came. `POST /v1/brief/run` sends it now (the settings page's "Send it now"). The page is Settings, Morning.
