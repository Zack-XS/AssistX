---
title: Turn engine
description: How one turn runs: threads, queues, the Safety Gate, limits and endings.
order: 3
---

# Turn engine

The engine is `server/src/engine.rs`. It runs turns in three modes: normal, build (see [Plugin maker](../agents/plugin-maker.md)) and dreaming (see [Dreaming](../agents/dreaming.md)). The step by step loop is in [Turn loop](../agents/turn-loop.md).

## Turns and threads

- A **thread** is a conversation. A **turn** is one run of the engine.
- Only one turn runs per thread at a time. New messages queue unless the client asks to interrupt.
- A turn waiting for a yes does not hold the thread for long (see [API](api.md), Approvals).
- Turns started by the scheduler, triggers or dreaming run in **system threads** (one per source) and appear on the global event stream.
- Dreaming takes a snapshot of memory and writes back with entry locks, so live turns are never blocked for long.

## Safety Gate

Every tool call passes the Safety Gate before it runs. The gate:

1. Finds the effective autonomy level for the tool (see [API](api.md), Autonomy).
2. Checks the call against the plugin's `needs` (hosts, secrets).
3. Checks budgets (tokens, tool runs, money).
4. For `ask`, builds the approval card from the real tool arguments, not from model text.
5. Writes the decision to the action log.

The full threat list is in [Threat model](../security/threat-model.md).

## Cost limits

Set in config, enforced by the core:

- Max tokens per turn, per day, and per dreaming run.
- Max tool calls per turn (stops loops).
- Max approval requests per turn (default 5) and per hour (default 20). A batch counts as one.
- Max money a tool may spend without asking (default 0).
- A cheaper model may be set for dreaming and index rebuilds.

When a limit is hit the turn stops with `turn.error` code `budget_exceeded`.

## How turns end

- Attended turns (a person is waiting) always end with words: if the model stops with no text, the core asks it twice to reply; if it still says nothing, the core writes what was done (`I finished, but the model did not write a reply. ...`). Scheduled and trigger turns may stay silent. The core's nudge is a `core`-trust user message and is hidden in the page's history.
- A turn that ends early (error, cancel, budget) adds an assistant message `This did not finish. ...` listing what it had saved (notes, entries, folders, other writes). Nothing saved says so.
- A restart ends every turn still `queued`, `running` or `waiting` with error `server_restarted` ("Please ask again"), so no client waits forever; their cards stay for a late yes.
- Model errors: 429, 5xx and network failures are retried 3 times (2s, 6s, 15s, or the server's `Retry-After` up to 30s), each wait announced as a `turn.note` event; the final error is a plain sentence. `src/openai_compat.rs`.
- Every tool result the model sees starts with a core line `[core] RESULT: DONE | DONE AND CHECKED | FAILED ...` built from the real outcome (`engine::verdict`), above the `<outside>` block. Small models follow it instead of guessing from the JSON. Guides open with a "Pick the right tool" table.
- Trust label `core` on messages the core writes (tool acks, job and late-yes notes).
- A tool call that needs a yes while nobody answers tells the model the tool works and only the tap is missing; a repeat of the same call reuses the pending card. Failed calls of common tools carry `right_way` (an exact example, `engine::usage`).
- `AGENT_MAX_TOKENS_PER_DAY` (default 2000000, 0 = no cap) caps cloud model tokens per day (column `turns.cloud_tokens`). Local models never count and keep working when the cap is hit.

## Power switch

`server/src/power.rs`. One switch turns the whole assistant on or off (Settings, General).

Off means no jobs fire, nothing thinks, nothing is dreamed, phone uploads are refused (the phone keeps them and sends them later), and work is refused with 503 `power_off`. Reading still works, so the app can show settings and turn it back on.

It survives a restart. Jobs that came due while off do not all fire at once: repeating ones move to their next time. Route: `GET,PUT /v1/power`.
