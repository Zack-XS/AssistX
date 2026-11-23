---
title: Next
description: Known gaps and work not built yet.
order: 2
---

# Next

## Gaps against the design

- **Linux sandbox** is bubblewrap only: files, and network only through the socket proxy (untested on a real Linux box yet: run web and weather there once). CPU, memory and process-count limits are not set; only wall time and output size are.
- **macOS profile not verified on a real login session** in this build: the dev machine ran the server inside another sandbox, so tests ran tools unsandboxed. Run `cargo test` from a normal Terminal to exercise it, especially Spotify's Apple Events rules.
- **HTTPS through the proxy** is checked by host only (CONNECT), so build-mode "GET only" holds for plain HTTP, not inside TLS.
- **The DB is encrypted** with SQLCipher (`src/db.rs`). Key: `AGENT_DB_KEY` (a passphrase) or else `data/db.key` (made on first run, 0600).

  A plain DB from an older version is encrypted at start and the plain copy kept as `data/core.db.plain-backup` (delete it by hand).

  The key file sits next to the DB, so this guards copies, backups and exports of the DB file, not a full copy of `data/`; set `AGENT_DB_KEY` from elsewhere for that. Secrets live inside the encrypted DB; the OS keychain (`keyring`) is still not used.
- **OAuth** only for Spotify (`src/oauth.rs`, PKCE); **no install by URL**, **no signing**, **no MCP-wrapped plugins**.
- **Forget** removes other entries built from the same sources instead of rewriting them.
- Long chats are summed up (`src/summary.rs`, [Prompt system](../architecture/prompt-system.md)): past `AGENT_SUMMARY_AT_TOKENS` (default 12000) or 40 messages, the older part is folded into one stored summary by the turn's model; the newest 12 stay word for word.
- Vectors are plain BLOBs scanned in Rust (no `sqlite-vec`); fine for thousands of entries.
- **No batches** of `low` approvals.
- **No OpenAPI file** (`utoipa`).
- **Multi-device** ([Multi-device](../architecture/multi-device.md)) is not started.
- `AGENT_MAX_TOKENS_PER_DAY` (default 2000000, 0 = no cap) caps cloud model tokens per day (column `turns.cloud_tokens`). Local models never count and keep working when the cap is hit.
- Live config: only per-tool autonomy can be changed with `PATCH /v1/config`; the rest is env vars.
- `serde_yaml` is archived upstream.

## Phone capture: not built yet

- Pairing by QR code, and listing devices with their last upload.
- Turning a kept appointment into a reminder automatically.
- Speaker-labelled summaries of a whole conversation.
- Encrypting the capture log at rest (same gap as the rest of the database).

## Computers: not yet

- Running commands on a computer (would be an `unrestricted` tool, like the terminal's `shell`).
- Sending a file from one computer to another, or to the phone.
- Signed builds; macOS may ask to allow an unsigned program the first time.

## Multi-device build order

1. Schema and parser: `devices.toml` + env merge + validation + `devices` table.
2. Read-only API and tools: `GET /v1/devices`, `list_devices`, `device_info`, device index in the prompt.
3. Per-device tokens, `origin_device` on every turn, `last_seen` and status.
4. Pairing and revoke (CLI + API) with action-log entries.
5. Safety Gate device checks and `can_approve` on approvals.
6. Notice routing and the short offline queue.
7. Nodes: tools that run on another device.

## Accounts: build order (done) and known gaps

1. Per-account home: data folder, database, settings, models, jobs. Account store and admin routes. 2. Key wrapping, passphrase and recovery key flow, unlock and lock. 3. Per-account tokens and route isolation, with cross-account tests on every route. 4.

   Phone: server pick, server token, account sign-in, create-account screens with the recovery-key ceremony. 5. Admin page. 6. Migration of today's data into the first account.

Known gaps are listed in [Accounts](../architecture/accounts.md), As built.

## App reshape plan (R1 review)

A review on 2026-11-22 planned a design doc and a safe-code doc for the app. Both are written: [Screens](../design/screens.md), [Tokens](../design/tokens.md) and [Safety rules](../security/safety-rules.md). Round 2 risks and acceptance:

What can break:
- **`NativeApp.pageReady()`** (`index.html`, 2 calls; `MainActivity.kt:765`). If a reshape throws before it runs, the app shows the rescue dialog after 8 s (`:151`) for every user. Keep the call first, guarded.
- **Native -> page callbacks:** the app calls `window.__open`, `__ready`, `__captureChanged`, `__updateChanged`, `__locationChanged`, `__voiceChanged`, `__healthChanged`, `__dictation`, `__dictationState`, `__oauthCheck`, `__shared`, `__photoText`, `__setupChanged` (`MainActivity.kt:74-729`).

  Renaming or dropping any = silent breakage. Done: it is the [bridge contract](../android/app-structure.md#bridge-contract-frozen).
- **Deep links by name:** `openPendingView` and `goPlan("calendar"|"new-event"|"alarms"|"new-alarm"|"lists")` (`index.html:1188-1190`) are hit by home-screen shortcuts (`android/app/src/main/res/xml/shortcuts.xml`, extra `open`), allowed by `OPEN_VIEWS = calendar, alarms, new-event, new-alarm, new-chat` (`MainActivity.kt:1025`). Keep names, or change both sides.
- **`localStorage` keys** (`planTab` `:1180`, others): renamed keys reset users' choices.
- **Release script:** `~/bin/assistantx-app-release:8-18` copies one file and only checks the first `<script>` parses with jsc. Splitting into several files, or adding a second script block, breaks the copy or the check. Keep one self-contained HTML file, or update the script and `build.gradle.kts:59` together.
- **Native screens** (`AssistActivity`, `RingActivity`) will look old next to a new page unless colors are mirrored.
- **Server-side:** `AGENT_ALLOWED_ORIGINS` must still include `https://appassets.androidplatform.net` (`server/docs/09-deploy.md:46`); don't change the host.
- **Memory:** the Mac is low on RAM; the APK builds on the server (`assistantx-app-release`), so round 2 must not need a local Gradle build.

Acceptance for round 2 (suggest):
1. jsc parse check passes (`assistantx-app-release` step) and the page calls `pageReady` on load.
2. A checklist in the PR: each of the 21 Settings rows + 3 Plan segments + chat history + connect/account reachable in the new map (from the [Screens](../design/screens.md#where-every-screen-goes) table).
3. Every `NA.*` call and `window.__*` callback above still present (grep diff = 0 removed, or both sides changed).
4. Screenshots light + dark of each top place, phone width, from a real install (or the Mac test page) — before and after.
5. No new `innerHTML` with data, no inline `style:` added; tokens only.
6. `build.gradle.kts:59` path fixed; `cargo test -j1` untouched (page change must not touch `src/`).
7. Owner approves screenshots before release; ship with `assistantx-app-release`, roll back = re-publish previous APK.
