//! Where the user goes (plugin `location`, docs/10).
//!
//! The phone sends a point about every 5 minutes. The core turns points into stays: when the
//! points keep within about 120 m for 15 minutes or more, that is a visit, and the visit belongs
//! to a place (an existing one within its radius, or a new one). A place gets a name by itself
//! when the user stays long (45 min) or comes back often (3 visits): the core asks OpenStreetMap
//! (Nominatim) what is there. A name the user gives ("Gym") always wins and is never replaced.
//!
//! Location is sensitive. Points stay on this server, are kept `AGENT_LOCATION_KEEP_DAYS` (90),
//! and reach the model only when it asks through a tool. Names from OpenStreetMap are outside
//! content.

use crate::{App, scheduler, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS location_points (n INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, lat REAL NOT NULL, lon REAL NOT NULL, acc REAL, device TEXT);
CREATE INDEX IF NOT EXISTS location_points_ts ON location_points (ts);
CREATE TABLE IF NOT EXISTS places (
  id TEXT PRIMARY KEY, lat REAL NOT NULL, lon REAL NOT NULL, radius REAL NOT NULL DEFAULT 100,
  user_name TEXT, auto_name TEXT, address TEXT, kind TEXT, visits INTEGER NOT NULL DEFAULT 0, total_secs INTEGER NOT NULL DEFAULT 0,
  first_seen INTEGER, last_seen INTEGER, named_at INTEGER, lookup_tried INTEGER, created TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS visits (id TEXT PRIMARY KEY, place_id TEXT, arrived INTEGER NOT NULL, left_at INTEGER NOT NULL, open INTEGER NOT NULL DEFAULT 1, lat REAL NOT NULL, lon REAL NOT NULL);
CREATE INDEX IF NOT EXISTS visits_arrived ON visits (arrived);
"#;

/// Points closer than this to the stay's middle are the same stay.
const STAY_RADIUS_M: f64 = 120.0;
/// A stay this long is a visit.
const MIN_STAY_SECS: i64 = 15 * 60;
/// Points less exact than this are kept but do not move stays.
const MAX_ACC_M: f64 = 250.0;
/// Name a place after one stay this long, or after this many visits.
const NAME_AFTER_SECS: i64 = 45 * 60;
const NAME_AFTER_VISITS: i64 = 3;

pub fn dist_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.sqrt().asin()
}

fn keep_days() -> i64 {
    std::env::var("AGENT_LOCATION_KEEP_DAYS").ok().and_then(|v| v.parse().ok()).filter(|d| *d > 0).unwrap_or(90)
}

// ---------------- the open stay ----------------

/// The stay being built, kept in `settings` so it survives a restart.
#[derive(Clone, Debug, Default)]
struct Stay {
    lat: f64,
    lon: f64,
    n: i64,
    start: i64,
    last: i64,
    visit: Option<String>,
}

fn load_stay(app: &App) -> Option<Stay> {
    let s: String = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'location.stay'", [], |r| r.get(0)).ok()?;
    let v: Value = serde_json::from_str(&s).ok()?;
    Some(Stay { lat: v["lat"].as_f64()?, lon: v["lon"].as_f64()?, n: v["n"].as_i64()?, start: v["start"].as_i64()?, last: v["last"].as_i64()?, visit: v["visit"].as_str().map(String::from) })
}

fn save_stay(app: &App, s: &Stay) {
    let v = json!({ "lat": s.lat, "lon": s.lon, "n": s.n, "start": s.start, "last": s.last, "visit": s.visit });
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('location.stay', ?1)", [v.to_string()]);
}

/// The nearest place whose circle holds this point.
fn place_at(app: &App, lat: f64, lon: f64) -> Option<(String, f64)> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT id, lat, lon, radius FROM places WHERE lat BETWEEN ?1 - 0.02 AND ?1 + 0.02 AND lon BETWEEN ?2 - 0.03 AND ?2 + 0.03").ok()?;
    let rows: Vec<(String, f64, f64, f64)> = st.query_map(params![lat, lon], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).ok()?.flatten().collect();
    rows.into_iter().map(|(id, la, lo, rad)| (id, dist_m((lat, lon), (la, lo)), rad)).filter(|(_, d, rad)| *d <= *rad).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, d, _)| (id, d))
}

fn new_place(app: &App, lat: f64, lon: f64, now: i64) -> String {
    let id = util::new_id("plc");
    let _ = app.db.lock().execute("INSERT INTO places (id, lat, lon, first_seen, last_seen, created) VALUES (?1,?2,?3,?4,?4,?5)", params![id, lat, lon, now, util::now_iso()]);
    id
}

/// The stay grew to a visit, or an open visit moved on: write it down.
fn touch_visit(app: &App, s: &mut Stay) {
    if s.last - s.start < MIN_STAY_SECS {
        return;
    }
    match &s.visit {
        None => {
            let place = place_at(app, s.lat, s.lon).map(|p| p.0).unwrap_or_else(|| new_place(app, s.lat, s.lon, s.start));
            let id = util::new_id("vis");
            let c = app.db.lock();
            let _ = c.execute("INSERT INTO visits (id, place_id, arrived, left_at, open, lat, lon) VALUES (?1,?2,?3,?4,1,?5,?6)", params![id, place, s.start, s.last, s.lat, s.lon]);
            let _ = c.execute("UPDATE places SET visits = visits + 1, last_seen = ?1 WHERE id = ?2", params![s.last, place]);
            s.visit = Some(id);
        }
        Some(v) => {
            let _ = app.db.lock().execute("UPDATE visits SET left_at = ?1, lat = ?2, lon = ?3 WHERE id = ?4", params![s.last, s.lat, s.lon, v]);
        }
    }
}

fn close_visit(app: &App, s: &Stay) {
    if let Some(v) = &s.visit {
        let c = app.db.lock();
        let _ = c.execute("UPDATE visits SET left_at = ?1, open = 0 WHERE id = ?2", params![s.last, v]);
        let _ = c.execute("UPDATE places SET total_secs = total_secs + ?1, last_seen = ?2 WHERE id = (SELECT place_id FROM visits WHERE id = ?3)", params![s.last - s.start, s.last, v]);
    }
}

/// Feed one point (in time order) into the stay.
fn step(app: &App, ts: i64, lat: f64, lon: f64, acc: Option<f64>) {
    if acc.is_some_and(|a| a > MAX_ACC_M) {
        return;
    }
    match load_stay(app) {
        Some(mut s) if ts >= s.last && dist_m((s.lat, s.lon), (lat, lon)) <= STAY_RADIUS_M => {
            // Same spot: move the middle a little toward this point.
            s.n += 1;
            let w = 1.0 / (s.n.min(50) as f64);
            s.lat += (lat - s.lat) * w;
            s.lon += (lon - s.lon) * w;
            s.last = ts;
            touch_visit(app, &mut s);
            save_stay(app, &s);
        }
        Some(s) if ts < s.last => {} // an old point arriving late does not move the stay
        old => {
            if let Some(s) = old {
                close_visit(app, &s);
            }
            save_stay(app, &Stay { lat, lon, n: 1, start: ts, last: ts, visit: None });
        }
    }
}

/// Which place the user is in right now, by point. Returns the arrivals and departures this point
/// caused. Leaving needs 1.5x the radius, so GPS jitter at the edge does not flap.
fn track_place(app: &App, lat: f64, lon: f64) -> Vec<(String, &'static str)> {
    let stored: Option<String> = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'location.inside'", [], |r| r.get(0)).ok();
    let get = |_: &App| -> String { stored.clone().unwrap_or_default() };
    let set = |a: &App, v: &str| { let _ = a.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('location.inside', ?1)", [v]); };
    let inside = get(app);
    let mut out = vec![];
    if let Some((pid, _)) = place_at(app, lat, lon) {
        if pid != inside {
            if !inside.is_empty() {
                out.push((inside, "leave"));
            }
            set(app, &pid);
            // The very first point after this feature starts only learns where the user already is.
            if stored.is_some() {
                out.push((pid, "arrive"));
            }
        }
    } else if stored.is_none() {
        set(app, "");
    } else if !inside.is_empty() {
        let far = app.db.lock().query_row("SELECT lat, lon, radius FROM places WHERE id = ?1", [&inside], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?))).ok()
            .is_none_or(|(la, lo, rad)| dist_m((lat, lon), (la, lo)) > rad * 1.5);
        if far {
            set(app, "");
            out.push((inside, "leave"));
        }
    }
    out
}

/// Places the phone should watch with its own geofences: those an active job waits on.
/// Only these are sent, so the phone does not learn about the rest.
pub fn fences(app: &App) -> Value {
    let ids: Vec<String> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT schedule FROM jobs WHERE kind IN ('task','reminder') AND status = 'active' AND next_run IS NULL").expect("prepare");
        let mut v: Vec<String> = st.query_map([], |r| r.get::<_, String>(0)).map(|x| x.flatten()
            .filter_map(|s| serde_json::from_str::<Value>(&s).ok()).filter(|s| s["on"] == "place")
            .filter_map(|s| s["place"].as_str().map(String::from)).collect()).unwrap_or_default();
        v.sort();
        v.dedup();
        v
    };
    // Home too, so leaving it is noticed at once (docs/10, Noticing).
    let ids: Vec<String> = match crate::detect::home_place(app) { Some(h) if !ids.contains(&h) => ids.into_iter().chain([h]).collect(), _ => ids };
    let list: Vec<Value> = ids.iter().filter_map(|id| get_place(app, id)).map(|p| json!({ "id": p["id"], "name": p["name"], "lat": p["lat"], "lon": p["lon"], "radius": p["radius"] })).collect();
    json!({ "fences": list })
}

/// The phone's own geofence says the user entered or left a place. One message instead of a batch
/// of points; the same state as `track_place`, so the two never fire a job twice.
pub fn transition(app: &Arc<App>, place: &str, what: &str, ts: Option<i64>) -> Result<Value, String> {
    if !["arrive", "leave"].contains(&what) {
        return Err("what is arrive or leave".into());
    }
    if get_place(app, place).is_none() {
        return Err("no such place".into());
    }
    let now = util::unix_now();
    if ts.is_some_and(|t| t < now - 30 * 60) {
        return Ok(json!({ "ignored": "too old" }));
    }
    let inside: Option<String> = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'location.inside'", [], |r| r.get(0)).ok();
    let inside = inside.unwrap_or_default();
    let set = |v: &str| { let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('location.inside', ?1)", [v]); };
    if what == "arrive" {
        if inside == place {
            return Ok(json!({ "ignored": "already there" }));
        }
        if !inside.is_empty() {
            scheduler::place_event(app, &inside, "leave");
        }
        set(place);
    } else {
        if inside != place {
            return Ok(json!({ "ignored": "was not there" }));
        }
        set("");
    }
    crate::audit::event(app, "place.crossed", json!({ "place": place, "what": what, "by": "phone geofence" }));
    scheduler::place_event(app, place, what);
    Ok(json!({ "ok": true }))
}

/// A place by id or by (part of) its name. One answer, or a plain error that says what to do.
pub fn find_place(app: &App, q: &str) -> Result<(String, String), String> {
    let q = q.trim();
    if q.is_empty() {
        return Err("place is required: the place's name (for example Home)".into());
    }
    if let Some(p) = get_place(app, q) {
        return Ok((q.to_string(), p["name"].as_str().unwrap_or("this place").to_string()));
    }
    let found = places(app, Some(q));
    match found.len() {
        0 => Err(format!("I do not know a place called '{q}'. The user can name where they are with name_place (while there), or you can name a spot with its lat and lon.")),
        1 => Ok((found[0]["id"].as_str().unwrap_or_default().to_string(), found[0]["name"].as_str().unwrap_or(q).to_string())),
        _ => {
            if let Some(p) = found.iter().find(|p| p["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(q))) {
                return Ok((p["id"].as_str().unwrap_or_default().to_string(), q.to_string()));
            }
            Err(format!("more than one place matches '{q}': {}. Ask which one.", found.iter().take(5).map(|p| format!("{} ({})", p["name"].as_str().unwrap_or("?"), p["id"].as_str().unwrap_or("?"))).collect::<Vec<_>>().join(", ")))
        }
    }
}

/// Points from the phone. Returns how many were kept. Naming runs afterwards, off this thread.
pub fn add_points(app: &Arc<App>, device: &str, points: &Value) -> Result<Value, String> {
    let list = points.as_array().ok_or("points must be a list")?;
    if list.len() > 2000 {
        return Err("at most 2000 points at a time".into());
    }
    let now = util::unix_now();
    let mut good: Vec<(i64, f64, f64, Option<f64>)> = vec![];
    for p in list {
        let (Some(ts), Some(lat), Some(lon)) = (p["ts"].as_i64(), p["lat"].as_f64(), p["lon"].as_f64()) else { continue };
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) || ts > now + 300 || ts < now - 30 * 86_400 {
            continue;
        }
        good.push((ts, lat, lon, p["acc"].as_f64().filter(|a| *a >= 0.0)));
    }
    good.sort_by_key(|p| p.0);
    {
        let c = app.db.lock();
        for (ts, lat, lon, acc) in &good {
            let _ = c.execute("INSERT INTO location_points (ts, lat, lon, acc, device) VALUES (?1,?2,?3,?4,?5)", params![ts, lat, lon, acc, device]);
        }
        let _ = c.execute("DELETE FROM location_points WHERE ts < ?1", [now - keep_days() * 86_400]);
    }
    for (ts, lat, lon, acc) in &good {
        step(app, *ts, *lat, *lon, *acc);
        // Only a fresh, fairly exact point can start something; a backlog from an offline phone cannot.
        if *ts > now - 30 * 60 && acc.is_none_or(|a| a <= 150.0) {
            for (place, what) in track_place(app, *lat, *lon) {
                crate::audit::event(app, "place.crossed", json!({ "place": place, "what": what, "by": "location points" }));
                scheduler::place_event(app, &place, what);
            }
        }
    }
    let app2 = app.clone();
    tokio::task::spawn_blocking(move || name_places(&app2));
    crate::audit::event(app, "phone.location", json!({ "kept": good.len(), "skipped": list.len() - good.len(), "device": device }));
    Ok(json!({ "kept": good.len(), "skipped": list.len() - good.len() }))
}

// ---------------- names from OpenStreetMap ----------------

/// Ask Nominatim what is at a spot. One request at a time, at most one a second (its rules).
/// Ask OpenStreetMap what is near a point. The point is rounded first (security review #24): OpenStreetMap never sees
/// an exact position, only one about 100 m across (`AGENT_GEOCODE_DECIMALS`, default 3; 4 = about 10 m).
pub fn reverse_lookup(base: &str, lat: f64, lon: f64) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(12))).http_status_as_error(false).build().into();
    let (lat, lon, d) = rounded(lat, lon);
    let url = format!("{base}/reverse?format=jsonv2&addressdetails=1&zoom=17&lat={lat:.d$}&lon={lon:.d$}");
    let mut r = agent.get(&url).header("User-Agent", "self-hosted-personal-assistant/0.1 (one user, a few requests a day)").header("Accept-Language", "en").call().map_err(|e| e.to_string())?;
    if r.status().as_u16() != 200 {
        return Err(format!("OpenStreetMap answered {}", r.status().as_u16()));
    }
    serde_json::from_str(&r.body_mut().read_to_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// A point rounded for outside lookups, and how many decimals were kept.
pub fn rounded(lat: f64, lon: f64) -> (f64, f64, usize) {
    let d: usize = std::env::var("AGENT_GEOCODE_DECIMALS").ok().and_then(|v| v.parse().ok()).filter(|d| (2..=5).contains(d)).unwrap_or(3);
    let f = 10f64.powi(d as i32);
    ((lat * f).round() / f, (lon * f).round() / f, d)
}

/// A short name for what Nominatim found: the place's own name, else the kind of place, else the street.
pub fn pick_name(v: &Value) -> Option<(String, String, String)> {
    let a = &v["address"];
    let nonempty = |x: &Value| x.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    let street = match (nonempty(&a["house_number"]), nonempty(&a["road"])) {
        (Some(n), Some(r)) => Some(format!("{n} {r}")),
        (None, Some(r)) => Some(r),
        _ => None,
    };
    let name = nonempty(&v["name"])
        .or_else(|| ["amenity", "shop", "leisure", "tourism", "building", "office"].iter().find_map(|k| nonempty(&a[*k])))
        .or(street)
        .or_else(|| ["neighbourhood", "suburb", "city", "town", "village"].iter().find_map(|k| nonempty(&a[*k])))?;
    let kind = [nonempty(&v["category"]), nonempty(&v["type"])].into_iter().flatten().collect::<Vec<_>>().join("/");
    Some((name.chars().take(80).collect(), nonempty(&v["display_name"]).unwrap_or_default().chars().take(300).collect(), kind))
}

static LOOKUP: std::sync::Mutex<i64> = std::sync::Mutex::new(0);

/// Name the places that earned it and have no name yet. Each place is looked up once a day at most.
fn name_places(app: &App) {
    let Ok(_one) = LOOKUP.try_lock() else { return };
    let now = util::unix_now();
    let due: Vec<(String, f64, f64, i64)> = {
        let c = app.db.lock();
        let Ok(mut st) = c.prepare(
            "SELECT p.id, p.lat, p.lon, p.visits FROM places p WHERE p.user_name IS NULL AND p.auto_name IS NULL AND (p.lookup_tried IS NULL OR p.lookup_tried < ?1)
             AND (p.visits >= ?2 OR p.total_secs >= ?3 OR EXISTS (SELECT 1 FROM visits v WHERE v.place_id = p.id AND v.left_at - v.arrived >= ?3)) LIMIT 5",
        ) else { return };
        st.query_map(params![now - 86_400, NAME_AFTER_VISITS, NAME_AFTER_SECS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    for (id, lat, lon, visits) in due {
        let _ = app.db.lock().execute("UPDATE places SET lookup_tried = ?1 WHERE id = ?2", params![now, id]);
        if let Ok(v) = reverse_lookup(&app.cfg.geocode_base, lat, lon)
            && let Some((name, address, kind)) = pick_name(&v)
        {
            let _ = app.db.lock().execute("UPDATE places SET auto_name = ?1, address = ?2, kind = ?3, named_at = ?4 WHERE id = ?5", params![name, address, kind, now, id]);
            // A place the user comes back to often deserves a second look: OpenStreetMap may have
            // guessed. A place from one long stay just keeps the OpenStreetMap name, quietly.
            if visits >= NAME_AFTER_VISITS {
                scheduler::notice_full(app, &scheduler::Notice { priority: "normal", title: Some("New place"), text: &often_text(&name), silent: false, thread: None });
            }
        }
        std::thread::sleep(Duration::from_millis(1100));
    }
}

/// Habits worth an automation: a named place the user is at on the same weekday in at least 3 of the last 4
/// weeks, with nothing waiting on it yet and not suggested before. Starts a chat with the idea (the
/// assistant's own message), and a notice that opens it; replying "yes, remind me" makes it with the tools.
/// One idea per run. Returns how many were raised.
pub fn suggest(app: &Arc<App>) -> usize {
    let now = util::unix_now();
    let z = scheduler::zone(&app.cfg.timezone);
    let rows: Vec<(String, i64)> = {
        let c = app.db.lock();
        let Ok(mut st) = c.prepare("SELECT place_id, arrived FROM visits WHERE place_id IS NOT NULL AND arrived >= ?1") else { return 0 };
        st.query_map([now - 28 * 86_400], |r| Ok((r.get(0)?, r.get(1)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let mut by_place: std::collections::BTreeMap<String, Vec<i64>> = Default::default();
    for (p, t) in rows {
        by_place.entry(p).or_default().push(t);
    }
    let watched: Vec<String> = fences(app)["fences"].as_array().map(|a| a.iter().filter_map(|f| f["id"].as_str().map(String::from)).collect()).unwrap_or_default();
    const DAYS: [&str; 7] = ["Sundays", "Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays"];
    for (pid, times) in by_place {
        if times.len() > 20 || watched.contains(&pid) {
            continue; // nearly every day (home), or a reminder already waits there
        }
        let key = format!("suggested.{pid}");
        if app.db.lock().query_row("SELECT 1 FROM settings WHERE key = ?1", [&key], |_| Ok(())).is_ok() {
            continue;
        }
        let Some(place) = get_place(app, &pid) else { continue };
        let Some(name) = place["name"].as_str().map(String::from) else { continue };
        let local: Vec<(i64, i64)> = times.iter().map(|t| { let l = t + z.offset_at(*t); ((l.div_euclid(86_400) + 4).rem_euclid(7), l.rem_euclid(86_400)) }).collect();
        let mut days: Vec<usize> = (0..7).filter(|d| local.iter().filter(|(w, _)| *w as usize == *d).count() >= 3).collect();
        if days.is_empty() {
            continue;
        }
        days.sort();
        let mut secs: Vec<i64> = local.iter().filter(|(w, _)| days.contains(&(*w as usize))).map(|(_, s)| *s).collect();
        secs.sort();
        let mid = secs[secs.len() / 2];
        let when = format!("{:02}:{:02}", mid / 3600, (mid % 3600) / 60);
        let day_words = days.iter().map(|d| DAYS[*d]).collect::<Vec<_>>().join(" and ");
        let thread = format!("idea:{}:{now}", &pid[pid.len().saturating_sub(6)..]);
        app.ensure_thread(&thread, "user", &format!("Idea: {name}"));
        let text = format!("I noticed you are usually at {name} on {day_words} around {when} (at least 3 of the last 4 weeks). Want me to set something up? For example: remind you when you arrive or leave, only on those days and after a certain time, or log each visit. Tell me what you would like, or say no and I will not ask again.");
        crate::engine::insert_message(app, &thread, None, "assistant", "assistant", &json!({ "role": "assistant", "content": text }));
        scheduler::notice_full(app, &scheduler::Notice { priority: "normal", title: Some("Idea"), text: &format!("You are often at {name} on {day_words}. Want a reminder?"), silent: false, thread: Some(&thread) });
        let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", params![key, now.to_string()]);
        crate::audit::event(app, "idea.suggested", json!({ "place": name, "days": day_words, "around": when }));
        return 1;
    }
    0
}

/// Once a day, in the daytime (a quiet moment to be asked).
pub fn maybe_suggest(app: &Arc<App>) {
    let now = util::unix_now();
    let hour = (now + scheduler::zone(&app.cfg.timezone).offset_at(now)).rem_euclid(86_400) / 3600;
    if (10..20).contains(&hour) && app.due("suggest", 20 * 3600) {
        suggest(app);
    }
}

/// The note for a place that is visited often and was just named by OpenStreetMap.
fn often_text(name: &str) -> String {
    let n: String = name.chars().take(30).collect();
    format!("You go to \"{n}\" often. Right name? Tell me if not.")
}

// ---------------- reading ----------------

fn place_json(app: &App, r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let user: Option<String> = r.get(4)?;
    let auto: Option<String> = r.get(5)?;
    let last: Option<i64> = r.get(11)?;
    Ok(json!({ "id": r.get::<_, String>(0)?, "lat": r.get::<_, f64>(1)?, "lon": r.get::<_, f64>(2)?, "radius": r.get::<_, f64>(3)?,
        "name": user.clone().or(auto.clone()), "named_by": if user.is_some() { "user" } else if auto.is_some() { "openstreetmap" } else { "nobody yet" },
        "openstreetmap_name": auto, "address": r.get::<_, Option<String>>(6)?, "kind": r.get::<_, Option<String>>(7)?,
        "visits": r.get::<_, i64>(8)?, "hours": (r.get::<_, i64>(9)? as f64 / 360.0).round() / 10.0,
        "last_seen": last.map(|t| scheduler::local_str(app, t)) }))
}

const PLACE_COLS: &str = "id, lat, lon, radius, user_name, auto_name, address, kind, visits, total_secs, first_seen, last_seen";

pub fn get_place(app: &App, id: &str) -> Option<Value> {
    let c = app.db.lock();
    c.query_row(&format!("SELECT {PLACE_COLS} FROM places WHERE id = ?1"), [id], |r| place_json(app, r)).ok()
}

pub fn places(app: &App, query: Option<&str>) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare(&format!("SELECT {PLACE_COLS} FROM places WHERE visits > 0 OR user_name IS NOT NULL ORDER BY visits DESC, last_seen DESC LIMIT 300")).expect("prepare");
    let q = query.map(|q| q.to_lowercase());
    st.query_map([], |r| place_json(app, r)).map(|x| x.flatten().filter(|p| q.as_ref().is_none_or(|q| {
        [&p["name"], &p["address"], &p["openstreetmap_name"]].iter().any(|v| v.as_str().unwrap_or("").to_lowercase().contains(q.as_str()))
    })).collect()).unwrap_or_default()
}

pub fn last_point(app: &App) -> Option<(i64, f64, f64, Option<f64>)> {
    app.db.lock().query_row("SELECT ts, lat, lon, acc FROM location_points ORDER BY ts DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().ok().flatten()
}

/// Where the user is now (as far as the last point says).
pub fn now(app: &App) -> Value {
    let Some((ts, lat, lon, acc)) = last_point(app) else {
        return json!({ "known": false, "note": "No location yet. The phone app sends it when Location is on in its settings." });
    };
    let here = place_at(app, lat, lon).and_then(|(id, _)| get_place(app, &id));
    let stay = load_stay(app).filter(|s| dist_m((s.lat, s.lon), (lat, lon)) <= STAY_RADIUS_M);
    json!({ "known": true, "at": scheduler::local_str(app, ts), "minutes_ago": (util::unix_now() - ts) / 60, "lat": lat, "lon": lon, "accuracy_m": acc,
        "place": here, "here_since": stay.map(|s| scheduler::local_str(app, s.start)) })
}

/// Visits between two local times (default: the last 7 days), newest first.
pub fn history(app: &App, from: Option<&str>, to: Option<&str>) -> Result<Value, String> {
    let z = scheduler::zone(&app.cfg.timezone);
    let parse = |s: Option<&str>, end: bool| -> Result<Option<i64>, String> {
        let Some(s) = s.map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
        let full = if s.len() == 10 { format!("{s} {}", if end { "23:59" } else { "00:00" }) } else { s.replace('T', " ") };
        scheduler::parse_local(&full).map(|l| Some(z.local_to_utc(l))).ok_or_else(|| format!("'{s}' is not a date"))
    };
    let now = util::unix_now();
    let to_u = parse(to, true)?.unwrap_or(now);
    let from_u = parse(from, false)?.unwrap_or(to_u - 7 * 86_400);
    let c = app.db.lock();
    let mut st = c.prepare("SELECT v.place_id, v.arrived, v.left_at, v.open, COALESCE(p.user_name, p.auto_name), v.lat, v.lon FROM visits v LEFT JOIN places p ON p.id = v.place_id
        WHERE v.left_at >= ?1 AND v.arrived <= ?2 ORDER BY v.arrived DESC LIMIT 300").map_err(|e| e.to_string())?;
    let items: Vec<Value> = st.query_map(params![from_u, to_u], |r| {
        let (a, l): (i64, i64) = (r.get(1)?, r.get(2)?);
        Ok(json!({ "place_id": r.get::<_, Option<String>>(0)?, "name": r.get::<_, Option<String>>(4)?, "arrived": scheduler::local_str(app, a),
            "left": if r.get::<_, i64>(3)? == 1 { Value::Null } else { json!(scheduler::local_str(app, l)) }, "minutes": (l - a) / 60,
            "lat": r.get::<_, f64>(5)?, "lon": r.get::<_, f64>(6)? }))
    }).map_err(|e| e.to_string())?.flatten().collect();
    let points: i64 = c.query_row("SELECT COUNT(*) FROM location_points WHERE ts BETWEEN ?1 AND ?2", params![from_u, to_u], |r| r.get(0)).unwrap_or(0);
    Ok(json!({ "from": scheduler::local_str(app, from_u), "to": scheduler::local_str(app, to_u), "visits": items, "points": points,
        "note": "A visit is a stay of 15 minutes or more. Short stops and travel are only points." }))
}

pub fn points(app: &App, from: i64, to: i64, limit: i64) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT ts, lat, lon, acc FROM location_points WHERE ts BETWEEN ?1 AND ?2 ORDER BY ts LIMIT ?3").expect("prepare");
    st.query_map(params![from, to, limit.clamp(1, 5000)], |r| Ok(json!({ "ts": r.get::<_, i64>(0)?, "lat": r.get::<_, f64>(1)?, "lon": r.get::<_, f64>(2)?, "acc": r.get::<_, Option<f64>>(3)? })))
        .map(|x| x.flatten().collect()).unwrap_or_default()
}

// ---------------- the user's own names ----------------

/// Name a place. `id` may be a place id; without one, the place where the user is now (made if needed).
pub fn name_place(app: &App, id: Option<&str>, name: &str, radius: Option<f64>, at: Option<(f64, f64)>) -> Result<Value, String> {
    let name: String = name.trim().chars().take(80).collect();
    if name.is_empty() {
        return Err("name is required, like Gym".into());
    }
    let radius = radius.map(|r| r.clamp(30.0, 2000.0));
    let id = match id.filter(|i| !i.is_empty()) {
        Some(i) => {
            get_place(app, i).ok_or("no such place")?;
            i.to_string()
        }
        None => {
            let (lat, lon) = match at {
                Some(p) => p,
                None => {
                    let (_, la, lo, _) = last_point(app).ok_or("I do not know where you are yet")?;
                    load_stay(app).filter(|s| dist_m((s.lat, s.lon), (la, lo)) <= STAY_RADIUS_M).map(|s| (s.lat, s.lon)).unwrap_or((la, lo))
                }
            };
            place_at(app, lat, lon).map(|p| p.0).unwrap_or_else(|| new_place(app, lat, lon, util::unix_now()))
        }
    };
    let prev = get_place(app, &id);
    app.db.lock().execute("UPDATE places SET user_name = ?1, radius = COALESCE(?2, radius) WHERE id = ?3", params![name, radius, id]).map_err(|e| e.to_string())?;
    let mut v = get_place(app, &id).ok_or("place vanished")?;
    v["previous"] = prev.unwrap_or(Value::Null);
    Ok(v)
}

/// Undo of a rename: put the old name (or none) back.
fn restore_name(app: &App, prev: &Value) -> Result<Value, String> {
    let id = prev["id"].as_str().ok_or("nothing to restore")?;
    let user = if prev["named_by"] == "user" { prev["name"].as_str() } else { None };
    app.db.lock().execute("UPDATE places SET user_name = ?1, radius = ?2 WHERE id = ?3", params![user, prev["radius"].as_f64().unwrap_or(100.0), id]).map_err(|e| e.to_string())?;
    Ok(json!({ "restored": id }))
}

pub fn forget_place(app: &App, id: &str) -> bool {
    let c = app.db.lock();
    let _ = c.execute("UPDATE visits SET place_id = NULL WHERE place_id = ?1", [id]);
    c.execute("DELETE FROM places WHERE id = ?1", [id]).map(|n| n > 0).unwrap_or(false)
}

/// `location` plugin tools.
pub fn tool(app: &App, tool: &str, a: &Value) -> Result<Value, String> {
    match tool {
        "where_now" => Ok(now(app)),
        "history" => history(app, a["from"].as_str(), a["to"].as_str()),
        "places" => Ok(json!({ "places": places(app, a["query"].as_str()) })),
        "name_place" => {
            let at = match (a["lat"].as_f64(), a["lon"].as_f64()) { (Some(la), Some(lo)) => Some((la, lo)), _ => None };
            name_place(app, a["id"].as_str(), a["name"].as_str().unwrap_or(""), a["radius"].as_f64(), at)
        }
        "restore_place_name" => restore_name(app, &a["result"]["previous"]),
        "travel_time" => crate::travel::travel_time(app, a),
        _ => Err("no such tool".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_often_note_fits_a_notice() {
        let t = often_text("A very long place name that goes on and on and on forever");
        assert!(t.chars().count() <= 80 && t.contains("often"), "{t}");
    }

    #[test]
    fn distance_and_names() {
        let d = dist_m((52.5200, 13.4050), (52.5210, 13.4050));
        assert!((d - 111.0).abs() < 2.0, "{d}");
        let gym = json!({ "name": "", "category": "leisure", "type": "fitness_centre", "display_name": "FitBox, 5 Main St", "address": { "leisure": "FitBox", "road": "Main St" } });
        assert_eq!(pick_name(&gym).unwrap().0, "FitBox");
        let house = json!({ "address": { "house_number": "12", "road": "Oak Ave", "city": "Town" } });
        assert_eq!(pick_name(&house).unwrap().0, "12 Oak Ave");
        assert!(pick_name(&json!({})).is_none());
    }
}

#[cfg(test)]
mod rounding_tests {
    #[test]
    fn outside_lookups_get_a_rounded_point() {
        let (lat, lon, d) = super::rounded(49.234_567_8, -121.765_432_1);
        assert_eq!(d, 3);
        assert_eq!(format!("{lat:.d$},{lon:.d$}"), "49.235,-121.765");
    }
}
