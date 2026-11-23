---
title: files
description: Browse, read, write, move and find files inside one allowed folder.
order: 4
---

# files

Browse, read, write, move and find files inside one allowed folder.

- Backend: `rust` (src/plugins/files.rs)
- Needs: `paths`: [$AGENT_FILES_ROOT]
- Guide for the model: [guide.md](../../server/plugins/files/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/files/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `list` | reads | no |
| `read` | reads | no |
| `search` | reads | no |
| `write` | writes | yes |
| `mkdir` | writes | yes |
| `move` | writes | yes |
| `restore` | writes | yes |
| `rmdir_empty` | writes | yes |
| `move_back` | writes | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

`needs.paths` = `AGENT_FILES_ROOT` only. No delete.
