---
title: Glossary
description: Short meanings of the words used across these docs.
order: 3
---

# Glossary

| Word | Meaning |
|---|---|
| Core | The Rust server program (`agent-core`). It is trusted. It enforces every rule. |
| Thread | A chat. User threads come from clients; system threads (`system:scheduler`, `system:dreaming`, `system:task:...`, `system:build:...`) come from the core. |
| Turn | One run of the engine in a thread. Only one runs per thread at a time. |
| Attended turn | A turn a person is waiting on. Unattended turns come from jobs, triggers, dreaming and the mind. |
| Event | Something that happened, sent to clients over SSE. |
| Index | The short list the model sees each turn: one line per plugin and memory folder. |
| Load | The `load(id)` tool. It brings a plugin guide, a memory entry or a folder listing into the prompt. |
| Plugin | A guide for the model (`guide.md`) plus optional tools, described by `plugin.yaml`. |
| Guide | The `guide.md` of a plugin. A runtime file the engine loads, not a doc. |
| Tool | One action a plugin offers. It has tags, a schema and a requested autonomy. |
| Tags | `reads`, `writes`, `sends`, `spends`, `deletes`, `shares`, plus `unrestricted` and `always_ask`. The core sets risk from them. |
| Autonomy | `auto` (just do it), `ask` (needs a yes), `never` (blocked). |
| Safety Gate | Core code that checks every tool call before it runs. |
| Approval (card) | A question to the user that blocks a risky act. Built by the core from the real call. |
| Action log | The record of every tool call and its outcome. Some actions can be undone. |
| Outside content | Anything not written by the user or the core. Wrapped in `<outside>`. It is data, never orders. |
| Trust | The label the core puts on text and memory: `user`, `inferred`, `outside`, `assistant`, `core`. |
| Memory entry | One sorted fact or profile in the memory DB. |
| Inbox | Notes the model saved during turns, waiting for dreaming. |
| `core.md` | A short profile of the user (at most 12 lines). Loaded on request. |
| Dreaming | The background job that sorts notes into memory. |
| Profile | A person, place or thing's own folder with a profile and a timeline. |
| Recipe | Saved steps of a multi-step run that can be replayed. |
| Job | A scheduled thing: reminder, alarm, task, or event reminder. |
| Task | A job that starts an unattended turn and leaves a report. |
| Notice | A message the assistant sends unasked (`assistant.notice`). |
| Quiet hours | Hours when normal notices wait. Default 22:00 to 07:00. |
| Mind | A quiet task that thinks every 30 minutes and keeps a journal. |
| Morning brief | A task that runs at the first wake of the day. |
| Phone capture | Speech heard by the phone, sent as text, kept short, picked into memory only with a yes. Also called ambient. |
| Pick | Something dreaming found in phone capture text, waiting for the user to keep or dismiss. |
| Node | A computer running `agent-node` that the assistant can work on. |
| Token scopes | `owner`, `client`, `trigger`, `device` (phone uploads only), `node` (computers only). |
| Server token | The server password. Opens only `/v1/server/*` and app updates. |
| Account | One person's own world on a server, in its own encrypted database. |
| Build mode | The turn mode that researches and writes a plugin draft, with no memory. |
| Draft | A plugin version waiting for approval. |
| Live mode | A voice turn: short spoken replies and `listen_for_reply`. |
| Profile (model) | A named model setting such as `local`, `local-light`, `gemini`. |
| Quick command | Words in `plugin.yaml` (`words:`) that load a plugin before the first model step. |
