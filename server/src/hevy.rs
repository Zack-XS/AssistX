//! Hevy (workout log) through its public API, for your account (docs/03, plugin `hevy`).
//!
//! Hevy gives each Pro user an API key. The user stores it once as the secret `HEVY_API_KEY`
//! (`PUT /v1/secrets/HEVY_API_KEY`). The core makes the calls to one fixed host and holds the
//! key, so the model and the plugin folder never see it. The API has no delete, so nothing here
//! can be undone; the one write tool always asks first.

use crate::{store, App};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

const KEY: &str = "HEVY_API_KEY";

pub async fn call(app: &Arc<App>, tool: &str, args: &Value) -> Result<Value, String> {
    let (app, tool, args) = (app.clone(), tool.to_string(), args.clone());
    tokio::task::spawn_blocking(move || {
        let key = store::secret_get(&app, KEY)
            .filter(|k| !k.trim().is_empty())
            .ok_or("Hevy is not connected. Save your Hevy API key (Hevy app, Settings, Developer; needs Hevy Pro) as the secret HEVY_API_KEY.")?;
        run(&app.cfg.hevy_base, key.trim(), &tool, &args)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Is a Hevy key saved? (The gym check in `detect` asks only then.)
pub fn connected(app: &App) -> bool {
    store::secret_get(app, KEY).is_some_and(|k| !k.trim().is_empty())
}

/// The latest workouts (first page, newest first), for the gym check in `detect`. Blocking.
pub fn recent_workouts(app: &App) -> Result<Vec<Value>, String> {
    let key = store::secret_get(app, KEY).filter(|k| !k.trim().is_empty()).ok_or("Hevy is not connected")?;
    let v = run(&app.cfg.hevy_base, key.trim(), "workouts", &json!({ "page_size": 10 }))?;
    Ok(v["workouts"].as_array().cloned().unwrap_or_default())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).http_status_as_error(false).build().into()
}

fn api(base: &str, key: &str, path: &str, query: &[(&str, String)], body: Option<&Value>) -> Result<Value, String> {
    let mut url = format!("{base}{path}");
    if !query.is_empty() {
        let q: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        url.push('?');
        url.push_str(&crate::oauth::form(&q));
    }
    let a = agent();
    let mut resp = match body {
        Some(b) => a.post(&url).header("api-key", key).header("Content-Type", "application/json").send(b.to_string()),
        None => a.get(&url).header("api-key", key).call(),
    }
    .map_err(|e| format!("Could not reach Hevy: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return Ok(v);
    }
    let msg = v["error"].as_str().or(v["message"].as_str()).unwrap_or("");
    Err(match status {
        401 | 403 => "Hevy rejected the API key. Check HEVY_API_KEY (Hevy Pro is needed).".into(),
        404 => "Hevy has nothing with that id.".into(),
        429 => "Hevy says to slow down. Try again in a moment.".into(),
        _ => format!("Hevy answered {status}{}", if msg.is_empty() { String::new() } else { format!(": {msg}") }),
    })
}

fn id_ok(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn page(args: &Value, max: i64) -> Vec<(&'static str, String)> {
    let p = args["page"].as_i64().unwrap_or(1).clamp(1, 10_000);
    let n = args["page_size"].as_i64().unwrap_or(max).clamp(1, max);
    vec![("page", p.to_string()), ("pageSize", n.to_string())]
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v[k].as_str().filter(|x| !x.is_empty())
}

/// Check a workout before sending it: Hevy's own errors are terse.
fn check_workout(w: &Value) -> Result<(), String> {
    if !w.is_object() {
        return Err("workout must be an object".into());
    }
    for k in ["title", "start_time", "end_time"] {
        if s(w, k).is_none() {
            return Err(format!("workout.{k} is required (times look like 2025-01-31T18:00:00Z)"));
        }
    }
    if s(w, "title").is_some_and(|t| t.len() > 200) {
        return Err("workout.title is too long".into());
    }
    let ex = w["exercises"].as_array().filter(|a| !a.is_empty() && a.len() <= 50).ok_or("workout.exercises needs 1 to 50 exercises")?;
    for e in ex {
        if !s(e, "exercise_template_id").is_some_and(id_ok) {
            return Err("each exercise needs an exercise_template_id taken from exercise_templates".into());
        }
        let sets = e["sets"].as_array().filter(|a| !a.is_empty() && a.len() <= 100).ok_or("each exercise needs 1 to 100 sets")?;
        for st in sets {
            if let Some(t) = st["type"].as_str() {
                if !["warmup", "normal", "failure", "dropset"].contains(&t) {
                    return Err("set type must be warmup, normal, failure or dropset".into());
                }
            }
        }
    }
    Ok(())
}

fn run(base: &str, key: &str, tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "workouts" => api(base, key, "/v1/workouts", &page(args, 10), None),
        "workout_count" => api(base, key, "/v1/workouts/count", &[], None),
        "workout" => {
            let id = s(args, "id").filter(|i| id_ok(i)).ok_or("id is required (from workouts)")?;
            api(base, key, &format!("/v1/workouts/{id}"), &[], None)
        }
        "routines" => api(base, key, "/v1/routines", &page(args, 10), None),
        "exercise_templates" => api(base, key, "/v1/exercise_templates", &page(args, 100), None),
        "log_workout" => {
            let w = &args["workout"];
            check_workout(w)?;
            let mut w = w.clone();
            if w["is_private"].is_null() {
                w["is_private"] = json!(true); // private unless the user said otherwise
            }
            api(base, key, "/v1/workouts", &[], Some(&json!({ "workout": w })))
        }
        _ => Err(format!("hevy has no tool {tool}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workout_is_checked_before_it_is_sent() {
        let good = json!({ "title": "Push", "start_time": "2025-01-31T18:00:00Z", "end_time": "2025-01-31T19:00:00Z",
            "exercises": [{ "exercise_template_id": "D04AC939", "sets": [{ "type": "normal", "weight_kg": 60, "reps": 8 }] }] });
        assert!(check_workout(&good).is_ok());
        let mut bad = good.clone();
        bad["exercises"][0]["sets"][0]["type"] = json!("easy");
        assert!(check_workout(&bad).unwrap_err().contains("set type"));
        bad = good.clone();
        bad["exercises"] = json!([]);
        assert!(check_workout(&bad).is_err());
        assert!(check_workout(&json!({ "title": "x" })).unwrap_err().contains("start_time"));
    }

    #[test]
    fn ids_cannot_change_the_path() {
        assert!(id_ok("b459cba5-cd6d-463c-abd6-54f8eafcadcb") && !id_ok("../x") && !id_ok("a/b") && !id_ok(""));
    }
}
