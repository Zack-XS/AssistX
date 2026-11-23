---
title: Agents checklist
description: Rules AI agents must follow when they change the code.
order: 9
---

# Agents checklist

AI agents write much of this code. These rules are for them. People follow them too.

`server/AGENTS.md` sends agents here. Its hard rules are the same as [Code style](../contributing/code-style.md); this page adds how to work.

## Top rule

**All backend code is Rust: server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS.** See [Code style](../contributing/code-style.md) rule 1.

## Before you start

1. Read the docs page for the part you change.
2. Read the code it touches, and every caller.
3. Check the work is not done already.
4. If the task rests on something false (a file or route that is not there), stop and say so.

## While you work

1. Make the smallest change that fully works.
2. Reuse what is in the codebase. Match nearby code.
3. No new dependency for a few lines.
4. Keep the [safety rules](safety.md). Never weaken a check to make a test pass.
5. Add logs for actions, errors, skips and model calls ([Logging](logging.md)).
6. Add a test ([Testing](testing.md)).
7. Update the docs in the same change ([Writing docs](writing-docs.md)).
8. Build and test with `-j1`. The machine is low on memory.

## Never

- Put secrets, tokens or personal data in code, tests, logs, docs or commits.
- Commit `data/`, `.token` or `target/`.
- Touch another account's data, or real user data, to test.
- Deploy, delete data or send messages without the owner's yes.
- Turn off a test, a check or the Safety Gate.
- Say "done" without running the tests.

## Before you say done

- [ ] Tests pass. You read the output.
- [ ] You ran the [review checklist](reviews.md) on your own diff.
- [ ] The [quality bar](quality-bar.md) is met.
- [ ] Logs added ([Logging](logging.md)). App changes keep the [battery rules](android-battery.md).
- [ ] Your last line says what you did not check, and any risk.
