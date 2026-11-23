---
title: plugin-maker
description: How to learn a new ability by building a plugin when nothing in the index fits.
order: 12
---

# plugin-maker

How to learn a new ability by building a plugin when nothing in the index fits.

- Backend: `core`
- Needs: nothing outside the core
- Guide for the model: [guide.md](../../server/plugins/plugin-maker/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/plugin-maker/plugin.yaml)
- More: [Plugin maker](../agents/plugin-maker.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `start_build` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Starts a build-mode turn; the draft needs the owner.
