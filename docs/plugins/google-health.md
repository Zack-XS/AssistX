---
title: google-health
description: Your Fitbit and Pixel Watch data via Google - steps, heart rate, sleep, exercise, weight and more. Read only.
order: 5
---

# google-health

Your Fitbit and Pixel Watch data via Google - steps, heart rate, sleep, exercise, weight and more. Read only.

- Backend: `core` (src/google_health.rs; the core makes the calls and keeps the token)
- Needs: `network`: [health.googleapis.com]; `oauth`: google-health
- Guide for the model: [guide.md](../../server/plugins/google-health/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/google-health/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `data` | reads | no |
| `daily_summary` | reads | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Read only, through Google Health (the Fitbit Web API's replacement, `health.googleapis.com/v4`). Sign in under Settings, Integrations with a Google Cloud OAuth Client ID and Client Secret. Data types and filter names are not checked against a live account yet.
