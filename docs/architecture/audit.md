---
title: Audit and reports
description: What is kept, the review pack, chat reports and automatic reports.
order: 16
---

# Audit and reports

Status: built. `src/audit.rs`.

Everything the assistant is told, pulls and says is kept, so the owner can hand a window of it to an AI agent now and then ("review this week, fix what is noisy, find what it missed").

## What is kept

| What | Where | Kept |
|---|---|---|
| The exact request of every model step (system prompt, all messages, tool names) and its reply, model, tokens, time | `model_calls`; each distinct system prompt once in `system_prompts` | The row and reply for good. The big part (the messages sent) for `AGENT_TRACE_KEEP_DAYS` days (default 30, `0` = forever), then thinned |
| Jobs that fired, places crossed (by geofence or by points), data batches from the phone | `audit_events` | for good |
| Tool arguments and the data tools returned, the trigger prompt, replies | `messages` (as before) | until the chat is deleted |
| Every notification, also held ones (quiet hours, daily cap): priority, title, text, silent, delivered, thread, reminder id, the owner's rating | `notices` | always |
| Every tool action with its decision | `actions` (as before) | always |

Secrets are scrubbed from everything written.

How to add records in new code: [Logging](../quality/logging.md). A plan to record everything for one account, when the owner opts in: [Full activity log](../plans/full-activity-log.md).

## Routes (owner token)

```
GET  /v1/audit/review?hours=24&turns=200   markdown pack for an AI agent
GET  /v1/audit/review?since=last&mark=1    everything since the last marked review, and mark this one
GET  /v1/audit/turns/{id}                 one run with every step's exact request (JSON)
POST /v1/notices/{n}/feedback             {rating: "useful"|"not_useful", note?}   owner or client token
```

The phone adds a **Not useful** button to every notification that is not a reminder; reminders keep Snooze and Done. One tap writes the rating.

## The pack

It opens with instructions for the agent: judge each notification, look for misses in runs that sent nothing, name the cause (trigger prompt, guide, shared rules, model too small), and propose exact edits, with the files where the behaviour lives. Then:

Notifications (with the owner's ratings and which job sent each), Runs that sent nothing, Every run (prompt, tool calls, data returned, reply, newest first), and each distinct system prompt once.

To use it: `curl -H "Authorization: Bearer $TOKEN" "http://<server>:8790/v1/audit/review?hours=168" > review.md`, then give `review.md` and the repo to an agent.

Limits: tool data in the pack is cut at 1,500 characters a piece (the full text is in the turn trace). Runs from before this feature have no model-call record.

## Asking an agent to review it

On the Mac: `assistantx-review` (outside the repo, in `~/bin`). It pulls everything since the last review into `AssistantX-reviews/review-<time>.md`, copies it to `AssistantX-reviews/latest.md` and marks it, so the next one starts there. `assistantx-review 168` is the last week without marking; `assistantx-review peek` looks without marking.

Then tell the agent: *"Read `AssistantX-reviews/latest.md` and the AssistantX repo. Tell me what the assistant did right and wrong in this window, which notifications were not worth it, what it missed, and the exact edits you would make."*

## Automatic reports

Status: built. `src/autoreport.rs`.

When one plugin tool has failed 3 times in 24 hours (failed calls in the action log), or a turn a person was waiting on ends on an error, the core saves the same report a "Send log" tap does (into the encrypted database), with `auto: true` and the note `auto: <plugin.tool> failed N times in 24 h: <last errors>` (or `auto: a turn ended on an error (<code>): ...`).

At most one per tool, or per error code, a day; secrets are scrubbed; model-test chats (`eval: ...`) never count. Each one is also an `auto_report` audit event. `GET /v1/reports?auto=1` lists only these. `AGENT_AUTO_REPORTS=off` turns it off.

## Reports

- `POST /v1/threads/{id}/report {note?}`: save everything about one chat for review: messages as the model saw them (tool calls, tool results with their `[core] RESULT` lines), turns with status and errors, actions, approvals, events, and model settings. Written to the encrypted database (table `reports`) (mode 0600), known secret values scrubbed, newest 200 kept. Any token may send one.
- `GET /v1/reports` lists them, `GET /v1/reports/{id}` returns one. Owner only: a report holds a whole chat.

Example review packs were kept in `reviews/` at the top of the project. They hold real chats, so they are not copied into these docs.
