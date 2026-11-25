---
title: Dev setup
description: Tools, build and test commands, and what never to commit.
order: 1
---

# Dev setup

## Server

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

## Working here

- Rust lives at `~/.cargo/bin`. Build and test with `-j1` on this machine.
- Never commit `data/`, `.token`, `target/` or any private data ([Private data](../security/private-data.md)).
- Turn on the private-data hook once: `git config core.hooksPath .githooks`.
- The dev Mac is low on memory: build and test with `-j1`, run targeted tests, and build the APK on the server.

## Android

Open `android/` in Android Studio, or run `./gradlew assembleDebug`. Release APKs are built on the server ([Updates and release](../android/updates-and-release.md)). `android/` is in the same git repo as the server.
