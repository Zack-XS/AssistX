# Health (from the phone)

Daily body numbers the user's phone collects: steps, distance, sleep, heart rate, weight, workouts. They come from the Android app, only if the user turned it on. Data is sensitive: tell it only to the user, and never put it in a web request.

## Tool

`summary` `{days?, from?, to?}`

- Default: the last 7 days that have data. `days` up to 120. Or give `from` and `to` as `YYYY-MM-DD`, both included.
- Each day has `metrics`. The result's `units` says what each means. `sleep_min` is minutes asleep; divide by 60 for hours.
- A missing metric means the phone had nothing that day. Do not guess it.
- Today's numbers grow during the day, so today is usually partial.
- Answer only from the result. For a trend, say which days you used.
- If there are no days, tell the user to open the app and turn on Health in its settings.
