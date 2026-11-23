---
title: weather
description: Current weather and a three day outlook for a named place.
order: 16
---

# weather

Current weather and a three day outlook for a named place.

- Backend: `rust` (src/plugins/weather.rs)
- Needs: `network`: [geocoding-api.open-meteo.com, api.open-meteo.com]
- Quick-command words: ["weather", "rain", "raining", "temperature", "forecast", "sunny", "snow", "umbrella", "cold outside", "hot outside"]
- Guide for the model: [guide.md](../../server/plugins/weather/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/weather/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `now` | reads | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Proxy allows only the two Open-Meteo hosts.
