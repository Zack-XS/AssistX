---
title: Branches
description: main, beta and production: what each is for, what runs, and how a change moves up.
order: 6
---

# Branches

A change moves up through three branches:

```
feature branch --PR--> main --PR--> beta --PR--> production
```

| Branch | What it is | What runs | Who merges |
|---|---|---|---|
| `main` | Daily work. Every PR goes here first. | Checks | Any reviewer |
| `beta` | Next release, for testers. | Checks | Owner |
| `production` | What users run. | Checks, then the release | Owner only |

**Checks** (`.github/workflows/checks.yml`):

- private data check ([Private data](../security/private-data.md))
- server: `cargo test -j1`
- docs site: `cargo test -j1` and `docs-site check`

**The release** (`.github/workflows/release.yml`) runs on each push to `production`, after the checks pass:

- runs the checks again
- makes the changelog: every commit message since the last release tag
- tags the release `vYYYY.MM.DD-N` and makes a GitHub release with that changelog

The Android build and server deploy still run from the Mac with `assistantx-app-release` and `assistantx-deploy`, from a `production` checkout. CI does not hold the signing key. Moving the Android build into CI is on [Next](../roadmap/next.md).

## Rules

- Never push straight to `main`, `beta` or `production`. Open a PR.
- Never force-push a shared branch.
- Feature branch names: `feat/...`, `fix/...`, `docs/...`, `chore/...`.
- `beta` only takes PRs from `main`. `production` only takes PRs from `beta`.
- A hot fix goes to `main` first, then moves up like any change. If it can't wait, branch `fix/...` from `production`, merge it there, then merge it back to `beta` and `main` the same day.
- A red check blocks the merge. Fix it; never skip it.

## Promote a release

1. On `main`, all checks green. Open a PR `main` -> `beta`. Title: `Beta: <date>`.
2. Testers use the beta build. Fix bugs on `main` and promote again.
3. When beta is good, open a PR `beta` -> `production`. Title: `Release: <date>`. The body lists what users will see.
4. The owner merges. The release workflow writes the changelog and tags it.
5. From the `production` checkout, run `assistantx-deploy` and `assistantx-app-release`.

Roll back: revert the merge on `production`, then deploy and re-publish the last good APK.
