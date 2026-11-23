---
title: Full activity log
description: Plan for an opt-in, per-account setting that records everything for that account so AI agents can review it.
order: 1
---

# Full activity log

Status: plan, not built. No code yet.

## The idea

Today the server keeps a lot already ([Audit and reports](../architecture/audit.md)). But some things are not kept: the model's thinking, what the phone did on its own, what screen the user was on, and the full picture of one moment.

The full activity log is one switch per account. When it is on, everything that account does is sent to the server and kept in one timeline. An AI agent can then read that timeline later and say what went right, what went wrong, and what to fix.

It is **off by default**. It holds the most private data in the system.

## What it records

| Group | Events |
|---|---|
| Data changes | Add, edit, delete of memory, lists, jobs, reminders, places, calendar events, files, settings. Edits keep a `ref` to the row and the list of changed fields, not copies of the text |
| Chats | Every message the user sends, every reply, every turn's status and error |
| Model | Which model and profile ran, tokens, time, and the model's **thinking** text |
| Tools | Every tool call, its arguments, its result, the gate's decision, approvals asked and answered |
| Notifications | Sent, held (quiet hours, cap), skipped (and why), tapped, snoozed, rated |
| Location | Place id at the time of each event (no coordinates). Points and visits stay in the location tables, under their own limit |
| Device | Which device, app version, battery level, charging, network type |
| App | Screens opened, buttons tapped, permissions changed, features turned on or off |
| Background | Jobs fired, the mind's runs, dreaming, detections, syncs, and their timings |
| Errors | Failed calls, retries, crashes on the phone |

## The setting

**Server.** A per-account key in the account's `settings` table: `activity_log.settings`, a JSON value. Same pattern as `brief.settings` in `src/signals.rs`.

```json
{ "enabled": false, "since": null, "off_at": null, "keep_days": 90, "parts": { "thinking": true, "location": true, "app": true } }
```

Routes (owner token only, like the audit routes):

```
GET   /v1/activity/settings
PATCH /v1/activity/settings        {enabled?, keep_days?, parts?}
```

Turning it on writes an `activity.on` audit event. Turning it off writes `activity.off`, sets `off_at`, and stops server capture at once.

What each `parts` switch drops:

| Switch off | Dropped |
|---|---|
| `thinking` | The `model_calls.thinking` value is not stored; `model.*` events keep only model, tokens and time |
| `location` | No `place` in `ctx`, and no `place.*` or `location.*` events |
| `app` | No `app.*` events (screens, taps) from the phone |

Phone capture is never on by default inside the log: overheard speech of other people (`CaptureService`) and the text of other apps' notifications (`NotifyListener`) are **not** logged. Only their counts and kinds are, like `capture.batch` with a size. Including them would need its own switch, named on the explain screen.

**Android.** A switch in the app's settings page: "Full activity log". Turning it on opens a full screen that says, in plain words:

- what is recorded, as the list above;
- that it stays on your own server, encrypted with your account key;
- that AI agents you choose will read it;
- how long it is kept, and how to delete it;
- that a review sends the log to an AI agent, which is often a cloud model, so the text leaves your server then;
- that other people's speech and other apps' notification text are not logged.

The user must tick "I understand" before it turns on. While it is on, the app's settings page shows "Full activity log: on" at the top. The phone reads the setting on each sync.

The server also pushes the switch over the live `NoticeStream`, so an open app stops at once. Any batch the server gets after `off_at` drops events newer than `off_at`. So turning it off stops capture at once, even if a phone is behind.

## Event format

One new table in the account's database:

```sql
CREATE TABLE activity (
  n      INTEGER PRIMARY KEY AUTOINCREMENT,
  id     TEXT NOT NULL UNIQUE,   -- set by the sender, so retries do not double
  ts     INTEGER NOT NULL,       -- when it happened (unix seconds)
  got    INTEGER NOT NULL,       -- when the server stored it
  source TEXT NOT NULL,          -- "server" | "device:<id>"
  kind   TEXT NOT NULL,          -- "area.what", like audit_events
  thread TEXT, turn TEXT,        -- links to the chat and the turn
  ref    TEXT,                   -- links to a row: "action:<id>", "model_call:<n>", "notice:<n>", "audit:<n>"
  ctx    TEXT NOT NULL,          -- JSON: place, device, app version, battery, network
  detail TEXT NOT NULL           -- JSON, scrubbed
);
CREATE INDEX activity_ts ON activity (ts);
CREATE INDEX activity_kind ON activity (kind, ts);
```

One event:

```json
{
  "id": "evt_01...",
  "ts": 1795400000,
  "source": "device:pixel",
  "kind": "notice.skipped",
  "ref": "notice:812",
  "ctx": { "place": "place_12", "device": "pixel", "app": "1.4.0", "battery": 62, "charging": false, "net": "wifi" },
  "detail": { "why": "quiet hours", "priority": "normal" }
}
```

Kinds use the same `area.what` names as `audit_events` ([Logging](../quality/logging.md)). New areas: `data` (add, edit, delete), `chat`, `model`, `tool`, `notice`, `app`, `device`.

Rules:

- `ctx` is filled by the sender. The server adds the last known place id if the phone sent none and `parts.location` is on.
- `ctx` never holds coordinates. The place is a place id. Where a point was is kept only in the location tables, under the location limit (`AGENT_LOCATION_KEEP_DAYS`, 90 days).
- Phone-sent `detail` is outside content. It is stored with `source: device:<id>` and shown to any agent wrapped in `<outside>`. Batches are capped like `/v1/detected`: known kinds only, at most 200 events, each `detail` at most 2 KB. Anything else gets a 400.
- `detail` is scrubbed with `util::scrub` before it is stored, like every other table.
- Big texts (tool results, prompts) are not copied. The event points to the row that has them with `ref`.

## How it builds on the audit trail

The audit trail already keeps most server-side facts. The plan reuses them and does not copy them.

| Already kept | Table | Change |
|---|---|---|
| Model requests and replies | `model_calls` | Add a `thinking` column. Fill it only when the log is on. Today thinking is shown to clients and then dropped (`src/model.rs`) |
| Tool calls and decisions | `actions` | No change. Each one gets a `tool.call` event with `ref: action:<id>` |
| Notifications | `notices` | Also keep notices the code chose not to send. Today held ones are kept; a skip shows only as a field on some audit events. Event `notice.sent` / `notice.held` / `notice.skipped` |
| Jobs, places, phone batches | `audit_events` | No change. Each one gets an event with `ref: audit:<n>` |
| Chats | `messages` | No change. `chat.user` and `chat.reply` events point to the message |

New, only when the log is on: data changes (a `ref` plus changed field names), app screens and taps, device state, phone-side errors.

The writing point is one function, `activity::log(app, kind, ref, ctx, detail)`, that does nothing when the setting is off. `audit::event`, `audit::record`, `engine::log_action` and the notice code call it. So every place that already logs feeds the timeline for free.

**The review pack.** `GET /v1/audit/review` gets a new section, "Timeline", when the log is on: the events in time order, grouped by turn, with place and device. The pack's instructions gain one line: "Use the timeline to see what the user was doing and where when each notice went out."

## Sending from the phone

The phone must not wake more for this ([Android battery](../quality/android-battery.md)).

- Events are written to a small local file queue, not sent one by one.
- The queue is sent with the existing 15-minute sync, in one request: `POST /v1/activity/batch {events: [...]}`.
- Also sent when the app is open and the queue has 50 events.
- Use WorkManager for the upload, with "network connected". On cell data, wait until the queue is big or an hour has passed.
- No new service, no new wake lock, no extra location requests. Location in `ctx` is the last known point only.
- The queue has a cap (say 5,000 events). When full, drop the oldest `app.*` events first and write one `device.dropped` event with the count.
- If the setting is off, the phone writes nothing and clears its queue.
- Each event has an `id` made on the phone, so a retry is safe (`INSERT OR IGNORE`).

Expected cost: one extra request body on a sync that already runs. Measure before and after with the steps in [Android battery](../quality/android-battery.md).

## Storage and retention

- In the account's own encrypted database (`data/accounts/<id>/core.db`, SQLCipher). See [Accounts and encryption](../architecture/accounts.md).
- Kept for `keep_days` (default 90, min 1, max 365). A daily prune, next to `audit::maybe_prune`, deletes older rows. The place id in `ctx` is cleared after the location limit (90 days), even if `keep_days` is longer.
- `thinking` in `model_calls` follows the same `keep_days`, not `AGENT_TRACE_KEEP_DAYS`.
- Turning the log off keeps what was recorded until it ages out, unless the user deletes it.

## Who can read it

- Only the account's owner token. Client, device and trigger tokens cannot read it. Device tokens can only **send** batches.
- No other account, and no admin, can read it.
- An AI agent reads it only when the owner hands it a pack (see below). The assistant itself does not see the log in its prompt.

## Read, export, delete

```
GET    /v1/activity?since=&until=&kind=&turn=     owner, paged, newest first
GET    /v1/activity/export?since=&until=          owner, JSON lines
DELETE /v1/activity?since=&until=                 owner; no range = delete all
```

- `GET /v1/export` includes the activity table.
- Delete removes the rows, the `thinking` column values in range, and the phone's queue on next sync. It writes one `activity.deleted` audit event with the range and count, not the content.
- The app's settings page has "Delete activity log" with a confirm step.

## Privacy and safety

This is the most private data the system holds: where you were, what you said, what the model thought about you.

- **Opt-in only.** Off by default. No other setting, job or plugin can turn it on. The model cannot turn it on: it is not a tool.
- **Clear UI.** A full explain screen, an "I understand" tick, and an "on" banner while it runs.
- **Encrypted at rest** with the account's data key, like everything else per account.
- **Secrets scrubbed** before storing. Passwords and tokens typed in the app are never captured; text fields marked secret are skipped.
- **Outside content stays data.** When an agent reads the log, mail, web text and tool results in it are still wrapped in `<outside>`. A hidden order in a logged mail must not steer the reviewing agent.
- **No stderr.** Nothing from the log goes to `eprintln!` or `journalctl`.
- **Not sent anywhere on its own.** The server never forwards the log to a cloud model. An export is a file the owner moves. But a review hands it to an AI agent, which is often a cloud model; the explain screen says so.
- **Redacted export.** `GET /v1/activity/export?redact=1` drops thinking, place ids and chat text. Use it for cloud reviews.
- **Forget reaches it.** "Forget" removes matching `activity` rows and `model_calls.thinking` values ([Memory](../architecture/memory.md#forget)).
- **Other people stay out.** No overheard speech and no notification text from other apps (see The setting).
- **Pause.** A "Pause for 1 hour" button, for moments the user wants unrecorded.

## How agents read it

1. The owner runs the review, as today: `assistantx-review` or `GET /v1/audit/review?since=last&mark=1`.
2. With the log on, the pack has the Timeline section.
3. For deep work, the agent gets `GET /v1/activity/export` for a window, as JSON lines. Use `redact=1` when the agent runs on a cloud model.
4. The owner tells the agent what to look for: "Find notices that came at a bad time", "Find where the model's thinking went wrong", "Find what I did by hand that the assistant could have done".

The agent proposes edits. It never gets write access to the account.

## Steps to build

0. Add the log to the [Threat model](../security/threat-model.md#activity-log-planned) and check each defense there.
1. Server: `activity` table, `activity::log`, settings and routes, prune. Tests: off writes nothing; device token cannot read; scrub works; delete removes all; a batch after `off_at` stores nothing; oversize or unknown kinds get a 400; no coordinates in `ctx`. Add `activity` and `thinking` to the Forget sweep; test that a forgotten fact is gone from both.
2. Server: wire in `audit::event`, `audit::record` (with thinking), `log_action`, notices (with skipped), data changes.
3. Review pack Timeline section.
4. Android: switch, explain screen, banner, local queue, batch upload on the sync with WorkManager, and the off switch over `NoticeStream`.
5. Android: app, device and error events.
6. Measure battery. Write the numbers here.
7. Move this page to `architecture/` once built, and link it from [Audit and reports](../architecture/audit.md).

## Open questions

- Should thinking be kept for cloud models only, or all models?
- Should the explain screen ask for the passphrase again before turning it on?
- How much app tapping is useful, and how much is noise?
