---
title: Configuration
description: Every environment setting, its default, and what changes at run time.
order: 3
---

# Configuration

The server reads its settings from environment values (on the main server: `data/server.env`, mode 0600). Only per-tool autonomy can change at run time, through `PATCH /v1/config` (owner). Model choices change through `PATCH /v1/models`, keys through `PUT /v1/secrets/{name}`.

Defaults below are read from `server/src/config.rs` and the modules that use them.

| Setting | Default | What it does |
|---|---|---|
| `AGENT_PORT` | 8787 | Port to listen on. |
| `AGENT_BIND` | 127.0.0.1 | Address to listen on. Anything else is opt-in; use a private tunnel. |
| `AGENT_ALLOWED_HOSTS` | (none) | Extra `Host` values accepted besides 127.0.0.1:<port>. |
| `AGENT_ALLOWED_ORIGINS` | (none) | Browser origins allowed (the app's page: `https://appassets.androidplatform.net`). |
| `AGENT_DATA_DIR` | data | Where the database, tokens and files live. |
| `AGENT_PLUGINS_DIR` | plugins | Plugin folder. |
| `AGENT_PROMPTS_DIR` | prompts | Prompt folder. |
| `AGENT_SINGLE_USER` | off | `1` = old one-owner mode, no accounts. |
| `AGENT_DB_KEY` | (key file) | DB passphrase; else `data/db.key` is made. |
| `AGENT_MODEL_KIND` | openai | `openai` (any OpenAI-compatible API) or `mock`. |
| `AGENT_MODEL_BASE_URL` | http://127.0.0.1:11434/v1 | Model API address for the `env` profile. |
| `AGENT_MODEL_NAME` | llama3.1 | Model name for the `env` profile. |
| `AGENT_MODEL_API_KEY` | (none) | Key for the `env` profile. |
| `AGENT_MODEL_MODE` | auto | Default model mode ([Models](../architecture/models.md)). |
| `AGENT_LOCAL_MODEL_BASE` | http://127.0.0.1:11434/v1 | Local model server (Ollama). |
| `AGENT_MOCK_SCRIPT` | (none) | Scripted replies for `mock`. |
| `AGENT_LUMO_URL / _TOKEN / _TIMEOUT_SECS / _COOLDOWN_MINS` | off / - / 300 / 30 | Lumo browser bridge ([Models](../architecture/models.md)). |
| `AGENT_PRICE_<PROFILE>_PER_MTOK` | (none) | Price per million tokens, for the usage panel. |
| `AGENT_MAX_TOKENS_PER_DAY` | 2000000 | Cloud tokens per day. 0 = no cap. Local models never count. |
| `AGENT_SUMMARY_AT_TOKENS` | 12000 | When long chats are summed up. 0 = never. |
| `AGENT_APPROVAL_WAIT_SECS` | 600 | How long a turn waits for a yes ([Approvals](../agents/approvals.md)). |
| `AGENT_SENSITIVE_TO_CLOUD` | ask | `ask`, `never` or `allow`: sensitive memory and cloud models. |
| `AGENT_TIMEZONE` | system zone | Time zone for jobs. |
| `AGENT_QUIET_HOURS` | 22:00-07:00 | Quiet hours for notices. |
| `AGENT_NOTICES_PER_DAY` | 10 | Daily cap on unasked notices. |
| `AGENT_DREAMING_HOUR` | 3 | Hour of the nightly dreaming run. |
| `AGENT_DREAM_EVERY_MINS` | 120 | Dream by itself when notes wait. 0 = off. |
| `AGENT_MEMORY_CORE / AGENT_MEMORY_RECENT` | on | `off` hides the `core` / `recent` index lines. |
| `AGENT_MIND` | on | `off` stops the Mind task being made. |
| `AGENT_EMBED_MODEL / AGENT_EMBED_URL` | off / http://127.0.0.1:11434/v1 | Search by meaning ([Memory](../architecture/memory.md)). |
| `AGENT_TTS_URL` | off | Local voice service for `/v1/speech` (loopback only). |
| `AGENT_FILES_ROOT` | ~/Documents | The only folder the files plugin may use. |
| `AGENT_TERM_CWD / AGENT_TERM_ALLOW` | ~ / built-in list | Terminal work folder and allowed programs. |
| `AGENT_UNSAFE_NO_SANDBOX` | off | `1` runs tools with no sandbox. Unsafe. |
| `AGENT_PROXY, AGENT_PROXY_SOCK, AGENT_RUNNER_EXE` | (set by core) | Used by the core for tool runs. |
| `AGENT_LOCATION_KEEP_DAYS` | 90 | Days raw location points are kept. |
| `AGENT_GEOCODE_BASE` | https://nominatim.openstreetmap.org | Place names and address lookup. |
| `AGENT_ROUTING_BASE` | https://router.project-osrm.org | Travel times. |
| `AGENT_WEATHER_BASE / AGENT_BRIEF_WEATHER_BASE` | https://api.open-meteo.com | Weather for leaving home and the brief. `off` skips. |
| `AGENT_SPOTIFY_API, AGENT_OAUTH_BASE, AGENT_GMAIL_API, AGENT_GOOGLE_HEALTH_API, AGENT_GOOGLE_AUTH_URL, AGENT_GOOGLE_TOKEN_URL, AGENT_HEVY_API` | the real services | Service addresses, changed for tests. |
| `AGENT_TRACE_KEEP_DAYS` | 30 | Days full model requests are kept ([Audit](../architecture/audit.md)). |
| `AGENT_AUTO_REPORTS` | on | `off` stops automatic reports. |
| `AGENT_EVAL_CASES, AGENT_EVAL_SERVER, AGENT_EVAL_TOKEN` | evals/cases.json | Model tests. |
| `AGENT_POWER_IDLE_W / _MAX_W / _PRICE_KWH` | (estimates) | Power estimate for the usage panel. |
| `AGENT_CURRENCY` | (none) | Currency shown for money. |

Never commit keys or `server.env`. Secrets for plugins (for example `HEVY_API_KEY`) are set with `PUT /v1/secrets/{name}` and stored in the encrypted database.

## Cost limits

The design for budgets is in [Turn engine](../architecture/turn-engine.md), Cost limits.
