---
title: Private data
description: What must never be committed, tests included, and the check that stops it.
order: 4
---

# Private data

The repo is public: [github.com/Zack-XS/AssistX](https://github.com/Zack-XS/AssistX). Anything committed can be read by anyone, forever. Deleting it in a later commit does not remove it from history.

## Never commit

| Kind | Examples | Use instead |
|---|---|---|
| Real addresses | server IP, Tailscale IP (any `100.64` to `100.127` address), home IP | `127.0.0.1`, `192.0.2.10` (docs range), `my-server.example` |
| Host names and SSH aliases | real server name, `ssh` alias, Tailscale name | `server.example`, `phone-mac.tail1234.ts.net` |
| People | real names, emails, phone numbers | `sam@example.com`, `+1 555 0100` |
| Keys and tokens | API keys, OAuth secrets, bearer tokens, `owner.token`, `db.key` | env vars or files in `data/` (ignored) |
| Signing keys | `~/.assistantx.keystore`, any `.jks`, `.pem`, `.p12` | keep outside the repo |
| Sign-in state | `auth.json`, cookies, browser profiles | keep outside the repo |
| User data | `server/data/`, databases, chats, memory, location, health, voice | never leaves the server |
| Machine files | `local.properties`, `.env`, `.DS_Store` | ignored by `.gitignore` |

**Tests and fixtures count too.** A test must use made-up data:

- Emails on `example.com` (`sam@example.com`), never a real domain. `x.com` is a real site.
- IPs from the docs ranges: `192.0.2.x`, `198.51.100.x`, `203.0.113.x`.
- Fake keys that look fake: `sk-test-xxxxxxxx`.
- No real chats, real places, real health numbers, or real recordings, even "just for a test".
- Golden test scripts and screenshots: check them for names, places and notices before you add them.

Logs follow the same rule ([Logging](../quality/logging.md)): never log tokens, keys or message text.

## The check

`tools/private-data-check` is a small Rust tool. It scans every file git would commit and fails on:

- public and Tailscale IP addresses
- email addresses (except `example.com` and other safe domains)
- token shapes (`sk-`, `ghp_`, `AKIA`, `AIza`, `xoxb-`, `nvapi-` and more)
- private key blocks (PEM "BEGIN PRIVATE KEY" headers)
- private files: `server/data/`, `.token`, `.env`, `auth.json`, `.keystore`, `.jks`, `.pem`, `.key`, `.db`

Run it before every commit:

```sh
cargo run -j1 --manifest-path tools/private-data-check/Cargo.toml
```

Exit 0 means clean. Exit 1 lists each finding as `path:line: kind: value`.

It also runs:

- **Before each commit**, once you turn on the repo hooks: `git config core.hooksPath .githooks`
- **In CI** on every pull request and every push to `production` ([Branches](../contributing/branches.md)).

The tool's own tests: `cargo test -j1 --manifest-path tools/private-data-check/Cargo.toml`.

## When the check is wrong

If a hit is truly fake data, add it to `.private-data-allow` at the repo root:

```
# Why this is safe (one line).
server/src/config.rs: 192.0.2.10
```

- `path` allows a whole file, `dir/` a whole folder, `path: text` one value.
- Every entry needs a reason line above it.
- A reviewer must agree with each new entry.
- Better: change the test to use safe data, and add nothing.

## If private data got pushed

1. Treat it as leaked. Rotate the key or token now. Change the password.
2. Remove it from the files and push the fix.
3. For a real secret, also rewrite history (`git filter-repo`) and force-push. Ask the owner first: this rewrites everyone's history.
4. Write what happened in [Reporting](reporting.md) terms: what leaked, when, what was rotated.
