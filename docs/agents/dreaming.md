---
title: Dreaming
description: The background job that sorts notes into memory.
order: 4
---

# Dreaming

A scheduled background job (default: nightly, or after enough new data). It runs in the `system:dreaming` thread using `prompts/dreaming.md` with no user waiting.

### What dreaming may use

Dreaming gets **memory tools only**: read the raw log and inbox, search, create, update, mark, and rebuild the index and `core.md`. It cannot load plugins, run plugin tools, send anything, or reach the network (except the model provider). This holds even if the content it reads asks for more.

### Concurrency

Dreaming works on a snapshot taken at start. When it writes an entry, it locks that entry and checks `updated`. If a live turn or the user changed the entry since the snapshot, the user's version wins and dreaming re-queues that fact for next time.

### Steps

1. Gather: new raw log since last run plus the inbox.
2. Extract: facts, events, preferences, people, goals, commitments. Each fact keeps its `source_trust`.
3. Match: for each fact, search for existing entries that cover it.
4. Merge or create: update the entry, or make a new one with title, summary, detail, labels, topic path. Merge only when people, time and topic agree; otherwise create a new entry.
5. Correct: if a new fact conflicts with an old one:
   - Higher trust wins (`user` > `inferred` > `outside`).
   - Same trust: the newer one wins only if it is a plain statement (not a joke, a "what if", or a quote of someone else). Mark the old entry `outdated` and link with `supersedes`.
   - If unclear, mark `disputed` and queue a question for the user.
6. Compress: older, low-use entries get shorter `summary` and `detail`; the raw log still holds the original.
7. Score: update importance from use, recency, and user emphasis. Importance slowly decays without use. Each run also reviews a small random sample of low-use entries, so things that were never loaded are not lost for good.
8. Rebalance core: promote the most important stable `user`-trust facts to `core.md`; demote the rest. Keep `core.md` under its cap. Skip `sensitive` unless the user agreed.
9. Rebuild index: keep ids stable, rewrite only lines whose entries changed, fold small groups into folders, keep the top level within 80 lines. Sensitive entries get vague lines.
10. Re-embed: entries whose `embedding_model` differs from the current one are re-embedded in batches.
11. Report: write a short change log (`dreaming.report` event). Any questions for the user go out as gentle prompts.

### Rules

- Never delete raw log items.
- Never invent facts. Each entry lists its sources.
- Mark uncertain things with low confidence instead of dropping them.
- Sensitive items (health, money, secrets, relationships, location history) get a `sensitive` tag and are not put in `core.md` unless the user agrees.
- Keep a version history of each entry so edits can be undone.
- A dry-run mode shows what would change.
- Budget: a dreaming run stops at its token budget and continues next time.

### Why the prompt stays small

```
millions of words (raw log)
   -> thousands of entries (memory DB, searchable)
   -> up to 80 top-level index lines, more inside folders
   -> tens of core facts (one index line, loaded on request)
```

Each level is a summary of the one below. The model walks down only when it needs to.

## When it runs

- Nightly at `AGENT_DREAMING_HOUR` (default 3).
- Dreaming by itself: when notes wait and the last run is older than `AGENT_DREAM_EVERY_MINS` (default 120, 0 = off), on top of the nightly job.
- When phone capture text waits (see [Phone capture](../architecture/phone-capture.md), Processing).
- By hand: `POST /v1/dreaming/run {dry_run}`.

The runtime prompt is [dreaming.md](../../server/prompts/dreaming.md).
