---
title: Models
description: Model profiles, auto mode, cloud rules and model tests.
order: 10
---

# Models

## Model requirements

"Any model" means any model that meets these:

- Reliable tool calling (native or through the adapter's JSON mode).
- A context window of at least 32k tokens.
- Follows a system prompt.

Adapters for models without native tool calls use a strict JSON reply format, which the adapter parses and checks. The golden conversations are run against each supported adapter.

Status: built (v0.1). `src/models.rs`, `GET/PATCH /v1/models`.

## Profiles

| id | What | Where | Key (secret) |
|---|---|---|---|
| `local-light` | Qwen 3.5 4B | this server's Ollama | none |
| `local` | Qwen 3.5 9B, the everyday model | this server's Ollama | none |
| `local-reasoning` | Phi-4 reasoning 14B | this server's Ollama | none. **Warning:** very slow here and takes most of the memory; not recommended |
| `nim` | NVIDIA NIM, `nvidia/nemotron-3-ultra-550b-a55b` | integrate.api.nvidia.com | `NIM_API_KEY`, else `AGENT_MODEL_API_KEY` |
| `gemini` | Google Gemini, `gemini-3.5-flash` | generativelanguage.googleapis.com (OpenAI-compatible) | `GEMINI_API_KEY` |
| `ovh` | OVH AI Endpoints, `gpt-oss-120b` | oai.endpoints.kepler.ai.cloud.ovh.net | `OVH_AI_KEY` (optional: works without one at a low rate) |
| `env` | whatever `AGENT_MODEL_*` says | | `AGENT_MODEL_API_KEY` |

The local address is `AGENT_LOCAL_MODEL_BASE` (default `http://127.0.0.1:11434/v1`). Keys are set with `PUT /v1/secrets/{name}` and never shown. A profile's model name can be changed: `PATCH /v1/models {"set": {"gemini": {"model": "gemini-3.6-flash"}}}`.

`nim`, `gemini` and `ovh` are **off the list until added**: `PATCH /v1/models {"add": "nim"}` puts one on, `{"remove": "nim"}` takes it off (and resets `cloud`, `mode` or job picks that named it).

`GET /v1/models` lists them under `can_add` (the page shows an Add button for each); listed profiles carry `removable`. One whose key is already saved counts as added unless it was removed. `lumo` shows when its bridge is set up (Provider notes).

## Choosing

`PATCH /v1/models` (owner):

- `mode`: `auto` (default, or `AGENT_MODEL_MODE`) or a profile id to always use that one.
- `cloud`: the cloud profile Auto (Mix) may move up to (default `lumo`).
- `cloud_upgrade`: `ask` (default), `auto`, or `never`.

**Per chat.** A chat may pick its own: `POST /v1/threads {"model": ...}` or `PATCH /v1/threads/{id} {"model": ...}` with `auto-local`, `auto-mix` or a profile id; `null` (or `"default"`) follows `mode` above.

The chat's pick decides where a turn starts and how far `use_bigger_model` may go (an `auto-local` chat never reaches the cloud). Pinning a cloud profile needs the owner; local picks and the two auto modes are fine for a client token.

Jobs and triggers run in system threads and keep the default.

**Auto (Smart)** (`auto-smart`): background work (dreaming, scheduled jobs and automations, outside events and the notices they raise) and every chat turn that is not a quick command or small talk start on `lumo`; quick commands and small talk stay on `local-light`.

With no Lumo bridge, `local` takes Lumo's place; while Lumo is limited or its bridge is down, the Lumo adapter hands the turn to `local` itself. A job with its own model keeps it.

**Auto.** A turn for a very light message (at most 8 words, nothing to do: "thanks", "hi there", "ok cool") starts on `local-light`; every other turn, and dreaming and builds, on `local`. The model has the core tool `use_bigger_model {reason}`:

- `local-light` moves to `local` at once.
- `local` or `local-reasoning` moves to the cloud profile only after the user says yes (an approval of kind `model_upgrade`, with the model's reason). Unattended turns never move to the cloud.
- A cloud model cannot move further.

Each choice and move is an event: `model.chosen {profile, label, why: start | upgrade, from?}`.

## Cloud and private data

The sensitive-memory rule follows the model answering now: a sensitive entry asks before it reaches a cloud profile, also after an upgrade. Every key a profile may use is scrubbed from history and tool output.

## Provider notes

- Gemini 3 sends a thought signature with each tool call (`extra_content`); the core keeps it and sends it back, as Gemini requires.
- Lumo (lumo.proton.me) has no API yet. Profile `lumo` (`src/lumo.rs`) talks to a browser bridge on the same machine (for example Lumo-Api-V2: one headless browser signed in to the owner's own account, `POST / {"prompt"}` returns plain text).

  It shows only when `AGENT_LUMO_URL` is set, and only a loopback `http` address is used; `AGENT_LUMO_TOKEN` is sent as a bearer token when set.

  The request is written out as one prompt; tool calls come back as `CALL {"name", "arguments"}` lines (calls to tools not offered are dropped). One request at a time; `AGENT_LUMO_TIMEOUT_SECS` (default 300).

  Each request names Lumo's model and speed from the turn's `purpose` (`model::Purpose`): a chat a person waits on gets Lite in Fast mode, dreaming gets Max in Thinking mode, other background work Lite in Thinking mode.

  When Lumo limits the account, the adapter rests for `AGENT_LUMO_COOLDOWN_MINS` (default 30; 2 minutes when the bridge is down) and the `local` model answers meanwhile, with a `turn.note` saying so.

  It never changes address, account or browser to get around a limit. The bridge is not code in this repo.

## Model tests

Status: built. `src/eval.rs`, `evals/cases.json`.

Fixed requests are sent to each local profile in throwaway chats (titled `eval: ...`, archived after; reminders they made are cancelled) and scored per case:

**right tool** (any tool in the case's `expect` was called; `[]` = no tool should run; left out = not scored), **no FAILED** (the turn did not end on a `[core] RESULT: FAILED` tool result; `may_fail` cases only warn), **replied** (non-empty reply), plus steps (model calls), seconds and tokens.

A tool that waits for a yes counts as answered and the turn is stopped, so nothing runs without the owner. The cases are the smoke test's ten requests plus `evals/cases.json` (`AGENT_EVAL_CASES` to change).

Cloud profiles run only with `cloud: true` / `--cloud` (they cost money).

- `POST /v1/evals/run {cloud?, profiles?, only?, wait_secs?}` (owner) starts a run in the background (one at a time, 409 otherwise); `GET /v1/evals` lists runs (newest first, with the per-model and per-skill tables) and the one `running`; `GET /v1/evals/{id}` is one run in full. Results are saved to `data/evals/<time>.json` (0600, secrets scrubbed).
- `agent-core eval [--cloud] [--profiles a,b] [--only ids-or-skills] [--wait-secs N]` runs it: with `--server URL --token-file F` it asks a running server (an account's key lives only there) and prints the table; without, it runs on the one-owner data folder.
- `assistantx-eval` (outside the repo, `~/bin`) runs it on the main server over ssh. It needs an owner token of the account in `~/.config/assistantx/eval.token` (0600, made once in the app); the token goes over ssh stdin and is never printed.
- The server panel (`examples/panel/`) shows the last run and has a button to start one.
