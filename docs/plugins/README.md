---
title: Plugins
description: One page per shipped plugin, with its tools and limits.
order: 6
---

# Plugins

A plugin is a guide for the model plus optional tools. The format is in [Plugin format](../architecture/plugins.md). How the agent loads one is in [Plugin loading](../agents/plugin-loading.md); how it builds a new one is in [Plugin maker](../agents/plugin-maker.md).

Each plugin folder is `server/plugins/<id>/` with `plugin.yaml` and `guide.md`. Those files are loaded by the engine. These pages sum them up and link to them; edit the plugin files, not these pages, to change what the model does.

| Plugin | Backend | What it does |
|---|---|---|
| [calendar](calendar.md) | core | The user's calendar - see, add, move and delete events, repeats, and reminders that can ring. |
| [computers](computers.md) | core | The user's own computers (Mac, Windows, Linux) running agent-node - list, read, search, copy and move their files. |
| [email](email.md) | core | Your Gmail - recent and unread mail, search, read one mail, a short inbox summary, drafts and replies; sending always asks first. |
| [files](files.md) | rust | Browse, read, write, move and find files inside one allowed folder. |
| [google-health](google-health.md) | core | Your Fitbit and Pixel Watch data via Google - steps, heart rate, sleep, exercise, weight and more. Read only. |
| [health](health.md) | core | The user's body numbers per day from their phone - steps, sleep, heart rate, weight, workouts. |
| [hevy](hevy.md) | core | Your Hevy workout log - recent workouts, routines, exercises; log a finished workout. |
| [lists](lists.md) | core | The user's lists - shopping, to-do or any name; add, tick off, remove and share items. |
| [location](location.md) | core | Where the user is and has been - places they stay or visit often, named by them or by OpenStreetMap. |
| [memory-tools](memory-tools.md) | core | Skill for writing things down. Load it when the user shares something worth keeping, or wants memory organised, folders made or facts rated. |
| [phone](phone.md) | core | Do things on the user's phone - open the dialer or a ready-to-send text, open an app, set a timer or a clock-app alarm, start directions. |
| [plugin-maker](plugin-maker.md) | core | How to learn a new ability by building a plugin when nothing in the index fits. |
| [scheduler](scheduler.md) | core | Reminders (by time or when you arrive at or leave a place), repeating jobs, alarms that ring, and tasks that start the assistant later to work on its own and report. |
| [spotify](spotify.md) | core | Music. Any music word (play, pause, resume, skip, louder, shuffle, like this song, what is playing) means this. Plays songs, artists, albums, playlists and liked songs on the user's Spotify devices. |
| [terminal](terminal.md) | rust | Run commands in the user's work folder - allowlisted programs, or any command line after a warned yes. |
| [weather](weather.md) | rust | Current weather and a three day outlook for a named place. |
| [web](web.md) | rust | Read public web pages as plain text and list their links. |

Backend `core` means the code runs inside the server (`server/src/<name>.rs`). Backend `rust` means a sandboxed tool run (`server/src/plugins/<id>.rs`, started as `agent-core tool-run <plugin> <tool>`).

## Plugins we ship (contributor table)

| Plugin | Backend | Tools (auto / ask) | Limits |
|---|---|---|---|
| files | rust | list, read, search / write, mkdir, move (+ undo tools) | `needs.paths` = `AGENT_FILES_ROOT` only. No delete. |
| terminal | rust | allowed / run, shell | `run`: allowlist, no shell. `shell`: any command through `/bin/sh`, tag `unrestricted` = always a yes with a loud warning showing the exact command (no setting makes it auto). Both in the sandbox, `needs.paths` = `AGENT_TERM_CWD`. |
| web | rust | fetch, links / — | Through the proxy; any public host; no redirects. |
| weather | rust | now / — | Proxy allows only the two Open-Meteo hosts. |
| spotify | core | now, devices, search, info (liked, top, top_artists, recent, playlists), play_liked / play_song, play, play_uri, pause, next, previous, volume, transfer (+ undo) | Your Spotify account through Spotify's web service, on any device. Needs Premium to control playback. Sign in once under Settings, Integrations. Details: [spotify](spotify.md). |
| hevy | core | workouts, workout, workout_count, routines, exercise_templates / log_workout | Your Hevy log through Hevy's API. Needs a Hevy Pro API key saved as secret `HEVY_API_KEY`. The API has no delete, so logging cannot be undone (it asks first; workouts are private unless told otherwise). |
| email | core | inbox, search, read, summarize_inbox, draft, reply_draft (+ undo: delete_draft) / send | Your Gmail through Gmail's API (`gmail.googleapis.com` only). Everything read is outside content. `send` always asks, with the exact mail on the card. Details: [email](email.md). |
| google-health | core | data | Read only, through Google Health (the Fitbit Web API's replacement, `health.googleapis.com/v4`). Sign in under Settings, Integrations with a Google Cloud OAuth Client ID and Client Secret. Data types and filter names are not checked against a live account yet. |
| health | core | summary | Daily body numbers sent by the Android app from Health Connect (steps, sleep, heart rate, weight, workouts). `POST /v1/health/days` (device or client token), `GET /v1/health/days`. Table `health_days`, one row per day and metric; raw readings stay on the phone. |
| scheduler | core | create_alarms (many at once), remind_at_place (arrive/leave a named place), run_task_at_place / ask, create_reminder, create_alarm, schedule_task, update_job, list_jobs, list_sounds, snooze_alarm, dismiss_alarm, cancel_job (+ undo) / create_recurring, schedule_recurring_task | Jobs after outside content are marked `outside`; they never ring and cannot be alarms. |
| calendar | core | list_events, find_events, get_event, add_event, update_event (+ undo) / delete_event | Events with repeats and reminders (`event` jobs that follow the event, may ring). Next 24 h shown in the index. |
| lists | core | list_lists, show_list, add_items (many at once), check_items, uncheck_items, remove_items, clear_checked, rename_list (+ undo) / delete_list | Shopping, to-do or any name; items with quantity, note, due date. Names match loosely (groceries = shopping). Lists with open items shown in the index as counts. App: Plan, Lists (tick, add, share). |
| location | core | where_now, history, places, name_place (+ undo), travel_time / — | Phone points every ~5 min become visits and places; OpenStreetMap names a place after 45 min or 3 visits; the user's name wins. `travel_time` asks OSRM (`router.project-osrm.org`); leave-now alerts for calendar events with a place ([Location](../architecture/location.md)). Not in the index. |
| computers | core | list_devices, info, list, stat, read, search / write, mkdir, copy, move, delete | Runs on a computer with agent-node, inside its allowed folders; key folders always refused; `--read-only` on the computer refuses all changes. |
| phone | core | open_app, set_timer, set_phone_alarm, navigate_to / call, text | Done by the Android app (`src/phone.rs`, queue `phone_actions`). Only opens screens: the dialer or a filled-in text, the user presses call or send. Contact names are looked up on the phone. App in the background: a notification to tap. No answer in 20 s: `sent_to_phone`. |
| plugin-maker | core | start_build / — | Starts a build-mode turn; the draft needs the owner. |
| memory-tools | core | remember (person, place, thing), list_disputed / set_status | Guide for bigger memory work. `remember` is also dreaming's `dream_remember`. |
