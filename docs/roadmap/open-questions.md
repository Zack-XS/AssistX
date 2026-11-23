---
title: Open questions
description: Decisions still needed, and where old docs and code disagree.
order: 3
---

# Open questions

## Decisions

Status of each: **Decided**, **Proposed** (suggestion, needs a yes), or **Open**. The other docs already assume the Proposed storage, sandbox and secrets answers (#2, #3, #9). Change them here first if they move.

| # | Question | Status | Answer | Reason |
|---|---|---|---|---|
| 1 | Language and runtime | **Decided** | Rust (`tokio`, `axum`) for the core and API. Plugin tools can be in any language. | Speed, memory safety, one binary. Front ends get typed clients from the generated OpenAPI file. |
| 2 | Storage | Proposed | SQLite (SQLCipher build) via `rusqlite`, FTS5 for text, `sqlite-vec` for vectors | One local file, easy backup, works offline. |
| 3 | Sandbox on macOS and Linux | Proposed | Linux: Landlock + seccomp (or bubblewrap). macOS: `sandbox-exec` profile. Both: network through a local allowlist proxy. Containers later as an option. | Light, starts fast. A proxy is the simplest way to limit hosts. `sandbox-exec` is deprecated but still works; watch for a replacement. |
| 4 | Which models first | Proposed | First: an OpenAI-compatible adapter. Second: Anthropic. | The first also covers Ollama, vLLM and many hosted models. The second proves the adapter interface. |
| 5 | Project name | Open | — | Owner's call. The assistant itself stays unnamed and the user may name it. |
| 6 | License | Proposed | Apache-2.0 | As easy to modify as MIT, plus a patent grant. |
| 7 | Proactive behavior | Proposed | Quiet hours, a daily cap on unasked messages, and three priorities: `urgent` (any time, still capped), `normal` (outside quiet hours), `digest` (batched into a morning or evening brief). | Stops nagging. Front ends decide how each priority is shown. Delivered as the `assistant.notice` event. |
| 8 | Multi-user | Proposed | Out of scope, but every table has `owner_id` and every path can be scoped per user. | Cheap now, costly later. |
| 9 | Secrets storage | Proposed | OS keychain via the `keyring` crate; encrypted file fallback for headless Linux. | Keychain is safest; servers often lack one. |
| 10 | Evaluation | Proposed | Golden conversations per adapter, plus dreaming tests (merge, conflict, forget) and injection tests (see [Threat model](../security/threat-model.md)). Run in CI on every prompt change. | Merges and attacks fail quietly; tests catch them. |


## Still open

- Plugin signing format (minisign, Sigstore, or none in v1).
- How the user grants OAuth for plugins on a headless install (device-code flow vs a link from the front end).
- Backup format and schedule.
- How a front end that holds only a `client` token handles a `high` approval. Proposed: the owner token is traded for a 5-minute token that can answer one approval, so the front end never stores the owner token.
- Whether SQLCipher and `sqlite-vec` build and work together. Check this in step 2; it decides the storage answer.
- Database migrations: how the schema is versioned, and a backup taken before each migration.
- What `sandbox-exec` replaces on macOS, if Apple removes it. Until then, document that this layer may break on new OS versions.

## Multi-device questions

- Should the host be allowed to move to another device (failover), or stay fixed in v1?
- How does a headless node pair when it has no screen for the code?
- Do we need device groups ("all home devices") for routing rules?
- Should location (home, work) come from the owner only, or may a trusted GPS device update it?

## Found while writing these docs (2026-11-22)

| Topic | Old docs say | Code or files say |
|---|---|---|
| Name | No product name in code, strings or docs (`server/AGENTS.md` rule 2) | Decided: the project is AssistX in docs. Code carries no product name. Screens use one display name, default "Assistant" ([Code style](../contributing/code-style.md) rule 2). |
| Front end | `examples/dummy-client/` is throwaway | Decided: an optional reference client, the app's real screens; `src/` never depends on it ([Code style](../contributing/code-style.md) rule 3). |
| Android project | A separate project `AssistantX-Android` | It is `AssistX/android/`, not in git. |
| Gradle page copy | `copyWeb` copies the page | It points at `../AssistantX/examples/...`, which is gone, so it skips. Fix to `../server/...`. |
| Linux tools on the main server | Off, no Linux sandbox (old deploy doc) | Code has a bubblewrap sandbox; newer note says tools run in it, network tools refused. Check on the server. |
| DB key | In the OS keychain | `AGENT_DB_KEY` or `data/db.key`; `keyring` not used. |
| OAuth routes | `POST /v1/secrets/{name}/oauth/start`, `GET /v1/oauth/callback` | `/v1/oauth/{provider}`, `/start`, `/finish`. |
| OAuth providers | Only Spotify (gaps list) | Spotify, Google Health and Gmail. |
| Approval batches | Low-risk calls are batched | Not built. |
| OpenAPI | Generated with `utoipa` | Not built. |
| Rate limits | 60 messages a minute per token, 10 triggers a minute | Message, notice and speech limits exist; exact numbers not checked here. |
| Port | Docs show 8790 | Code default is 8787; the main server sets 8790. |
| Phone capture page | Three tabs | Four tabs. |
| Review packs | Saved to `AssistantX-reviews/` | Found in `AssistX/reviews/`. They hold real chats; keep them private. |
