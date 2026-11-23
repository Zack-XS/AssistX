---
title: Code layout
description: Where the code, prompts, plugins, tests and app live.
order: 2
---

# Code layout

```
AssistX/
  docs/                    these docs
  server/                  Rust workspace (git repo)
    src/                   the core: API, engine, memory, scheduler, plugins in the core
    src/plugins/           Rust tool code for sandboxed plugins (files, terminal, weather, web)
    node/                  agent-node, the program a computer runs
    prompts/               runtime prompts (loaded by the engine)
    plugins/<id>/          plugin.yaml + guide.md (loaded by the engine)
    data/                  runtime data: database, tokens, memory copy (never commit)
    tests/                 golden.rs, accounts.rs, lists.rs
    evals/cases.json       model test cases
    examples/dummy-client/ the app's page (reference client)
    examples/panel/        usage panel page
    SOUND EFFECTS/         sounds copied into the app
    docs/                  old design docs (to be retired; see MIGRATION.md)
  android/                 Android app (not in git)
  reviews/                 review packs and personal notes (not docs; private)
```

The module map of `server/src/` is in [Architecture](../architecture/README.md), Code map.
