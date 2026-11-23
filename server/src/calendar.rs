//! The assistant's own calendar (plugin `calendar`, docs/03).
//!
//! Events live in the database, in local wall time plus a zone, so daylight saving never moves
//! them. An event can repeat (day, week, weekdays, month, year, until a date) and can carry
//! reminders: each one is a scheduler job of kind `event` that follows the event. Change or delete
//! the event and its reminders move or go with it. A reminder may ring with a chosen sound.

use crate::scheduler::{self, JobExtra};
use crate::{App, tz, util};
use rusqlite::params;
use serde_json::{Value, json};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS calendar_events (
  id TEXT PRIMARY KEY, title TEXT NOT NULL, start TEXT NOT NULL, end TEXT, all_day INTEGER NOT NULL DEFAULT 0,
  timezone TEXT NOT NULL, location TEXT, notes TEXT, repeat TEXT NOT NULL DEFAULT 'none', until TEXT,
  remind TEXT NOT NULL DEFAULT '[]', outside INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'active',
  created TEXT NOT NULL, updated TEXT NOT NULL, skip TEXT NOT NULL DEFAULT '[]'
);
"#;

const REPEATS: &[&str] = &["none", "day", "week", "weekdays", "month", "year"];
/// Upcoming events shown in the index.
const INDEX_HOURS: i64 = 24;

#[derive(Clone, Debug)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start: String,
    pub end: Option<String>,
    pub all_day: bool,
    pub timezone: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub repeat: String,
    pub until: Option<String>,
    pub remind: Value,
    pub outside: bool,
    pub status: String,
    /// Days (`YYYY-MM-DD`) a repeating event does not happen on. The other days are untouched.
    pub skip: Vec<String>,
}

impl Event {
    fn json(&self) -> Value {
        json!({ "id": self.id, "title": self.title, "start": self.start, "end": self.end, "all_day": self.all_day,
            "timezone": self.timezone, "location": self.location, "notes": self.notes, "repeat": self.repeat,
            "until": self.until, "remind": self.remind, "outside": self.outside, "status": self.status, "skip": self.skip })
    }
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Event> {
    Ok(Event {
        id: r.get(0)?, title: r.get(1)?, start: r.get(2)?, end: r.get(3)?, all_day: r.get::<_, i64>(4)? != 0, timezone: r.get(5)?,
        location: r.get(6)?, notes: r.get(7)?, repeat: r.get(8)?, until: r.get(9)?,
        remind: serde_json::from_str(&r.get::<_, String>(10)?).unwrap_or(json!([])), outside: r.get::<_, i64>(11)? != 0, status: r.get(12)?,
        skip: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
    })
}

const COLS: &str = "id, title, start, end, all_day, timezone, location, notes, repeat, until, remind, outside, status, skip";

pub fn get(app: &App, id: &str) -> Option<Event> {
    app.db.lock().query_row(&format!("SELECT {COLS} FROM calendar_events WHERE id = ?1"), [id], row).ok()
}

fn active(app: &App) -> Vec<Event> {
    let c = app.db.lock();
    let mut st = c.prepare(&format!("SELECT {COLS} FROM calendar_events WHERE status = 'active'")).expect("prepare");
    st.query_map([], row).map(|x| x.flatten().collect()).unwrap_or_default()
}

// ---------------- time ----------------

/// Local seconds (as if UTC) for `YYYY-MM-DD HH:MM`, or midnight for a bare date.
fn local_secs(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() == 10 { scheduler::parse_local(&format!("{s} 00:00")) } else { scheduler::parse_local(s) }
}

fn fmt_local(l: i64, all_day: bool) -> String {
    let (y, m, d, s) = tz::civil(l);
    if all_day { format!("{y:04}-{m:02}-{d:02}") } else { format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", s / 3600, s % 3600 / 60) }
}

fn add_months(l: i64, months: i64) -> Option<i64> {
    let (y, m, d, s) = tz::civil(l);
    let total = y * 12 + (m as i64 - 1) + months;
    let (ny, nm) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    let mid = tz::local_midnight(ny, nm, d);
    // The 31st does not exist in every month: skip that one.
    (tz::civil(mid).2 == d).then_some(mid + s)
}

/// Local start times of an event's occurrences that begin in [from, to) (local seconds).
fn occurrences_local(ev: &Event, from: i64, to: i64, max: usize) -> Vec<i64> {
    let Some(first) = local_secs(&ev.start) else { return vec![] };
    let until = ev.until.as_deref().and_then(local_secs).map(|u| u + 86_399).unwrap_or(i64::MAX);
    let to = to.min(until.saturating_add(1));
    let mut out = vec![];
    let push = |l: i64, out: &mut Vec<i64>| -> bool {
        if l >= to || out.len() >= max { return false; }
        if l >= from { out.push(l); }
        true
    };
    match ev.repeat.as_str() {
        "day" | "week" | "weekdays" => {
            let step = if ev.repeat == "week" { 7 * 86_400 } else { 86_400 };
            let mut k = ((from - first) / step - 1).max(0);
            loop {
                let l = first + k * step;
                k += 1;
                if ev.repeat == "weekdays" && matches!(((l.div_euclid(86_400)) + 4).rem_euclid(7), 0 | 6) {
                    if l >= to { break; }
                    continue;
                }
                if !push(l, &mut out) { break; }
            }
        }
        "month" | "year" => {
            let step = if ev.repeat == "year" { 12 } else { 1 };
            for k in 0..2400 {
                let Some(l) = add_months(first, k * step) else { continue };
                if !push(l, &mut out) { break; }
            }
        }
        _ => { push(first, &mut out); }
    }
    out
}

fn length_secs(ev: &Event) -> i64 {
    match (local_secs(&ev.start), ev.end.as_deref().and_then(local_secs)) {
        (Some(a), Some(b)) if b > a => b - a,
        _ if ev.all_day => 86_400,
        _ => 3600,
    }
}

fn is_skipped(ev: &Event, local: i64) -> bool {
    !ev.skip.is_empty() && ev.skip.contains(&fmt_local(local, true))
}

/// Occurrences as JSON between two UTC instants. Skipped days are left out, or kept and marked when `with_skipped`
/// (the calendar page shows them dimmed, with a way back).
fn occurrences(ev: &Event, from_utc: i64, to_utc: i64, max: usize, with_skipped: bool) -> Vec<Value> {
    let z = scheduler::zone(&ev.timezone);
    let len = length_secs(ev);
    // Widen by a day for the zone offset, then filter exactly in UTC.
    occurrences_local(ev, from_utc - 86_400 - len, to_utc + 86_400, max + 4).into_iter()
        .map(|l| (l, z.local_to_utc(l)))
        .filter(|(l, u)| *u + len > from_utc && *u < to_utc && (with_skipped || !is_skipped(ev, *l)))
        .take(max)
        .map(|(l, _)| json!({ "event_id": ev.id, "title": ev.title, "start": fmt_local(l, ev.all_day), "end": fmt_local(l + len, ev.all_day && len % 86_400 == 0),
            "all_day": ev.all_day, "location": ev.location, "repeat": ev.repeat, "outside": ev.outside, "skipped": is_skipped(ev, l) }))
        .collect()
}

fn next_start_utc(ev: &Event, after_utc: i64) -> Option<i64> {
    let z = scheduler::zone(&ev.timezone);
    let from = after_utc + z.offset_at(after_utc) - 86_400;
    occurrences_local(ev, from, i64::MAX / 4, 40).into_iter().filter(|l| !is_skipped(ev, *l)).map(|l| z.local_to_utc(l)).find(|u| *u > after_utc)
}

// ---------------- reminders ----------------

/// `remind` may be `[10, 60]` or `[{"minutes": 10, "ring": true, "sound": "bell"}]`.
fn parse_remind(app: &App, v: &Value) -> Result<Value, String> {
    let list = match v {
        Value::Null => return Ok(json!([])),
        Value::Number(_) | Value::Object(_) => vec![v.clone()],
        Value::Array(a) => a.clone(),
        _ => return Err("remind is a list of minutes before, e.g. [10] or [{\"minutes\": 10, \"ring\": true}]".into()),
    };
    let mut out = vec![];
    for r in list.iter().take(5) {
        let (minutes, ring, sound) = match r {
            Value::Number(n) => (n.as_i64(), false, None),
            Value::Object(_) => (r["minutes"].as_i64(), r["ring"].as_bool().unwrap_or(false), r["sound"].as_str().filter(|s| !s.is_empty()).map(String::from)),
            _ => (None, false, None),
        };
        let minutes = minutes.filter(|m| (0..=60 * 24 * 14).contains(m)).ok_or("each reminder is 0 to 20160 minutes before")?;
        if let Some(s) = &sound {
            crate::sounds::check(app, s)?;
        }
        out.push(json!({ "minutes": minutes, "ring": ring, "sound": sound }));
    }
    Ok(Value::Array(out))
}

/// When an event reminder runs next (after `now`), or None when the event is gone or over.
pub fn next_reminder_run(app: &App, schedule: &Value, now: i64) -> Option<i64> {
    let ev = get(app, schedule["event"].as_str()?).filter(|e| e.status == "active")?;
    let before = schedule["before"].as_i64().unwrap_or(0) * 60;
    next_start_utc(&ev, now + before).map(|s| s - before)
}

/// Words for a due event reminder: `Dentist in 30 min (15:00, Main St)`.
pub fn reminder_text(app: &App, schedule: &Value) -> Option<String> {
    let ev = get(app, schedule["event"].as_str()?)?;
    let before = schedule["before"].as_i64().unwrap_or(0);
    let now = util::unix_now();
    let start = next_start_utc(&ev, now - 120)?;
    let when = if ev.all_day { "today".to_string() } else { scheduler::local_str(app, start)[11..].to_string() };
    let soon = match before { 0 => "now".to_string(), m if m < 60 => format!("in {m} min"), m if m % 60 == 0 && m < 1440 => format!("in {} h", m / 60), m => format!("in {} days", (m + 720) / 1440) };
    let place = ev.location.as_deref().map(|l| format!(", {l}")).unwrap_or_default();
    Some(format!("{} {soon} ({when}{place})", ev.title))
}

/// Replace an event's reminder jobs with fresh ones from its `remind` list.
fn sync_jobs(app: &App, ev: &Event) -> Vec<Value> {
    let _ = app.db.lock().execute("UPDATE jobs SET status = 'cancelled' WHERE event_id = ?1 AND status = 'active'", [&ev.id]);
    if ev.status != "active" {
        return vec![];
    }
    let mut made = vec![];
    for r in ev.remind.as_array().into_iter().flatten() {
        let sched = json!({ "event": ev.id, "before": r["minutes"], "timezone": ev.timezone });
        let Some(next) = next_reminder_run(app, &sched, util::unix_now()) else { continue };
        let x = JobExtra { ring: r["ring"] == true, sound: r["sound"].as_str().map(String::from), event_id: Some(ev.id.clone()), next_run: Some(next), name: None, description: None };
        if let Ok(j) = scheduler::create_job_with(app, "event", &ev.title, &sched, ev.outside, "calendar", None, &x) {
            made.push(j);
        }
    }
    made
}

// ---------------- writes ----------------

fn check_time(s: &str, all_day: bool) -> Result<String, String> {
    let s = s.trim().replace('T', " ");
    let s = if all_day && s.len() > 10 { s[..10].to_string() } else { s };
    local_secs(&s).ok_or_else(|| format!("'{s}' is not a time; use 2026-11-14 15:00 (or 2026-11-14 for all day)"))?;
    Ok(if s.len() == 10 || all_day { s } else { fmt_local(local_secs(&s).unwrap(), false) })
}

fn text_field(a: &Value, key: &str, max: usize) -> Option<String> {
    a[key].as_str().map(|s| s.trim().chars().take(max).collect::<String>()).filter(|s| !s.is_empty())
}

fn save(app: &App, ev: &Event) -> Result<(), String> {
    app.db.lock().execute(
        "INSERT INTO calendar_events (id, title, start, end, all_day, timezone, location, notes, repeat, until, remind, outside, status, created, updated)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)
         ON CONFLICT(id) DO UPDATE SET title=?2, start=?3, end=?4, all_day=?5, timezone=?6, location=?7, notes=?8, repeat=?9, until=?10, remind=?11, outside=?12, status=?13, updated=?14",
        params![ev.id, ev.title, ev.start, ev.end, ev.all_day as i64, ev.timezone, ev.location, ev.notes, ev.repeat, ev.until, ev.remind.to_string(), ev.outside as i64, ev.status, util::now_iso()],
    ).map(|_| ()).map_err(|e| e.to_string())
}

/// Fill or change an event from tool or API fields. `base` is the event being changed.
fn apply(app: &App, base: Option<&Event>, a: &Value, outside: bool) -> Result<Event, String> {
    let all_day = a["all_day"].as_bool().unwrap_or(base.is_some_and(|b| b.all_day) || a["start"].as_str().is_some_and(|s| s.trim().len() == 10));
    let title = text_field(a, "title", 120).or(base.map(|b| b.title.clone())).ok_or("title is required")?;
    let start = match a["start"].as_str() { Some(s) => check_time(s, all_day)?, None => base.map(|b| b.start.clone()).ok_or("start is required, e.g. 2026-11-14 15:00")? };
    let end = match &a["end"] {
        Value::String(s) if !s.trim().is_empty() => Some(check_time(s, all_day)?),
        Value::Null if a.get("end").is_none() => base.and_then(|b| b.end.clone()),
        _ => None,
    };
    if let (Some(s), Some(e)) = (local_secs(&start), end.as_deref().and_then(local_secs))
        && e < s
    {
        return Err("end is before start".into());
    }
    let repeat = a["repeat"].as_str().map(|r| r.trim().to_ascii_lowercase()).or(base.map(|b| b.repeat.clone())).unwrap_or_else(|| "none".into());
    let repeat = if repeat.is_empty() || repeat == "once" { "none".into() } else { repeat };
    if !REPEATS.contains(&repeat.as_str()) {
        return Err(format!("repeat is one of {}", REPEATS.join(", ")));
    }
    let until = match a["until"].as_str() { Some(u) if !u.trim().is_empty() => Some(check_time(u, true)?), Some(_) => None, None => base.and_then(|b| b.until.clone()) };
    let remind = if a.get("remind").is_some() { parse_remind(app, &a["remind"])? } else { base.map(|b| b.remind.clone()).unwrap_or(json!([])) };
    let tzname = a["timezone"].as_str().map(String::from).or(base.map(|b| b.timezone.clone())).unwrap_or_else(|| app.cfg.timezone.clone());
    if tz::Zone::load(&tzname).is_none() {
        return Err(format!("unknown timezone '{tzname}'"));
    }
    let keep = |k: &str, max: usize, old: Option<&Option<String>>| -> Option<String> {
        if a.get(k).is_some() { text_field(a, k, max) } else { old.cloned().flatten() }
    };
    Ok(Event {
        id: base.map(|b| b.id.clone()).unwrap_or_else(|| util::new_id("evt")),
        title, start, end, all_day, timezone: tzname,
        location: keep("location", 200, base.map(|b| &b.location)),
        notes: keep("notes", 2000, base.map(|b| &b.notes)),
        repeat, until, remind,
        outside: outside || base.is_some_and(|b| b.outside),
        status: "active".into(),
        skip: base.map(|b| b.skip.clone()).unwrap_or_default(), // changing an event keeps the days already skipped (the database keeps them too)
    })
}

pub fn add(app: &App, a: &Value, outside: bool) -> Result<Value, String> {
    let ev = apply(app, None, a, outside)?;
    save(app, &ev)?;
    let jobs = sync_jobs(app, &ev);
    app.force_index();
    Ok(json!({ "event": ev.json(), "reminders": jobs, "next": next_start_utc(&ev, util::unix_now() - 1).map(|u| scheduler::local_str(app, u)) }))
}

pub fn update(app: &App, id: &str, a: &Value, outside: bool) -> Result<Value, String> {
    let old = get(app, id).filter(|e| e.status == "active").ok_or("no such event")?;
    let ev = apply(app, Some(&old), a, outside)?;
    save(app, &ev)?;
    let jobs = sync_jobs(app, &ev);
    app.force_index();
    Ok(json!({ "event": ev.json(), "reminders": jobs, "previous": old.json() }))
}

pub fn delete(app: &App, id: &str) -> Result<Value, String> {
    let mut ev = get(app, id).filter(|e| e.status == "active").ok_or("no such event")?;
    let before = ev.json();
    ev.status = "deleted".into();
    save(app, &ev)?;
    sync_jobs(app, &ev);
    app.force_index();
    Ok(json!({ "deleted": id, "previous": before }))
}

/// Undo of update and delete: put the earlier event back, with its reminders.
fn restore(app: &App, prev: &Value) -> Result<Value, String> {
    let id = prev["id"].as_str().ok_or("nothing to restore")?;
    let mut ev = get(app, id).ok_or("no such event")?;
    ev = apply(app, Some(&ev), prev, prev["outside"] == true)?;
    save(app, &ev)?;
    sync_jobs(app, &ev);
    app.force_index();
    Ok(json!({ "restored": id }))
}

fn undo_add(app: &App, result: &Value) -> Result<Value, String> {
    delete(app, result["event"]["id"].as_str().ok_or("nothing to undo")?)
}

// ---------------- schedule import (src/schedule_import.rs) ----------------

/// Check an event that came from outside this module without saving it. Gives the event as it would be stored
/// and its first time from now.
pub fn check(app: &App, a: &Value) -> Result<Value, String> {
    let ev = apply(app, None, a, false)?;
    let first = next_start_utc(&ev, util::unix_now() - 1).map(|u| scheduler::local_str(app, u));
    Ok(json!({ "event": ev.json(), "first": first }))
}

/// Is there already an active event with this title, start and repeat?
pub fn exists_like(app: &App, ev: &Value) -> bool {
    let (t, s, r) = (ev["title"].as_str().unwrap_or("").trim().to_lowercase(), ev["start"].as_str().unwrap_or(""), ev["repeat"].as_str().unwrap_or("none"));
    active(app).iter().any(|e| e.title.trim().to_lowercase() == t && e.start == s && e.repeat == r)
}

/// The title of an existing event that overlaps this one on its first day (all-day events never clash), if any.
pub fn clash(app: &App, ev: &Value, first: &str) -> Option<String> {
    if ev["all_day"] == true || first.len() < 16 {
        return None;
    }
    let day = &first[..10];
    let len = match (local_secs(ev["start"].as_str()?), ev["end"].as_str().and_then(local_secs)) { (Some(a), Some(b)) if b > a => b - a, _ => 3600 };
    let (s0, e0) = (local_secs(first)?, local_secs(first)? + len);
    let listed = list(app, Some(day), Some(day), None).ok()?;
    listed["events"].as_array()?.iter().filter(|e| e["all_day"] != true).find_map(|e| {
        let (s, f) = (local_secs(e["start"].as_str()?)?, local_secs(e["end"].as_str().unwrap_or(e["start"].as_str()?))?);
        let f = if f > s { f } else { s + 3600 };
        (s < e0 && f > s0).then(|| e["title"].as_str().unwrap_or("something").to_string())
    })
}

/// Occurrences between two local times (default: now to 7 days), soonest first.
pub fn list(app: &App, from: Option<&str>, to: Option<&str>, query: Option<&str>) -> Result<Value, String> {
    list_with(app, from, to, query, false)
}

/// Skip one day of a repeating event, or bring it back (`undo`). The other days, and the reminders for them, stay.
pub fn skip_day(app: &App, id: &str, date: &str, undo: bool) -> Result<Value, String> {
    let date = date.trim();
    if date.len() != 10 || local_secs(date).is_none() {
        return Err("date looks like 2026-11-20".into());
    }
    let ev = get(app, id).filter(|e| e.status == "active").ok_or("no such event")?;
    if ev.repeat == "none" {
        return Err("this event does not repeat; delete it instead".into());
    }
    let mut skip = ev.skip.clone();
    skip.retain(|d| d != date);
    if !undo {
        skip.push(date.to_string());
        skip.sort();
        skip.dedup();
        let keep_from = fmt_local(util::unix_now() - 400 * 86_400, true);
        skip.retain(|d| *d >= keep_from);
    }
    app.db.lock().execute("UPDATE calendar_events SET skip = ?1, updated = ?2 WHERE id = ?3", params![json!(skip).to_string(), util::now_iso(), id]).map_err(|e| e.to_string())?;
    if let Some(fresh) = get(app, id) {
        sync_jobs(app, &fresh); // the reminder for the next time follows
    }
    Ok(json!({ "event_id": id, "date": date, "skipped": !undo, "skip": skip }))
}

pub fn list_with(app: &App, from: Option<&str>, to: Option<&str>, query: Option<&str>, with_skipped: bool) -> Result<Value, String> {
    let z = scheduler::zone(&app.cfg.timezone);
    let now = util::unix_now();
    let parse = |s: Option<&str>| -> Result<Option<i64>, String> {
        s.map(|s| local_secs(&s.replace('T', " ")).map(|l| z.local_to_utc(l)).ok_or_else(|| format!("'{s}' is not a date"))).transpose()
    };
    let from_utc = parse(from)?.unwrap_or(now);
    let to_utc = match (parse(to)?, to.map(|t| t.trim().len() == 10)) {
        (Some(t), Some(true)) => t + 86_400, // a bare end date includes that whole day
        (Some(t), _) => t,
        (None, _) => from_utc + 7 * 86_400,
    };
    if to_utc - from_utc > 400 * 86_400 {
        return Err("ask for at most about a year at a time".into());
    }
    let q = query.map(|q| q.to_lowercase());
    let mut items: Vec<Value> = active(app).iter()
        .filter(|e| q.as_ref().is_none_or(|q| e.title.to_lowercase().contains(q) || e.location.as_deref().unwrap_or("").to_lowercase().contains(q) || e.notes.as_deref().unwrap_or("").to_lowercase().contains(q)))
        .flat_map(|e| occurrences(e, from_utc, to_utc, 200, with_skipped))
        .collect();
    items.sort_by(|a, b| a["start"].as_str().cmp(&b["start"].as_str()));
    items.truncate(200);
    Ok(json!({ "from": scheduler::local_str(app, from_utc), "to": scheduler::local_str(app, to_utc), "timezone": app.cfg.timezone, "events": items }))
}

/// The next day's events for the index, wrapped as outside content (titles may come from mail).
pub fn upcoming_line(app: &App) -> Option<String> {
    let now = util::unix_now();
    let list = list(app, None, None, None).ok()?;
    let soon: Vec<String> = list["events"].as_array()?.iter()
        .filter(|e| e["start"].as_str().and_then(local_secs).map(|l| scheduler::zone(&app.cfg.timezone).local_to_utc(l) < now + INDEX_HOURS * 3600).unwrap_or(false))
        .take(6)
        .map(|e| format!("- {} {}{}", e["start"].as_str().unwrap_or(""), e["title"].as_str().unwrap_or(""), e["location"].as_str().map(|l| format!(" ({l})")).unwrap_or_default()))
        .collect();
    if soon.is_empty() {
        return None;
    }
    Some(util::wrap_outside("calendar:upcoming", &format!("Now: {}\n{}", scheduler::local_str(app, now), soon.join("\n"))))
}

/// `calendar` plugin tools.
pub fn tool(app: &App, outside: bool, tool: &str, a: &Value) -> Result<Value, String> {
    let id = a["id"].as_str().or(a["event_id"].as_str()).unwrap_or_default();
    match tool {
        "list_events" => list(app, a["from"].as_str(), a["to"].as_str(), a["query"].as_str()),
        "find_events" => {
            let q = a["query"].as_str().filter(|q| !q.trim().is_empty()).ok_or("query is required")?;
            let now = util::unix_now();
            let from = a["from"].as_str().map(String::from).unwrap_or_else(|| scheduler::local_str(app, now - 90 * 86_400));
            let to = a["to"].as_str().map(String::from).unwrap_or_else(|| scheduler::local_str(app, now + 300 * 86_400));
            list(app, Some(&from), Some(&to), Some(q))
        }
        "get_event" => get(app, id).filter(|e| e.status == "active").map(|e| e.json()).ok_or_else(|| "no such event".into()),
        "add_event" => add(app, a, outside),
        "update_event" => update(app, id, a, outside),
        "delete_event" => delete(app, id),
        "skip_occurrence" => skip_day(app, id, a["date"].as_str().unwrap_or_default(), false),
        "unskip_occurrence" => skip_day(app, a["result"]["event_id"].as_str().unwrap_or(id), a["result"]["date"].as_str().or(a["date"].as_str()).unwrap_or_default(), true),
        "undo_add_event" => undo_add(app, &a["result"]),
        "restore_event" => restore(app, &a["result"]["previous"]),
        _ => Err("no such tool".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(start: &str, repeat: &str) -> Event {
        Event { id: "e".into(), title: "T".into(), start: start.into(), end: None, all_day: false, timezone: "UTC".into(), location: None,
            notes: None, repeat: repeat.into(), until: None, remind: json!([]), outside: false, status: "active".into(), skip: vec![] }
    }

    #[test]
    fn repeats_land_on_the_right_days() {
        let day = |s: &str| local_secs(s).unwrap();
        let week = occurrences_local(&ev("2026-11-02 09:00", "weekdays"), day("2026-11-06 00:00"), day("2026-11-11 00:00"), 10);
        let shown: Vec<String> = week.iter().map(|l| fmt_local(*l, false)).collect();
        assert_eq!(shown, ["2026-11-06 09:00", "2026-11-09 09:00", "2026-11-10 09:00"], "Friday, then Monday and Tuesday; no weekend");
        let monthly = occurrences_local(&ev("2026-01-31 10:00", "month"), day("2026-01-01 00:00"), day("2026-06-01 00:00"), 10);
        let shown: Vec<String> = monthly.iter().map(|l| fmt_local(*l, true)).collect();
        assert_eq!(shown, ["2026-01-31", "2026-03-31", "2026-05-31"], "months without a 31st are skipped");
        let mut until = ev("2026-11-01 08:00", "day");
        until.until = Some("2026-11-03".into());
        assert_eq!(occurrences_local(&until, day("2026-10-01 00:00"), day("2027-01-01 00:00"), 50).len(), 3);
        assert_eq!(occurrences_local(&ev("2026-11-01 08:00", "none"), day("2026-11-02 00:00"), day("2027-01-01 00:00"), 5).len(), 0);
    }
}
