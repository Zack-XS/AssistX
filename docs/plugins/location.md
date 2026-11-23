---
title: location
description: Where the user is and has been - places they stay or visit often, named by them or by OpenStreetMap.
order: 9
---

# location

Where the user is and has been - places they stay or visit often, named by them or by OpenStreetMap.

- Backend: `core` (src/location.rs; the phone sends points, the core finds stays and places)
- Needs: nothing outside the core
- Quick-command words: ["travel time", "how far", "how long to get", "how long will it take", "when should i leave", "drive to", "walk to"]
- Guide for the model: [guide.md](../../server/plugins/location/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/location/plugin.yaml)
- More: [Location](../architecture/location.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `where_now` | reads | no |
| `history` | reads | no |
| `places` | reads | no |
| `name_place` | writes | no |
| `restore_place_name` | writes | no |
| `travel_time` | reads | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Phone points every ~5 min become visits and places; OpenStreetMap names a place after 45 min or 3 visits; the user's name wins. `travel_time` asks OSRM (`router.project-osrm.org`); leave-now alerts for calendar events with a place ([Location](../architecture/location.md)). Not in the index.
