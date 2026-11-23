//! Signals from the phone, and the morning brief (docs/14, "Morning brief").
//!
//! The phone tells the server small facts about the user's day with `POST /v1/signals`. The first
//! kind is `woke`: the phone saw the user wake up (the first unlock after a long night with the
//! screen off, dismissing a morning alarm, or a sleep session ending in Health Connect). Every
//! signal is kept in `signals`. The first `woke` of a local day starts the **morning brief**: a
//! task run that gets the day's facts from the core (weather where the user is, today's calendar,
//! last night's sleep, today's alarms and reminders, open to-dos when lists exist) and writes a short
//! brief; the core sends it as a normal notice that opens its thread. If the model gives nothing,
//! the core sends the facts in plain words. An optional fixed time (`fallback`) sends the brief on
//! days the phone never said "woke".

use crate::{App, scheduler, util};
use rusqlite::params;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS signals (n INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, at INTEGER NOT NULL, source TEXT, device TEXT, received INTEGER NOT NULL, result TEXT);
CREATE INDEX IF NOT EXISTS signals_at ON signals (at);
"#;

pub const KINDS: &[&str] = &["woke"];
pub const SOURCES: &[&str] = &["unlock", "alarm", "sleep", "manual", "fallback"];
pub const PARTS: &[&str] = &["weather", "calendar", "sleep", "reminders", "lists"];

/// A wake counts only between these local times (minutes after midnight): 03:00 up to 12:00.
pub const WAKE_FROM_MIN: i64 = 3 * 60;
pub const WAKE_UNTIL_MIN: i64 = 12 * 60;
/// An unlock counts as waking only after the screen was off this long.
pub const NIGHT_OFF_SECS: i64 = 4 * 3600;

/// The wake rule, shared with the phone (Woke.kt). `minute` is the local minute of the day of the
/// signal; `off_secs` how long the screen was off before an unlock (only checked for `unlock`).
pub fn is_wake(source: &str, minute: i64, off_secs: Option<i64>) -> bool {
    if !(WAKE_FROM_MIN..WAKE_UNTIL_MIN).contains(&minute) {
        return false;
    }
    match source {
        "unlock" => off_secs.is_none_or(|s| s >= NIGHT_OFF_SECS),
        "alarm" | "sleep" | "manual" | "fallback" => true,
        _ => false,
    }
}

/// Once a day: a brief is due only when the last one was on another local day.
pub fn brief_due(last_day: Option<&str>, today: &str) -> bool {
    last_day != Some(today)
}

// ---------------- settings ----------------

fn setting(app: &App, key: &str) -> Option<String> {
    app.db.lock().query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).ok()
}

fn set_setting(app: &App, key: &str, v: &str) {
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", params![key, v]);
}

/// `{enabled, parts: {weather: true, ...}, fallback: "08:30" | null}`. On by default, every part, no fallback.
pub fn settings(app: &App) -> Value {
    let saved: Value = setting(app, "brief.settings").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    let parts: serde_json::Map<String, Value> = PARTS.iter().map(|p| (p.to_string(), json!(saved["parts"][*p].as_bool().unwrap_or(true)))).collect();
    json!({ "enabled": saved["enabled"].as_bool().unwrap_or(true), "parts": parts, "fallback": saved["fallback"].as_str().filter(|s| !s.is_empty()),
        "last_day": setting(app, "brief.last_day") })
}

pub fn patch_settings(app: &App, b: &Value) -> Result<Value, String> {
    let mut s = settings(app);
    if let Some(e) = b.get("enabled") {
        s["enabled"] = json!(e.as_bool().ok_or("enabled is true or false")?);
    }
    if let Some(p) = b.get("parts").and_then(|p| p.as_object()) {
        for (k, v) in p {
            if !PARTS.contains(&k.as_str()) {
                return Err(format!("unknown part '{k}'; parts are {}", PARTS.join(", ")));
            }
            s["parts"][k] = json!(v.as_bool().ok_or("each part is true or false")?);
        }
    }
    match b.get("fallback") {
        None => {}
        Some(Value::Null) => s["fallback"] = Value::Null,
        Some(Value::String(t)) if t.trim().is_empty() => s["fallback"] = Value::Null,
        Some(Value::String(t)) => {
            scheduler::parse_hm(t).ok_or("fallback is a clock time like 08:30, or null for none")?;
            s["fallback"] = json!(t.trim());
        }
        _ => return Err("fallback is a clock time like 08:30, or null for none".into()),
    }
    let keep = json!({ "enabled": s["enabled"], "parts": s["parts"], "fallback": s["fallback"] });
    set_setting(app, "brief.settings", &keep.to_string());
    Ok(settings(app))
}

fn part_on(s: &Value, p: &str) -> bool {
    s["parts"][p].as_bool().unwrap_or(true)
}

// ---------------- signals ----------------

fn local_day_minute(app: &App, utc: i64) -> (String, i64) {
    let s = scheduler::local_str(app, utc);
    let minute = scheduler::parse_hm(&s[11..]).map(|x| x / 60).unwrap_or(0);
    (s[..10].to_string(), minute)
}

/// `{kind: "woke", at?: unix, source: "unlock"|"alarm"|"sleep"|"manual", off_secs?}`. Kept, then acted on.
pub fn receive(app: &Arc<App>, device: &str, b: &Value) -> Result<Value, String> {
    let kind = b["kind"].as_str().filter(|k| KINDS.contains(k)).ok_or_else(|| format!("kind must be one of {}", KINDS.join(", ")))?;
    let source = b["source"].as_str().unwrap_or("manual");
    if !SOURCES.contains(&source) || source == "fallback" {
        return Err("source must be unlock, alarm, sleep or manual".into());
    }
    let now = util::unix_now();
    let at = b["at"].as_i64().unwrap_or(now);
    if at > now + 300 || at < now - 2 * 86_400 {
        return Err("at must be a unix time from the last two days".into());
    }
    let off = b["off_secs"].as_i64().filter(|s| *s >= 0);
    let result = match kind {
        "woke" => woke(app, at, source, off),
        _ => "kept",
    };
    app.db.lock().execute("INSERT INTO signals (kind, at, source, device, received, result) VALUES (?1,?2,?3,?4,?5,?6)", params![kind, at, source, device, now, result]).map_err(|e| e.to_string())?;
    crate::audit::event(app, "phone.signal", json!({ "kind": kind, "source": source, "result": result }));
    Ok(json!({ "kind": kind, "at": util::iso(at), "result": result }))
}

fn woke(app: &Arc<App>, at: i64, source: &str, off: Option<i64>) -> &'static str {
    let (day, minute) = local_day_minute(app, at);
    if !is_wake(source, minute, off) {
        return "not_a_wake";
    }
    if util::unix_now() - at > 3 * 3600 {
        return "too_old"; // a backlog from a phone that was offline: the morning has passed
    }
    start_brief(app, &day)
}

/// Start the brief for `day` unless one went out already or it is off.
fn start_brief(app: &Arc<App>, day: &str) -> &'static str {
    let s = settings(app);
    if s["enabled"] != true {
        return "brief_off";
    }
    if !brief_due(setting(app, "brief.last_day").as_deref(), day) {
        return "already_today";
    }
    set_setting(app, "brief.last_day", day);
    // The weather is fetched first, off the async threads, so the run itself makes no network call.
    let app = app.clone();
    let work = move || {
        refresh_weather(&app);
        if ensure_job(&app).and_then(|id| scheduler::run_now(&app, &id)).is_err() {
            send(&app, None, None); // no task could run: send the facts as they are
        }
    };
    match tokio::runtime::Handle::try_current() {
        Ok(rt) => drop(rt.spawn_blocking(work)),
        Err(_) => work(),
    }
    "brief_started"
}

pub fn list(app: &App, limit: i64) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT n, kind, at, source, device, result FROM signals ORDER BY n DESC LIMIT ?1").expect("prepare");
    st.query_map([limit.clamp(1, 200)], |r| Ok(json!({ "n": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "at": util::iso(r.get(2)?),
        "source": r.get::<_, Option<String>>(3)?, "device": r.get::<_, Option<String>>(4)?, "result": r.get::<_, Option<String>>(5)? })))
        .map(|x| x.flatten().collect()).unwrap_or_default()
}

/// The fixed time, for days the phone never said "woke". Called by the core timer.
pub fn maybe_fallback(app: &Arc<App>) {
    if !app.due("brief.fallback", 60) {
        return;
    }
    let s = settings(app);
    let Some(t) = s["fallback"].as_str().and_then(scheduler::parse_hm) else { return };
    let now = util::unix_now();
    let (day, minute) = local_day_minute(app, now);
    // Due from the set time until noon; later than that the morning has passed.
    if minute * 60 < t || minute >= WAKE_UNTIL_MIN || s["enabled"] != true || !brief_due(s["last_day"].as_str(), &day) {
        return;
    }
    let result = start_brief(app, &day);
    let _ = app.db.lock().execute("INSERT INTO signals (kind, at, source, device, received, result) VALUES ('woke', ?1, 'fallback', 'core', ?1, ?2)", params![now, result]);
}

// ---------------- the brief ----------------

const JOB_TEXT: &str = "Morning brief. The core gives you today's facts below. Write the brief for the user waking up: a short greeting line, then the day in a few short lines (weather, events with times, sleep, alarms and reminders, to-dos). Plain words, no markdown tables.";

/// The task that writes the brief. Made when first needed; an ordinary task the user can see in Automation.
fn ensure_job(app: &App) -> Result<String, String> {
    let existing: Option<String> = {
        let c = app.db.lock();
        c.query_row("SELECT id FROM jobs WHERE kind = 'task' AND status = 'active' AND json_extract(schedule, '$.brief') = 1 LIMIT 1", [], |r| r.get(0)).ok()
    };
    if let Some(id) = existing {
        return Ok(id);
    }
    let sched = json!({ "on": "wake", "brief": true, "quiet": true, "model": "local" });
    let x = scheduler::JobExtra { name: Some("Morning brief".into()), description: Some("Your day when you wake up: weather, calendar, sleep, alarms".into()), ..Default::default() };
    let j = scheduler::create_job_with(app, "task", JOB_TEXT, &sched, false, "core", None, &x)?;
    j["id"].as_str().map(String::from).ok_or_else(|| "no job id".into())
}

/// Is this job the brief?
pub fn is_brief_job(sched: &Value) -> bool {
    sched["brief"] == true
}

/// Rules for the brief run (mind::task_rules uses this).
pub const RULES: &str = "This is the morning brief. Use only the facts the core gives below; do not load plugins or search unless a fact is missing and matters. Do not call notify: the core sends your reply to the user as the brief. Keep it under 80 words. Leave out any part that has nothing in it. Never invent weather, events or numbers.";

/// What the day looks like, part by part (each a few plain lines). Missing data means the part is left out.
pub struct Facts {
    pub weather: Option<String>,
    pub calendar: Option<String>,
    pub sleep: Option<String>,
    pub reminders: Option<String>,
    pub lists: Option<String>,
}

pub fn gather(app: &App) -> Facts {
    let s = settings(app);
    let now = util::unix_now();
    let today = scheduler::local_str(app, now)[..10].to_string();
    Facts {
        weather: if part_on(&s, "weather") { weather(app) } else { None },
        calendar: if part_on(&s, "calendar") && app.plugin_usable("calendar") { calendar_today(app, &today) } else { None },
        sleep: if part_on(&s, "sleep") { sleep(app, &today) } else { None },
        reminders: if part_on(&s, "reminders") { reminders_today(app, &today) } else { None },
        lists: if part_on(&s, "lists") { open_todos(app) } else { None },
    }
}

/// The facts for the model. Weather and calendar text came from outside, so they are wrapped.
pub fn facts_for_run(app: &App) -> String {
    let f = gather(app);
    let mut out = format!("# Today's facts\n\n{}\n", scheduler::now_line(app));
    if let Some(w) = &f.weather {
        out += &format!("\n## Weather\n{}\n", util::wrap_outside("brief:weather", w));
    }
    if let Some(c) = &f.calendar {
        out += &format!("\n## Calendar today\n{}\n", util::wrap_outside("brief:calendar", c));
    }
    if let Some(s) = &f.sleep {
        out += &format!("\n## Sleep\n{s}\n");
    }
    if let Some(r) = &f.reminders {
        out += &format!("\n## Alarms and reminders today\n{}\n", util::wrap_outside("brief:jobs", r));
    }
    if let Some(l) = &f.lists {
        out += &format!("\n## Open to-dos\n{l}\n");
    }
    if f.weather.is_none() && f.calendar.is_none() && f.sleep.is_none() && f.reminders.is_none() && f.lists.is_none() {
        out += "\nNothing is known about today yet. Say good morning and that the day looks open.\n";
    }
    out
}

/// The brief in plain words, written by the core (no model). Used when the model gave nothing.
pub fn plain(f: &Facts) -> String {
    let mut lines = vec!["Good morning.".to_string()];
    if let Some(w) = &f.weather {
        lines.push(w.clone());
    }
    match &f.calendar {
        Some(c) => lines.push(format!("Today: {}", c.replace('\n', "; "))),
        None => lines.push("Nothing in the calendar today.".into()),
    }
    if let Some(s) = &f.sleep {
        lines.push(s.clone());
    }
    if let Some(r) = &f.reminders {
        lines.push(format!("Coming up: {}", r.replace('\n', "; ")));
    }
    if let Some(l) = &f.lists {
        lines.push(l.clone());
    }
    lines.join("\n")
}

/// Send the brief as a notice. `text` is the model's reply; without one the core writes it.
pub fn send(app: &App, text: Option<&str>, thread: Option<&str>) {
    let body = match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => t.to_string(),
        None => plain(&gather(app)),
    };
    scheduler::notice_full(app, &scheduler::Notice { priority: "normal", title: Some("Good morning"), text: &body, silent: false, thread });
}

fn weather_word(code: i64) -> &'static str {
    match code {
        0 => "clear",
        1 | 2 => "partly cloudy",
        3 => "cloudy",
        45 | 48 => "fog",
        51..=57 => "drizzle",
        61..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "thunderstorms",
        _ => "mixed",
    }
}

/// Fetch today's weather where the phone last was (any point from the last week) from Open-Meteo, and keep
/// the line for the brief (`brief.weather`). Blocking: call it off the async threads.
pub fn refresh_weather(app: &App) {
    if let Some(w) = fetch_weather(app) {
        set_setting(app, "brief.weather", &json!({ "at": util::unix_now(), "text": w }).to_string());
    }
}

fn fetch_weather(app: &App) -> Option<String> {
    let (ts, lat, lon, _) = crate::location::last_point(app)?;
    if util::unix_now() - ts > 7 * 86_400 {
        return None;
    }
    let base = std::env::var("AGENT_BRIEF_WEATHER_BASE").unwrap_or_else(|_| "https://api.open-meteo.com".into());
    let url = format!("{base}/v1/forecast?latitude={lat:.3}&longitude={lon:.3}&current=temperature_2m&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=1");
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).http_status_as_error(false).build().into();
    let mut r = agent.get(&url).call().ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    let v: Value = serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()?;
    weather_line(&v)
}

/// The weather line fetched in the last 3 hours, if any. Never a network call.
fn weather(app: &App) -> Option<String> {
    let v: Value = serde_json::from_str(&setting(app, "brief.weather")?).ok()?;
    (util::unix_now() - v["at"].as_i64()? < 3 * 3600).then(|| v["text"].as_str().map(String::from)).flatten()
}

/// "Now 6°C. Today rain, 3 to 11°C, 70% chance of rain." from an Open-Meteo answer.
pub fn weather_line(v: &Value) -> Option<String> {
    let d = &v["daily"];
    let hi = d["temperature_2m_max"][0].as_f64()?;
    let lo = d["temperature_2m_min"][0].as_f64()?;
    let mut s = String::new();
    if let Some(c) = v["current"]["temperature_2m"].as_f64() {
        s += &format!("Now {}°C. ", c.round());
    }
    s += &format!("Today {}, {} to {}°C", weather_word(d["weather_code"][0].as_i64().unwrap_or(-1)), lo.round(), hi.round());
    if let Some(p) = d["precipitation_probability_max"][0].as_f64().filter(|p| *p >= 20.0) {
        s += &format!(", {}% chance of rain", p.round());
    }
    Some(s + ".")
}

fn calendar_today(app: &App, today: &str) -> Option<String> {
    let v = crate::calendar::list(app, Some(today), Some(today), None).ok()?;
    let lines: Vec<String> = v["events"].as_array()?.iter().take(10).map(|e| {
        let start = e["start"].as_str().unwrap_or("");
        let time = if e["all_day"] == true { "all day".to_string() } else { start.get(11..16).unwrap_or(start).to_string() };
        let place = e["location"].as_str().filter(|l| !l.is_empty()).map(|l| format!(" ({l})")).unwrap_or_default();
        format!("{time} {}{place}", util::clean_line(e["title"].as_str().unwrap_or(""), 12))
    }).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn sleep(app: &App, today: &str) -> Option<String> {
    let days = crate::health::days(app, Some(today), Some(today), 1).ok()?;
    let m = days.first()?["metrics"]["sleep_min"].as_f64()? as i64;
    (m > 0).then(|| format!("Slept {} h {:02} min.", m / 60, m % 60))
}

fn reminders_today(app: &App, today: &str) -> Option<String> {
    let z = scheduler::zone(&app.cfg.timezone);
    let end = z.local_to_utc(scheduler::parse_local(&format!("{today} 00:00"))? + 86_400);
    let now = util::unix_now();
    let mut items: Vec<(i64, String)> = scheduler::list_jobs(app, false).into_iter()
        .filter(|j| j["status"] == "active" && matches!(j["kind"].as_str(), Some("alarm" | "reminder")))
        .filter_map(|j| {
            let t = j["next_run_ts"].as_i64().filter(|t| *t >= now && *t < end)?;
            let what = if j["kind"] == "alarm" { "alarm" } else { "reminder" };
            Some((t, format!("{} {what}: {}", &scheduler::local_str(app, t)[11..], util::clean_line(j["text"].as_str().unwrap_or(""), 12))))
        }).collect();
    items.sort();
    items.truncate(8);
    (!items.is_empty()).then(|| items.into_iter().map(|i| i.1).collect::<Vec<_>>().join("\n"))
}

/// Open to-dos, when a lists module keeps them (`list_items`, optionally joined to `lists`). Read softly: any
/// other shape is left out, never an error.
fn open_todos(app: &App) -> Option<String> {
    let c = app.db.lock();
    let has = |t: &str| c.query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", [t], |_| Ok(())).is_ok();
    if !has("list_items") {
        return None;
    }
    let cols: Vec<String> = c.prepare("SELECT name FROM pragma_table_info('list_items')").ok()?
        .query_map([], |r| r.get::<_, String>(0)).ok()?.flatten().collect();
    let open = if cols.iter().any(|c| c == "done") { "COALESCE(i.done, 0) = 0" } else if cols.iter().any(|c| c == "status") { "COALESCE(i.status, '') NOT IN ('done', 'deleted', 'removed')" } else { return None };
    let deleted = if cols.iter().any(|c| c == "deleted") { " AND COALESCE(i.deleted, 0) = 0" } else { "" };
    let by_list = has("lists") && cols.iter().any(|c| c == "list_id");
    let sql = if by_list {
        format!("SELECT COALESCE(l.name, 'list'), COUNT(*) FROM list_items i LEFT JOIN lists l ON l.id = i.list_id WHERE {open}{deleted} GROUP BY 1 ORDER BY 2 DESC LIMIT 5")
    } else {
        format!("SELECT 'to-dos', COUNT(*) FROM list_items i WHERE {open}{deleted}")
    };
    let rows: Vec<(String, i64)> = c.prepare(&sql).ok()?.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).ok()?.flatten().filter(|r| r.1 > 0).collect();
    (!rows.is_empty()).then(|| format!("Open: {}.", rows.iter().map(|(n, k)| format!("{k} on {}", util::clean_line(n, 4))).collect::<Vec<_>>().join(", ")))
}

// ---------------- HTTP (merged into the /v1 router; the guard runs first) ----------------

use crate::auth::{Caller, Scope};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};

fn bad(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": { "code": "invalid_input", "message": msg.into() } }))).into_response()
}

fn not_device(c: &Caller) -> Option<Response> {
    (!matches!(c.scope, Scope::Owner | Scope::Client)).then(|| (StatusCode::FORBIDDEN, Json(json!({ "error": { "code": "forbidden_scope", "message": "This needs an owner or app token." } }))).into_response())
}

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => bad(e),
    }
}

/// `POST,GET /v1/signals`, `GET,PATCH /v1/brief`, `POST /v1/brief/run`, `GET,PATCH /v1/travel`.
pub fn router() -> Router<Arc<App>> {
    Router::new()
        .route("/v1/signals", post(post_signal).get(get_signals))
        .route("/v1/brief", get(get_brief).patch(patch_brief))
        .route("/v1/brief/run", post(run_brief))
        .route("/v1/travel", get(get_travel).patch(patch_travel))
}

async fn post_signal(State(app): State<Arc<App>>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> Response {
    let device = if c.label.is_empty() { c.token_id.clone() } else { c.label.clone() };
    answer(receive(&app, &device, &b))
}

#[derive(serde::Deserialize)]
struct Lim {
    limit: Option<i64>,
}

async fn get_signals(State(app): State<Arc<App>>, Extension(c): Extension<Caller>, Query(q): Query<Lim>) -> Response {
    if let Some(r) = not_device(&c) {
        return r;
    }
    Json(json!({ "items": list(&app, q.limit.unwrap_or(50)) })).into_response()
}

async fn get_brief(State(app): State<Arc<App>>) -> Response {
    Json(settings(&app)).into_response()
}

async fn patch_brief(State(app): State<Arc<App>>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> Response {
    if let Some(r) = not_device(&c) {
        return r;
    }
    answer(patch_settings(&app, &b))
}

/// Send the brief now, even if one went out today (the settings page's "Try it now").
async fn run_brief(State(app): State<Arc<App>>, Extension(c): Extension<Caller>) -> Response {
    if let Some(r) = not_device(&c) {
        return r;
    }
    let _ = app.db.lock().execute("DELETE FROM settings WHERE key = 'brief.last_day'", []);
    let (day, _) = local_day_minute(&app, util::unix_now());
    let result = if settings(&app)["enabled"] == true { start_brief(&app, &day) } else { "brief_off" };
    Json(json!({ "result": result })).into_response()
}

async fn get_travel(State(app): State<Arc<App>>) -> Response {
    Json(crate::travel::settings(&app)).into_response()
}

async fn patch_travel(State(app): State<Arc<App>>, Extension(c): Extension<Caller>, Json(b): Json<Value>) -> Response {
    if let Some(r) = not_device(&c) {
        return r;
    }
    answer(crate::travel::patch_settings(&app, &b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wake_rule() {
        let h = |x: i64| x * 60;
        // A long night, unlocked in the morning: a wake.
        assert!(is_wake("unlock", h(7), Some(7 * 3600)));
        // A short screen-off is not a night.
        assert!(!is_wake("unlock", h(7), Some(2 * 3600)));
        assert!(is_wake("unlock", h(7), Some(NIGHT_OFF_SECS)));
        // Only between 03:00 and 12:00.
        assert!(!is_wake("unlock", h(2) + 59, Some(8 * 3600)));
        assert!(is_wake("unlock", h(3), Some(8 * 3600)));
        assert!(!is_wake("unlock", h(12), Some(8 * 3600)));
        assert!(!is_wake("alarm", h(14), None));
        assert!(is_wake("alarm", h(6) + 30, None) && is_wake("sleep", h(9), None));
        assert!(!is_wake("something", h(8), None));
    }

    #[test]
    fn once_a_day() {
        assert!(brief_due(None, "2026-11-17"));
        assert!(!brief_due(Some("2026-11-17"), "2026-11-17"));
        assert!(brief_due(Some("2026-11-16"), "2026-11-17"));
    }

    #[test]
    fn weather_and_plain_text() {
        let v = json!({ "current": { "temperature_2m": 6.2 }, "daily": { "weather_code": [61], "temperature_2m_max": [11.4], "temperature_2m_min": [2.6], "precipitation_probability_max": [70] } });
        assert_eq!(weather_line(&v).unwrap(), "Now 6°C. Today rain, 3 to 11°C, 70% chance of rain.");
        assert!(weather_line(&json!({})).is_none());
        let f = Facts { weather: None, calendar: Some("09:00 Dentist\n13:00 Lunch".into()), sleep: Some("Slept 7 h 05 min.".into()), reminders: None, lists: None };
        let p = plain(&f);
        assert!(p.starts_with("Good morning.") && p.contains("Today: 09:00 Dentist; 13:00 Lunch") && p.contains("Slept 7 h 05 min."), "{p}");
        let empty = plain(&Facts { weather: None, calendar: None, sleep: None, reminders: None, lists: None });
        assert!(empty.contains("Nothing in the calendar today."));
    }
}
