---
title: phone
description: Do things on the user's phone - open the dialer or a ready-to-send text, open an app, set a timer or a clock-app alarm, start directions.
order: 11
---

# phone

Do things on the user's phone - open the dialer or a ready-to-send text, open an app, set a timer or a clock-app alarm, start directions.

- Backend: `core` (src/phone.rs; the phone app fetches each action and opens the screen for it)
- Needs: nothing outside the core
- Quick-command words: ["call", "dial", "sms", "timer", "directions", "navigate"]
- Guide for the model: [guide.md](../../server/plugins/phone/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/phone/plugin.yaml)
- More: [Talking to the server](../android/talking-to-server.md)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `call` | sends | yes |
| `text` | sends | yes |
| `open_app` | writes | no |
| `set_timer` | writes | no |
| `set_phone_alarm` | writes | no |
| `navigate_to` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Done by the Android app (`src/phone.rs`, queue `phone_actions`). Only opens screens: the dialer or a filled-in text, the user presses call or send. Contact names are looked up on the phone. App in the background: a notification to tap. No answer in 20 s: `sent_to_phone`.

Things done on the user's phone by the Android app (`src/phone.rs`, table `phone_actions`). Tools:

`call {to}` and `text {to, message}` (`ask`, tag `sends`), `open_app {name}`, `set_timer {seconds|minutes|hours, label?}`, `set_phone_alarm {time, days?, label?}`, `navigate_to {destination, mode?}` (`auto`).

Every action only opens a screen: the dialer with the number (`ACTION_DIAL`), the messages app with the words (`ACTION_SENDTO`), an app's launch screen, the clock app's timer or alarm, or maps (`google.navigation:` when present, else `geo:`).

Nothing is dialled or sent silently. The core checks the arguments; contact names become numbers on the phone (`READ_CONTACTS`), so the server never holds the address book.

The queue: a call adds a `pending` row, emits `phone.action` on the chat (the page and the app's event stream both hear it) and waits up to 20 s.

The app takes rows with `GET /v1/phone/actions` (each handed out once, marked `sent`; `pending` rows older than 2 minutes are dropped as `expired`) and answers `POST /v1/phone/actions/{id} {ok, error?, shown?: screen|notification, contact?}`.

No answer in time: the result says `sent_to_phone: true, done: false`. When the app is not in front (and may not draw over other apps) Android forbids opening screens, so the action waits as a high-priority notification to tap (`shown: notification`).

Shared and photo text: the app is a share-sheet target (`ACTION_SEND` text and images) and has a camera button. Images are read on the phone with the bundled ML Kit text model; no image bytes reach the server.

The text goes into the message box after a marker line, `[photo text]` or `[shared text]`.

The core splits a message at the marker: the words before it are the user's (`user` trust), the rest is a separate `outside` message wrapped in `<outside source="phone:shared">`, so memory saved from it gets outside trust.

Quick-command words are read only from the user's own part.

- Photo and shared text ([Plugins](../architecture/plugins.md) `phone`): the app reads photos on the phone (ML Kit, bundled) and takes shares; text after a `[photo text]` or `[shared text]` line in a message becomes a separate `outside` message. No image bytes reach the server.
