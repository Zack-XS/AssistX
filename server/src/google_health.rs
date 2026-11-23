//! Google Health (the Fitbit Web API's replacement) for your account (docs/03, plugin `google-health`).
//!
//! Read only. The core talks to one fixed host, builds the address from checked pieces, and
//! holds the token, so the model and the plugin folder never see it. Sign-in is in `oauth.rs`
//! (provider `google-health`: you make a Google Cloud OAuth client, paste its Client ID and
//! Client Secret, and approve once).

use crate::oauth;
use crate::App;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

/// Data types that are single readings at a moment; the rest cover a stretch of time.
const SAMPLE_TYPES: &[&str] = &[
    "weight", "body-fat", "heart-rate", "resting-heart-rate", "heart-rate-variability", "oxygen-saturation",
    "respiratory-rate", "blood-glucose", "body-temperature", "vo2-max",
];
const MAX_POINTS: usize = 200;

pub async fn call(app: &Arc<App>, tool: &str, args: &Value) -> Result<Value, String> {
    let (app, tool, args) = (app.clone(), tool.to_string(), args.clone());
    tokio::task::spawn_blocking(move || {
        let token = oauth::fresh_token(&app, "google-health")?;
        run(&app.cfg.google_health_base, &token, &tool, &args)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn type_ok(t: &str) -> bool {
    (1..=40).contains(&t.len()) && t.split('-').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// `YYYY-MM-DD`, with a real-looking month and day.
fn date_ok(d: &str) -> bool {
    let p: Vec<&str> = d.split('-').collect();
    p.len() == 3
        && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2
        && p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit()))
        && (1..=12).contains(&p[1].parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&p[2].parse::<u32>().unwrap_or(0))
}

/// The list filter for a data type over `[from, to)` in the user's own calendar days.
fn filter(data_type: &str, from: &str, to: &str) -> String {
    let name = data_type.replace('-', "_");
    let field = if SAMPLE_TYPES.contains(&data_type) { "sample_time.civil_time" } else { "interval.civil_start_time" };
    format!("{name}.{field} >= \"{from}\" AND {name}.{field} < \"{to}\"")
}

fn explain(status: u16, body: &Value) -> String {
    let msg = body["error"]["message"].as_str().unwrap_or("");
    match status {
        401 => "Google rejected the login. Connect Google Health again in Settings, Integrations.".into(),
        403 => "Google has not allowed this. Connect Google Health again, allow every permission, and check the Google Health API is enabled for your Cloud project.".into(),
        429 => "Google says to slow down. Try again in a moment.".into(),
        400 | 404 if !msg.is_empty() => format!("Google Health said: {msg}"),
        _ => format!("Google Health answered {status}{}", if msg.is_empty() { String::new() } else { format!(": {msg}") }),
    }
}

fn run(base: &str, token: &str, tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "data" => {
            let t = args["data_type"].as_str().filter(|t| type_ok(t)).ok_or("data_type is required, like steps, weight, heart-rate, sleep or exercise")?;
            let from = args["from"].as_str().filter(|d| date_ok(d)).ok_or("from is required, like 2025-01-31")?;
            let to = args["to"].as_str().filter(|d| date_ok(d)).ok_or("to is required, like 2025-02-01 (the day after the last day you want)")?;
            if to <= from {
                return Err("to must be after from".into());
            }
            let size = args["page_size"].as_i64().unwrap_or(100).clamp(1, 100).to_string();
            let flt = filter(t, from, to);
            let mut q: Vec<(&str, &str)> = vec![("filter", &flt), ("pageSize", &size)];
            if let Some(p) = args["page_token"].as_str().filter(|p| !p.is_empty() && p.len() <= 500) {
                q.push(("pageToken", p));
            }
            let url = format!("{base}/v4/users/me/dataTypes/{t}/dataPoints?{}", oauth::form(&q));
            let a: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
            let mut resp = a.get(&url).header("Authorization", &format!("Bearer {token}")).call().map_err(|e| format!("Could not reach Google Health: {e}"))?;
            let status = resp.status().as_u16();
            let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
            if !(200..300).contains(&status) {
                return Err(explain(status, &v));
            }
            let all = v["dataPoints"].as_array().cloned().unwrap_or_default();
            let cut = all.len() > MAX_POINTS;
            Ok(json!({
                "data_type": t, "from": from, "to": to,
                "points": all.into_iter().take(MAX_POINTS).collect::<Vec<_>>(),
                "truncated": cut, "next_page_token": v["nextPageToken"],
            }))
        }
        "daily_summary" => {
            let from = args["from"].as_str().filter(|d| date_ok(d)).ok_or("from is required, like 2025-01-31")?;
            let to = args["to"].as_str().filter(|d| date_ok(d)).ok_or("to is required, like 2025-02-01 (the day after the last day you want)")?;
            let days = day_number(to) - day_number(from);
            if !(1..=MAX_ROLLUP_DAYS).contains(&days) {
                return Err(format!("the range must be 1 to {MAX_ROLLUP_DAYS} days (to is the day after the last day)"));
            }
            let types: Vec<String> = match args["data_types"].as_array() {
                Some(a) if !a.is_empty() && a.len() <= 8 => a.iter().filter_map(|t| t.as_str().filter(|t| type_ok(t)).map(String::from)).collect(),
                Some(_) => return Err("data_types needs 1 to 8 names".into()),
                None => ["steps", "distance", "total-calories", "active-zone-minutes"].map(String::from).to_vec(),
            };
            if types.is_empty() {
                return Err("data_types has no valid names".into());
            }
            let body = json!({ "range": { "start": civil(from), "end": civil(to) }, "windowSizeDays": 1 });
            let (mut days_out, mut errors) = (serde_json::Map::new(), serde_json::Map::new());
            for t in &types {
                let url = format!("{base}/v4/users/me/dataTypes/{t}/dataPoints:dailyRollUp");
                let a: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
                let mut resp = a
                    .post(&url)
                    .header("Authorization", &format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .send(body.to_string())
                    .map_err(|e| format!("Could not reach Google Health: {e}"))?;
                let status = resp.status().as_u16();
                let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
                if status == 401 {
                    return Err(explain(status, &v));
                }
                if !(200..300).contains(&status) {
                    errors.insert(t.clone(), json!(explain(status, &v)));
                    continue;
                }
                for p in v["rollupDataPoints"].as_array().into_iter().flatten().take(MAX_ROLLUP_DAYS as usize) {
                    let c = &p["civilStartTime"]["date"];
                    let day = format!("{:04}-{:02}-{:02}", c["year"].as_i64().unwrap_or(0), c["month"].as_i64().unwrap_or(0), c["day"].as_i64().unwrap_or(0));
                    let mut value = p.clone();
                    if let Some(o) = value.as_object_mut() {
                        o.remove("civilStartTime");
                        o.remove("civilEndTime");
                    }
                    days_out.entry(day).or_insert_with(|| json!({}))[t.as_str()] = value;
                }
            }
            Ok(json!({ "from": from, "to": to, "days": days_out, "errors": errors }))
        }
        _ => Err(format!("google-health has no tool {tool}")),
    }
}

/// Longest range one daily summary covers (heart rate allows 14 days, so all use 14).
const MAX_ROLLUP_DAYS: i64 = 14;

fn civil(d: &str) -> Value {
    let p: Vec<i64> = d.split('-').filter_map(|x| x.parse().ok()).collect();
    json!({ "date": { "year": p[0], "month": p[1], "day": p[2] } })
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` date (proleptic Gregorian).
fn day_number(d: &str) -> i64 {
    let p: Vec<i64> = d.split('-').filter_map(|x| x.parse().ok()).collect();
    let (y, m, dd) = (p[0] - (p[1] <= 2) as i64, p[1], p[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dd - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_are_checked() {
        assert!(type_ok("steps") && type_ok("heart-rate") && !type_ok("a/b") && !type_ok("") && !type_ok("-x") && !type_ok("A"));
        assert!(date_ok("2025-01-31") && !date_ok("2025-1-31") && !date_ok("2025-13-01") && !date_ok("x"));
    }

    #[test]
    fn day_counting_crosses_months_and_leap_days() {
        assert_eq!(day_number("2024-03-01") - day_number("2024-02-01"), 29);
        assert_eq!(day_number("2025-01-01") - day_number("2024-12-25"), 7);
    }

    #[test]
    fn filter_uses_the_right_time_field() {
        assert_eq!(filter("steps", "2025-01-01", "2025-01-02"), "steps.interval.civil_start_time >= \"2025-01-01\" AND steps.interval.civil_start_time < \"2025-01-02\"");
        assert!(filter("heart-rate", "2025-01-01", "2025-01-02").starts_with("heart_rate.sample_time.civil_time >="));
    }
}
