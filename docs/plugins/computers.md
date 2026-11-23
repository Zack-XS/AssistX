---
title: computers
description: The user's own computers (Mac, Windows, Linux) running agent-node - list, read, search, copy and move their files.
order: 2
---

# computers

The user's own computers (Mac, Windows, Linux) running agent-node - list, read, search, copy and move their files.

- Backend: `core` (src/nodes.rs; the computer runs the tool itself, inside its own allowed folders)
- Needs: nothing outside the core
- Guide for the model: [guide.md](../../server/plugins/computers/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/computers/plugin.yaml)
- More: [Computers](../architecture/computers.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `list_devices` | reads | no |
| `info` | reads | no |
| `list` | reads | no |
| `stat` | reads | no |
| `read` | reads | no |
| `search` | reads | no |
| `write` | writes | yes |
| `mkdir` | writes | yes |
| `copy` | writes | yes |
| `move` | writes | yes |
| `delete` | writes, deletes | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Runs on a computer with agent-node, inside its allowed folders; key folders always refused; `--read-only` on the computer refuses all changes.
