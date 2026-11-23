//! Phone actions (plugin `phone`).
//!
//! The server cannot reach into the phone, so the phone fetches its work: a tool call puts one
//! row in `phone_actions`, emits `phone.action` (the app hears it on the event stream and asks at
//! once), and waits up to `wait_secs()` for the answer. The app takes pending rows with
//! `GET /v1/phone/actions` and reports each with `POST /v1/phone/actions/{id} {ok, error?}`.
//!
//! Every action opens a screen on the phone and nothing more: `call` opens the dialer with the
//! number filled in (the user presses call), `text` opens the messages app with the words filled
//! in (the user presses send). Nothing is ever sent or dialled silently. Contact names are turned
//! into numbers on the phone; the server never sees or keeps the address book.

use crate::{App, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS phone_actions (id TEXT PRIMARY KEY, tool TEXT NOT NULL, args TEXT NOT NULL, status TEXT NOT NULL,
  error TEXT, result TEXT, thread_id TEXT, device TEXT, created INTEGER NOT NULL, updated INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS phone_actions_status ON phone_actions(status, created);
"#;

pub const TOOLS: &[&str] = &["call", "text", "open_app", "set_timer", "set_phone_alarm", "navigate_to"];
/// An action the phone has not picked up in this long is dropped: a timer or a route started
/// minutes late is worse than none.
pub const FRESH_SECS: i64 = 120;
static WAIT_SECS: AtomicU64 = AtomicU64::new(20);

/// How long a tool call waits for the phone's answer (tests make it short).
pub fn set_wait_secs(s: u64) {
    WAIT_SECS.store(s, Ordering::Relaxed);
}

type Waiters = Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>;
fn waiters() -> &'static Waiters {
    static W: OnceLock<Waiters> = OnceLock::new();
    W.get_or_init(Default::default)
}

fn text_arg(a: &Value, key: &str, max: usize) -> Result<String, String> {
    let s = a[key].as_str().map(str::trim).unwrap_or("");
    if s.is_empty() {
        return Err(format!("{key} is missing"));
    }
    if s.chars().count() > max {
        return Err(format!("{key} is too long (at most {max} characters)"));
    }
    if s.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("{key} has control characters"));
    }
    Ok(s.to_string())
}

fn opt_label(a: &Value) -> Result<Option<String>, String> {
    if a["label"].as_str().is_none_or(|s| s.trim().is_empty()) {
        return Ok(None);
    }
    text_arg(a, "label", 60).map(Some)
}

/// Check the model's arguments and keep only what the phone needs.
pub fn check(tool: &str, a: &Value) -> Result<Value, String> {
    match tool {
        "call" => Ok(json!({ "to": text_arg(a, "to", 80)? })),
        "text" => Ok(json!({ "to": text_arg(a, "to", 80)?, "message": text_arg(a, "message", 1000)? })),
        "open_app" => Ok(json!({ "name": text_arg(a, "name", 60)? })),
        "set_timer" => {
            let n = |k: &str| a[k].as_f64().unwrap_or(0.0);
            let secs = (n("seconds") + n("minutes") * 60.0 + n("hours") * 3600.0).round();
            if !(1.0..=86_400.0).contains(&secs) {
                return Err("a timer is 1 second to 24 hours: give seconds, minutes or hours".into());
            }
            Ok(json!({ "seconds": secs as i64, "label": opt_label(a)? }))
        }
        "set_phone_alarm" => {
            let t = a["time"].as_str().unwrap_or("").trim();
            let (h, m) = t.split_once(':').ok_or("time looks like 07:30 (24-hour)")?;
            let (h, m): (u32, u32) = (h.parse().map_err(|_| "time looks like 07:30")?, m.parse().map_err(|_| "time looks like 07:30")?);
            if h > 23 || m > 59 {
                return Err("time looks like 07:30 (24-hour)".into());
            }
            const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
            let mut days = vec![];
            for d in a["days"].as_array().into_iter().flatten() {
                let d = d.as_str().unwrap_or("").trim().to_ascii_lowercase();
                let i = DAYS.iter().position(|x| d.starts_with(x)).ok_or("days are names like mon, tue, wed")?;
                if !days.contains(&(i + 1)) {
                    days.push(i + 1); // Android counts Sunday as 1
                }
            }
            Ok(json!({ "hour": h, "minute": m, "days": days, "label": opt_label(a)? }))
        }
        "navigate_to" => {
            let mode = a["mode"].as_str().unwrap_or("driving").trim().to_ascii_lowercase();
            if !matches!(mode.as_str(), "driving" | "walking" | "bicycling" | "transit") {
                return Err("mode is driving, walking, bicycling or transit".into());
            }
            Ok(json!({ "destination": text_arg(a, "destination", 200)?, "mode": mode }))
        }
        _ => Err(format!("no such tool '{tool}'")),
    }
}

/// Run one phone tool: queue it, tell the phone, wait a little for the answer.
pub async fn call(app: &App, thread: &str, tool: &str, args: &Value) -> Result<Value, String> {
    let a = check(tool, args)?;
    let id = util::new_id("pact");
    let now = util::unix_now();
    let (tx, rx) = oneshot::channel();
    waiters().lock().unwrap().insert(id.clone(), tx);
    app.db.lock().execute(
        "INSERT INTO phone_actions (id, tool, args, status, thread_id, created, updated) VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?5)",
        params![id, tool, a.to_string(), thread, now],
    ).map_err(|e| e.to_string())?;
    let th = Some(thread).filter(|t| !t.is_empty());
    app.feed.emit(th, None, "phone.action", json!({ "id": id, "tool": tool }));
    let secs = WAIT_SECS.load(Ordering::Relaxed);
    match tokio::time::timeout(Duration::from_secs(secs), rx).await {
        Ok(Ok(Ok(r))) => {
            let mut out = json!({ "done": true, "action_id": id, "on_phone": what(tool, &a), "shown": r["shown"] });
            if matches!(tool, "call" | "text") {
                out["note"] = json!(if tool == "call" { "The dialer is open with the number. The user presses call." } else { "The message is written in the messages app. The user presses send." });
                if let Some(c) = r["contact"].as_str() {
                    out["contact"] = json!(c.chars().take(80).collect::<String>());
                }
            }
            Ok(out)
        }
        Ok(Ok(Err(e))) => Err(format!("the phone could not do it: {e}")),
        Ok(Err(_)) => Err("the phone action was dropped".into()),
        Err(_) => {
            waiters().lock().unwrap().remove(&id);
            let status: String = app.db.lock().query_row("SELECT status FROM phone_actions WHERE id = ?1", [&id], |r| r.get(0)).unwrap_or_default();
            let note = if status == "pending" {
                format!("Sent to the phone, but the app has not picked it up yet (closed or offline). It runs if the app checks in within {} minutes; after that it is dropped.", FRESH_SECS / 60)
            } else {
                "Sent to the phone. The phone took it but has not said yet if it worked.".to_string()
            };
            Ok(json!({ "sent_to_phone": true, "done": false, "action_id": id, "status": status, "on_phone": what(tool, &a), "note": note }))
        }
    }
}

fn what(tool: &str, a: &Value) -> String {
    match tool {
        "call" => format!("dialer open for {}", a["to"].as_str().unwrap_or("")),
        "text" => format!("message to {} ready to send", a["to"].as_str().unwrap_or("")),
        "open_app" => format!("open {}", a["name"].as_str().unwrap_or("")),
        "set_timer" => format!("timer for {} seconds", a["seconds"]),
        "set_phone_alarm" => format!("alarm at {:02}:{:02} in the phone's clock app", a["hour"].as_u64().unwrap_or(0), a["minute"].as_u64().unwrap_or(0)),
        "navigate_to" => format!("route to {} ({})", a["destination"].as_str().unwrap_or(""), a["mode"].as_str().unwrap_or("")),
        _ => tool.to_string(),
    }
}

/// The phone asks for work: every fresh pending action, now marked `sent` to this device.
/// Old ones are dropped first.
pub fn take(app: &App, device: &str) -> Vec<Value> {
    let now = util::unix_now();
    let c = app.db.lock();
    let _ = c.execute("UPDATE phone_actions SET status = 'expired', updated = ?1 WHERE status = 'pending' AND created < ?2", params![now, now - FRESH_SECS]);
    let rows: Vec<(String, String, String, i64)> = c
        .prepare("SELECT id, tool, args, created FROM phone_actions WHERE status = 'pending' ORDER BY created LIMIT 10")
        .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map(|x| x.flatten().collect()))
        .unwrap_or_default();
    for r in &rows {
        let _ = c.execute("UPDATE phone_actions SET status = 'sent', device = ?2, updated = ?3 WHERE id = ?1", params![r.0, device, now]);
    }
    rows.into_iter()
        .map(|(id, tool, args, created)| json!({ "id": id, "tool": tool, "args": serde_json::from_str::<Value>(&args).unwrap_or(Value::Null), "created": util::iso(created) }))
        .collect()
}

/// The phone's answer. `Ok(false)` when it was already answered; `Err` when there is no such action.
pub fn answer(app: &App, id: &str, body: &Value) -> Result<bool, String> {
    let ok = body["ok"] == true;
    let error: Option<String> = if ok { None } else { Some(body["error"].as_str().unwrap_or("it did not work").chars().take(300).collect()) };
    // Only what the core expects from the phone is kept.
    let shown = body["shown"].as_str().filter(|s| matches!(*s, "screen" | "notification")).map(String::from);
    let contact = body["contact"].as_str().map(|s| s.chars().take(80).collect::<String>());
    let result = json!({ "shown": shown, "contact": contact });
    let status: Option<String> = app.db.lock().query_row("SELECT status FROM phone_actions WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    match status.as_deref() {
        None => return Err("no such action".into()),
        Some("done" | "failed" | "expired") => return Ok(false),
        _ => {}
    }
    app.db.lock().execute(
        "UPDATE phone_actions SET status = ?2, error = ?3, result = ?4, updated = ?5 WHERE id = ?1",
        params![id, if ok { "done" } else { "failed" }, error, result.to_string(), util::unix_now()],
    ).map_err(|e| e.to_string())?;
    if let Some(tx) = waiters().lock().unwrap().remove(id) {
        let _ = tx.send(if ok { Ok(result) } else { Err(error.unwrap_or_default()) });
    }
    Ok(true)
}

/// A message may carry text shared from another app or read from a photo on the phone, after a
/// marker line. The part before the marker is the user's own words; the rest is outside content.
/// Returns (own words, Some((what, shared text))).
pub fn split_shared(text: &str) -> (String, Option<(&'static str, String)>) {
    for (marker, what) in [("[photo text]", "text read from a photo"), ("[shared text]", "text shared from another app")] {
        if let Some(i) = text.find(marker) {
            let shared = text[i + marker.len()..].trim().to_string();
            return (text[..i].trim().to_string(), Some((what, shared)));
        }
    }
    (text.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_checked() {
        assert_eq!(check("set_timer", &json!({ "minutes": 5 })).unwrap()["seconds"], 300);
        assert!(check("set_timer", &json!({ "hours": 30 })).is_err());
        assert!(check("set_timer", &json!({})).is_err());
        let a = check("set_phone_alarm", &json!({ "time": "7:05", "days": ["Mon", "friday"] })).unwrap();
        assert_eq!((a["hour"].as_u64(), a["minute"].as_u64(), a["days"].clone()), (Some(7), Some(5), json!([2, 6])));
        assert!(check("set_phone_alarm", &json!({ "time": "25:00" })).is_err());
        assert!(check("navigate_to", &json!({ "destination": "Home", "mode": "teleport" })).is_err());
        assert!(check("text", &json!({ "to": "Mia" })).is_err(), "a text needs words");
        assert!(check("call", &json!({ "to": "x".repeat(81) })).is_err());
        assert!(check("call", &json!({ "to": "Mia\u{0}" })).is_err());
        assert!(check("delete_everything", &json!({})).is_err());
    }

    #[test]
    fn shared_text_is_split_off() {
        let (own, sh) = split_shared("what is the total?\n\n[photo text]\nTOTAL 12.50");
        assert_eq!(own, "what is the total?");
        assert_eq!(sh, Some(("text read from a photo", "TOTAL 12.50".to_string())));
        assert_eq!(split_shared("hello").1, None);
        assert_eq!(split_shared("[shared text] https://x.y").0, "");
    }
}
