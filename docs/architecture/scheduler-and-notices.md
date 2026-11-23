---
title: Scheduler and notices
description: Jobs, alarms, reminders, tasks, sounds and unasked notices.
order: 8
---

# Scheduler and notices

## Scheduler: core vs plugin

- The **Scheduler** is core. It is a timer that starts turns at set times (dreaming, morning brief, reminders that are due).
- The **`scheduler` plugin** is the guide and tools the model uses to create, list and cancel reminders and recurring jobs. It writes jobs; the core timer runs them.
- If the computer was asleep or off at the due time, the core runs the job once when it wakes, not once per missed slot. A missed reminder is shown as late. Times are stored with the user's timezone, so daylight saving does not move them.
- A turn started by the scheduler or a trigger has no user watching. It may use `auto` tools. Anything that needs a yes becomes a pending approval and the turn moves on (see [API](api.md), Approvals).

## Notices and tasks

- Core tool `notify(text, title?, silent?, priority?)`: the model tells the user something unasked. At most 5 per turn. `priority: urgent` goes through quiet hours; in a turn that read outside content it is sent as `normal` (the core decides, not the model).
- `GET /v1/notices?cursor=<n>&limit=` lists recent notices (`n`, `priority`, `title`, `text`, `silent`, `delivered`, `thread_id`, `created`) so a client that was offline can catch up.
- Job kind `task` (scheduler tools `schedule_task` and `schedule_recurring_task`): when due, the core starts an unattended turn in its own thread `system:task:<job>:<time>` with the task text.

  The turn works with its tools, writes a report as its last reply, and may call `notify`. The report stays in that thread, and a silent notice titled "Report ready" points to it. A task made after outside content is outside content each time it runs.

  Repeating tasks ask first and run at most every 15 minutes.

## Jobs (reminders, recurring work, alarms)

```
GET    /v1/jobs                             list scheduled jobs (with ring, sound, event_id, next_run_local)
POST   /v1/jobs                             set an alarm or reminder without the model (see below)
GET    /v1/jobs/{id}                        one job
PATCH  /v1/jobs/{id}                        change it: text|label, at|time, in_minutes, days, every ("once" stops repeating), weekday, ring, sound
DELETE /v1/jobs/{id}                        cancel a job
GET    /v1/alarms/ringing                   what is ringing now
POST   /v1/alarms/snooze                    {id?, minutes?}  stop the ring, ring again later (default 9 min)
POST   /v1/alarms/dismiss                   {id?}  stop the ring
GET    /v1/sounds                           built-in names plus custom ringtones
PUT    /v1/sounds/{name}                    upload a ringtone: the raw mp3, ogg, wav or m4a file, at most 3 MB
GET    /v1/sounds/{name}                    the ringtone file, for the app to play
DELETE /v1/sounds/{name}                    remove it
```

`POST /v1/jobs` bodies: `{"kind": "alarm", "time": "07:00", "days": "weekdays" | [1,3,5], "label": "Wake up", "sound": "bell"}` or `{"kind": "reminder", "text": "Call Sam", "in_minutes": 30 | "at": "2026-11-14 15:00", "ring": true, "sound": "chime"}`. `days` uses 0 = Sunday.

The model creates jobs through the `scheduler` plugin. A front end must be able to show, change and cancel them without asking the model.

A job keeps its own text. If it was made in a turn that held outside content, the job is marked `outside` and its text is wrapped as outside content each time it runs, so an injected "every day, do X" is data, not an order. Outside jobs never ring, and an alarm cannot be set from such a turn at all.

**Ringing.** An alarm is due: the core sends `alarm.ring` at once, with no model turn.

The app rings with that sound until the user snoozes or dismisses it (on any device: `alarm.stopped` tells the others), or until 10 minutes pass; then it is `missed` and one urgent notice says so.

An alarm more than 30 minutes late (the server was off) only sends that notice. A reminder sends `reminder.due` at once (the phone can show it right away), rings too if `ring` is set, and then the model's turn may add a notice with the same job.

Alarms ring in quiet hours and do not count toward the daily notice cap: the user set them.

**Sounds.** Built-in names (`default`, `chime`, `bell`, `beep`, `gentle`, `radar`) are sounds the app ships; their `sound_url` is null. A custom ringtone is a file the owner uploads; it is checked by its first bytes, kept in `data/sounds/`, never shown to the model and never transcribed. This is separate from phone capture, which still refuses all audio (08).

## More on jobs and notices

- Alarms and reminders on chosen dates: `create_alarm {time|times, dates|days, label}` (dates never repeat, days repeat weekly, not both), `create_alarms {alarms:[...]}` up to 30; each date is its own job; undo cancels the batch (`result.ids`).
- A held notice (quiet hours, daily cap) is replaced by a new delivered row when released, so a phone whose cursor already passed it still gets it. The phone keeps a posted-list so the page and the background check never show one twice.
- Jobs can be paused, resumed, run now or skipped for one day (`/v1/jobs/{id}/pause|resume|run|skip`). Scheduler tools: `pause_task`, `resume_task`, `run_task_now`, `skip_day`, `unskip_day`.
- A task can start on a phone notification: `run_task_on_notification` (asks first). The phone forwards only watched apps (`GET /v1/automations/watch`, `POST /v1/automations/notify`).
- Defaults in code: quiet hours `22:00-07:00` (`AGENT_QUIET_HOURS`), 10 notices a day (`AGENT_NOTICES_PER_DAY`).

Phone side: how alarms ring and notices show is in [Talking to the server](../android/talking-to-server.md). Place reminders are in [Location](location.md).
