---
title: Run the server
description: Build, test and start the Rust server, and make tokens.
order: 1
---

# Run the server

The server is in `server/`. It is a Rust workspace: `agent-core` (the server) and `node/` (`agent-node`, for [Computers](../architecture/computers.md)).

## Build, test, run

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -j1                      # this machine is low on memory
cargo test -j1 -- --test-threads=2   # unit and golden tests
cargo test -j1 -p agent-node         # the computer program (node/)

# Any OpenAI-compatible model (Ollama shown):
AGENT_MODEL_BASE_URL=http://127.0.0.1:11434/v1 AGENT_MODEL_NAME=llama3.1 cargo run -j1
# No model at all, scripted replies:
AGENT_MODEL_KIND=mock AGENT_MOCK_SCRIPT=script.json cargo run -j1
# Tokens from the CLI:
cargo run -j1 -- token create client phone
```

First run writes the owner token to `data/owner.token` (mode 0600); the DB keeps only its hash.

The default port is 8787 (`AGENT_PORT`). Check it is up: `curl http://127.0.0.1:8787/v1/health`.

## Command line

| Command | What it does |
|---|---|
| `agent-core` | Start the server. |
| `agent-core token create <client\|owner\|trigger\|device> [label]` | Make a token. |
| `agent-core token list` / `token revoke <id>` | List or revoke tokens. |
| `agent-core eval [--cloud] ...` | Run the model tests ([Models](../architecture/models.md)). |
| `agent-core server-password` | Set the server password from stdin ([Accounts](../architecture/accounts.md)). |
| `agent-core tool-run <plugin> <tool>` | Used by the core to run one sandboxed tool. Not for people. |

## Accounts

By default the server runs with accounts ([Accounts](../architecture/accounts.md)): the phone signs in with the server token, then an account name and passphrase. `AGENT_SINGLE_USER=1` keeps the old one-owner mode, used for development and the golden tests.

## Sandbox

tools run under `sandbox-exec` on macOS and under bubblewrap (`bwrap`) on Linux. The core probes them at start.

On Linux each run gets an empty root with read-only `/usr`, the plugin folder and the runner; read-write only its scratch, its `state/` and `needs.paths`; no network; and the server's `data/` plus `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.config/gh` are covered by empty folders.

Plugins that need the network reach the run's allowlist proxy through a Unix socket in their scratch folder (a loopback bridge inside the sandbox passes it on); there is no other route out.

If no sandbox can be applied (for example when the server itself runs inside another sandbox), tools are **refused** with `sandbox_denied` unless `AGENT_UNSAFE_NO_SANDBOX=1`.

## Talk to it with curl

```sh
TOKEN=$(cat data/owner.token)
curl -s -X POST -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8787/v1/threads
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"text":"hi"}' http://127.0.0.1:8787/v1/threads/<id>/messages
```

Test pages: `server/examples/dummy-client/index.html` (the app's page) and `server/examples/panel/index.html` (usage panel). Both only call the API.
