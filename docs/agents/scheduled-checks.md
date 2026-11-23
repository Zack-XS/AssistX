---
title: Scheduled checks
description: Work the assistant starts on its own: jobs, tasks, the mind, the brief and watches.
order: 5
---

# Scheduled checks

Unattended turns have no person watching. They follow the same rules: tools that need a yes are not run (a card waits instead), outside content stays outside, an urgent notice from a turn that read outside content goes out as normal, and quiet hours and the daily cap hold.

| What | Starts | Page |
|---|---|---|
| Reminders, alarms, tasks | The core timer, at their time | [Scheduler and notices](../architecture/scheduler-and-notices.md) |
| Place reminders and tasks | Arriving or leaving a place | [Location](../architecture/location.md) |
| Tasks on a phone notification | A watched app's notification | [Scheduler and notices](../architecture/scheduler-and-notices.md) |
| Mind | Every 30 minutes | [Mind](../architecture/mind.md) |
| Morning brief | First wake of the day | [Mind](../architecture/mind.md) |
| Dreaming | Nightly, or when notes wait | [Dreaming](dreaming.md) |
| Ideas, leaving home, gym visits | The core timer and the phone | [Location](../architecture/location.md) |
| Leave-now alerts | Calendar events with a place | [Location](../architecture/location.md) |
| Triggers | `POST /v1/triggers/{source}` | [API](../architecture/api.md) |
| Health watch, auto reports | Every 5 minutes, on failures | below, [Audit](../architecture/audit.md) |

## Health watch

- Health watch (`src/status.rs`): every 5 minutes, in the background, the core checks each local model (is the model server up, is the model installed), the voice and the sandbox. A part that goes down gets one silent notice ("Something is down"), and one when it works again. Nothing on start.

## Long chats

Long chats are summed up before a turn. See [Prompt system](../architecture/prompt-system.md), Assembly order and budget.
