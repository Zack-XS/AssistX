---
title: Change flow
description: How a change goes in: docs first, a test, review, deploy.
order: 5
---

# Change flow

1. Read the page for the part you change. The design wins: if code and docs disagree, fix the code, or change the doc on purpose in the same change.
2. Make the smallest change that works. Keep the hard rules ([Code style](code-style.md)).
3. Add a test ([Testing](testing.md)).
4. Run `cargo test -j1` for the server. For the page, check it still calls `NativeApp.pageReady()` and keeps the page contract ([App structure](../android/app-structure.md)).
5. Check the safety rules ([Safety rules](../security/safety-rules.md)).
6. Update the docs pages you touched, and [Done](../roadmap/done.md) or [Next](../roadmap/next.md).
7. Commit in `server/` (a git repo). Never commit `data/`, `.token` or `target/`. `android/` is not in git yet.
8. Deploy with `assistantx-deploy`, and ship the app with `assistantx-app-release` ([Deploy](../architecture/deploy.md), [Updates and release](../android/updates-and-release.md)).

Security problems go to [Reporting](../security/reporting.md), not public issues.
