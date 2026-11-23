---
title: Logging
description: Every change adds logs. What to log, levels, fields, and what never to log.
order: 3
---

# Logging

**The rule: every change adds logs.** If your change does something, decides something or can fail, it writes a record of it. A change with no new log line or audit event is not done.

Why: AI agents review what the assistant did ([Audit and reports](../architecture/audit.md)). They can only fix what they can see.

## Where logs go today

The server has no logging library. It writes in four places:

| Place | What goes there | Code |
|---|---|---|
| `audit_events` table | Things that happened: a job fired, a place crossed, a phone batch came in | `crate::audit::event(app, "kind", json!({...}))` in `src/audit.rs` |
| `model_calls` table | Each model step: model, profile, request, reply, tokens, time | `crate::audit::record(...)`, called from `src/engine.rs` |
| `actions` table | Each tool call, its arguments, result and the gate's decision | `engine::log_action` and `engine::finish_action` |
| stderr | Start-up, config problems, a service that is down | `eprintln!`, read with `journalctl` ([Deploy](../architecture/deploy.md)) |

All four are per account, inside the account's encrypted database (stderr aside). All but stderr pass through `util::scrub`.

## Which one to use

- **Something the assistant did or chose** -> `audit::event`. This is the default.
- **A tool call** -> it is already logged by the engine. Add an audit event only for a choice the engine cannot see.
- **A model call** -> already logged by `audit::record`. If you add a new place that calls a model, call `audit::record` there too.
- **The server itself** (start, stop, bad config, a local service down) -> `eprintln!`. Never put user data here: stderr is not encrypted.

## What to log

| Kind | Log it when | Example kind name |
|---|---|---|
| Action | The assistant does something on its own | `job.fired`, `travel.alert` |
| Error | Something fails, even if you retry | `detect.gym` with `error` |
| Decision | The code picks one path over another, like skip or send | `detect.leave_home` with `notice: false` |
| Model call | Any model call | `model_calls` row |
| Timing | Something can be slow: a model call, a sync, a sweep | `ms` field |
| Data in | A batch arrives from the phone | `phone.location` with `kept`, `skipped` |

Log skips, not just sends. "Did not send because quiet hours" is what a reviewer needs most.

## Kind names and fields

- Kind: `area.what`, lower case, dots. Reuse an area that exists (`job`, `place`, `phone`, `detect`, `travel`, `power`).
- Detail: a small JSON object. Short keys. Counts and ids, not whole texts.
- Common keys: `device`, `job`, `place`, `result`, `error`, `why`, `ms`, `kept`, `skipped`, `notice`.
- Cut long text with `util::clean_line` before you log it.

```rust
crate::audit::event(app, "detect.gym", json!({ "visit": visit, "logged": false, "notice": sent }));
```

## Levels

`audit_events` has no level. The kind and the fields say what matters. Use these words in `eprintln!` lines:

| Level | Use for |
|---|---|
| error | The server cannot do its job: start-up failed, database locked |
| warn | A part is off but the rest works: search by meaning off, retry later |
| info | A normal, rare event: the database was moved, a token was made |

No debug spam. A line that fires every second is noise; count it instead.

## Never log

- Tokens, passphrases, API keys, recovery words, cookies, OAuth codes.
- Whole mails, messages or files in stderr.
- Exact location in stderr.
- Anything from another account.

`util::scrub` masks known secrets and common key shapes. It is a safety net, not a licence. Do not log a secret and hope scrub catches it.

## The app

The Android app has no logs today: no `Log.d` calls. That is a gap. Rules for new app code:

- Use `android.util.Log` with tag `assistant`, for errors and skipped work only.
- Never log the token, the server address with a token, or location points.
- Things the owner should review go to the server, not Logcat.

## Ties to the audit trail

Logs written with `audit::event` show up in the review pack (`GET /v1/audit/review`). So a good kind name and clear fields make the review pack better. The planned [Full activity log](../plans/full-activity-log.md) builds on this same table.

## Check

- Does each new branch that sends, skips or fails write an event?
- Does each new model call write a `model_calls` row?
- Is every logged text scrubbed and cut short?
- Could a stranger read this log line safely? If not, it does not belong in stderr.
