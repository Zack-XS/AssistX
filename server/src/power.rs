//! The master switch: AssistX on or off (Settings > General). Off means off: no jobs fire, nothing
//! thinks, nothing is dreamed, the phone's uploads are refused (it keeps them and sends them later),
//! and anything that would start work is refused with 503 `power_off`. Reading stays possible, so the
//! app can still show settings, status and history, and turn it back on. The switch is stored in the
//! database and survives a restart.

use crate::{App, scheduler, util};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

pub fn is_on(app: &App) -> bool {
    match app.power_state.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let off = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'power'", [], |r| r.get::<_, String>(0)).map(|v| v == "off").unwrap_or(false);
            app.power_state.store(if off { 2 } else { 1 }, Ordering::Relaxed);
            !off
        }
    }
}

/// Requests that still work while it is off: reading, the switch itself, and managing access.
pub fn allowed_while_off(method: &str, path: &str) -> bool {
    method == "GET" || method == "HEAD" || method == "OPTIONS"
        || ["/v1/power", "/v1/tokens", "/v1/secrets", "/v1/config"].iter().any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

pub fn set(app: &App, on: bool) -> Value {
    let was = is_on(app);
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('power', ?1)", [if on { "on" } else { "off" }]);
    app.power_state.store(if on { 1 } else { 2 }, Ordering::Relaxed);
    if on && !was {
        // Jobs that came due while it was off do not all fire at once: repeating ones move to their next time.
        let now = util::unix_now();
        for j in scheduler::list_jobs(app, true) {
            let (Some(id), true) = (j["id"].as_str(), j["status"] == "active") else { continue };
            if j["next_run_ts"].as_i64().is_some_and(|t| t < now) && (j["schedule"]["every"].is_string() || j["schedule"]["every_minutes"].is_number()) {
                let tz = j["timezone"].as_str().unwrap_or("UTC");
                if let Some(next) = scheduler::next_run(&j["schedule"], tz, now) {
                    let _ = app.db.lock().execute("UPDATE jobs SET next_run = ?1 WHERE id = ?2", rusqlite::params![next, id]);
                }
            }
        }
    }
    if on != was {
        crate::audit::event(app, if on { "power.on" } else { "power.off" }, json!({}));
        app.feed.emit(None, None, "power.changed", json!({ "on": on }));
    }
    json!({ "on": on })
}
