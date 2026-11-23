---
title: Code review
description: The checklist a reviewer runs on every change.
order: 7
---

# Code review

Every change gets a review, by a person or by a second AI agent that did not write it. The reviewer reads the code, not just the summary.

## Checklist

Go down the list. Write a note for each "no".

**Does it work?**

- [ ] It does what the task asked, and nothing else.
- [ ] Bad input gets a clear error, not a crash. No `unwrap` on input.
- [ ] Edge cases: empty, very long, missing, twice in a row.

**Is it safe?** ([Safety](safety.md))

- [ ] Outside content stays data.
- [ ] New tools have honest tags. Risky ones ask first.
- [ ] New routes check the token and scope.
- [ ] No data crosses accounts.
- [ ] SQL uses bound params.
- [ ] No secrets in code, tests, logs or docs.

**Is it logged?** ([Logging](logging.md))

- [ ] Actions, errors, skips and model calls write a record.
- [ ] Logged text is scrubbed and short.

**Is it tested?** ([Testing](testing.md))

- [ ] A new test covers the change. Old tests pass.

**Is it small and clear?**

- [ ] One reason for the change.
- [ ] No unused code, options or layers.
- [ ] Names say what things are. Matches nearby code.

**Does it cost too much?** ([Performance](performance.md), [Android battery](android-battery.md))

- [ ] No new query without an index. No unbounded list.
- [ ] No new service, wake lock without a timeout, or one-by-one upload in the app.

**Are the docs right?** ([Writing docs](writing-docs.md))

- [ ] Docs for the changed part match the code.

## How to give a review

- Say what is wrong and where: `file:line`.
- Say why it matters, in one line.
- Split "must fix" from "nice to have".
- Approve only when every must-fix is done.
