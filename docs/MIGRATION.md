---
title: Migration
description: Where each old doc went in the new docs tree.
order: 99
---

# Migration

The old docs stay in place until a later task retires them. This table says where each part went. Facts that did not fit a page, or where old docs and code disagree, are in [Open questions](roadmap/open-questions.md).

| Old file | Part | New file(s) |
|---|---|---|
| `server/docs/00-overview.md` | What this is, Parts, Out of scope | [What AssistX is](overview/what-is-assistx.md) |
| | Principles | [Principles](overview/principles.md) |
| | Scheduler: core vs plugin | [Scheduler and notices](architecture/scheduler-and-notices.md) |
| | Safety Gate, Turns and threads, Cost limits | [Turn engine](architecture/turn-engine.md) |
| | Tech stack | [Architecture](architecture/README.md) (with a code check) |
| | Documents list | Replaced by [docs index](README.md) |
| `server/docs/01-api.md` | Transport, Auth, Scopes, Core concepts, Endpoints, Sending a message, Errors, Turn sources, Rate limits, Stability, Phone capture | [API](architecture/api.md) |
| | Event format, Event types | [Events](architecture/events.md) |
| | Approvals (cards, waiting, batches), Autonomy levels | [Approvals](agents/approvals.md) |
| | Notices and tasks, Jobs, ringing, sounds | [Scheduler and notices](architecture/scheduler-and-notices.md) |
| | Calendar routes | [calendar](plugins/calendar.md) (also in [API](architecture/api.md)) |
| | Lists routes | [lists](plugins/lists.md) (also in [API](architecture/api.md)) |
| | Reports | [Audit](architecture/audit.md) |
| `server/docs/02-prompt-system.md` | Layers, personality, turn rules, basic data, index, outside content, budget, prompt files | [Prompt system](architecture/prompt-system.md) |
| | Built-in tools, Decision flow | [Turn loop](agents/turn-loop.md) |
| | Model requirements | [Models](architecture/models.md) |
| `server/docs/03-plugins.md` | Folder layout, plugin.yaml, tags, guide.md, tools, sandbox, tests, sharing, MCP, real programs | [Plugin format](architecture/plugins.md) |
| | Lifecycle, Approval card, plugin-maker outline, Build mode | [Plugin maker](agents/plugin-maker.md) |
| | `lists`, `phone`, Email (Gmail) | [lists](plugins/lists.md), [phone](plugins/phone.md), [email](plugins/email.md) |
| `server/docs/04-memory-and-dreaming.md` | Stores, entry, trust, reading, profiles, links, search, user control, forget, risks | [Memory](architecture/memory.md) |
| | Dreaming | [Dreaming](agents/dreaming.md) |
| | Recipes | [Recipes](architecture/recipes.md) |
| `server/docs/05-open-questions.md` | Decisions table, Still open | [Open questions](roadmap/open-questions.md) |
| | Suggested build order | [Done](roadmap/done.md) |
| `server/docs/06-threat-model.md` | All | [Threat model](security/threat-model.md) |
| | Front-end rules | also [Safety rules](security/safety-rules.md) |
| | Testing | also [Testing](contributing/testing.md) |
| `server/docs/07-multi-device.md` | All | [Multi-device](architecture/multi-device.md) |
| | Threats to cover | also [Threat model](security/threat-model.md) |
| | Build order | also [Next](roadmap/next.md) |
| | Open questions | also [Open questions](roadmap/open-questions.md) |
| `server/docs/08-phone-capture.md` | Rules to Review on the phone | [Phone capture](architecture/phone-capture.md) |
| | Not built yet | [Next](roadmap/next.md) (also on the page) |
| | Reference design | [Talking to the server](android/talking-to-server.md) |
| `server/docs/09-deploy.md` | Layout, Deploy, Look at it, Settings | [Deploy](architecture/deploy.md) |
| | Release and update, Install, Update banner | [Updates and release](android/updates-and-release.md) |
| | What the app does on the phone, Sound effects | [App structure](android/app-structure.md) |
| | Signing in to Spotify, How playback stays reliable | [spotify](plugins/spotify.md) |
| `server/docs/10-location.md` | All | [Location](architecture/location.md) |
| `server/docs/11-computers.md` | All | [Computers](architecture/computers.md); Not yet also in [Next](roadmap/next.md) |
| `server/docs/12-models.md` | All | [Models](architecture/models.md) |
| `server/docs/13-audit.md` | All | [Audit](architecture/audit.md) |
| `server/docs/14-mind.md` | All | [Mind](architecture/mind.md) |
| `server/docs/15-accounts.md` | All | [Accounts](architecture/accounts.md); build order also in [Next](roadmap/next.md) |
| `server/docs/16-app-design.md` | Scope, Principles, Top places, Information map, You groups, States, Voice and notices, Layout, Add a screen, Test a page change, Words, Not decided | [Screens](design/screens.md) |
| | Tokens, Scales, contrast | [Tokens](design/tokens.md) (one table; settled values listed there), [Color](design/color.md#contrast) |
| | Building parts | [Components](design/components.md#parts-and-class-names) |
| | Icons | [Icons](design/icons.md#meanings) |
| | Motion | [Motion](design/motion.md) |
| | Bridge contract, Stored keys, Open names, One file, Android mirrors | [App structure](android/app-structure.md#bridge-contract-frozen) |
| | No product name | [Principles](design/principles.md) rule 11, [Code style](contributing/code-style.md) rule 2 |
| `server/docs/17-safe-code.md` | Rules table, Rust core, Plugins, Page, Android, tests by kind, Before you say done, Known gaps | [Safety rules](security/safety-rules.md) |
| | Short list | [Safety](quality/safety.md) |
| `server/docs/reviews/r1-plan-review.md` | Wrong premises | [Open questions](roadmap/open-questions.md), [App structure](android/app-structure.md) |
| | Tabs and areas, page contract | [App structure](android/app-structure.md) |
| | Gaps for doc 16 (app design) | [Screens](design/screens.md), [Tokens](design/tokens.md) |
| | Gaps for doc 17 (safe code) | [Safety rules](security/safety-rules.md) |
| | Round 2 risks and acceptance | [Next](roadmap/next.md) |
| `server/AGENTS.md` | Goal | [Principles](overview/principles.md) |
| | Hard rules | [Code style](contributing/code-style.md), [Safety rules](security/safety-rules.md) |
| | Run it, Working here | [Run the server](getting-started/run-server.md), [Dev setup](contributing/dev-setup.md) |
| | Sandbox | [Plugin format](architecture/plugins.md), [Run the server](getting-started/run-server.md) |
| | Layout | [Architecture](architecture/README.md), [Code layout](contributing/code-layout.md) |
| | Build order status | [Done](roadmap/done.md) |
| | Routes | [API](architecture/api.md), Route summary |
| | Plugins we ship | [Plugins](plugins/README.md) and each plugin page |
| | Gaps against the docs | [Next](roadmap/next.md) |
| | Additions to the docs | Spread by topic: [Memory](architecture/memory.md), [Prompt system](architecture/prompt-system.md), [Turn engine](architecture/turn-engine.md), [Events](architecture/events.md), [Plugin format](architecture/plugins.md) |
| | (more) | [Plugin loading](agents/plugin-loading.md), [Approvals](agents/approvals.md), [Dreaming](agents/dreaming.md), [Scheduled checks](agents/scheduled-checks.md), [Voice](architecture/voice.md) |
| | (more) | [Phone capture](architecture/phone-capture.md), [Location](architecture/location.md), [phone](plugins/phone.md), [Talking to the server](android/talking-to-server.md), [Updates and release](android/updates-and-release.md) |
| | Main server | [Deploy](architecture/deploy.md) |
| | Memory people, dates, alarms | [Memory](architecture/memory.md), [Scheduler and notices](architecture/scheduler-and-notices.md), [Turn engine](architecture/turn-engine.md), [Talking to the server](android/talking-to-server.md) |
| | Search by meaning and recipes | [Memory](architecture/memory.md), [Recipes](architecture/recipes.md) |
| `server/SECURITY.md` | All | [Reporting a problem](security/reporting.md) |
| `reviews/latest.md`, `reviews/review-2026-11-15-1331.md` | A review pack (the two files are the same) | Not copied: it holds real chats and notices. Its format is in [Audit](architecture/audit.md). |
| `reviews/memory-bank-*.md` | Personal memory notes | Not copied: personal data. |
| `android/` | No Markdown files | App facts read from code: [Android app](android/README.md) |
| `server/prompts/*.md`, `server/plugins/*/guide.md`, `server/data/**` | Runtime files | Not moved. Linked from [Prompt system](architecture/prompt-system.md) and each [plugin page](plugins/README.md). |

## Old doc numbers in code

Code comments, prompts and plugins still name old docs by number (`docs/12`, `03`). Use this map until those comments are rewritten, when the old tree is retired.

| Old | New page |
|---|---|
| `docs/00` | [What AssistX is](overview/what-is-assistx.md), [Principles](overview/principles.md) |
| `docs/01` | [API](architecture/api.md), [Events](architecture/events.md) |
| `docs/02` | [Prompt system](architecture/prompt-system.md) |
| `docs/03` | [Plugin format](architecture/plugins.md) |
| `docs/04` | [Memory](architecture/memory.md), [Dreaming](agents/dreaming.md) |
| `docs/05` | [Open questions](roadmap/open-questions.md) |
| `docs/06` | [Threat model](security/threat-model.md) |
| `docs/07` | [Multi-device](architecture/multi-device.md) |
| `docs/08` | [Phone capture](architecture/phone-capture.md) |
| `docs/09` | [Deploy](architecture/deploy.md) |
| `docs/10` | [Location](architecture/location.md) |
| `docs/11` | [Computers](architecture/computers.md) |
| `docs/12` | [Models](architecture/models.md) |
| `docs/13` | [Audit](architecture/audit.md) |
| `docs/14` | [Mind](architecture/mind.md) |
| `docs/15` | [Accounts](architecture/accounts.md) |
| `docs/16` | [Screens](design/screens.md), [Tokens](design/tokens.md) |
| `docs/17` | [Safety rules](security/safety-rules.md) |

## Changes made while moving

- Private network addresses were replaced with `<server-ip>` and `<tailscale-ip>`.
- Example names that matched real people were swapped for other names.
- Old cross-links such as `docs/04` now point at the new pages.
- Short "Code check" notes mark places where the old text and today's code differ.
