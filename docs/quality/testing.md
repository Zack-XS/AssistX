---
title: Testing
description: What to test, how, and which tests each kind of change needs.
order: 4
---

# Testing

Every change ships with a test. The commands and test sets are in [Testing](../contributing/testing.md). This page says what each kind of change needs.

## Which test

| You changed | Add |
|---|---|
| A route | A test in `tests/golden.rs`: the happy path, a missing token, a wrong scope |
| Something per account | A test in `tests/accounts.rs` that account B cannot see account A's data |
| A tool or plugin | A case in the plugin's `tests/*.json`, and a golden test if the gate is involved |
| A risky tag or the gate | A golden test that the act waits for approval |
| Outside content handling | An injection case: a hidden order in the content does nothing |
| A prompt or guide | A model test case (`agent-core eval`) on the smallest model |
| Scrubbing or logging | A unit test that the secret does not come out |
| A bug | A test that fails before the fix and passes after |
| The app page | Check it still calls `NativeApp.pageReady()` ([App structure](../android/app-structure.md)) |

## Good tests

- Test what the user sees, not inside parts.
- One test, one idea. Name it for the idea.
- No network, no real model, no real clock. Fake them.
- No real names, places or tokens in test data.
- Fast. The whole run must stay quick on a small machine (`-j1`).

## Model tests

Prompts change what the model does, so test them with the model. Run on the smallest model you support, with real tool calls. If the small model gets it wrong, fix the guide or the code, not the test.

## Gaps

- The app has no automated tests.
- There is no lint step yet.
- The test sets in [Testing](../contributing/testing.md) are not all written.
