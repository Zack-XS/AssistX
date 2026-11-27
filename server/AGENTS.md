# AGENTS.md

Rules for anyone (human or AI) working in this repo.

**The design lives in the docs tree at `../docs/` (start at `../docs/README.md`). It wins.** If code and docs disagree, fix the code, or change the doc on purpose in the same change. The old `server/docs/` is now archived at `server/docs-retired/` (see `../docs/RETIRED.md`); where the two differ, `../docs/` wins (map: `../docs/MIGRATION.md`). Writing code: `../docs/security/safety-rules.md` and `../docs/quality/agents-checklist.md`. Changing the app's screens: `../docs/design/` (values in `../docs/design/tokens.md`).

## Goal (docs/00)

An open-source backend that turns any language model into a personal assistant that runs a person's day. It makes the system around the model smarter: it remembers, acts, starts work on its own, and grows new abilities through plugins.

## Hard rules

1. **All backend code is Rust.** Server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS. No Python, Node or other runtimes for backend code. Shell only to run `cargo` and for quick checks. (Plugins a *user* installs or the agent builds may use any runtime the sandbox allows, per `docs/03`. That is not code in this repo.)
2. **No forced branding.** "AssistantX" is only this folder's name. Never put it, or any product name, in crate names, routes, types, prompts, strings or docs. The assistant stays unnamed; the user may name it (`05` #5).
3. **No forced front end.** This is a backend. It must work with `curl` alone. Nothing in `src/` may import, serve or depend on a front end. `examples/dummy-client/` is an optional reference client: the app's screens, shown by the Android app (`../android/`). It only calls the API; delete it and the backend still works. `../docs/design/` governs how it looks, `../docs/security/safety-rules.md` (Page, Android) what it must not do.
4. **Safety lives in code** (`00` principle 10). Prompts may remind the model; the core enforces.
5. **Outside content is data** (`00` principle 9). Tool output, files, web pages, trigger bodies, outside guides and outside-trust memory are wrapped in `<outside>`; any tag inside is broken.
6. **Few dependencies, exact pins** (`=x.y.z`), with the reason next to each in `Cargo.toml`.
7. **Local first.** Bind `127.0.0.1`. Token on every route but `/v1/health`. `Host` must match; `Origin` must be allowlisted.
8. **Every change ships with a test.** Golden conversations live in `tests/golden.rs`.
9. **Every change adds logs** for actions, errors, skips and model calls (`../docs/quality/logging.md`).
10. **Android battery rules.** App changes keep `../docs/quality/android-battery.md`: no new wake locks, services or wake-ups without a reason and a measure.
11. **Agents checklist.** Before you say done, run `../docs/quality/agents-checklist.md`.

## Run it

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -j1                      # this machine is low on memory
cargo test -j1 -- --test-threads=2   # unit + golden tests
cargo test -j1 -p agent-node         # the computer program (node/)

# Any OpenAI-compatible model (Ollama shown):
AGENT_MODEL_BASE_URL=http://127.0.0.1:11434/v1 AGENT_MODEL_NAME=llama3.1 cargo run -j1
# No model at all, scripted replies:
AGENT_MODEL_KIND=mock AGENT_MOCK_SCRIPT=script.json cargo run -j1
# Tokens from the CLI:
cargo run -j1 -- token create client phone
```

First run writes the owner token to `data/owner.token` (mode 0600); the DB keeps only its hash.

**Sandbox:** tools run under `sandbox-exec` on macOS and under bubblewrap (`bwrap`) on Linux. The core probes them at start. On Linux each run gets an empty root with read-only `/usr`, the plugin folder and the runner; read-write only its scratch, its `state/` and `needs.paths`; no network; and the server's `data/` plus `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gh` are covered by empty folders. Plugins that need the network reach the run's allowlist proxy through a Unix socket in their scratch folder (a loopback bridge inside the sandbox passes it on); there is no other route out. If no sandbox can be applied (for example when the server itself runs inside another sandbox), tools are **refused** with `sandbox_denied` unless `AGENT_UNSAFE_NO_SANDBOX=1`.

## Layout

```
docs/                    the design (source of truth: design/, quality/, security/, plans/)
examples/dummy-client/   the app's page (reference client, docs/16); ../android/ is the app
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
src/location.rs          phone points, stays, visits, places, OpenStreetMap names (docs/10)
src/signals.rs           phone signals (`woke`) and the morning brief (docs/14)
src/travel.rs            travel_time, OSRM routing, leave-now alerts (docs/10)
src/nodes.rs             connected computers: node tokens, long-poll work queue (docs/11)
src/phone.rs             phone actions queue (call, text, open_app, timers, directions) and the shared/photo text split
src/models.rs            model profiles (local 4B/9B/reasoning, NIM, Gemini, OVH), auto mode, `use_bigger_model` (docs/12)
src/lumo.rs              Proton Lumo through a loopback browser bridge (AGENT_LUMO_URL), written tool calls, cool-down to the local model (docs/12)
node/                    agent-node, the program a computer runs (macOS, Windows, Linux)
src/tz.rs                time zones from the system tz database (DST-safe schedules)
src/export.rs            export zip, undo
src/schema.rs            JSON Schema subset for tool input/output
src/ambient.rs        phone capture: text chunks in, controls, retention, picks (docs/08)
src/folders.rs        memory folder tree: index lines, confidence report, disk copy
src/openai_compat.rs  model adapter with retry and plain error messages
src/prompt.rs, model.rs, manifest.rs, events.rs, auth.rs, db.rs, config.rs, util.rs
src/plugins/<id>.rs      Rust tool code for shipped plugins (`agent-core tool-run <p> <t>`)
tests/golden.rs          golden conversations, injection and auth tests
```

## Build order status (docs/05)

| Step | What | State |
|---|---|---|
| 1 | Turn engine, model adapter, prompt, threads/messages/events, auth, tokens, tests | Done |
| 2 | Memory DB, `search_memory`, `save_note`, raw-log scrubbing, core-set trust | Done |
| 3 | Index + `load` / `unload` (plugins, `mem:`, `dir:` folders, 80-line cap, batched rebuild) | Done |
| 4 | Safety Gate, approvals, action log, loader, sandbox, proxy, hash check, user autonomy | Done (macOS sandbox only) |
| 5 | plugin-maker, build mode, drafts, approval cards, install, versions, rollback | Done |
| 6 | Dreaming (dry run, snapshot locks, trust rules), forget | Done |
| 7 | Scheduler, jobs, triggers, notices | Done |
| 8 | Undo, export | Done |

## Routes

Threads, turns, events: `POST,GET /v1/threads` · `GET,PATCH,DELETE /v1/threads/{id}` (`{model}`: this chat's own model, docs/12) · `GET,POST /v1/threads/{id}/messages` · `POST /v1/threads/{id}/cancel` · `GET /v1/threads/{id}/events` · `GET /v1/events` · `GET /v1/turns/{id}`
Approvals, actions: `GET /v1/approvals` · `GET,POST /v1/approvals/{id}` · `GET /v1/actions` · `GET /v1/actions/{id}` · `POST /v1/actions/{id}/undo`
Memory: `GET,POST /v1/memory` · `GET,PATCH,DELETE /v1/memory/{id}` · `GET /v1/memory/{id}/history` · `POST /v1/memory/{id}/forget` · `GET /v1/memory/core` · `GET /v1/memory/index`
Dreaming: `POST /v1/dreaming/run` · `GET /v1/dreaming/runs` · `GET /v1/dreaming/runs/{id}`
Plugins: `GET /v1/plugins` · `GET,DELETE /v1/plugins/{id}` · `GET /v1/plugins/{id}/versions` · `GET /v1/plugins/{id}/diff` · `POST /v1/plugins/install` · `POST /v1/plugins/{id}/approve|reject|disable|rollback`
Jobs, secrets, triggers: `GET,POST /v1/jobs` · `GET,PATCH,DELETE /v1/jobs/{id}` · `GET /v1/secrets` · `PUT,DELETE /v1/secrets/{name}` · `POST /v1/triggers/{source}`
Reports: `POST /v1/threads/{id}/report {note?}` ("Send log" in the app; any token) saves the whole chat to the encrypted database (table `reports`, secrets scrubbed; no plain file); `GET /v1/reports`, `GET /v1/reports/{id}` (owner). Review one: `GET /v1/reports` then `GET /v1/reports/{id}` with the owner token
Audit (docs/13): `GET /v1/audit/review` · `GET /v1/audit/turns/{id}` · `POST /v1/notices/{n}/feedback`
Auto reports (docs/13): a plugin tool failing 3 times in 24 h, or an attended turn ending on an error, writes a report itself (`auto: true`, note `auto: <tool> failed N times: ...`, one per tool or error code a day, `src/autoreport.rs`); `GET /v1/reports?auto=1` lists only those; `AGENT_AUTO_REPORTS=off`
Model tests (docs/12): `POST /v1/evals/run {cloud?, profiles?, only?}` · `GET /v1/evals` · `GET /v1/evals/{id}` (owner; `src/eval.rs`, cases `evals/cases.json` + the smoke ten; results in the encrypted database (table `eval_runs`); CLI `agent-core eval [--cloud] [--server URL --token-file F]`; on the server `assistantx-eval`, token in `~/.config/assistantx/eval.token` on the Mac)
Notices: `POST /v1/notices/{n}/snooze` · `GET /v1/notices` (event `assistant.notice`: priority, title, text, silent, thread_id)
Speech: `GET,POST /v1/speech` (`{text}` in, WAV out; the Kokoro voice of the service at `AGENT_TTS_URL`, loopback only, always engine `kokoro`; 503 when unset)
Alarms, sounds: `GET /v1/alarms/ringing` · `POST /v1/alarms/snooze|dismiss` · `GET /v1/sounds` · `GET,PUT,DELETE /v1/sounds/{name}`
Calendar: `GET,POST /v1/calendar/events` · `GET,PATCH,DELETE /v1/calendar/events/{id}`
Lists: `GET,POST /v1/lists` · `GET,PATCH,DELETE /v1/lists/{id}` · `POST /v1/lists/{id}/items` · `PATCH,DELETE /v1/lists/{id}/items/{item}` · `POST /v1/lists/{id}/clear` · `GET /v1/lists/{id}/text` (plain text; the app shares it through Android's share sheet, `NativeApp.shareText`)
Location (docs/10): `GET /v1/location/fences` · `POST /v1/location/transitions` (phone geofences) · `POST,GET /v1/location/points` · `GET /v1/location/now` · `GET /v1/location/visits` · `GET,POST /v1/places` · `GET,PATCH,DELETE /v1/places/{id}`
Noticing (docs/10, `src/detect.rs`): `POST /v1/detected {items}` (phone, device token ok) · `GET /v1/detected` · `GET,PATCH /v1/detected/settings` (switches leave_home, gym, parcel, bill, plan; device may GET)
Computers (docs/11): `GET,POST /v1/nodes` · `POST /v1/nodes/work/poll` · `POST /v1/nodes/work/{id}` (scope `node` only)
Phone actions (docs/03 `phone`): `GET /v1/phone/actions` (takes waiting actions, each once) · `POST /v1/phone/actions/{id} {ok, error?, shown?, contact?}` (device, client or owner)
Models (docs/12): `GET,PATCH /v1/models` (PATCH owner only; keys through `/v1/secrets`)
Phone capture (docs/08): `POST,GET,DELETE /v1/ambient/chunks` · `GET /v1/ambient/status` · `GET,PATCH /v1/ambient/settings` · `POST /v1/ambient/pause|resume|process` · `GET /v1/ambient/picks` · `POST /v1/ambient/picks/{id}`
Usage panel: `GET /v1/usage?days=30` (owner: models used, estimated cloud money from `AGENT_PRICE_<PROFILE>_PER_MTOK`, power from `power_samples` taken every minute: RAPL + GPU when readable else CPU-load estimate with `AGENT_POWER_IDLE_W`/`MAX_W`, `AGENT_POWER_PRICE_KWH`, busiest days) · `GET /v1/security` (owner: DB encrypted, key, plain copies, disk encryption). The page is `examples/panel/` (throwaway, calls the API only).
Status: `GET /v1/status` (owner; each local model, the voice and the sandbox: ok or the problem in plain words)
Morning and travel (docs/14, docs/10): `POST,GET /v1/signals` (`{kind: "woke", at, source}`; POST also device scope) · `GET,PATCH /v1/brief` · `POST /v1/brief/run` · `GET,PATCH /v1/travel`
Recipes: `GET /v1/recipes` · `DELETE /v1/recipes/{id}`
Other: `GET /v1/health` · `GET,PATCH /v1/config` · `GET /v1/export` · `GET,POST /v1/tokens` · `DELETE /v1/tokens/{id}`

## Plugins we ship

| Plugin | Backend | Tools (auto / ask) | Limits |
|---|---|---|---|
| files | rust | list, read, search / write, mkdir, move (+ undo tools) | `needs.paths` = `AGENT_FILES_ROOT` only. No delete. |
| terminal | rust | allowed / run, shell | `run`: allowlist, no shell. `shell`: any command through `/bin/sh`, tag `unrestricted` = always a yes with a loud warning showing the exact command (no setting makes it auto). Both in the sandbox, `needs.paths` = `AGENT_TERM_CWD`. |
| web | rust | fetch, links / — | Through the proxy; any public host; no redirects. |
| weather | rust | now / — | Proxy allows only the two Open-Meteo hosts. |
| spotify | core | now, devices, search, info (liked, top, top_artists, recent, playlists), play_liked / play_song, play, play_uri, pause, next, previous, volume, transfer (+ undo) | Your Spotify account through Spotify's web service, on any device. Needs Premium to control playback. Sign in once under Settings, Integrations. `play_song` searches precisely, wakes a device, checks sound really started (6 looks), restores the old music on failure, and the last seen music line is added to the index. |
| hevy | core | workouts, workout, workout_count, routines, exercise_templates / log_workout | Your Hevy log through Hevy's API. Needs a Hevy Pro API key saved as secret `HEVY_API_KEY`. The API has no delete, so logging cannot be undone (it asks first; workouts are private unless told otherwise). |
| email | core | inbox, search, read, summarize_inbox, draft, reply_draft (+ undo: delete_draft) / send | Your Gmail through Gmail's API (`gmail.googleapis.com` only). Sign in under Settings, Integrations (provider `gmail`: Google Cloud Client ID + Secret, Gmail API enabled, scopes gmail.readonly, gmail.compose, gmail.send; docs/03 Email). Everything read is outside content. `send` is tagged `always_ask`: a high-risk card with the exact To/Cc/Subject/text, never auto, never by voice. |
| google-health | core | data | Read only, through Google Health (the Fitbit Web API's replacement, `health.googleapis.com/v4`). Sign in under Settings, Integrations with a Google Cloud OAuth Client ID and Client Secret. Data types and filter names are not checked against a live account yet. |
| health | core | summary | Daily body numbers sent by the Android app from Health Connect (steps, sleep, heart rate, weight, workouts). `POST /v1/health/days` (device or client token), `GET /v1/health/days`. Table `health_days`, one row per day and metric; raw readings stay on the phone. |
| scheduler | core | create_alarms (many at once), remind_at_place (arrive/leave a named place), run_task_at_place / ask, create_reminder, create_alarm, schedule_task, update_job, list_jobs, list_sounds, snooze_alarm, dismiss_alarm, cancel_job (+ undo) / create_recurring, schedule_recurring_task | Jobs after outside content are marked `outside`; they never ring and cannot be alarms. |
| calendar | core | list_events, find_events, get_event, add_event, update_event (+ undo) / delete_event | Events with repeats and reminders (`event` jobs that follow the event, may ring). Next 24 h shown in the index. |
| lists | core | list_lists, show_list, add_items (many at once), check_items, uncheck_items, remove_items, clear_checked, rename_list (+ undo) / delete_list | Shopping, to-do or any name; items with quantity, note, due date. Names match loosely (groceries = shopping). Lists with open items shown in the index as counts. App: Plan, Lists (tick, add, share). |
| location | core | where_now, history, places, name_place (+ undo), travel_time / — | Phone points every ~5 min become visits and places; OpenStreetMap names a place after 45 min or 3 visits; the user's name wins. `travel_time` asks OSRM (`router.project-osrm.org`); leave-now alerts for calendar events with a place (docs/10). Not in the index. |
| computers | core | list_devices, info, list, stat, read, search / write, mkdir, copy, move, delete | Runs on a computer with agent-node, inside its allowed folders; key folders always refused; `--read-only` on the computer refuses all changes. |
| phone | core | open_app, set_timer, set_phone_alarm, navigate_to / call, text | Done by the Android app (`src/phone.rs`, queue `phone_actions`). Only opens screens: the dialer or a filled-in text, the user presses call or send. Contact names are looked up on the phone. App in the background: a notification to tap. No answer in 20 s: `sent_to_phone`. |
| plugin-maker | core | start_build / — | Starts a build-mode turn; the draft needs the owner. |
| memory-tools | core | remember (person, place, thing), list_disputed / set_status | Guide for bigger memory work. `remember` is also dreaming's `dream_remember`. |

## Gaps against the docs (fix later)

- **Linux sandbox** is bubblewrap only: files, and network only through the socket proxy (untested on a real Linux box yet: run web and weather there once). CPU, memory and process-count limits are not set; only wall time and output size are.
- **macOS profile not verified on a real login session** in this build: the dev machine ran the server inside another sandbox, so tests ran tools unsandboxed. Run `cargo test` from a normal Terminal to exercise it, especially Spotify's Apple Events rules.
- **HTTPS through the proxy** is checked by host only (CONNECT), so build-mode "GET only" holds for plain HTTP, not inside TLS.
- **The DB is encrypted** with SQLCipher (`src/db.rs`). Key: `AGENT_DB_KEY` (a passphrase) or else `data/db.key` (made on first run, 0600). A plain DB from an older version is encrypted at start and the plain copy kept as `data/core.db.plain-backup` (delete it by hand). The key file sits next to the DB, so this guards copies, backups and exports of the DB file, not a full copy of `data/`; set `AGENT_DB_KEY` from elsewhere for that. Secrets live inside the encrypted DB; the OS keychain (`keyring`) is still not used.
- **OAuth** only for Spotify (`src/oauth.rs`, PKCE); **no install by URL**, **no signing**, **no MCP-wrapped plugins**.
- **Forget** removes other entries built from the same sources instead of rewriting them.
- Long chats are summed up (`src/summary.rs`, docs/02): past `AGENT_SUMMARY_AT_TOKENS` (default 12000) or 40 messages, the older part is folded into one stored summary by the turn's model; the newest 12 stay word for word.
- Vectors are plain BLOBs scanned in Rust (no `sqlite-vec`); fine for thousands of entries.
- **No batches** of `low` approvals.
- **No OpenAPI file** (`utoipa`).
- **Multi-device** (`docs/07`) is not started.
- `AGENT_MAX_TOKENS_PER_DAY` (default 2000000, 0 = no cap) caps cloud model tokens per day (column `turns.cloud_tokens`). Local models never count and keep working when the cap is hit.
- Live config: only per-tool autonomy can be changed with `PATCH /v1/config`; the rest is env vars.
- `serde_yaml` is archived upstream.

## Additions to the docs (decide, then write them into docs/)

- Folders can nest and the model can make its own: skill plugin `memory-tools` (tools `list_folders`, `make_folder`, `write_entry`, `move_entry`, `move_folder`, `list_confidence`, `set_confidence`). `topic_path` is the folder an entry lives in (its own name is the title). Folders made on purpose live in table `memory_folders`; an unknown top-level name goes to `Other/`. `write_entry` takes its sources from the turn's own user/outside messages, so the core sets trust; a repeat title in the same folder updates the file. New code for the index and the disk copy is in `src/folders.rs`.
- Memory is five folders: People, Places, Questions, Things, Other. `topic_path` is forced to start with one (anything else goes under `Other/`); old entries are moved at start. The index shows each folder with a count and a few names; `load dir:People` lists its files with descriptions; `load mem:<id>` reads the profile. No plain copy is kept on disk: memory lives only in the encrypted database (security review #3). An export (`GET /v1/export`) makes a readable copy on request; old `data/memory/` folders are deleted at start.
- Memory directory in the index: `core` (basic facts) and `recent` (digest of the last chats, built from the user's own words, no model call). Both load on request. Switches: `AGENT_MEMORY_CORE=off`, `AGENT_MEMORY_RECENT=off`.
- Core tools `update_note` and `forget_note`: only the model's own unsorted notes, refused in a turn that has outside content.
- Dates: index lines, search results and loaded entries show the last-updated date; newer facts win.
- Dreaming by itself: when notes wait and the last run is older than `AGENT_DREAM_EVERY_MINS` (default 120, 0 = off), on top of the nightly job.
- Phone capture (docs/08): the repo ships only the backend; a separate Android app records, turns speech into text on the phone, and calls `/v1/ambient/*`. Scope `device` can only upload and read status. No audio and no voice data are ever accepted (400); what others say is `outside` trust; dreaming makes picks that the user confirms. `AGENT_BIND` and `AGENT_ALLOWED_HOSTS` let a private tunnel reach the server.
- Model errors: 429, 5xx and network failures are retried 3 times (2s, 6s, 15s, or the server's `Retry-After` up to 30s), each wait announced as a `turn.note` event; the final error is a plain sentence. `src/openai_compat.rs`.
- A turn that ends early (error, cancel, budget) adds an assistant message `This did not finish. ...` listing what it had saved (notes, entries, folders, other writes). Nothing saved says so.
- `core.md` is a short profile: at most 12 lines, `user` trust, confidence 0.5 or more, no open questions, ranked by importance and confidence. The rest stays in the folders.
- `GET /v1/memory/folders` (tree with counts, descriptions, average confidence) and `POST /v1/memory/folders {path, description}`.
- Attended turns (a person is waiting) always end with words: if the model stops with no text, the core asks it twice to reply; if it still says nothing, the core writes what was done (`I finished, but the model did not write a reply. ...`). Scheduled and trigger turns may stay silent. The core's nudge is a `core`-trust user message and is hidden in the page's history.
- App updates (docs/09): `GET /v1/app/version` and `GET /v1/app/apk` serve `data/app/assistant.apk` + `version.json` (size and SHA-256 come from the file). Published by `~/bin/assistantx-app-release`, which builds a release APK on the server with the server's key. The Android app checks on open and installs by itself. The page calls `NativeApp.pageReady()` when it starts; if it does not within 8 s of loading (a broken version), the app shows its own dialog with Update now (installs even with auto-update off) and Reload. `assistantx-app-release` refuses to publish a page whose script does not parse.
- Plugin sign-in: `needs.oauth: spotify` in `plugin.yaml`. `GET/DELETE /v1/oauth/{provider}`, `POST /v1/oauth/{provider}/start|finish` (owner). PKCE with a Client ID the user pastes (no client secret); Google (`google-health`) also needs a Client Secret in `start` (`client_secret`), asks for offline access, and keeps `GOOGLE_*` secrets; Gmail (`gmail`, plugin `email`) is the same but keeps its own `GMAIL_*` secrets; `GET /v1/oauth/{provider}` also says `needs_secret`, and the page then shows a Client Secret field; the core keeps `SPOTIFY_*` secrets and refreshes the access token itself. The `spotify` plugin runs in the core (`src/spotify.rs`), talks to one fixed host, and needs no sandbox, so it works on the Linux server.
- Live mode (the voice screen): `POST /v1/threads/{id}/messages {text, live: true}`. The core adds `prompts/live-mode.md` to that turn (short spoken sentences, no markdown) and gives it the core tool `listen_for_reply`, which sets `listen: true` on `GET /v1/turns/{id}` and emits `live.listen`; the phone then reads the answer out and opens the microphone. Outside live mode the tool does not exist. In live mode `ask_user` raises no card and does not wait (a waiting turn would hold up the chat): it sets `listen` and the spoken answer is the next message. The voice screen sends `interrupt: true`, so speaking again stops a turn still waiting for a yes; a cancelled wait voids its card. A waiting approval carries `voice_ok`; the phone reads its question out and listens, then answers with `via: "voice"`. Voice may answer questions and low or medium risk tool calls; high risk and plugin installs or updates are refused in code (400, "tap") and need a tap on the screen.
- Voice out: `AGENT_TTS_URL` (on the server `http://127.0.0.1:5004`, the user service `assistantx-voice`: Kokoro, British male `bm_george` + `bm_fable`, separate from the other app's voice on 5002) turns on `POST /v1/speech`. Only a loopback `http` address is used, the request always asks for the local Kokoro engine (never a cloud voice), text is cut to 1500 characters, and the app falls back to the phone's own voice when the server has none.
- A `run_plugin_tool` call to a real tool of an active plugin that is not loaded yet loads the plugin first and goes on (the gate decides as usual); the result ends with a `[core] Note` saying so. An unknown plugin or tool is still refused. Small models skipped the load and then stopped after the refusal.
- A restart ends every turn still `queued`, `running` or `waiting` with error `server_restarted` ("Please ask again"), so no client waits forever; their cards stay for a late yes.
- Health watch (`src/status.rs`): every 5 minutes, in the background, the core checks each local model (is the model server up, is the model installed), the voice and the sandbox. A part that goes down gets one silent notice ("Something is down"), and one when it works again. Nothing on start.
- Noticing (`src/detect.rs`, docs/10): leaving Home sends one notice (Open-Meteo weather and umbrella, first event today, leave reminders + the "Bring" list; once per departure, not 23:00-05:00; Home joins the phone fences). A gym visit of 25 min+ with Hevy connected and no Hevy workout in that window opens a "Log workout" chat and a notice. The phone matches parcels, bills and plans in its notifications (`Detect.kt`, codes and sign-in messages always ignored) and sends only small items to `POST /v1/detected`; they become outside-trust inbox notes and show, wrapped, in the mind digest and the index. Each switch is in Settings, Noticing.
- Quick commands: `words:` in `plugin.yaml` (spotify, weather, scheduler, calendar ship with some). When the user's own message holds one, the core loads that plugin before the first model step (at most two; never a plugin with an `.outside` marker; never from trigger or job text), so "pause my music" needs no load round trip.
- Phone notifications (`../android/` `Notices.kt`): no Google services. The app polls `GET /v1/notices` with its saved cursor on the alarm-sync timer (inexact, about every 15 minutes) and when it opens; the first run only stores the cursor. Channels: normal (sound), quiet (`silent`: no sound or buzz), urgent (high). Tap opens the notice's `thread_id`. Nothing pops up while the app page is on screen (it shows them itself).
- Events `tool.started`, `tool.done`, `thinking`.
- Photo and shared text (docs/03 `phone`): the app reads photos on the phone (ML Kit, bundled) and takes shares; text after a `[photo text]` or `[shared text]` line in a message becomes a separate `outside` message. No image bytes reach the server.
- Every tool result the model sees starts with a core line `[core] RESULT: DONE | DONE AND CHECKED | FAILED ...` built from the real outcome (`engine::verdict`), above the `<outside>` block. Small models follow it instead of guessing from the JSON. Guides open with a "Pick the right tool" table.
- Streaming: `message.delta` carries pieces of text (batched, about every 80 ms) with `message_id` = a stream id. `message.done` carries the whole text, the real `message_id`, and `stream_id` to link them. Adapters that cannot stream send one delta with the whole text and `stream_id: null`. The model reasoning text still arrives as one `thinking` event after the reply.
- Trust label `core` on messages the core writes (tool acks, job and late-yes notes).
- `needs.paths`, `needs.apple_events`, `needs.platform` in `plugin.yaml`; `runtime.builtin: rust | core`.
- Approval status `void`; approval kinds `memory_load` and `memory_question`. Core tool `ask_user(question, options?)` raises a `question` approval; answer `{answer:"reply",text}` or `no`. Core tool `notify(text, title?, silent?, priority?)` sends a notice (max 5 per turn; outside content cannot make it urgent). Job kind `task` (`schedule_task`, `schedule_recurring_task`) starts an unattended turn in its own thread `system:task:<job>:<time>`, whose last reply is the report, plus a silent "Report ready" notice.
- `.outside` marker file in a plugin folder: its guide is wrapped as outside content.
- Script tool protocol: stdin `{"tool","args"}`, stdout `{"ok":true,"result":...}` or `{"ok":false,"error":...}`; tests in `tests/*.json` as `{"tool","args","expect_ok"}`.
- Undo tools get `{"args", "result"}` of the action they undo.

## Main server (docs/09)

The code runs all day on a Debian server, `ssh your-server`, at `http://your-server-ip:8790`. Work on the Mac, then run `assistantx-deploy` (all `~/bin/assistantx-*` scripts reach the server through `your-ssh-wrapper`: the home address when it answers, else Tailscale (your Tailscale IP)) (outside the repo, `~/bin`): it sends the source, builds on the server and restarts the service. `assistantx-deploy smoke` (or `assistantx-smoke` alone, both in `~/bin`) then makes a chat on `local-light`, sends 10 fixed requests (weather, music, reminder set and cancel, calendar, memory) and fails if a turn errors, a turn ends on a FAILED tool result, or a reply is empty; the chat is archived after and the owner token is never printed. The server has its own `data/`. Tools run there in the bubblewrap sandbox (terminal, files); tools that need the network are refused there. The Android app is `android/`, next to `server/` in the same parent folder (not in git).

## Working here

- Rust lives at `~/.cargo/bin`. Build and test with `-j1` on this machine.
- Never commit `data/`, `.token` or `target/`.

## Memory people, dates, alarms (added Nov 2026)

- People, places and things live in their own folder: `People/mia/` holds `Mia profile` (Relation, Birthday, Likes, Dislikes, Groups, Facts) and `Mia timeline` (dated lines). The core writes both from one fixed call (`remember` in chat, `dream_remember` in dreaming; one name per call, two names are refused) and merges into what exists. `Groups` also write `People/groups/<Group> group` with a member list. Dreaming's kickoff lists flat `People` entries so the model converts them.
- Dates in memory look like `(Fri 2026-11-20, upcoming)`. The core computes the weekday and flips `upcoming` -> `today` -> `done` (`remember::refresh_dates`, at each real dreaming run and every 3 hours from `maybe_auto`). Words like "Friday" or "tomorrow" in text written by dreaming or `write_entry` get the exact date added after them (`remember::annotate`).
- Alarms and reminders on chosen dates: `create_alarm {time|times, dates|days, label}` (dates never repeat, days repeat weekly, not both), `create_alarms {alarms:[...]}` up to 30; each date is its own job; undo cancels the batch (`result.ids`).
- A held notice (quiet hours, daily cap) is replaced by a new delivered row when released, so a phone whose cursor already passed it still gets it. The phone keeps a posted-list so the page and the background check never show one twice.
- A tool call that needs a yes while nobody answers tells the model the tool works and only the tap is missing; a repeat of the same call reuses the pending card. Failed calls of common tools carry `right_way` (an exact example, `engine::usage`).
- Battery: the events stream keep-alive is 55 s (was 15 s); the phone's 15-minute check is a re-arming `setAndAllowWhileIdle` alarm (the old inexact repeating alarm was put off for hours in Doze); location slows to 15 minutes after three points within 150 m.

## Search by meaning and recipes (added Nov 2026, docs/04)

- `src/embed.rs`: `AGENT_EMBED_MODEL` (server: `nomic-embed-text`) + `AGENT_EMBED_URL` (default local Ollama, loopback only). Vectors in `memory_vectors`, embedded in the background by the core timer (new, changed, backfill, model change); `search_memory` = FTS5 + vectors merged by RRF; falls back to FTS5 when the embedder is off or down.
- `src/recipes.rs`: after an attended turn with 3+ successful plugin calls, one low-risk `question` card offers to save the steps as a recipe (typed values become `{{placeholders}}`). Core tools `list_recipes`, `run_recipe {name, inputs}`, `forget_recipe`; index line `recipes`; Settings > Recipes in the page. An `outside` recipe asks before every step.
