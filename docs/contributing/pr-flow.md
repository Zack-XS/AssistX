---
title: Change flow
description: How a change goes in: docs first, a test, a pull request, review, deploy.
order: 5
---

# Change flow

1. Read the page for the part you change. The design wins: if code and docs disagree, fix the code, or change the doc on purpose in the same change.
2. Branch from `main`: `feat/...`, `fix/...`, `docs/...` or `chore/...` ([Branches](branches.md)).
3. Make the smallest change that works. Keep the hard rules ([Code style](code-style.md)).
4. Add a test ([Testing](testing.md)) and logs ([Logging](../quality/logging.md)).
5. Run `cargo test -j1` for the server. For the page, check it still calls `NativeApp.pageReady()` and keeps the page contract ([App structure](../android/app-structure.md)).
6. Run the private data check ([Private data](../security/private-data.md)).
7. Check the safety rules ([Safety rules](../security/safety-rules.md)).
8. Update the docs pages you touched, and [Done](../roadmap/done.md) or [Next](../roadmap/next.md).
9. Open a pull request into `main`. Follow the etiquette below.
10. Once it reaches `production`, deploy with `assistantx-deploy` and ship the app with `assistantx-app-release` ([Deploy](../architecture/deploy.md), [Updates and release](../android/updates-and-release.md)).

Security problems go to [Reporting](../security/reporting.md), not public issues or PRs.

## Pull request etiquette

### Size

- One change per PR. A bug fix and a refactor are two PRs.
- Aim for under 400 changed lines. Big work goes in steps that each work.
- No drive-by changes: no reformatting files you did not need to touch.

### Title

Short, plain, in the present tense, with a type:

- `feat: add place reminders to the scheduler`
- `fix: stop alarms ringing twice after restart`
- `docs: add branch guide`
- `chore: bump sqlx`

### Body

```
## What
One or two lines: what changes for the user or the code.

## Why
The problem, or a link to the issue or doc.

## How to test
Commands you ran and what you saw. Screenshots for any screen change (light and dark).

## Checklist
- [ ] Tests added or changed
- [ ] Logs added (docs/quality/logging.md)
- [ ] Private data check passes
- [ ] Docs updated
- [ ] No new permission, network call or battery cost, or it is explained above
```

### Before you ask for review

- All checks green. Don't ask people to review a red PR.
- Read your own diff top to bottom first.
- Rebase on `main` if it is behind. Keep history clean: squash "fix typo" commits.
- Mark it Draft until it is ready.

### Reviewing

- Review within one working day, or say when you will.
- Be kind and specific. Comment on the code, never the person.
- Say how strong a comment is: `blocking:`, `suggestion:`, `nit:`, `question:`.
- Explain why, and offer a fix when you can.
- Run the [review checklist](../quality/reviews.md).
- Approve when it is good enough and safe. Perfect can come later.

### Answering review

- Reply to every comment: fix it, or say why not.
- Push fixes as new commits so the reviewer sees what changed. Squash on merge.
- Don't resolve someone else's thread; let them.
- Disagree once, with reasons. If still stuck, the owner decides.

### Merging

- Needs one approval and green checks. Changes to the Safety Gate, auth, sandbox, proxy or crypto (`server/src/gate.rs`, `auth.rs`, `sandbox.rs`, `proxy.rs`, `vault.rs`, `db.rs`) need the owner's approval.
- Squash merge into `main`. The PR title becomes the commit message, so keep it clean.
- Delete the branch after merge.

### Never

- Never commit private data, tests included ([Private data](../security/private-data.md)).
- Never skip or turn off a check to get a merge through.
- Never merge your own PR into `beta` or `production` without the owner.
- Never push secrets "for a minute". Pushed is leaked.

### For AI agents

Agents follow the same rules, plus [Agents checklist](../quality/agents-checklist.md). An agent says in the PR body that an agent wrote it. An agent never approves its own PR.
