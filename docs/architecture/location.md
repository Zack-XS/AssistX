---
title: Location
description: Phone points, visits, places, place triggers, geofences, noticing and travel time.
order: 13
---

# Location

Status: built (v0.1). Plugin `location`, `src/location.rs`, Android `LocationService`.

## What it does

The phone notes where it is about every 5 minutes and sends the points to the server. The server turns them into visits and places, so the assistant can answer "where was I on Tuesday?", "how long was I at the gym?", and "where do I go most?".

## Phone (battery)

- A foreground service with a quiet notification ("Location on", with Turn off). Android requires the notification for any app that keeps knowing its place.
- `LocationRequest` with balanced power (Wi-Fi and cell, GPS only when the system already has a fix), 5 minute interval, and up to 15 minutes of batching, so the phone stays asleep between deliveries.
- Points go to a small file first, then to the server in one request at most every 15 minutes. Offline, they wait (about two weeks at most).
- It restarts after a reboot or an update if it was on and "Allow all the time" was given.
- Turning it on shows a plain consent note once.

## Server

```
POST   /v1/location/points     {points: [{ts, lat, lon, acc}]}   device or client token
GET    /v1/location/points     ?from=&to= (unix seconds)          raw points
GET    /v1/location/now        last point, the place there, since when
GET    /v1/location/visits     ?from=&to= (local dates)           visits, newest first
GET    /v1/places              ?q=                                places by visits
POST   /v1/places              {name, lat?, lon?, radius?}        name where you are now (or a spot)
GET    /v1/places/{id}
PATCH  /v1/places/{id}         {name, radius?}                    your name always wins
DELETE /v1/places/{id}         forget it (its visits stay, without a place)
```

**Stays.** Points within 120 m of the stay's middle belong to the same stay. A stay of 15 minutes or more is a visit; a visit belongs to the nearest place whose circle (radius, default 100 m) holds it, or to a new place. Points less exact than 250 m are stored but do not move stays.

**Names.** A place with no name is looked up once it has a visit of 45 minutes or more, or 3 visits: the core asks OpenStreetMap Nominatim (`AGENT_GEOCODE_BASE`, default nominatim.openstreetmap.org) with a clear User-Agent, one request a second at most, each place at most once a day.

It keeps the place's own name, else its kind (amenity, shop, leisure), else the street. A name the user gives (`POST/PATCH /v1/places`, or the `name_place` tool) is never replaced.

**Keeping.** Raw points are kept `AGENT_LOCATION_KEEP_DAYS` (default 90). Visits and places stay until forgotten. Export includes them.

## Privacy and trust

- Location never reaches the model unless it calls a `location` tool. It is not in the index.
- With a cloud model, a tool answer is sent to that provider. That is the user's choice of model.
- Place names and addresses from OpenStreetMap are outside content.
- Only the coordinates of a place being named are sent to OpenStreetMap, and only for places that earned a name.

## Place triggers (automations by telling the assistant)

The user says "remind me to walk the dog when I get home" and the assistant sets it up itself with scheduler tools; nothing is hand-built per request.

- `remind_at_place {text, place, when?, once?}` (auto, with undo) makes a `reminder` job with schedule `{on: "place", place, when: "arrive"|"leave", once}`. It fires as an urgent notice.
- `run_task_at_place {name, text, place, when?, once?}` (asks first) starts a full unattended run with the arrival as outside-content data, so it can use any tool (weather, music, memory).
- `place` is a name the user gave or a place id (`location.find_place`). An unknown name gets a plain error that tells the assistant to ask the user to name the spot (`name_place`).
- Detection (`location::track_place`): each fresh point (under 30 minutes old, accuracy 150 m or better) is checked against the known places. Entering a circle is `arrive`; leaving needs 1.5 times the radius, so GPS jitter does not flap. The state is kept in `settings` (`location.inside`).
- A job fires at most once every 10 minutes. With `once` (the default) it is `done` after firing. Old points from an offline phone never fire anything. Outside content cannot create these jobs.
- Timing follows the phone's point rate (about every 5 minutes, batched up to 15), so "when I get home" is within minutes, not seconds.

## Geofences (fast arrive and leave)

The phone watches circles itself, so a place reminder does not wait for the 15 minute batch of points. No Google services: Android's own proximity alerts (`Fences.kt`).

```
GET  /v1/location/fences       places an active place job waits on: [{id, name, lat, lon, radius}]   device or client token
POST /v1/location/transitions  {place, what: "arrive"|"leave", ts?}                                  device or client token
```

- Only places a reminder or task waits on are sent. The phone syncs the list when the location service starts and every 15 minutes.
- A transition shares `location.inside` with the 5 minute tracker, so a job never fires twice. Arriving where the server already says you are, leaving a place you were not in, and anything older than 30 minutes are ignored.
- The phone ignores an "enter" within 90 seconds of registering a circle (Android fires at once if you are already inside).
- Circle size is the place's radius, at least 120 m. The 5 minute tracking stays as the backup.
- A reminder made just now can wait up to 15 minutes for its circle to be registered.

## Instant delivery and buttons

- **Live connection.** While the location service runs, the app holds one `GET /v1/events` connection open (`NoticeStream.kt`). `assistant.notice` makes it fetch new notices at once; `fences.changed` makes it re-read `GET /v1/location/fences` at once. It reconnects with a growing wait and the 15 minute check stays as a backup. Without the location service there is no live connection.
- **`fences.changed`** is emitted when a place job is created, cancelled, or used up.
- **Buttons.** A notice a reminder made carries `job_id` (`GET /v1/notices`). The phone adds Snooze 10 min and Done. `POST /v1/notices/{n}/snooze {minutes?}` makes the same words again as a reminder (400 for a notice that is not from a reminder).

## Place and time together, and ideas

- `remind_at_place` and `run_task_at_place` take `after`, `before` (clock times, "18:00"; a window may pass midnight) and `days` (0-6, 0 = Sunday, or "weekdays" / "weekends").

  The window is checked when the arrival or departure comes in (`scheduler::in_window`, local time, the job's time zone or the server's). Outside it nothing happens and the job keeps waiting; an arrival is not saved up for later.
- **Ideas** (`location::suggest`, once a day between 10:00 and 20:00 local): a named place the user was at on the same weekday in at least 3 of the last 4 weeks, with no reminder waiting on it, not visited nearly daily (more than 20 visits in 28 days, like home), and not suggested before.

  The core starts a chat titled "Idea: <place>" with the assistant's own message and sends a notice that opens it. If the user answers there, the assistant makes what they ask for with the scheduler tools. One idea per day, and each place is asked about once.

  This is a core check, not part of the dreaming run itself.

## Noticing (leaving home, gym visits, phone notices)

`src/detect.rs`. Five switches, all on by default: `GET /v1/detected/settings` (device, client or owner), `PATCH /v1/detected/settings {leave_home, gym, parcel, bill, plan, home_place?}` (owner or client). Page: Settings, Noticing, shown only with developer mode on.

- **Leaving home.** Home is the place named Home (or `home_place`). Its circle is added to `GET /v1/location/fences`, so the phone's geofence reports the departure at once.

  On `leave` (`scheduler::place_event` calls `detect::on_place` first) the core sends one normal notice, "Leaving home".

  It holds:

  - the weather now and for the next 8 hours from Open-Meteo (`AGENT_WEATHER_BASE`, default `https://api.open-meteo.com`, `off` to skip; the spot is rounded to 2 decimals);
  - "take an umbrella" when rain is likely (50% or more, 0.5 mm or more, or a rain code);
  - the first calendar event still to come today;
  - "Remember to bring": the texts of leave reminders waiting on Home plus the open items of a list called "Bring" (read softly from the `lists` tables).

  At most once per 30 minutes, never between 23:00 and 05:00 local. The leave reminders still fire as their own notices.
- **Gym visits.** Every 2 minutes the timer looks at visits that ended 15 minutes to 3 hours ago, lasted 25 minutes or more, at a place whose name or OpenStreetMap kind looks like a gym (`fitness_centre`, "gym", "fitness", "crossfit", ...).

  Only when `HEVY_API_KEY` is saved: the core reads the latest Hevy workouts; if none touches the visit (15 minutes of slack each side), it opens a chat "Log workout" with the assistant's message (times, minutes, that day's Health Connect `exercise_min` if any, an example reply) and sends a notice "Log your workout?" that opens it.

  Logging is `hevy.log_workout`, which still asks for a yes. Once per visit (`detect_done`).
- **Phone notices.** The app matches every notification on the phone (`Detect.kt`): parcels (out for delivery, delivered, shipped, tracking numbers), bills (amount due, due date, invoice), plans (invites, "see you at 7", "dinner at").

  Codes, one-time passwords, verification and sign-in messages are ignored before any matching. Only `{kind, title, date?, amount?, ref?, app}` is sent to `POST /v1/detected {items}` (device token allowed, at most 20, 30 a minute).

  The server checks again: refuses code-like items, skips turned-off kinds and repeats within a day, cuts fields short. Each kept item is a `detected` row and an inbox note with `outside` trust (dreaming files it).

  The mind's digest and the index show the last items wrapped in `<outside source="phone:notifications">`. `GET /v1/detected` (owner/client) lists the last 30 days; rows are kept 90 days.

## Travel time and "time to leave"

`src/travel.rs`. Tool `travel_time {to, mode?}` in the location plugin: from the phone's last point (no older than 2 hours) to `to`, which is a place the user named or an address.

- **Where to.** A named place first; else OpenStreetMap's Nominatim `/search` (the same host as place names, `AGENT_GEOCODE_BASE`), one request a second, answers kept a month in `travel_geo` (a miss is kept a day).
- **How long.** The public OSRM router, one fixed host: `https://router.project-osrm.org/route/v1/driving/...` (`AGENT_ROUTING_BASE` changes it). No key, no live traffic. That router only drives; walking and cycling are the driving route's length at 5 km/h and 15 km/h.
- **Leave-now alerts.** Each minute the core timer looks at calendar events with a location that start in the next 4 hours (not all-day).

  It works the trip out (again every 15 minutes) and when `start - travel - buffer` is reached it sends "Leave in 10 min for X (25 min drive)" (or "Leave now ..." when late), once per event occurrence (`travel_alerts`).

  Nothing when the phone has no fresh point or is already within 300 m. Priority `urgent` (it is about to be missed), `normal` for events from outside content.
- **Settings.** `GET,PATCH /v1/travel {enabled, mode: driving|walking|cycling, buffer_min: 0-60}`; defaults on, driving, 10. In the app: Settings, Automation, "Built in".
