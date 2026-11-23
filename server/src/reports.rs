//! Chat reports: the user taps "Send log" on a chat and the core writes everything about it to one
//! file, `data/reports/<id>.json`, for a person (or a coding assistant) to review later.
//!
//! A report holds the chat's messages exactly as the model saw them (tool calls, tool results with
//! their `[core] RESULT` lines), every turn with its status and error, the action log, approvals,
//! the event feed (model choices, tool starts, notes) and the model settings. Known secret values
//! are scrubbed from the whole file before it is written. Reports stay on the server.

use crate::{App, util};
use rusqlite::params;
use serde_json::{Value, json};

pub const MAX_KEEP: usize = 200;

fn dir(app: &App) -> std::path::PathBuf {
    app.cfg.data_dir.join("reports")
}

fn rows(app: &App, sql: &str, thread: &str, f: impl Fn(&rusqlite::Row) -> rusqlite::Result<Value>) -> Vec<Value> {
    let c = app.db.lock();
    let Ok(mut st) = c.prepare(sql) else { return vec![] };
    st.query_map(params![thread], |r| f(r)).map(|x| x.flatten().collect()).unwrap_or_default()
}

fn parse(s: Option<String>) -> Value {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

/// Build and save a report for one chat. Returns `{id, created, counts}`.
pub fn create(app: &App, thread: &str, note: &str, by: &str) -> Result<Value, String> {
    save(app, thread, note, by, false)
}

/// The same report, written by the core itself (`auto: true`, src/autoreport.rs).
pub fn create_auto(app: &App, thread: &str, note: &str) -> Result<Value, String> {
    save(app, thread, note, "core", true)
}

fn save(app: &App, thread: &str, note: &str, by: &str, auto: bool) -> Result<Value, String> {
    let t = app.db.lock().query_row("SELECT id, title, kind, created, model FROM threads WHERE id = ?1", [thread], |r| Ok(json!({
        "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?,
        "created": r.get::<_, String>(3)?, "model": r.get::<_, Option<String>>(4)?,
    }))).map_err(|_| "no such chat".to_string())?;
    let messages = rows(app, "SELECT n, id, turn_id, role, trust, body, created, model FROM messages WHERE thread_id = ?1 ORDER BY n", thread, |r| Ok(json!({
        "n": r.get::<_, i64>(0)?, "id": r.get::<_, String>(1)?, "turn_id": r.get::<_, Option<String>>(2)?, "role": r.get::<_, String>(3)?,
        "trust": r.get::<_, String>(4)?, "body": parse(r.get(5)?), "created": r.get::<_, String>(6)?, "model": r.get::<_, Option<String>>(7)?,
    })));
    let turns = rows(app, "SELECT id, source, status, input, created, finished, error_code, error_message, tokens_used FROM turns WHERE thread_id = ?1 ORDER BY created", thread, |r| Ok(json!({
        "id": r.get::<_, String>(0)?, "source": r.get::<_, String>(1)?, "status": r.get::<_, String>(2)?, "input": parse(r.get(3)?),
        "created": r.get::<_, String>(4)?, "finished": r.get::<_, Option<String>>(5)?,
        "error": r.get::<_, Option<String>>(6)?.map(|c| json!({ "code": c, "message": r.get::<_, Option<String>>(7).ok().flatten() })),
        "tokens": r.get::<_, i64>(8)?,
    })));
    let actions = rows(app, "SELECT id, turn_id, plugin, tool, args, result, ok, outcome, decision, approval_id, created FROM actions WHERE thread_id = ?1 ORDER BY n", thread, |r| Ok(json!({
        "id": r.get::<_, String>(0)?, "turn_id": r.get::<_, Option<String>>(1)?, "plugin": r.get::<_, String>(2)?, "tool": r.get::<_, String>(3)?,
        "args": parse(r.get(4)?), "result": parse(r.get(5)?), "ok": r.get::<_, Option<i64>>(6)?.map(|v| v != 0),
        "outcome": r.get::<_, String>(7)?, "decision": r.get::<_, String>(8)?, "approval_id": r.get::<_, Option<String>>(9)?, "created": r.get::<_, String>(10)?,
    })));
    let approvals = rows(app, "SELECT id, turn_id, kind, risk, question, details, status, answer, created FROM approvals WHERE thread_id = ?1 ORDER BY created", thread, |r| Ok(json!({
        "id": r.get::<_, String>(0)?, "turn_id": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?, "risk": r.get::<_, String>(3)?,
        "question": r.get::<_, String>(4)?, "details": parse(r.get(5)?), "status": r.get::<_, String>(6)?, "answer": parse(r.get(7)?),
        "created": util::iso(r.get(8)?),
    })));
    let events: Vec<Value> = app.feed.replay(0, Some(thread), 20_000).into_iter()
        .filter(|e| e.kind != "message.delta") // the whole text is in message.done
        .map(|e| json!({ "seq": e.seq, "type": e.kind, "ts": e.ts, "turn_id": e.turn_id, "data": e.data })).collect();
    let id = util::new_id("rep");
    let created = util::now_iso();
    let counts = json!({ "messages": messages.len(), "turns": turns.len(), "actions": actions.len(), "approvals": approvals.len(), "events": events.len() });
    let report = json!({
        "id": id, "created": created, "by": by, "auto": auto, "note": note.chars().take(2000).collect::<String>(),
        "server": { "version": env!("CARGO_PKG_VERSION"), "routed": app.is_routed(), "models": crate::models::settings(app), "chat_model": crate::models::mode_for(app, thread) },
        "thread": t, "counts": counts, "turns": turns, "messages": messages, "actions": actions, "approvals": approvals, "events": events,
    });
    let text = util::scrub(&serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?, &app.secret_words());
    let d = dir(app);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(format!("{id}.json"));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    prune(app);
    Ok(json!({ "id": id, "created": created, "thread_id": thread, "auto": auto, "counts": counts }))
}

/// Newest first: `{id, created, thread_id, title, note, auto, counts}` for each saved report. `only_auto` keeps the core's own.
pub fn list(app: &App, only_auto: bool) -> Vec<Value> {
    let mut out: Vec<Value> = std::fs::read_dir(dir(app)).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(|r| !only_auto || r["auto"] == true)
        .map(|r| json!({ "id": r["id"], "created": r["created"], "thread_id": r["thread"]["id"], "title": r["thread"]["title"], "note": r["note"], "auto": r["auto"] == true, "counts": r["counts"] }))
        .collect();
    out.sort_by(|a, b| b["created"].as_str().cmp(&a["created"].as_str()));
    out
}

/// One report in full. The id must look like one of ours (no paths).
pub fn get(app: &App, id: &str) -> Option<Value> {
    if !id.starts_with("rep_") || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    std::fs::read_to_string(dir(app).join(format!("{id}.json"))).ok().and_then(|s| serde_json::from_str(&s).ok())
}

/// Keep the newest `MAX_KEEP`.
fn prune(app: &App) {
    let mut files: Vec<_> = std::fs::read_dir(dir(app)).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("rep_"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= MAX_KEEP {
        return;
    }
    files.sort();
    for (_, p) in &files[..files.len() - MAX_KEEP] {
        let _ = std::fs::remove_file(p);
    }
}
