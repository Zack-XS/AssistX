---
title: Done
description: What is built today.
order: 1
---

# Done

## Build order status

| Step | What | State |
|---|---|---|
| 1 | Turn engine, model adapter, prompt, threads/messages/events, auth, tokens, tests | Done |
| 2 | Memory DB, `search_memory`, `save_note`, raw-log scrubbing, core-set trust | Done |
| 3 | Index + `load` / `unload` (plugins, `mem:`, `dir:` folders, 80-line cap, batched rebuild) | Done |
| 4 | Safety Gate, approvals, action log, loader, sandbox, proxy, hash check, user autonomy | Done (macOS sandbox only) |
| 5 | plugin-maker, build mode, drafts, approval cards, install, versions, rollback | Done |
| 6 | Dreaming (dry run, snapshot locks, trust rules), forget | Done |
| 7 | Scheduler, jobs, triggers, notices | Done |
| 8 | Undo, export | Done |

Also built since: phone capture, location, computers, models, audit, the mind, accounts. See the [Architecture](../architecture/README.md) pages; each opens with its status.

## App page (Nov 2026)

What changed in the page (`server/examples/dummy-client/index.html`, copied into the app). Details: [Screens](../design/screens.md), [Components](../design/components.md), [Motion](../design/motion.md).

| What | Done |
|---|---|
| Theme switch | No edges of the other colour |
| Flicker | Home, Plan and closing pages no longer flicker (`settle()`) |
| Settings search | Finds pages by what is on them, plugins and small typos |
| Lists | One grouped card, one add box, instant ticks, foldable Done |
| Alarms | Next-alarm card, one list, instant switches, round day buttons, toasts |
| Mind map | A real mind map in the monochrome theme |
| Simpler settings | Intelligence, Privacy, Health, Memory; Morning page removed; Noticing only in developer mode |
| Spacing | More room on every screen ([Spacing](../design/spacing-and-layout.md)) |
| Branches | `production` is the only branch ([Branches](../contributing/branches.md)) |

## The planned build order

Safety comes in with the first risky tool, not at the end.

1. Turn engine + model adapter + base prompt + API (threads, messages, events, auth, tokens) + the golden-conversation test harness, so every later step is tested.
2. Memory DB + `search_memory` + `save_note` + raw log with secret scrubbing and core-set trust labels (04), so poisoning defenses exist before dreaming does.
3. Index generation + `load` / `unload`.
4. **Safety Gate + approvals + action log**, then plugin loader + sandbox + proxy + one real plugin (calendar). The sandbox escape and API auth tests (see [Threat model](../security/threat-model.md)) land in this step.
5. plugin-maker + build mode + plugin approval flow.
6. Dreaming job (memory tools only) + forget.
7. Scheduler + triggers.
8. Undo for each plugin tool that supports it + export.
