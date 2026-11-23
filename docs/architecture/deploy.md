---
title: Deploy
description: The main server: layout, deploy from the Mac, logs and settings.
order: 18
---

# Deploy

The code is written and tested on the Mac, then sent to a Debian server that runs it all day. The phone app talks to that server.


## Layout

| What | Where |
|---|---|
| Server | Debian 13, reached with `ssh assistantx-server` (an alias in `~/.ssh/config`, key login only) |
| Source and build | `~/assistantx` on the server (`target/release/agent-core`) |
| Data (database, owner token, memory folders) | `~/assistantx/data` on the server. Separate from the Mac's `data/`. |
| Settings | `~/assistantx/data/server.env` (mode 0600; model key, bind address, allowed hosts and origins) |
| Service | `systemd --user` unit `assistantx` (`~/.config/systemd/user/assistantx.service`), started at boot (linger is on) |
| Address | `http://<server-ip>:8790` (home network). Port 8787 on that machine belongs to another program. |

## Deploy (Mac to server)

1. Change and test on the Mac.
2. Run `assistantx-deploy` (or `assistantx-deploy test` to run the tests on the server first). It does this:
   - sends the source with `tar` over ssh, leaving out `target/`, `data/`, `.git` and plugin state;
   - runs `cargo build --release` on the server;
   - restarts the service and checks `/v1/health`.

The same steps by hand:

```sh
COPYFILE_DISABLE=1 tar --no-xattrs --exclude=./target --exclude=./data --exclude=./.git -czf - . \
  | ssh assistantx-server 'cd ~/assistantx && rm -rf src docs prompts tests examples && tar -xzf -'
ssh assistantx-server '. ~/.cargo/env && cd ~/assistantx && cargo build --release && systemctl --user restart assistantx'
```

The helper script lives outside this repo (`~/bin/assistantx-deploy`) because the repo holds Rust only.

## Look at it

```sh
ssh assistantx-server 'systemctl --user status assistantx'
ssh assistantx-server 'journalctl --user -u assistantx -f'
ssh assistantx-server 'cat ~/assistantx/data/owner.token'     # the owner token, for the app
ssh assistantx-server 'cd ~/assistantx && AGENT_DATA_DIR=data ./target/release/agent-core token list'
```

## Settings that matter

- `AGENT_BIND=<server-ip>` and `AGENT_ALLOWED_HOSTS=<server-ip>`: the server answers on the home network only, not on every address. This is plain HTTP, so the token can be seen by anyone on that network. For use away from home, put a private tunnel (Tailscale) in front and list its name in `AGENT_ALLOWED_HOSTS`.
- `AGENT_ALLOWED_ORIGINS`: the Android app's page origin (`https://appassets.androidplatform.net`) and the Mac test page.
- **Tools are off on this server.** Linux has no sandbox yet (see [Next](../roadmap/next.md)), so plugin tools such as web and weather are refused. Reminders, memory and folders still work. To turn them on anyway, add `AGENT_UNSAFE_NO_SANDBOX=1` to `server.env` and restart; then they run with no sandbox.
- The model key is in `server.env` only. Never commit it.

> Code check: the line above says tools are off because Linux has no sandbox. The code now has a bubblewrap sandbox on Linux; the newer note below says tools run there in bubblewrap and tools that need the network are refused. See [Open questions](../roadmap/open-questions.md).

## Main server notes

The code runs all day on a Debian server, `ssh assistantx-server`, at `http://<server-ip>:8790`.

Work on the Mac, then run `assistantx-deploy` (all `~/bin/assistantx-*` scripts reach the server through `assistantx-ssh`: the home address when it answers, else Tailscale `<tailscale-ip>`) (outside the repo, `~/bin`): it sends the source, builds on the server and restarts the service.

`assistantx-deploy smoke` (or `assistantx-smoke` alone, both in `~/bin`) then makes a chat on `local-light`, sends 10 fixed requests (weather, music, reminder set and cancel, calendar, memory) and fails if a turn errors, a turn ends on a FAILED tool result, or a reply is empty; the chat is archived after and the owner token is never printed.

The server has its own `data/`. Tools run there in the bubblewrap sandbox (terminal, files); tools that need the network are refused there. The Android app is the separate project `AssistantX-Android`.

The Android build and release flow is in [Updates and release](../android/updates-and-release.md).
