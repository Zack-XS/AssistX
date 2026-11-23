---
title: Plugin format
description: Plugin folders, plugin.yaml, tags, tools, sandbox and sharing.
order: 7
---

# Plugin format

A plugin is a big prompt that says how to do one job, plus optional tools. The agent can write new ones.


One page per shipped plugin is in [Plugins](../plugins/README.md). How the agent builds new ones is in [Plugin maker](../agents/plugin-maker.md).

## Folder layout

```
plugins/
  calendar/             the live version
    plugin.yaml         metadata
    guide.md            the big prompt: how to work in this domain
    schemas/            JSON Schema for tool inputs and outputs
    tools/              optional scripts or definitions
    tests/              optional checks
    state/              plugin-owned data (caches; secrets are never stored here)
  .drafts/<id>/         a draft being built or waiting for approval
  .versions/<id>/<version>/   old versions
```

Drafts are built in `.drafts/`, so the live version keeps working while a change waits for approval. Approval copies the draft over the live folder. Rollback is a copy from `.versions/`.

The core stores a hash of each live version (all files except `state/`). It checks the hash before every tool run. A mismatch blocks the run with `plugin_tampered`.

Status, provenance, hash and `created_by` are kept by the core in its own database. The core ignores any such fields in a `plugin.yaml` written by the agent or a downloaded plugin. Otherwise a plugin could declare itself `active` or `signed`.

## plugin.yaml

```yaml
id: calendar
version: 1.2.0            # semver
requires_core: ">=0.3"    # core versions it works with; the core refuses others
status: active            # draft | active | disabled   (kept by the core; see above)
summary: Read, create and move calendar events.   # becomes the index line (cleaned, max 25 words)
needs:                    # what it touches, shown to the user on approval
  network: [calendar.example.com]
  secrets: [calendar_token]
tools:
  - name: list_events
    tags: [reads]
    autonomy: auto        # requested level: auto | ask | never
    input: schemas/list_events.in.json
    output: schemas/list_events.out.json
  - name: create_event
    tags: [writes, shares]
    autonomy: ask
    undo: delete_event    # optional: the tool that reverses this one, called with the saved result
  - name: book_table
    tags: [spends]
    autonomy: ask
    cost: {amount: "$.price", currency: "$.currency"}   # required for `spends`
runtime:                  # for script tools
  command: ["python3", "tools/main.py"]
  deps: requirements.txt  # optional, installed at build time only
  timeout_secs: 120       # optional, wall time per run, at most 300 (default 30); longer is shown on the card
created_by: agent         # agent | human
provenance:               # filled by the core, not the plugin
  source: local | path | url
  url: null
  sha256: null
  installed_at: null
  signed_by: null
```

### Tool tags

`reads`, `writes`, `sends`, `spends`, `deletes`, `shares`. The core uses tags for its policy: any tool tagged `sends`, `spends`, `deletes` or `shares` is at least `ask`, whatever the plugin requests. A tool with no tags is treated as `ask`.

Two more tags can never be made `auto`, not even by the user: `unrestricted` (a shell; its card shows the exact command) and `always_ask` (every call needs a yes; risk `high`, so voice cannot approve it; used by `email.send`).

Tags are claims by the plugin author, and a bad plugin may lie. So:
- The user approves the tags on the card. Changing a tag later is a `high` risk update.
- For declared HTTP calls, the core sets `writes` on any method other than GET or HEAD, whatever the plugin says.
- The sandbox and `needs` still limit where data can go if a tag is wrong.

The plugin's `autonomy` is a **request**. The effective level is set by the Safety Gate (see [API](api.md), Autonomy levels).

Two rules the core needs to keep its promises:
- A `spends` tool must say where the amount and currency are in its arguments (`cost`). The core reads them to check the spending cap and to show the price on the card. No `cost`, no install. If the amount cannot be read at call time, the call is refused.
- Undo only exists for a tool with an `undo` entry. The core never guesses. Without one, the card and the action log say "cannot be undone".

More `plugin.yaml` fields in code:

- `needs.paths`, `needs.apple_events`, `needs.platform` in `plugin.yaml`; `runtime.builtin: rust | core`.
- `.outside` marker file in a plugin folder: its guide is wrapped as outside content.
- Plugin sign-in: `needs.oauth: spotify` in `plugin.yaml`. `GET/DELETE /v1/oauth/{provider}`, `POST /v1/oauth/{provider}/start|finish` (owner).

  PKCE with a Client ID the user pastes (no client secret).

  - Google (`google-health`) also needs a Client Secret in `start` (`client_secret`), asks for offline access, and keeps `GOOGLE_*` secrets.
  - Gmail (`gmail`, plugin `email`) is the same but keeps its own `GMAIL_*` secrets.
  - `GET /v1/oauth/{provider}` also says `needs_secret`, and the page then shows a Client Secret field.
  - The core keeps `SPOTIFY_*` secrets and refreshes the access token itself.

  The `spotify` plugin runs in the core (`src/spotify.rs`), talks to one fixed host, and needs no sandbox, so it works on the Linux server.
- Quick commands: `words:` in `plugin.yaml` (spotify, weather, scheduler, calendar ship with some). When the user's own message holds one, the core loads that plugin before the first model step (at most two; never a plugin with an `.outside` marker; never from trigger or job text), so "pause my music" needs no load round trip.

## guide.md

Free text written for the model. Should hold: what the domain covers, how to do common tasks step by step, edge cases, what to ask the user, what never to do, examples. Long is fine; it is only loaded on demand.

A guide from outside (installed by path or URL) is loaded as outside content (see [Prompt system](prompt-system.md)). It can teach steps but cannot unlock anything: the Safety Gate still decides every call.

## Tools

A tool is a script run in the sandbox or a declared HTTP call. Each has a name, tags, a JSON Schema input, a JSON Schema output, and a requested autonomy level.

- The core checks every input against the schema before the call and every output after it. Bad output is a `tool_failed` error.
- Plugins can only use the network hosts and secrets listed in `needs`.
- Tool output goes back to the model as outside content.

- Script tool protocol: stdin `{"tool","args"}`, stdout `{"ok":true,"result":...}` or `{"ok":false,"error":...}`; tests in `tests/*.json` as `{"tool","args","expect_ok"}`.
- Undo tools get `{"args", "result"}` of the action they undo.

## Sandbox

Each tool run is a separate process started by the Rust tool runner.

- **Files:** own working folder and its own plugin folder only. The plugin folder is read-only, except `state/` and a scratch folder. A tool cannot rewrite its own code or guide after approval. No home directory, no `prompts/`, no config, no memory DB, no other plugins.
  - Linux: Landlock rules + seccomp filter (or bubblewrap).
  - macOS: a `sandbox-exec` profile.
- **Network:** the sandbox has no network of its own. Its only route is a Unix socket to the local proxy (or, where the OS cannot do that, one proxy port and nothing else). The proxy allows only the hosts in `needs` (build mode: GET only, any host).

  It does its own DNS lookup and refuses answers in `127.0.0.1`, private or link-local ranges, so an allowed name cannot be turned into an internal address.

  The proxy sees host names, not encrypted content: a plugin may send any data to a host it is allowed to reach.
- **Limits:** CPU time, wall time, memory, process count, output size.
- **Secrets:** injected by name at run time as environment values. The model never sees secret values. The core removes secret values from tool output and logs.
- **Terminal:** available to the agent for building drafts, but inside the same sandbox.
- **Log:** all tool runs go to the action log.

Sandbox as built:

- tools run under `sandbox-exec` on macOS and under bubblewrap (`bwrap`) on Linux. The core probes them at start.

  On Linux each run gets an empty root with read-only `/usr`, the plugin folder and the runner; read-write only its scratch, its `state/` and `needs.paths`; no network; and the server's `data/` plus `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gh` are covered by empty folders.

  Plugins that need the network reach the run's allowlist proxy through a Unix socket in their scratch folder (a loopback bridge inside the sandbox passes it on); there is no other route out.

  If no sandbox can be applied (for example when the server itself runs inside another sandbox), tools are **refused** with `sandbox_denied` unless `AGENT_UNSAFE_NO_SANDBOX=1`.

## Tests

Tests are written by the agent, so they are a weak signal. They show the plugin runs and its schema is right, not that it is safe. Safety comes from the sandbox, the `needs` list and the Safety Gate.

## Sharing

A plugin is a folder. Zip it, put it in a git repo, install by path or URL.

- Installed plugins arrive as `draft` and need the same approval, at `high` risk.
- The core records source, hash and signer in `provenance`.
- Signing (optional in v1): a plugin may ship a signature file. The user can choose to trust some signers. Trust never skips approval; it only adds a "signed by" line.

## MCP servers

A plugin may wrap an MCP (Model Context Protocol) server instead of its own scripts:

```yaml
mcp:
  command: ["npx", "some-mcp-server"]
  tools: [search, get_page]   # only these are exposed
```

The MCP server runs in the same sandbox with the same `needs`. Each exposed tool still needs tags and a requested autonomy level in `plugin.yaml`.

> Not built yet: install by URL, signing and MCP-wrapped plugins. See [Next](../roadmap/next.md).

## Built-in plugins (small set)

- `plugin-maker`
- `memory-tools` (guide for bigger memory work)
- `scheduler` (create, list and cancel reminders and recurring jobs; the core timer runs them)

Everything else (mail, calendar, files, music, home) is an ordinary plugin.

The full list of shipped plugins is in [Plugins](../plugins/README.md).

## Real programs and longer jobs

A plugin can ask for real programs beyond the terminal plugin's allowlist, and for more time per run:

```yaml
needs:
  programs: [git, ffmpeg, /opt/homebrew/bin/jq]   # plain names or absolute paths
runtime:
  timeout_secs: 120                               # 1 to 300
```

- The core finds each program on the host when the tool runs. A missing one fails with a plain message. One inside the server's `data/` folder is refused.
- The run gets those folders on its `PATH` and read access to them. The sandbox still limits files (only `needs.paths` can be written) and network (only `needs.network`).
- The approval card lists `programs` and `added_programs`. A new program, or a longer `timeout_secs` than the default, makes the card `high` risk. Shells and interpreters (`sh`, `bash`, `python3`, `node`, `osascript`, ...) add a loud `programs_warning`.
- An update that adds a program needs a new approval. The hash check still blocks any change after approval.
