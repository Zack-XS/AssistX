---
title: hevy
description: Your Hevy workout log - recent workouts, routines, exercises; log a finished workout.
order: 7
---

# hevy

Your Hevy workout log - recent workouts, routines, exercises; log a finished workout.

- Backend: `core` (src/hevy.rs; the core makes the calls and keeps the key)
- Needs: `network`: [api.hevyapp.com]; `secrets`: [HEVY_API_KEY]
- Guide for the model: [guide.md](../../server/plugins/hevy/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/hevy/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `workouts` | reads | no |
| `workout` | reads | no |
| `workout_count` | reads | no |
| `routines` | reads | no |
| `exercise_templates` | reads | no |
| `log_workout` | writes | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Your Hevy log through Hevy's API. Needs a Hevy Pro API key saved as secret `HEVY_API_KEY`. The API has no delete, so logging cannot be undone (it asks first; workouts are private unless told otherwise).
