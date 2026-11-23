---
title: scheduler
description: Reminders by time or place, repeating jobs, alarms that ring, and tasks that start the assistant later.
order: 13
---

# scheduler

Reminders (by time or when you arrive at or leave a place), repeating jobs, alarms that ring, and tasks that start the assistant later to work on its own and report.

- Backend: `core` (the core timer runs jobs; these tools write them ([Overview](../overview/what-is-assistx.md), Scheduler))
- Needs: nothing outside the core
- Quick-command words: ["remind", "reminder", "reminders", "alarm", "alarms", "timer", "wake me", "snooze", "when i get home", "when i arrive", "when i leave", "when i get to"]
- Guide for the model: [guide.md](../../server/plugins/scheduler/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/scheduler/plugin.yaml)
- More: [Scheduler and notices](../architecture/scheduler-and-notices.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `create_reminder` | writes | no |
| `create_recurring` | writes | yes |
| `schedule_task` | writes | no |
| `schedule_recurring_task` | writes | yes |
| `run_task_on_notification` | writes | yes |
| `remind_at_place` | writes | no |
| `run_task_at_place` | writes | yes |
| `skip_day` | writes | no |
| `unskip_day` | writes | no |
| `known_apps` | reads | no |
| `pause_task` | writes | no |
| `resume_task` | writes | no |
| `run_task_now` | writes | no |
| `create_alarm` | writes | no |
| `create_alarms` | writes | no |
| `cancel_job_undo` | writes | no |
| `update_job` | writes | no |
| `restore_job` | writes | no |
| `list_jobs` | reads | no |
| `list_sounds` | reads | no |
| `snooze_alarm` | writes | no |
| `dismiss_alarm` | writes | no |
| `cancel_job` | writes | no |
| `uncancel_job` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Jobs after outside content are marked `outside`; they never ring and cannot be alarms.
