//! Spotify through its web service, for your account (docs/03, plugin `spotify`).
//!
//! It controls Spotify itself, on whichever device is playing (your phone, the desktop app, a
//! speaker), not this computer. Playback control needs Spotify Premium. The core makes the calls:
//! it talks to one fixed host, builds every address from encoded pieces, and holds the token,
//! so the model and the plugin folder never see it. Sign-in is in `oauth.rs`.

use crate::oauth;
use crate::App;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Wait between checks that sound really started (Spotify says OK even when nothing plays).
/// Milliseconds; tests lower it.
pub static VERIFY_GAP_MS: AtomicU64 = AtomicU64::new(1500);
const VERIFY_TRIES: usize = 6;
/// A remembered "what is playing" line older than this is not shown to the model.
const LINE_MAX_AGE_SECS: i64 = 2 * 3600;

/// Run one Spotify tool. The network work happens off the async threads.
pub async fn call(app: &Arc<App>, tool: &str, args: &Value) -> Result<Value, String> {
    let (app, tool, args) = (app.clone(), tool.to_string(), args.clone());
    tokio::task::spawn_blocking(move || {
        let token = oauth::fresh_token(&app, "spotify")?;
        let r = run(&app.cfg.spotify_base, &token, &tool, &args);
        if let Ok(v) = &r {
            remember(&app, &tool, v);
        }
        r
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Keep the last known music state so the next turn knows it without a lookup. Anything that
/// changes playback in a way we cannot describe clears it, so the line never lies.
fn remember(app: &App, tool: &str, v: &Value) {
    let line = match tool {
        "now" => Some(v["line"].as_str().unwrap_or("Spotify: nothing playing.").to_string()),
        "play_song" | "play_liked" | "play" | "play_uri" | "transfer" => v["line"].as_str().map(String::from),
        "pause" => Some("Spotify: paused.".to_string()),
        "volume" | "devices" | "search" | "restore_volume" | "info" | "shuffle" | "repeat" | "queue" | "like" | "unlike" => return,
        _ => None,
    };
    *app.music.lock().unwrap() = line.map(|l| (crate::util::unix_now(), l));
}

/// The remembered music line, wrapped as outside content (titles come from Spotify), with its age.
pub fn status_line(app: &App) -> Option<String> {
    let (at, line) = app.music.lock().unwrap().clone()?;
    let age = crate::util::unix_now() - at;
    if age > LINE_MAX_AGE_SECS {
        return None;
    }
    Some(crate::util::wrap_outside("tool:spotify.now", &format!("{line} (seen {} min ago)", age / 60)))
}

/// One sentence for a player state, e.g. `Spotify: playing X by Y on Z, volume 40%.`
fn describe(v: &Value) -> String {
    let name = v["item"]["name"].as_str().unwrap_or("");
    if name.is_empty() {
        return "Spotify: nothing playing.".into();
    }
    let by = names(&v["item"]["artists"]).join(", ");
    let by = if by.is_empty() { String::new() } else { format!(" by {by}") };
    let dev = v["device"]["name"].as_str().unwrap_or("a device");
    let vol = v["device"]["volume_percent"].as_i64().map(|n| format!(", volume {n}%")).unwrap_or_default();
    let state = if v["is_playing"].as_bool() == Some(true) { "playing" } else { "paused on" };
    if state == "playing" { format!("Spotify: playing {name}{by} on {dev}{vol}.") } else { format!("Spotify: {name}{by} is paused on {dev}{vol}.") }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into()
}

/// One request. Returns the status and the JSON body (Null when empty).
fn api(base: &str, token: &str, method: &str, path: &str, query: &[(&str, &str)], body: Option<&Value>) -> Result<(u16, Value), String> {
    let mut url = format!("{base}{path}");
    if !query.is_empty() {
        url.push('?');
        url.push_str(&oauth::form(query));
    }
    let a = agent();
    let auth = format!("Bearer {token}");
    let mut resp = match method {
        "GET" => a.get(&url).header("Authorization", &auth).call(),
        "POST" => a.post(&url).header("Authorization", &auth).send(""),
        _ => a.put(&url).header("Authorization", &auth).header("Content-Type", "application/json").send(body.map(|b| b.to_string()).unwrap_or_default()),
    }
    .map_err(|e| format!("Could not reach Spotify: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// Turn Spotify's complaints into something to tell the user.
fn explain(status: u16, body: &Value) -> String {
    let reason = body["error"]["reason"].as_str().unwrap_or("");
    let msg = body["error"]["message"].as_str().unwrap_or("");
    match (status, reason) {
        (_, "PREMIUM_REQUIRED") => "Spotify Premium is needed to control playback.".into(),
        (404, "NO_ACTIVE_DEVICE") | (404, _) if msg.contains("device") || reason == "NO_ACTIVE_DEVICE" => {
            "No Spotify device is active. Open Spotify on your phone or another device and play something for a second, or use devices and transfer.".into()
        }
        (401, _) => "Spotify rejected the login. Connect Spotify again in Settings, Integrations.".into(),
        (403, _) if msg.to_ascii_lowercase().contains("scope") => {
            "Spotify has not allowed this yet. Connect Spotify again in Settings, Integrations, and allow the new permissions.".into()
        }
        (429, _) => "Spotify says to slow down. Try again in a moment.".into(),
        _ => format!("Spotify answered {status}{}", if msg.is_empty() { String::new() } else { format!(": {msg}") }),
    }
}

fn expect_ok(r: (u16, Value)) -> Result<Value, String> {
    if (200..300).contains(&r.0) { Ok(r.1) } else { Err(explain(r.0, &r.1)) }
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|a| a["name"].as_str().map(String::from)).collect()
}

fn track(t: &Value) -> Value {
    json!({ "name": t["name"], "artists": names(&t["artists"]), "album": t["album"]["name"], "uri": t["uri"] })
}

fn device_arg(args: &Value) -> Vec<(&str, &str)> {
    args["device_id"].as_str().filter(|d| !d.is_empty()).map(|d| vec![("device_id", d)]).unwrap_or_default()
}

/// The device to send a play command to: the one asked for, else the one last used (even if paused).
fn device_or_last(base: &str, token: &str, args: &Value) -> Option<String> {
    if let Some(d) = args["device_id"].as_str().filter(|d| !d.is_empty()) {
        return Some(d.to_string());
    }
    match api(base, token, "GET", "/v1/me/player", &[], None) {
        Ok((200, v)) => v["device"]["id"].as_str().filter(|d| !d.is_empty()).map(String::from),
        _ => None,
    }
}

fn dev_q(d: &Option<String>) -> Vec<(&str, &str)> {
    d.as_deref().map(|d| vec![("device_id", d)]).unwrap_or_default()
}

fn valid_uri(u: &str) -> bool {
    let mut p = u.split(':');
    p.next() == Some("spotify")
        && matches!(p.next(), Some("track" | "album" | "playlist" | "artist" | "show" | "episode"))
        && p.next().is_some_and(|id| !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric()))
        && p.next().is_none()
}

fn run(base: &str, token: &str, tool: &str, args: &Value) -> Result<Value, String> {
    let fixed = fix_args(base, token, tool, args)?;
    let args = &fixed;
    let ok_done = |r: Result<(u16, Value), String>| r.and_then(expect_ok).map(|_| json!({ "ok": true }));
    match tool {
        "now" => {
            let (status, v) = api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None)?;
            if status == 204 || (status == 200 && v.is_null()) {
                return Ok(json!({ "playing": false, "note": "Nothing is playing and no device is active. Open Spotify on a device." }));
            }
            let v = expect_ok((status, v))?;
            let item = &v["item"];
            Ok(json!({
                "line": describe(&v),
                "playing": v["is_playing"], "track": track(item), "progress_ms": v["progress_ms"], "duration_ms": item["duration_ms"],
                "shuffle": v["shuffle_state"], "repeat": v["repeat_state"],
                "device": { "id": v["device"]["id"], "name": v["device"]["name"], "type": v["device"]["type"], "volume": v["device"]["volume_percent"] },
            }))
        }
        "devices" => {
            let v = expect_ok(api(base, token, "GET", "/v1/me/player/devices", &[], None)?)?;
            let list: Vec<Value> = v["devices"].as_array().into_iter().flatten()
                .map(|d| json!({ "id": d["id"], "name": d["name"], "type": d["type"], "active": d["is_active"], "volume": d["volume_percent"] })).collect();
            Ok(json!({ "devices": list }))
        }
        "search" => {
            let q = args["query"].as_str().filter(|q| !q.trim().is_empty()).ok_or("missing string arg 'query'")?;
            let kinds = args["type"].as_str().filter(|t| t.split(',').all(|k| matches!(k, "track" | "album" | "artist" | "playlist"))).unwrap_or("track,album,artist,playlist");
            let v = expect_ok(api(base, token, "GET", "/v1/search", &[("q", q), ("type", kinds), ("limit", "5")], None)?)?;
            let pick = |key: &str, f: &dyn Fn(&Value) -> Value| -> Vec<Value> { v[key]["items"].as_array().into_iter().flatten().filter(|i| !i.is_null()).map(f).collect() };
            Ok(json!({
                "tracks": pick("tracks", &track),
                "albums": pick("albums", &|a| json!({ "name": a["name"], "artists": names(&a["artists"]), "uri": a["uri"] })),
                "artists": pick("artists", &|a| json!({ "name": a["name"], "uri": a["uri"] })),
                "playlists": pick("playlists", &|p| json!({ "name": p["name"], "owner": p["owner"]["display_name"], "uri": p["uri"] })),
            }))
        }
        "play_song" => play_song(base, token, args),
        "info" => info(base, token, args),
        "play_liked" => play_liked(base, token, args),
        "restore_playback" => {
            restore(base, token, &args["result"]["previous"])?;
            Ok(json!({ "ok": true }))
        }
        "play" => {
            let d = device_or_last(base, token, args);
            // Already playing: "resume" is done. (Spotify answers 403 "Restriction violated" to a play while playing.)
            if let Ok((200, v)) = api(base, token, "GET", "/v1/me/player", &[], None)
                && v["is_playing"].as_bool() == Some(true)
            {
                return Ok(json!({ "ok": true, "playing": true, "already": true, "line": describe(&v), "device": v["device"]["name"] }));
            }
            ok_done(api(base, token, "PUT", "/v1/me/player/play", &dev_q(&d), Some(&json!({}))))?;
            checked(base, token, "resume")
        }
        "play_uri" => {
            let uri = args["uri"].as_str().ok_or("missing string arg 'uri'")?;
            if !valid_uri(uri) {
                return Err("uri must look like spotify:track:<id>, spotify:album:<id>, spotify:playlist:<id> or spotify:artist:<id>".into());
            }
            let body = if uri.starts_with("spotify:track:") || uri.starts_with("spotify:episode:") { json!({ "uris": [uri] }) } else { json!({ "context_uri": uri }) };
            let d = device_or_last(base, token, args);
            ok_done(api(base, token, "PUT", "/v1/me/player/play", &dev_q(&d), Some(&body)))?;
            checked(base, token, "that link")
        }
        "pause" => {
            match api(base, token, "PUT", "/v1/me/player/pause", &device_arg(args), Some(&json!({})))? {
                (s, _) if (200..300).contains(&s) => Ok(json!({ "ok": true })),
                // Pausing what is already paused is answered 403; that is still "paused".
                (403, b) if b.to_string().contains("Restriction") => Ok(json!({ "ok": true, "already": true })),
                (s, b) => Err(explain(s, &b)),
            }
        }
        "next" => ok_done(api(base, token, "POST", "/v1/me/player/next", &device_arg(args), None)),
        "previous" => ok_done(api(base, token, "POST", "/v1/me/player/previous", &device_arg(args), None)),
        "volume" => {
            // Remember the old level so the change can be undone.
            let before = api(base, token, "GET", "/v1/me/player", &[], None).ok().and_then(|(_, v)| v["device"]["volume_percent"].as_i64());
            // `level` 0-100, or `change` (+10 / -10), or words: up, down, louder, quieter, max, mute.
            let word = args["level"].as_str().or(args["direction"].as_str()).or(args["change"].as_str()).unwrap_or("").trim().to_ascii_lowercase();
            let num = |v: &Value| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('%').parse().ok()));
            let base_level = before.unwrap_or(50);
            let level = if let Some(c) = num(&args["change"]) {
                base_level + c
            } else if let Some(l) = num(&args["level"]).filter(|_| !word.starts_with('+') && !word.starts_with('-')) {
                l
            } else if let Some(c) = num(&json!(word)) {
                base_level + c
            } else {
                match word.as_str() {
                    "up" | "louder" | "higher" | "raise" => base_level + 15,
                    "down" | "quieter" | "lower" | "softer" => base_level - 15,
                    "max" | "full" | "maximum" => 100,
                    "mute" | "off" | "silent" => 0,
                    _ => return Err("give level (0 to 100), or change (+10 or -10), or level \"up\" / \"down\"".into()),
                }
            }
            .clamp(0, 100);
            expect_ok(api(base, token, "PUT", "/v1/me/player/volume", &[("volume_percent", &level.to_string())], Some(&json!({})))?)?;
            Ok(json!({ "ok": true, "volume": level, "previous": before }))
        }
        "shuffle" => {
            let on = args["on"].as_bool().or(args["state"].as_bool()).or_else(|| args["state"].as_str().or(args["on"].as_str()).map(|s| matches!(s, "on" | "true" | "yes"))).unwrap_or(true);
            let d = device_or_last(base, token, args);
            let mut q = dev_q(&d);
            q.push(("state", if on { "true" } else { "false" }));
            expect_ok(api(base, token, "PUT", "/v1/me/player/shuffle", &q, None)?)?;
            Ok(json!({ "ok": true, "shuffle": on }))
        }
        "repeat" => {
            let mode = match args["mode"].as_str().or(args["state"].as_str()).unwrap_or("context").trim().to_ascii_lowercase().as_str() {
                "off" | "none" | "no" | "false" => "off",
                "track" | "song" | "one" | "this" => "track",
                _ => "context",
            };
            let d = device_or_last(base, token, args);
            let mut q = dev_q(&d);
            q.push(("state", mode));
            expect_ok(api(base, token, "PUT", "/v1/me/player/repeat", &q, None)?)?;
            Ok(json!({ "ok": true, "repeat": mode }))
        }
        "queue" => {
            let t = find_one(base, token, args, "track")?;
            let d = device_or_last(base, token, args);
            let mut q = dev_q(&d);
            q.push(("uri", t.uri.as_str()));
            expect_ok(api(base, token, "POST", "/v1/me/player/queue", &q, None)?)?;
            Ok(json!({ "ok": true, "queued": t.label }))
        }
        "like" => {
            let (status, v) = api(base, token, "GET", "/v1/me/player", &[], None)?;
            let id = v["item"]["id"].as_str().filter(|_| status == 200).ok_or("Nothing is playing, so there is no song to like.")?.to_string();
            let label = format!("{} - {}", v["item"]["name"].as_str().unwrap_or(""), names(&v["item"]["artists"]).join(", "));
            match api(base, token, "PUT", "/v1/me/tracks", &[("ids", &id)], Some(&json!({})))? {
                (s, _) if (200..300).contains(&s) => Ok(json!({ "ok": true, "liked": label, "id": id })),
                // The sign-in is older than the "change your library" permission.
                (403, _) => Err("Spotify has not allowed liking songs yet. Tell the user to open Settings, Integrations, Spotify, tap Connect again and allow the new permissions.".into()),
                (s, b) => Err(explain(s, &b)),
            }
        }
        "unlike" => {
            let id = args["result"]["id"].as_str().map(String::from).ok_or("there is no liked song to take back")?;
            expect_ok(api(base, token, "DELETE", "/v1/me/tracks", &[("ids", &id)], Some(&json!({})))?)?;
            Ok(json!({ "ok": true }))
        }
        "restore_volume" => {
            let prev = args["result"]["previous"].as_i64().ok_or("there is no earlier volume to go back to")?;
            ok_done(api(base, token, "PUT", "/v1/me/player/volume", &[("volume_percent", &prev.to_string())], Some(&json!({}))))
        }
        "transfer" => {
            let id = args["device_id"].as_str().filter(|d| !d.is_empty()).ok_or("missing string arg 'device_id' (see devices)")?;
            ok_done(api(base, token, "PUT", "/v1/me/player", &[], Some(&json!({ "device_ids": [id], "play": true }))))?;
            checked(base, token, "the move")
        }
        other => Err(format!("no such tool '{other}'")),
    }
}

/// Lower-case letters and digits only, so "Don't Stop!" and "dont stop" compare equal.
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// One thing Spotify found: a label for people, its uri, and (songs) the album to play it from.
struct Found {
    label: String,
    name: String,
    artists: Vec<String>,
    uri: String,
    album_uri: Option<String>,
}

fn found_from(v: &Value, key: &str) -> Vec<Found> {
    v[key]["items"].as_array().into_iter().flatten().filter(|i| i["uri"].is_string()).map(|i| {
        let name = i["name"].as_str().unwrap_or("").to_string();
        let artists = names(&i["artists"]);
        let label = if artists.is_empty() { name.clone() } else { format!("{name} - {}", artists.join(", ")) };
        Found { label, name, artists, uri: i["uri"].as_str().unwrap_or("").to_string(), album_uri: i["album"]["uri"].as_str().map(String::from) }
    }).collect()
}

/// The player's state in a small, replayable form (for putting music back).
fn snapshot(v: &Value) -> Value {
    if !v["item"]["uri"].is_string() {
        return json!({ "was_playing": false });
    }
    json!({
        "was_playing": v["is_playing"].as_bool().unwrap_or(false),
        "track_uri": v["item"]["uri"], "context_uri": v["context"]["uri"],
        "position_ms": v["progress_ms"], "device_id": v["device"]["id"],
    })
}

/// Put earlier music back. Nothing was playing before: pause.
fn restore(base: &str, token: &str, prev: &Value) -> Result<(), String> {
    let dev = prev["device_id"].as_str().filter(|d| !d.is_empty()).map(|d| vec![("device_id", d)]).unwrap_or_default();
    if prev["was_playing"].as_bool() != Some(true) || !prev["track_uri"].is_string() {
        return expect_ok(api(base, token, "PUT", "/v1/me/player/pause", &dev, Some(&json!({})))?).map(|_| ());
    }
    let mut with_context = json!({ "uris": [prev["track_uri"]] });
    if let Some(c) = prev["context_uri"].as_str().filter(|c| !c.is_empty()) {
        with_context = json!({ "context_uri": c, "offset": { "uri": prev["track_uri"] } });
    }
    let mut plain = json!({ "uris": [prev["track_uri"]] });
    for b in [&mut with_context, &mut plain] {
        if let Some(p) = prev["position_ms"].as_i64() {
            b["position_ms"] = json!(p);
        }
    }
    // Some contexts (an artist page) cannot start at a given song; the plain song always can.
    match api(base, token, "PUT", "/v1/me/player/play", &dev, Some(&with_context))? {
        (s, _) if (200..300).contains(&s) => Ok(()),
        _ => expect_ok(api(base, token, "PUT", "/v1/me/player/play", &dev, Some(&plain))?).map(|_| ()),
    }
}

/// Nothing is open: wake a device. Prefers the one last used, then one already active, then the
/// computer, a speaker, a TV, the phone.
fn wake_device(base: &str, token: &str, last_used: Option<&str>) -> Result<String, String> {
    let v = expect_ok(api(base, token, "GET", "/v1/me/player/devices", &[], None)?)?;
    let rank = |d: &Value| -> usize {
        if last_used.is_some() && d["id"].as_str() == last_used { return 0; }
        if d["is_active"].as_bool() == Some(true) { return 1; }
        2 + ["Computer", "Speaker", "CastAudio", "TV", "Smartphone"].iter().position(|t| d["type"].as_str() == Some(t)).unwrap_or(9)
    };
    let best = v["devices"].as_array().into_iter().flatten()
        .filter(|d| d["id"].is_string() && d["is_restricted"].as_bool() != Some(true))
        .min_by_key(|d| rank(d)).cloned();
    let Some(d) = best else { return Err(explain(404, &json!({ "error": { "reason": "NO_ACTIVE_DEVICE" } }))) };
    let id = d["id"].as_str().unwrap_or("").to_string();
    expect_ok(api(base, token, "PUT", "/v1/me/player", &[], Some(&json!({ "device_ids": [id], "play": false })))?)?;
    Ok(id)
}

/// After a song starts, queue up to 5 more by the same artist so the music does not stop. Best effort.
fn queue_more(base: &str, token: &str, target: &Found, device: Option<&str>) -> usize {
    let Some(artist) = target.artists.first() else { return 0 };
    let q = format!("artist:\"{}\"", artist.replace('"', ""));
    let Ok((200, v)) = api(base, token, "GET", "/v1/search", &[("q", &q), ("type", "track"), ("limit", "10")], None) else { return 0 };
    let mut seen = vec![norm(&target.name)];
    let mut n = 0;
    for f in found_from(&v, "tracks") {
        if n == 5 { break; }
        let key = norm(&f.name);
        if f.uri == target.uri || seen.contains(&key) || !f.artists.iter().any(|a| a == artist) { continue; }
        seen.push(key);
        let mut query = vec![("uri", f.uri.as_str())];
        if let Some(d) = device { query.push(("device_id", d)); }
        if matches!(api(base, token, "POST", "/v1/me/player/queue", &query, None), Ok((s, _)) if (200..300).contains(&s)) { n += 1; }
    }
    n
}

/// The user's own Spotify: liked songs, top tracks or artists, recent plays, playlists.
/// `what`: now | devices | liked | top | top_artists | recent | playlists.
fn info(base: &str, token: &str, args: &Value) -> Result<Value, String> {
    let what = args["what"].as_str().unwrap_or("now");
    let limit = args["limit"].as_i64().unwrap_or(20).clamp(1, 50).to_string();
    let range = match args["time_range"].as_str().unwrap_or("medium") {
        "short" | "short_term" | "month" => "short_term",
        "long" | "long_term" | "all" | "years" => "long_term",
        _ => "medium_term",
    };
    let get = |path: &str, q: &[(&str, &str)]| -> Result<Value, String> { expect_ok(api(base, token, "GET", path, q, None)?) };
    match what {
        "now" | "devices" => run(base, token, what, &json!({})),
        "liked" => {
            let v = get("/v1/me/tracks", &[("limit", &limit)])?;
            let items: Vec<Value> = v["items"].as_array().into_iter().flatten().map(|i| { let mut t = track(&i["track"]); t["added"] = i["added_at"].clone(); t }).collect();
            Ok(json!({ "what": "liked", "total": v["total"], "tracks": items }))
        }
        "top" | "top_tracks" => {
            let v = get("/v1/me/top/tracks", &[("limit", &limit), ("time_range", range)])?;
            Ok(json!({ "what": "top tracks", "time_range": range, "tracks": v["items"].as_array().into_iter().flatten().map(track).collect::<Vec<_>>() }))
        }
        "top_artists" => {
            let v = get("/v1/me/top/artists", &[("limit", &limit), ("time_range", range)])?;
            let a: Vec<Value> = v["items"].as_array().into_iter().flatten().map(|a| json!({ "name": a["name"], "genres": a["genres"], "uri": a["uri"] })).collect();
            Ok(json!({ "what": "top artists", "time_range": range, "artists": a }))
        }
        "recent" => {
            let v = get("/v1/me/player/recently-played", &[("limit", &limit)])?;
            let items: Vec<Value> = v["items"].as_array().into_iter().flatten().map(|i| { let mut t = track(&i["track"]); t["played_at"] = i["played_at"].clone(); t }).collect();
            Ok(json!({ "what": "recently played", "tracks": items }))
        }
        "playlists" => {
            let v = get("/v1/me/playlists", &[("limit", &limit)])?;
            let p: Vec<Value> = v["items"].as_array().into_iter().flatten().filter(|p| !p.is_null())
                .map(|p| json!({ "name": p["name"], "owner": p["owner"]["display_name"], "tracks": p["tracks"]["total"], "uri": p["uri"] })).collect();
            Ok(json!({ "what": "your playlists", "total": v["total"], "playlists": p }))
        }
        other => Err(format!("what '{other}' is not one of now, devices, liked, top, top_artists, recent, playlists")),
    }
}

/// Spotify says OK even when nothing plays. Look a few times; true when `want` is what plays.
fn sound_started(base: &str, token: &str, want: &str, by_context: bool) -> Option<Value> {
    let gap = Duration::from_millis(VERIFY_GAP_MS.load(Ordering::Relaxed));
    for _ in 0..VERIFY_TRIES {
        std::thread::sleep(gap);
        let Ok((200, v)) = api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None) else { continue };
        let here = if by_context { v["context"]["uri"].as_str() } else { v["item"]["uri"].as_str() };
        if v["is_playing"].as_bool() == Some(true) && here == Some(want) {
            return Some(v);
        }
    }
    None
}

/// After a plain play command: look until sound is on (any song), so "resume" is never a guess.
fn checked(base: &str, token: &str, what: &str) -> Result<Value, String> {
    let gap = Duration::from_millis(VERIFY_GAP_MS.load(Ordering::Relaxed));
    for _ in 0..4 {
        std::thread::sleep(gap);
        if let Ok((200, v)) = api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None)
            && v["is_playing"].as_bool() == Some(true)
        {
            return Ok(json!({ "ok": true, "playing": true, "line": describe(&v), "device": v["device"]["name"] }));
        }
    }
    Err(format!("Spotify took {what} but no sound started. Ask the user to open Spotify on the device they want and press play once. Do not say it is playing."))
}

/// Play the user's liked songs, shuffled, on the last-used device; checked like play_song.
fn play_liked(base: &str, token: &str, args: &Value) -> Result<Value, String> {
    // Up to 200 liked songs (4 pages), newest first.
    let mut liked: Vec<Found> = vec![];
    for page in 0..4 {
        let off = (page * 50).to_string();
        let v = expect_ok(api(base, token, "GET", "/v1/me/tracks", &[("limit", "50"), ("offset", &off)], None)?)?;
        let items = v["items"].as_array().cloned().unwrap_or_default();
        liked.extend(items.iter().filter(|i| i["track"]["uri"].is_string()).map(|i| {
            let t = &i["track"];
            let name = t["name"].as_str().unwrap_or("").to_string();
            let artists = names(&t["artists"]);
            Found { label: format!("{name} - {}", artists.join(", ")), name, artists, uri: t["uri"].as_str().unwrap_or("").to_string(), album_uri: None }
        }));
        if items.len() < 50 { break; }
    }
    if liked.is_empty() {
        return Err("There are no liked songs on this Spotify account yet.".into());
    }
    // "Play my liked songs by Drake": only that artist's. "Play X from my liked songs": X first.
    let artist = args["artist"].as_str().map(norm).filter(|a| !a.is_empty());
    let first = args["query"].as_str().map(norm).filter(|q| !q.is_empty());
    if let Some(a) = &artist {
        liked.retain(|f| f.artists.iter().any(|x| norm(x).contains(a.as_str())));
        if liked.is_empty() {
            return Err(format!("None of the user's liked songs are by \"{}\". Use play_song with kind artist to play that artist instead.", args["artist"].as_str().unwrap_or("")));
        }
    }
    let start = match &first {
        Some(q) => Some(liked.iter().position(|f| norm(&f.name) == *q).or_else(|| liked.iter().position(|f| norm(&f.name).contains(q.as_str())))
            .ok_or_else(|| {
                let some: Vec<String> = liked.iter().take(8).map(|f| f.label.clone()).collect();
                format!("\"{}\" is not in the user's liked songs (looked at the newest {}). Use play_song to play it from all of Spotify. Some liked songs: {}.", args["query"].as_str().unwrap_or(""), liked.len(), some.join("; "))
            })?),
        None => None,
    };
    let picked = start.map(|i| liked.remove(i));
    let mut uris: Vec<String> = liked.into_iter().map(|f| f.uri).collect();
    // Shuffle with a few random bytes (no extra crate).
    let seed = crate::util::random_hex(8);
    let mut r = u64::from_str_radix(&seed, 16).unwrap_or(7) | 1;
    for i in (1..uris.len()).rev() {
        r ^= r << 13; r ^= r >> 7; r ^= r << 17;
        uris.swap(i, (r % (i as u64 + 1)) as usize);
    }
    if let Some(p) = &picked {
        uris.insert(0, p.uri.clone());
    }
    uris.truncate(100);
    let what = match (&picked, &artist) {
        (Some(p), _) => format!("{} from your liked songs, then more of them shuffled", p.label),
        (None, Some(_)) => format!("your liked songs by {}, shuffled", args["artist"].as_str().unwrap_or("")),
        _ => "your liked songs, shuffled".to_string(),
    };
    let (before, last_device) = match api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None)? {
        (200, p) => (snapshot(&p), p["device"]["id"].as_str().filter(|d| !d.is_empty()).map(String::from)),
        _ => (json!({ "was_playing": false }), None),
    };
    let asked = args["device_id"].as_str().filter(|d| !d.is_empty()).map(String::from);
    let mut device = asked.clone().or(last_device.clone());
    let body = json!({ "uris": uris });
    let mut r = api(base, token, "PUT", "/v1/me/player/play", &dev_q(&device), Some(&body))?;
    if r.0 == 404 && asked.is_none() {
        device = Some(wake_device(base, token, last_device.as_deref())?);
        r = api(base, token, "PUT", "/v1/me/player/play", &dev_q(&device), Some(&body))?;
    }
    if !(200..300).contains(&r.0) {
        return Err(explain(r.0, &r.1));
    }
    if let Some(p) = sound_started(base, token, &uris[0], false) {
        return Ok(json!({ "ok": true, "playing": true, "what": what, "songs": uris.len(), "device": p["device"]["name"], "line": describe(&p), "previous": before }));
    }
    let back = restore(base, token, &before);
    Err(format!("Spotify accepted {what} but no sound started. {} Ask the user to open Spotify on the device they want, then try again. Do not say it is playing.",
        if back.is_ok() { "The earlier music was put back." } else { "The earlier music could not be put back." }))
}

/// The user's own playlist with this name, if any (before searching everyone's).
fn own_playlist(base: &str, token: &str, name: &str) -> Option<Found> {
    let (200, v) = api(base, token, "GET", "/v1/me/playlists", &[("limit", "50")], None).ok()? else { return None };
    let want = norm(name);
    let list = found_from(&json!({ "p": v }), "p");
    list.iter().position(|f| norm(&f.name) == want).or_else(|| list.iter().position(|f| norm(&f.name).contains(&want)))
        .map(|i| Found { label: format!("{} (your playlist)", list[i].name), name: list[i].name.clone(), artists: vec![], uri: list[i].uri.clone(), album_uri: None })
}

/// Tool names small models make up, mapped to the real tool (and `kind`). The gate then judges the
/// real tool, so an alias can never reach more than its target.
pub fn alias(tool: &str, mut args: Value) -> (String, Value) {
    let t = tool.trim().to_ascii_lowercase().replace(['-', ' ', '.'], "_");
    let t = t.strip_prefix("spotify_").unwrap_or(&t).to_string();
    let mut kind = |k: &str| { if args.is_object() && args["kind"].is_null() { args["kind"] = json!(k); } };
    let real = match t.as_str() {
        "play_track" | "play_music" | "play_search" | "search_and_play" | "play_by_name" | "play_name" => "play_song",
        "play_artist" => { kind("artist"); "play_song" }
        "play_album" => { kind("album"); "play_song" }
        "play_playlist" => { kind("playlist"); "play_song" }
        "play_liked_songs" | "play_favorites" | "play_favourites" | "liked_songs" | "play_saved" | "play_library" => "play_liked",
        "resume" | "unpause" | "continue" | "start" => "play",
        "stop" => "pause",
        "skip" | "next_track" | "skip_track" | "skip_song" | "forward" => "next",
        "back" | "previous_track" | "prev" | "rewind" | "last" => "previous",
        "set_volume" | "volume_up" | "volume_down" | "louder" | "quieter" => {
            if args.is_object() && args["level"].is_null() && args["change"].is_null() {
                if t.ends_with("up") || t == "louder" { args["level"] = json!("up"); }
                if t.ends_with("down") || t == "quieter" { args["level"] = json!("down"); }
            }
            "volume"
        }
        "now_playing" | "current" | "current_track" | "currently_playing" | "status" | "what_is_playing" | "get_current" => "now",
        "add_to_queue" | "queue_song" | "enqueue" | "play_next" => "queue",
        "save" | "like_song" | "save_song" | "heart" | "add_to_liked" => "like",
        "get_devices" | "list_devices" => "devices",
        "set_shuffle" | "toggle_shuffle" => "shuffle",
        "set_repeat" | "loop" => "repeat",
        "transfer_playback" | "switch_device" | "move_to" => "transfer",
        "my_playlists" | "playlists" | "top_tracks" | "recent" | "recently_played" | "top_artists" | "liked" => {
            let what = match t.as_str() { "my_playlists" | "playlists" => "playlists", "top_tracks" => "top", "recently_played" | "recent" => "recent", other => other };
            if args.is_object() && args["what"].is_null() { args["what"] = json!(what); }
            "info"
        }
        _ => return (tool.to_string(), args),
    };
    (real.to_string(), args)
}

/// Find exactly one song, album, artist or playlist (`stype`: track | album | artist | playlist)
/// from `query` and `artist`. An exact name beats one that only starts the same ("Drake" never
/// plays "Draken"). Nothing right: the error lists the closest real ones, so the model picks one
/// instead of making a name up.
fn find_one(base: &str, token: &str, args: &Value, stype: &str) -> Result<Found, String> {
    let clean = |s: &str| s.replace('"', "").trim().to_string();
    let q = args["query"].as_str().map(clean).filter(|q| !q.is_empty()).ok_or("missing string arg 'query' (the song, album, artist or playlist name)")?;
    let artist = args["artist"].as_str().map(clean).filter(|a| !a.is_empty());
    let (kind, key) = match stype {
        "track" => ("song", "tracks"),
        "album" => ("album", "albums"),
        "artist" => ("artist", "artists"),
        _ => ("playlist", "playlists"),
    };
    // "Play my workout playlist": the user's own playlist wins over a stranger's with that name.
    if stype == "playlist"
        && let Some(f) = own_playlist(base, token, &q)
    {
        return Ok(f);
    }
    let by = artist.as_ref().map(|a| format!(" by {a}")).unwrap_or_default();
    // A precise search, so a device name or a mood word never leaks into the song name.
    let precise = match (stype, &artist) {
        ("track", Some(a)) => format!("track:\"{q}\" artist:\"{a}\""),
        ("track", None) => format!("track:\"{q}\""),
        ("album", Some(a)) => format!("album:\"{q}\" artist:\"{a}\""),
        ("album", None) => format!("album:\"{q}\""),
        ("artist", _) => format!("artist:\"{q}\""),
        _ => q.clone(),
    };
    let search = |text: &str| -> Result<Vec<Found>, String> {
        let v = expect_ok(api(base, token, "GET", "/v1/search", &[("q", text), ("type", stype), ("limit", "10")], None)?)?;
        Ok(found_from(&v, key))
    };
    let want = norm(&q);
    let want_artist = artist.as_deref().map(norm);
    let artist_ok = |f: &Found| want_artist.as_ref().is_none_or(|a| f.artists.iter().any(|x| norm(x).contains(a.as_str())));
    let found = search(&precise)?;
    let exact = found.iter().position(|f| norm(&f.name) == want && artist_ok(f));
    // Close enough only for songs, albums and playlists ("Espresso" finds "Espresso - Remix" last).
    let near = || found.iter().position(|f| {
        let n = norm(&f.name);
        let ok = match stype { "artist" => false, "playlist" => n.contains(&want), _ => n.starts_with(&want) };
        ok && artist_ok(f)
    });
    if let Some(i) = exact.or_else(near) {
        let mut found = found;
        return Ok(found.swap_remove(i));
    }
    let close: Vec<String> = search(&format!("{q}{}", artist.as_ref().map(|a| format!(" {a}")).unwrap_or_default()))?.into_iter().take(5).map(|f| f.label).collect();
    Err(if close.is_empty() {
        format!("Spotify has no {kind} called \"{q}\"{by} and nothing close. Ask the user to check the spelling. Do not make up a name.")
    } else {
        format!("Spotify has no {kind} called \"{q}\"{by}. Closest real ones: {}. Pick one of these or ask the user. Do not make up a name or an artist.", close.join("; "))
    })
}

/// Small models send near-miss argument names (`song`, `artist_name`, a device name). Map them to
/// the real ones so a good request is not lost. Also turns a device name into its id.
fn fix_args(base: &str, token: &str, tool: &str, args: &Value) -> Result<Value, String> {
    let mut a = if args.is_object() { args.clone() } else { json!({}) };
    let get = |a: &Value, k: &str| -> Option<String> {
        match &a[k] {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Array(v) => v.iter().filter_map(|x| x.as_str()).next().map(String::from),
            _ => None,
        }
    };
    if get(&a, "query").is_none() {
        for k in ["song", "title", "track", "name", "song_name", "track_name", "song_title", "q", "search", "album", "playlist", "album_name", "playlist_name"] {
            if let Some(v) = get(&a, k) {
                a["query"] = json!(v);
                if a["kind"].is_null() && (k.starts_with("album") || k.starts_with("playlist")) {
                    a["kind"] = json!(if k.starts_with("album") { "album" } else { "playlist" });
                }
                break;
            }
        }
    }
    if get(&a, "artist").is_none() {
        for k in ["artist_name", "artists", "by", "singer", "band"] {
            if let Some(v) = get(&a, k) {
                a["artist"] = json!(v);
                break;
            }
        }
    }
    // Only an artist named ("play some Drake"): play that artist.
    if tool == "play_song" && get(&a, "query").is_none()
        && let Some(ar) = get(&a, "artist")
    {
        a["query"] = json!(ar);
        a["kind"] = json!("artist");
        a.as_object_mut().map(|o| o.remove("artist"));
    }
    let kind = a["kind"].as_str().map(|k| k.trim().to_ascii_lowercase());
    if let Some(k) = kind {
        a["kind"] = json!(match k.as_str() {
            "track" | "tracks" | "songs" | "music" | "single" | "" => "song",
            "artists" | "band" | "singer" => "artist",
            "albums" | "record" => "album",
            "playlists" | "list" => "playlist",
            other => other,
        });
    }
    // A device by name: "the kitchen speaker", "my phone".
    let looks_like_id = |d: &str| d.len() >= 32 && d.chars().all(|c| c.is_ascii_alphanumeric());
    let named = get(&a, "device_name").or_else(|| get(&a, "device")).or_else(|| get(&a, "device_id").filter(|d| !looks_like_id(d)));
    if let Some(name) = named {
        let v = expect_ok(api(base, token, "GET", "/v1/me/player/devices", &[], None)?)?;
        let list: Vec<&Value> = v["devices"].as_array().into_iter().flatten().filter(|d| d["id"].is_string()).collect();
        let want = norm(&name);
        let phone = ["phone", "myphone", "mobile", "cell"].contains(&want.as_str());
        let hit = list.iter().find(|d| norm(d["name"].as_str().unwrap_or("")) == want)
            .or_else(|| list.iter().find(|d| { let n = norm(d["name"].as_str().unwrap_or("")); !want.is_empty() && (n.contains(&want) || want.contains(&n)) }))
            .or_else(|| if phone { list.iter().find(|d| d["type"] == "Smartphone") } else { None })
            .or_else(|| list.iter().find(|d| norm(d["type"].as_str().unwrap_or("")) == want));
        match hit {
            Some(d) => a["device_id"] = d["id"].clone(),
            None => {
                let names: Vec<&str> = list.iter().filter_map(|d| d["name"].as_str()).collect();
                return Err(format!("There is no Spotify device called \"{name}\". Devices Spotify can see now: {}. Use one of these, or leave the device out to use the last one.",
                    if names.is_empty() { "none (open Spotify on a device)".to_string() } else { names.join(", ") }));
            }
        }
    }
    Ok(a)
}

/// Play one named thing, and only say it worked when sound really started.
fn play_song(base: &str, token: &str, args: &Value) -> Result<Value, String> {
    let kind = args["kind"].as_str().unwrap_or("song");
    let stype = match kind {
        "song" => "track",
        "album" | "artist" | "playlist" => kind,
        _ => return Err("kind must be song, album, artist or playlist".into()),
    };
    let target = find_one(base, token, args, stype)?;
    // 3. Remember what was playing, so a failed try can put it back.
    let (before, last_device) = match api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None)? {
        (200, v) => (snapshot(&v), v["device"]["id"].as_str().filter(|d| !d.is_empty()).map(String::from)),
        _ => (json!({ "was_playing": false }), None),
    };
    let body = match (stype, &target.album_uri) {
        ("track", Some(album)) => json!({ "context_uri": album, "offset": { "uri": target.uri } }),
        ("track", None) => json!({ "uris": [target.uri] }),
        _ => json!({ "context_uri": target.uri }),
    };
    // Always name the device. Without one, Spotify picks "the active device", which a phone stops
    // being soon after it pauses, so the music lands nowhere (or somewhere else).
    let asked = args["device_id"].as_str().filter(|d| !d.is_empty()).map(String::from);
    let mut device = asked.clone().or(last_device.clone());
    let start = |device: &Option<String>| {
        let dev: Vec<(&str, &str)> = device.as_deref().map(|d| vec![("device_id", d)]).unwrap_or_default();
        api(base, token, "PUT", "/v1/me/player/play", &dev, Some(&body))
    };
    let mut r = start(&device)?;
    if r.0 == 404 && asked.is_none() {
        device = Some(wake_device(base, token, last_device.as_deref())?);
        r = start(&device)?;
    }
    if !(200..300).contains(&r.0) {
        return Err(explain(r.0, &r.1));
    }
    // 4. Spotify says OK even when nothing plays. Look, a few times.
    let gap = Duration::from_millis(VERIFY_GAP_MS.load(Ordering::Relaxed));
    let mut seen = String::from("nothing");
    for _ in 0..VERIFY_TRIES {
        std::thread::sleep(gap);
        let Ok((200, v)) = api(base, token, "GET", "/v1/me/player", &[("additional_types", "episode")], None) else { continue };
        seen = format!("playing={} song={} context={}", v["is_playing"], v["item"]["uri"].as_str().unwrap_or("-"), v["context"]["uri"].as_str().unwrap_or("-"));
        // Spotify does not always report the artist or album as the context. A song by that artist
        // (or from that album) playing is proof enough.
        let want = target.uri.as_str();
        let here = match stype {
            "track" => v["item"]["uri"].as_str() == Some(want),
            "artist" => v["context"]["uri"].as_str() == Some(want) || v["item"]["artists"].as_array().is_some_and(|a| a.iter().any(|x| x["uri"].as_str() == Some(want))),
            "album" => v["context"]["uri"].as_str() == Some(want) || v["item"]["album"]["uri"].as_str() == Some(want),
            _ => v["context"]["uri"].as_str() == Some(want),
        };
        if v["is_playing"].as_bool() == Some(true) && here {
            // A one-song album (a single) ends after that song and the player stops. Queue more by the artist.
            let queued = if stype == "track" { queue_more(base, token, &target, device.as_deref()) } else { 0 };
            return Ok(json!({ "ok": true, "playing": true, "what": target.label, "device": v["device"]["name"], "line": describe(&v), "queued_after": queued, "previous": before }));
        }
    }
    // 5. No sound. Put the old music back and say so plainly.
    let back = restore(base, token, &before);
    Err(format!(
        "Spotify accepted the request for \"{}\" but no sound started. {} Ask the user to open Spotify on the device they want, play anything for a second, then try again. Do not say it is playing. (Player showed: {seen})",
        target.label,
        if back.is_ok() { "The earlier music was put back." } else { "The earlier music could not be put back." },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_without_punctuation_and_the_line_reads_plainly() {
        assert_eq!(norm("Don't Stop!"), norm("dont stop"));
        let v = json!({ "is_playing": true, "item": { "name": "Song", "artists": [{ "name": "Band" }] }, "device": { "name": "Pixel", "volume_percent": 40 } });
        assert_eq!(describe(&v), "Spotify: playing Song by Band on Pixel, volume 40%.");
        assert_eq!(describe(&json!({})), "Spotify: nothing playing.");
        assert_eq!(snapshot(&json!({})), json!({ "was_playing": false }));
    }

    #[test]
    fn only_real_spotify_uris_pass() {
        assert!(valid_uri("spotify:track:4uLU6hMCjMI75M1A2tKUQC") && valid_uri("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M"));
        assert!(!valid_uri("spotify:track:") && !valid_uri("https://open.spotify.com/track/x") && !valid_uri("spotify:user:x") && !valid_uri("spotify:track:a:b") && !valid_uri("spotify:track:../x"));
    }

    #[test]
    fn complaints_become_plain_sentences() {
        assert!(explain(403, &json!({ "error": { "reason": "PREMIUM_REQUIRED" } })).contains("Premium"));
        assert!(explain(404, &json!({ "error": { "reason": "NO_ACTIVE_DEVICE", "message": "Player command failed: No active device found" } })).contains("Open Spotify"));
        assert!(explain(401, &Value::Null).contains("again"));
    }
}
