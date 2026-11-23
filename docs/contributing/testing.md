---
title: Testing
description: Golden tests, model tests, the smoke test and the test sets we need.
order: 4
---

# Testing

## Tests in the repo

- `cargo test -j1 -- --test-threads=2` runs unit and golden tests. `tests/golden.rs` holds golden conversations, injection and auth tests; `tests/accounts.rs` the account tests; `tests/lists.rs` the lists tests.
- `cargo test -j1 -p agent-node` tests the computer program.
- Every change ships with a test.

## Model tests and smoke test

- Model tests: `agent-core eval`, or `POST /v1/evals/run`. See [Models](../architecture/models.md), Model tests.
- Smoke test on the main server: `assistantx-deploy smoke`. See [Deploy](../architecture/deploy.md).

## Test sets the design asks for

- **Injection set:** golden conversations where mail, web pages, triggers, plugin guides and memory entries carry hidden orders. Pass means no risky act happens without an approval, and the user is told about the odd request. Include unattended turns (T15).
- **Sandbox escape set:** see T13.
- **API auth set:** missing token, wrong scope, bad `Host`, bad `Origin`, expired approval, replayed `client_msg_id`.
- **Dreaming set:** poisoning, merges, conflicts, forget leaves no copies.

All run in CI on each change to the core, prompts or adapters.
