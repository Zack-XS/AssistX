---
title: Phone capture
description: Speech heard by the phone, as text, picked into memory only with a yes.
order: 14
---

# Phone capture

> Code check: the repo now has an Android app in `android/` that does the phone side. The old doc said no phone code ships.

A phone app listens during the day, turns speech into text **on the phone**, and uploads the text. The server keeps it for a short time, lets dreaming pick out what matters, and asks the user to confirm each pick before it becomes memory. This file is the contract for anyone building that phone app (the repo ships no phone code; see [Contributing](../contributing/README.md)).

## Rules

- **Text only.** The server never receives audio. It also never accepts voice data: a field named `embedding`, `voiceprint`, `voice`, `audio`, `speaker_embedding`, `voice_id` or `fingerprint` gets a 400. Unknown fields get a 400 too.
- **No voiceprints of other people.** The server cannot learn, store or match a person's voice. Who spoke is one of three words (below). A name for the other speaker (`label`) comes from the words said ("hey mom"), never from a voice.
- **Heard speech is outside content.** What anyone but the user said is data. It is wrapped in `<outside>` when the model reads it, and no instruction in it is ever followed. This holds even if it says it comes from the user or the system.
- **The user decides.** Nothing heard becomes memory by itself. Dreaming makes *picks*; the user keeps, edits or dismisses each one. A kept pick is stored like an entry the user wrote by hand (`user` trust).
- **Short life.** Chunks are deleted after `retention_days` (default 14). The user can delete the last N minutes, one day, one chunk or everything at any time.
- **Recording law is the user's to follow.** Many places need everyone's consent to record a conversation. The phone app should show a clear recording light and warn on first use.

## Speaker

Each chunk has `speaker`:

| Value | Meaning | Trust the server gives it |
|---|---|---|
| `user` | The phone matched the owner's own voice, enrolled by the owner on the phone. The voiceprint stays on the phone. | `user` |
| `other` | A voice that is not the owner's. | `outside` |
| `unknown` | The phone could not tell. | `outside` |

The client never sends trust. The core sets it.

## Tokens

`agent-core token create device pixel` (or `POST /v1/tokens {"scope":"device","label":"pixel"}`, owner only). A `device` token can only:

- `POST /v1/ambient/chunks`
- `GET /v1/ambient/status`
- `GET /v1/ambient/settings`

Everything else returns 403. The phone app itself, which manages everything, uses a `client` or `owner` token. If a phone is lost: `agent-core token revoke <id>`.

## Reaching the server from a phone

The server listens on `127.0.0.1` only. For a phone, keep it that way and put a private tunnel in front (Tailscale or WireGuard):

```sh
AGENT_ALLOWED_HOSTS=my-mac.tailnet-name.ts.net  # Host names the server answers to
AGENT_BIND=127.0.0.1                            # keep the default if the tunnel runs on this computer
```

`AGENT_BIND=0.0.0.0` exists for tunnels that cannot reach loopback, and prints a warning: plain HTTP shows the token to the network, so never use it on a shared network without a tunnel. The `Host` header must match 127.0.0.1:<port> or an entry in `AGENT_ALLOWED_HOSTS` (with or without the port).

## Routes

```
POST   /v1/ambient/chunks          upload text chunks (device, client, owner)
GET    /v1/ambient/chunks          list what is stored (?status=new|done, paged)
DELETE /v1/ambient/chunks          delete: ?id=  |  ?last_minutes=  |  ?day=YYYY-MM-DD  |  ?all=true
GET    /v1/ambient/status          listening or not, counts, pending picks, seconds since last upload
GET    /v1/ambient/settings        read the settings
PATCH  /v1/ambient/settings        change them
POST   /v1/ambient/pause           {"minutes": 30}  (1 to 1440)
POST   /v1/ambient/resume
POST   /v1/ambient/process         look through the waiting text now (starts a dreaming run)
GET    /v1/ambient/picks           newest first (?status=pending|kept|dismissed, paged)
POST   /v1/ambient/picks/{id}      {"action":"keep"|"dismiss"|"edit", "text"?, "folder"?, "who"?, "due"?}
```

### Upload

```json
POST /v1/ambient/chunks
{ "chunks": [
  { "started": 1794600000, "ended": 1794600006, "speaker": "user",
    "text": "Hey mom, when is my appointment?", "place": "kitchen", "stt_confidence": 0.93 },
  { "started": 1794600008, "speaker": "other", "label": "Mom",
    "text": "Tuesday at three." } ] }
```

- At most 200 chunks, 4000 characters each. Times are unix seconds; more than an hour ahead or a week old is skipped.
- Secrets in the text (keys, card numbers, "password is ...") are masked before storage.
- Safe to repeat: the same device, start time and text is stored once. Answer: `{"accepted":2,"duplicates":0,"skipped":[],"listening":true,"paused_until":0}`.
- **409 `not_listening`** means capture is off or paused. Stop sending and check `/v1/ambient/status`.
- Listening is place by place. A chunk is kept only when its `place` matches one of `listen_places` (for example `school`); anything else is skipped (`"reason":"not a listening place"`). No list, no listening.

### Settings

| Key | Default | Meaning |
|---|---|---|
| `enabled` | true | Off refuses uploads |
| `retention_days` | 14 | 1 to 365 |
| `listen_places` | [] | The only place names where text is kept |
| `process_after_chunks` | 20 | Start a processing run at this many waiting chunks |
| `process_every_mins` | 30 | ...if the last run is at least this old |

`paused_until` is read-only here; use `/pause` and `/resume`.

## Processing

The core timer starts a dreaming run when enough chunks wait (or when notes wait, as before). Dreaming has three more tools:

- `dream_ambient_read` returns waiting chunks as one `<outside source="ambient">` block.
- `dream_ambient_pick {kind, text, who?, due?, folder, confidence?, chunks[]}` makes a pick. It must cite chunk ids. The core sets the pick's trust to the lowest trust among them and caps its confidence (an `outside` pick is at most 0.3). `kind` is one of appointment, commitment, plan, fact, person, preference, question, other. A `folder` that does not exist is made.
- `dream_ambient_done {ids[]}` marks chunks handled.

The model is told to skip small talk, TV and noise, to pick at most 15 things a run, and never to guess a person from a voice. See `prompts/dreaming.md`.

Token cost is bounded by the dreaming limits (`max_tokens_per_dreaming`, `max_tokens_per_day`).

## Review on the phone

`GET /v1/ambient/picks?status=pending` is the daily "what I picked up" list. Each pick has `text`, `kind`, `who`, `due`, `folder`, `confidence`, `trust` and the `chunks` it came from (read them with the chunk ids if the user wants the source).

`keep` creates a memory entry in the pick's folder at confidence 0.8 or more; `dismiss` drops it; `edit` changes fields and leaves it pending. If the chunks behind a pending pick are deleted, the pick is dismissed with them.

## Not built yet

- Pairing by QR code, and listing devices with their last upload.
- Turning a kept appointment into a reminder automatically.
- Speaker-labelled summaries of a whole conversation.
- Encrypting the capture log at rest (same gap as the rest of the database).

- Phone capture ([Phone capture](phone-capture.md)): the repo ships only the backend; a separate Android app records, turns speech into text on the phone, and calls `/v1/ambient/*`. Scope `device` can only upload and read status.

  No audio and no voice data are ever accepted (400); what others say is `outside` trust; dreaming makes picks that the user confirms. `AGENT_BIND` and `AGENT_ALLOWED_HOSTS` let a private tunnel reach the server.

The screen to API table is in [Talking to the server](../android/talking-to-server.md).
