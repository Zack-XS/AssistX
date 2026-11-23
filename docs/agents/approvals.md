---
title: Approvals
description: Cards, risk, autonomy levels, batches, voice answers and waiting.
order: 6
---

# Approvals

```json
{
  "id": "apr_...",
  "kind": "tool_call | tool_call_batch | plugin_install | plugin_update | memory_question | question",
  "risk": "low | medium | high",
  "question": "Send this email to sam@example.com?",
  "details": { "plugin": "mail", "tool": "send", "args": { } },
  "options": ["yes", "no", "edit"],
  "expires_at": "..."
}
```

- `question` and `details` are built by the core from the real tool call. The model can add a short reason, shown separately and marked as the assistant's words.
- `risk`:
  - `low`: reversible and private (create a draft, add a calendar hold).
  - `medium`: visible to others or hard to undo (send a message, move a meeting with guests).
  - `high`: money, deleting data, sharing private data, new or changed plugin permissions. Needs an `owner` token.
- The core sets `risk` from the tool's tags, not from the model: `reads` and `writes` are `low`; `sends` is `medium`; `spends`, `deletes` and `shares` are `high`. The highest tag wins. A tool with no tags is `medium`.
- A "yes" is tied to a hash of the exact tool, arguments and plugin version on the card. If any of them change before the call runs, the approval is void and a new one is raised.
- Approvals expire (default 24 hours). Expired means no.
- Answering an approval twice returns `conflict`. The first answer stands.

What the turn does while it waits:

- The turn pauses (`turn.waiting`). Waiting time does not count against turn budgets.
- After `approval_wait` (default 10 minutes) the turn ends with the call marked "not run yet". The approval stays pending until it expires.
- A late "yes" runs that exact call (the hash still has to match), logs it, and starts a short follow-up turn so the model can tell the user what happened.
- Turns started by the scheduler or a trigger do not wait at all. They raise the approval and carry on with what they can do.
- A pending approval never blocks new messages on the thread.

Answer:

```json
{ "answer": "yes" }
{ "answer": "no", "note": "optional reason shown to the model" }
{ "answer": "edit", "args": { "subject": "New subject" } }
```

A `question` card comes from the model's `ask_user` tool. `details` holds `question` and `options` (up to 6, only suggestions). Answer with `{ "answer": "reply", "text": "..." }` (any text, up to 1000 characters) or `{ "answer": "no" }` to skip.

It is low risk, a client token may answer it, and a late reply starts a short follow-up turn. Turns the user is not waiting on (scheduler, trigger) cannot ask.

`edit` replaces some tool arguments. The core checks the new arguments against the tool's input schema and the Safety Gate again before running. `edit` is not allowed on a batch.

### Batches

Six calendar holds should be one tap, not six. The core, not the model, groups them.

- Only `low` risk calls are grouped. `medium` and `high` calls always get their own card.
- Calls are grouped when they come from the same turn, the same plugin and the same tool, and the model asked for them in the same step.
- A batch holds at most 20 calls (config). Extra calls start a second batch.
- The card is `tool_call_batch` with an `items` list. Each item shows its real arguments, built by the core. Nothing is hidden behind "and 5 more": the front end must be able to show every item.
- The "yes" is bound to a hash of the whole list: every tool, every argument set, the plugin version. If any item changes, the approval is void.
- A batch counts as one request toward the approval limits. This is why batching helps with fatigue.
- Answer with `{ "answer": "yes", "except": ["item_id"] }` to run all but some. `no` rejects all. Skipped items are told to the model as "user declined".
- Each item is still its own action in the log, and can be undone alone.

```json
{
  "id": "apr_...",
  "kind": "tool_call_batch",
  "risk": "low",
  "question": "Add 6 calendar holds?",
  "items": [
    { "item_id": "it_1", "plugin": "calendar", "tool": "create_event", "args": { } }
  ]
}
```

Why only `low`: a batch of sends or deletes invites a blind yes. For `low` calls the worst case is a few private entries that can be undone.

## Autonomy levels

- `auto`: just do it (read calendar, search memory).
- `ask`: needs an approval (send mail, spend money, delete).
- `never`: blocked.

The **effective level** comes from these rules, in this order:

1. **Plugin `never`** is final. Only disabling the plugin or tool changes it.
2. **User config** (per tool, owner token): if set, it is used. A user may loosen a tool the plugin marked `ask`, or tighten one the plugin marked `auto`.
3. **Plugin request**: the level in `plugin.yaml`, used when the user has set nothing. It is only a request and is shown on the approval card.
4. **Core floor** (built in, not editable by the model): any tool tagged `sends`, `spends`, `deletes` or `shares` is at least `ask`, whatever the first three say. The user can lower one of these to `auto` only through `PATCH /v1/config` with an owner token, per tool, and only with a spending cap for `spends`.

`never` from the user always blocks. Unknown tools default to `ask`.

Tags are claims made by the plugin author. See [Plugins](../architecture/plugins.md), Tool tags, for how they are checked.

## More in code

- Approval status `void`; approval kinds `memory_load` and `memory_question`. Core tool `ask_user(question, options?)` raises a `question` approval; answer `{answer:"reply",text}` or `no`.

  Core tool `notify(text, title?, silent?, priority?)` sends a notice (max 5 per turn; outside content cannot make it urgent).

  Job kind `task` (`schedule_task`, `schedule_recurring_task`) starts an unattended turn in its own thread `system:task:<job>:<time>`, whose last reply is the report, plus a silent "Report ready" notice.
- Voice answers: an approval answer may carry `via: "voice"`, accepted only when the card's `voice_ok` is true (see [Voice](../architecture/voice.md)).
- Model upgrades raise an approval of kind `model_upgrade` (see [Models](../architecture/models.md)).
- The old docs describe batches of `low` calls. The code does not batch yet (see [Next](../roadmap/next.md)).
