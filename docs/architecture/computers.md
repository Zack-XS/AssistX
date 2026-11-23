---
title: Computers
description: Your own computers connect with agent-node so the assistant can work with their files.
order: 15
---

# Computers

Status: built (v0.1). Plugin `computers`, `src/nodes.rs`, program `node/` (`agent-node`).

## What it does

Your own computers (macOS, Windows, Linux) connect to the server, and the assistant can list, read, search, write, copy, move and delete files on them, inside the folders you allow.

## How it connects

- Each computer has its own token with scope `node`. A node token reaches only `/v1/nodes/work/*`.
- The computer dials out: `POST /v1/nodes/work/poll` waits up to 25 s for a job (a long poll), then the computer runs it and posts `POST /v1/nodes/work/{id}` with `{ok, result | error}`. No port is opened on the computer, and it works behind any router.
- A computer that has not polled for 75 s is offline. A tool call waits 45 s for an answer.
- `agent-node status` sends a probe poll that never takes a job.

```
GET  /v1/nodes                  computers, online or not, system, allowed folders
POST /v1/nodes {name}           owner: make a node token (shown once)
POST /v1/nodes/work/poll        node: wait for work
POST /v1/nodes/work/{id}        node: answer a job
```

Remove a computer by revoking its token (`DELETE /v1/tokens/{id}`).

## Safety

Two locks, so neither side alone is trusted:

1. **Server (Safety Gate).** `list_devices`, `info`, `list`, `stat`, `read`, `search` run by themselves. `write`, `mkdir`, `copy`, `move` ask; `delete` asks with high risk. Results are outside content.
2. **Computer.** Every path is settled by the system (links and `..` resolved) and must be inside an allowed folder (`--root`, default the home folder).

   Folders named `.ssh`, `.gnupg`, `.aws`, `.azure`, `.kube`, `.docker`, `.password-store`, `Keychains`, `Cookies`, `Credentials`, `Protect`, and agent-node's own settings are never opened, listed or copied. An allowed folder itself cannot be moved or deleted.

   `--read-only` refuses every change on the computer, whatever the server says.

Limits: read 200 KB by default (1 MB max per call, `offset` for more), write 5 MB, search up to 200 results, 100 000 entries or 25 s; text search skips files over 2 MB and binary files; copy up to 20 000 files.

## Install

```
agent-node setup --server http://SERVER:8790 --token tok_... [--root ~/Documents]... [--read-only]
agent-node install      # macOS: launchd agent; Linux: systemd --user; Windows: a task at logon
agent-node status
agent-node uninstall
```

- macOS: reading Documents, Desktop or Downloads needs Full Disk Access for `~/.local/bin/agent-node` (System Settings, Privacy & Security). Log: `~/Library/Logs/agent-node.log`.
- Linux: `loginctl enable-linger $USER` keeps it running while logged out.
- Windows: the program lives in `%LOCALAPPDATA%\agent-node\`, settings in `%APPDATA%\agent-node\`.

Builds: `cargo build --release -p agent-node` (macOS, Linux). Windows from Linux: target `x86_64-pc-windows-gnullvm` with llvm-mingw as the linker. The program uses only `ureq` and `serde_json`, the same pins as the server.

## Not yet

- Running commands on a computer (would be an `unrestricted` tool, like the terminal's `shell`).
- Sending a file from one computer to another, or to the phone.
- Signed builds; macOS may ask to allow an unsigned program the first time.
