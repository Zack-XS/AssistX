---
title: terminal
description: Run commands in the user's work folder - allowlisted programs, or any command line after a warned yes.
order: 15
---

# terminal

Run commands in the user's work folder - allowlisted programs, or any command line after a warned yes.

- Backend: `rust` (src/plugins/terminal.rs)
- Needs: `paths`: [$AGENT_TERM_CWD]
- Guide for the model: [guide.md](../../server/plugins/terminal/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/terminal/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `allowed` | reads | no |
| `run` | writes | yes |
| `shell` | writes, deletes, unrestricted | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

`run`: allowlist, no shell. `shell`: any command through `/bin/sh`, tag `unrestricted` = always a yes with a loud warning showing the exact command (no setting makes it auto). Both in the sandbox, `needs.paths` = `AGENT_TERM_CWD`.
