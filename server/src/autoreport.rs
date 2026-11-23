//! A tool that keeps failing writes its own report (docs/13, "Automatic reports").
//!
//! The action log already holds every failed tool call (`actions.ok = 0`). When one plugin tool has
//! failed `THRESHOLD` times in the last 24 hours, or a turn a person was waiting on ends on an error,
//! the core saves the same report a "Send log" tap would (`reports::create_auto`, `auto: true`),
//! with a note naming the tool and its last errors. At most one per tool (or per error code) a day.
//! Secrets are scrubbed by the report writer. Eval runs (threads titled `eval: ...`) never count.
//! `AGENT_AUTO_REPORTS=off` turns it off.

use crate::{App, reports, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

pub const THRESHOLD: usize = 3;
pub const WINDOW_SECS: i64 = 86_400;
/// Threads made by the eval runner (src/eval.rs) start with this title.
pub const EVAL_TITLE: &str = "eval:";

pub fn enabled() -> bool {
    std::env::var("AGENT_AUTO_REPORTS").map(|v| !matches!(v.trim(), "off" | "0" | "false")).unwrap_or(true)
}

/// The rule, on its own so it can be tested: enough failures, and no report for this key in the last day.
pub fn due(failures: usize, last_report: Option<i64>, now: i64) -> bool {
    failures >= THRESHOLD && last_report.is_none_or(|t| now - t >= WINDOW_SECS)
}

/// The note on the report: `auto: <tool> failed N times: <last errors>`.
pub fn note(key: &str, failures: usize, errors: &[String]) -> String {
    let last: Vec<String> = errors.iter().take(3).map(|e| e.chars().take(240).collect()).collect();
    format!("auto: {key} failed {failures} times in 24 h: {}", if last.is_empty() { "no error text".to_string() } else { last.join(" | ") })
}

fn last_report(app: &App, key: &str) -> Option<i64> {
    app.db.lock().query_row("SELECT value FROM settings WHERE key = ?1", [format!("autoreport:{key}")], |r| r.get::<_, String>(0))
        .optional().ok().flatten().and_then(|v| v.parse().ok())
}

fn mark(app: &App, key: &str, now: i64) {
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)", params![format!("autoreport:{key}"), now.to_string()]);
}

pub fn is_eval_thread(app: &App, thread: &str) -> bool {
    app.db.lock().query_row("SELECT title FROM threads WHERE id = ?1", [thread], |r| r.get::<_, String>(0))
        .is_ok_and(|t| t.starts_with(EVAL_TITLE))
}

/// Failed calls of one tool in the last 24 hours, newest first: (thread, error text). Eval threads do not count.
fn recent_failures(app: &App, plugin: &str, tool: &str, now: i64) -> Vec<(String, String)> {
    let since = util::iso(now - WINDOW_SECS);
    let c = app.db.lock();
    let Ok(mut st) = c.prepare(
        "SELECT a.thread_id, a.result FROM actions a LEFT JOIN threads t ON t.id = a.thread_id
         WHERE a.plugin = ?1 AND a.tool = ?2 AND a.ok = 0 AND a.created >= ?3 AND COALESCE(t.title, '') NOT LIKE 'eval:%'
         ORDER BY a.n DESC LIMIT 50") else { return vec![] };
    st.query_map(params![plugin, tool, since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))
        .map(|rows| rows.flatten().map(|(t, res)| {
            let v: Value = res.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            let e = v["error"].as_str().map(String::from).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() });
            (t, e.split_whitespace().collect::<Vec<_>>().join(" "))
        }).collect())
        .unwrap_or_default()
}

/// Called after every failed plugin tool call. Writes a report when the tool crossed the line.
pub fn tool_failed(app: &App, plugin: &str, tool: &str) {
    if !enabled() {
        return;
    }
    let now = util::unix_now();
    let key = format!("{plugin}.{tool}");
    let fails = recent_failures(app, plugin, tool, now);
    if !due(fails.len(), last_report(app, &key), now) {
        return;
    }
    mark(app, &key, now); // first, so two failures at once make one report
    let errors: Vec<String> = fails.iter().map(|f| f.1.clone()).collect();
    let text = note(&key, fails.len(), &errors);
    if let Ok(r) = reports::create_auto(app, &fails[0].0, &text) {
        crate::audit::event(app, "auto_report", json!({ "report": r["id"], "tool": key, "failures": fails.len() }));
    }
}

/// Called when a turn a person was waiting on ends on an error. One report per error code a day.
pub fn turn_error(app: &App, thread: &str, code: &str, message: &str) {
    if !enabled() || is_eval_thread(app, thread) {
        return;
    }
    let now = util::unix_now();
    let key = format!("turn_error.{code}");
    if !due(THRESHOLD, last_report(app, &key), now) {
        return;
    }
    mark(app, &key, now);
    let text = format!("auto: a turn ended on an error ({code}): {}", message.chars().take(400).collect::<String>());
    if let Ok(r) = reports::create_auto(app, thread, &text) {
        crate::audit::event(app, "auto_report", json!({ "report": r["id"], "error": code }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_failures_in_a_day_make_one_report() {
        let now = 1_800_000_000;
        assert!(!due(2, None, now));
        assert!(due(3, None, now));
        assert!(due(7, None, now));
        // One a day per tool.
        assert!(!due(5, Some(now - 3600), now));
        assert!(!due(5, Some(now - WINDOW_SECS + 1), now));
        assert!(due(5, Some(now - WINDOW_SECS), now));
    }

    #[test]
    fn note_names_tool_count_and_last_errors() {
        let n = note("weather.now", 4, &["timeout".into(), "dns failed".into(), "500".into(), "old".into()]);
        assert!(n.starts_with("auto: weather.now failed 4 times"));
        assert!(n.contains("timeout | dns failed | 500"));
        assert!(!n.contains("old"));
        assert!(note("x.y", 3, &[]).contains("no error text"));
    }
}
