//! Scheduler (docs/00 Scheduler, docs/05 #7): the core timer, jobs, the `scheduler` plugin tools,
//! and unasked messages (`assistant.notice`) with quiet hours and a daily cap.
//!
//! Times are stored as local wall time + zone, so daylight saving does not move them. A job missed
//! while the computer slept runs once on wake, marked late.

use crate::tz::{self, Zone};
use crate::{App, dreaming, util};
use rusqlite::params;
use serde_json::{Value, json};
use std::sync::Arc;

pub const SYSTEM_THREAD: &str = "system:scheduler";
const LATE_AFTER_SECS: i64 = 120;
/// An alarm nobody stops is marked missed after this long (seconds).
pub const RING_SECS: i64 = 10 * 60;
pub const DEFAULT_SNOOZE_MINS: i64 = 9;

/// Columns and tables added after the first release. Safe to run on every start.
pub fn migrate(db: &crate::db::Db) {
    crate::ambient::ensure_noticed_column(db);
    let c = db.lock();
    for col in ["ring INTEGER NOT NULL DEFAULT 0", "sound TEXT", "event_id TEXT", "name TEXT", "last_result TEXT", "last_thread TEXT", "runs INTEGER NOT NULL DEFAULT 0", "fired_at INTEGER", "description TEXT"] {
        let _ = c.execute(&format!("ALTER TABLE jobs ADD COLUMN {col}"), []);
    }
    for col in ["title TEXT", "silent INTEGER NOT NULL DEFAULT 0", "thread_id TEXT", "job_id TEXT", "rating TEXT", "rating_note TEXT"] {
        let _ = c.execute(&format!("ALTER TABLE notices ADD COLUMN {col}"), []);
    }
    let _ = c.execute_batch(
        "CREATE TABLE IF NOT EXISTS alarm_rings (id TEXT PRIMARY KEY, job_id TEXT NOT NULL, text TEXT NOT NULL, sound TEXT, started INTEGER NOT NULL, status TEXT NOT NULL, ended INTEGER);",
    );
}

/// Local wall time for a UTC instant, e.g. `2026-11-14 07:00`.
/// Skip one day of a repeating alarm, reminder or task, or bring it back (`undo`). Every other day stays as it was.
pub fn skip_job_day(app: &App, id: &str, date: &str, undo: bool) -> Result<Value, String> {
    let date = date.trim();
    let ok = date.len() == 10 && date.split('-').count() == 3 && parse_local(&format!("{date} 00:00")).is_some();
    if !ok {
        return Err("date looks like 2026-11-20".into());
    }
    let job = get_job(app, id).filter(|j| j["status"] == "active").ok_or("no such active job")?;
    if job["event_id"].is_string() || !(job["schedule"]["every"].is_string()) {
        return Err("only a job that repeats every day, on chosen days or every week can skip one day".into());
    }
    let mut sched = job["schedule"].clone();
    let mut skip: Vec<String> = sched["skip"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    skip.retain(|d| d != date);
    if !undo {
        skip.push(date.to_string());
        skip.sort();
        skip.dedup();
        let (y, m, d, _) = tz::civil(util::unix_now() - 400 * 86_400);
        let keep_from = format!("{y:04}-{m:02}-{d:02}");
        skip.retain(|d| *d >= keep_from);
    }
    sched["skip"] = json!(skip);
    let tz_name = job["timezone"].as_str().unwrap_or("UTC");
    let next = next_run(&sched, tz_name, util::unix_now());
    app.db.lock().execute("UPDATE jobs SET schedule = ?1, next_run = ?2 WHERE id = ?3", params![sched.to_string(), next, id]).map_err(|e| e.to_string())?;
    let mut v = get_job(app, id).ok_or("job vanished")?;
    v["date"] = json!(date);
    v["skipped"] = json!(!undo);
    Ok(v)
}

/// Jobs that never named a zone of their own follow the account's zone. Run when an account starts, so a change of
/// zone (for example the server moving from Los Angeles to Vancouver, which stopped changing its clocks) takes effect
/// for the reports and reminders already made: their zone is updated and their next time recomputed.
pub fn follow_zone(app: &App) -> usize {
    let now = util::unix_now();
    let rows: Vec<(String, String, String, String, bool)> = {
        let c = app.db.lock();
        let Ok(mut st) = c.prepare("SELECT id, schedule, timezone, kind, next_run IS NULL FROM jobs WHERE status = 'active' AND kind != 'event' AND timezone != ?1") else { return 0 };
        st.query_map([&app.cfg.timezone], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let mut n = 0;
    for (id, sched, _old, _kind, waits_for_event) in rows {
        let s: Value = serde_json::from_str(&sched).unwrap_or(Value::Null);
        if s["timezone"].is_string() {
            continue; // this one was set for a zone on purpose
        }
        let next = if waits_for_event || !(s["every"].is_string() || s["every_minutes"].is_number()) { None } else { next_run(&s, &app.cfg.timezone, now) };
        let c = app.db.lock();
        let _ = match next {
            Some(t) => c.execute("UPDATE jobs SET timezone = ?1, next_run = ?2 WHERE id = ?3", params![app.cfg.timezone, t, id]),
            None => c.execute("UPDATE jobs SET timezone = ?1 WHERE id = ?2", params![app.cfg.timezone, id]),
        };
        n += 1;
    }
    n
}

/// The clock for the prompt. A model has no clock of its own and guesses (it said 3:31 PM at 11:32 AM), so every
/// prompt starts with the real local time, the day and the zone.
pub fn now_line(app: &App) -> String {
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let now = util::unix_now();
    let off = zone(&app.cfg.timezone).offset_at(now);
    let local = now + off;
    let (y, mo, d, secs) = tz::civil(local);
    let (h, m) = ((secs / 3600) as i64, ((secs % 3600) / 60) as i64);
    let (h12, ampm) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" });
    let sign = if off < 0 { '-' } else { '+' };
    format!("Now: {} {} {} {}, {}:{:02} {} ({}), UTC{}{:02}:{:02}. Use this for today, tonight and any time you say; never guess the time.",
        DAYS[(local.div_euclid(86_400) + 4).rem_euclid(7) as usize], d, MONTHS[(mo as usize).saturating_sub(1).min(11)], y, h12, m, ampm, app.cfg.timezone, sign, off.abs() / 3600, (off.abs() % 3600) / 60)
}

pub fn local_str(app: &App, utc: i64) -> String {
    let z = zone(&app.cfg.timezone);
    let (y, mo, d, s) = tz::civil(utc + z.offset_at(utc));
    format!("{y:04}-{mo:02}-{d:02} {:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// `days` may be a list of weekday numbers (0 = Sunday) or a word.
fn parse_days(v: &Value) -> Result<Option<Vec<i64>>, String> {
    let list = match v {
        Value::Null => return Ok(None),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "weekdays" => vec![1, 2, 3, 4, 5],
            "weekends" => vec![0, 6],
            "every day" | "daily" | "everyday" => (0..7).collect(),
            "" | "once" => return Ok(None),
            other => {
                // "[2,4,5]", "2,4,5", "tue thu fri", "Tuesday, Thursday and Friday": a small model often sends the list as text.
                let names = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
                let mut found: Vec<i64> = Vec::new();
                for tok in other.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty() && *t != "and") {
                    let d = tok.parse::<i64>().ok().filter(|d| (0..7).contains(d)).or_else(|| names.iter().position(|n| tok.starts_with(n)).map(|p| p as i64));
                    match d {
                        Some(d) => if !found.contains(&d) { found.push(d) },
                        None => return Err(format!("days '{other}' not understood; use a list of 0-6 (0 = Sunday), weekdays, weekends or every day")),
                    }
                }
                if found.is_empty() {
                    return Err(format!("days '{other}' not understood; use a list of 0-6 (0 = Sunday), weekdays, weekends or every day"));
                }
                found
            }
        },
        Value::Array(a) => a.iter().map(|d| d.as_i64().filter(|d| (0..7).contains(d))).collect::<Option<Vec<_>>>().ok_or("days are numbers 0-6, 0 = Sunday")?,
        _ => return Err("days are numbers 0-6, 0 = Sunday".into()),
    };
    if list.is_empty() { Ok(None) } else { Ok(Some(list)) }
}

/// The offset in seconds a zone has at a moment (for tests and displays).
pub fn zone_offset(name: &str, at: i64) -> i64 {
    zone(name).offset_at(at)
}

pub(crate) fn zone(name: &str) -> Zone {
    Zone::load(name).unwrap_or_else(Zone::utc)
}

pub(crate) fn parse_hm(s: &str) -> Option<i64> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    (0..24).contains(&h).then_some(())?;
    (0..60).contains(&m).then_some(h * 3600 + m * 60)
}

/// "2026-11-12 08:30" -> local seconds (as if UTC).
pub(crate) fn parse_local(s: &str) -> Option<i64> {
    let (d, t) = s.trim().split_once([' ', 'T'])?;
    let mut it = d.split('-');
    let (y, mo, da): (i64, u32, u32) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    Some(tz::local_midnight(y, mo, da) + parse_hm(&t[..t.len().min(5)])?)
}

/// Next run strictly after `after` (UTC). Schedules:
/// `{"at": "YYYY-MM-DD HH:MM"}` once, `{"every": "day", "at": "HH:MM"}`,
/// `{"every": "week", "weekday": 0-6 (0 = Sunday), "at": "HH:MM"}`, `{"every_minutes": n}`.
pub fn next_run(schedule: &Value, zone_name: &str, after: i64) -> Option<i64> {
    let z = zone(zone_name);
    if let Some(n) = schedule["every_minutes"].as_i64() {
        let n = n.clamp(5, 7 * 24 * 60);
        return Some(after + n * 60);
    }
    match schedule["every"].as_str() {
        None => {
            let local = parse_local(schedule["at"].as_str()?)?;
            let utc = z.local_to_utc(local);
            (utc > after).then_some(utc)
        }
        Some(every) => {
            let at = parse_hm(schedule["at"].as_str().unwrap_or("09:00"))?;
            let local_now = after + z.offset_at(after);
            let (y, m, d, _) = tz::civil(local_now);
            let today = tz::local_midnight(y, m, d);
            let skipped = |day: i64| -> bool {
                let (sy, sm, sd, _) = tz::civil(day);
                schedule["skip"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(&format!("{sy:04}-{sm:02}-{sd:02}"))))
            };
            for k in 0..60 {
                let day = today + k * 86_400;
                if skipped(day) {
                    continue;
                }
                if every == "week" {
                    let wd = ((day / 86_400) + 4).rem_euclid(7);
                    if wd != schedule["weekday"].as_i64().unwrap_or(1) {
                        continue;
                    }
                } else if every == "days" {
                    let wd = ((day / 86_400) + 4).rem_euclid(7);
                    if !schedule["days"].as_array().is_some_and(|a| a.iter().any(|d| d.as_i64() == Some(wd))) {
                        continue;
                    }
                } else if every != "day" {
                    return None;
                }
                let utc = z.local_to_utc(day + at);
                if utc > after {
                    return Some(utc);
                }
            }
            None
        }
    }
}

pub fn create_job(app: &App, kind: &str, text: &str, schedule: &Value, outside: bool, created_by: &str, origin: Option<&str>) -> Result<Value, String> {
    create_job_with(app, kind, text, schedule, outside, created_by, origin, &JobExtra::default())
}

/// Ringing and links a job may carry.
#[derive(Default, Clone)]
pub struct JobExtra {
    pub ring: bool,
    pub sound: Option<String>,
    pub event_id: Option<String>,
    /// Fixed first run (calendar jobs compute their own).
    pub next_run: Option<i64>,
    /// A short name for a task ("Morning brief"), shown in the Automation list.
    pub name: Option<String>,
    /// One short line saying what the task does, shown in Upcoming instead of its prompt.
    pub description: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn create_job_with(app: &App, kind: &str, text: &str, schedule: &Value, outside: bool, created_by: &str, origin: Option<&str>, x: &JobExtra) -> Result<Value, String> {
    let tzname = schedule["timezone"].as_str().map(String::from).unwrap_or_else(|| app.cfg.timezone.clone());
    if Zone::load(&tzname).is_none() {
        return Err(format!("unknown timezone '{tzname}'"));
    }
    let mut schedule = schedule.clone();
    if let Some(o) = schedule.as_object_mut() {
        o.retain(|_, v| !v.is_null());
    }
    // A task started by a phone notification has no time; it waits for the notification.
    let on_event = schedule["on"].is_string();
    let next: Option<i64> = match x.next_run {
        Some(n) => Some(n),
        None if on_event => None,
        None => Some(next_run(&schedule, &tzname, util::unix_now()).ok_or("could not work out when this runs (is the time in the past?)")?),
    };
    if let Some(s) = &x.sound {
        crate::sounds::check(app, s)?;
    }
    let id = util::new_id("job");
    // Something from outside content may remind, but never ring (no 3 a.m. alarms from a web page).
    let ring = x.ring && !outside;
    app.db.lock().execute(
        "INSERT INTO jobs (id, kind, text, schedule, timezone, next_run, outside, created, created_by, origin_thread, ring, sound, event_id, name, description) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![id, kind, util::clean_line(text, if kind == "task" { 300 } else { 80 }), schedule.to_string(), tzname, next, outside as i64, util::now_iso(), created_by, origin, ring as i64, x.sound, x.event_id, x.name, x.description.as_deref().map(|d| util::clean_line(d, 90)).filter(|d| !d.is_empty())],
    ).map_err(|e| e.to_string())?;
    if on_event && schedule["on"] == "place" {
        fences_changed(app);
    }
    Ok(json!({ "id": id, "kind": kind, "name": x.name, "next_run": next.map(util::iso), "next_run_local": next.map(|n| local_str(app, n)), "timezone": tzname, "outside": outside, "ring": ring, "sound": x.sound, "schedule": schedule }))
}

fn job_row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    let sched: String = r.get(3)?;
    Ok(json!({ "id": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "text": r.get::<_, String>(2)?,
        "schedule": serde_json::from_str::<Value>(&sched).unwrap_or(Value::Null), "timezone": r.get::<_, String>(4)?,
        "next_run": r.get::<_, Option<i64>>(5)?.map(util::iso), "last_run": r.get::<_, Option<i64>>(6)?.map(util::iso),
        "outside": r.get::<_, i64>(7)? != 0, "status": r.get::<_, String>(8)?, "created": r.get::<_, String>(9)?,
        "created_by": r.get::<_, String>(10)?, "ring": r.get::<_, i64>(11)? != 0, "sound": r.get::<_, Option<String>>(12)?,
        "event_id": r.get::<_, Option<String>>(13)?, "next_run_ts": r.get::<_, Option<i64>>(5)?,
        "name": r.get::<_, Option<String>>(14)?, "last_result": r.get::<_, Option<String>>(15)?, "last_thread": r.get::<_, Option<String>>(16)?,
        "runs": r.get::<_, i64>(17)?, "description": r.get::<_, Option<String>>(18)?, "trigger": serde_json::from_str::<Value>(&sched).ok().map(|v| v["on"].as_str().map(String::from)).unwrap_or(None).filter(|o| o == "notification" || o == "place").map(|o| json!(o)).unwrap_or(json!("time")) }))
}

const JOB_COLS: &str = "id, kind, text, schedule, timezone, next_run, last_run, outside, status, created, created_by, ring, sound, event_id, name, last_result, last_thread, runs, description";

pub fn get_job(app: &App, id: &str) -> Option<Value> {
    app.db.lock().query_row(&format!("SELECT {JOB_COLS} FROM jobs WHERE id = ?1"), [id], job_row).ok()
}

pub fn list_jobs(app: &App, include_system: bool) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare(&format!("SELECT {JOB_COLS} FROM jobs WHERE status IN ('active', 'paused') ORDER BY status, next_run")).expect("prepare");
    let mut out: Vec<Value> = st.query_map([], job_row).map(|x| x.flatten().filter(|j| include_system || j["kind"] != "system").collect()).unwrap_or_default();
    drop(st);
    drop(c);
    for j in &mut out {
        if let Some(n) = j["next_run_ts"].as_i64() {
            j["next_run_local"] = json!(local_str(app, n));
        }
        j.as_object_mut().map(|o| o.remove("next_run_ts"));
    }
    out
}

pub fn cancel_job(app: &App, id: &str) -> bool {
    let done = app.db.lock().execute("UPDATE jobs SET status = 'cancelled' WHERE id = ?1 AND status IN ('active', 'paused') AND kind != 'system'", [id]).map(|n| n > 0).unwrap_or(false);
    if done {
        fences_changed(app);
    }
    done
}

const KNOWN_APPS: &[(&str, &str)] = &[
    ("com.google.android.gm", "Gmail"), ("com.microsoft.office.outlook", "Outlook"), ("com.whatsapp", "WhatsApp"),
    ("org.telegram.messenger", "Telegram"), ("com.google.android.apps.messaging", "Messages (SMS)"), ("com.discord", "Discord"),
    ("com.slack", "Slack"), ("com.google.android.calendar", "Google Calendar"), ("com.instagram.android", "Instagram"),
    ("com.amazon.mShop.android.shopping", "Amazon"), ("com.ubercab", "Uber"), ("com.google.android.dialer", "Phone (missed calls)"),
];

/// `model` (a profile id; "" or "auto" clears it) and `quiet` on a task's schedule. A task with its own
/// model keeps it for the whole run: loading a plugin that has a model of its own does not move it.
fn apply_model(app: &App, sched: &mut Value, a: &Value) -> Result<(), String> {
    if let Some(m) = a["model"].as_str() {
        if m.is_empty() || m == "auto" {
            sched.as_object_mut().map(|o| o.remove("model"));
        } else if crate::models::get(app, m).is_some() {
            sched["model"] = json!(m);
        } else {
            return Err(format!("no model '{m}'; use auto or one of: {}", crate::models::profile_ids(app).join(", ")));
        }
    }
    for k in ["quiet", "report"] {
        if let Some(q) = a[k].as_bool() {
            if q { sched[k] = json!(true) } else { sched.as_object_mut().map(|o| o.remove(k)); }
        }
    }
    Ok(())
}

fn task_desc(a: &Value) -> Option<String> {
    a["description"].as_str().map(|d| util::clean_line(d, 90)).filter(|d| !d.is_empty())
}

fn task_name(a: &Value, text: &str) -> String {
    a["name"].as_str().map(|n| util::clean_line(n, 8)).filter(|n| !n.is_empty()).unwrap_or_else(|| util::clean_line(text, 6))
}

/// Pause or resume a task (or any repeating job). A paused time task keeps its schedule.
pub fn set_paused(app: &App, id: &str, pause: bool) -> Result<Value, String> {
    let job = get_job(app, id).filter(|j| j["status"] == "active" || j["status"] == "paused").ok_or("no such job")?;
    if job["kind"] == "system" {
        return Err("this is a core job".into());
    }
    let next = if pause { None } else { next_run(&job["schedule"], job["timezone"].as_str().unwrap_or("UTC"), util::unix_now()) };
    app.db.lock().execute("UPDATE jobs SET status = ?1, next_run = ?2 WHERE id = ?3", params![if pause { "paused" } else { "active" }, next, id]).map_err(|e| e.to_string())?;
    get_job(app, id).ok_or_else(|| "job vanished".into())
}

/// Start a task now, as if it were due. Returns the thread its report goes to.
pub fn run_now(app: &Arc<App>, id: &str) -> Result<String, String> {
    let job = get_job(app, id).filter(|j| j["kind"] == "task" && j["status"] != "cancelled").ok_or("no such task")?;
    Ok(start_task(app, id, job["text"].as_str().unwrap_or(""), job["outside"] == true, false, None))
}

/// Each run gets its own thread, so its report can be read later. `event` is what set it off
/// (a notification), outside content.
fn start_task(app: &Arc<App>, id: &str, text: &str, outside: bool, late: bool, event: Option<Value>) -> String {
    let now = util::unix_now();
    let thread = format!("system:task:{id}:{now}");
    let name = get_job(app, id).and_then(|j| j["name"].as_str().map(String::from)).unwrap_or_else(|| util::clean_line(text, 8));
    app.ensure_thread(&thread, "system", &format!("task: {name}"));
    let _ = app.db.lock().execute("UPDATE jobs SET last_thread = ?1, runs = runs + 1, last_run = ?2 WHERE id = ?3", params![thread, now, id]);
    app.start_turn(&thread, "schedule", json!({ "job": { "id": id, "kind": "task", "text": text, "outside": outside, "late": late, "priority": "normal", "event": event } }));
    thread
}

/// The answer a task's run wrote, kept on the job for the Automation page.
pub fn task_finished(app: &App, job_id: &str, report: &str) {
    let r: String = report.chars().take(600).collect();
    let _ = app.db.lock().execute("UPDATE jobs SET last_result = ?1 WHERE id = ?2", params![r, job_id]);
}

/// Apps whose notifications can start a task, for the phone to watch. Only these are sent.
pub fn watched_apps(app: &App) -> Vec<String> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT schedule FROM jobs WHERE kind = 'task' AND status = 'active'").expect("prepare");
    let mut v: Vec<String> = st.query_map([], |r| r.get::<_, String>(0)).map(|x| x.flatten()
        .filter_map(|s| serde_json::from_str::<Value>(&s).ok()).filter(|s| s["on"] == "notification")
        .filter_map(|s| s["app"].as_str().map(String::from)).collect()).unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

/// A notification from the phone. Starts every matching task (at most once a minute each).
/// Title and text are outside content: they reach the task wrapped, never as orders.
pub fn notification(app: &Arc<App>, b: &Value) -> Result<Value, String> {
    let pkg = b["app"].as_str().ok_or("app is required")?;
    let title: String = b["title"].as_str().unwrap_or("").chars().take(200).collect();
    let text: String = b["text"].as_str().unwrap_or("").chars().take(2000).collect();
    let label: String = b["app_name"].as_str().unwrap_or(pkg).chars().take(60).collect();
    let now = util::unix_now();
    let jobs: Vec<(String, String, Value, Option<i64>)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, text, schedule, fired_at FROM jobs WHERE kind = 'task' AND status = 'active' AND next_run IS NULL").expect("prepare");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, serde_json::from_str::<Value>(&r.get::<_, String>(2)?).unwrap_or(Value::Null), r.get(3)?)))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let hay = format!("{title}\n{text}").to_lowercase();
    let mut started = vec![];
    for (id, task, sched, fired) in jobs {
        if sched["on"] != "notification" || sched["app"].as_str() != Some(pkg) {
            continue;
        }
        if let Some(word) = sched["contains"].as_str().filter(|w| !w.is_empty())
            && !hay.contains(&word.to_lowercase())
        {
            continue;
        }
        if fired.is_some_and(|f| now - f < 60) {
            continue; // a burst of mail starts it once
        }
        let _ = app.db.lock().execute("UPDATE jobs SET fired_at = ?1 WHERE id = ?2", params![now, id]);
        let ev = json!({ "what": "notification", "app": label, "package": pkg, "title": title, "text": text });
        started.push(json!({ "job": id, "thread": start_task(app, &id, &task, false, false, Some(ev)) }));
    }
    Ok(json!({ "started": started }))
}

/// Is `now` inside a place job's window (`after`, `before`, `days`, local time)? No window = always.
pub(crate) fn in_window(app: &App, sched: &Value, now: i64) -> bool {
    let z = zone(sched["timezone"].as_str().unwrap_or(&app.cfg.timezone));
    let local = now + z.offset_at(now);
    let secs = tz::civil(local).3 as i64;
    let (a, b) = (sched["after"].as_str().and_then(parse_hm), sched["before"].as_str().and_then(parse_hm));
    let in_time = match (a, b) {
        (Some(a), Some(b)) if a <= b => secs >= a && secs < b,
        (Some(a), Some(b)) => secs >= a || secs < b, // wraps past midnight
        (Some(a), None) => secs >= a,
        (None, Some(b)) => secs < b,
        (None, None) => true,
    };
    let wd = (local.div_euclid(86_400) + 4).rem_euclid(7);
    let in_day = sched["days"].as_array().is_none_or(|d| d.iter().any(|x| x.as_i64() == Some(wd)));
    in_time && in_day
}

/// The user arrived at or left a place. Starts every matching job (reminder: a notice; task: a run).
/// At most once every 10 minutes each; a job with `once` is finished after it fires.
pub fn place_event(app: &Arc<App>, place: &str, what: &str) {
    crate::detect::on_place(app, place, what); // leaving home (docs/10, Noticing)
    let now = util::unix_now();
    let jobs: Vec<(String, String, String, Value, Option<i64>, bool)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, kind, text, schedule, fired_at, outside FROM jobs WHERE kind IN ('task','reminder') AND status = 'active' AND next_run IS NULL").expect("prepare");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, serde_json::from_str::<Value>(&r.get::<_, String>(3)?).unwrap_or(Value::Null), r.get(4)?, r.get::<_, i64>(5)? != 0)))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let label = crate::location::get_place(app, place).and_then(|p| p["name"].as_str().map(String::from)).unwrap_or_else(|| "a place".into());
    let mut changed = false;
    for (id, kind, text, sched, fired, outside) in jobs {
        if sched["on"] != "place" || sched["place"].as_str() != Some(place) || sched["when"].as_str().unwrap_or("arrive") != what {
            continue;
        }
        if fired.is_some_and(|f| now - f < 600) {
            continue;
        }
        if !in_window(app, &sched, now) {
            continue; // outside its hours or days: it keeps waiting
        }
        let once = sched["once"].as_bool().unwrap_or(true);
        changed |= once;
        crate::audit::event(app, "place.job", json!({ "job": id, "kind": kind, "place": label, "what": what, "once": once }));
        let _ = app.db.lock().execute(
            "UPDATE jobs SET fired_at = ?1, last_run = ?1, runs = runs + 1, status = CASE WHEN ?2 THEN 'done' ELSE status END WHERE id = ?3", params![now, once, id]);
        if kind == "reminder" {
            notice_for_job(app, "urgent", &text, &id);
        } else {
            let ev = json!({ "what": "place", "when": what, "place": label, "package": format!("place:{place}") });
            start_task(app, &id, &text, outside, false, Some(ev));
        }
    }
    if changed {
        fences_changed(app);
    }
}

fn place_schedule(app: &App, a: &Value) -> Result<Value, String> {
    let (id, label) = crate::location::find_place(app, a["place"].as_str().unwrap_or_default())?;
    let when = a["when"].as_str().unwrap_or("arrive");
    if !["arrive", "leave"].contains(&when) {
        return Err("when is arrive or leave".into());
    }
    let once = a["once"].as_bool().unwrap_or(true);
    let _ = label;
    let mut s = json!({ "on": "place", "place": id, "when": when, "once": once });
    // Place and time together: only fire inside a window of the day ("after 18:00", "between 18:00 and 23:00") and on chosen days.
    for k in ["after", "before"] {
        if let Some(t) = a[k].as_str().filter(|t| !t.trim().is_empty()) {
            parse_hm(t).ok_or_else(|| format!("{k} must be a clock time like 18:00"))?;
            s[k] = json!(t.trim());
        }
    }
    if let Some(d) = parse_days(&a["days"])? {
        s["days"] = json!(d);
    }
    Ok(s)
}

/// "in 30 minutes" -> a one-off schedule at that local time.
fn in_minutes_schedule(app: &App, m: i64) -> Value {
    let utc = util::unix_now() + m.clamp(1, 60 * 24 * 366) * 60;
    json!({ "at": local_str(app, utc) })
}

/// A time with no date means the next time the clock shows it.
fn next_clock_time(app: &App, hm: &str) -> Result<String, String> {
    if hm.trim().len() > 5 {
        return Ok(hm.trim().to_string());
    }
    let secs = parse_hm(hm).ok_or("time must look like 07:30 or 2026-11-14 07:30")?;
    let now = util::unix_now();
    let today = &local_str(app, now)[..10];
    let z = zone(&app.cfg.timezone);
    let at = parse_local(&format!("{today} 00:00")).ok_or("bad date")? + secs;
    let at = if z.local_to_utc(at) > now { at } else { at + 86_400 };
    Ok(local_str(app, z.local_to_utc(at)))
}

/// Change a job: its words, its time or repeat, its ring and sound. Calendar jobs follow their
/// event and are changed through the calendar.
pub fn update_job(app: &App, id: &str, a: &Value, outside: bool) -> Result<Value, String> {
    let job = get_job(app, id).filter(|j| j["status"] == "active").ok_or("no such active job")?;
    if job["kind"] == "system" {
        return Err("this is a core job and cannot be changed".into());
    }
    if job["event_id"].is_string() {
        return Err("this reminder belongs to a calendar event; change the event instead".into());
    }
    let mut sched = job["schedule"].clone();
    if let Some(m) = a["every_minutes"].as_i64() {
        if job["kind"] == "task" && m < 15 {
            return Err("every_minutes must be 15 or more".into());
        }
        sched = json!({ "every_minutes": m.clamp(5, 7 * 24 * 60), "timezone": sched["timezone"] });
    } else if sched["on"].is_string() {
        // A task started by notifications: only its app and word can change here.
        if let Some(p) = a["app"].as_str().filter(|p| !p.trim().is_empty()) { sched["app"] = json!(p.trim()); }
        if let Some(w) = a["contains"].as_str() { sched["contains"] = if w.trim().is_empty() { Value::Null } else { json!(w.trim()) }; }
    } else if let Some(m) = a["in_minutes"].as_i64() {
        sched = in_minutes_schedule(app, m);
    } else {
        let o = sched.as_object_mut().ok_or("bad schedule")?;
        if let Some(d) = parse_days(&a["days"])? {
            o.insert("every".into(), json!("days"));
            o.insert("days".into(), json!(d));
        }
        if a["every"].is_string() {
            match a["every"].as_str() {
                Some("once" | "") => { o.remove("every"); o.remove("days"); o.remove("weekday"); }
                Some(e) => { o.insert("every".into(), json!(e)); }
                None => {}
            }
        }
        if !a["weekday"].is_null() { o.insert("weekday".into(), a["weekday"].clone()); }
        if let Some(at) = a["at"].as_str().or(a["time"].as_str()) {
            let at = if o.contains_key("every") { at.trim().to_string() } else { next_clock_time(app, at)? };
            o.insert("at".into(), json!(at));
        }
    }
    // Keep what the other branches rebuilt without: a quiet task stays quiet, a task keeps its model.
    if let Some(o) = job["schedule"].as_object() {
        for k in ["quiet", "model", "report", "mind"] {
            if sched.get(k).is_none() && let Some(v) = o.get(k) {
                sched[k] = v.clone();
            }
        }
    }
    if job["kind"] == "task" {
        apply_model(app, &mut sched, a)?;
    }
    let tzname = job["timezone"].as_str().unwrap_or("UTC").to_string();
    let next = if sched["on"].is_string() { None } else { Some(next_run(&sched, &tzname, util::unix_now()).ok_or("could not work out when this runs (is the time in the past?)")?) };
    let max = if job["kind"] == "task" { 300 } else { 80 };
    let text = a["text"].as_str().or(a["label"].as_str()).filter(|t| !t.trim().is_empty()).map(|t| util::clean_line(t, max)).unwrap_or_else(|| job["text"].as_str().unwrap_or("").to_string());
    if let Some(d) = task_desc(a) {
        let _ = app.db.lock().execute("UPDATE jobs SET description = ?1 WHERE id = ?2", params![d, id]);
    }
    if let Some(n) = a["name"].as_str().filter(|n| !n.trim().is_empty()) {
        let _ = app.db.lock().execute("UPDATE jobs SET name = ?1 WHERE id = ?2", params![util::clean_line(n, 8), id]);
    }
    let sound = match &a["sound"] {
        Value::String(s) => { crate::sounds::check(app, s)?; Some(s.clone()) }
        Value::Null => job["sound"].as_str().map(String::from),
        _ => None,
    };
    let outside = outside || job["outside"] == true;
    let ring = a["ring"].as_bool().unwrap_or(job["ring"] == true) && !outside;
    app.db.lock().execute(
        "UPDATE jobs SET text = ?1, schedule = ?2, next_run = ?3, ring = ?4, sound = ?5, outside = ?6 WHERE id = ?7",
        params![text, sched.to_string(), next, ring as i64, sound, outside as i64, id],
    ).map_err(|e| e.to_string())?;
    let mut v = get_job(app, id).ok_or("job vanished")?;
    v["next_run_local"] = json!(next.map(|n| local_str(app, n)));
    v["previous"] = job;
    Ok(v)
}

/// Undo of `update_job`: put the old words, time and sound back.
fn restore_job(app: &App, prev: &Value) -> Result<Value, String> {
    let id = prev["id"].as_str().ok_or("nothing to restore")?;
    let tzname = prev["timezone"].as_str().unwrap_or("UTC");
    let next = next_run(&prev["schedule"], tzname, util::unix_now());
    app.db.lock().execute(
        "UPDATE jobs SET text = ?1, schedule = ?2, next_run = ?3, ring = ?4, sound = ?5, status = CASE WHEN ?3 IS NULL AND schedule NOT LIKE '%\"on\"%' THEN 'done' ELSE status END WHERE id = ?6",
        params![prev["text"].as_str(), prev["schedule"].to_string(), next, (prev["ring"] == true) as i64, prev["sound"].as_str(), id],
    ).map_err(|e| e.to_string())?;
    Ok(json!({ "restored": id }))
}

/// Dates a one-off alarm or reminder may use: a list of `YYYY-MM-DD` (or one text with commas).
fn parse_dates(app: &App, v: &Value) -> Result<Option<Vec<String>>, String> {
    let list: Vec<String> = match v {
        Value::Null => return Ok(None),
        Value::String(s) => s.split([',', ';', ' ']).map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect(),
        Value::Array(a) => a.iter().map(|x| x.as_str().map(|s| s.trim().to_string())).collect::<Option<Vec<_>>>().ok_or("dates are text like 2026-11-20")?,
        _ => return Err("dates are a list of text like 2026-11-20".into()),
    };
    if list.is_empty() {
        return Ok(None);
    }
    let today = local_str(app, util::unix_now())[..10].to_string();
    let mut out: Vec<String> = Vec::new();
    for d in list {
        if d.len() != 10 || parse_local(&format!("{d} 00:00")).is_none() {
            return Err(format!("date '{d}' must look like 2026-11-20"));
        }
        if d < today {
            return Err(format!("{d} is already past (today is {today})"));
        }
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out.sort();
    Ok(Some(out))
}

/// Most alarms or reminders one call may make.
const MAX_AT_ONCE: usize = 60;

/// Alarms and reminders at one or more clock times, repeating on weekdays (`days`) or only on
/// the listed dates (`dates`, never repeating). `time`/`times` are `07:00`; one call makes them all.
fn make_timed(app: &App, kind: &str, a: &Value, outside: bool, created_by: &str, origin: Option<&str>) -> Result<Value, String> {
    let alarm = kind == "alarm";
    if alarm && outside {
        return Err("An alarm cannot be set from outside content (a page, a mail, a file). Ask the user to say it themselves.".into());
    }
    let mut times: Vec<String> = Vec::new();
    if let Some(l) = a["times"].as_array() {
        times.extend(l.iter().filter_map(|t| t.as_str().map(|t| t.trim().to_string())));
    } else if let Some(t) = a["times"].as_str() {
        times.extend(t.split([',', ';']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
    }
    if let Some(t) = a["time"].as_str().or(a["at"].as_str()) {
        times.push(t.trim().to_string());
    }
    times.dedup();
    if times.is_empty() {
        return Err("time is required, e.g. time \"07:00\" or times [\"06:30\",\"07:00\"]".into());
    }
    let label = if alarm { a["label"].as_str().or(a["text"].as_str()) } else { a["text"].as_str() }
        .map(str::trim).filter(|t| !t.is_empty()).unwrap_or(if alarm { "Alarm" } else { "" });
    if label.is_empty() {
        return Err("text is required".into());
    }
    let days = parse_days(&a["days"])?;
    let dates = parse_dates(app, &a["dates"])?;
    if days.is_some() && dates.is_some() {
        return Err("give either days (it repeats every week on them) or dates (only those days, no repeat), not both".into());
    }
    let tz = a["timezone"].clone();
    let mut scheds: Vec<(String, Value)> = Vec::new();
    for t in &times {
        match (&dates, &days) {
            (Some(ds), _) => {
                for d in ds {
                    let at = if t.len() > 5 { t.clone() } else { format!("{d} {t}") };
                    scheds.push((at.clone(), json!({ "at": at, "timezone": tz })));
                }
            }
            (None, Some(d)) => scheds.push((t.clone(), json!({ "every": "days", "days": d, "at": t, "timezone": tz }))),
            (None, None) => scheds.push((t.clone(), json!({ "at": next_clock_time(app, t)?, "timezone": tz }))),
        }
    }
    if scheds.len() > MAX_AT_ONCE {
        return Err(format!("that is {} at once; the most is {MAX_AT_ONCE}. Split it in two calls.", scheds.len()));
    }
    let sound = a["sound"].as_str().filter(|s| !s.is_empty()).map(String::from);
    let ring = alarm || a["ring"].as_bool().unwrap_or(false);
    let (mut made, mut skipped): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
    for (what, sched) in scheds {
        match create_job_with(app, kind, label, &sched, outside, created_by, origin, &JobExtra { ring, sound: sound.clone(), ..Default::default() }) {
            Ok(j) => made.push(j),
            Err(e) => skipped.push(json!({ "when": what, "why": e })),
        }
    }
    if made.is_empty() {
        return Err(skipped.first().and_then(|s| s["why"].as_str()).unwrap_or("nothing was set").to_string());
    }
    if made.len() == 1 && skipped.is_empty() {
        let mut one = made.remove(0);
        one["repeats"] = json!(if days.is_some() { "every week on those days" } else { "NO: this happens once. For every day use days: \"every day\"" });
        return Ok(one);
    }
    let ids: Vec<Value> = made.iter().map(|j| j["id"].clone()).collect();
    Ok(json!({
        "id": ids[0], "ids": ids, "created": made.len(), "skipped": skipped,
        "items": made.iter().map(|j| json!({ "id": j["id"], "next_run_local": j["next_run_local"] })).collect::<Vec<_>>(),
    }))
}

/// Make an alarm. `time` is `07:00` (next time the clock shows it) or `2026-11-14 07:00`.
/// Also: `times` (several clock times), `days` (repeat on those weekdays) or `dates` (only those days).
pub fn create_alarm(app: &App, a: &Value, outside: bool, created_by: &str, origin: Option<&str>) -> Result<Value, String> {
    make_timed(app, "alarm", a, outside, created_by, origin)
}

/// Many alarms in one call: `alarms` is a list of the same arguments `create_alarm` takes.
pub fn create_alarms(app: &App, a: &Value, outside: bool, created_by: &str, origin: Option<&str>) -> Result<Value, String> {
    let list = a["alarms"].as_array().filter(|l| !l.is_empty()).ok_or("alarms is a list like [{\"time\":\"07:00\",\"label\":\"Wake up\",\"dates\":[\"2026-11-20\"]}]")?;
    if list.len() > 30 {
        return Err("at most 30 alarms in one call; split it in two".into());
    }
    let (mut ids, mut items, mut skipped): (Vec<Value>, Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new(), Vec::new());
    for (i, one) in list.iter().enumerate() {
        match make_timed(app, "alarm", one, outside, created_by, origin) {
            Ok(r) => {
                if let Some(l) = r["ids"].as_array() {
                    ids.extend(l.iter().cloned());
                    items.extend(r["items"].as_array().cloned().unwrap_or_default());
                } else {
                    ids.push(r["id"].clone());
                    items.push(json!({ "id": r["id"], "next_run_local": r["next_run_local"] }));
                }
                skipped.extend(r["skipped"].as_array().cloned().unwrap_or_default());
            }
            Err(e) => skipped.push(json!({ "item": i + 1, "why": e })),
        }
    }
    if ids.is_empty() {
        return Err(skipped.first().and_then(|s| s["why"].as_str()).unwrap_or("nothing was set").to_string());
    }
    Ok(json!({ "id": ids[0], "ids": ids, "created": ids.len(), "items": items, "skipped": skipped }))
}

// ---------------- ringing ----------------

fn ring_now(app: &App, job_id: &str, text: &str, sound: Option<&str>) {
    let id = util::new_id("ring");
    let _ = app.db.lock().execute(
        "INSERT INTO alarm_rings (id, job_id, text, sound, started, status) VALUES (?1,?2,?3,?4,?5,'ringing')",
        params![id, job_id, text, sound, util::unix_now()],
    );
    app.feed.emit(None, None, "alarm.ring", json!({
        "ring_id": id, "job_id": job_id, "text": text, "sound": sound.unwrap_or("default"),
        "sound_url": sound.and_then(|s| crate::sounds::url(app, s)), "ring_seconds": RING_SECS, "snooze_minutes": DEFAULT_SNOOZE_MINS,
    }));
}

pub fn ringing(app: &App) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT id, job_id, text, sound, started FROM alarm_rings WHERE status = 'ringing' ORDER BY started").expect("prepare");
    st.query_map([], |r| Ok(json!({ "ring_id": r.get::<_, String>(0)?, "job_id": r.get::<_, String>(1)?, "text": r.get::<_, String>(2)?,
        "sound": r.get::<_, Option<String>>(3)?, "started": util::iso(r.get(4)?) }))).map(|x| x.flatten().collect()).unwrap_or_default()
}

/// The ring to act on: the one named, else the newest ringing one, else (for a job id) its ring.
fn find_ring(app: &App, id: Option<&str>) -> Option<(String, String, String, Option<String>)> {
    let c = app.db.lock();
    let q = "SELECT id, job_id, text, sound FROM alarm_rings WHERE status = 'ringing' AND (?1 IS NULL OR id = ?1 OR job_id = ?1) ORDER BY started DESC LIMIT 1";
    c.query_row(q, [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).ok()
}

fn stop_ring(app: &App, ring: &str, status: &str) {
    let _ = app.db.lock().execute("UPDATE alarm_rings SET status = ?1, ended = ?2 WHERE id = ?3", params![status, util::unix_now(), ring]);
    app.feed.emit(None, None, "alarm.stopped", json!({ "ring_id": ring, "reason": status }));
}

pub fn dismiss(app: &App, id: Option<&str>) -> Result<Value, String> {
    let (ring, job, ..) = find_ring(app, id).ok_or("nothing is ringing")?;
    stop_ring(app, &ring, "dismissed");
    Ok(json!({ "dismissed": ring, "job_id": job }))
}

/// Stop the ring and ring again in `minutes` (a new one-off alarm with the same words and sound).
pub fn snooze(app: &App, id: Option<&str>, minutes: Option<i64>) -> Result<Value, String> {
    let (ring, _job, text, sound) = find_ring(app, id).ok_or("nothing is ringing")?;
    let m = minutes.unwrap_or(DEFAULT_SNOOZE_MINS).clamp(1, 120);
    stop_ring(app, &ring, "snoozed");
    let next = create_job_with(app, "alarm", &text, &in_minutes_schedule(app, m), false, "core", None, &JobExtra { ring: true, sound, ..Default::default() })?;
    Ok(json!({ "snoozed": ring, "minutes": m, "next": next }))
}

/// Alarms nobody stopped become missed, with one urgent notice.
fn expire_rings(app: &App) {
    let old: Vec<(String, String)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, text FROM alarm_rings WHERE status = 'ringing' AND started < ?1").expect("prepare");
        st.query_map([util::unix_now() - RING_SECS], |r| Ok((r.get(0)?, r.get(1)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    for (id, text) in old {
        stop_ring(app, &id, "missed");
        notice(app, "urgent", &format!("Missed alarm: {text}"));
    }
}

/// Nightly dreaming is a core job.
pub fn ensure_system_jobs(app: &App) {
    let exists: bool = app.db.lock().query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id = 'job_dreaming')", [], |r| r.get(0)).unwrap_or(false);
    if !exists {
        let sched = json!({ "every": "day", "at": format!("{:02}:00", app.cfg.dreaming_hour) });
        if let Some(next) = next_run(&sched, &app.cfg.timezone, util::unix_now()) {
            let _ = app.db.lock().execute(
                "INSERT INTO jobs (id, kind, text, schedule, timezone, next_run, created, created_by) VALUES ('job_dreaming', 'system', 'dreaming', ?1, ?2, ?3, ?4, 'core')",
                params![sched.to_string(), app.cfg.timezone, next, util::now_iso()],
            );
        }
    }
}

/// The ready-made reports, made once: a daily one at 09:00 and a nightly one at 21:00 (the
/// server's time zone). They are ordinary tasks: pause, change or cancel them in Automation, and
/// they stay gone (`reports.seeded`). Each ends with a phone notification of the key points.
pub fn ensure_reports(app: &App) {
    let seeded: bool = app.db.lock().query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key = 'reports.seeded')", [], |r| r.get(0)).unwrap_or(true);
    if seeded {
        return;
    }
    let reports = [
        ("Daily report", "09:00", "Today's calendar, reminders and weather", "Daily report. Check today's calendar, reminders due today and the weather where I am. Then call notify (title Today): 2-3 short lines on what is on, anything late or clashing, and one thing to remember. If the day is empty, say so in one line."),
        ("Nightly report", "21:00", "A look back at your day and what's next", "Nightly report. Look back on today: calendar events, steps and sleep from health, where I went, reminders done or missed. Then call notify (title Tonight): 2-3 short lines of highlights, anything unfinished, and what tomorrow starts with. Keep it brief."),
    ];
    for (name, at, desc, text) in reports {
        let sched = json!({ "every": "day", "at": at, "report": true });
        let _ = create_job_with(app, "task", text, &sched, false, "core", None, &JobExtra { name: Some(name.into()), description: Some(desc.into()), ..Default::default() });
    }
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('reports.seeded', '1')", []);
}

/// `scheduler` plugin tools. Jobs made in a turn that held outside content are marked `outside`.
pub fn tool(app: &Arc<App>, thread: &str, outside: bool, tool: &str, a: &Value) -> Result<Value, String> {
    let text = a["text"].as_str().unwrap_or_default();
    let sound = a["sound"].as_str().filter(|s| !s.is_empty()).map(String::from);
    match tool {
        "create_reminder" if !a["dates"].is_null() || !a["times"].is_null() => make_timed(app, "reminder", a, outside, "assistant", Some(thread)),
        "create_reminder" => {
            if text.trim().is_empty() {
                return Err("text is required".into());
            }
            let sched = if let Some(m) = a["in_minutes"].as_i64() {
                in_minutes_schedule(app, m)
            } else {
                let at = a["at"].as_str().ok_or("give at (2026-11-14 15:00 or 15:00) or in_minutes")?;
                json!({ "at": next_clock_time(app, at)?, "timezone": a["timezone"] })
            };
            let ring = a["ring"].as_bool().unwrap_or(false);
            create_job_with(app, "reminder", text, &sched, outside, "assistant", Some(thread), &JobExtra { ring, sound, ..Default::default() })
        }
        "create_recurring" => {
            if text.trim().is_empty() {
                return Err("text is required".into());
            }
            let a = &{ let mut b = a.clone(); if b["at"].is_null() { b["at"] = b["ats"].clone(); } b };
            let sched = match parse_days(&a["days"])? {
                Some(d) => json!({ "every": "days", "days": d, "at": a["at"], "timezone": a["timezone"] }),
                None => json!({ "every": a["every"], "at": a["at"], "weekday": a["weekday"], "timezone": a["timezone"] }),
            };
            let ring = a["ring"].as_bool().unwrap_or(false);
            create_job_with(app, "recurring", text, &sched, outside, "assistant", Some(thread), &JobExtra { ring, sound, ..Default::default() })
        }
        "schedule_task" | "schedule_recurring_task" => {
            let repeats = a["every_minutes"].is_number() || a["every"].is_string() || !a["days"].is_null();
            if repeats != (tool == "schedule_recurring_task") {
                return Err(if repeats { "that repeats: use schedule_recurring_task".into() } else { "give a repeat (every_minutes, every or days)".to_string() });
            }
            if text.trim().is_empty() {
                return Err("text is required: say what to do when it starts".into());
            }
            let sched = if let Some(m) = a["every_minutes"].as_i64() {
                if m < 15 {
                    return Err("every_minutes must be 15 or more".into());
                }
                json!({ "every_minutes": m, "timezone": a["timezone"], "quiet": a["quiet"].as_bool().filter(|q| *q), "report": a["report"].as_bool().filter(|q| *q) })
            } else if a["every"].is_string() || !a["days"].is_null() {
                match parse_days(&a["days"])? {
                    Some(d) => json!({ "every": "days", "days": d, "at": a["at"], "timezone": a["timezone"] }),
                    None => json!({ "every": a["every"], "at": a["at"], "weekday": a["weekday"], "timezone": a["timezone"] }),
                }
            } else if let Some(m) = a["in_minutes"].as_i64() {
                in_minutes_schedule(app, m)
            } else {
                let at = a["at"].as_str().ok_or("give in_minutes, or at (2026-11-14 15:00 or 15:00), or a repeat")?;
                json!({ "at": next_clock_time(app, at)?, "timezone": a["timezone"] })
            };
            let mut sched = sched;
            apply_model(app, &mut sched, a)?;
            let name = task_name(a, text);
            create_job_with(app, "task", text, &sched, outside, "assistant", Some(thread), &JobExtra { name: Some(name), description: task_desc(a), ..Default::default() })
        }
        "run_task_on_notification" => {
            if outside {
                return Err("A task that runs on notifications cannot be set up from outside content. Ask the user to say it themselves.".into());
            }
            if text.trim().is_empty() {
                return Err("text is required: say what to do each time the notification comes".into());
            }
            let app_id = a["app"].as_str().map(str::trim).filter(|x| !x.is_empty()).ok_or("app is required: the app's package name, e.g. com.google.android.gm for Gmail (see known_apps)")?;
            if !app_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
                return Err("app must be a package name like com.google.android.gm".into());
            }
            let contains = a["contains"].as_str().map(str::trim).filter(|x| !x.is_empty()).map(|x| x.chars().take(80).collect::<String>());
            let mut sched = json!({ "on": "notification", "app": app_id, "contains": contains });
            apply_model(app, &mut sched, a)?;
            let name = task_name(a, text);
            create_job_with(app, "task", text, &sched, false, "assistant", Some(thread), &JobExtra { name: Some(name), description: task_desc(a), ..Default::default() })
        }
        "remind_at_place" | "run_task_at_place" => {
            if outside {
                return Err("This cannot be set up from outside content. Ask the user to say it themselves.".into());
            }
            if text.trim().is_empty() {
                return Err("text is required".into());
            }
            let mut sched = place_schedule(app, a)?;
            if tool == "run_task_at_place" {
                apply_model(app, &mut sched, a)?;
            }
            let (kind, name) = if tool == "remind_at_place" { ("reminder", None) } else { ("task", Some(task_name(a, text))) };
            create_job_with(app, kind, text, &sched, false, "assistant", Some(thread), &JobExtra { name, ..Default::default() })
        }
        "known_apps" => Ok(json!({ "apps": KNOWN_APPS.iter().map(|(p, n)| json!({ "app": p, "name": n })).collect::<Vec<_>>(),
            "note": "Common package names. The phone's Automation page lists every app that has sent a notification." })),
        "pause_task" => set_paused(app, a["id"].as_str().unwrap_or_default(), true),
        "resume_task" => set_paused(app, a["id"].as_str().unwrap_or_default(), false),
        "run_task_now" => run_now(app, a["id"].as_str().unwrap_or_default()).map(|t| json!({ "started": t })),
        "create_alarm" => create_alarm(app, a, outside, "assistant", Some(thread)),
        "create_alarms" => create_alarms(app, a, outside, "assistant", Some(thread)),
        "skip_day" => skip_job_day(app, a["id"].as_str().unwrap_or_default(), a["date"].as_str().unwrap_or_default(), false),
        "unskip_day" => skip_job_day(app, a["result"]["id"].as_str().or(a["id"].as_str()).unwrap_or_default(), a["result"]["date"].as_str().or(a["date"].as_str()).unwrap_or_default(), true),
        "update_job" => update_job(app, a["id"].as_str().unwrap_or_default(), a, outside),
        "restore_job" => restore_job(app, &a["result"]["previous"]),
        "list_jobs" => Ok(json!({ "jobs": list_jobs(app, false), "ringing": ringing(app) })),
        "list_sounds" => Ok(json!({ "sounds": crate::sounds::list(app) })),
        "snooze_alarm" => snooze(app, a["id"].as_str(), a["minutes"].as_i64()),
        "dismiss_alarm" => dismiss(app, a["id"].as_str()),
        "cancel_job" => {
            let id = a["id"].as_str().unwrap_or_default();
            if cancel_job(app, id) { Ok(json!({ "cancelled": id })) } else { Err("no such active job".into()) }
        }
        "cancel_job_undo" => {
            let mut ids: Vec<String> = a["result"]["ids"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            if ids.is_empty() {
                ids.push(a["result"]["id"].as_str().unwrap_or_default().to_string());
            }
            let done = ids.iter().filter(|id| cancel_job(app, id)).count();
            if done > 0 { Ok(json!({ "cancelled": ids, "count": done })) } else { Err("that job already ran or is gone".into()) }
        }
        "uncancel_job" => {
            let id = a["args"]["id"].as_str().unwrap_or_default();
            let n = app.db.lock().execute("UPDATE jobs SET status = 'active' WHERE id = ?1 AND status = 'cancelled' AND next_run > ?2", params![id, util::unix_now()]).unwrap_or(0);
            if n > 0 { Ok(json!({ "restored": id })) } else { Err("that job's time has passed; make a new one".into()) }
        }
        _ => Err("no such tool".into()),
    }
}

/// A new app version was published (its `version.json` changed): tell every connected phone at
/// once (`app.updated`), so it shows the update banner without waiting for its next check.
fn app_update_watch(app: &App) {
    let m = std::fs::metadata(app.cfg.server_dir.join("app").join("version.json")).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
    let prev = app.set_mark("appver", m); // -1 the first time: starting up says nothing
    if prev >= 0 && m != prev && m > 0 {
        app.feed.emit(None, None, "app.updated", json!({}));
    }
}

/// Called every few seconds by the core timer.
pub fn tick(app: &Arc<App>) {
    if !crate::power::is_on(app) {
        return; // AssistX is switched off: nothing fires, nothing thinks
    }
    app.sync_memory_files();
    crate::ambient::maybe_purge(&app.db);
    crate::dreaming::stop_stuck(app);
    crate::ambient::maybe_notice(app);
    crate::dreaming::maybe_auto(app);
    crate::embed::maybe_work(app);
    crate::models::keep_light_warm(app);
    crate::status::maybe_watch(app);
    crate::usage::maybe_sample(app);
    expire_rings(app);
    app_update_watch(app);
    crate::audit::maybe_prune(app);
    crate::location::maybe_suggest(app);
    crate::signals::maybe_fallback(app);
    crate::travel::maybe_check(app);
    crate::detect::maybe_gym(app);
    let now = util::unix_now();
    #[allow(clippy::type_complexity)]
    let due: Vec<(String, String, String, String, String, i64, i64, i64, Option<String>, Option<String>)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, kind, text, schedule, timezone, next_run, outside, ring, sound, event_id FROM jobs WHERE status = 'active' AND next_run <= ?1").expect("prepare");
        st.query_map([now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    for (id, kind, text, sched, tzname, due_at, outside, ring, sound, event_id) in due {
        let schedule: Value = serde_json::from_str(&sched).unwrap_or(Value::Null);
        let late = now - due_at > LATE_AFTER_SECS;
        // Missed slots collapse into one run (docs/00).
        let next = if kind == "event" {
            crate::calendar::next_reminder_run(app, &schedule, now)
        } else if schedule["every"].is_string() || schedule["every_minutes"].is_number() {
            next_run(&schedule, &tzname, now)
        } else {
            None
        };
        let status = if next.is_some() { "active" } else { "done" };
        let _ = app.db.lock().execute("UPDATE jobs SET last_run = ?1, next_run = ?2, status = ?3 WHERE id = ?4", params![now, next, status, id]);
        if kind == "system" && text == "dreaming" {
            let _ = dreaming::start_run(app, false);
            continue;
        }
        crate::audit::event(app, "job.fired", json!({ "job": id, "kind": kind, "text": util::clean_line(&text, 12), "late": late, "due": util::iso(due_at) }));
        let ring = ring != 0 && outside == 0;
        // An alarm that is hours late (the server was off) is a notice, not a ring.
        let stale = now - due_at > 30 * 60;
        if kind == "alarm" {
            if ring && !stale { ring_now(app, &id, &text, sound.as_deref()) } else { notice(app, "urgent", &format!("Missed alarm: {text}")) }
            continue;
        }
        // Reminders and calendar reminders reach the phone at once, without waiting for the model.
        if matches!(kind.as_str(), "reminder" | "event") {
            let text = if kind == "event" { crate::calendar::reminder_text(app, &schedule).unwrap_or(text.clone()) } else { text.clone() };
            app.feed.emit(None, None, "reminder.due", json!({
                "job_id": id, "text": text, "event_id": event_id, "late": late, "outside": outside != 0,
                "ring": ring && !stale, "sound": sound.as_deref().unwrap_or("default"), "sound_url": sound.as_deref().and_then(|s| crate::sounds::url(app, s)),
            }));
            if ring && !stale {
                ring_now(app, &id, &text, sound.as_deref());
            }
            if kind == "event" {
                continue;
            }
        }
        if kind == "task" {
            start_task(app, &id, &text, outside != 0, late, None);
            continue;
        }
        app.ensure_thread(SYSTEM_THREAD, "system", "scheduler");
        app.start_turn(SYSTEM_THREAD, "schedule", json!({ "job": { "id": id, "text": text, "outside": outside != 0, "late": late, "priority": if kind == "reminder" { "urgent" } else { "normal" } } }));
    }
    deliver_held(app);
}

fn local_minutes(app: &App, utc: i64) -> u32 {
    let z = zone(&app.cfg.timezone);
    (tz::civil(utc + z.offset_at(utc)).3 / 60) as u32
}

fn in_quiet(app: &App, utc: i64) -> bool {
    let Some((a, b)) = app.cfg.quiet_hours else { return false };
    let m = local_minutes(app, utc);
    if a <= b { m >= a && m < b } else { m >= a || m < b }
}

fn sent_today(app: &App) -> u32 {
    app.db.lock().query_row("SELECT COUNT(*) FROM notices WHERE delivered = 1 AND silent = 0 AND ts > ?1", [util::unix_now() - 86_400], |r| r.get::<_, i64>(0)).unwrap_or(0) as u32
}

/// An unasked message (docs/05 #7). `urgent` any time (still capped); `normal` outside quiet
/// hours; `digest` waits for the next brief after quiet hours.
pub fn notice(app: &App, priority: &str, text: &str) {
    notice_full(app, &Notice { priority, title: None, text, silent: false, thread: None });
}

/// A notice a reminder made. It carries the job id, so the phone can offer Snooze and Done on it.
pub fn notice_for_job(app: &App, priority: &str, text: &str, job_id: &str) {
    notice(app, priority, text);
    let _ = app.db.lock().execute("UPDATE notices SET job_id = ?1 WHERE n = (SELECT MAX(n) FROM notices)", [job_id]);
}

/// The phone's list of places to watch changed (a place job came, went or was used up).
fn fences_changed(app: &App) {
    app.feed.emit(None, None, "fences.changed", json!({}));
}

/// Snooze a notice that came from a reminder: the same words again after `minutes`.
pub fn snooze_notice(app: &App, n: i64, minutes: i64) -> Result<Value, String> {
    let text: Option<String> = app.db.lock().query_row("SELECT text FROM notices WHERE n = ?1 AND job_id IS NOT NULL", [n], |r| r.get(0)).ok();
    let text = text.ok_or("that notice has nothing to snooze")?;
    let m = minutes.clamp(1, 24 * 60);
    create_job_with(app, "reminder", &text, &in_minutes_schedule(app, m), false, "user", None, &JobExtra::default())
}

/// A notice with all its parts. `silent` ones carry no sound or buzz (the client shows them quietly), skip
/// quiet hours and the daily cap. Returns what happened, in words the model can use.
pub struct Notice<'a> {
    pub priority: &'a str,
    pub title: Option<&'a str>,
    pub text: &'a str,
    pub silent: bool,
    pub thread: Option<&'a str>,
}

pub fn notice_full(app: &App, n: &Notice) -> &'static str {
    let priority = if ["urgent", "normal", "digest"].contains(&n.priority) { n.priority } else { "normal" };
    let now = util::unix_now();
    let capped = !n.silent && sent_today(app) >= app.cfg.notices_per_day;
    let can_send = n.silent || (!capped && match priority {
        "urgent" => true,
        "normal" => !in_quiet(app, now),
        _ => false,
    });
    let text = util::clean_line(n.text, 80);
    let title = n.title.map(|t| util::clean_line(t, 10)).filter(|t| !t.is_empty());
    let _ = app.db.lock().execute(
        "INSERT INTO notices (priority, text, ts, delivered, title, silent, thread_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![priority, text, now, can_send as i64, title, n.silent as i64, n.thread],
    );
    if can_send {
        app.feed.emit(None, None, "assistant.notice", json!({ "priority": priority, "title": title, "text": text, "silent": n.silent, "thread_id": n.thread }));
        "sent"
    } else if capped {
        "not sent: the daily limit of notifications is used up (it is saved and will go out later)"
    } else {
        "held: it is quiet hours, so it goes out when they end"
    }
}

/// After quiet hours: send held normal notices, and digests as one brief.
fn deliver_held(app: &App) {
    let now = util::unix_now();
    if in_quiet(app, now) {
        return;
    }
    let held: Vec<(i64, String, String, Option<String>, Option<String>, Option<String>)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT n, priority, text, title, thread_id, job_id FROM notices WHERE delivered = 0 ORDER BY n LIMIT 50").expect("prepare");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    if held.is_empty() || sent_today(app) >= app.cfg.notices_per_day {
        return;
    }
    // A phone remembers how far it has read (a cursor on `n`). A held notice sits BEHIND that
    // cursor, so flipping its flag would never reach any phone. Instead the held row is replaced
    // by a new, delivered row at the end of the list, and only then is the phone told.
    let digest: Vec<&str> = held.iter().filter(|h| h.1 == "digest").map(|h| h.2.as_str()).collect();
    {
        let c = app.db.lock();
        for (n, p, t, title, th, job) in &held {
            let _ = c.execute("DELETE FROM notices WHERE n = ?1", [n]);
            if p != "digest" {
                let _ = c.execute("INSERT INTO notices (priority, text, ts, delivered, title, silent, thread_id, job_id) VALUES (?1, ?2, ?3, 1, ?4, 0, ?5, ?6)", params![p, t, now, title, th, job]);
            }
        }
        if !digest.is_empty() {
            let _ = c.execute("INSERT INTO notices (priority, text, ts, delivered, title, silent) VALUES ('digest', ?1, ?2, 1, 'Digest', 0)", params![digest.join("\n"), now]);
        }
    }
    for (_, p, t, title, th, _) in held.iter().filter(|h| h.1 != "digest") {
        app.feed.emit(None, None, "assistant.notice", json!({ "priority": p, "title": title, "text": t, "silent": false, "thread_id": th, "held": true }));
    }
    if !digest.is_empty() {
        app.feed.emit(None, None, "assistant.notice", json!({ "priority": "digest", "text": digest.join("\n"), "items": digest.len() }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_job_keeps_local_time_across_dst() {
        // Berlin: 08:00 local is 07:00Z in winter and 06:00Z in summer.
        if Zone::load("Europe/Berlin").is_none() {
            return;
        }
        let s = json!({ "every": "day", "at": "08:00" });
        let jan = tz::local_midnight(2026, 1, 10);
        let jul = tz::local_midnight(2026, 7, 10);
        assert_eq!(next_run(&s, "Europe/Berlin", jan), Some(jan + 7 * 3600));
        assert_eq!(next_run(&s, "Europe/Berlin", jul), Some(jul + 6 * 3600));
    }

    #[test]
    fn weekly_and_once() {
        let now = tz::local_midnight(2026, 11, 11); // a Wednesday
        let w = json!({ "every": "week", "weekday": 5, "at": "10:00" });
        assert_eq!(next_run(&w, "UTC", now), Some(tz::local_midnight(2026, 11, 13) + 36_000));
        assert_eq!(next_run(&json!({ "at": "2026-11-12 09:30" }), "UTC", now), Some(tz::local_midnight(2026, 11, 12) + 9 * 3600 + 1800));
        assert_eq!(next_run(&json!({ "at": "2020-01-01 09:30" }), "UTC", now), None);
    }
}

#[cfg(test)]
mod pacific_tests {
    use super::*;

    #[test]
    fn pacific_clock_times_become_the_right_utc_before_and_after_the_november_change() {
        // 2026-11-16 07:00 in Los Angeles is PST (UTC-8): 15:00 UTC. The change back from daylight time was on 2026-11-01.
        let after = 1_794_787_200; // 2026-11-15 ~17:20 PST
        let day = next_run(&json!({ "every": "day", "at": "07:00" }), "America/Los_Angeles", after).unwrap();
        assert_eq!(util::iso(day), "2026-11-16T15:00:00Z", "a 7am alarm in November is 15:00 UTC");
        let once = next_run(&json!({ "at": "2026-11-16 19:30" }), "America/Vancouver", after).unwrap();
        // Vancouver no longer changes its clock: it stays at UTC-7 all year in the zone data.
        assert_eq!(util::iso(once), "2026-11-17T02:30:00Z");
        // A summer date is one hour different (PDT, UTC-7): the zone data really knows the change.
        let summer = next_run(&json!({ "at": "2027-07-01 07:00" }), "America/Los_Angeles", after).unwrap();
        assert_eq!(util::iso(summer), "2027-07-01T14:00:00Z");
    }
}

#[cfg(test)]
mod days_text_tests {
    use super::*;

    #[test]
    fn days_sent_as_text_are_understood() {
        let want = Some(vec![2, 4, 5]);
        assert_eq!(parse_days(&json!("[2,4,5]")).unwrap(), want);
        assert_eq!(parse_days(&json!("2, 4, 5")).unwrap(), want);
        assert_eq!(parse_days(&json!("Tuesday, Thursday and Friday")).unwrap(), want);
        assert_eq!(parse_days(&json!("tue thu fri")).unwrap(), want);
        assert_eq!(parse_days(&json!([2, 4, 5])).unwrap(), want);
        assert!(parse_days(&json!("sometimes")).is_err());
    }
}
