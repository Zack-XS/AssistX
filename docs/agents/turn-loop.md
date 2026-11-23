---
title: Turn loop
description: What the agent does inside one turn, and the tools it always has.
order: 1
---

# Turn loop

One turn, in order:

1. A message, job, trigger or dreaming run starts a turn (see [Turn engine](../architecture/turn-engine.md)).
2. The core builds the prompt: personality, turn rules, the index, loaded items, the chat (see [Prompt system](../architecture/prompt-system.md)).
3. Quick-command words in the user's own message load their plugin before the first model step (see [Plugin loading](plugin-loading.md)).
4. The model answers or calls tools. Each tool call goes through the Safety Gate: `auto` runs, `ask` raises a card (see [Approvals](approvals.md)), `never` is refused.
5. Each tool result comes back with a `[core] RESULT` line, wrapped as outside content.
6. Steps repeat until the model replies with no tool call, or a limit stops it.
7. An attended turn always ends with words. The core streams the reply as events (see [Events](../architecture/events.md)).

## Decision flow inside a turn

```
read message
  -> need an app or skill?  check index (open folders if needed)
        found     -> load plugin -> use it
        not found -> is it possible at all?
                       no  -> say so plainly
                       yes -> ask the user "Want me to learn to do X?"
                                yes -> load plugin-maker -> create draft -> approval
  -> need past facts?       check index -> load entry, or search_memory
  -> tool call              Safety Gate decides auto / ask / never
  -> answer
  -> save_note for anything worth keeping
```

## Built-in tools the model always has

| Tool | Use |
|---|---|
| `load(id)` | Bring a plugin or memory entry into the prompt |
| `unload(id)` | Drop a loaded item to free budget |
| `search_memory(query)` | Find entries not obvious from the index |
| `save_note(text, topic?, importance?, people?)` | Put something in the inbox for dreaming. Extra fields are hints. |
| `ask_user(question, options?)` | Ask the user one short question on a card (kind `question`) and wait for the reply. Up to 6 options; the user may type anything. Up to 3 per turn; not in scheduled or trigger turns. Tool approvals are raised by the core, not by this. |
| `notify(text, title?, silent?, priority?)` | Send the user a notification they did not ask for. Silent or normal; urgent only for what cannot wait. Max 5 per turn. |
| `run_plugin_tool(plugin, tool, args)` | Call a tool a loaded plugin exposes |

Plugin tools are not visible until the plugin is loaded. This keeps the tool list short.

These core tools handle everyday memory use. The `memory-tools` plugin is a guide for bigger memory work: reviewing disputed entries, merging by hand, cleaning a topic at the user's request.

More core tools in code: `update_note`, `forget_note` ([Memory](../architecture/memory.md)), `list_recipes`, `run_recipe`, `forget_recipe` ([Recipes](../architecture/recipes.md)), `use_bigger_model` ([Models](../architecture/models.md)), `listen_for_reply` (live mode only, [Voice](../architecture/voice.md)), `journal_add`, `journal_read` (Mind runs only, [Mind](../architecture/mind.md)).
