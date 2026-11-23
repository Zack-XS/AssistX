---
title: Plugin maker
description: How the agent learns a new ability: drafts, build mode, approval and versions.
order: 3
---

# Plugin maker

## Lifecycle

```
none -> draft -> active -> disabled
          ^         |
          +---------+  (edit creates a new draft version)
```

1. Missing ability found in the index, and the user says yes to learning it.
2. Model loads `plugin-maker` and builds a draft in `plugins/.drafts/<id>/` with status `draft`.
3. Draft is tested in the sandbox, using its `tests/` if present.
4. User sees an approval card (below).
5. Approved: status `active`, index rebuilt.
6. Any later change by the agent makes a new draft; the old version stays live until approved.
7. Rollback makes an older version live. Uninstall removes the plugin; its `state/` is removed only if the user asks.

## Approval card

Built by the core from the files, not from model text:

- One-sentence purpose (from `summary`).
- **Network hosts** it may reach.
- **Secrets** it may use.
- **Each tool** with its tags and requested autonomy. Any `auto` request on a `writes` tool is highlighted.
- For updates: what changed in `needs` and autonomy, and a link to the full diff.
- For installs from outside, and for drafts built after reading web pages: source, hash, signer if any, and the full `guide.md` (expandable). For all other drafts the guide is one tap away.
- What the tests did: the calls they made and their results.
- A warning if the guide mentions other plugins' tools.

A plugin that adds a network host or a secret, or asks for `auto` on a `writes` tool, is `high` risk and needs an owner token.

## plugin-maker prompt (outline)

The built-in prompt tells the agent to:
1. Restate the goal of the plugin in one sentence.
2. Check the index again for anything close that could be extended.
3. Research the app or API if needed (docs, `--help`, test calls) using build mode (below).
4. Write `plugin.yaml` with the smallest permissions that work.
5. Write `guide.md`: steps, edge cases, what to ask, what to avoid.
6. Write tools, with JSON Schema inputs/outputs and clear errors.
7. Write tests and run them in the sandbox.
8. Request autonomy: reads are `auto`; sends, spends, deletes, shares are `ask`.
9. Save as `draft` and request approval with a plain-language summary.
10. Never touch files outside its own plugin folder. (The sandbox enforces this; the rule is a reminder.)

## Build mode

Drafts have no approved `needs` yet, but research needs the internet.

- Build mode runs in its own thread (`system:build:<id>`). That turn gets the plugin goal and no memory: no `core.md`, no `search_memory`, no memory entries. A web page that fools the model then has nothing private to leak.
- Build mode allows read-only HTTP GET to any host through the proxy, with no secrets and no request body. URLs are capped at 200 characters, and requests are rate limited.
- Every URL fetched is logged and shown on the approval card.
- Build mode can install the plugin's `deps` into the plugin's own folder. After approval, deps are frozen; changing them is a new version.
- Test calls to the real service that use a secret need a `medium` approval each.

The runtime prompt is [plugin-maker.md](../../server/prompts/plugin-maker.md). The plugin page is [plugin-maker](../plugins/plugin-maker.md).
