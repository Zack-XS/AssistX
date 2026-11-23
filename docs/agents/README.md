---
title: Agents
description: How the agent works end to end: turns, plugins, dreaming, scheduled work and approvals.
order: 4
---

# Agents

The agent is the model plus the core around it. The model decides what to do; the core decides what is allowed.

```
message / job / trigger
   -> turn starts (one per thread)
   -> prompt: personality + turn rules + index + loaded items + chat
   -> model step: reply, or tool calls
        -> Safety Gate: auto runs | ask raises a card | never refused
        -> result back, wrapped as outside content
   -> repeat until a reply
   -> save_note for things worth keeping
later: dreaming sorts notes into memory
```

| Page | What it covers |
|---|---|
| [Turn loop](turn-loop.md) | One turn step by step, the tools the model always has |
| [Plugin loading](plugin-loading.md) | The index, `load`, quick commands |
| [Plugin maker](plugin-maker.md) | Learning a new ability: drafts, build mode, approval |
| [Dreaming](dreaming.md) | Sorting notes into memory |
| [Scheduled checks](scheduled-checks.md) | Work it starts on its own |
| [Approvals](approvals.md) | Cards, risk, autonomy, voice answers |
