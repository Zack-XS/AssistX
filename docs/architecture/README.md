---
title: Architecture
description: How the server is built, part by part.
order: 3
---

# Architecture

The server is one Rust program. Start with the parts picture in [What AssistX is](../overview/what-is-assistx.md).

| Page | What it covers |
|---|---|
| [API](api.md) | Transport, auth, scopes, every route |
| [Events](events.md) | The SSE stream and event types |
| [Turn engine](turn-engine.md) | Threads, turns, Safety Gate, limits, power |
| [Prompt system](prompt-system.md) | Prompt layers, index, outside content |
| [Memory](memory.md) | Stores, trust, folders, profiles, search, forget |
| [Recipes](recipes.md) | Saved multi-step runs |
| [Plugin format](plugins.md) | `plugin.yaml`, tags, tools, sandbox |
| [Scheduler and notices](scheduler-and-notices.md) | Jobs, alarms, tasks, notices |
| [Voice](voice.md) | Live mode, speech |
| [Models](models.md) | Profiles, auto mode, model tests |
| [Accounts](accounts.md) | Accounts, keys, remembered phones |
| [Multi-device](multi-device.md) | Plan, not built |
| [Location](location.md) | Places, triggers, geofences, travel |
| [Phone capture](phone-capture.md) | Heard speech as text |
| [Computers](computers.md) | agent-node |
| [Audit](audit.md) | Traces, review pack, reports |
| [Mind](mind.md) | The thinking task, morning brief |
| [Deploy](deploy.md) | The main server |

## Code map

```
docs/                    the design (source of truth)
prompts/                 personality.md, turn-rules.md, plugin-maker.md, dreaming.md
plugins/<id>/            plugin.yaml + guide.md (+ schemas/, tools/, tests/, state/)
plugins/.drafts/<id>/    drafts waiting for approval      plugins/.versions/<id>/<v>/  older versions
src/lib.rs               App: per-thread queues, cancel, waiters, loaded items, index cache, schema
src/api.rs               /v1 routes, guard (Host, Origin, token, scope), SSE replay
src/engine.rs            turns in 3 modes (normal, build, dreaming), gate, approvals, action log
src/gate.rs              effective autonomy, risk from tags, call hash, card question
src/runner.rs            one sandboxed process per tool run: proxy, schemas, hash check, secrets, limits
src/sandbox.rs           sandbox per run: macOS profile, Linux bubblewrap arguments
src/proxy.rs             per-run allowlist proxy; own DNS, refuses private addresses
src/store.rs             plugin lifecycle: state, hash, drafts, cards, approve, rollback, install, secrets, build fetch
src/memory.rs            inbox, entries, history, FTS5 search, trust, core facts, index lines
src/dreaming.rs          dreaming run, memory-only tools, core-built core facts, forget
src/status.rs            health watch and GET /v1/status
src/usage.rs             model_usage + power_samples, GET /v1/usage, GET /v1/security
src/scheduler.rs         jobs, core timer, scheduler tools, alarms (ring, snooze, dismiss, missed), notices (quiet hours, daily cap)
src/sounds.rs            built-in sound names and custom ringtones (data/sounds/)
src/remember.rs          `remember`: one folder per person/place/thing (profile + timeline), exact dates (Fri 2026-11-20, upcoming -> done), group member lists
src/calendar.rs          calendar events, repeats, reminders as `event` jobs, index lines
src/lists.rs             lists (shopping, to-do, any name): items, tick, undo, index counts, `/v1/lists` routes, plain-text share
src/location.rs          phone points, stays, visits, places, OpenStreetMap names (location.md)
src/signals.rs           phone signals (`woke`) and the morning brief (mind.md)
src/travel.rs            travel_time, OSRM routing, leave-now alerts (location.md)
src/nodes.rs             connected computers: node tokens, long-poll work queue (computers.md)
src/phone.rs             phone actions queue (call, text, open_app, timers, directions) and the shared/photo text split
src/models.rs            model profiles (local 4B/9B/reasoning, NIM, Gemini, OVH), auto mode, `use_bigger_model` (models.md)
src/lumo.rs              Proton Lumo through a loopback browser bridge (AGENT_LUMO_URL), written tool calls, cool-down to the local model (models.md)
node/                    agent-node, the program a computer runs (macOS, Windows, Linux)
src/tz.rs                time zones from the system tz database (DST-safe schedules)
src/export.rs            export zip, undo
src/schema.rs            JSON Schema subset for tool input/output
src/ambient.rs        phone capture: text chunks in, controls, retention, picks (phone-capture.md)
src/folders.rs        memory folder tree: index lines, confidence report, disk copy
src/openai_compat.rs  model adapter with retry and plain error messages
src/prompt.rs, model.rs, manifest.rs, events.rs, auth.rs, db.rs, config.rs, util.rs
src/plugins/<id>.rs      Rust tool code for shipped plugins (`agent-core tool-run <p> <t>`)
tests/golden.rs          golden conversations, injection and auth tests
```

More modules in `server/src/` not in that list: `hub.rs`, `hub_http.rs`, `vault.rs` (accounts), `power.rs`, `detect.rs`, `embed.rs`, `recipes.rs`, `links.rs`, `summary.rs`, `audit.rs`, `autoreport.rs`, `mind.rs`, `eval.rs`, `email.rs`, `spotify.rs`, `hevy.rs`, `health.rs`, `google_health.rs`, `oauth.rs`, `reports.rs`, `util.rs`.

## Tech stack (planned)

| Part | Choice |
|---|---|
| Async runtime | `tokio` |
| HTTP + SSE | `axum` |
| API schema | `utoipa` (OpenAPI file generated from code, so front ends get typed clients) |
| JSON + schemas | `serde`, `schemars` (JSON Schema for tool inputs/outputs) |
| Database | SQLite via `rusqlite`, FTS5 for text search, `sqlite-vec` for vectors |
| Encryption at rest | SQLCipher build of SQLite |
| Secrets | `keyring` crate (OS keychain), encrypted file fallback |
| Sandbox | Linux: Landlock + seccomp (`landlock`, `seccompiler`) or bubblewrap. macOS: `sandbox-exec` profile. Network through a local allowlist proxy. |
| Logs | `tracing` |
| Supply chain | `cargo-deny`, `cargo-audit` in CI |

Core crates use `#![forbid(unsafe_code)]` where possible.

> Code check: the code uses `axum`, `tokio`, `rusqlite` with bundled SQLCipher, `serde`, `serde_json`, `serde_yaml`, `sha2`, `ureq`, `argon2`, `chacha20poly1305`, `tower`. Not used today: `utoipa` (no OpenAPI file), `sqlite-vec`, `keyring`, `schemars`, `tracing`, `landlock`, `seccompiler`. Linux uses bubblewrap. See `server/Cargo.toml`.
