---
title: email
description: Your Gmail - recent and unread mail, search, read one mail, a short inbox summary, drafts and replies; sending always asks first.
order: 3
---

# email

Your Gmail - recent and unread mail, search, read one mail, a short inbox summary, drafts and replies; sending always asks first.

- Backend: `core` (src/email.rs; the core makes the calls and keeps the token)
- Needs: `network`: [gmail.googleapis.com]; `oauth`: gmail
- Quick-command words: [email, emails, e-mail, gmail, inbox, unread mail, my mail]
- Guide for the model: [guide.md](../../server/plugins/email/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/email/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `inbox` | reads | no |
| `search` | reads | no |
| `read` | reads | no |
| `summarize_inbox` | reads | no |
| `draft` | writes | no |
| `reply_draft` | writes | no |
| `send` | sends, always_ask | yes |
| `delete_draft` | deletes | yes |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Your Gmail through Gmail's API (`gmail.googleapis.com` only).

Sign in under Settings, Integrations (provider `gmail`: Google Cloud Client ID + Secret, Gmail API enabled, scopes gmail.readonly, gmail.compose, gmail.send; [Plugins](../architecture/plugins.md) Email). Everything read is outside content.

`send` is tagged `always_ask`: a high-risk card with the exact To/Cc/Subject/text, never auto, never by voice.

The `email` plugin runs in the core (`src/email.rs`) and talks only to `gmail.googleapis.com` (`AGENT_GMAIL_API` for tests). Sign-in is the `gmail` OAuth provider: Google, offline access, Client ID and Client Secret, its own `GMAIL_*` secrets (separate from Google Health's `GOOGLE_*`, so each signs in and out alone). Scopes: `gmail.readonly`, `gmail.compose` (drafts), `gmail.send`.

Tools: `inbox`, `search` (Gmail query words), `read` (one mail; text/plain part, else HTML made plain; attachments by name only), `summarize_inbox` (the core gathers sender, subject, date and snippet; the model writes the summary), `draft` and `reply_draft` (auto; undo deletes the draft), `send` (`sends` + `always_ask`).

Safety, in code:
- Every result is outside content (`<outside>`, any tag inside broken), so a turn that read mail is an outside turn: jobs it makes never ring, `notify` cannot be urgent, and so on.
- `send` is always a `high` risk card, whatever the settings say (`PATCH /v1/config` refuses `auto`). The card question is built by the core from the real arguments: To, Cc, Subject and the first 1500 characters of the text, plus "Sent mail cannot be taken back". `details.email` carries the same fields for the page. Voice cannot approve it.
- Addresses and the subject are checked; a newline anywhere in a header is refused (no header injection). At most 20 recipients per field.

Setup (one time):
1. In the Google Cloud console, open the project (the Google Health one works), go to APIs and Services, Library, and enable the **Gmail API**.
2. OAuth consent screen, Data access: add the scopes `.../auth/gmail.readonly`, `.../auth/gmail.compose`, `.../auth/gmail.send`. While the app is in testing, add your own Google account as a test user.
3. Credentials: use an OAuth client (or make one) and add the redirect address shown in Settings, Integrations, Email.
4. In the app: Settings, Integrations, Email: paste the Client ID and Client Secret, Connect Gmail, allow every box. If Google Health was set up before the Gmail scopes existed, this sign-in is separate; connecting again later asks for consent again (`prompt=consent`) so Google sends a fresh refresh token.
