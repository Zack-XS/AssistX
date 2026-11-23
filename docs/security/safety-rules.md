---
title: Safety rules
description: The rules every change must keep: core, plugins, page and app.
order: 2
---

# Safety rules

These rules come from the principles, the repo's hard rules, the threat model, and a review of the app on 2026-11-22. This page is the one home for safety rules; `server/docs/17-safe-code.md` is folded in here.

## Core rules

1. Safety lives in code. Prompts may remind the model; the core enforces.
2. Outside content is data. Tool output, files, web pages, trigger bodies, outside guides and outside-trust memory are wrapped in `<outside>`; any tag inside is broken.
3. Risky tags (`sends`, `spends`, `deletes`, `shares`) are at least `ask`. `unrestricted` and `always_ask` are never `auto`.
4. Approval cards are built from the real call, and a yes is bound to a hash of it.
5. The core sets trust labels. The model cannot.
6. Local first: bind `127.0.0.1`, token on every route but `/v1/health`, `Host` must match, `Origin` must be allowlisted.
7. Secrets are never shown to the model, and are scrubbed from logs, reports and output.
8. Outbound calls go through the proxy, or to one fixed host per service. Local services (`AGENT_TTS_URL`, `AGENT_EMBED_URL`, `AGENT_LUMO_URL`) must be loopback.

## Front-end rules

Front ends are not ours, but the API docs will ask them to:

1. Render model text as safe markdown: no raw HTML, no scripts.
2. Do not auto-load remote images or links in model text. Show them as links the user taps.
3. Show approval `question` and `details` from the core. Show the model's reason apart from them, with a label.
4. Store tokens in the platform's secure storage.
5. Use a `client` token by default. Ask for the owner token only for owner actions.

## Where each rule lives in code

This is the "how to keep the rules true" layer, folded from `server/docs/17-safe-code.md`; this page wins. Threat ids (T1-T15) are in [Threat model](threat-model.md). Paths are from `server/` unless they start with `android/`. Golden tests are in `tests/golden.rs`; unit tests sit at the end of the named file. Line numbers drift; search for the name.

| Rule | Threat | Enforced in | Proved by |
|---|---|---|---|
| Outside content is wrapped; tags inside are broken | T1, T5 | `src/util.rs` `wrap_outside`, `neutralize_outside_tags` (:70-93) | `util::outside_cannot_be_closed_early`; `auto_tool_runs_and_output_is_outside_content`; `email::mail_text_is_outside_content_and_cannot_close_the_wrapper` |
| A turn that read outside content stays marked | T1, T15 | `src/engine.rs` (`force_ask`, :1079) | `job_made_after_outside_content_is_marked_outside`; `an_outside_recipe_asks_before_every_step` |
| `sends`/`spends`/`deletes`/`shares` are at least `ask` | T1, T4 | `src/gate.rs` `effective` (:62), `FLOOR_TAGS` (:46) | `gate::floor_tags_force_ask`; `owner_can_loosen_autonomy_per_tool` |
| `always_ask` / `unrestricted` never become `auto` | T1, T4 | `src/gate.rs` `always_asks` (:54) | `gate::unrestricted_always_asks_whatever_the_settings_say`; `email_reads_as_outside_content_drafts_alone_and_send_always_asks` |
| Risk comes from tags, never from the model | T8 | `src/gate.rs` `risk` (:79) | `gate::risk_from_tags`; `gate::no_tags_or_unknown_tool_is_ask` |
| A yes is bound to tool, args and version | T8 | `src/gate.rs` `call_hash` (:95) | `gate::hash_changes_with_args_and_version`; `edit_changes_args_before_running`; `late_yes_runs_exact_call_and_starts_follow_up` |
| Voice never answers high risk | T8 | `src/engine.rs:1562` (`voice_ok`) | `voice_may_answer_questions_and_low_risk_calls_but_never_high_risk` |
| Token, `Host`, `Origin` on every route | T6, T7 | `src/api.rs` `guard` (:178), `src/config.rs` `host_ok` | `auth_host_origin_and_scopes`; `config::host_header_must_be_local_or_listed`; `a_listed_host_name_is_accepted_and_others_are_not` |
| Narrow tokens reach only their routes | T6, T7 | `src/api.rs` `guard` (:211-226) | `trigger_tokens_reach_only_their_source_and_bodies_are_outside`; `a_device_token_uploads_text_and_can_do_nothing_else` |
| Secrets scrubbed from logs, audit, reports | T9 | `src/util.rs` `scrub` (:104); callers in `src/engine.rs:72`, `src/audit.rs:39-55`, `src/eval.rs:402` | `util::scrubs_common_secrets`; `util::card_numbers_and_spoken_passwords_are_masked` |
| Secrets listed and exported by name only | T9 | `src/export.rs` (`secret_names`, :123) | `export_zip_and_secrets_by_name_only` |
| Plugin network only to `needs` hosts, never private | T2, T3, T13 | `src/proxy.rs` (:1-6, `resolve_public` :75) | `proxy::allow_rules`; `proxy::refuses_loopback_and_unlisted` |
| Sandbox hides data, plugin folder read-only | T3, T4, T13 | `src/sandbox.rs`, `src/runner.rs` | `sandbox::bwrap_hides_data_and_keeps_narrow_binds_last`; `linux_sandbox_hides_the_data_folder_and_keeps_plugins_read_only` |
| Plugin edits on disk are caught | T3, T4 | `src/store.rs` (hash), `src/runner.rs` | `build_mode_draft_approval_and_tamper_check` |
| Local services are loopback only | T2 | `src/api.rs` `tts_base_of` (:1064), `src/embed.rs` `loopback` (:96), `src/lumo.rs` | `embed::only_loopback_addresses`; `lumo::only_loopback_bridges`; `speech_goes_to_the_local_kokoro_voice_only` |
| Ids cannot change a fixed host's path | T2 | `src/email.rs:86`, `src/hevy.rs`, `src/spotify.rs` `valid_uri` | `hevy::ids_cannot_change_the_path`; `spotify::only_real_spotify_uris_pass` |
| Outside content cannot make a notice urgent | T1, T15 | `src/engine.rs` `notify` | `notify_sends_normal_and_silent_notices_and_outside_content_cannot_make_one_urgent` |
| Outside facts never reach core facts | T5 | `src/dreaming.rs`, `src/memory.rs` | `outside_facts_never_reach_core_and_dry_run_changes_nothing` |
| Loops and spend stop | T11 | `src/engine.rs` budgets | `tool_call_budget_stops_loops`; `only_cloud_tokens_count_toward_the_daily_cap` |
| No audio or voice data stored | Phone capture | `src/api.rs` ambient routes | `voice_data_is_refused_and_unknown_fields_are_errors` |
| No `unsafe` | T12 | `src/lib.rs:2` `#![forbid(unsafe_code)]` | the build |


A new rule gets a row here, a place in code and a test, in the same change.

## Rust core

### Outside content

Anything the user did not type is outside content: tool output, mail, web pages, files, trigger bodies, notifications, shared text, heard speech, outside-trust memory, outside guides. Wrap it once, where it enters a prompt.

Do:

```rust
format!("[core] An outside event arrived from '{src}'. ... The event:\n{}", util::wrap_outside(&format!("trigger:{src}"), body))
```

Don't:

```rust
format!("<outside>{body}</outside>")       // a body with </outside> closes it early
format!("The user's mail says: {body}")     // not marked at all
```

- Use `util::wrap_outside`. Never build the tag by hand.
- Put your own `[core]` lines above the block, not inside it.
- If the turn read outside content, set its trust to `outside` (`force_ask` in `src/engine.rs`). Jobs and recipes made in that turn stay `outside`.
- Guides of plugins with an `.outside` marker are wrapped too (`src/prompt.rs`).

### A new tool

Every tool, core or plugin, goes through the gate. The gate reads tags, never model text.

| Field | Rule |
|---|---|
| `tags` | Honest. Anything that leaves the box is `sends` or `shares`. Money is `spends`. Removal is `deletes`. No tags = `medium` risk and `ask`. |
| `always_ask` | On every send, spend and delete the user must see each time. |
| `autonomy` | A request only. User config and the core floor win. |
| `undo` | Name the undo tool, or leave it out. Never fake an undo. |
| Card text | Built by the core from the real call (`gate::question`). The model's reason is shown apart. |

Don't: tag a send `writes` to skip the card; run a core tool outside `engine`; build card text from model output.

### A new route

`guard` in `src/api.rs` runs for every route. It checks `Host`, `Origin` and the token, and keeps narrow tokens narrow. Your handler still checks scope.

- Owner actions call `need_owner` first. Anything that lowers safety (autonomy, plugin status, undo, export, tokens) is owner only.
- `trigger`, `device` and `node` tokens are allowlists in `guard`. A new route is closed to them. Open it only if needed, with a test that the others still get 403.
- Read input with typed bodies or checked fields. Unknown fields that matter are errors.
- Tokens come in the `Authorization` header only, never in a URL.
- Errors go out as `ApiError` with a plain message. Never echo secrets or raw input.

### Secrets

- Read secrets by name from the vault. The model never sees a value.
- Every write to the raw log, audit, action log, mind journal and reports goes through `util::scrub`. A new store of text does too.
- Never print a token, secret, `Authorization` header or full request body.
- Export lists secret names only. Events and SSE never carry a secret.

### Outbound network

1. **Plugin code:** only through the per-run proxy (`src/proxy.rs`). It refuses private, loopback and link-local answers. Hosts come from `needs.network`.
2. **A core plugin with one service:** one fixed base from config, never from the model (`spotify_base`, `gmail_base` in `src/config.rs`). Check every path part from the model first.
3. **A local helper** (voice, embedder, model bridge): loopback `http` only, checked at start.

Don't: `format!("{base}/{}", args["path"])`; fetch a URL taken from model or tool output; accept `http://localhost.evil.com` (check the host exactly).

### Input, SQL, crates

- **No `unwrap` or `expect` on input.** Use `?`, `unwrap_or`, or a 400. Fine in tests and on values the code just made.
- **SQL only with bound params.** `format!` may add only fixed column lists or table names that are code constants. Never a value.
- **Crates:** few, exact pin, reason beside it in `Cargo.toml`. A new crate needs a line saying why nothing current can do it. `cargo-deny` and `cargo-audit` run in CI.
- **No `unsafe`.** `src/lib.rs` has `#![forbid(unsafe_code)]`. Keep it.

## Plugins

Format rules: [Plugin format](../architecture/plugins.md).

- **`needs` minimal.** Name each host. `*public*` only for a web reader.
- **Tags honest**, and `always_ask` on every send, spend and delete.
- **`.outside` marker** on plugins built from outside input or from others. Their guide is wrapped and quick words never load them.
- **Script protocol:** stdin `{"tool","args"}`, stdout `{"ok":true,"result":...}` or `{"ok":false,"error":...}`. Never print secrets.
- **Tests:** `tests/*.json` per tool, plus one bad input. A shipped plugin also gets a golden test.
- **Guide** says what each tool does. It never tells the model to use another plugin's tools.

## Page

The page is `server/examples/dummy-client/index.html`, shown in the app's WebView. How it looks: [Screens](../design/screens.md). What it must not do: here.

**Why it matters.** `NativeApp.getConfig()` hands the decrypted token to the page's script. Any script in the page can read it and call every bridge method. So any XSS is full account theft.

- Build DOM from data with `textContent` only (the `h()` helper). `innerHTML` only with fixed strings in the file (`ICONS`, the ring SVG), each with a comment.
- No `eval`, `new Function`, string `setTimeout`, or `on*` attributes built from data.
- `style` in `h()` takes fixed strings only. `href` only from a checked `https?://` match, never `javascript:`.
- **Model text** is outside content. Draw it with `md()`, which uses `textContent` only. Only `[label](https://...)` becomes a link, with `rel="noopener noreferrer"`. Nothing opens by itself.
- **No images from model text, ever.** An image URL can leak data when it loads (T2).
- Approval cards show the core's `question` and `details`, with the model's reason apart and labelled.
- **No outside code:** no `<script src>`, CDN, web fonts or remote CSS. No `<iframe>`, `<object>` or `<embed>` (the bridge is visible to every frame). No `postMessage` listeners.
- Advice: add a CSP `<meta>`: `default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'unsafe-inline'; connect-src http: https:; img-src data:`.
- **Storage:** the token comes from `getConfig()`; after a sign-in it is also in `sessionStorage`. Never put it in `localStorage`, which holds view choices only.

## Android

The app is `android/`. Screens are listed in [App structure](../android/app-structure.md).

- **Bridge methods** take strings from page script. Check, cap and allowlist every argument (a plain chat id regex, `OPEN_VIEWS`, https only for `openUrl`). Cap text before it leaves (`take(120)`, `take(20000)`). Quote strings passed to `js(...)` with `JSONObject.quote`.
- Don't open any URL, file path or intent the page names. Don't add a method that returns a secret. Prefer a parsed `scheme` and `host` over `startsWith`.
- A new bridge method goes in the [frozen list](../android/app-structure.md#bridge-contract-frozen), on both sides.
- **Keep the page on its host:** served only from `https://appassets.androidplatform.net`; other links go to the browser. File and content access and multiple windows stay off.
- Mixed content is on because the server is plain `http`, so the token crosses the network in clear text. Use loopback or a private tunnel ([Deploy](../architecture/deploy.md)).
- **Token:** sealed with a Keystore AES key (`Vault.kt`). Never in a log, notification, intent extra or URL. Debug intents that set the server or token work only in debuggable builds.
- **Exported parts:** any app can send to them, so every extra and `data` URI is untrusted. `app.assistant://` links carry only `code`, `state`, `error`. Shared text and photo text go to the server as `outside`. A new exported part needs a reason and the narrowest filter.
- **Notifications** from other apps are outside content and reach the server only as small detected items, never codes or sign-in messages.
- **No Google services.** Notices come by polling. ML Kit is bundled and runs on the phone.

## Every change ships with a test

| Change | Test |
|---|---|
| Gate, tags, risk, hash | Unit test in `src/gate.rs` + a golden test that a fooled model still gets a card |
| New route | Golden test: no token 401, wrong scope 403, bad `Host`/`Origin` 403, happy path |
| New source of outside content | Golden test that the model sees it in one `<outside>` block, with a fake closing tag in the input |
| New outbound call | Unit test that bad hosts, private addresses and path tricks are refused |
| New store of text | Test that a known secret is scrubbed from it |
| New plugin | `tests/*.json` per tool + a golden test for its tags and card |
| Page | [Test a page change](../design/screens.md#test-a-page-change) |
| Android bridge or intent | A bad-input case in the PR notes until the app has unit tests |

## Before you say done

1. Rules table: if a row changes, update the row, the code and the test together.
2. Outside text wrapped with `wrap_outside`; turn marked `outside`.
3. New tool: honest tags, `always_ask` where needed, real undo or none.
4. New route: owner check where needed; closed to `trigger`, `device`, `node` unless listed.
5. No secret in a log, event, error, report, export, URL or notification.
6. Outbound: proxy, one fixed host, or loopback only. Path parts checked.
7. No `unwrap` on input; bound SQL params; no new crate without a pin and a reason.
8. Page: no `innerHTML` with data, no remote loads, no images from model text, no iframes.
9. Android: every bridge argument checked; page stays on its host; token only in the Vault.
10. Tests added, `cargo test -j1` passes, and no product name added to code.

## Known gaps

Today's code does not meet these yet. Fix them, then delete the line.

- The page has no CSP `<meta>`.
- `md()` links allow plain `http://` as well as `https://`.
- `openUrl` checks with `startsWith("https://")`, not a parsed scheme and host.
- The `power_off` error in `guard` hard-codes a product name.
- There are no page or Android tests.

More tests and checks live in [Testing](../contributing/testing.md).
