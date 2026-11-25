---
title: Haptics
description: How AssistX uses the phone's vibration motor: the few buzzes we use, when, how to code them, and what never to do.
order: 6
---

# Haptics

Haptics are small buzzes from the phone's vibration motor. Done well, they make a tap feel real. Done badly, they nag, drain the battery and feel cheap.

The one rule: **a buzz confirms something the user did or must know now. Nothing else buzzes.**

## Principles

1. **Few and meaningful.** Each buzz means one thing. Users learn it. If everything buzzes, nothing means anything.
2. **Tied to an action or a moment.** A buzz happens at the same instant as the tap, the snap or the result. Never late, never on its own.
3. **Short and crisp.** UI buzzes are under 30 ms each. Crisp beats strong.
4. **Never the only signal.** Every buzz goes with something you can see or hear ([Accessibility](accessibility.md)).
5. **The user is in charge.** One switch turns all UI buzzes off. The system's own "touch feedback" setting is respected too.
6. **Quiet when quiet.** No UI buzzes during a call, in Do Not Disturb (except alarms) or when the screen is off.

## The haptic set

These are the only UI buzzes. Use the name in code and in reviews.

| Token | Feels like | Use for | Android (first choice) | Fallback |
|---|---|---|---|---|
| `haptic-tick` | the lightest tap | sliders and pickers stepping, a list snapping, a segment changing | `HapticFeedbackConstants.SEGMENT_TICK` (API 34) or `CLOCK_TICK` | `PRIMITIVE_TICK`, then `EFFECT_TICK` |
| `haptic-click` | a firm tap | a toggle, a chip, a tab, a button that does something small, mic on | `TOGGLE_ON` / `TOGGLE_OFF` (API 34), else `CONTEXT_CLICK` | `PRIMITIVE_CLICK`, then `EFFECT_CLICK` |
| `haptic-confirm` | a tap with a little lift | something saved, sent or done: answer finished, item added, event saved | `CONFIRM` | `EFFECT_DOUBLE_CLICK` |
| `haptic-reject` | two short knocks | something failed or was refused: error, can't do that | `REJECT` | 2 × `PRIMITIVE_CLICK`, 60 ms apart |
| `haptic-heavy` | a deep thud | long press, drag start, a destructive step (delete) armed | `LONG_PRESS` or `DRAG_START` (API 34) | `EFFECT_HEAVY_CLICK` |
| `haptic-attention` | a soft rise | the assistant needs you now: an approval card appears | `PRIMITIVE_QUICK_RISE` + `PRIMITIVE_CLICK` | `EFFECT_CLICK` |

Not UI buzzes, and handled by their own system:

| Kind | Pattern | Usage | Rule |
|---|---|---|---|
| Alarm | 600 on, 400 off, 600 on, 1400 off, repeat | `USAGE_ALARM` | Rings through Do Not Disturb. Stops the moment the alarm is dismissed or snoozed. |
| Notification | the channel's setting | `USAGE_NOTIFICATION` | Let the notification channel buzz, never the app by hand. The user controls it in Android settings. |

### Which to pick

```
Did the user just touch something?
  ├─ it moves in steps (slider, picker, snap)  → haptic-tick
  ├─ it switches or selects                    → haptic-click
  ├─ it is a long press or drag                → haptic-heavy
Did something finish?
  ├─ it worked                                 → haptic-confirm
  ├─ it failed                                 → haptic-reject
Does the assistant need the user right now?    → haptic-attention
Anything else                                  → no buzz
```

## When to buzz in AssistX

| Moment | Buzz |
|---|---|
| Send a message | `haptic-click` |
| Answer finished | `haptic-confirm` (once per answer) |
| Answer streaming in | **none** |
| Approval card appears | `haptic-attention` + the approval sound |
| Turn error | `haptic-reject` + the error sound |
| Toggle in Settings | `haptic-click` |
| Item added, event saved, job run | `haptic-confirm` |
| Delete confirmed | `haptic-confirm` (the arming long press was `haptic-heavy`) |
| Mic / dictation starts | `haptic-click` |
| Voice mode: the assistant starts talking | none (the sound is enough) |
| Pull to refresh passes the threshold | `haptic-tick` |
| Alarm rings | alarm pattern |
| Background work, Mind runs, sync | **never** |

## How to code it

### Android (Kotlin)

**1. Prefer the view's own feedback.** It needs no permission, follows the user's system setting, and the phone maker tunes it for its motor.

```kotlin
view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
```

**2. For the web page, go through the bridge.** The page calls `NativeApp.haptic(kind)` with a token name. Kotlin maps it to a constant and runs it on the WebView:

```kotlin
@JavascriptInterface
fun haptic(kind: String) = runOnUiThread {
    val c = when (kind) {
        "tick" -> if (SDK_INT >= 34) HapticFeedbackConstants.SEGMENT_TICK else HapticFeedbackConstants.CLOCK_TICK
        "click" -> HapticFeedbackConstants.CONTEXT_CLICK
        "confirm", "success" -> HapticFeedbackConstants.CONFIRM
        "reject", "error" -> HapticFeedbackConstants.REJECT
        "heavy" -> HapticFeedbackConstants.LONG_PRESS
        else -> return@runOnUiThread
    }
    web.performHapticFeedback(c)
}
```

**3. Only when no constant fits, use a composition.** Check the motor can play it first; old or cheap motors can't.

```kotlin
val v = getSystemService(VibratorManager::class.java)?.defaultVibrator ?: return
if (!v.hasVibrator()) return
val fx = if (v.areAllPrimitivesSupported(PRIMITIVE_QUICK_RISE, PRIMITIVE_CLICK))
    VibrationEffect.startComposition()
        .addPrimitive(PRIMITIVE_QUICK_RISE, 0.5f)
        .addPrimitive(PRIMITIVE_CLICK, 0.8f, 40)
        .compose()
else VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
vibrate(v, fx, usage = USAGE_TOUCH)
```

**4. Always say what the buzz is for.** Pass a usage, so the system can apply the user's settings (touch, alarm, notification strength). `VibrationAttributes` is API 33; the app's `minSdk` is 31, so guard it:

```kotlin
fun vibrate(v: Vibrator, fx: VibrationEffect, usage: Int) {
    if (SDK_INT >= 33) v.vibrate(fx, VibrationAttributes.createForUsage(usage))
    else v.vibrate(fx, AudioAttributes.Builder().setUsage(
        if (usage == VibrationAttributes.USAGE_ALARM) AudioAttributes.USAGE_ALARM
        else AudioAttributes.USAGE_ASSISTANCE_SONIFICATION).build())
}
```

**5. Raw waveforms only for alarms.** Hand-made `createWaveform` timings feel buzzy on most motors, and ignore the user's strength settings. The alarm is the one exception.

**6. Stop what you start.** A repeating pattern (`repeat = 0`) must be cancelled in every exit path: dismiss, snooze, timeout, service destroyed.

### Web page

The page never calls the motor itself inside the app. It calls one helper:

```js
hap("confirm");   // tick | click | confirm | reject | heavy | attention
```

`hap()` checks the user's Vibration switch, drops repeats (one `tick` per 40 ms, any buzz at most 10 per second) and calls `NativeApp.haptic`. Outside the app it falls back to `navigator.vibrate` with a short time (5-20 ms). iPhones' Safari has no `navigator.vibrate`; nothing happens, and that is fine.

## Battery and comfort

- The motor is one of the costlier parts to run. Keep UI buzzes under 30 ms and rare. See [Android battery](../quality/android-battery.md).
- **Never buzz in a loop** for streaming, loading, progress or typing. Use motion instead.
- At most one buzz per user action, and one per assistant event.
- Never buzz from a background service, except the alarm.
- Never wake the screen or the CPU only to buzz.

## Accessibility

- The **Vibration** switch (Settings, General) turns off every UI buzz. Alarms and notifications follow Android's own settings.
- With Android "Touch feedback" off, `performHapticFeedback` is silent by itself. Compositions must check the same: use `USAGE_TOUCH`.
- Some people rely on buzzes (low vision, noisy places). Keep the meaning of each token fixed, so it can be learned.
- Some people find buzzes painful or startling. Never force one on, and never use a buzz as a punishment or a nag.

## Testing

- The emulator has no motor. Test on a real phone, with the system touch feedback both on and off.
- Test on one phone with a good motor (Pixel) and one cheap phone. The fallbacks must still feel clean.
- Check: Vibration switch off → no UI buzz anywhere; Do Not Disturb on → alarm still buzzes, UI buzzes don't; alarm dismissed → motor stops at once.
- Code review asks: which token, why here, does it pair with a visual, is it inside a loop? ([Code review](../quality/reviews.md)).

## Never

- Never buzz for something the user didn't do and doesn't need now.
- Never buzz while text streams in.
- Never use long raw waveforms for UI.
- Never call `vibrate` without a usage.
- Never ignore the user's switch or the system's touch feedback setting.
- Never leave a repeating pattern running.

## Gaps

Where the app differs today (checked 2026-11-25). `index.html` is the app page, `android/app/src/main/assets/index.html` (same file as `server/examples/dummy-client/index.html`). Kotlin files are in `android/app/src/main/java/app/assistant/`.

1. **Alarm can crash on Android 12.** `RingService.kt:82-84` calls `vibrate(effect, VibrationAttributes)`, which is API 33. The app's `minSdk` is 31, and there is no version check. Guard it as in step 4.
2. **Buzzing while the answer streams.** `index.html:1816` calls `hapTalk()` on every `message.delta`, about 7 buzzes a second for the whole answer. Remove it; keep one `confirm` when the answer is done.
3. **No usage on UI buzzes.** `MainActivity.kt:736-745` calls `vibrate(fx)` with no attributes, so Android's touch feedback setting is ignored. Switch to `performHapticFeedback` on the WebView.
4. **Hand-made waveforms** for success and error (`MainActivity.kt:741-742`). Use `CONFIRM` and `REJECT`.
5. **Send uses `heavy`** (`index.html:1975`). Send is a normal action: use `click`. Keep `heavy` for long press, drag and arming a risky step (the unrestricted-command card at `:1761` is right).
6. **No `attention` token yet.** Approval cards use `click` (`index.html:1814`).
7. **No `hasVibrator()` check** before vibrating (`MainActivity.kt:737`).
8. The page's names are `success` / `error`; the tokens are `confirm` / `reject`. The bridge should accept both until the page is updated.
