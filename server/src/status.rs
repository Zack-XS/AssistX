//! Server health watch: are the local models, the voice and the sandbox working?
//! `GET /v1/status` (owner) runs the checks now. Every 5 minutes the core runs them in the
//! background and sends one silent notice when something goes down, and one when it is back.

use crate::App;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static BUSY: AtomicBool = AtomicBool::new(false);
const EVERY_SECS: i64 = 300;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(4))).http_status_as_error(false).build().into()
}

/// Model names a local OpenAI-compatible server offers (Ollama: GET /v1/models). `None` = no answer.
fn offered(base: &str) -> Option<Vec<String>> {
    let mut r = agent().get(&format!("{}/models", base.trim_end_matches('/'))).call().ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    let v: Value = serde_json::from_str(&r.body_mut().read_to_string().ok()?).ok()?;
    Some(v["data"].as_array()?.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
}

/// One plain line per part: ok, and what is wrong if not.
pub fn check(app: &App) -> Value {
    let mut parts = vec![];
    let mut by_base: std::collections::HashMap<String, Option<Vec<String>>> = Default::default();
    for p in crate::models::builtins(app).into_iter().filter(|p| p.local) {
        let list = by_base.entry(p.base_url.clone()).or_insert_with(|| offered(&p.base_url)).clone();
        let (ok, why) = match list {
            None => (false, "the model server does not answer".to_string()),
            Some(l) if l.iter().any(|m| m == &p.model || m.split(':').next() == Some(p.model.as_str())) => (true, String::new()),
            Some(_) => (false, format!("model '{}' is not installed", p.model)),
        };
        parts.push(json!({ "part": format!("model:{}", p.id), "label": p.label, "ok": ok, "problem": why }));
    }
    if let Some(base) = crate::api::tts_base_of(&app.cfg.tts_url) {
        let ok = agent().get(&format!("{base}/health")).call().map(|r| r.status().as_u16() == 200).unwrap_or(false);
        parts.push(json!({ "part": "voice", "label": "Kokoro voice", "ok": ok, "problem": if ok { "" } else { "the voice service does not answer" } }));
    }
    let sb = app.cfg.sandbox.as_str();
    let sb_ok = sb != "unavailable";
    parts.push(json!({ "part": "sandbox", "label": format!("tool sandbox ({sb})"), "ok": sb_ok, "problem": if sb_ok { "" } else { "no sandbox: tools are refused" } }));
    let ok = parts.iter().all(|p| p["ok"] == true);
    json!({ "ok": ok, "checked": crate::util::now_iso(), "parts": parts })
}

/// The numbers behind the Status screen: power, version, use today, when each part last did something.
pub fn overview(app: &App) -> Value {
    use crate::util::{iso, unix_now};
    let now = unix_now();
    let day = now / 86_400 * 86_400;
    let c = app.db.lock();
    let one = |sql: &str, p: &[&dyn rusqlite::ToSql]| c.query_row(sql, p, |r| r.get::<_, Option<i64>>(0)).ok().flatten();
    let tokens = one("SELECT SUM(tokens_used) FROM turns WHERE created >= ?1", &[&iso(day)]).unwrap_or(0);
    let cloud = one("SELECT SUM(cloud_tokens) FROM turns WHERE created >= ?1", &[&iso(day)]).unwrap_or(0);
    let last_loc = one("SELECT MAX(ts) FROM location_points", &[]);
    let last_mind = one("SELECT last_run FROM jobs WHERE name = 'Mind' AND status = 'active' LIMIT 1", &[]);
    let notices_today = one("SELECT COUNT(*) FROM notices WHERE delivered = 1 AND silent = 0 AND ts > ?1", &[&(now - 86_400)]).unwrap_or(0);
    let jobs = one("SELECT COUNT(*) FROM jobs WHERE status = 'active' AND kind != 'system'", &[]).unwrap_or(0);
    let last_health: Option<String> = c.query_row("SELECT MAX(day) FROM health_days", [], |r| r.get(0)).ok().flatten();
    let last_turn = one("SELECT strftime('%s', MAX(created)) FROM turns", &[]);
    let db = std::fs::metadata(app.cfg.data_dir.join("core.db")).map(|m| m.len()).unwrap_or(0);
    let plain = app.cfg.data_dir.join("core.db.plain-backup").exists();
    drop(c);
    json!({
        "power": crate::power::is_on(app), "version": env!("CARGO_PKG_VERSION"), "now": iso(now), "timezone": app.cfg.timezone,
        "tokens_today": tokens, "cloud_tokens_today": cloud, "cloud_cap": app.cfg.limits.max_tokens_per_day,
        "last_location": last_loc.map(|t| json!({ "at": iso(t), "minutes_ago": (now - t) / 60 })),
        "last_mind_run": last_mind.map(|t| json!({ "at": iso(t), "minutes_ago": (now - t) / 60 })),
        "last_chat_turn": last_turn.map(|t| json!({ "at": iso(t), "minutes_ago": (now - t) / 60 })),
        "last_health_day": last_health, "notices_today": notices_today, "notice_cap": app.cfg.notices_per_day,
        "active_jobs": jobs, "database_mb": (db as f64 / 1_048_576.0 * 10.0).round() / 10.0, "database_encrypted": true,
        "plain_backup_present": plain,
        "sandbox": app.cfg.sandbox,
    })
}

/// From the core timer: at most every 5 minutes, in the background, never blocking the timer.
pub fn maybe_watch(app: &std::sync::Arc<App>) {
    let now = crate::util::unix_now();
    let _ = now;
    if !app.due("status.watch", EVERY_SECS) || BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let r = check(&app);
        let down: BTreeSet<String> = r["parts"].as_array().into_iter().flatten()
            .filter(|p| p["ok"] == false).map(|p| format!("{}: {}", p["label"].as_str().unwrap_or(""), p["problem"].as_str().unwrap_or(""))).collect();
        let before = app.down.lock().unwrap().replace(down.clone());
        if let Some(before) = before {
            for d in down.difference(&before) {
                crate::scheduler::notice_full(&app, &crate::scheduler::Notice { priority: "normal", title: Some("Something is down"), text: d, silent: true, thread: None });
            }
            for d in before.difference(&down) {
                let part = d.split(": ").next().unwrap_or(d);
                crate::scheduler::notice_full(&app, &crate::scheduler::Notice { priority: "normal", title: Some("Working again"), text: &format!("{part} works again."), silent: true, thread: None });
            }
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}
