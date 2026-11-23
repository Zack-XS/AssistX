//! Travel time and "leave now" alerts (plugin `location`, tool `travel_time`; docs/10, "Travel time").
//!
//! Where to: a place the user named (Gym), else the text is looked up on OpenStreetMap (Nominatim
//! `/search`, the same host `location.rs` uses for names; one request a second, answers cached a
//! month). How long: the public OSRM router at one fixed host, `router.project-osrm.org`
//! (`AGENT_ROUTING_BASE` changes it). The public router only drives; walking and cycling times come
//! from the driving route's length at 5 km/h and 15 km/h. No key, no account.
//!
//! On the core timer, for each calendar event with a location starting in the next 4 hours, the core
//! works out the trip from where the phone is now (a point from the last 2 hours) and, when
//! `start - travel - 10 min` is reached, sends "Leave in 10 min for X (25 min drive)", once per event.

use crate::{App, scheduler, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS travel_geo (query TEXT PRIMARY KEY, lat REAL, lon REAL, name TEXT, ts INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS travel_alerts (key TEXT PRIMARY KEY, event_id TEXT, title TEXT, start INTEGER NOT NULL, travel_secs INTEGER, mode TEXT, checked INTEGER NOT NULL, sent INTEGER);
"#;

pub const MODES: &[&str] = &["driving", "walking", "cycling"];
/// Look this far ahead for events.
const AHEAD_SECS: i64 = 4 * 3600;
/// Work the trip out again after this long (traffic, the user moved).
const RECHECK_SECS: i64 = 15 * 60;
/// The phone's last point must be this fresh to say "from here".
const FRESH_SECS: i64 = 2 * 3600;
/// Already there.
const THERE_M: f64 = 300.0;

const UA: &str = "self-hosted-personal-assistant/0.1 (one user, a few requests a day)";

// ---------------- pure parts ----------------

/// When the alert goes out: the time to leave, minus the warning.
pub fn alert_at(start: i64, travel_secs: i64, buffer_secs: i64) -> i64 {
    start - travel_secs - buffer_secs
}

/// Send now? Not before the alert time, not after the event started, and only once.
pub fn should_alert(now: i64, start: i64, travel_secs: i64, buffer_secs: i64, sent: bool) -> bool {
    !sent && now >= alert_at(start, travel_secs, buffer_secs) && now < start
}

/// Seconds for the trip by `mode`, from the driving route (`drive_secs`, `meters`).
pub fn mode_secs(mode: &str, drive_secs: f64, meters: f64) -> i64 {
    let s = match mode {
        "walking" => meters / (5.0 / 3.6),
        "cycling" => meters / (15.0 / 3.6),
        _ => drive_secs,
    };
    s.round().max(60.0) as i64
}

fn trip_word(mode: &str) -> &'static str {
    match mode {
        "walking" => "walk",
        "cycling" => "bike ride",
        _ => "drive",
    }
}

/// "Leave in 10 min for Dentist (25 min drive)", or "Leave now ..." when the time to leave has come.
pub fn alert_text(title: &str, now: i64, start: i64, travel_secs: i64, mode: &str) -> String {
    let leave_in = (start - travel_secs - now) / 60;
    let trip = format!("{} min {}", (travel_secs + 30) / 60, trip_word(mode));
    let title = util::clean_line(title, 10);
    if leave_in <= 0 { format!("Leave now for {title} ({trip})") } else { format!("Leave in {leave_in} min for {title} ({trip})") }
}

// ---------------- settings ----------------

/// `{enabled, mode, buffer_min}`. On by default, driving, 10 minutes.
pub fn settings(app: &App) -> Value {
    let saved: Value = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'travel.settings'", [], |r| r.get::<_, String>(0)).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    json!({ "enabled": saved["enabled"].as_bool().unwrap_or(true), "mode": saved["mode"].as_str().filter(|m| MODES.contains(m)).unwrap_or("driving"),
        "buffer_min": saved["buffer_min"].as_i64().unwrap_or(10).clamp(0, 60) })
}

pub fn patch_settings(app: &App, b: &Value) -> Result<Value, String> {
    let mut s = settings(app);
    if let Some(e) = b.get("enabled") {
        s["enabled"] = json!(e.as_bool().ok_or("enabled is true or false")?);
    }
    if let Some(m) = b.get("mode") {
        s["mode"] = json!(m.as_str().filter(|m| MODES.contains(m)).ok_or("mode is driving, walking or cycling")?);
    }
    if let Some(m) = b.get("buffer_min") {
        s["buffer_min"] = json!(m.as_i64().filter(|m| (0..=60).contains(m)).ok_or("buffer_min is 0 to 60")?);
    }
    app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('travel.settings', ?1)", [s.to_string()]).map_err(|e| e.to_string())?;
    Ok(s)
}

// ---------------- where and how long ----------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(12))).http_status_as_error(false).build().into()
}

fn get_json(url: &str) -> Result<Value, String> {
    let mut r = agent().get(url).header("User-Agent", UA).header("Accept-Language", "en").call().map_err(|e| e.to_string())?;
    if r.status().as_u16() != 200 {
        return Err(format!("the service answered {}", r.status().as_u16()));
    }
    serde_json::from_str(&r.body_mut().read_to_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

static NOMINATIM: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A place the user named, else a cached lookup, else OpenStreetMap. `(lat, lon, name)`.
pub fn resolve(app: &App, to: &str) -> Result<(f64, f64, String), String> {
    let to = to.trim();
    if to.is_empty() {
        return Err("to is required: a place name or an address".into());
    }
    if let Ok((id, name)) = crate::location::find_place(app, to)
        && let Some(p) = crate::location::get_place(app, &id)
        && let (Some(lat), Some(lon)) = (p["lat"].as_f64(), p["lon"].as_f64())
    {
        return Ok((lat, lon, name));
    }
    let key = to.to_lowercase();
    let cached: Option<(Option<f64>, Option<f64>, Option<String>, i64)> = app.db.lock()
        .query_row("SELECT lat, lon, name, ts FROM travel_geo WHERE query = ?1", [&key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().ok().flatten();
    let now = util::unix_now();
    if let Some((lat, lon, name, ts)) = cached {
        match (lat, lon) {
            (Some(la), Some(lo)) if now - ts < 30 * 86_400 => return Ok((la, lo, name.unwrap_or_else(|| to.into()))),
            (None, _) if now - ts < 86_400 => return Err(format!("OpenStreetMap does not know '{to}'. Try a fuller address.")),
            _ => {}
        }
    }
    let _one = NOMINATIM.lock().unwrap_or_else(|e| e.into_inner());
    let url = format!("{}/search?format=jsonv2&limit=1&q={}", app.cfg.geocode_base, enc(to));
    let v = get_json(&url);
    std::thread::sleep(Duration::from_millis(1100)); // Nominatim's rule: at most one request a second
    let v = v.map_err(|e| format!("could not look up '{to}': {e}"))?;
    let hit = v.get(0);
    let lat = hit.and_then(|h| h["lat"].as_str()).and_then(|s| s.parse::<f64>().ok());
    let lon = hit.and_then(|h| h["lon"].as_str()).and_then(|s| s.parse::<f64>().ok());
    let name: Option<String> = hit.and_then(|h| h["display_name"].as_str()).map(|s| s.chars().take(200).collect());
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO travel_geo (query, lat, lon, name, ts) VALUES (?1,?2,?3,?4,?5)", params![key, lat, lon, name, now]);
    match (lat, lon) {
        (Some(la), Some(lo)) => Ok((la, lo, name.unwrap_or_else(|| to.into()))),
        _ => Err(format!("OpenStreetMap does not know '{to}'. Try a fuller address.")),
    }
}

/// The driving route between two points: `(seconds, metres)`.
pub fn route(from: (f64, f64), to: (f64, f64)) -> Result<(f64, f64), String> {
    let base = std::env::var("AGENT_ROUTING_BASE").unwrap_or_else(|_| "https://router.project-osrm.org".into());
    let url = format!("{base}/route/v1/driving/{:.6},{:.6};{:.6},{:.6}?overview=false", from.1, from.0, to.1, to.0);
    let v = get_json(&url).map_err(|e| format!("the router: {e}"))?;
    parse_route(&v)
}

pub fn parse_route(v: &Value) -> Result<(f64, f64), String> {
    if v["code"] != "Ok" {
        return Err(format!("no route found ({})", v["code"].as_str().unwrap_or("unknown")));
    }
    let r = &v["routes"][0];
    Ok((r["duration"].as_f64().ok_or("no route found")?, r["distance"].as_f64().ok_or("no route found")?))
}

fn here(app: &App) -> Option<(f64, f64)> {
    let (ts, lat, lon, _) = crate::location::last_point(app)?;
    (util::unix_now() - ts <= FRESH_SECS).then_some((lat, lon))
}

/// Tool `travel_time {to, mode?}`: from where the phone is now.
pub fn travel_time(app: &App, a: &Value) -> Result<Value, String> {
    let mode = a["mode"].as_str().filter(|m| !m.is_empty()).unwrap_or(settings(app)["mode"].as_str().unwrap_or("driving")).to_string();
    if !MODES.contains(&mode.as_str()) {
        return Err("mode is driving, walking or cycling".into());
    }
    let from = here(app).ok_or("I do not know where the user is now (no phone location in the last 2 hours). Ask where they start from, or turn Location on in the app.")?;
    let (lat, lon, name) = resolve(app, a["to"].as_str().unwrap_or(""))?;
    let straight = crate::location::dist_m(from, (lat, lon));
    let (secs, meters) = if straight < 50.0 { (0.0, straight) } else { route(from, (lat, lon))? };
    let t = if straight < 50.0 { 0 } else { mode_secs(&mode, secs, meters) };
    let now = util::unix_now();
    Ok(json!({ "to": name, "mode": mode, "minutes": (t + 30) / 60, "km": (meters / 100.0).round() / 10.0,
        "arrive_if_leaving_now": scheduler::local_str(app, now + t),
        "note": if mode == "driving" { "Driving time from OpenStreetMap routing, without live traffic." } else { "Estimated from the road distance at a steady pace." } }))
}

// ---------------- the leave-now check ----------------

static CHECKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Called by the core timer; the work runs off the async threads, once a minute at most.
pub fn maybe_check(app: &Arc<App>) {
    if !app.due("travel.check", 60) || settings(app)["enabled"] != true || !app.plugin_usable("calendar") {
        return;
    }
    let Ok(rt) = tokio::runtime::Handle::try_current() else { return };
    let app = app.clone();
    rt.spawn_blocking(move || {
        let Ok(_one) = CHECKING.try_lock() else { return };
        check(&app);
    });
}

fn check(app: &App) {
    let now = util::unix_now();
    let _ = app.db.lock().execute("DELETE FROM travel_alerts WHERE start < ?1", [now - 2 * 86_400]);
    let Some(from) = here(app) else { return };
    let s = settings(app);
    let (mode, buffer) = (s["mode"].as_str().unwrap_or("driving").to_string(), s["buffer_min"].as_i64().unwrap_or(10) * 60);
    let Ok(list) = crate::calendar::list(app, None, Some(&scheduler::local_str(app, now + AHEAD_SECS)), None) else { return };
    let z = scheduler::zone(&app.cfg.timezone);
    for e in list["events"].as_array().cloned().unwrap_or_default() {
        let Some(place) = e["location"].as_str().map(str::trim).filter(|l| !l.is_empty()) else { continue };
        if e["all_day"] == true {
            continue;
        }
        let Some(start) = e["start"].as_str().and_then(scheduler::parse_local).map(|l| z.local_to_utc(l)) else { continue };
        if start <= now {
            continue;
        }
        let title = e["title"].as_str().unwrap_or("your event").to_string();
        let key = format!("{}@{}", e["event_id"].as_str().unwrap_or(""), start);
        let row: Option<(Option<i64>, i64, Option<i64>)> = app.db.lock()
            .query_row("SELECT travel_secs, checked, sent FROM travel_alerts WHERE key = ?1", [&key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().ok().flatten();
        if row.as_ref().is_some_and(|r| r.2.is_some()) {
            continue; // sent (or skipped) already: once per event
        }
        let mut travel = row.as_ref().and_then(|r| r.0);
        if row.as_ref().is_none_or(|r| now - r.1 >= RECHECK_SECS) {
            travel = match resolve(app, place) {
                Ok((lat, lon, _)) if crate::location::dist_m(from, (lat, lon)) < THERE_M => {
                    // Already there: nothing to say.
                    let _ = app.db.lock().execute("INSERT OR REPLACE INTO travel_alerts (key, event_id, title, start, travel_secs, mode, checked, sent) VALUES (?1,?2,?3,?4,0,?5,?6,0)",
                        params![key, e["event_id"].as_str(), title, start, mode, now]);
                    continue;
                }
                Ok((lat, lon, _)) => route(from, (lat, lon)).ok().map(|(secs, m)| mode_secs(&mode, secs, m)),
                Err(_) => None,
            };
            let _ = app.db.lock().execute("INSERT OR REPLACE INTO travel_alerts (key, event_id, title, start, travel_secs, mode, checked, sent) VALUES (?1,?2,?3,?4,?5,?6,?7,NULL)",
                params![key, e["event_id"].as_str(), title, start, travel, mode, now]);
        }
        let Some(t) = travel else { continue };
        if should_alert(now, start, t, buffer, false) {
            let text = alert_text(&title, now, start, t, &mode);
            // Events from outside content (mail) may not buzz through quiet hours.
            let priority = if e["outside"] == true { "normal" } else { "urgent" };
            scheduler::notice_full(app, &scheduler::Notice { priority, title: Some("Time to go"), text: &text, silent: false, thread: None });
            crate::audit::event(app, "travel.alert", json!({ "event": e["event_id"], "travel_min": t / 60, "mode": mode }));
            let _ = app.db.lock().execute("UPDATE travel_alerts SET sent = ?1 WHERE key = ?2", params![now, key]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leave_time_math() {
        let start = 10_000_000;
        // A 25 minute drive with 10 minutes' warning: the alert is 35 minutes before the start.
        assert_eq!(alert_at(start, 25 * 60, 10 * 60), start - 35 * 60);
        assert!(!should_alert(start - 36 * 60, start, 25 * 60, 600, false));
        assert!(should_alert(start - 35 * 60, start, 25 * 60, 600, false));
        assert!(should_alert(start - 5 * 60, start, 25 * 60, 600, false), "late is still worth saying");
        assert!(!should_alert(start, start, 25 * 60, 600, false), "never after the start");
        assert_eq!(alert_text("Dentist", start - 35 * 60, start, 25 * 60, "driving"), "Leave in 10 min for Dentist (25 min drive)");
        assert_eq!(alert_text("Dentist", start - 20 * 60, start, 25 * 60, "driving"), "Leave now for Dentist (25 min drive)");
        assert_eq!(alert_text("Gym", start - 20 * 60, start, 20 * 60, "walking"), "Leave now for Gym (20 min walk)");
    }

    #[test]
    fn once_only() {
        let start = 10_000_000;
        assert!(!should_alert(start - 30 * 60, start, 25 * 60, 600, true));
    }

    #[test]
    fn modes_and_routes() {
        assert_eq!(mode_secs("driving", 900.0, 12_000.0), 900);
        assert_eq!(mode_secs("walking", 900.0, 1_500.0), 1080); // 1.5 km at 5 km/h = 18 min
        assert_eq!(mode_secs("cycling", 900.0, 5_000.0), 1200); // 5 km at 15 km/h = 20 min
        assert_eq!(mode_secs("driving", 10.0, 50.0), 60, "never under a minute");
        let ok = json!({ "code": "Ok", "routes": [{ "duration": 1500.2, "distance": 18000.0 }] });
        assert_eq!(parse_route(&ok).unwrap(), (1500.2, 18000.0));
        assert!(parse_route(&json!({ "code": "NoRoute" })).is_err());
        assert_eq!(enc("12 Oak Ave, Town"), "12%20Oak%20Ave%2C%20Town");
    }
}
