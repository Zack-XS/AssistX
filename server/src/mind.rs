//! The mind: the assistant thinks on its own, often, without being asked (docs/14).
//!
//! A ready-made quiet task, "Mind", runs every 30 minutes. The core gives each run a digest of what
//! changed since the last one (places, calendar, health, chats, notices, jobs, memory) and its own
//! journal, so it can piece things together across runs instead of starting blank. It records
//! thoughts with `journal_add`, reads them back with `journal_read`, and uses its normal tools to act
//! (notes, reminders, ideas). It speaks only through `notify`, and every guard stays: tools that need
//! a yes are not run unattended, outside content stays outside, the notification cap holds.
//! `AGENT_MIND=off` turns it off.

use crate::{App, util};
use rusqlite::params;
use serde_json::{Value, json};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS mind_journal (n INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, kind TEXT NOT NULL, text TEXT NOT NULL, turn_id TEXT);
CREATE INDEX IF NOT EXISTS mind_journal_ts ON mind_journal (ts);
"#;

pub const KINDS: &[&str] = &["observation", "connection", "question", "plan", "decision"];

pub fn enabled() -> bool {
    std::env::var("AGENT_MIND").map(|v| v != "off").unwrap_or(true)
}

/// The prompt of the Mind task (a task's text is at most 300 characters).
const TEXT: &str = "Mind pass. Read the digest. Piece things together: what changed, what links up (calendar, places, health, chats, notes), what is unresolved. Record thoughts with journal_add. Act with your tools when clearly useful and safe (notes, reminders). Notify only if it matters. Otherwise stay silent.";

/// Made once (`mind.seeded`). It is an ordinary task: change its model, its pace or cancel it in Automation.
pub fn ensure(app: &App) {
    if !enabled() {
        return;
    }
    let seeded: bool = app.db.lock().query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key = 'mind.seeded')", [], |r| r.get(0)).unwrap_or(true);
    if seeded {
        return;
    }
    let sched = json!({ "every_minutes": 30, "quiet": true, "mind": true, "model": "local" });
    let x = crate::scheduler::JobExtra { name: Some("Mind".into()), description: Some("Thinks across your day and connects the dots".into()), ..Default::default() };
    let _ = crate::scheduler::create_job_with(app, "task", TEXT, &sched, false, "core", None, &x);
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('mind.seeded', '1')", []);
}

pub fn journal_add(app: &App, turn: &str, kind: &str, text: &str) -> Result<Value, String> {
    let kind = if KINDS.contains(&kind) { kind } else { "observation" };
    let text = text.trim();
    if text.is_empty() {
        return Err("text is required".into());
    }
    let known = app.secret_words();
    let n = {
        let c = app.db.lock();
        c.execute("INSERT INTO mind_journal (ts, kind, text, turn_id) VALUES (?1, ?2, ?3, ?4)", params![util::unix_now(), kind, util::scrub(&text.chars().take(1200).collect::<String>(), &known), turn]).map_err(|e| e.to_string())?;
        c.last_insert_rowid()
    };
    Ok(json!({ "saved": n, "kind": kind }))
}

/// The journal, newest first. `days` back (default 3), at most `limit`; `kind` narrows it.
pub fn journal_read(app: &App, days: i64, limit: i64, kind: Option<&str>) -> Value {
    let since = util::unix_now() - days.clamp(1, 90) * 86_400;
    let c = app.db.lock();
    let mut st = c.prepare("SELECT n, ts, kind, text FROM mind_journal WHERE ts > ?1 AND (?2 IS NULL OR kind = ?2) ORDER BY n DESC LIMIT ?3").expect("prepare");
    let items: Vec<Value> = st.query_map(params![since, kind, limit.clamp(1, 100)], |r| Ok(json!({ "n": r.get::<_, i64>(0)?, "at": util::iso(r.get(1)?), "kind": r.get::<_, String>(2)?, "text": r.get::<_, String>(3)? })))
        .map(|x| x.flatten().collect()).unwrap_or_default();
    json!({ "entries": items })
}

fn cut(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n { format!("{t}...") } else { t }
}

/// What changed since `since`, as plain text for the model. Compact on purpose: the model can dig with its tools.
pub fn digest(app: &App, since: i64) -> String {
    let now = util::unix_now();
    let mut out = format!("# Digest for the mind\n\nNow: {} ({}). Covers the last {} minutes.\n", crate::scheduler::local_str(app, now), app.cfg.timezone, ((now - since) / 60).max(1));
    // Where
    let loc = crate::location::now(app);
    if loc["known"] == true {
        out += &format!("\n## Where\nNow: {} (as of {} min ago{}).\n", loc["place"]["name"].as_str().unwrap_or("an unnamed spot"), loc["minutes_ago"], loc["here_since"].as_str().map(|s| format!(", here since {s}")).unwrap_or_default());
    } else {
        out += "\n## Where\nNo location yet.\n";
    }
    let visits: Vec<String> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT COALESCE(p.user_name, p.auto_name, 'an unnamed place'), v.arrived, v.left_at FROM visits v LEFT JOIN places p ON p.id = v.place_id WHERE v.left_at > ?1 ORDER BY v.arrived DESC LIMIT 8").expect("prepare");
        st.query_map([since], |r| Ok(format!("- {} {}-{}", r.get::<_, String>(0)?, crate::scheduler::local_str(app, r.get(1)?), crate::scheduler::local_str(app, r.get(2)?)))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    if !visits.is_empty() {
        out += &format!("Visits:\n{}\n", visits.join("\n"));
    }
    // Calendar
    if app.plugin_usable("calendar") && let Some(cal) = crate::calendar::upcoming_line(app) {
        out += &format!("\n## Calendar, next 24 hours\n{cal}\n");
    }
    // Health
    if let Ok(days) = crate::health::days(app, None, None, 2) && !days.is_empty() {
        out += &format!("\n## Health (latest days)\n{}\n", cut(&Value::Array(days).to_string(), 700));
    }
    // Parcels, bills and plans the phone noticed (outside content)
    if let Some(d) = crate::detect::digest_section(app, since) {
        out += &d;
    }
    // What the assistant itself did and said
    let notices: Vec<String> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT priority, COALESCE(title, ''), text, delivered, COALESCE(rating, '') FROM notices WHERE ts > ?1 ORDER BY n DESC LIMIT 8").expect("prepare");
        st.query_map([since], |r| Ok(format!("- [{}{}] {} {}{}", r.get::<_, String>(0)?, if r.get::<_, i64>(3)? == 0 { ", held" } else { "" }, r.get::<_, String>(1)?, cut(&r.get::<_, String>(2)?, 120), { let g: String = r.get(4)?; if g.is_empty() { String::new() } else { format!(" (owner: {g})") } })))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    if !notices.is_empty() {
        out += &format!("\n## Notifications sent since\n{}\n", notices.join("\n"));
    }
    // What the user said to it
    let said: Vec<String> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT body FROM messages WHERE role = 'user' AND trust = 'user' AND created > ?1 ORDER BY n DESC LIMIT 6").expect("prepare");
        st.query_map([util::iso(since)], |r| r.get::<_, String>(0)).map(|x| x.flatten().filter_map(|b| serde_json::from_str::<Value>(&b).ok()).filter_map(|v| v["content"].as_str().map(|t| format!("- {}", cut(t, 160)))).collect()).unwrap_or_default()
    };
    if !said.is_empty() {
        out += &format!("\n## What the user said to the assistant\n{}\n", said.join("\n"));
    }
    // Coming up and things that fired
    let jobs: Vec<Value> = crate::scheduler::list_jobs(app, false).into_iter().filter(|j| j["status"] == "active" && j["next_run"].is_string() && j["name"] != "Mind").take(6).collect();
    if !jobs.is_empty() {
        out += "\n## Coming up\n";
        for j in jobs {
            out += &format!("- {} at {}\n", j["name"].as_str().filter(|n| !n.is_empty()).unwrap_or_else(|| j["text"].as_str().unwrap_or("")), j["next_run"].as_str().unwrap_or(""));
        }
    }
    let notes: i64 = app.db.lock().query_row("SELECT COUNT(*) FROM inbox", [], |r| r.get(0)).unwrap_or(0);
    out += &format!("\n## Memory\n{notes} unsorted notes waiting for dreaming.\n");
    // Its own earlier thoughts
    let j = journal_read(app, 3, 10, None);
    let entries = j["entries"].as_array().cloned().unwrap_or_default();
    out += "\n## Your journal (newest first)\n";
    if entries.is_empty() {
        out += "Empty. This is the first pass.\n";
    }
    for e in entries {
        out += &format!("- {} [{}] {}\n", e["at"].as_str().unwrap_or(""), e["kind"].as_str().unwrap_or(""), cut(e["text"].as_str().unwrap_or(""), 300));
    }
    cut(&out, 7000)
}

/// The digest for a Mind run, from where the last one ended.
pub fn digest_for_run(app: &App) -> String {
    let now = util::unix_now();
    let last: Option<i64> = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'mind.last'", [], |r| r.get::<_, String>(0)).ok().and_then(|v| v.parse().ok());
    let since = last.unwrap_or(now - 3600).max(now - 24 * 3600);
    let d = digest(app, since);
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('mind.last', ?1)", [now.to_string()]);
    d
}

/// Standing instructions for a Mind run.
pub const GUIDE: &str = "You are the assistant's own mind, running in the background with nobody waiting. Think like a good chief of staff: notice what changed, connect it to what you already know (calendar, places, health, what the user said, your journal), and ask what is unresolved or about to matter. Use journal_add to keep your thoughts (observation, connection, question, plan, decision) so your next pass starts where this one ended; keep entries short and specific, and do not repeat what the journal already says. Use journal_read or search_memory when you need more. You may act when it is clearly useful and safe: save a note, set a reminder, adjust something you made. Never spend money, send messages, delete, or guess; if an action needs a yes, journal it as a plan and tell the user only if it matters. Use notify rarely, one or two short lines, and only for something NEW that is time-sensitive or that the user would be glad to hear right now. Good: 'Your 3pm starts in 20 minutes and you are 15 minutes away', 'Resting heart rate has been 25% above your week for three days'. Bad, never send: 'Activity reviewed', 'Heart rate hit 169 six days ago', 'No events found', anything already in the notifications sent or your journal. Do not save memory notes about routine observations; the journal is for those. Most passes should end with journal entries and no notification. End with one short line saying what you concluded.";

/// What every unattended task is told about working and about notifying. Written from the review of real runs: the
/// check that reloaded the same plugins every time, saved the same "Activity Snapshot" twice per run, and sent a
/// notification about a heart rate six days old. Three kinds: a report (must notify its key points), a quiet task
/// and the mind (stay quiet unless something new and time-sensitive), and any other task.
pub fn task_rules(sched: &Value) -> String {
    const WORK: &str = "Nobody is waiting, so do not ask questions. Work it out with your tools (search_memory, load plugins such as calendar, files or scheduler). Load each plugin once; its tools stay available. End with one short reply saying what you concluded.";
    let rules = if sched["brief"] == true {
        crate::signals::RULES
    } else if sched["report"] == true {
        "This is a report. Use only today's data: nothing older than a day unless it is still open. Finish by calling notify once, with a short title and two or three short lines of key points, not the whole report, and silent: true. If there is truly nothing to report, one line saying so is enough. Do not save a note or memory entry about the report itself."
    } else if sched["quiet"] == true || sched["mind"] == true {
        "Stay quiet by default. Notify only for something NEW, from the last day, that is time-sensitive or that the user would plainly want to hear right now: an event or reminder about to be missed, a clash, a message from someone close with no reminder, a reading far outside their usual. Never notify to say that a check ran, that nothing was found, what the data looks like in general, or about anything older than a day or already told (see the notifications already sent in the digest, and your journal). Before notifying, ask: would they thank me for this buzz? If not, do not send it. Do not save notes or memory entries about routine checks: a note is for a lasting fact the user would want later, and only if it is not already known."
    } else {
        "If something is important for the user to know or do soon (a message from someone close with no reminder yet, a deadline, a clash), call notify, and set a reminder with the scheduler if one is missing. Use silent for routine news and urgent only when it cannot wait. If nothing is worth saying, do not notify; never notify to say that you looked and found nothing."
    };
    format!("{WORK} {rules}")
}

#[cfg(test)]
mod rules_tests {
    use super::*;

    #[test]
    fn each_kind_of_task_gets_its_own_notify_rules() {
        let report = task_rules(&json!({ "every": "day", "report": true }));
        assert!(report.contains("calling notify once") && report.contains("only today's data"), "{report}");
        for quiet in [json!({ "quiet": true }), json!({ "mind": true })] {
            let r = task_rules(&quiet);
            assert!(r.contains("Stay quiet by default") && r.contains("older than a day") && r.contains("Do not save notes"), "{r}");
            assert!(!r.contains("always call notify"), "a quiet task is never told to always notify");
        }
        let plain = task_rules(&Value::Null);
        assert!(plain.contains("never notify to say that you looked and found nothing") && !plain.contains("Stay quiet by default"));
        assert!(plain.starts_with("Nobody is waiting") && plain.contains("Load each plugin once"));
    }
}
