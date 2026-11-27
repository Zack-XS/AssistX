---
title: AssistX docs
description: Index of all AssistX documentation.
order: 0
---

# AssistX docs

AssistX is an AI assistant that acts like a buddy who runs your life. It remembers you, acts for you, starts work on its own and learns new skills, with safety kept in code. It has two parts: the server in `server/` (Rust engine and API) and the Android app in `android/`.

New here? Read [What AssistX is](overview/what-is-assistx.md), then [Getting started](getting-started/README.md).

## Top rule

All backend code is Rust: server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS. See [Code style](contributing/code-style.md) rule 1.

## [Overview](overview/README.md)

What AssistX is, the rules it follows, and the words it uses.

- [What AssistX is](overview/what-is-assistx.md): An AI assistant that acts like a buddy who runs your life.
- [Principles](overview/principles.md): The rules the whole design follows.
- [Glossary](overview/glossary.md): Short meanings of the words used across these docs.

## [Getting started](getting-started/README.md)

Run the server, run the Android app, and set it up.

- [Run the server](getting-started/run-server.md): Build, test and start the Rust server, and make tokens.
- [Run the Android app](getting-started/run-android.md): Build or install the app and connect it to your server.
- [Configuration](getting-started/configuration.md): Every environment setting, its default, and what changes at run time.

## [Architecture](architecture/README.md)

How the server is built, part by part.

- [API](architecture/api.md): The one outside door: transport, auth, scopes and every route.
- [Events](architecture/events.md): The event stream: format, replay and every event type.
- [Turn engine](architecture/turn-engine.md): How one turn runs: threads, queues, the Safety Gate, limits and endings.
- [Prompt system](architecture/prompt-system.md): The layers of the prompt, the index, outside content and the budget.
- [Memory](architecture/memory.md): Memory stores, entries, trust, folders, profiles, links, search and forget.
- [Recipes](architecture/recipes.md): Saved multi-step tool runs the user can replay.
- [Plugin format](architecture/plugins.md): Plugin folders, plugin.yaml, tags, tools, sandbox and sharing.
- [Scheduler and notices](architecture/scheduler-and-notices.md): Jobs, alarms, reminders, tasks, sounds and unasked notices.
- [Voice](architecture/voice.md): Live mode, spoken answers and the voice screen.
- [Models](architecture/models.md): Model profiles, auto mode, cloud rules and model tests.
- [Accounts and encryption](architecture/accounts.md): Several people on one server, keys, sign-in and remembered phones.
- [Multi-device plan](architecture/multi-device.md): A plan for one owner with many devices. Not built yet.
- [Location](architecture/location.md): Phone points, visits, places, place triggers, geofences, noticing and travel time.
- [Phone capture](architecture/phone-capture.md): Speech heard by the phone, as text, picked into memory only with a yes.
- [Computers](architecture/computers.md): Your own computers connect with agent-node so the assistant can work with their files.
- [Audit and reports](architecture/audit.md): What is kept, the review pack, chat reports and automatic reports.
- [Mind and morning brief](architecture/mind.md): The quiet task that thinks every 30 minutes, and the morning brief.
- [Deploy](architecture/deploy.md): The main server: layout, deploy from the Mac, logs and settings.

## [Agents](agents/README.md)

How the agent works end to end: turns, plugins, dreaming, scheduled work and approvals.

- [Turn loop](agents/turn-loop.md): What the agent does inside one turn, and the tools it always has.
- [Plugin loading](agents/plugin-loading.md): How the agent finds and loads a plugin when it needs one.
- [Plugin maker](agents/plugin-maker.md): How the agent learns a new ability: drafts, build mode, approval and versions.
- [Dreaming](agents/dreaming.md): The background job that sorts notes into memory.
- [Scheduled checks](agents/scheduled-checks.md): Work the assistant starts on its own: jobs, tasks, the mind, the brief and watches.
- [Approvals](agents/approvals.md): Cards, risk, autonomy levels, batches, voice answers and waiting.

## [Android app](android/README.md)

How the phone app is built and how it talks to the server.

- [App structure](android/app-structure.md): The files, screens, permissions and sounds of the Android app.
- [Talking to the server](android/talking-to-server.md): Tokens, the live connection, uploads, and which screen calls which route.
- [Updates and release](android/updates-and-release.md): How the app is built, published, installed and updated from your own server.

## [Plugins](plugins/README.md)

One page per shipped plugin, with its tools and limits.

- [calendar](plugins/calendar.md): The user's calendar - see, add, move and delete events, repeats, and reminders that can ring.
- [computers](plugins/computers.md): The user's own computers (Mac, Windows, Linux) running agent-node - list, read, search, copy and move their files.
- [email](plugins/email.md): Your Gmail - recent and unread mail, search, read one mail, a short inbox summary, drafts and replies; sending always asks first.
- [files](plugins/files.md): Browse, read, write, move and find files inside one allowed folder.
- [google-health](plugins/google-health.md): Your Fitbit and Pixel Watch data via Google - steps, heart rate, sleep, exercise, weight and more. Read only.
- [health](plugins/health.md): The user's body numbers per day from their phone - steps, sleep, heart rate, weight, workouts.
- [hevy](plugins/hevy.md): Your Hevy workout log - recent workouts, routines, exercises; log a finished workout.
- [lists](plugins/lists.md): The user's lists - shopping, to-do or any name; add, tick off, remove and share items.
- [location](plugins/location.md): Where the user is and has been - places they stay or visit often, named by them or by OpenStreetMap.
- [memory-tools](plugins/memory-tools.md): Skill for writing things down. Load it when the user shares something worth keeping, or wants memory organised, folders made or facts rated.
- [phone](plugins/phone.md): Do things on the user's phone - open the dialer or a ready-to-send text, open an app, set a timer or a clock-app alarm, start directions.
- [plugin-maker](plugins/plugin-maker.md): How to learn a new ability by building a plugin when nothing in the index fits.
- [scheduler](plugins/scheduler.md): Reminders by time or place, repeating jobs, alarms that ring, and tasks that start the assistant later.
- [spotify](plugins/spotify.md): Music: plays songs, artists, albums, playlists and liked songs on your Spotify devices.
- [terminal](plugins/terminal.md): Run commands in the user's work folder - allowlisted programs, or any command line after a warned yes.
- [weather](plugins/weather.md): Current weather and a three day outlook for a named place.
- [web](plugins/web.md): Read public web pages as plain text and list their links.

## [Security](security/README.md)

Threat model, safety rules, and how to report a problem.

- [Threat model](security/threat-model.md): What we protect, who attacks, and what stops them.
- [Safety rules](security/safety-rules.md): The rules every change must keep: core, plugins, page and app.
- [Reporting a problem](security/reporting.md): How to report a security problem, scope and safe harbor.
- [Private data](security/private-data.md): What must never be committed, tests included, and the check that stops it.

## [Contributing](contributing/README.md)

How to work on AssistX: setup, code rules, tests and the change flow.

- [Dev setup](contributing/dev-setup.md): Tools, build and test commands, and what never to commit.
- [Code layout](contributing/code-layout.md): Where the code, prompts, plugins, tests and app live.
- [Code style](contributing/code-style.md): The hard rules for code in this project.
- [Testing](contributing/testing.md): Golden tests, model tests, the smoke test and the test sets we need.
- [Change flow](contributing/pr-flow.md): How a change goes in, and pull request etiquette.
- [Branches](contributing/branches.md): one branch, production: what runs on it and how a change gets there.

## [Roadmap](roadmap/README.md)

What is done, what is next, and what is still open.

- [Done](roadmap/done.md): What is built today.
- [Next](roadmap/next.md): Known gaps and work not built yet.
- [Open questions](roadmap/open-questions.md): Decisions still needed, and where old docs and code disagree.

## [Design](design/README.md)

The AssistX design language: how the app and the docs site look, move and adapt.

- [Principles](design/principles.md): The few rules every AssistX screen follows.
- [Color](design/color.md): Colour tokens for light and dark, and how colour is used.
- [Typography](design/typography.md): The font, the type scale and how text is set.
- [Spacing and layout](design/spacing-and-layout.md): Compact spacing, corner sizes, the grid and layouts from phone to desktop.
- [Motion](design/motion.md): Durations, easing, springs, page moves and reduced motion.
- [Haptics](design/haptics.md): The few vibrations we use, when, how to code them, and what never to do.
- [Icons](design/icons.md): One soft, round icon set, sizes, states and animated icons.
- [Components](design/components.md): Buttons, cards, chat bubbles, lists, sheets, inputs and notices.
- [Smart UI](design/smart-ui.md): How screens adapt to time, place and what you are doing.
- [Accessibility](design/accessibility.md): Contrast, touch size, text size, screen readers and motion.
- [Tokens](design/tokens.md): One table of design values for the app page, Android and the docs site.
- [Screens](design/screens.md): The four tabs, where every screen lives, the states each screen shows, and how to add one.

## [Quality](quality/README.md)

The rules that keep AssistX code safe, fast, tested, logged and easy to read.

- [Quality bar](quality/quality-bar.md): What "done" means: tests, review, lint, no secrets, small changes.
- [Safety](quality/safety.md): The safety rules every change must keep.
- [Logging](quality/logging.md): Every change adds logs. What to log, levels, fields, and what never to log.
- [Testing](quality/testing.md): What to test, how, and which tests each kind of change needs.
- [Performance](quality/performance.md): Keep the server quick and small, and the model calls cheap.
- [Android battery](quality/android-battery.md): How the app stays light on battery, and the gaps.
- [Code review](quality/reviews.md): The checklist a reviewer runs on every change.
- [Writing docs](quality/writing-docs.md): Format, front matter, style, where pages go, and keeping docs in sync.
- [Agents checklist](quality/agents-checklist.md): Rules AI agents must follow when they change the code.

## [Plans](plans/README.md)

Specs for work that is not built yet.

- [Full activity log](plans/full-activity-log.md): An opt-in, per-account setting that records everything for that account so AI agents can review it.
- [Docs site](plans/docs-site.md): Plan for a small Rust site builder that turns docs/ into a private AssistX docs site.
- [Design language rollout](plans/design-language-rollout.md): Steps to bring the Android app and its page onto the design language.

## About these docs

- [Migration](MIGRATION.md): where each old doc went.
- Runtime files are not docs: `server/prompts/*.md`, `server/plugins/*/guide.md` and `server/data/**` are loaded by the engine. These pages link to them.
- Each page has front matter (`title`, `description`, `order`) for the docs website.

## Archive

- [Retired docs](RETIRED.md): Where the old server docs went and what supersedes them.
