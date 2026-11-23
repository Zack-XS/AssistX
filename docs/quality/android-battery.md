---
title: Android battery
description: How the Android app stays light on battery today, the gaps, and how to measure it.
order: 6
---

# Android battery

A buddy that kills your battery gets turned off. Every app change must keep the phone asleep as much as it can.

## What the app does today

Code is in `android/app/src/main/java/app/assistant/`.

| Part | How it runs | Battery care |
|---|---|---|
| Sync (`Alarms.ensurePeriodic`) | One alarm about every 15 min, `setAndAllowWhileIdle`, re-armed each time | One tiny request. Inexact, so Android can group it with other wakes |
| Reminders and alarms (`Alarms.kt`) | `setAlarmClock` for ringing alarms, `setExactAndAllowWhileIdle` for the rest | Exact only when the user set a time. Falls back to inexact without the permission |
| Location (`LocationService`) | Foreground service, a point about every 5 min | Balanced accuracy (Wi-Fi and cell, not GPS). Slows to every 15 min after 3 points within 150 m (`SLOW_MS`). System may hold points up to 15 min. Points sent together at most every 15 min |
| Geofences (`Fences.kt`) | System geofences for named places | The system watches; the app sleeps. The 5 min points are the backup |
| Live notices (`NoticeStream`) | An open event stream while location is on | Rides on the location service, so no extra service. Backs off on errors |
| Listening places (`PlaceWatch`) | Runs after each location upload | One small request; no own timer. Shows a tap-to-start notice at a chosen place |
| Notifications (`NotifyListener`) | System-bound listener; the system wakes it | Sends only watched apps. Asks the server for the watch list on connect |
| Detected items (`Detect`) | Runs inside `NotifyListener` on the phone | Sends one small item to `/v1/detected`, skips repeats (last 50) |
| Health (`HealthSync`) | Sync when asked, at most every 10 min | Skips if it ran recently |
| Listening (`CaptureService`) | Foreground service, mic on | Sends text in batches every 15 s or 5 lines. Waits 60 s when the server says no |
| Wake word (`WakeService`) | Foreground service, speech recognizer on | Costly: no low-power wake chip is open to apps. Off by default. Pauses while another app has the mic, then retries after 45 s or 3 min |
| Ringing (`RingService`) | Foreground service with a wake lock | Wake lock has a timeout (ring time + 5 s) |

The app asks to skip battery optimisation (`REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`) so alarms and location arrive on time.

## Rules for new app code

1. **Do not add a service.** Ride on one that runs, or use a scheduled wake.
2. **Batch network calls.** Queue, then send together. Never one request per point or event.
3. **Inexact first.** Use exact alarms only for a time the user chose.
4. **Every wake lock has a timeout.** Always `acquire(ms)`, never `acquire()`.
5. **Balanced location.** No GPS-only or high-accuracy requests unless the user is moving and asked for it.
6. **Back off on errors.** Wait longer after each failure. Stop on 401 or 409.
7. **Do nothing when off.** A feature switched off must stop its timers and services.
8. **Say the cost.** A feature that costs battery says so on its switch, as Location does.

## Gaps

- **No WorkManager.** Retries and syncs use `Thread.sleep` loops and alarms. WorkManager would group work with other apps, wait for network and retry for us. Worth moving `HealthSync` and uploads to it.
- **`Thread.sleep` waits** in `AssistActivity`, `RingService`, `Fences` and `NoticeStream` (30 s with no token, and its back-off) hold a thread awake.
- **The event stream stays open** while location is on. One open socket costs radio time. Check if the 15 min sync is enough for most people.
- **Few network checks.** Only auto-update downloads wait for Wi-Fi (`Updater.kt`). Other uploads do not wait for Wi-Fi or charging, even when they could.
- **No measurement.** Nobody has measured the app's battery use. There are no numbers to compare against.
- **No logs** to see how often the app wakes ([Logging](logging.md)).

## How to measure

1. Charge the phone full. Turn on only the feature you test.
2. Leave it a set time, like 8 hours, screen off.
3. Read the app's use in Settings > Battery > App battery use.
4. For detail, take a bug report and open it in Battery Historian:

```
adb shell dumpsys batterystats --reset
# use the phone for a while
adb bugreport bugreport.zip
```

5. Check wakes and alarms: `adb shell dumpsys alarm | grep app.assistant`.
6. Write the before and after numbers in the change notes.

Planned features that send more data, like the [Full activity log](../plans/full-activity-log.md), must follow these rules and batch their uploads.
