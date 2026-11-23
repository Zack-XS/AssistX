---
title: web
description: Read public web pages as plain text and list their links.
order: 17
---

# web

Read public web pages as plain text and list their links.

- Backend: `rust` (src/plugins/web.rs; GET only, no redirects, 1 MB cap)
- Needs: `network`: ["*public*"]
- Guide for the model: [guide.md](../../server/plugins/web/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/web/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `fetch` | reads | no |
| `links` | reads | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Through the proxy; any public host; no redirects.
