---
title: spotify
description: Music: plays songs, artists, albums, playlists and liked songs on your Spotify devices.
order: 14
---

# spotify

Music. Any music word (play, pause, resume, skip, louder, shuffle, like this song, what is playing) means this. Plays songs, artists, albums, playlists and liked songs on the user's Spotify devices.

- Backend: `core` (src/spotify.rs; the core makes the calls and keeps the token)
- Needs: `network`: [api.spotify.com]; `oauth`: spotify
- Quick-command words: ["music", "song", "songs", "spotify", "play", "playing", "pause", "unpause", "resume", "skip", "next song", "previous song", "louder", "quieter", "volume", "shuffle", "playlist", "album"]
- Guide for the model: [guide.md](../../server/plugins/spotify/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/spotify/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `now` | reads | no |
| `devices` | reads | no |
| `search` | reads | no |
| `info` | reads | no |
| `play_liked` | writes | no |
| `play` | writes | yes |
| `pause` | writes | yes |
| `next` | writes | yes |
| `previous` | writes | yes |
| `volume` | writes | yes |
| `restore_volume` | writes | yes |
| `play_uri` | writes | yes |
| `play_song` | writes | yes |
| `restore_playback` | writes | yes |
| `transfer` | writes | yes |
| `shuffle` | writes | no |
| `repeat` | writes | no |
| `queue` | writes | no |
| `like` | writes | no |
| `unlike` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Your Spotify account through Spotify's web service, on any device. Needs Premium to control playback. Sign in once under Settings, Integrations. `play_song` searches precisely, wakes a device, checks sound really started (6 looks), restores the old music on failure, and the last seen music line is added to the index.

## Signing in to Spotify

The Spotify plugin controls your Spotify account through Spotify's own service, so it works on any device that is playing (your phone, a speaker, the desktop app). It does not control this computer. Playback control needs Spotify Premium.

One-time setup:

1. Go to developer.spotify.com/dashboard and make an app (free). Copy its **Client ID**. No secret is needed.
2. In that app's settings, add the Redirect URI the page shows: `app.assistant://spotify` in the phone app (or `http://127.0.0.1:8000/` when testing the page on the Mac).
3. In the app: Settings, Integrations, Spotify, paste the Client ID, tap **Connect Spotify**, and approve in the browser. The browser returns to the app, which finishes the sign-in.

The core keeps the tokens (`SPOTIFY_REFRESH_TOKEN` and friends, in the secrets table), refreshes the short-lived one itself, and never shows them to the plugin folder or the model. Disconnect removes them. Tokens are stored in plain text like other secrets (see [Next](../roadmap/next.md)).

If playback commands say "No Spotify device is active", open Spotify on the phone and play something for a second, then try again.

## How Spotify playback stays reliable

- `play_song` takes fields (`query`, `artist`, `kind`, `device_id`), so a device name never leaks into the song search. It searches precisely (`track:"X" artist:"Y"`).
- A wrong guess returns "Closest real ones: ..." and the model must pick one or ask. It may not invent a name.
- Spotify answers OK even when nothing plays, so the core looks at the player up to 6 times, 1.5 s apart, and says "playing" only when the right song is playing.
- Every play command names a device: the one asked for, else the one last used (even if paused). Without a name Spotify picks "the active device", which a phone stops being soon after it pauses. If that fails it wakes one (last used, then active, computer, speaker, TV, phone).
- After a song starts it queues up to 5 more by the same artist, so a single does not end in silence.
- If no sound starts, it puts the earlier music back and says so plainly. Undo does the same.
- The last seen music line is kept in memory and added to the index with its age. It is outside content and is never fetched per turn.
- `info` reads the user's own library: liked songs, top tracks and artists, recent plays, playlists. `play_liked` plays liked songs shuffled. A playlist name tries the user's own playlists first.
- These need four more sign-in permissions (`user-library-read`, `user-top-read`, `user-read-recently-played`, `playlist-read-private`). A sign-in made before them must be done once more: Settings, Integrations, Spotify, Connect. `GET /v1/oauth/spotify` lists anything still `missing_scopes`.
