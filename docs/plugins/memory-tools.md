---
title: memory-tools
description: Skill for writing things down. Load it when the user shares something worth keeping, or wants memory organised, folders made or facts rated.
order: 10
---

# memory-tools

Skill for writing things down. Load it when the user shares something worth keeping, or wants memory organised, folders made or facts rated.

- Backend: `core`
- Needs: nothing outside the core
- Guide for the model: [guide.md](../../server/plugins/memory-tools/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/memory-tools/plugin.yaml)
- More: [Memory](../architecture/memory.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `remember` | writes | no |
| `list_folders` | reads | no |
| `list_confidence` | reads | no |
| `list_disputed` | reads | no |
| `make_folder` | writes | no |
| `write_entry` | writes | no |
| `move_entry` | writes | no |
| `move_folder` | writes | no |
| `set_confidence` | writes | no |
| `set_status` | writes | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Guide for bigger memory work. `remember` is also dreaming's `dream_remember`.
