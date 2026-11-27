---
title: API
description: The one outside door: transport, auth, scopes and every route.
order: 1
---

# API

Goal: one small, stable door. Any front end (app, CLI, voice, bot) can use it.

Built in Rust with `axum`. The OpenAPI file is generated from the code (`utoipa`) and published with each release, so front ends can generate typed clients.


## Transport

- HTTP + JSON for commands.
- Server-Sent Events (SSE) for event streams. No WebSocket in v1.
- Bind to `127.0.0.1` by default. Binding to any other address needs an explicit config flag and TLS.
- Versioned: `/v1/...`.

## Auth

A token is required on every request, **including localhost**. Any app on the machine, or a web page using DNS rebinding, can reach localhost, so "local" is not "trusted".

- Header: `Authorization: Bearer <token>`.
- Only two routes take no token: `GET /v1/health` (says nothing private) and `GET /v1/oauth/callback` (a browser redirect; it is protected by its one-time `state` value).
- The first run creates an owner token in a file only the user can read (mode `0600`). The CLI can create, list and revoke tokens.
- The server rejects requests whose `Host` header is not the bound address, and requests with an `Origin` header unless that origin is on an allowlist in config.
- Tokens are stored hashed.

### Scopes

| Scope | Can do |
|---|---|
| `client` | Threads, messages, events, read and edit memory (add, edit, erase an entry), read actions, read plugins, list and cancel jobs, dry-run dreaming, answer `low` and `medium` approvals |
| `owner` | Everything in `client`, plus: answer `high` approvals, approve/install/uninstall plugins, change config and autonomy, set secrets, manage tokens, run dreaming for real, export, forget, undo |
| `trigger` | Only `POST /v1/triggers/{source}`. One token per trigger source. |

The sandbox has no network route to the API. The model and plugins can never call it.

Browser `EventSource` cannot send an `Authorization` header. Front ends read SSE with `fetch` and a stream reader. The server never takes a token in the URL, because URLs end up in logs and history.

## Core concepts

| Concept | Meaning |
|---|---|
| Thread | A conversation. Has an id. User threads are made by clients; system threads (`system:scheduler`, `system:dreaming`, `system:trigger:<source>`) are made by the core. |
| Turn | One run of the engine, started by a message, a schedule, or an outside event. |
| Event | Something that happened during a turn. Streamed to clients. |
| Approval | A question to the user that blocks a risky act. |
| Action | A logged thing the assistant did. May be undoable. |

## Endpoints

Lists are paged: `?limit=50&cursor=...`, and the reply holds `next_cursor` (null at the end). Entries a client adds or edits by hand are stored with `source_trust: user`.

### Threads and turns

```
POST   /v1/threads                          create a thread
GET    /v1/threads                          list threads (paged)
GET    /v1/threads/{id}                     thread info (+ `summary`: the long-chat summary or null)
DELETE /v1/threads/{id}                     archive a thread (raw log kept)
GET    /v1/threads/{id}/messages            message history (paged)
POST   /v1/threads/{id}/messages            send a message, starts or queues a turn
POST   /v1/threads/{id}/cancel              stop the running turn
GET    /v1/turns/{id}                       turn status and summary
```

### Events

```
GET    /v1/events                           global stream (all threads, incl. system)
GET    /v1/threads/{id}/events              one thread's stream
```

Both are SSE and support replay with `Last-Event-ID`.

### Reports

- `POST /v1/threads/{id}/report {note?}`: save everything about one chat for review: messages as the model saw them (tool calls, tool results with their `[core] RESULT` lines), turns with status and errors, actions, approvals, events, and model settings. Written to the encrypted database (table `reports`) (mode 0600), known secret values scrubbed, newest 200 kept. Any token may send one.
- `GET /v1/reports` lists them, `GET /v1/reports/{id}` returns one. Owner only: a report holds a whole chat.

### Approvals (routes)

```
GET    /v1/approvals?status=pending         list waiting approvals
GET    /v1/approvals/{id}                   one approval
POST   /v1/approvals/{id}                   answer: yes | no | edit
```

### Actions

```
GET    /v1/actions                          list the action log (paged, filter by thread/plugin/time)
GET    /v1/actions/{id}                     one action, with tool args and result
POST   /v1/actions/{id}/undo                undo an action          (owner)
```

### Memory

```
GET    /v1/memory?q=...                     search memory
GET    /v1/memory/{id}                      one entry, with sources
GET    /v1/memory/{id}/history              version history
POST   /v1/memory                           add an entry by hand
PATCH  /v1/memory/{id}                      edit an entry
DELETE /v1/memory/{id}                      erase an entry (keeps raw sources)
POST   /v1/memory/{id}/forget               full forget, see 04 "Forget"   (owner)
GET    /v1/memory/core                      read core.md (a short profile, at most 12 lines)
GET    /v1/memory/folders                   the folder tree: counts, descriptions, average confidence
POST   /v1/memory/folders                   make a folder {path, description}
GET    /v1/memory/index                     read the current index
```

### Dreaming

```
POST   /v1/dreaming/run                     run now  {dry_run: bool}
GET    /v1/dreaming/runs                    list past runs
GET    /v1/dreaming/runs/{id}               change log and questions from one run
```

### Plugins

```
GET    /v1/plugins                          list plugins + status
GET    /v1/plugins/{id}                     plugin info, needs, autonomy, provenance
GET    /v1/plugins/{id}/versions            version list
GET    /v1/plugins/{id}/diff?from=&to=      file diff between versions (default: live vs draft)
POST   /v1/plugins/install                  install from path or URL, arrives as draft   (owner)
POST   /v1/plugins/{id}/approve             promote a draft to active                     (owner)
POST   /v1/plugins/{id}/reject              discard a draft                              (owner)
POST   /v1/plugins/{id}/disable             turn a plugin off                            (owner)
POST   /v1/plugins/{id}/rollback            make an older version live                   (owner)
DELETE /v1/plugins/{id}                     uninstall (state folder removed on request)  (owner)
```

### Jobs (reminders, recurring work, alarms)

```
GET    /v1/jobs                             list scheduled jobs (with ring, sound, event_id, next_run_local)
POST   /v1/jobs                             set an alarm or reminder without the model (see below)
GET    /v1/jobs/{id}                        one job
PATCH  /v1/jobs/{id}                        change it: text|label, at|time, in_minutes, days, every ("once" stops repeating), weekday, ring, sound
DELETE /v1/jobs/{id}                        cancel a job
GET    /v1/alarms/ringing                   what is ringing now
POST   /v1/alarms/snooze                    {id?, minutes?}  stop the ring, ring again later (default 9 min)
POST   /v1/alarms/dismiss                   {id?}  stop the ring
GET    /v1/sounds                           built-in names plus custom ringtones
PUT    /v1/sounds/{name}                    upload a ringtone: the raw mp3, ogg, wav or m4a file, at most 3 MB
GET    /v1/sounds/{name}                    the ringtone file, for the app to play
DELETE /v1/sounds/{name}                    remove it
```

`POST /v1/jobs` bodies: `{"kind": "alarm", "time": "07:00", "days": "weekdays" | [1,3,5], "label": "Wake up", "sound": "bell"}` or `{"kind": "reminder", "text": "Call Sam", "in_minutes": 30 | "at": "2026-11-14 15:00", "ring": true, "sound": "chime"}`. `days` uses 0 = Sunday.

The model creates jobs through the `scheduler` plugin. A front end must be able to show, change and cancel them without asking the model.

A job keeps its own text. If it was made in a turn that held outside content, the job is marked `outside` and its text is wrapped as outside content each time it runs, so an injected "every day, do X" is data, not an order. Outside jobs never ring, and an alarm cannot be set from such a turn at all.

**Ringing.** An alarm is due: the core sends `alarm.ring` at once, with no model turn.

The app rings with that sound until the user snoozes or dismisses it (on any device: `alarm.stopped` tells the others), or until 10 minutes pass; then it is `missed` and one urgent notice says so.

An alarm more than 30 minutes late (the server was off) only sends that notice. A reminder sends `reminder.due` at once (the phone can show it right away), rings too if `ring` is set, and then the model's turn may add a notice with the same job.

Alarms ring in quiet hours and do not count toward the daily notice cap: the user set them.

**Sounds.** Built-in names (`default`, `chime`, `bell`, `beep`, `gentle`, `radar`) are sounds the app ships; their `sound_url` is null. A custom ringtone is a file the owner uploads; it is checked by its first bytes, kept in `data/sounds/`, never shown to the model and never transcribed. This is separate from phone capture, which still refuses all audio (08).

### Calendar

```
GET    /v1/calendar/events?from=&to=&q=     occurrences in a range (local time; default the next 7 days)
POST   /v1/calendar/events                  add: title, start, end?, all_day?, location?, notes?, repeat?, until?, remind?
GET    /v1/calendar/events/{id}             one event
PATCH  /v1/calendar/events/{id}             change only the fields given
DELETE /v1/calendar/events/{id}             delete it and its reminders
```

Times are local wall time (`2026-11-14 15:00`, or `2026-11-14` for all day) plus a zone. `repeat` is `none`, `day`, `week`, `weekdays`, `month` or `year`, with `until` a date. `remind` is minutes before:

`[10, 60]` or `[{"minutes": 10, "ring": true, "sound": "bell"}]`, up to 5. Each reminder is a job of kind `event` that follows its event: moving the event moves it, deleting the event cancels it, and the job itself cannot be changed directly.

The next 24 hours of events appear in the index, wrapped as outside content.

### Lists

```
GET    /v1/lists                            lists with open and done counts
POST   /v1/lists                            make one: name, items?                  201
GET    /v1/lists/{id}                       the list and its items (open first, then done)
PATCH  /v1/lists/{id}                       rename: name
DELETE /v1/lists/{id}                       delete the list (items kept with it for undo)
POST   /v1/lists/{id}/items                 add: text, quantity?, note?, due?  or  items: [...]   201
PATCH  /v1/lists/{id}/items/{item}          done?, text?, quantity?, note?, due? ("" clears)
DELETE /v1/lists/{id}/items/{item}          remove one item
POST   /v1/lists/{id}/clear                 remove the ticked items
GET    /v1/lists/{id}/text                  the list as plain text, for sharing (text/plain)
```

A list is found by id or by name: `shopping list`, `Shopping` and `groceries` are one list, as are `todo`, `to do` and `to-do`. Names are unique. Items carry an optional `quantity`, `note` and `due` (`YYYY-MM-DD` or `YYYY-MM-DD HH:MM`).

The same open words are not added twice; adding a ticked item opens it again. Limits: 100 lists, 500 items per list, 100 items per call.

The model uses the `lists` plugin ([Plugins](plugins.md)); lists with open items appear in the index as names and counts only, wrapped as outside content. Sharing is the client's job: the Android app passes the plain text to the phone's share sheet.

### Secrets

```
GET    /v1/secrets                          list secret names (never values)       (owner)
PUT    /v1/secrets/{name}                   set a secret                           (owner)
DELETE /v1/secrets/{name}                   remove a secret                        (owner)
POST   /v1/secrets/{name}/oauth/start       begin an OAuth login, returns a URL    (owner)
GET    /v1/oauth/callback                   OAuth redirect target (state-checked)
```

### Triggers, config, export, health

```
POST   /v1/triggers/{source}                outside event starts a turn             (trigger)
GET    /v1/config                           read config (secrets hidden)
PATCH  /v1/config                           change config                           (owner)
GET    /v1/export                           export everything as a zip: markdown + JSON   (owner)
GET    /v1/health                           liveness, no auth
GET    /v1/tokens                           list tokens                              (owner)
POST   /v1/tokens                           create a token with scopes               (owner)
DELETE /v1/tokens/{id}                      revoke a token                           (owner)
```

Approval cards, batches and autonomy are in [Approvals](../agents/approvals.md). Events are in [Events](events.md). Jobs, notices and alarms are explained in [Scheduler and notices](scheduler-and-notices.md).

## Routes the old docs did not list

These are in `server/src/api.rs` today.

| Route | What it does |
|---|---|
| `GET /v1/memory/graph` | People, places and things with their links, for the Mind map. |
| `POST /v1/memory/clear` | Clear all memory (owner). |
| `POST /v1/jobs/{id}/pause`, `/resume` | Pause or resume a job. |
| `POST /v1/jobs/{id}/run` | Start a task now. Returns the thread its report will be in. |
| `POST /v1/jobs/{id}/skip` | Skip one day of a repeating alarm, reminder or task. |
| `POST /v1/calendar/events/{id}/skip` | Skip one day of a repeating event: `{date, undo?}`. The rest stay. |
| `GET /v1/automations/watch` | Which phone apps the phone should forward notifications from (only apps that start a task). |
| `POST /v1/automations/notify` | The phone saw a notification from a watched app: `{app, app_name?, title, text}`. Device or client token. |
| `GET,PUT /v1/power` | The master switch. Off: no jobs, no thinking, no dreaming, uploads refused, work refused with 503 `power_off`. Reading still works. |
| `GET /v1/health/days`, `POST /v1/health/days` | Daily body numbers from the phone. See [health](../plugins/health.md). |
| `/v1/server/*` | Accounts and the server token. See [Accounts](accounts.md). |

OAuth in code is `GET,DELETE /v1/oauth/{provider}` and `POST /v1/oauth/{provider}/start|finish` (owner). The `POST /v1/secrets/{name}/oauth/start` and `GET /v1/oauth/callback` routes above are not in the code today.

## Sending a message

```
POST /v1/threads/{id}/messages
{ "text": "...", "interrupt": false, "client_msg_id": "uuid" }

202 Accepted
{ "turn_id": "...", "queued": true }
```

- Only one turn runs per thread. If one is running, the message queues, or with `interrupt: true` the running turn is cancelled first.
- Cancel stops the turn between steps. A tool already running is killed, and its action is logged as `outcome unknown`, because the core cannot know if it finished. The model is told so.
- `live: true` says the user is talking out loud (the voice screen). The model is told its words are read aloud and gets `listen_for_reply`; when it calls it, `GET /v1/turns/{id}` shows `listen: true` and the client turns the microphone on after speaking the reply. In a live turn `ask_user` does the same instead of raising a card. A turn cancelled while it waits voids its approval.
- An approval answer may carry `via: "voice"`. The core accepts it only when the approval's `voice_ok` is true: questions and low or medium risk tool calls. High risk calls and plugin installs or updates must be answered by a tap (400 otherwise).
- `client_msg_id` makes the call safe to retry. A repeat with the same id returns the same `turn_id`.

## Errors

Every error has a human sentence and a machine code. Front ends show the sentence.

```json
{ "error": { "code": "approval_expired", "message": "That request timed out, so nothing was sent." } }
```

Common codes: `unauthorized`, `forbidden_scope`, `not_found`, `conflict`, `invalid_input`, `rate_limited`, `budget_exceeded`, `model_unavailable`, `tool_failed`, `sandbox_denied`, `approval_expired`, `approval_limit`, `plugin_tampered`.

## Turn sources

1. User message.
2. Scheduler (morning brief, reminders, dreaming).
3. Trigger (webhook, file change, calendar change).

Trigger bodies are untrusted. The core wraps them as outside content before the model sees them (see [Prompt system](prompt-system.md)).

## Rate limits

Per token, set in config. Defaults: 60 messages per minute, 10 triggers per minute per source. Over the limit returns `429` with code `rate_limited`.

## Stability promise

Event names and endpoint shapes follow semver. Plugins and prompts can change freely; the API cannot break within a major version.

## Phone capture

See [Phone capture](phone-capture.md) for the whole contract: `POST/GET/DELETE /v1/ambient/chunks`, `GET /v1/ambient/status`, `GET/PATCH /v1/ambient/settings`, `POST /v1/ambient/pause`, `/resume`, `/process`, `GET /v1/ambient/picks`, `POST /v1/ambient/picks/{id}`. Token scope `device` may only `POST /v1/ambient/chunks` and `GET /v1/ambient/status|settings`.

## Route summary

The short list kept for contributors. It also covers routes from later docs.

Threads, turns, events: `POST,GET /v1/threads` · `GET,PATCH,DELETE /v1/threads/{id}` (`{model}`: this chat's own model, [Models](models.md)) · `GET,POST /v1/threads/{id}/messages` · `POST /v1/threads/{id}/cancel` · `GET /v1/threads/{id}/events` · `GET /v1/events` · `GET /v1/turns/{id}`
Approvals, actions: `GET /v1/approvals` · `GET,POST /v1/approvals/{id}` · `GET /v1/actions` · `GET /v1/actions/{id}` · `POST /v1/actions/{id}/undo`
Memory: `GET,POST /v1/memory` · `GET,PATCH,DELETE /v1/memory/{id}` · `GET /v1/memory/{id}/history` · `POST /v1/memory/{id}/forget` · `GET /v1/memory/core` · `GET /v1/memory/index`
Dreaming: `POST /v1/dreaming/run` · `GET /v1/dreaming/runs` · `GET /v1/dreaming/runs/{id}`
Plugins: `GET /v1/plugins` · `GET,DELETE /v1/plugins/{id}` · `GET /v1/plugins/{id}/versions` · `GET /v1/plugins/{id}/diff` · `POST /v1/plugins/install` · `POST /v1/plugins/{id}/approve|reject|disable|rollback`
Jobs, secrets, triggers: `GET,POST /v1/jobs` · `GET,PATCH,DELETE /v1/jobs/{id}` · `GET /v1/secrets` · `PUT,DELETE /v1/secrets/{name}` · `POST /v1/triggers/{source}`
Reports: `POST /v1/threads/{id}/report {note?}` ("Send log" in the app; any token) saves the whole chat to the encrypted database (table `reports`, secrets scrubbed; no plain file); `GET /v1/reports`, `GET /v1/reports/{id}` (owner). Review one: `GET /v1/reports` then `GET /v1/reports/{id}` with the owner token
Audit ([Audit](audit.md)): `GET /v1/audit/review` · `GET /v1/audit/turns/{id}` · `POST /v1/notices/{n}/feedback`
Auto reports ([Audit](audit.md)): a plugin tool failing 3 times in 24 h, or an attended turn ending on an error, writes a report itself (`auto: true`, note `auto: <tool> failed N times: ...`, one per tool or error code a day, `src/autoreport.rs`); `GET /v1/reports?auto=1` lists only those; `AGENT_AUTO_REPORTS=off`
Model tests ([Models](models.md)): `POST /v1/evals/run {cloud?, profiles?, only?}` · `GET /v1/evals` · `GET /v1/evals/{id}` (owner; `src/eval.rs`, cases `evals/cases.json` + the smoke ten; results in the encrypted database (table `eval_runs`); CLI `agent-core eval [--cloud] [--server URL --token-file F]`; on the server `assistantx-eval`, token in `~/.config/assistantx/eval.token` on the Mac)
Notices: `POST /v1/notices/{n}/snooze` · `GET /v1/notices` (event `assistant.notice`: priority, title, text, silent, thread_id)
Speech: `GET,POST /v1/speech` (`{text}` in, WAV out; the Kokoro voice of the service at `AGENT_TTS_URL`, loopback only, always engine `kokoro`; 503 when unset)
Alarms, sounds: `GET /v1/alarms/ringing` · `POST /v1/alarms/snooze|dismiss` · `GET /v1/sounds` · `GET,PUT,DELETE /v1/sounds/{name}`
Calendar: `GET,POST /v1/calendar/events` · `GET,PATCH,DELETE /v1/calendar/events/{id}`
Lists: `GET,POST /v1/lists` · `GET,PATCH,DELETE /v1/lists/{id}` · `POST /v1/lists/{id}/items` · `PATCH,DELETE /v1/lists/{id}/items/{item}` · `POST /v1/lists/{id}/clear` · `GET /v1/lists/{id}/text` (plain text; the app shares it through Android's share sheet, `NativeApp.shareText`)
Location ([Location](location.md)): `GET /v1/location/fences` · `POST /v1/location/transitions` (phone geofences) · `POST,GET /v1/location/points` · `GET /v1/location/now` · `GET /v1/location/visits` · `GET,POST /v1/places` · `GET,PATCH,DELETE /v1/places/{id}`
Noticing ([Location](location.md), `src/detect.rs`): `POST /v1/detected {items}` (phone, device token ok) · `GET /v1/detected` · `GET,PATCH /v1/detected/settings` (switches leave_home, gym, parcel, bill, plan; device may GET)
Computers ([Computers](computers.md)): `GET,POST /v1/nodes` · `POST /v1/nodes/work/poll` · `POST /v1/nodes/work/{id}` (scope `node` only)
Phone actions ([Plugins](plugins.md) `phone`): `GET /v1/phone/actions` (takes waiting actions, each once) · `POST /v1/phone/actions/{id} {ok, error?, shown?, contact?}` (device, client or owner)
Models ([Models](models.md)): `GET,PATCH /v1/models` (PATCH owner only; keys through `/v1/secrets`)
Phone capture ([Phone capture](phone-capture.md)): `POST,GET,DELETE /v1/ambient/chunks` · `GET /v1/ambient/status` · `GET,PATCH /v1/ambient/settings` · `POST /v1/ambient/pause|resume|process` · `GET /v1/ambient/picks` · `POST /v1/ambient/picks/{id}`
Usage panel:

`GET /v1/usage?days=30` (owner: models used, estimated cloud money from `AGENT_PRICE_<PROFILE>_PER_MTOK`, power from `power_samples` taken every minute: RAPL + GPU when readable else CPU-load estimate with `AGENT_POWER_IDLE_W`/`MAX_W`, `AGENT_POWER_PRICE_KWH`, busiest days) · `GET /v1/security` (owner: DB encrypted, key, plain copies, disk encryption).

The page is `examples/panel/` (throwaway, calls the API only).
Status: `GET /v1/status` (owner; each local model, the voice and the sandbox: ok or the problem in plain words)
Morning and travel ([Mind](mind.md), [Location](location.md)): `POST,GET /v1/signals` (`{kind: "woke", at, source}`; POST also device scope) · `GET,PATCH /v1/brief` · `POST /v1/brief/run` · `GET,PATCH /v1/travel`
Recipes: `GET /v1/recipes` · `DELETE /v1/recipes/{id}`
Other: `GET /v1/health` · `GET,PATCH /v1/config` · `GET /v1/export` · `GET,POST /v1/tokens` · `DELETE /v1/tokens/{id}`
