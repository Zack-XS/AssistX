//! Body numbers from the phone (plugin `health`).
//!
//! The Android app reads Android's on-device store (steps, sleep, heart rate, weight, workouts)
//! after the user allows it there, adds the numbers up per day, and sends one row per day to
//! `POST /v1/health/days`. The server keeps only these daily totals, never raw readings. They
//! reach the model only when it asks through the `summary` tool, and arrive as outside content.

use crate::{App, util};
use rusqlite::params;
use serde_json::{Map, Value, json};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS health_days (day TEXT NOT NULL, metric TEXT NOT NULL, value REAL NOT NULL, device TEXT, updated INTEGER NOT NULL, PRIMARY KEY (day, metric));
"#;

/// The numbers a day may hold, and what they mean. Anything else is skipped.
pub const METRICS: &[(&str, &str)] = &[
    ("steps", "count"),
    ("distance_m", "metres"),
    ("floors", "count"),
    ("active_kcal", "kcal burned in activity"),
    ("total_kcal", "kcal burned all day"),
    ("sleep_min", "minutes asleep"),
    ("resting_hr", "beats per minute"),
    ("avg_hr", "beats per minute"),
    ("max_hr", "beats per minute"),
    ("hrv_ms", "milliseconds"),
    ("spo2_pct", "percent"),
    ("weight_kg", "kilograms"),
    ("body_fat_pct", "percent"),
    ("exercise_min", "minutes of workouts"),
    ("exercise_count", "number of workouts"),
    ("hydration_ml", "millilitres"),
];

fn day_ok(d: &str) -> bool {
    let p: Vec<&str> = d.split('-').collect();
    p.len() == 3
        && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2
        && p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit()))
        && (2000..=2100).contains(&p[0].parse::<u32>().unwrap_or(0))
        && (1..=12).contains(&p[1].parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&p[2].parse::<u32>().unwrap_or(0))
}

/// `{"days": [{"day": "2025-01-31", "metrics": {"steps": 8123, "sleep_min": 421}}]}`. A day sent
/// again replaces the numbers it names (the phone sends today again as it grows).
pub fn add_days(app: &App, device: &str, days: &Value) -> Result<Value, String> {
    let list = days.as_array().ok_or("days must be a list")?;
    if list.len() > 120 {
        return Err("at most 120 days at a time".into());
    }
    let now = util::unix_now();
    let (mut kept, mut skipped) = (0, 0);
    let c = app.db.lock();
    for d in list {
        let (Some(day), Some(m)) = (d["day"].as_str().filter(|x| day_ok(x)), d["metrics"].as_object()) else { skipped += 1; continue };
        for (name, v) in m {
            let ok = METRICS.iter().any(|(n, _)| n == name);
            match v.as_f64().filter(|x| ok && x.is_finite() && (0.0..=10_000_000.0).contains(x)) {
                Some(x) => {
                    let _ = c.execute(
                        "INSERT INTO health_days (day, metric, value, device, updated) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(day, metric) DO UPDATE SET value = ?3, device = ?4, updated = ?5",
                        params![day, name, x, device, now],
                    );
                    kept += 1;
                }
                None => skipped += 1,
            }
        }
    }
    Ok(json!({ "kept": kept, "skipped": skipped }))
}

/// Days newest first. With `from`/`to` (inclusive dates) it is that range; else the last `last` days that have data.
pub fn days(app: &App, from: Option<&str>, to: Option<&str>, last: i64) -> Result<Vec<Value>, String> {
    for d in [from, to].into_iter().flatten() {
        if !day_ok(d) {
            return Err("dates look like 2025-01-31".into());
        }
    }
    let (lo, hi) = (from.unwrap_or("0000-00-00"), to.unwrap_or("9999-99-99"));
    let c = app.db.lock();
    let mut st = c
        .prepare("SELECT day, metric, value FROM health_days WHERE day >= ?1 AND day <= ?2 AND day IN (SELECT DISTINCT day FROM health_days WHERE day >= ?1 AND day <= ?2 ORDER BY day DESC LIMIT ?3) ORDER BY day DESC")
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params![lo, hi, last.clamp(1, 120)], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)))
        .map_err(|e| e.to_string())?;
    let mut out: Vec<(String, Map<String, Value>)> = vec![];
    for r in rows.flatten() {
        if out.last().map(|l| l.0 != r.0).unwrap_or(true) {
            out.push((r.0.clone(), Map::new()));
        }
        // Whole numbers look like whole numbers.
        let v = if r.2.fract() == 0.0 { json!(r.2 as i64) } else { json!((r.2 * 10.0).round() / 10.0) };
        out.last_mut().unwrap().1.insert(r.1, v);
    }
    Ok(out.into_iter().map(|(day, m)| json!({ "day": day, "metrics": m })).collect())
}

pub fn tool(app: &App, tool: &str, a: &Value) -> Result<Value, String> {
    match tool {
        "summary" => {
            let items = days(app, a["from"].as_str(), a["to"].as_str(), a["days"].as_i64().unwrap_or(7))?;
            let units: Map<String, Value> = METRICS.iter().map(|(n, u)| (n.to_string(), json!(u))).collect();
            let note = if items.is_empty() { "No body numbers yet. The phone app sends them once Health Connect is allowed and turned on." } else { "Totals per day from the phone. A missing number means the phone had none that day." };
            Ok(json!({ "days": items, "units": units, "note": note }))
        }
        _ => Err("no such tool".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_checked() {
        assert!(day_ok("2025-01-31") && !day_ok("2025-1-31") && !day_ok("1999-01-01") && !day_ok("2025-13-01") && !day_ok("x"));
    }
}
