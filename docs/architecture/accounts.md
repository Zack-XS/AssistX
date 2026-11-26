---
title: Accounts and encryption
description: Several people on one server, keys, sign-in and remembered phones.
order: 11
---

# Accounts and encryption

Status: built (`src/hub.rs`, `src/hub_http.rs`, `src/vault.rs`), key lifetime option A. The old one-owner mode remains with `AGENT_SINGLE_USER=1` (development and the golden tests).

## Goal

Several people can use one server. Each account has its own mind, memory, settings, jobs, places, health data, logs and models; nothing is shared and nobody can read another's data. Strong encryption, a recovery key shown once, and no loss of what the assistant can do.

## What E2EE can and cannot mean here

Real end-to-end encryption means the server never sees plaintext. But this assistant thinks on the server: it must read your memory, calendar, places and health to run a model, send tool calls and write notifications. So the server necessarily sees your data **while it works**. What can be done, and is worth doing:

- **Encrypted at rest with a key only you can unlock.** The disk, copies, backups and exports hold nothing readable without your key. A stolen disk or backup is useless.
- **Zero access when locked.** When your account is locked, even the server's operator cannot read it.
- **Real E2EE only with on-device models** (nothing to protect from the server then). Not possible for the 9B and 35B on the server.

## Design

1. **Server login.** The phone picks a server and enters the server token (as today). That proves the phone may talk to this server. Admin work (create accounts, lock, remove) needs the server's owner token and an admin page.
2. **Accounts.** The admin creates an account; the user then signs in on their phone with a username and passphrase. Each account is its own world: a separate encrypted database file (`data/accounts/<id>/core.db`), memory folder, plugin state, secrets, models settings, jobs, logs and audit trail. One running assistant per account, so memory, journal and mind never touch.
3. **Keys.** Creating an account makes a random 256-bit **data key**.

   It is stored only wrapped twice: by a key from the passphrase (Argon2id), and by a **recovery key** (shown once as 24 words, with a "copy this and keep it; if you lose both the passphrase and this key, your data is gone for good" confirmation the user must tick, and a retype check of 3 random words).

   The database is SQLCipher-encrypted with the data key. No wrapped copy is kept in plain.
4. **Unlock.** Signing in unwraps the data key into the server's memory. It is never written to disk unwrapped.
5. **Tokens per account.** Device, client and trigger tokens belong to one account and can only reach that account. The server token stays the outer gate.
6. **Isolation.** No route lets one account name another's thread, memory, place or job; cross-account access is tested with golden tests for every route.
7. **Changes to move.** `owner_id` columns already exist in the schema; the plan is separate files, not shared tables. Plugins on disk are shared and read-only; their state is per account.

## The decision: how long does an unlocked key stay in the server's memory?

This sets how much the assistant can do on its own, and how safe it is.

| Option | The key stays unlocked | Mind, reminders, reports, geofence while you are away | If someone takes over the running server |
|---|---|---|---|
| **A. Until the server restarts** (recommended) | Until a restart or you lock | Keep working | They can read unlocked accounts while it runs; after a restart nothing runs until someone signs in |
| **B. Only while you are signed in** | Until you sign out or after a time you choose (say 30 min idle) | Stop when locked; resume when you open the app | They can read only accounts that are signed in right now |

With B the assistant cannot do anything on its own while you are away, which defeats the Mind, daily reports and place reminders. A is what keeps its abilities. A mix is possible per account.

## Build order once decided

1. Per-account home: data folder, database, settings, models, jobs. Account store and admin routes. 2. Key wrapping, passphrase and recovery key flow, unlock and lock. 3. Per-account tokens and route isolation, with cross-account tests on every route. 4.

   Phone: server pick, server token, account sign-in, create-account screens with the recovery-key ceremony. 5. Admin page. 6. Migration of today's data into the first account.

## As built

- **Server token** (`data/server.token`, hash in `data/server.db`): the **Server Access Code (SAC)**, at least 25 characters when set with `agent-core server-password` (the one the server makes on first run is 52). It opens only `/v1/server/*` and the app update files. It started as the old owner token, so a phone that held it can still connect and update itself.
- **Routes** (server token): `GET /v1/server/info`, `GET /v1/server/overview`, `GET /v1/server/log?limit=50`, `GET|POST /v1/server/accounts` (list, invite), `DELETE /v1/server/accounts/{id}`, `POST /v1/server/accounts/{id}/lock|disable|enable|invite-code|revoke-devices`, `GET /v1/server/accounts/{id}/devices`, `DELETE /v1/server/accounts/{id}/devices/{device}`, `POST /v1/server/signup`, `/signup/confirm`, `/login`, `/recover`, `/passphrase`. Everything else goes to the account named in the token, `<id>.<secret>`; a locked account answers 423 `account_locked`.
- **Per account** (`data/accounts/<id>/`): `core.db` (SQLCipher, its own data key), `files/`, `work/` (the terminal's folder), `plugins/` (its own copy of the shipped plugins, refreshed on update; `state/` never copied). Memory, mind, journal, jobs, models, secrets, tokens, logs, places and health live in that database.
- **Keys** (`vault.rs`): data key = 32 random bytes, stored only wrapped: by Argon2id(passphrase) and by the recovery key (13 groups of 4, shown once; sign-up is not final until its last 8 characters are typed back). ChaCha20-Poly1305, the wrap bound to the account id.
- **Limits:** five wrong sign-in, sign-up or recovery tries lock a name for a minute, then longer. Names quiet for an hour are forgotten once more than 5000 are tracked.

  Five wrong tokens in a row from one address (a wrong server password, or a guessed account token) lock that address for 30 seconds, then 1, 2, 4 minutes, up to 15; the answer is 429 with `Retry-After`. A right token clears the count.

  No token at all does not count. A passphrase needs 10 characters. The server holds an unlocked key in memory until a restart or a lock; after a restart nothing of an account runs until its owner signs in once.
- **Known gaps:** the periodic health watch, the model keep-warm and the Nominatim rate limit are shared across accounts (by design for the last two). `assistantx-review` and `assistantx-smoke` used the old owner token and need an account token now. Sandbox, models and the plugin catalogue are shared; an admin with shell access to the server can still read an unlocked account's memory.

## Invite codes

Inviting a username returns a one-time **invite code** (`XXXX-XXXX-XXXX`, 12 characters without `0 O 1 I L`). Sign-up needs it (`invite_code`), so the server password plus a guessed name is not enough to take an invited place.

- The server keeps only the code's SHA-256. Case, spaces and dashes do not matter.
- A code works for 7 days. The admin can make a new one (`POST /v1/server/accounts/{id}/invite-code`); the old one stops working, and a half-done sign-up goes back to the start.
- Wrong codes count as wrong tries for that name (five lock it for a minute, then longer).
- The code is cleared once sign-up is confirmed.
- An account invited before codes existed needs a new code before it can sign up.

## Switched-off accounts

`POST /v1/server/accounts/{id}/disable` switches an active account off: its key is forgotten, sign-in and phone unlock are refused, and its token gets 403 `account_disabled`. Its data stays. `enable` switches it back on; the person then signs in again.

## Admin dashboard

**Two locks.** The dashboard needs the Server Access Code *and* the admin login (one username and password, at least 12 characters, kept only as an Argon2id hash in `server.db`). The code alone, which everyone who signs in has, no longer lists, invites, locks, switches off or removes accounts: those routes answer 401 `admin_login` without a live admin session (`X-Admin-Session`). Signing in gives a session held only in memory; it ends after 30 idle minutes, 12 hours at most, on sign-out, on a password change (all sessions) or on a restart. Wrong admin passwords count like wrong sign-ins (five lock the name for a minute, then longer).

- **First admin login:** while none exists, the server makes a one-time **setup code** at start, prints it to its log and writes it to `<data dir>/admin-setup.code` (0600). `/admin` asks for it once; the file is removed after use. A restart makes a new one until the login exists.
- **From the server itself:** `agent-core admin-password` reads the username and password from stdin and sets or resets the login.
- **Routes:** `GET /v1/server/admin/status`, `POST /v1/server/admin/setup|login` (code only), `POST /v1/server/admin/logout|password` (code + session).
- **Pages:** Overview (numbers, "Needs a look", recent activity), Accounts (search, filter, table; a row opens a side panel with details, actions, phones and a typed-name remove), Activity (by day), Settings (admin password). Works on a phone with a bottom tab bar.

The **admin dashboard** is a web page on the server: open `http(s)://<server>/admin` in any browser (`server/web/admin.html`, built into the server). It is not part of the app, and the app does not link to it. It asks for the Server Access Code and keeps it only in that page's memory, so a reload asks again. The page holds no data itself; it calls the routes below from the server's own origin, which is let through without being in `AGENT_ALLOWED_ORIGINS`. It is sent with `Cache-Control: no-store`, a strict Content-Security-Policy (`connect-src 'self'`) and may not be framed.

| Part | Shows | From |
|---|---|---|
| Numbers | accounts, unlocked now, remembered phones, disk used, version, uptime, slowed-down addresses | `GET /v1/server/overview` |
| Accounts | filter All / Active / Waiting / Off; each row: state, phones, disk. Tap for dates, phones and actions | `GET /v1/server/accounts`, `.../devices` |
| Actions | new invite code, lock, switch off/on, forget one phone, forget all phones, remove | the routes above |
| Invite | username in, one-time code out (shown once, with Copy) | `POST /v1/server/accounts` |
| Recent admin actions | the newest 20 lines | `GET /v1/server/log` |

The **admin log** is table `admin_log` in `server.db` (newest 1000 lines): time, action, account id, a short detail. Actions: `invite`, `invite_code`, `signup`, `recover`, `lock`, `disable`, `enable`, `delete`, `revoke_device`, `revoke_devices`. It never holds a passphrase, key or code. Each line is also written to stderr as `admin: <action> <id>`.

## Connecting in the app

1. **Connect to the server.** Type the server's domain or address and tap Done (or the keyboard's Done). Without a port the app tries `http://<name>:8790` first, then `https://<name>`. Below, the app lists AssistX servers it finds on the phone's Wi-Fi ("Want to connect to this server?").
2. **Server Access Code.** At least 25 characters; kept only in memory until sign-in.
3. **Sign in / Create account.** Bottom left: *How to set up an account*. Bottom right: *Switch home server* (back to step 1).

**Finding servers on the Wi-Fi** (`LanScan.kt`): only on Wi-Fi or Ethernet, the /24 around the phone's address, port 8790, `GET /v1/health`, 32 at a time, about 20 seconds at most. The handshake: the answer must say `"name": "AssistX"`. A server shows up only if it listens on that network and lists its address in `AGENT_ALLOWED_HOSTS`.

## Remembered phones (no sign-in after a restart)

A server restart forgets every account's key (it is only ever in memory), and every request then answers 423. So that nobody has to sign in again, a phone can be remembered:

- Signing in from the app asks to enroll (`enroll: true`). The server makes a random 256-bit **device key**, wraps the account's data key with it (a third wrapping, in table `devices`), and gives the device key to the phone once. The server never stores it; the phone keeps it sealed in the Android keystore, like the account token.
- After a restart the phone gets a 423, sends `POST /v1/server/unlock {device_id, key}` (no server password needed: the device key is the proof), the account unlocks, and the request is tried again. The app does this in the page (`api()`), in every native call (`Alarms.http`) and in the live connection, so the Mind, reminders and notices resume as soon as the phone next talks to the server.
- Sign out on the phone calls `POST /v1/server/unlock/forget`; that key stops working. A recovery with the recovery key (`/recover`) and the admin's `POST /v1/server/accounts/{id}/revoke-devices` forget every remembered phone of the account.
- Wrong device keys are slowed down like wrong tokens (429 after five).
- What this keeps: a copy of the server's disk is still useless, because the device key is not on it. What it costs: while the server runs, anyone holding a remembered phone's key can unlock that account, as with the account token itself.

  After a restart, an account stays locked until one of its remembered phones talks to the server, so a phone that is off or out of reach delays the Mind until it is back.
