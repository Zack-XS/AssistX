---
title: Quality bar
description: What "done" means: tests, review, lint, no secrets, small changes.
order: 1
---

# Quality bar

A change is done only when every line below is true. If one is not, it is not done yet.

## Top rule

**All backend code is Rust: server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS.** See [Code style](../contributing/code-style.md) rule 1.

| Check | What it means | How to check |
|---|---|---|
| Works | It does what the task asked, end to end | Run it: `curl` the route, or use the app |
| Tested | A new test covers it, and old tests pass | `cargo test -j1 -- --test-threads=2` ([Testing](testing.md)) |
| Logged | New actions, errors and choices write a log line or audit event | [Logging](logging.md) |
| Safe | The safety rules still hold | [Safety](safety.md) |
| Clean | No new build warnings; match nearby style | `cargo build -j1` shows no new warnings. Lint (`cargo clippy`) is not set up yet: a gap |
| No secrets | No token, key, passphrase or personal data in code, tests, logs or docs | Read your diff; search it for `sk-`, `token`, `Bearer` |
| Small | One change does one thing | The diff fits in one review |
| Reviewed | Someone (a person or a second agent) ran the [review checklist](reviews.md) | The review notes |
| Docs | Pages for the part you changed match the code | [Writing docs](writing-docs.md) |
| Battery | App changes do not wake the phone more | [Android battery](android-battery.md) |

## Small changes

- One change, one reason. A fix and a refactor are two changes.
- Prefer deleting code to adding it.
- Do not add options, layers or "for later" code nobody asked for.
- Keep the shape the code already has. Match nearby code.

## Before you say "done"

1. Run the tests and read the result.
2. Read your whole diff once, top to bottom.
3. Say what you did not check, and any risk you know of.

The full change flow is in [Change flow](../contributing/pr-flow.md). The hard code rules are in [Code style](../contributing/code-style.md).
