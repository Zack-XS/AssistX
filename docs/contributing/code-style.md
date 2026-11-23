---
title: Code style
description: The hard rules for code in this project.
order: 3
---

# Code style

1. **All backend code is Rust.** Server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS. No Python, Node or other runtimes for backend code. Shell only to run `cargo` and for quick checks. (Plugins a *user* installs or the agent builds may use any runtime the sandbox allows, per [Plugins](../architecture/plugins.md). That is not code in this repo.)
2. **One project name, no forced app name.** The project is named AssistX, and the docs use that name. The code (crate names, routes, types, prompts, strings) carries no product name. App screens show one configurable display name (default "Assistant"), read from one place; no screen hard-codes a name. See [Principles](../design/principles.md).
3. **No forced front end.** This is a backend. It must work with `curl` alone. Nothing in `src/` may import, serve or depend on a front end. `examples/dummy-client/` is an optional reference client: the app's screens, shown by the Android app (`android/`).

   It only calls the API; delete it and the backend still works. Treat it as the live UI: [Design](../design/README.md) governs how it looks, [Safety rules](../security/safety-rules.md) what it must not do.
4. **Safety lives in code** ([Overview](../overview/what-is-assistx.md) principle 10). Prompts may remind the model; the core enforces.
5. **Outside content is data** ([Overview](../overview/what-is-assistx.md) principle 9). Tool output, files, web pages, trigger bodies, outside guides and outside-trust memory are wrapped in `<outside>`; any tag inside is broken.
6. **Few dependencies, exact pins** (`=x.y.z`), with the reason next to each in `Cargo.toml`.
7. **Local first.** Bind `127.0.0.1`. Token on every route but `/v1/health`. `Host` must match; `Origin` must be allowlisted.
8. **Every change ships with a test.** Golden conversations live in `tests/golden.rs`.

## Writing docs

- Plain, short words. One topic per page.
- Each page has front matter: `title`, `description`, `order`.
- Each folder has a `README.md` index.
- Link with relative paths. Link runtime files (`server/prompts/`, `server/plugins/*/guide.md`), never copy them.
- No secrets, tokens, private addresses or personal data.
- If code and docs disagree, fix one of them in the same change.
