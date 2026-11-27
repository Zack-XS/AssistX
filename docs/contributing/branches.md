---
title: Branches
description: One branch, production: what runs on it and how a change gets there.
order: 6
---

# Branches

There is one shared branch, `production`. Every change gets there through a pull request:

```
feature branch --PR--> production
```

| Branch | What it is | What runs | Who merges |
|---|---|---|---|
| `production` | What users run. | Checks, then the release | Owner only |

**Checks** (`.github/workflows/checks.yml`) run on every PR and every push to `production`:

- private data check ([Private data](../security/private-data.md))
- server: `cargo test -j1`
- docs site: `cargo test -j1` and `docs-site check`

**The release** (`.github/workflows/release.yml`) runs on each push to `production`, after the checks pass:

- runs the checks again
- makes the changelog: every commit message since the last release tag
- tags the release `vYYYY.MM.DD-N` and makes a GitHub release with that changelog

The Android build and server deploy still run from the Mac with `assistantx-app-release` and `assistantx-deploy`, from a `production` checkout. CI does not hold the signing key. Moving the Android build into CI is on [Next](../roadmap/next.md).

## Rules

- Never push straight to `production`. Open a PR.
- Never force-push `production`.
- Feature branch names: `feat/...`, `fix/...`, `docs/...`, `chore/...`. Branch from `production`.
- A red check blocks the merge. Fix it; never skip it.

## Ship a change

1. Branch from `production`, make the change, open a PR into `production`.
2. All checks green. The owner merges. The release workflow writes the changelog and tags it.
3. From the `production` checkout, run `assistantx-deploy` and `assistantx-app-release`.

Roll back: revert the merge on `production`, then deploy and re-publish the last good APK.

`main` and `beta` were removed on 2026-11-26; `production` is the only long-lived branch.
