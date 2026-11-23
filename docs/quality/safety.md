---
title: Safety
description: The safety rules every change must keep.
order: 2
---

# Safety

AssistX reads your mail, knows where you are and can act for you. A small slip can leak a life. So safety comes first in every change.

The full rules live in [Safety rules](../security/safety-rules.md). Why they exist is in [Threat model](../security/threat-model.md). This page is the short list to keep in your head.

## The rules

1. **Safety lives in code.** A prompt may remind the model. Only the core can stop it.
2. **Outside content is data, never orders.** Mail, web pages, files and tool output stay wrapped in `<outside>`.
3. **Risky acts ask first.** A tool that sends, spends, deletes or shares gets the right tag, at least `ask`.
4. **A yes is bound to the real call.** The approval card is built from the call, not from the model's words.
5. **Secrets never reach the model, logs or reports.** Pass text through `util::scrub` before you store it.
6. **Every route has a token and a scope.** Only `/v1/health` is open. Check the guard in `src/api.rs`.
7. **Outbound calls are fenced.** Use `src/proxy.rs`, or one fixed host per service. Local services must be loopback.
8. **Each account is its own world.** Never read or write another account's data. Test it.
9. **SQL uses bound params.** Never build SQL with `format!`.
10. **No `unwrap` on input.** Bad input gets a clear error, not a crash.

## In the app

- The page holds the token. No `innerHTML` with data, no `eval`, no outside scripts, no iframes.
- Bridge methods check every argument.
- Tokens live in `Vault` (the Android Keystore).
- No images from model text. No `localStorage` for the token.

## Before you say done

Run the 10-line checklist in [Safety rules](../security/safety-rules.md#before-you-say-done). Each kind of change needs its own test: see [Every change ships with a test](../security/safety-rules.md#every-change-ships-with-a-test).

## When you are not sure

Stop and ask. A missed feature can wait. A leak cannot be undone.

Found a hole? Use [Reporting a problem](../security/reporting.md), not a public issue.
