---
title: Writing docs
description: Format, front matter, style, where pages go, and keeping docs in sync with code.
order: 8
---

# Writing docs

The docs are the design. If the docs and the code disagree, one of them is wrong.

## Format

Every page is a markdown file that starts with front matter:

```
---
title: Short page name
description: One line that says what the page holds.
order: 3
---

# Short page name
```

- `title`: the page name. Same as the `#` heading.
- `description`: one line. It shows in the folder index and on the docs site.
- `order`: where the page sits in its folder. `README.md` sets the folder's place among folders.

## Where pages go

| Folder | Holds |
|---|---|
| `overview/` | What AssistX is, principles, glossary |
| `getting-started/` | Run and set up |
| `architecture/` | How each server part works |
| `agents/` | How the agent works end to end |
| `android/` | The phone app |
| `plugins/` | One page per shipped plugin |
| `security/` | Threat model, safety rules, reporting |
| `contributing/` | Setup, code rules, change flow |
| `quality/` | What a good change looks like |
| `plans/` | Specs for work not built yet |
| `roadmap/` | Done, next, open questions |

- Each folder has a `README.md` that lists every page with its description.
- A new page goes in the folder's `README.md` and in [the main index](../README.md).
- One topic per page. If a page grows two topics, split it.

## Style

- Plain, short words. Short sentences. One idea each.
- Say what is, not what might be. Mark plans as plans: `Status: plan, not built`.
- Tables for facts that line up. Lists for steps.
- Name the file and function: `src/audit.rs`, `audit::event`.
- Link with relative paths, like `../security/safety-rules.md`.
- Link runtime files (`server/prompts/`, `server/plugins/*/guide.md`). Never copy them.
- No secrets, tokens, private addresses or personal data. No real chats.

## Keep docs in sync

- **Same change.** Code and its docs change together. A change that makes a page wrong fixes that page.
- Update [Done](../roadmap/done.md) or [Next](../roadmap/next.md) when a feature lands or a gap is found.
- If you find a page that is wrong and you cannot fix it now, add it to [Open questions](../roadmap/open-questions.md).
- A plan in `plans/` becomes an architecture page once it is built. Then the plan links to it.

## Check

- Front matter is there and the `title` matches the heading.
- Every link works.
- The folder index lists the page.
