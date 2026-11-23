# Docs review: `docs/` tree (2026-11-22)

Reviewer: kirocrew-worker (item it_c726b404). Read-only review. No docs were edited and nothing was built.

**Verdict: Not ready yet. The tree is well laid out and mostly matches the code. But it competes with two newer old-style docs (16, 17), it breaks two hard rules it copies (all-Rust, no branding), the activity-log plan has privacy holes, and agents are never sent to the new tree.**

Mechanical checks, all 98 pages: every page has front matter (`title`, `description`, `order`). Every `title` matches the `#` heading. Every folder has a `README.md` that lists all its pages. No broken relative links. No real emails, keys, tokens or private IPs found. Only `example.com` and `<server-ip>`-style placeholders appear.

Note: `docs/design/tokens.md` was changed at 16:50 today, during this review. Someone may still be editing it.

## Findings

1. **High - two design specs and two safe-code homes now compete.**
   Files: `server/docs/16-app-design.md`, `server/docs/17-safe-code.md` vs `docs/design/*.md`, `docs/security/safety-rules.md`, `docs/MIGRATION.md`.
   What is wrong: 16 and 17 were written at 14:39 and 14:42. That is after `MIGRATION.md` (14:34). They are not in the migration table. `safety-rules.md:34` still says the safe-code doc "is not written yet", and `roadmap/next.md:57` treats both as future. The two design sets disagree on real values:
   | Thing | 16-app-design | docs/design |
   |---|---|---|
   | Accent | none | `accent` #4B57D6 / #9AA3FF |
   | Light `mute` / `ok` / `warn` / `bad` | #5e5e68 / #157a42 / #8a5d00 / **#b8291f** | #5F5F69 / #177F45 / #8A6100 / **#D1342F (unchanged)** |
   | Space | 4 8 12 16 20 24 32 | 2 4 6 8 12 16 20 24 32 48 |
   | Radius | 8 12 16 22 999 | 6 10 14 20 22 30 999 |
   | Type sizes | adds 18 and 44 | 17 instead of 18; no 44 |
   | Icon size | 24 | 22 (`icon-md`) |
   | Tabs | Today, Plan, Chat, You | "Home, Plan, Assistant" (`typography.md`) |
   Fix: pick one source. Best: fold 16 (information map, page contract, bridge list, `localStorage` keys, screen states) into `docs/design/` and `docs/android/app-structure.md`. Fold 17 into `docs/security/safety-rules.md` and `docs/quality/safety.md`. Then add both to `MIGRATION.md` and settle each value in one tokens table.

2. **High - the docs-site plan breaks hard rule 1 (all Rust).**
   File: `docs/plans/docs-site.md`, vs `docs/contributing/code-style.md` rule 1 and `server/AGENTS.md` rule 1 ("No Python, Node or other runtimes in this repo").
   What is wrong: it picks VitePress (Node, `package.json`, TypeScript `sidebar.ts`, Vue components) in `site/` at the repo root. The plan never names this clash. The `tokens.json` build script in `design-language-rollout.md` step 1 has the same problem if it is not Rust.
   Fix: either (a) make a written exception for `site/` and `design-tokens/` in rule 1 and open-questions, or (b) choose a Rust tool (mdBook with a custom theme, or Zola), or a small Rust build step. Also state the generator's language for `tokens.json`.

3. **High - agents are never sent to the new tree.**
   File: `server/AGENTS.md:5`. There is no root `AGENTS.md`.
   What is wrong: `AGENTS.md` is the file agents read. It still says "The design lives in `docs/`. It wins" and means `server/docs/00-17`. It does not mention `docs/quality/logging.md` (the "every change adds logs" rule), `docs/quality/android-battery.md` or `docs/quality/agents-checklist.md`. So the owner's new rules do not reach agents. `agents-checklist.md:9` defers to `server/AGENTS.md`, which goes in a circle.
   Fix: add a root `AGENTS.md`, or update `server/AGENTS.md`, to point at `docs/README.md`. Add rule 9, "Every change adds logs (`docs/quality/logging.md`)". Add rule 10 for the Android battery rules. Say which tree wins until the old one is retired.

4. **High - the activity log escapes "Forget" and the location limits.**
   File: `docs/plans/full-activity-log.md` (Storage, Privacy, Event format), vs `docs/architecture/memory.md` "Forget" and `server/src/location.rs:9` (`AGENT_LOCATION_KEEP_DAYS` 90).
   What is wrong:
   - The Forget sweep (`memory.md`, steps 1-9) does not include the `activity` table, the `before`/`after` copies of data changes, or the new `model_calls.thinking`. So a forgotten fact lives on in the log.
   - Every event's `ctx` carries exact `lat`/`lon`, and `keep_days` can be up to 365. That goes past the 90-day location limit and also ignores `parts.location: false`.
   - "Before and after for edits" copies whole texts. That breaks the plan's own rule, "Big texts are not copied".
   Fix: add `activity` and `thinking` to the Forget sweep, with a test. Store only a place id in `ctx` (no coordinates), or cap location-bearing events at the location limit. Define exactly what each `parts.*` switch drops. Store edits as a `ref` plus a field list, not full copies.

5. **Medium - more activity-log privacy and safety gaps.**
   File: `docs/plans/full-activity-log.md`.
   - "Turning it off stops all capture at once" conflicts with "turns off everywhere within one sync" (up to 15 minutes). Fix: also push the switch over the live `NoticeStream`. Also have the server drop batches whose events are newer than `since`/the switch-off time.
   - A device token can `POST /v1/activity/batch`. A stolen or buggy phone can then plant text that the review agent reads. Fix: treat all phone-sent `detail` as `outside` and wrap it, and cap kinds and field sizes like `/v1/detected`.
   - "Not sent anywhere else" is not the whole truth. The review step hands the export to an AI agent, which is often a cloud model. Fix: say so on the explain screen, and add a redacted export (no thinking, no location, no chat text).
   - It does not say how phone capture (overheard speech of other people) and `NotifyListener` notification text are treated. Fix: exclude them by default, or name them on the explain screen.
   - The threat model (`docs/security/threat-model.md`) has no entry for the log. Fix: add one before step 1 of the build.

6. **Medium - false contrast claim.**
   File: `docs/design/color.md` Rules ("Text on `page`, `card` and `field` must reach 4.5:1. All text tokens above do.").
   What is wrong, by WCAG math: light `ok` #177F45 on `page` is 4.17:1. Light `bad` #D1342F is 4.09:1 on `page` and 4.43:1 on `field`. `warn` and `ok` on `bubble` are 4.49 and 4.10.
   Fix: darken `ok` and `bad` (16 already proposes #157a42 and #b8291f), or limit state colours to icons and dots on `page`.

7. **Medium - the docs site is only a plan, and its default hosting is public.**
   File: `docs/plans/docs-site.md` (Hosting).
   What is wrong: the owner asked for "a site that reads them", but no site exists yet. The default host is public Cloudflare Pages. The docs map the attack surface: routes, the sandbox escape notes, the 46 bridge calls, the token in `NativeApp.getConfig()`, and `ssh assistantx-server` paths. The step-8 check also needs gitleaks, which is not installed on this Mac, and this folder is not a git repo.
   Fix: default to private (served on the main server over the tailnet, or Cloudflare Access required). Make public an explicit choice. Use a check that runs without gitleaks, or install it as step 0.

8. **Medium - about 64 code files still point at old doc numbers.**
   Files: `server/src/*.rs`, `server/prompts`, `server/plugins` (137 refs such as `docs/12`, `docs/08`). Also `docs/security/safety-rules.md:36` links to `server/docs/*.md` by line number.
   What is wrong: when the old docs retire, these pointers die. `MIGRATION.md` gives no number-to-page map for code.
   Fix: add a "`docs/NN` → new page" table to `MIGRATION.md`. Rewrite the comments when the old tree is removed. Turn the line-number links into section links on new pages.

9. **Medium - code style changed the meaning of rule 3.**
   File: `docs/contributing/code-style.md` rule 3.
   What is wrong: it calls `examples/dummy-client/` "a throwaway page". `server/AGENTS.md` and the whole `docs/design/` tree treat it as the app's real screens, shipped to the phone. That invites careless edits to the live UI.
   Fix: copy the current `AGENTS.md` wording ("optional reference client: the app's screens, shown by the Android app").

10. **Medium - the branding rule is broken all over the new tree.**
    Files: 83 uses of "AssistX" across `docs/` (for example `quality/safety.md`, `design/principles.md`, `plans/docs-site.md`) vs `code-style.md` rule 2.
    What is wrong: it is flagged in `roadmap/open-questions.md:48` and in a note, but the tree goes ahead anyway. `design/*` also does not carry 16's "one display name, default 'Assistant'" rule for screens.
    Fix: the owner decides. Until then, add the display-name rule to `design/principles.md` so screens never hard-code a name.

11. **Low - stale test counts.**
    Files: `docs/getting-started/run-server.md:16`, `docs/contributing/dev-setup.md:14` ("56 unit + 97 golden").
    What is wrong: the code has 121 `#[test]` in `server/src` and 135 in `server/tests/golden.rs`.
    Fix: drop the numbers or update them.

12. **Low - small errors in the battery page.**
    File: `docs/quality/android-battery.md`.
    - "No network checks" is wrong: `Updater.kt:83` waits for Wi-Fi before an auto-update download.
    - The `Thread.sleep` list misses `NoticeStream.kt:33,69`.
    - `NotifyListener`, `PlaceWatch`, `WakeService` details and `Detect` are missing from the table.
    - It does not mention that location slows to 15 minutes when still (`SLOW_MS`).
    Fix: correct these lines.

13. **Low - two descriptions are cut off mid-word.**
    Files: `docs/plugins/scheduler.md:3` ("...to work on its "), `docs/plugins/spotify.md:3` ("...albums, playlist"). The same cut text appears in `docs/README.md`.
    Fix: write full one-line descriptions.

14. **Low - small clashes inside the design pages.**
    - `design/principles.md` says "three weights", but `typography.md` lists four (400/500/600/700).
    - `motion.md` uses 300 ms and 220 ms for subpages, which are not tokens.
    - `typography.md` names the tabs "Home, Plan, Assistant".
    Fix: align them with the tokens and the tab names chosen in finding 1.

15. **Low - style and format.**
    - 70 lines are over 400 characters. The worst pages are `architecture/location.md` (7), `models.md` (5), and `api.md`, `memory.md`, `accounts.md`, `mind.md`, `plugins.md` (4 each). That goes against "short sentences, one idea each" (`quality/writing-docs.md`). Split them into lists or tables.
    - `architecture/multi-device.md` has two status lines ("not started" and "draft v0.1") and no standard `Status: plan, not built`.

## Owner asks vs tree

| Ask | State |
|---|---|
| All docs as .md in folders | Done |
| Site that reads them | Plan only (findings 2, 7) |
| Sections: how it works, plans, contribute, agents, others | Done |
| Design language (fluid, compact, mobile + desktop, smart, motion, icons) | Done on paper; clashes with 16 (finding 1) |
| Quality docs: safety, writing docs, quality bar, battery | Done; not linked from `AGENTS.md` (finding 3) |
| Every change adds logs | Written in `quality/logging.md`; not a hard rule agents see (finding 3) |
| Opt-in full activity log | Plan written; privacy fixes needed (findings 4, 5) |

## Claims spot-checked against code (18)

| Claim (page) | Result |
|---|---|
| Android `colors.xml` has only `page`, `icon_bg` (design) | True |
| Page `:root` has 13 colour vars (tokens) | True (plus `--shadow`) |
| App has no `Log.*` calls (logging) | True |
| Server has no logging crate (logging) | True |
| `audit::event`, `audit::record`, `log_action`, `finish_action`, `util::scrub`, `clean_line` exist | True |
| `ensurePeriodic` uses 15 min `setAndAllowWhileIdle` (battery) | True |
| Exact alarm logic: `setAlarmClock`, else exact, else inexact | True |
| Location: balanced, 5 min, batched 15 min | True |
| HealthSync at most every 10 min | True |
| Capture: 15 s or 5 lines, wait 60 s on 409 | True |
| Ring wake lock = ring + 5 s | True |
| Asks to skip battery optimisation | True |
| "No network checks" | **False** (`Updater.kt:83`) |
| `brief.settings` pattern, `maybe_prune`, `AGENT_TRACE_KEEP_DAYS`, thinking dropped, `/v1/export`, per-account `core.db` (activity log) | True |
| Mind runs every 30 min | True |
| `/v1/detected` caps at 20 | True |
| "56 unit + 97 golden tests" | **False** (121 / 135) |
| "All text tokens reach 4.5:1" | **False** (finding 6) |

Not checked: every route in `architecture/api.md` (105 `.route(` calls vs 11 table rows; it is not clear the page means to list them all), the plugin pages one by one, and the Android screen-to-route table. No builds or tests were run (low memory).
