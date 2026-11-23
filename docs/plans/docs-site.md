---
title: Docs site
description: Plan for a small Rust site builder that turns docs/ into a private AssistX docs site.
order: 2
---

# Docs site

Status: plan, not built.

## What it must do

- Read the `.md` files straight from `docs/`. No copies, no second source.
- Folders are sidebar sections. Each folder's `README.md` is its section page.
- Front matter sets the page `title`, the `description` and the `order` (pages in a folder; `README.md` orders the folders).
- Relative links like `../security/safety-rules.md` work as written.
- Use the [design language](../design/README.md): tokens, Inter, Lucide icons, motion.
- Search, dark mode (follows the system, with a switch), and a good phone layout.
- A **Design** area that shows the language live: colour swatches in light and dark, the type scale, spacing and corners, motion demos, icons, and real components.

## Pick: a small Rust site builder

The repo is all Rust ([Code style](../contributing/code-style.md) rule 1). So the site is a small Rust program, not VitePress, Starlight or another Node tool. It uses `pulldown-cmark` to turn markdown into HTML. That keeps the rule with no exception.

| Need | How |
|---|---|
| Read `docs/` in place | The builder walks `docs/` and reads each `.md` file |
| `README.md` as section page | `dir/README.md` becomes `dir/index.html` |
| Relative `.md` links | Rewrite `x.md` to `x.html` (and `README.md` to `index.html`) while rendering |
| Sidebar | Built from front matter: folder title and place from `README.md`, pages by `order` |
| Search | A small JSON index (title, description, headings) and a few lines of plain script in the page |
| Dark mode, phone layout | Our own CSS from the [tokens](../design/tokens.md) |
| Design showcase | Plain HTML parts the builder adds under each `docs/design/*.md` page |

Trade-off: we build search, dark mode and the phone layout ourselves. They are small, and the look is ours.

## Build steps

1. **Crate.** A new binary crate, `docs-site/` at the repo root (outside `docs/`). Few crates, exact pins, a reason beside each: `pulldown-cmark` for markdown; front matter parsed by hand (three keys) or with the YAML crate the server already uses.
2. **Pages.** Render every `.md` under `docs/` to HTML in `docs-site/out/`. One HTML template, no outside scripts, fonts or CSS from the web. Inter is bundled.
3. **Links.** Rewrite relative `.md` links. Fail the build on a link to a file that does not exist.
4. **Sidebar.** One section per folder, ordered by the `README.md` `order`, pages by their own `order`. `MIGRATION.md` goes under "About these docs".
5. **Theme.** A `tokens.css` made from the token file (see [rollout](design-language-rollout.md) step 1). Until it exists, a hand-made copy of [Tokens](../design/tokens.md). Follow the [motion](../design/motion.md) curves and reduced motion. Use Lucide icons.
6. **Search.** Write `search.json` (title, description, headings, path). The page searches it in the browser.
7. **Design showcase.** Swatches in light and dark, the type scale, spacing and corners, motion demos with a play button, the icon grid, and live copies of buttons, bubbles, cards, sheet and toast. The `docs/design/*.md` pages stay plain markdown.
8. **Checks.** `cargo run -j1 -- check` (in `docs-site/`) fails on: missing front matter, a `title` that does not match the `#` heading, a page not listed in its folder's `README.md`, a broken relative link, and likely secrets (a built-in scan for keys, tokens and private IP ranges). It runs without gitleaks; gitleaks is an extra when installed.
9. **Build.** `cargo run -j1 -- build` makes static files in `docs-site/out/`. Tests: a golden test per check, in Rust.

## Hosting

**Local only, by default.** The docs map the attack surface: routes, sandbox notes, bridge calls, deploy paths. So they are never public unless the owner says so.

- **On this Mac:** `cargo run -j1 -- serve` serves `out/` on `127.0.0.1`.
- **On the main server, over the tailnet:** copy `out/` to the server and serve it on the tailnet address only, beside the API. Same private tunnel, no public port.
- **Public:** only if the owner asks, and then as a separate, written choice (for example a static host behind a login).

Docs hold no secrets, tokens or private addresses ([Writing docs](../quality/writing-docs.md)); the check in step 8 keeps it that way.

## Staying in sync with code

- **One source.** The site reads `docs/` directly. Nothing is copied, so nothing drifts.
- **Same change.** Code and its page change together ([Writing docs](../quality/writing-docs.md#keep-docs-in-sync)). The check runs before each build.
- **Tokens from one file.** The app page, Android resources and the site all build from `design-tokens/tokens.json`. The swatches come from that file.
- **Code links.** Pages name files like `src/audit.rs`. A later check can warn when a named file is gone.
- **One command.** `assistantx-docs-deploy` (beside `assistantx-deploy`) runs check, build and the copy to the server. Run it after a docs change.

## Not in this plan

Versioned docs, other languages, comments.
