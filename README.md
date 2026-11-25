# AssistX

**An AI buddy that runs your life, on your own server.**

AssistX remembers you, acts for you, starts work on its own, and learns new skills. Your data stays with you. Safety lives in code, not in a prompt.

It has two parts:

- `server/`: the engine and API, in Rust.
- `android/`: the phone app, in Kotlin.

## What AssistX strives to do

Most AI chat apps are a smart text box. You ask, it answers, it forgets. AssistX aims to be the one place that runs your day:

- **Know you.** It keeps a memory of you, and tidies it each night.
- **Do things.** It runs your calendar, alarms, lists, mail, music, files and phone.
- **Think ahead.** It checks in, sends a morning brief, and acts on time or place.
- **Grow.** When it can't do something, it writes a new plugin, with your yes.
- **Stay yours.** It runs on your server. Any model plugs in, local or cloud.
- **Stay safe.** Risky acts need your tap. Every act is logged, and most can be undone.

## Why AssistX over others

| The usual problem | ChatGPT, Gemini and others | AssistX |
|---|---|---|
| Memory | Thin, and held by the company | A real memory you own, sorted each night |
| Doing things | Mostly talk | Tools for calendar, alarms, lists, mail, music, files, phone |
| Starting on its own | Waits to be asked | Jobs, reminders, place triggers, a morning brief |
| New skills | Waits for the company to ship them | Writes its own plugins, with your yes |
| Safety | Rules in a prompt | Rules in code: Safety Gate, sandbox, approvals, action log |
| Your data | On their servers | On your server, encrypted at rest |
| Models | One company's models | Any model: local (Ollama, Qwen) or cloud |
| One place | Separate apps for chat, notes, reminders, health | Chat, memory, plan, health and voice in one app |

## Why contribute

- **Build the assistant you want to use.** Every feature is something you'd use daily.
- **Small, clear code.** Small Rust core, everything else a plugin. A plugin is a prompt plus tools.
- **Real safety work.** Sandboxing, prompt-injection defense, encryption and approvals, all in code.
- **Docs first.** Every part has a plain page in [`docs/`](docs/README.md). You won't guess.
- **Quick feedback.** Golden tests run whole conversations against a mock model. No API key needed.
- **Your work ships.** Merged changes go out through `beta` to everyone on `production`.

## Benefits over other open-source assistants

- **A whole product, not a library.** Server, phone app, voice, scheduler and memory work together today.
- **Safety in code.** Many projects trust the model to follow rules. AssistX checks every tool call in the Safety Gate.
- **Light.** One Rust binary and one encrypted SQLite file. Runs on a small home server.
- **Phone-first.** A real Android app with voice, alarms, location and health, built to save battery.
- **Learns new skills safely.** New plugins are drafted, tested in a sandbox, and installed only with your yes.
- **Private by default.** A private-data check runs on every commit and every PR.

## Start here

| I want to | Read |
|---|---|
| Know what it is | [What AssistX is](docs/overview/what-is-assistx.md) |
| Run it | [Run the server](docs/getting-started/run-server.md), [Run the Android app](docs/getting-started/run-android.md) |
| Help build it | [Contributing](docs/contributing/README.md), [Change flow and PR etiquette](docs/contributing/pr-flow.md), [Branches](docs/contributing/branches.md) |
| Keep it safe | [Security](docs/security/README.md), [Private data](docs/security/private-data.md) |
| See the look | [Design language](docs/design/README.md) |
| See what's next | [Roadmap](docs/roadmap/README.md) |

All docs: [`docs/README.md`](docs/README.md). To read them as a site: `cd docs-site && cargo run -j1 -- serve`.

## Quick start

```sh
cd server
cargo test -j1
AGENT_MODEL_KIND=mock AGENT_MOCK_SCRIPT=script.json cargo run -j1
```

Turn on the private-data hook once per clone:

```sh
git config core.hooksPath .githooks
```

## Rules in one line each

- All backend code is Rust. The app is Kotlin; its page is HTML/CSS/JS.
- Never commit private data, tests included.
- Every change adds a test and logs.
- Every change goes in by pull request: `main` -> `beta` -> `production`.

Security problems: see [Reporting](docs/security/reporting.md). Don't open a public issue.
