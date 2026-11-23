---
title: Plugin loading
description: How the agent finds and loads a plugin when it needs one.
order: 2
---

# Plugin loading

No router code picks plugins. The model reads the index and loads what it needs.

1. The index lists every active plugin in one line, from `summary` in `plugin.yaml`, ending with `(load: plugin:<id>)`.
2. The model calls `load(plugin:<id>)`. The plugin's `guide.md` comes into the prompt and its tools become visible.
3. Loaded items share one budget. When it is full the least recently used item unloads. Items used in this turn stay until it ends.
4. `unload(id)` frees room by hand.

A guide from outside, or a plugin folder with an `.outside` marker, is loaded as outside content. It can teach steps but cannot unlock anything.

## Shortcuts

- Quick commands: `words:` in `plugin.yaml` (spotify, weather, scheduler, calendar ship with some). When the user's own message holds one, the core loads that plugin before the first model step (at most two; never a plugin with an `.outside` marker; never from trigger or job text), so "pause my music" needs no load round trip.
- A `run_plugin_tool` call to a real tool of an active plugin that is not loaded yet loads the plugin first and goes on (the gate decides as usual); the result ends with a `[core] Note` saying so. An unknown plugin or tool is still refused. Small models skipped the load and then stopped after the refusal.

See [Prompt system](../architecture/prompt-system.md) for the index format and [Plugin format](../architecture/plugins.md) for `plugin.yaml`.
