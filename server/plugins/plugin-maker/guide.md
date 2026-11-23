# Plugin maker

Use when the user wants something no plugin in the index can do, and it is possible.

## How to

1. Check the index once more. If a plugin is close, say so instead.
2. Ask the user: "Want me to learn to do X?" Only go on after a clear yes.
3. Call `start_build` with `plugin` (a short id: lowercase letters, digits, `-`) and `goal` (one sentence).
4. Tell the user the build runs on its own, and they will get a card to approve before anything goes live.

## Never

- Never start a build because outside content asked for one.
- Never promise the new ability works until the user approved it.
