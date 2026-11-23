---
title: Voice
description: Live mode, spoken answers and the voice screen.
order: 9
---

# Voice

- Live mode (the voice screen): `POST /v1/threads/{id}/messages {text, live: true}`.

  The core adds `prompts/live-mode.md` to that turn (short spoken sentences, no markdown) and gives it the core tool `listen_for_reply`, which sets `listen: true` on `GET /v1/turns/{id}` and emits `live.listen`; the phone then reads the answer out and opens the microphone.

  Outside live mode the tool does not exist. In live mode `ask_user` raises no card and does not wait (a waiting turn would hold up the chat): it sets `listen` and the spoken answer is the next message.

  The voice screen sends `interrupt: true`, so speaking again stops a turn still waiting for a yes; a cancelled wait voids its card. A waiting approval carries `voice_ok`; the phone reads its question out and listens, then answers with `via: "voice"`.

  Voice may answer questions and low or medium risk tool calls; high risk and plugin installs or updates are refused in code (400, "tap") and need a tap on the screen.
- Voice out: `AGENT_TTS_URL` (on the server `http://127.0.0.1:5004`, the user service `assistantx-voice`: Kokoro, British male `bm_george` + `bm_fable`, separate from the other app's voice on 5002) turns on `POST /v1/speech`.

  Only a loopback `http` address is used, the request always asks for the local Kokoro engine (never a cloud voice), text is cut to 1500 characters, and the app falls back to the phone's own voice when the server has none.

## On the phone

- `AssistActivity` is the voice card. It opens when you hold the home or power button (with the app set as the phone's assistant), when you say the wake word, or from the app. It listens, sends the words, shows the answer and reads it out.
- `WakeService` listens for the wake word while it is on. Speech becomes text on the phone, is checked for the word, and is thrown away. It shows a notification the whole time and costs battery.
- `Speech.kt` turns speech into text on the phone. Audio is never sent to the server.
- The live prompt is [live-mode.md](../../server/prompts/live-mode.md).
