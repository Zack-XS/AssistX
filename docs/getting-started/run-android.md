---
title: Run the Android app
description: Build or install the app and connect it to your server.
order: 2
---

# Run the Android app

The app is in `android/` (package `app.assistant`, Android 12 or newer: `minSdk` 31, `targetSdk` 36). It is a full-screen WebView that shows the page `assets/index.html`, plus native screens and services. See [App structure](../android/app-structure.md).

## Get the app

- **From your server (normal way).** The server builds and signs the release APK and hands it out. See [Updates and release](../android/updates-and-release.md) for the first install and automatic updates.
- **Build it yourself.** From `android/`, run `./gradlew assembleDebug` (needs the Android SDK). A debug build is in `android/dist/assistant-debug.apk`. A release build and a debug build have different keys, so remove one before installing the other.

The page comes from `server/examples/dummy-client/index.html`. The Gradle task `copyWeb` copies it into `app/src/main/assets/` but still points at an old path (`../AssistantX/...`), so it skips silently. Copy the page by hand or use the release script. See [Open questions](../roadmap/open-questions.md).

## Connect

1. Open the app. Enter the server address (for example `http://<server-ip>:8790`) and the server token.
2. Sign in to your account with its name and passphrase, or make one. A new account shows a recovery key once: keep it.
3. Allow "remember this phone", so a server restart does not ask you to sign in again ([Accounts](../architecture/accounts.md)).
4. Allow the microphone and notifications when asked.

The server must accept the app's page origin: `AGENT_ALLOWED_ORIGINS` must hold `https://appassets.androidplatform.net`. If the phone reaches the server by a name or address, list it in `AGENT_ALLOWED_HOSTS` and set `AGENT_BIND` ([Configuration](configuration.md)).

## Turn on what you want

All in Settings in the app:

| Feature | Page |
|---|---|
| Location and place reminders | [Location](../architecture/location.md) |
| Listening (phone capture) | [Phone capture](../architecture/phone-capture.md) |
| Health Connect | [health](../plugins/health.md) |
| Spotify, Gmail, Google Health, Hevy sign-in | [Plugins](../plugins/README.md) |
| Noticing (parcels, bills, plans) | [Location](../architecture/location.md), Noticing |
| Morning brief | [Mind](../architecture/mind.md) |
| Wake word and voice | [Voice](../architecture/voice.md) |
