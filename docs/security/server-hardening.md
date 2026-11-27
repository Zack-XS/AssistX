---
title: Server hardening
description: The 2026-11-27 security review, item by item: what is fixed in code, what the server needs, and the plan for what is left.
order: 5
---

# Server hardening

The 2026-11-27 security review listed 30 problems against three goals: the server cannot read user data, everything that can be encrypted is, and each user's data is separate. This page tracks each one. Numbers match the review.

## Status

| # | Problem | State | Where |
| --- | --- | --- | --- |
| 1 | Data keys stay in RAM | Partly fixed: idle accounts lock after `AGENT_IDLE_LOCK_HOURS` (24); full fix needs a confidential VM (below) | `hub.rs` `lock_idle` |
| 2 | Passphrase reaches the server | Planned (below) | |
| 3 | Plain memory copy on disk | Fixed: only in the encrypted database; old folders deleted at start | `memory.rs`, `lib.rs` |
| 4 | Plain report and model-test files | Fixed: tables `reports`, `eval_runs`; old files moved in and deleted | `reports.rs`, `eval.rs` |
| 5 | Plain user files, work folder, plugin state | Server step (fscrypt, below) | |
| 6 | Plain HTTP | Fixed in the app: http only to private, Tailscale and `.local` addresses. Server: use Tailscale HTTPS (below) | page `privateHost` |
| 7 | Cloud models see prompts | Fixed: Keep everything on my server (`local_only`) or `AGENT_LOCAL_ONLY=1` | `models.rs` `allowed_profile` |
| 8 | One cloud key and Lumo login for all | Fixed: `AGENT_SHARE_CLOUD=all\|none\|<ids>` | `models.rs` `shares_server_cloud` |
| 9 | Keys in plain env files | Fixed: systemd credentials `model_api_key`, `lumo_token` | `config.rs` `credential` |
| 10 | `server.db` not encrypted | Fixed: SQLCipher, key `AGENT_SERVER_DB_KEY` or `server.key` (credentials folder first) | `db.rs` `open_server` |
| 11 | Old one-owner data on disk | Start-up warning; deleting it is a server step (below) | `main.rs` |
| 12 | Tokens never expire | Fixed: sign-in tokens end after 30 unused days; a new passphrase ends them all | `auth.rs` |
| 13 | One process for all accounts | Planned (below) | |
| 14 | Plugins may name any path | Fixed for accounts: only the account's own folders | `runner.rs` `rw_paths` |
| 15 | Phone unlock key has no user check | Fixed: Keystore key usable only within 15 minutes of unlocking the phone | `Vault.kt`, `Unlock.kt` |
| 16 | Server Access Code as fast SHA-256 | Fixed: Argon2id with salt; old hash upgraded on first use | `hub.rs` `sac_hash` |
| 17 | Sign-in codes in notifications | Fixed: never sent, even from watched apps | `NotifyListener.kt` |
| 18 | Limits by IP behind a proxy | Fixed: `AGENT_TRUST_PROXY=1` reads `X-Forwarded-For` from a local proxy | `hub_http.rs` `caller` |
| 19 | Anyone can lock a person out | Fixed: counted per name and address; name-wide only after 50 | `hub.rs` `throttle` |
| 20 | No second factor | Fixed for the admin: authenticator codes (TOTP) | `totp.rs`, `hub_admin.rs` |
| 21 | Recovery key made on the server | Planned with #2 | |
| 22 | Setup code file | No change: removed once used; also printed to the log | |
| 23 | Token visible to the page | Fixed: the page runs only its own script (CSP hash) | `MainActivity.kt` `pageCsp` |
| 24 | Exact points to OpenStreetMap and OSRM | Fixed: rounded to about 100 m (`AGENT_GEOCODE_DECIMALS`) | `location.rs` `rounded` |
| 25 | Health and location in cloud prompts | Fixed by #7's switch | |
| 26 | Prompt injection | No change: trust labels, `<outside>` wrapping and approvals already guard tools | `engine.rs`, `gate.rs` |
| 27 | Admin page allows inline scripts | Fixed: CSP by script hash | `hub_http.rs` `admin_csp` |
| 28 | Admin sees names and sizes | No change: needed to manage accounts; no content is shown | |
| 29 | Invite code letter bias | Fixed: rejection sampling | `hub.rs` `invite_code` |
| 30 | `server.log` beside data | Server step: log to journald only | |

## Settings

| Variable | Default | What it does |
| --- | --- | --- |
| `AGENT_IDLE_LOCK_HOURS` | `24` | Lock an account nothing reached for this long. `0` = never. |
| `AGENT_LOCAL_ONLY` | off | `1` = no account may use a cloud model. |
| `AGENT_SHARE_CLOUD` | `all` | Which accounts may use the server's cloud key and Lumo login: `all`, `none`, or account ids. |
| `AGENT_SERVER_DB_KEY` | none | Passphrase for `server.db`. Else `server.key`. |
| `AGENT_KEY_DIR` | data folder | Where `server.key` and `db.key` live when not in systemd credentials. |
| `AGENT_TRUST_PROXY` | off | `1` = trust `X-Forwarded-For` from a loopback proxy. |
| `AGENT_GEOCODE_DECIMALS` | `3` | Decimals sent to OpenStreetMap and OSRM (3 is about 100 m). |

## Server steps

These need root on the server. They are not done by the code.

1. **Keys out of `data/`.** In the systemd unit:

    ```ini
    LoadCredentialEncrypted=server.key:/etc/credstore.encrypted/server.key
    LoadCredentialEncrypted=model_api_key:/etc/credstore.encrypted/model_api_key
    LoadCredentialEncrypted=lumo_token:/etc/credstore.encrypted/lumo_token
    ```

    Make each with `systemd-creds encrypt --name=<name> <plain file> /etc/credstore.encrypted/<name>`, then delete the plain file and the matching lines in `model.env`. On first start after this, move the existing `data/server.key` into the store the same way.
2. **Encrypt the account folders (#5).** On ext4, turn on `encrypt` (`tune2fs -O encrypt`), then `fscrypt setup` and `fscrypt encrypt data/accounts --source=custom_passphrase` or a raw key from the credential store. The folders, file names included, are then unreadable on a copied disk.
3. **HTTPS (#6).** `tailscale cert` for the server's MagicDNS name, and `tailscale serve --https=443 http://127.0.0.1:8790`. Then set `AGENT_TRUST_PROXY=1` and use `https://<name>.ts.net` in the app.
4. **Old one-owner data (#11).** After checking an account holds everything, delete `data/core.db*`, `data/db.key`, `data/owner.token`, `data/memory/`, `data/reports/`. The start-up log lists what is left.
5. **Logs (#30).** Run under systemd with output to journald only; set `Storage=persistent` and `SystemMaxUse=` in `journald.conf`.
6. **Encrypted swap.** So keys and plain text never land on disk.

## Plan for what is left

**#2 and #21: the passphrase and recovery key never reach the server.** The phone runs Argon2id itself and sends only the wrapped data key; recovery keys are made on the phone. Sign-in uses OPAQUE (a PAKE), so the server can check a passphrase without ever seeing it. This only pays off together with #1's full fix: while the server holds the data key in RAM, a changed server can read data without the passphrase.

**#13: one process per account.** The hub starts one `agent-core account <id>` worker per unlocked account, under its own OS user, and passes it the data key over a pipe. The hub forwards requests over a Unix socket. A bug in one account then reaches only that account's memory.

**#1 in full: the operator cannot read memory.** Run the server in a confidential VM (AMD SEV-SNP or Intel TDX). The phone checks the VM's attestation report before sending its device key, so a changed server or a host with root gets nothing. Without that, the only full answer is a server on hardware you own.
