---
title: Memory
description: Memory stores, entries, trust, folders, profiles, links, search and forget.
order: 5
---

# Memory

> Code check: the old doc said the DB key sits in the OS keychain. The code keeps it in `AGENT_DB_KEY` or `data/db.key` (see [Next](../roadmap/next.md)). Vectors are plain BLOBs, not `sqlite-vec`.

Goal: the assistant knows the person deeply, but the prompt never grows without limit.


## Three stores

| Store | Holds | Edited by |
|---|---|---|
| Raw log | Every message, tool result, event. Append only. | Nobody. Never deleted by jobs. Only a user "forget" removes items. |
| Inbox | Notes the model saved during turns. | Model writes, dreaming empties. |
| Memory DB | Clean, labeled, sorted entries. | Dreaming, and the user. |

A fourth kind of data, rolling conversation summaries, is kept per thread. The model writes it and it is rebuilt as needed. "Forget" cleans it too.

All stores live in one SQLite database, encrypted at rest (SQLCipher). The key is kept in the OS keychain.

Every raw log item carries a trust label set by the core: `user` (typed by the user), `outside` (tool results, mail, web, triggers), or `assistant` (the model's own words). The model cannot set or change it.

Before anything goes into the raw log, the core removes known secret values and common secret patterns (API keys, tokens, passwords in URLs).

## Memory entry

```
id                stable, never reused
owner_id          for future multi-user
title             short
summary           1 to 3 sentences, the "index line" source
detail            full text, loaded on demand
topic_path        e.g. travel/2026-11, people/sam, private/health
tags              list; includes `sensitive` when needed
people, places    linked names
time_range        when it happened or applies (stored in UTC, with the user's timezone)
importance        0 to 1
confidence        0 to 1
source_trust      user | inferred | outside
status            current | outdated | disputed
sources           ids of raw log items
supersedes        ids of entries it replaces
created, updated, last_used
embedding         vector for search (optional)
embedding_model   name and version of the model that made it
```

Search combines: topic path, tags, time range, keywords (FTS5), and vectors when an embedding model is set (see Search by meaning).

## Source trust

Where a fact came from matters more than when.

| Level | Example | Starting confidence |
|---|---|---|
| `user` | The user said "my birthday is May 2" | high |
| `inferred` | The model guessed a habit from calendar events | medium |
| `outside` | An email says "John's new number is ..." | low |

- Facts from `outside` content are stored as facts **about** the source ("an email from X said ..."), not as plain truths about the user.
- A derived fact never has more trust than its sources. The core computes `source_trust` from the trust labels of the listed `sources` (the lowest wins). The model, including dreaming, can only list sources; it cannot pick the trust. A fact with no sources can only come from a user edit through the API.
- `save_note` records which outside sources were in the turn. A note made in such a turn is `outside` unless the user said the thing directly. Dreaming checks the note against the user's own words in the raw log.
- An `outside` fact never goes into `core.md` and never overrides a `user` fact.
- Instructions found in content ("remember that the user wants ...") are never stored as preferences.

## Reading memory in a turn

1. Model sees basic data + index.
2. It loads an entry by id, or calls `search_memory`.
3. Loaded entries use `detail`. Lists of results show `summary` only.
4. Entries with `source_trust: outside` are wrapped as outside content.
5. `last_used` is updated. This feeds dreaming's importance score.

## Folders

- Memory is five folders: People, Places, Questions, Things, Other. `topic_path` is forced to start with one (anything else goes under `Other/`); old entries are moved at start.

  The index shows each folder with a count and a few names; `load dir:People` lists its files with descriptions; `load mem:<id>` reads the profile.

  A readable copy is kept at `data/memory/<Folder>/<name>-<id>.md` (description on top; the database stays the source of truth, edits to the files are not read back).
- Folders can nest and the model can make its own: skill plugin `memory-tools` (tools `list_folders`, `make_folder`, `write_entry`, `move_entry`, `move_folder`, `list_confidence`, `set_confidence`).

  `topic_path` is the folder an entry lives in (its own name is the title). Folders made on purpose live in table `memory_folders`; an unknown top-level name goes to `Other/`.

  `write_entry` takes its sources from the turn's own user/outside messages, so the core sets trust; a repeat title in the same folder updates the file. New code for the index and the disk copy is in `src/folders.rs`.
- `GET /v1/memory/folders` (tree with counts, descriptions, average confidence) and `POST /v1/memory/folders {path, description}`.
- `core.md` is a short profile: at most 12 lines, `user` trust, confidence 0.5 or more, no open questions, ranked by importance and confidence. The rest stays in the folders.
- Core tools `update_note` and `forget_note`: only the model's own unsorted notes, refused in a turn that has outside content.
- Dates: index lines, search results and loaded entries show the last-updated date; newer facts win.

## Profiles keep everything

A person, place or thing has its own folder (`People/lee`) with a profile: header lines `Relation`, `Owner`, `Birthday`, `Likes`, `Dislikes`, `Groups`, then `Facts:`, then `Before:`. The core merges every write (`remember`, `src/remember.rs`):

- A repeat is skipped. A fuller version of a fact takes the short one's place.
- Nothing is deleted. A changed `Relation`, `Owner` or `Birthday`, a newer age or grade, and any fact listed in `replaces` move to `Before:` with `(until <date>)`.
- Facts that go stale ("now", "currently", ages) get `(said <date>)`.

People, dates and groups (added Nov 2026):

- People, places and things live in their own folder: `People/kim/` holds `Kim profile` (Relation, Birthday, Likes, Dislikes, Groups, Facts) and `Kim timeline` (dated lines).

  The core writes both from one fixed call (`remember` in chat, `dream_remember` in dreaming; one name per call, two names are refused) and merges into what exists. `Groups` also write `People/groups/<Group> group` with a member list.

  Dreaming's kickoff lists flat `People` entries so the model converts them.
- Dates in memory look like `(Fri 2026-11-20, upcoming)`. The core computes the weekday and flips `upcoming` -> `today` -> `done` (`remember::refresh_dates`, at each real dreaming run and every 3 hours from `maybe_auto`). Words like "Friday" or "tomorrow" in text written by dreaming or `write_entry` get the exact date added after them (`remember::annotate`).

## Links

`src/links.rs` works links out from what is saved; there is no second copy to drift.

| Link | From |
|---|---|
| relation | `Relation:` is who they are to the user, so it links to the user's own folder (the profile whose relation is "me, the user"). A relation naming someone else (`dog, Robin's`) links there. |
| owner | `Owner:`, or a fact like "My dog" when there is no relation. |
| field | `people` and `places` on any entry in the folder. |
| mention | a saved line that names another known person, place or thing (whole words, 3+ letters). |

One link per pair; the strongest wins. A loaded entry ends with `Linked:` (name, `dir:` path, why). `search_memory` hits carry `linked`. The Mind map draws them. Entries with `outside` trust never add words to these lists.

`remember` returns `linked` and a `hint` when a file links to nothing; dreaming's kickoff lists such files to fix. `core.md` leads with the user's own profile (who they are, then their top facts), then the rest by score.

## Search by meaning

`src/embed.rs`. Off unless `AGENT_EMBED_MODEL` is set (for example `nomic-embed-text`); `AGENT_EMBED_URL` defaults to `http://127.0.0.1:11434/v1` (any OpenAI-compatible `/embeddings`) and must be a loopback `http` address, so memory text never leaves the computer.

- Table `memory_vectors(entry_id, model, dim, vec, text_hash)`: one f32 little-endian BLOB per entry. Cosine is plain Rust; no vector extension.
- The core timer (every 15 s) embeds entries whose vector is missing, from another model, or older than their text (hash of title, summary, detail, topic, tags), up to 256 per pass in batches of 32. This covers new writes, backfill of old entries and re-embedding after a model change. Vectors of erased or forgotten entries are dropped.
- `search_memory` (chat, dreaming's `dream_search`, `GET /v1/memory?q=`) is hybrid: FTS5 hits and vector hits (cosine >= 0.45) merged with reciprocal rank fusion (k = 60).
- If the embedding server fails, search is FTS5 alone and the core leaves it alone for 5 minutes. Models named `nomic*` get their `search_query:` / `search_document:` prefixes.

- `src/embed.rs`: `AGENT_EMBED_MODEL` (server: `nomic-embed-text`) + `AGENT_EMBED_URL` (default local Ollama, loopback only). Vectors in `memory_vectors`, embedded in the background by the core timer (new, changed, backfill, model change); `search_memory` = FTS5 + vectors merged by RRF; falls back to FTS5 when the embedder is off or down.

## User control

- Browse and search all memory by API.
- Edit or erase any entry. Erasing an entry keeps its raw sources, but marks them so dreaming does not rebuild the entry from them. To remove the sources too, use "Forget".
- Export everything as plain files (markdown + JSON).
- "Forget this" removes the entry and all its copies (below).

## Forget

"Forget" is the one case where raw data is deleted. It is a full sweep:

1. The entry and every version in its history.
2. Its raw log sources (whole items, or only the matching parts if the user picks that).
3. Other entries built from those sources: they are rewritten without the fact, or removed if nothing is left.
4. Lines in `core.md` and the index.
5. Embeddings.
6. Rolling conversation summaries that mention it.
7. Pending inbox notes about it.
8. Action log entries that hold the data (tool arguments and results), and stored SSE events (the replay copy).
9. Backups made by the engine are rotated; the user is told how long old backups keep it (default 7 days).
10. *Planned, with the [Full activity log](../plans/full-activity-log.md):* matching `activity` rows and `model_calls.thinking` values. Not built yet.

Forget cannot take back what a model provider already received in earlier prompts. The user is told this. Exports the user already saved are also out of reach.

Forget writes a tombstone (id and time, no content) so dreaming does not learn the fact back from the same sources. It needs an owner token. It cannot be undone.

## Open risks

- Wrong merges can blur two different facts. Mitigation: strict merge rule, sources and version history, review `disputed` items.
- Summaries can lose detail. Mitigation: keep raw log and let the model load sources.
- Memory poisoning from outside content. Mitigation: source trust, outside facts never reach `core.md`, never stored as preferences.
- Nightly cost on a paid model. Mitigation: only run when enough new data exists; allow a cheaper model for dreaming; token budget per run.
- Cloud models see what is in the prompt. Mitigation: tell the user which provider is used. `sensitive` entries are kept out of `core.md` and the index detail, and loading one into a prompt for a cloud provider needs a yes from the user (config `sensitive_to_cloud: ask | never | allow`, default `ask`).

Dreaming, the job that sorts memory, is in [Dreaming](../agents/dreaming.md). Recipes are in [Recipes](recipes.md).
