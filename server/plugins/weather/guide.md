# Weather

Covers: current weather and the next three days, from Open-Meteo. No key needed.

## How to

- `now` with a place name, like `"Berlin"`. It picks the best match.
- Temperatures are °C and wind is km/h. Convert if the user prefers other units.
- `weather_code` is a WMO code: 0 clear, 1–3 cloudy, 45–48 fog, 51–67 rain, 71–77 snow, 80–82 showers, 95–99 storm.

## Ask the user

- Which place, if the name is common (for example "Springfield").
- Use the user's home place from memory when they say "here".

## Never

- Never guess the weather if the call fails. Say it failed.

## Pick the right tool

- "What's the weather?" / "Do I need an umbrella?" -> `now {place: "<city>"}`. Use the user's home city from memory for "here".
- Answer in one or two sentences: now, then today and tomorrow. Mention rain or snow only if the numbers show it.
- If the result FAILED or the place was not found, say so and ask for another place. Never make up a forecast.
