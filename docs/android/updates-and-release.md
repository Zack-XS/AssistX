---
title: Updates and release
description: How the app is built, published, installed and updated from your own server.
order: 3
---

# Updates and release

> Code check: the app now lives in `android/` in this project (not a separate `AssistantX-Android` folder). It is not in git.

## Release and update (automatic)

Run `assistantx-app-release "what changed"` (a helper in `~/bin`, outside the repo). It:

1. copies the latest page into the app and sends the project to the server;
2. builds a **release** APK there and signs it with the server's own key (`~/.assistantx.keystore`, password in `~/.assistantx-keystore.pass`, both mode 0600);
3. puts the APK at `~/assistantx/data/app/assistant.apk` and then `version.json` (`versionCode`, `versionName`, `notes`). The version file goes last, so the server never offers a version without its file. Each build's code counts up (minutes since 2026-01-01).

The server hands them out: `GET /v1/app/version` (version, notes, and the file's size and SHA-256, worked out from the file itself) and `GET /v1/app/apk`. Both need a token; a `device` token may use them too. They return 404 until something is published.

The installed app:

- checks when it opens and every 30 minutes while it stays open, after a short wait;
- downloads the new APK (not over mobile data unless you tap **Check for updates**), checks the SHA-256, and checks that the file is this app at the announced version;
- installs it through the system installer. You allow "install unknown apps" for the app once, and the system confirms the first update; after that updates install without a prompt. Turn the automatic part off under Settings, General, App updates;
- shows an **Update ready** card on Home, and a notification "Assistant updated" after the restart. An update stops the app and the listening service, so open it again and turn listening back on.

**Keep the signing key safe.** Android only accepts an update signed with the same key as the installed app. Back up `~/.assistantx.keystore` and its password file. With a new key, every phone has to uninstall the app first. The first install of a release build also needs the old debug build removed (different key).

## Install the first time

With USB debugging on and the phone plugged in: `adb install -r dist/assistant.apk` after `scp assistantx-server:assistantx/data/app/assistant.apk dist/`. Or open the APK from the server on the phone's browser while the temporary share is running (see below).

```sh
ssh assistantx-server '(nohup timeout 1800 python3 -m http.server 8791 --bind <server-ip> --directory ~/apk-share >/dev/null 2>&1 &)'
# copy the file there first: ssh assistantx-server 'mkdir -p ~/apk-share && cp ~/assistantx/data/app/assistant.apk ~/apk-share/'
# phone: http://<server-ip>:8791/assistant.apk   (closes itself after 30 minutes)
```

Release builds are not debuggable, so they ignore the `adb ... --es token` shortcut; paste the token once.

## Update banner and change log

- When a new version is published, the server notices `data/app/version.json` changed (checked each core tick) and emits `app.updated`. A phone with the live connection (`NoticeStream`) checks at once. Any other phone checks when the app opens or comes back (at most every 30 minutes).
- The page shows a banner across the top while an update is available, downloading, ready or failed: **Update app** (installs when ready, otherwise downloads), and **What's new** with the change log for every version newer than the installed one.
- `GET /v1/app/version` carries `history` (newest 15) from `data/app/changelog.json`. `assistantx-app-release` appends one entry (version, notes, date) before it publishes the new `version.json`.

- App updates ([Deploy](../architecture/deploy.md)): `GET /v1/app/version` and `GET /v1/app/apk` serve `data/app/assistant.apk` + `version.json` (size and SHA-256 come from the file).

  Published by `~/bin/assistantx-app-release`, which builds a release APK on the server with the server's key. The Android app checks on open and installs by itself.

  The page calls `NativeApp.pageReady()` when it starts; if it does not within 8 s of loading (a broken version), the app shows its own dialog with Update now (installs even with auto-update off) and Reload.

  `assistantx-app-release` refuses to publish a page whose script does not parse.
