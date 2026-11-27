---
title: App structure
description: The files, screens, permissions and sounds of the Android app.
order: 1
---

# App structure

Code is in `android/app/src/main/java/app/assistant/`. The app is two things: the WebView page (most screens) and native Kotlin screens and services. It uses no Google services.

## Files

| File | Job |
|---|---|
| `MainActivity.kt` | Hosts the page (`assets/index.html`) in a full-screen WebView. Gives it native things through `NativeApp` (46 bridge methods): server address and token, microphone, dictation, listening, vibration, dialogs, Back. Shows a rescue dialog if the page does not call `pageReady()` within 8 s. |
| `AssistActivity.kt` | The voice card (hold home or power, wake word, or from the app). |
| `RingActivity.kt / RingService.kt` | Full-screen ringing card over the lock screen; the ring sound, louder over time, with vibration. |
| `Alarms.kt` | Copies coming alarms and reminders into Android's alarm clock, so they ring with the app closed or offline. Syncs on open, after each turn, every 15 minutes, after a restart, and when one fires. |
| `Notices.kt` | Assistant notices as phone notifications (normal, quiet, urgent channels). |
| `NoticeStream.kt` | One held-open `GET /v1/events` connection while location runs, for instant notices and fence changes. |
| `CaptureService.kt` | Listening: speech to text on the phone, uploads text chunks. See [Phone capture](../architecture/phone-capture.md). |
| `PlaceWatch.kt` | Listening is place by place: asks the server where the phone is after each location upload. |
| `WakeService.kt` | Listens for the wake word; opens the voice card. |
| `Speech.kt` | Speech to text on the phone. Audio never leaves the phone. |
| `LocationService.kt` | Location points every ~5 minutes. See [Location](../architecture/location.md). |
| `Fences.kt` | Android's own proximity alerts for place reminders (no Google services). |
| `HealthSync.kt` | Daily totals from Health Connect, one row per day to the server. |
| `Detect.kt` | Finds parcels, bills and plans in notifications on the phone; sends only small items. |
| `NotifyListener.kt` | Forwards notifications of watched apps so automations can start on them. |
| `PhoneActions.kt` | Runs the `phone` plugin's actions: dialer, text, open app, timer, alarm, directions. |
| `PhotoText.kt` | Reads text in photos on the phone (bundled ML Kit). |
| `Woke.kt` | Tells the server the user woke up, once a day, for the morning brief. |
| `Unlock.kt` | Remembered phone: unlocks the account after a server restart. |
| `Vault.kt` | Keeps the token encrypted with a Keystore key. |
| `Updater.kt / InstallResult.kt / Replaced.kt` | Self-update from your server. |
| `Sfx.kt` | Short sound effects. |
| `Tiles.kt` | Quick settings tiles (Chat, Talk). |
| `VoiceGlow.kt` | The edge glow of the voice screen. |

## The page

The page is one self-contained HTML file: `server/examples/dummy-client/index.html`, copied into `android/app/src/main/assets/index.html`. Today it has four tabs:

Home, Plan (Calendar, Alarms, Lists), Assistant (chat) and Settings (12 rows in cards, plus Noticing and Developer with developer mode on; the rest are found by search). The redesign renames them Today, Plan, Chat and You, with the same view ids ([Screens](../design/screens.md)).

The server's rules call it a reference client: the backend never depends on it.

## Bridge contract (frozen)

The page and the app talk through these names. Renaming one breaks the app silently. Change both sides in the same release. Safety for each call: [Safety rules](../security/safety-rules.md#android).

**Page to app** (`NativeApp.*`, also `NA.*`): 46 methods in `MainActivity.Bridge`.

`alarmStatus` `captureState` `checkUpdate` `clearConfig` `forgetDevice` `getConfig` `haptic` `hasDictation` `healthState` `holdEnd` `holdStart` `installUpdate`
`locationState` `notifyApps` `notifyState` `openAlarmSettings` `openAssistantSettings` `openNotifySettings` `openOverlaySettings` `openUrl` `pageReady` `phoneActions` `postNotice` `readPhoto`
`saveConfig` `saveDevice` `setAutoUpdate` `setCapture` `setHealth` `setLocation` `setSfx` `setTheme` `setWake` `setupFix` `setupState` `sfx`
`sfxEnabled` `shareText` `syncAlarms` `syncHealth` `syncNotify` `takeOauth` `talk` `unlockNow` `updateInfo` `voiceState`

The page must also work in a plain browser with no `NativeApp`. Check a method exists before calling it.

**`pageReady`:** the page calls it as soon as its script starts, before any API call. If it does not come within 8 s, the app shows the rescue dialog. Nothing that can throw runs before it.

**App to page** (`window.__*`, called by `MainActivity`): `__open` `__ready` `__captureChanged` `__updateChanged` `__locationChanged` `__voiceChanged` `__healthChanged` `__dictation` `__dictationState` `__oauthCheck` `__shared` `__photoText` `__setupChanged`. Page-only, free to change: `__acct` `__closeChatMenu` `__lost`.

**Open names** (`window.__open(name)`, allowed list `OPEN_VIEWS` in `MainActivity`, shortcuts in `res/xml/shortcuts.xml`): `calendar` `alarms` `new-event` `new-alarm` `new-chat`. `lists` works in the page only; add it to `OPEN_VIEWS` before a shortcut uses it.

**Stored keys** (`localStorage`). Renaming one resets a choice or loses data: `lang` `lastUser` `noticeCursor` `oauth.provider` `opt.*` `outbox` `planTab` `setupSkipped` `startView` `theme` `thread` `titles` `weakUntil`. `planTab` gains `task`; `cal`, `alarm`, `list` stay valid.

**One file.** The page is one HTML file with one `<script>` block. The release script copies that one file and checks the block parses. Splitting it breaks both.

## Native colours

Native screens use the [token](../design/tokens.md) names in `res/values/colors.xml` and `res/values-night/colors.xml`: `page`, `card`, `line`, `text`, `mute`, `ink`, `on_ink`, `ok`, `warn`, `bad` (today only `page` and `icon_bg` exist). `MainActivity` takes the window colour from `page`. The ringing and voice screens stay dark on purpose, but take `bad`, `text` and `mute` from the dark set.

Design rules for the page and native screens belong to [Design](../design/README.md).

## What the app does on the phone

- **Full screen:** the status and navigation bars are hidden (swipe from the edge to show them). The page keeps clear of the camera cutout and the keyboard.
- **Microphone:** the first launch explains and asks for the microphone (and notifications, which Android needs for the listening notice). Dictation turns speech into text in the message box. Speech is recognised on the phone when the phone has an on-device model; otherwise the phone's own speech service is used, and that may use the network. The server only ever gets text.
- **Listen on this phone** (Settings, Privacy; also on Home): a foreground service (`CaptureService`) listens, turns speech into text, and uploads chunks to `POST /v1/ambient/chunks` every 15 seconds. It shows a notification with a Stop button.

  Turning it on shows a plain warning about recording laws once. It cannot yet tell whose voice it hears, so every chunk is sent as speaker `unknown` (outside trust) and nothing becomes memory until you keep it.
- **Vibration:** taps, tab changes, sending, and the assistant's answer as it arrives (a light tick while text streams, a double buzz when it finishes, a long one on an error). Off under Settings, General.

## Permissions

From `AndroidManifest.xml`:

- Internet, notifications, microphone, camera, vibration.
- Contacts (names to numbers for calls and texts).
- Fine, coarse and background location.
- Foreground services (location, microphone, media).
- Exact alarms and full-screen alarms, boot completed.
- Install packages (self-update), battery optimisation opt-out, draw over apps.
- Health Connect reads: steps, sleep, heart rate, resting heart rate, weight, exercise, distance, calories, floors, hydration.

It is a share target (text and images) and can be the phone's assistant app.

## Sound effects

The mp3 files in `SOUND EFFECTS/` are copied into the Android app (`res/raw`, lower-case names) and played by the phone:

| File | Where it plays |
|---|---|
| `Response:send.mp3` | You send a message in the chat |
| `Error.mp3` | A turn fails, or a message could not be sent |
| `aproval check.mp3` | The assistant asks for a yes (an approval card appears) |
| `LiveModeStart.mp3` / `LiveModeEnd.mp3` | The voice screen opens / closes |
| `Live Loop.mp3` | A soft loop on the voice screen while it works on the answer (never while listening, so the microphone does not hear it) |
| `Notifcation Freindly.mp3` | The sound of normal notices and quiet reminders (a new notification channel, because Android fixes a channel's sound when it is made) |
| `ALARM REPEAT.mp3` | The ring sound of an alarm whose sound is `default` (it loops until stopped) |

They are silent when the phone is on silent or vibrate. One switch, Settings > General > Sound effects. To change a sound, replace the file in the folder and run `assistantx-app-release` (copy it to the same name under `AssistantX-Android/app/src/main/res/raw/`).
