---
title: Performance
description: Keep the server quick and small, and the model calls cheap.
order: 5
---

# Performance

The server runs on a small box, often with little memory. The model is the slow and costly part. Keep both in mind.

## Model calls

- Load only what the turn needs. Plugins load when used ([Plugin loading](../agents/plugin-loading.md)).
- Keep the prompt inside its budget ([Prompt system](../architecture/prompt-system.md)).
- Cut tool results before they go to the model. Return counts and short lines, not whole files.
- Do not call a model when plain code can decide.
- Every model call is timed (`ms` in `model_calls`). Look there first when a turn feels slow.

## Server

- One SQLite database per account. Keep each query on an index. Add the index in the same change as the query.
- Do not hold the database lock while you wait on the network or a model.
- Background loops sleep long, wake rarely, and do little each time.
- Cap every list: `LIMIT` in SQL, a max on every route.
- Prune old data on a timer, as `audit::maybe_prune` does.

## Build

- The build machine is low on memory. Build and test with `-j1`.
- Few dependencies, exact pins ([Code style](../contributing/code-style.md)). Each one costs build time and memory.

## The phone

The app's cost is battery. See [Android battery](android-battery.md).

## Measuring

- Turn time and tokens: `GET /v1/audit/turns/{id}`.
- Slow spots: add an `ms` field to the audit event for the part you changed ([Logging](logging.md)).
- Do not guess. Measure before and after a speed fix, and write both numbers in the change notes.
