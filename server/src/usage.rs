//! Usage, cost, power and encryption report for a status panel.
//! `GET /v1/usage?days=30` and `GET /v1/security` (owner). Every model call is written to
//! `model_usage`; every minute the core samples the server's power into `power_samples`.
//! Money is an estimate: tokens x a blended price per million tokens (`AGENT_PRICE_<PROFILE>_PER_MTOK`),
//! power x the price of a kWh (`AGENT_POWER_PRICE_KWH`).

use crate::App;
use crate::db::Db;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static BUSY: AtomicBool = AtomicBool::new(false);
/// CPU counters (busy, total) and RAPL energy (microjoules, time) from the previous sample.
static PREV_CPU: Mutex<Option<(u64, u64)>> = Mutex::new(None);
static PREV_RAPL: Mutex<Option<(u64, i64)>> = Mutex::new(None);
const SAMPLE_EVERY_SECS: i64 = 60;
const KEEP_DAYS: i64 = 400;

pub fn migrate(db: &Db) {
    let _ = db.lock().execute_batch(
        "CREATE TABLE IF NOT EXISTS model_usage (
           id INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, day TEXT NOT NULL, turn_id TEXT,
           profile TEXT NOT NULL, model TEXT NOT NULL, local INTEGER NOT NULL, tokens INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS model_usage_day ON model_usage(day);
         CREATE TABLE IF NOT EXISTS power_samples (
           ts INTEGER PRIMARY KEY, watts REAL NOT NULL, cpu REAL, gpu_watts REAL, source TEXT NOT NULL
         );",
    );
}

fn day_of(ts: i64) -> String {
    crate::util::iso(ts).chars().take(10).collect()
}

/// One model call: who answered and how many tokens it used.
pub fn record(app: &App, turn_id: &str, profile: &str, model: &str, local: bool, tokens: u64) {
    let ts = crate::util::unix_now();
    let _ = app.db.lock().execute(
        "INSERT INTO model_usage (ts, day, turn_id, profile, model, local, tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![ts, day_of(ts), turn_id, profile, model, local as i64, tokens as i64],
    );
}

fn envf(k: &str, d: f64) -> f64 {
    std::env::var(k).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(d)
}

/// Blended price per million tokens. Rough defaults; set the real ones in the environment.
fn price_per_mtok(profile: &str, local: bool) -> f64 {
    if local {
        return 0.0;
    }
    let key = format!("AGENT_PRICE_{}_PER_MTOK", profile.to_uppercase().replace('-', "_"));
    let default = match profile { "ovh" => 0.3, _ => 1.0 };
    envf(&key, default)
}

struct Assume {
    idle_w: f64,
    max_w: f64,
    kwh_price: f64,
    currency: String,
}

fn assume() -> Assume {
    Assume {
        idle_w: envf("AGENT_POWER_IDLE_W", 40.0),
        max_w: envf("AGENT_POWER_MAX_W", 120.0),
        kwh_price: envf("AGENT_POWER_PRICE_KWH", 0.30),
        currency: std::env::var("AGENT_CURRENCY").unwrap_or_else(|_| "$".into()),
    }
}

// ---------------- power sampling ----------------

fn cpu_busy_fraction() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let line = s.lines().next()?;
    let n: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if n.len() < 5 {
        return None;
    }
    let total: u64 = n.iter().take(8).sum();
    let idle = n[3] + n.get(4).copied().unwrap_or(0);
    let busy = total.saturating_sub(idle);
    let prev = PREV_CPU.lock().unwrap().replace((busy, total));
    let (pb, pt) = prev?;
    let dt = total.saturating_sub(pt);
    if dt == 0 { return None; }
    Some((busy.saturating_sub(pb) as f64 / dt as f64).clamp(0.0, 1.0))
}

/// CPU package watts from Intel/AMD RAPL, when the file is readable. Needs two samples.
fn rapl_watts(now: i64) -> Option<f64> {
    let uj: u64 = std::fs::read_to_string("/sys/class/powercap/intel-rapl:0/energy_uj").ok()?.trim().parse().ok()?;
    let prev = PREV_RAPL.lock().unwrap().replace((uj, now));
    let (pu, pt) = prev?;
    if now <= pt || uj < pu { return None; }
    Some((uj - pu) as f64 / 1e6 / (now - pt) as f64)
}

/// Graphics card draw in watts (NVIDIA), when `nvidia-smi` exists.
fn gpu_watts() -> Option<f64> {
    let o = std::process::Command::new("nvidia-smi").args(["--query-gpu=power.draw", "--format=csv,noheader,nounits"]).output().ok()?;
    if !o.status.success() { return None; }
    let v: Vec<f64> = String::from_utf8_lossy(&o.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect();
    if v.is_empty() { None } else { Some(v.iter().sum()) }
}

/// From the core timer: one power sample a minute, in the background.
pub fn maybe_sample(app: &std::sync::Arc<App>) {
    let now = crate::util::unix_now();
    let _ = now;
    if !app.due("usage.sample", SAMPLE_EVERY_SECS) || BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let a = assume();
        let cpu = cpu_busy_fraction();
        let gpu = gpu_watts();
        let rapl = rapl_watts(now);
        // base_w covers what RAPL does not see: board, fans, disks, memory.
        let (cpu_w, source) = match (rapl, cpu) {
            (Some(w), _) => (Some(w + a.idle_w * 0.5), "rapl"),
            (None, Some(c)) => (Some(a.idle_w + (a.max_w - a.idle_w) * c), "estimate"),
            _ => (None, "none"),
        };
        if let Some(w) = cpu_w {
            let total = w + gpu.unwrap_or(0.0);
            let source = if gpu.is_some() { format!("{source}+gpu") } else { source.to_string() };
            let _ = app.db.lock().execute(
                "INSERT OR REPLACE INTO power_samples (ts, watts, cpu, gpu_watts, source) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![now, total, cpu, gpu, source],
            );
            let _ = app.db.lock().execute("DELETE FROM power_samples WHERE ts < ?1", [now - KEEP_DAYS * 86400]);
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

// ---------------- report ----------------

pub fn report(app: &App, days: i64) -> Value {
    let days = days.clamp(1, 366);
    let now = crate::util::unix_now();
    let since = now - days * 86400;
    let a = assume();
    let db = app.db.lock();

    let mut models = vec![];
    let mut cloud_cost = 0.0;
    if let Ok(mut st) = db.prepare("SELECT profile, model, local, COUNT(*), SUM(tokens) FROM model_usage WHERE ts >= ?1 GROUP BY profile, model, local ORDER BY SUM(tokens) DESC") {
        let rows = st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)));
        for (profile, model, local, calls, tokens) in rows.into_iter().flatten().flatten() {
            let price = price_per_mtok(&profile, local);
            let cost = tokens as f64 / 1e6 * price;
            cloud_cost += cost;
            models.push(json!({ "profile": profile, "model": model, "local": local, "calls": calls, "tokens": tokens, "price_per_mtok": price, "cost": cost }));
        }
    }

    // Per day: calls, tokens, cloud cost (needs the model per row), power.
    let mut per_day: std::collections::BTreeMap<String, (i64, i64, f64, f64, f64)> = Default::default();
    if let Ok(mut st) = db.prepare("SELECT day, profile, local, COUNT(*), SUM(tokens) FROM model_usage WHERE ts >= ?1 GROUP BY day, profile, local") {
        let rows = st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)));
        for (day, profile, local, calls, tokens) in rows.into_iter().flatten().flatten() {
            let e = per_day.entry(day).or_default();
            e.0 += calls;
            e.1 += tokens;
            e.2 += tokens as f64 / 1e6 * price_per_mtok(&profile, local);
        }
    }
    // Power: each sample stands for the time to the next one (at most 5 minutes).
    let mut samples: Vec<(i64, f64)> = vec![];
    let mut source = "none".to_string();
    if let Ok(mut st) = db.prepare("SELECT ts, watts, source FROM power_samples WHERE ts >= ?1 ORDER BY ts") {
        let rows = st.query_map([since], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?)));
        for (ts, w, s) in rows.into_iter().flatten().flatten() {
            samples.push((ts, w));
            source = s;
        }
    }
    let mut total_kwh = 0.0;
    for (i, (ts, w)) in samples.iter().enumerate() {
        let span = samples.get(i + 1).map(|n| n.0 - ts).unwrap_or(SAMPLE_EVERY_SECS).clamp(0, 300);
        let kwh = w * span as f64 / 3600.0 / 1000.0;
        total_kwh += kwh;
        per_day.entry(day_of(*ts)).or_default().3 += kwh;
    }
    let power_cost = total_kwh * a.kwh_price;
    let now_w = samples.last().filter(|s| now - s.0 < 300).map(|s| s.1);
    let avg_w = if samples.is_empty() { None } else { Some(samples.iter().map(|s| s.1).sum::<f64>() / samples.len() as f64) };
    // The last 24 hours, one point per hour.
    let mut hourly: std::collections::BTreeMap<i64, (f64, u32)> = Default::default();
    for (ts, w) in samples.iter().filter(|s| s.0 >= now - 86400) {
        let e = hourly.entry(ts / 3600 * 3600).or_default();
        e.0 += w;
        e.1 += 1;
    }
    let hourly: Vec<Value> = hourly.into_iter().map(|(h, (sum, n))| json!({ "ts": h, "watts": sum / n as f64 })).collect();

    let mut day_rows: Vec<Value> = per_day.iter().map(|(d, (calls, tokens, cost, kwh, _))| {
        json!({ "day": d, "calls": calls, "tokens": tokens, "cloud_cost": cost, "kwh": kwh, "power_cost": kwh * a.kwh_price })
    }).collect();
    let mut busiest = day_rows.clone();
    busiest.sort_by(|x, y| y["tokens"].as_i64().cmp(&x["tokens"].as_i64()));
    busiest.truncate(5);
    day_rows.sort_by(|x, y| x["day"].as_str().cmp(&y["day"].as_str()));
    // Busiest weekday by calls (0 = Monday).
    let mut weekday = [0i64; 7];
    for (d, v) in &per_day {
        if let Some(w) = weekday_of(d) { weekday[w] += v.0; }
    }

    json!({
        "days": days,
        "currency": a.currency,
        "models": models,
        "per_day": day_rows,
        "busiest_days": busiest,
        "weekday_calls": weekday,
        "totals": { "cloud_cost": cloud_cost, "power_kwh": total_kwh, "power_cost": power_cost, "all": cloud_cost + power_cost },
        "power": { "now_watts": now_w, "avg_watts": avg_w, "source": source, "samples": samples.len(), "hourly": hourly },
        "assumptions": { "kwh_price": a.kwh_price, "idle_watts": a.idle_w, "max_watts": a.max_w, "note": "Money is an estimate: tokens x a blended price per million, power x your kWh price. Set AGENT_PRICE_<PROFILE>_PER_MTOK, AGENT_POWER_PRICE_KWH, AGENT_POWER_IDLE_W, AGENT_POWER_MAX_W. Power is measured (rapl, gpu) when the machine allows, else estimated from CPU load." },
    })
}

/// Monday = 0, from `YYYY-MM-DD` (Sakamoto's method).
fn weekday_of(day: &str) -> Option<usize> {
    let p: Vec<i64> = day.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 3 { return None; }
    let (mut y, m, d) = (p[0], p[1], p[2]);
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 { y -= 1; }
    let sun0 = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d).rem_euclid(7);
    Some(((sun0 + 6) % 7) as usize)
}

// ---------------- encryption report ----------------

#[cfg(unix)]
fn mode_of(p: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).ok().map(|m| m.permissions().mode() & 0o777)
}
#[cfg(not(unix))]
fn mode_of(_: &Path) -> Option<u32> { None }

fn run_line(cmd: &str, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new(cmd).args(args).output().ok()?;
    if !o.status.success() { return None; }
    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Is the disk holding `data/` encrypted? `Some(true/false)`, or `None` = could not tell.
fn disk_encrypted(dir: &Path) -> (Option<bool>, String) {
    if cfg!(target_os = "macos") {
        return match run_line("fdesetup", &["status"]) {
            Some(s) => (Some(s.contains("FileVault is On")), s.lines().next().unwrap_or("").to_string()),
            None => (None, "could not ask FileVault".into()),
        };
    }
    let d = dir.to_string_lossy().to_string();
    let Some(src) = run_line("findmnt", &["-no", "SOURCE", "-T", &d]) else { return (None, "could not find the disk".into()) };
    match run_line("lsblk", &["-s", "-no", "TYPE", &src]) {
        Some(t) => { let enc = t.lines().any(|l| l.trim() == "crypt"); (Some(enc), format!("disk {src}: {}", if enc { "LUKS/dm-crypt" } else { "no disk encryption found" })) }
        None => (None, format!("disk {src}: could not tell")),
    }
}

pub fn security(app: &App) -> Value {
    let dir = &app.cfg.data_dir;
    let db_path = dir.join("core.db");
    let mut header = [0u8; 16];
    let read = std::fs::File::open(&db_path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut header)).is_ok();
    let db_encrypted = read && &header != b"SQLite format 3\0";
    let env_key = std::env::var("AGENT_DB_KEY").map(|v| !v.is_empty()).unwrap_or(false);
    let key_file = dir.join("db.key");
    let (disk, disk_note) = disk_encrypted(dir);
    let m = |p: &Path| mode_of(p).map(|x| format!("{x:o}"));
    let mut checks = vec![];
    let mut add = |name: &str, ok: Option<bool>, note: String| checks.push(json!({ "name": name, "ok": ok, "note": note }));
    add("Database file encrypted (SQLCipher)", Some(db_encrypted), if db_encrypted { "core.db does not start with the plain SQLite header".into() } else if read { "core.db is a PLAIN database: anyone with the file can read it".into() } else { "core.db not found".into() });
    add("Database key kept away from the data folder", Some(env_key || !key_file.exists()),
        if env_key { "key comes from AGENT_DB_KEY".into() } else { "key is the file data/db.key next to the database: it guards copies of the DB file, not a copy of all of data/".into() });
    if key_file.exists() { add("db.key is private (0600)", Some(m(&key_file).as_deref() == Some("600")), format!("mode {}", m(&key_file).unwrap_or("?".into()))); }
    let tok = dir.join("owner.token");
    if tok.exists() { add("owner.token is private (0600)", Some(m(&tok).as_deref() == Some("600")), format!("mode {} (the database keeps only its hash)", m(&tok).unwrap_or("?".into()))); }
    let backup = db_path.with_extension("db.plain-backup");
    add("No plain-text database backup left", Some(!backup.exists()), if backup.exists() { "core.db.plain-backup is unencrypted: delete it by hand".into() } else { "none found".into() });
    let mem = dir.join("memory");
    add("Readable memory copy (data/memory)", Some(!mem.exists()), if mem.exists() { "plain Markdown files, NOT encrypted: the database is the source, this is a convenience copy".into() } else { "not present".into() });
    let rep = dir.join("reports");
    if rep.exists() { add("Saved chat reports (data/reports)", Some(false), "plain JSON, secrets scrubbed, mode 0600 each: not encrypted".into()); }
    add("Whole disk encrypted", disk, disk_note);
    add("Server listens on this computer only", Some(app.cfg.bind == "127.0.0.1" || app.cfg.bind == "::1"), format!("bind {}", app.cfg.bind));
    add("Tools run in a sandbox", Some(app.cfg.sandbox != "unavailable" && app.cfg.sandbox != "off"), format!("sandbox: {}", app.cfg.sandbox));
    let all_ok = checks.iter().all(|c| c["ok"] == true);
    json!({ "ok": all_ok, "checked": crate::util::now_iso(), "checks": checks })
}

#[cfg(test)]
mod tests {
    #[test]
    fn weekday() {
        assert_eq!(super::weekday_of("2026-11-15"), Some(6)); // Sunday
        assert_eq!(super::weekday_of("2026-11-16"), Some(0)); // Monday
    }
}
