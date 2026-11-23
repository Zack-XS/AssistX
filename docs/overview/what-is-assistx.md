---
title: What AssistX is
description: An AI assistant that acts like a buddy who runs your life.
order: 1
---

# What AssistX is

AssistX is an AI assistant that acts like a buddy who runs your life. It fixes the usual problems of chat assistants like ChatGPT or Gemini, and puts it all in one place:

| Usual problem | What AssistX does |
|---|---|
| Forgets you between chats | Keeps a memory and sorts it each night ([Memory](../architecture/memory.md), [Dreaming](../agents/dreaming.md)) |
| Only talks, does not act | Uses tools: calendar, alarms, lists, music, mail, files, phone ([Plugins](../plugins/README.md)) |
| Waits to be asked | Runs jobs, checks in, sends a morning brief ([Scheduled checks](../agents/scheduled-checks.md)) |
| Cannot learn new skills | Writes its own plugins, with your yes ([Plugin maker](../agents/plugin-maker.md)) |
| Risky when it acts | Risky acts need your tap; safety is in code ([Security](../security/README.md)) |
| Your data sits with someone else | Runs on your own server; any model can plug in ([Models](../architecture/models.md)) |

Two parts make it: the server in `server/` (Rust engine and API) and the Android app in `android/`.

> Naming: the server's own rules say the assistant has no fixed name and the user may name it. "AssistX" is the project name. See [Open questions](../roadmap/open-questions.md).

Old design status: draft v0.3. The core engine and API are written in Rust.

## What this is

An open-source backend that turns any language model into a personal assistant that runs a person's day. It has no front end and no brand name. Others build faces on top of the API.

It does not make the model smarter. It makes the system around the model smarter: it remembers, it acts, it starts work on its own, and it grows new abilities.


## Parts

```
Front end (not ours)
   |  HTTP + SSE
   v
API  -->  Turn Engine  -->  Model Adapter  --> any LLM
            |   ^
            |   +-- Prompt Assembler (base prompt + index + loaded items)
            v
   Safety Gate  -->  Tool Runtime (sandbox)  <-->  Plugins (files)
            |
            v
   Memory Store (DB)  <--  Dreaming job
   Scheduler (core timer, starts turns)
   Event Feed (tells front ends what happened)
   Action Log
```

The scheduler and Safety Gate parts are in [Scheduler and notices](../architecture/scheduler-and-notices.md) and [Turn engine](../architecture/turn-engine.md).

## Out of scope for v0.1

Any UI. Voice. Multi-user hosting. Mobile. These sit on top of the API later. The data layout keeps an `owner_id` so multi-user can be added without a rewrite.

> Code check: since v0.1, a UI (the Android app and its page), voice, mobile and several accounts per server were built. See [Done](../roadmap/done.md).
