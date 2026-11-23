# docs-site

A small Rust program that turns `../docs/` into a static docs website with a
design showcase. Local only.

## Use

Run from this crate (single job keeps memory low on this Mac):

- `cargo run -j1 -- check` — check the docs: front matter, title vs heading,
  page listed in its folder README, broken links, likely secrets.
- `cargo run -j1 -- build` — render `../docs/` to `out/`.
- `cargo run -j1 -- serve [PORT]` — serve `out/` on `http://127.0.0.1` (default 3000).

## How it works

It reads `../docs/` in place; nothing is copied. Each folder's `README.md`
becomes a section `index.html`; the root `README.md` is the home page.
Relative `.md` links inside `docs/` are rewritten to `.html`. Links that escape
`docs/` (runtime files like `server/plugins/.../guide.md`) are left as-is.

The theme uses the design tokens from `../docs/design/tokens.md`: light/dark
(follows the system, with a manual switch), compact spacing, phone layout.
Inter and Lucide icons are bundled, so there are no web requests at runtime.

Each `docs/design/` page gets a live showcase appended by the builder: colour
swatches in light and dark, the type scale, spacing and corners, motion demos
(with a play button, and still under reduced motion), the icon grid, and live
buttons, bubbles, cards, a sheet and a toast. The `.md` pages stay plain.

## Tests

`cargo test -j1` runs golden tests for each check, plus the front matter and
secret scanners.
