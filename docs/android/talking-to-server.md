---
title: Talking to the server
description: Tokens, the live connection, uploads, and which screen calls which route.
order: 2
---

# Talking to the server

## Sign-in and tokens

- The app keeps the server address and the account token, sealed with a Keystore key (`Vault.kt`).
- After a server restart it unlocks the account with its device key (`Unlock.kt`). See [Accounts](../architecture/accounts.md).
- Background uploads may use a `device` token, which can only upload.

## Getting news from the server

- Phone notifications (AssistantX-Android `Notices.kt`): no Google services. The app polls `GET /v1/notices` with its saved cursor on the alarm-sync timer (inexact, about every 15 minutes) and when it opens; the first run only stores the cursor.

  Channels: normal (sound), quiet (`silent`: no sound or buzz), urgent (high). Tap opens the notice's `thread_id`. Nothing pops up while the app page is on screen (it shows them itself).
- While the location service runs, `NoticeStream.kt` keeps `GET /v1/events` open for instant notices and `fences.changed`.
- Alarms are copied into Android's alarm clock (`Alarms.kt`); `alarm.ring` and `alarm.stopped` keep devices in step.
- Phone actions are fetched with `GET /v1/phone/actions` and answered with `POST /v1/phone/actions/{id}` ([phone](../plugins/phone.md)).
- Battery: the events stream keep-alive is 55 s (was 15 s); the phone's 15-minute check is a re-arming `setAndAllowWhileIdle` alarm (the old inexact repeating alarm was put off for hours in Doze); location slows to 15 minutes after three points within 150 m.

## What the phone sends

| Route | Sent by | Page |
|---|---|---|
| `POST /v1/location/points` | `LocationService` | [Location](../architecture/location.md) |
| `POST /v1/location/transitions`, `GET /v1/location/fences` | `Fences` | [Location](../architecture/location.md) |
| `POST /v1/ambient/chunks` | `CaptureService` | [Phone capture](../architecture/phone-capture.md) |
| `POST /v1/health/days` | `HealthSync` | [health](../plugins/health.md) |
| `POST /v1/detected` | `Detect` | [Location](../architecture/location.md), Noticing |
| `POST /v1/automations/notify`, `GET /v1/automations/watch` | `NotifyListener` | [Scheduler and notices](../architecture/scheduler-and-notices.md) |
| `POST /v1/signals` | `Woke` | [Mind](../architecture/mind.md) |
| `GET /v1/app/version`, `GET /v1/app/apk` | `Updater` | [Updates](updates-and-release.md) |
| `POST /v1/threads/{id}/messages {live}`, `GET,POST /v1/speech` | `AssistActivity` | [Voice](../architecture/voice.md) |

## Which screen calls which route

`examples/dummy-client/index.html` is an optional reference client: the app's screens, shown by the Android app. It only calls the API; delete it and the backend still works. Layout rules are in [Screens](../design/screens.md). Use it to see which API call each screen needs:

| Screen | Calls |
|---|---|
| Home: today's overview and upcoming | `GET /v1/jobs` (reminders; done today vs due today, next three) |
| Home: noticed | `GET /v1/ambient/picks?status=pending`, `GET /v1/approvals?status=pending`, `GET /v1/memory` (entries under 0.5 confidence) |
| Home: phone capture | `GET /v1/ambient/status`, `POST /v1/ambient/pause`, `/resume` |
| Assistant | `POST /v1/threads`, `POST /v1/threads/{id}/messages`, `GET /v1/threads/{id}/events` (SSE: `message.delta`, `message.done`, `tool.*`, `turn.*`), `POST /v1/threads/{id}/cancel`, `GET /v1/threads/{id}/messages` |
| Settings: Privacy | `GET/PATCH /v1/ambient/settings`, `DELETE /v1/ambient/chunks`, `POST /v1/ambient/process` |
| Settings: Memory | `GET /v1/memory/folders`, `GET /v1/memory`, `PATCH/DELETE /v1/memory/{id}`, `POST /v1/memory/folders` |
| Settings: Integrations | `GET /v1/plugins`, `PATCH /v1/config` (per-tool `auto`, `ask`, `never`) |
| Settings: Intelligence | `GET /v1/config`, `POST /v1/dreaming/run`, `GET /v1/dreaming/runs` |
| Settings: Automation | `GET /v1/jobs`, `DELETE /v1/jobs/{id}`, `GET /v1/actions`, `POST /v1/actions/{id}/undo` |
| Settings: Devices | `GET/POST /v1/tokens`, `DELETE /v1/tokens/{id}` |

> Code check: the page now has four tabs (Home, Plan, Assistant, Settings), not three.
