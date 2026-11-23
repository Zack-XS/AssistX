//! The audit trail: what the assistant was told, what it pulled, what it said and every
//! notification it sent or held, so the owner can hand a window of it to an AI agent and ask
//! for better prompts (docs/13).
//!
//! Three pieces: `model_calls` (the exact request and reply of every model step, with the
//! system prompt stored once per distinct text), the existing `messages` (tool arguments and the
//! data tools returned) and `notices` (every notification, held or not, with the owner's
//! feedback). `review` joins them into one markdown pack for an agent.

use crate::{App, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS model_calls (
  n INTEGER PRIMARY KEY AUTOINCREMENT, turn_id TEXT NOT NULL, thread_id TEXT, step INTEGER NOT NULL, ts INTEGER NOT NULL,
  profile TEXT, model TEXT, system_hash TEXT NOT NULL, messages TEXT NOT NULL, tools TEXT NOT NULL,
  reply_text TEXT, reply_calls TEXT, tokens INTEGER NOT NULL DEFAULT 0, ms INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS model_calls_turn ON model_calls (turn_id, step);
CREATE INDEX IF NOT EXISTS model_calls_ts ON model_calls (ts);
CREATE TABLE IF NOT EXISTS audit_events (n INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, kind TEXT NOT NULL, detail TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS audit_events_ts ON audit_events (ts);
CREATE TABLE IF NOT EXISTS system_prompts (hash TEXT PRIMARY KEY, text TEXT NOT NULL, first_seen INTEGER NOT NULL);
"#;

/// How long the full request of a model step is kept. After that the big part (the messages sent)
/// is dropped but the row stays for good: model, time, tokens, tools, the reply, the system prompt.
/// `AGENT_TRACE_KEEP_DAYS=0` keeps the full requests forever.
fn keep_days() -> i64 {
    std::env::var("AGENT_TRACE_KEEP_DAYS").ok().and_then(|v| v.parse().ok()).filter(|d| *d >= 0).unwrap_or(30)
}

/// A thing that happened outside a chat, for the review pack: a job firing, a place crossed,
/// a batch of data from the phone. Detail is a small JSON object.
pub fn event(app: &App, kind: &str, detail: Value) {
    let known = app.secret_words();
    let _ = app.db.lock().execute("INSERT INTO audit_events (ts, kind, detail) VALUES (?1, ?2, ?3)", params![util::unix_now(), kind, util::scrub(&detail.to_string(), &known)]);
}

/// Write down one model step. Never fails the turn.
#[allow(clippy::too_many_arguments)]
pub fn record(app: &App, turn: &str, thread: &str, profile: &str, model: &str, req: &crate::model::Request, reply: &crate::model::Reply, ms: u64) {
    let known = app.secret_words();
    let hash = util::sha256_hex(req.system.as_bytes())[..16].to_string();
    let c = app.db.lock();
    let step: i64 = c.query_row("SELECT COUNT(*) FROM model_calls WHERE turn_id = ?1", [turn], |r| r.get(0)).unwrap_or(0);
    let _ = c.execute("INSERT OR IGNORE INTO system_prompts (hash, text, first_seen) VALUES (?1, ?2, ?3)", params![hash, util::scrub(&req.system, &known), util::unix_now()]);
    let tools: Vec<&str> = req.tools.iter().filter_map(|t| t["function"]["name"].as_str()).collect();
    let calls: Vec<Value> = reply.tool_calls.iter().map(|t| json!({ "name": t.name, "args": t.args })).collect();
    let _ = c.execute(
        "INSERT INTO model_calls (turn_id, thread_id, step, ts, profile, model, system_hash, messages, tools, reply_text, reply_calls, tokens, ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![turn, thread, step, util::unix_now(), profile, model, hash, util::scrub(&Value::Array(req.messages.clone()).to_string(), &known),
            json!(tools).to_string(), reply.text.as_deref().map(|t| util::scrub(t, &known)), util::scrub(&Value::Array(calls).to_string(), &known), reply.tokens as i64, ms as i64],
    );
}

/// Thin old requests (see `keep_days`). Nothing is deleted: rows, replies, notices, messages and events stay.
pub fn prune(app: &App) {
    let days = keep_days();
    if days == 0 {
        return;
    }
    let cut = util::unix_now() - days * 86_400;
    let _ = app.db.lock().execute("UPDATE model_calls SET messages = '[]' WHERE ts < ?1 AND messages != '[]'", [cut]);
}

/// Once an hour, from the core timer.
pub fn maybe_prune(app: &App) {
    if app.due("audit.prune", 3600) {
        prune(app);
    }
}

/// The owner's verdict on a notice: `useful` or `not_useful`, with a note ("I was asleep", "too small").
pub fn feedback(app: &App, n: i64, rating: &str, note: &str) -> Result<Value, String> {
    if !["useful", "not_useful"].contains(&rating) {
        return Err("rating is useful or not_useful".into());
    }
    let changed = app.db.lock().execute("UPDATE notices SET rating = ?1, rating_note = ?2 WHERE n = ?3",
        params![rating, util::clean_line(note, 60), n]).map_err(|e| e.to_string())?;
    if changed == 0 { Err("no such notice".into()) } else { Ok(json!({ "n": n, "rating": rating })) }
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}... [{} more characters]", s.chars().take(max).collect::<String>(), s.chars().count() - max) }
}

/// One turn's full trace as JSON: every model step with its exact request.
pub fn turn_trace(app: &App, turn: &str) -> Option<Value> {
    let c = app.db.lock();
    let t: (String, String, String, String, i64) = c.query_row("SELECT thread_id, source, status, input, tokens_used FROM turns WHERE id = ?1", [turn], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).ok()?;
    let mut st = c.prepare("SELECT step, ts, profile, model, system_hash, messages, tools, reply_text, reply_calls, tokens, ms FROM model_calls WHERE turn_id = ?1 ORDER BY step").ok()?;
    let steps: Vec<Value> = st.query_map([turn], |r| Ok(json!({
        "step": r.get::<_, i64>(0)?, "ts": util::iso(r.get(1)?), "profile": r.get::<_, Option<String>>(2)?, "model": r.get::<_, Option<String>>(3)?,
        "system_prompt": r.get::<_, String>(4)?, "messages": serde_json::from_str::<Value>(&r.get::<_, String>(5)?).unwrap_or(Value::Null),
        "tools": serde_json::from_str::<Value>(&r.get::<_, String>(6)?).unwrap_or(Value::Null), "reply_text": r.get::<_, Option<String>>(7)?,
        "reply_calls": r.get::<_, Option<String>>(8)?.and_then(|s| serde_json::from_str::<Value>(&s).ok()), "tokens": r.get::<_, i64>(9)?, "ms": r.get::<_, i64>(10)? }))).ok()?.flatten().collect();
    let prompts: BTreeMap<String, String> = steps.iter().filter_map(|s| s["system_prompt"].as_str()).filter_map(|h| {
        c.query_row("SELECT text FROM system_prompts WHERE hash = ?1", [h], |r| r.get::<_, String>(0)).optional().ok().flatten().map(|t| (h.to_string(), t))
    }).collect();
    Some(json!({ "turn": turn, "thread": t.0, "source": t.1, "status": t.2, "input": serde_json::from_str::<Value>(&t.3).unwrap_or(Value::Null), "tokens": t.4, "steps": steps, "system_prompts": prompts }))
}

/// The job behind a task thread (`system:task:<job>:<time>`), by name.
fn job_label(app: &App, thread: &str) -> Option<String> {
    let id = thread.strip_prefix("system:task:")?.split(':').next()?;
    let j = crate::scheduler::get_job(app, id)?;
    Some(format!("{} ({id})", j["name"].as_str().unwrap_or("task")))
}

const HEADER: &str = r#"# Review pack

You are reviewing the logs of a personal assistant that runs on its owner's server. It sends phone notifications on its own (reminders, reports, quiet checks every 30 minutes, place and health events). The owner wants it to **stop sending notifications that do not matter, and never miss something that does**.

## What to do

1. Read "Notifications". For each, decide: was it worth interrupting the owner for? Use the owner's ratings first (`not_useful` is a verdict, with a note).
2. Read "Runs that sent nothing". For each, decide: did the data pulled show something the owner should have been told? (A miss.)
3. For every wrong call, name the cause: the trigger prompt, a plugin guide, the shared rules in the system prompt, or the model being too small.
4. Propose **concrete edits**: exact replacement text for the prompt or guide, and which file holds it. Keep edits small. Do not invent data that is not in the log.

## Where the behaviour lives (in the repo)

- `prompts/turn-rules.md`, `prompts/personality.md`: shared rules in every system prompt.
- `plugins/<id>/guide.md`: how each plugin is used (scheduler, calendar, health, location, spotify).
- `src/engine.rs`, the variable `how`: the instruction every scheduled task gets (when to call `notify`).
- A task's own prompt is its `text` (Automation page, or `GET /v1/jobs`). The ready-made reports are in `src/scheduler.rs` (`ensure_reports`).
- `src/scheduler.rs` `notice_full`: quiet hours, the daily cap, and what counts as urgent.

"#;

/// A markdown pack of the last `hours` hours, for an AI agent. `max_turns` caps how many runs are shown in full.
pub fn review(app: &App, hours: i64, max_turns: i64, since_last: bool, mark: bool) -> String {
    let now = util::unix_now();
    let last: Option<i64> = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'audit.last_review'", [], |r| r.get::<_, String>(0)).ok().and_then(|v| v.parse().ok());
    let since = match (since_last, last) {
        (true, Some(l)) => l,
        _ => now - hours.clamp(1, 24 * 365) * 3600,
    };
    let mut out = String::from(HEADER);
    out += &format!("Window: {} to now ({}).{}\n\n", util::iso(since), util::now_iso(), if since_last && last.is_none() { " (No earlier review was marked, so this is the last day.)" } else { "" });
    if mark {
        let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('audit.last_review', ?1)", [now.to_string()]);
    }

    // ---- notices ----
    let notices: Vec<(i64, String, Option<String>, String, bool, bool, Option<String>, Option<String>, i64, Option<String>, Option<String>)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT n, priority, title, text, silent, delivered, thread_id, job_id, ts, rating, rating_note FROM notices WHERE ts > ?1 ORDER BY n").expect("prepare");
        st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0, r.get::<_, i64>(5)? != 0, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?)))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    out += "## Notifications\n\n";
    let (sent, held, silent, bad) = (notices.iter().filter(|n| n.5 && !n.4).count(), notices.iter().filter(|n| !n.5).count(), notices.iter().filter(|n| n.4).count(), notices.iter().filter(|n| n.9.as_deref() == Some("not_useful")).count());
    out += &format!("{} in total: {sent} sent with sound, {silent} silent, {held} held (quiet hours or the daily cap), {bad} rated not useful.\n\n", notices.len());
    if notices.is_empty() {
        out += "None in this window.\n\n";
    }
    for (n, prio, title, text, sil, deliv, thread, job, ts, rating, note) in &notices {
        let from = thread.as_deref().and_then(|t| job_label(app, t)).or_else(|| job.as_ref().map(|j| format!("reminder {j}"))).unwrap_or_else(|| "the core".into());
        out += &format!("- #{n} {} [{prio}{}{}] **{}** {} (from: {from}{}){}\n", util::iso(*ts), if *sil { ", silent" } else { "" }, if *deliv { "" } else { ", HELD" },
            title.as_deref().unwrap_or("-"), text, thread.as_deref().map(|t| format!(", thread {t}")).unwrap_or_default(),
            rating.as_ref().map(|r| format!(" - OWNER SAYS: {r}{}", note.as_deref().filter(|x| !x.is_empty()).map(|x| format!(" ({x})")).unwrap_or_default())).unwrap_or_default());
    }

    // ---- runs ----
    let turns: Vec<(String, String, String, String, String, i64)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, thread_id, source, status, created, tokens_used FROM turns WHERE created > ?1 ORDER BY created DESC LIMIT ?2").expect("prepare");
        st.query_map(params![util::iso(since), max_turns.clamp(1, 400)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let notified_threads: std::collections::HashSet<String> = notices.iter().filter(|n| !n.4 || n.2.as_deref() != Some("Report ready")).filter_map(|n| n.6.clone()).collect();
    let quiet: Vec<_> = turns.iter().filter(|t| t.1.starts_with("system:") && !notified_threads.contains(&t.1)).collect();
    out += "\n## Runs that sent nothing\n\nScheduled and automatic runs that ended without a notification. Check each for a miss.\n\n";
    if quiet.is_empty() {
        out += "None.\n";
    }
    for t in &quiet {
        out += &format!("- {} {} {} ({}), turn {}\n", t.4, t.2, job_label(app, &t.1).unwrap_or_else(|| t.1.clone()), t.3, t.0);
    }

    out += "\n## Every run, newest first\n\nEach shows the prompt it was given, each step's tool calls with the data they returned, and the reply. Full requests: `GET /v1/audit/turns/<turn id>`.\n\n";
    let mut prompts_used: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, thread, source, status, created, tokens) in &turns {
        out += &format!("### {created} · {source} · {} · turn {id} · {status} · {tokens} tokens\n", job_label(app, thread).unwrap_or_else(|| thread.clone()));
        let calls: Vec<(i64, String, String, String, String)> = {
            let c = app.db.lock();
            let mut st = c.prepare("SELECT step, COALESCE(model, ''), system_hash, COALESCE(tools, '[]'), tokens || ' tokens, ' || ms || ' ms' FROM model_calls WHERE turn_id = ?1 ORDER BY step").expect("prepare");
            st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).map(|x| x.flatten().collect()).unwrap_or_default()
        };
        if let Some(first) = calls.first() {
            out += &format!("Model: {} · {} step(s) · system prompt `{}`\n", first.1, calls.len(), first.2);
            for c in &calls {
                prompts_used.entry(c.2.clone()).or_default().push(id.clone());
            }
        } else {
            out += "(no model call was recorded: it ran before the audit trail existed, or never reached the model)\n";
        }
        let msgs: Vec<(String, String, String)> = {
            let c = app.db.lock();
            let mut st = c.prepare("SELECT role, trust, body FROM messages WHERE turn_id = ?1 ORDER BY n").expect("prepare");
            st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map(|x| x.flatten().collect()).unwrap_or_default()
        };
        for (role, trust, body) in msgs {
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            match role.as_str() {
                "user" => out += &format!("\n**Prompt** ({trust}):\n> {}\n", cut(v["content"].as_str().unwrap_or(""), 2500).replace('\n', "\n> ")),
                "assistant" => {
                    for tc in v["tool_calls"].as_array().cloned().unwrap_or_default() {
                        out += &format!("\n**Tool call**: `{}` {}\n", tc["function"]["name"].as_str().unwrap_or("?"), cut(tc["function"]["arguments"].as_str().unwrap_or(""), 600));
                    }
                    if let Some(t) = v["content"].as_str().filter(|t| !t.trim().is_empty()) {
                        out += &format!("\n**Reply**: {}\n", cut(t, 1500));
                    }
                }
                "tool" => out += &format!("\n**Data returned** ({trust}):\n```\n{}\n```\n", cut(v["content"].as_str().unwrap_or(""), 1500)),
                _ => {}
            }
        }
        out += "\n";
    }

    // ---- the mind's journal ----
    let thoughts = crate::mind::journal_read(app, ((now - since) / 86_400 + 1).max(1), 100, None);
    out += "\n## The mind's journal\n\nWhat the assistant wrote down for itself while thinking in the background, newest first.\n\n";
    let entries: Vec<&Value> = thoughts["entries"].as_array().into_iter().flatten().filter(|e| e["at"].as_str().is_some_and(|a| a > util::iso(since).as_str())).collect();
    if entries.is_empty() {
        out += "None.\n";
    }
    for e in entries {
        out += &format!("- {} [{}] {}\n", e["at"].as_str().unwrap_or(""), e["kind"].as_str().unwrap_or(""), e["text"].as_str().unwrap_or(""));
    }

    // ---- other events ----
    let events: Vec<(i64, String, String)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT ts, kind, detail FROM audit_events WHERE ts > ?1 ORDER BY n DESC LIMIT 400").expect("prepare");
        st.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    out += "## Other events\n\nJobs that fired, places crossed and data the phone sent, newest first.\n\n";
    if events.is_empty() {
        out += "None.\n\n";
    }
    for (ts, kind, detail) in events.iter() {
        out += &format!("- {} {kind} {}\n", util::iso(*ts), cut(detail, 400));
    }
    out += "\n";

    // ---- system prompts, once each ----
    out += "## System prompts used\n\nEach distinct system prompt appears once.\n\n";
    for (hash, ids) in &prompts_used {
        let text: Option<String> = app.db.lock().query_row("SELECT text FROM system_prompts WHERE hash = ?1", [hash], |r| r.get(0)).ok();
        out += &format!("### `{hash}` (used in {} model steps)\n```\n{}\n```\n\n", ids.len(), cut(&text.unwrap_or_default(), 12000));
    }
    out
}
