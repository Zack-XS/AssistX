---
title: health
description: The user's body numbers per day from their phone - steps, sleep, heart rate, weight, workouts.
order: 6
---

# health

The user's body numbers per day from their phone - steps, sleep, heart rate, weight, workouts.

- Backend: `core` (src/health.rs; the Android app sends daily totals, the core keeps them)
- Needs: nothing outside the core
- Guide for the model: [guide.md](../../server/plugins/health/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/health/plugin.yaml)
- More: [Talking to the server](../android/talking-to-server.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `summary` | reads | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Daily body numbers sent by the Android app from Health Connect (steps, sleep, heart rate, weight, workouts). `POST /v1/health/days` (device or client token), `GET /v1/health/days`. Table `health_days`, one row per day and metric; raw readings stay on the phone.
