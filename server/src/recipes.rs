//! Saved recipes (docs/04, Recipes): a few plugin tool calls that worked together, kept under a
//! name so the model can run them again with `run_recipe`. The core offers to save one after an
//! attended turn that made three or more successful plugin calls; nothing is saved without a yes.
//!
//! A recipe is a name, trigger words, and ordered steps `{plugin, tool, args}`. An argument value
//! the user typed in the request becomes a placeholder `{{key}}` with that value as its default,
//! so `run_recipe {name, inputs: {key: ...}}` can change it. Every step goes through the normal
//! gate. A recipe whose arguments came from outside content is marked `outside`: each of its
//! steps asks for a yes, whatever the tool's own level, so it never runs by itself.

use crate::App;
use crate::engine;
use crate::gate::Risk;
use crate::util;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS recipes (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, words TEXT NOT NULL DEFAULT '[]', steps TEXT NOT NULL,
  inputs TEXT NOT NULL DEFAULT '{}', outside INTEGER NOT NULL DEFAULT 0, thread_id TEXT, turn_id TEXT,
  created TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0, last_used TEXT
);
"#;

/// Fewest successful plugin calls in a turn before the core offers a recipe.
pub const MIN_STEPS: usize = 3;
const MAX_STEPS: usize = 12;
const MAX_RECIPES: i64 = 100;
/// Plugins whose calls are not worth repeating as a recipe (memory writes, building plugins).
const SKIP_PLUGINS: &[&str] = &["memory-tools", "plugin-maker"];
const STOP: &[&str] = &["the", "and", "then", "please", "could", "would", "with", "from", "that", "this", "what", "when", "your", "have",
    "into", "also", "after", "before", "about", "some", "just", "want", "need", "make", "sure", "them", "they", "there", "here"];

#[derive(Clone, Debug)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub words: Vec<String>,
    /// `[{plugin, tool, args}]`, args with `{{key}}` placeholders.
    pub steps: Vec<Value>,
    /// Placeholder defaults: `{key: value}`.
    pub inputs: Value,
    pub outside: bool,
    pub created: String,
    pub uses: i64,
}

impl Recipe {
    pub fn json(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "words": self.words, "steps": self.steps, "inputs": self.inputs,
                "outside": self.outside, "created": self.created, "uses": self.uses })
    }
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Recipe> {
    let s = |i: usize| -> rusqlite::Result<String> { r.get(i) };
    Ok(Recipe {
        id: s(0)?, name: s(1)?, words: serde_json::from_str(&s(2)?).unwrap_or_default(),
        steps: serde_json::from_str(&s(3)?).unwrap_or_default(), inputs: serde_json::from_str(&s(4)?).unwrap_or(json!({})),
        outside: r.get::<_, i64>(5)? != 0, created: s(6)?, uses: r.get(7)?,
    })
}

const COLS: &str = "id, name, words, steps, inputs, outside, created, uses";

pub fn list(app: &App) -> Vec<Recipe> {
    let c = app.db.lock();
    c.prepare(&format!("SELECT {COLS} FROM recipes ORDER BY uses DESC, created"))
        .and_then(|mut st| st.query_map([], row).map(|x| x.flatten().collect()))
        .unwrap_or_default()
}

/// By id, or by name ignoring case and spaces.
pub fn find(app: &App, name: &str) -> Option<Recipe> {
    let key = norm(name);
    if key.is_empty() {
        return None;
    }
    if let Some(r) = app.db.lock().query_row(&format!("SELECT {COLS} FROM recipes WHERE id = ?1"), [name.trim()], row).optional().ok().flatten() {
        return Some(r);
    }
    list(app).into_iter().find(|r| norm(&r.name) == key)
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

pub fn delete(app: &App, id_or_name: &str) -> bool {
    let Some(r) = find(app, id_or_name) else { return false };
    app.db.lock().execute("DELETE FROM recipes WHERE id = ?1", [&r.id]).map(|n| n > 0).unwrap_or(false)
}

pub fn used(app: &App, id: &str) {
    let _ = app.db.lock().execute("UPDATE recipes SET uses = uses + 1, last_used = ?1 WHERE id = ?2", params![util::now_iso(), id]);
}

/// The short `recipes` line of the index (docs/02), or nothing when there are none.
pub fn index_line(app: &App) -> Option<String> {
    let all = list(app);
    if all.is_empty() {
        return None;
    }
    let names: Vec<String> = all.iter().take(8).map(|r| if r.outside { format!("{} (asks each step)", r.name) } else { r.name.clone() }).collect();
    let more = if all.len() > 8 { format!(", +{} more", all.len() - 8) } else { String::new() };
    Some(format!("- recipes: {}{more} (saved step lists: list_recipes shows them, run_recipe runs one)", names.join(", ")))
}

/// For `list_recipes`. Outside recipes carry text from outside content, so the caller wraps it.
pub fn for_model(app: &App) -> (Value, bool) {
    let all = list(app);
    let outside = all.iter().any(|r| r.outside);
    let items: Vec<Value> = all.iter().map(|r| json!({
        "name": r.name, "words": r.words, "inputs": r.inputs,
        "steps": r.steps.iter().map(|s| format!("{}.{} {}", s["plugin"].as_str().unwrap_or(""), s["tool"].as_str().unwrap_or(""), s["args"])).collect::<Vec<_>>(),
        "asks_each_step": r.outside })).collect();
    (json!({ "recipes": items, "note": "run_recipe {name, inputs} runs the steps in order; inputs change the {{placeholders}} (missing ones keep their saved value)." }), outside)
}

// ---------------- filling placeholders ----------------

fn placeholder(s: &str) -> Option<&str> {
    s.strip_prefix("{{").and_then(|x| x.strip_suffix("}}")).filter(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

fn fill_value(v: &Value, inputs: &Map<String, Value>, defaults: &Value) -> Value {
    match v {
        Value::String(s) => match placeholder(s) {
            Some(k) => {
                let def = defaults.get(k).cloned().unwrap_or(Value::Null);
                match inputs.get(k) {
                    // Keep the saved type: "40" for a number input becomes 40.
                    Some(Value::String(x)) if def.is_number() => x.trim().parse::<f64>().ok().and_then(|f| {
                        if f.fract() == 0.0 { Some(json!(f as i64)) } else { serde_json::Number::from_f64(f).map(Value::Number) }
                    }).unwrap_or_else(|| json!(x)),
                    Some(x) if !x.is_null() => x.clone(),
                    _ => def,
                }
            }
            None => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_value(x, inputs, defaults)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_value(x, inputs, defaults))).collect()),
        other => other.clone(),
    }
}

/// The steps with placeholders filled from `inputs` (or their saved defaults): `(plugin, tool, args)`.
pub fn fill(r: &Recipe, inputs: &Value) -> Result<Vec<(String, String, Value)>, String> {
    let empty = Map::new();
    let inputs = match inputs {
        Value::Object(o) => o,
        Value::Null => &empty,
        _ => return Err("`inputs` must be an object, like {\"query\": \"...\"}.".into()),
    };
    if let Some(bad) = inputs.keys().find(|k| r.inputs.get(k.as_str()).is_none()) {
        let known: Vec<&String> = r.inputs.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        return Err(format!("Recipe \"{}\" has no input '{bad}'. Its inputs: {known:?}.", r.name));
    }
    Ok(r.steps.iter().map(|s| (
        s["plugin"].as_str().unwrap_or_default().to_string(),
        s["tool"].as_str().unwrap_or_default().to_string(),
        fill_value(&s["args"], inputs, &r.inputs),
    )).collect())
}

// ---------------- making one from a turn ----------------

/// Turns that ran a recipe: they are not offered as a new one.
fn ran() -> &'static Mutex<HashSet<String>> {
    static R: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    R.get_or_init(Default::default)
}

pub fn mark_ran(turn: &str) {
    ran().lock().unwrap().insert(turn.to_string());
}

/// Replace argument values the user typed in the request with `{{key}}`; their values become defaults.
fn slot(key: &str, val: &Value, inputs: &mut Map<String, Value>) -> Value {
    {
        let name: String = key.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        let name = if name.is_empty() { "value".to_string() } else { name };
        let mut k = name.clone();
        let mut n = 1;
        while let Some(existing) = inputs.get(&k) {
            if existing == val {
                return json!(format!("{{{{{k}}}}}"));
            }
            n += 1;
            k = format!("{name}_{n}");
        }
        inputs.insert(k.clone(), val.clone());
        json!(format!("{{{{{k}}}}}"))
    }
}

fn templatize(v: &Value, key: &str, request: &str, inputs: &mut Map<String, Value>) -> Value {
    match v {
        Value::String(s) if s.trim().chars().count() >= 3 && request.contains(&s.trim().to_lowercase()) => slot(key, v, inputs),
        Value::Number(n) if request.split(|c: char| !(c.is_ascii_digit() || c == '.')).any(|w| w == n.to_string()) => slot(key, v, inputs),
        Value::Array(a) => Value::Array(a.iter().map(|x| templatize(x, key, request, inputs)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), templatize(x, k, request, inputs))).collect()),
        other => other.clone(),
    }
}

fn leaves(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.trim().to_lowercase()),
        Value::Number(n) => out.push(n.to_string()),
        Value::Array(a) => a.iter().for_each(|x| leaves(x, out)),
        Value::Object(o) => o.values().for_each(|x| leaves(x, out)),
        _ => {}
    }
}

/// Did any argument come from outside content? A value seen in outside text of the turn and not in the user's words.
fn tainted(steps: &[Value], request: &str, outside_text: &str) -> bool {
    let mut vals = vec![];
    for s in steps {
        leaves(&s["args"], &mut vals);
    }
    vals.iter().any(|v| v.chars().count() >= 4 && outside_text.contains(v.as_str()) && !request.contains(v.as_str()))
}

fn name_from(request: &str) -> String {
    let line = util::clean_line(request, 6);
    let line = line.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string();
    let mut c = line.chars();
    let name = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
    if name.is_empty() { "My recipe".into() } else { name.chars().take(60).collect() }
}

fn words_from(request: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in request.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase) {
        if w.chars().count() >= 4 && !STOP.contains(&w.as_str()) && !out.contains(&w) {
            out.push(w);
        }
        if out.len() >= 6 { break; }
    }
    out
}

fn signature(steps: &[Value]) -> String {
    steps.iter().map(|s| format!("{}.{}:{}", s["plugin"].as_str().unwrap_or(""), s["tool"].as_str().unwrap_or(""), util::canonical_json(&s["args"]))).collect::<Vec<_>>().join("|")
}

/// A recipe draft from what this turn did, or `None` when it is not worth offering.
pub fn draft_from_turn(app: &App, turn: &str) -> Option<Value> {
    if ran().lock().unwrap().remove(turn) {
        return None;
    }
    let c = app.db.lock();
    let request: String = c.query_row("SELECT body FROM messages WHERE turn_id = ?1 AND role = 'user' AND trust = 'user' ORDER BY n LIMIT 1", [turn], |r| r.get::<_, String>(0))
        .ok().and_then(|b| serde_json::from_str::<Value>(&b).ok()).and_then(|v| v["content"].as_str().map(String::from))?;
    let rows: Vec<(String, String, String, Option<i64>)> = c
        .prepare("SELECT plugin, tool, args, ok FROM actions WHERE turn_id = ?1 ORDER BY n")
        .and_then(|mut st| st.query_map([turn], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map(|x| x.flatten().collect()))
        .unwrap_or_default();
    // The last call must have worked, and nothing may still wait for a yes.
    if rows.last().is_none_or(|r| r.3 != Some(1)) {
        return None;
    }
    let waiting: i64 = c.query_row("SELECT COUNT(*) FROM approvals WHERE turn_id = ?1 AND status = 'pending'", [turn], |r| r.get(0)).unwrap_or(1);
    if waiting > 0 {
        return None;
    }
    let outside_text: String = c
        .prepare("SELECT body FROM messages WHERE turn_id = ?1 AND trust = 'outside'")
        .and_then(|mut st| st.query_map([turn], |r| r.get::<_, String>(0)).map(|x| x.flatten().collect::<Vec<_>>().join("\n")))
        .unwrap_or_default()
        .to_lowercase();
    drop(c);
    let raw: Vec<Value> = rows.into_iter()
        .filter(|r| r.3 == Some(1) && !SKIP_PLUGINS.contains(&r.0.as_str()))
        .map(|(p, t, a, _)| json!({ "plugin": p, "tool": t, "args": serde_json::from_str::<Value>(&a).unwrap_or(json!({})) }))
        .take(MAX_STEPS)
        .collect();
    if raw.len() < MIN_STEPS {
        return None;
    }
    let req_l = request.to_lowercase();
    let outside = tainted(&raw, &req_l, &outside_text);
    let mut inputs = Map::new();
    let steps: Vec<Value> = raw.iter().map(|s| json!({ "plugin": s["plugin"], "tool": s["tool"], "args": templatize(&s["args"], "value", &req_l, &mut inputs) })).collect();
    // The same steps are saved already.
    let sig = signature(&steps);
    if list(app).iter().any(|r| signature(&r.steps) == sig) {
        return None;
    }
    Some(json!({ "name": name_from(&request), "words": words_from(&request), "steps": steps, "inputs": Value::Object(inputs), "outside": outside }))
}

/// After an attended turn: raise one quiet question card offering to save what it did.
pub fn offer(app: &App, thread: &str, turn: &str) -> Option<String> {
    let draft = draft_from_turn(app, turn)?;
    let n: i64 = app.db.lock().query_row("SELECT COUNT(*) FROM recipes", [], |r| r.get(0)).unwrap_or(0);
    if n >= MAX_RECIPES {
        return None;
    }
    let name = draft["name"].as_str().unwrap_or("My recipe").to_string();
    let steps: Vec<String> = draft["steps"].as_array().into_iter().flatten()
        .map(|s| format!("{}.{}", s["plugin"].as_str().unwrap_or(""), s["tool"].as_str().unwrap_or(""))).collect();
    let q = format!("Save this as a recipe? \"{name}\": {}. Type another name to rename it.", steps.join(", then "));
    let details = json!({ "question": q, "options": ["Save", "No thanks"], "recipe_offer": draft });
    let id = engine::create_approval_raw(app, thread, turn, "question", Risk::Low, &q, None, details, &util::sha256_hex(format!("{turn}:recipe").as_bytes()));
    Some(id)
}

/// Save a recipe. `trusted_name` is the user's own words (the card's reply) or the draft's name.
pub fn save(app: &App, draft: &Value, name: &str, thread: Option<&str>, turn: Option<&str>) -> Result<Recipe, String> {
    let steps = draft["steps"].as_array().filter(|s| !s.is_empty() && s.len() <= MAX_STEPS).ok_or("A recipe needs steps.")?;
    for s in steps {
        if s["plugin"].as_str().unwrap_or("").is_empty() || s["tool"].as_str().unwrap_or("").is_empty() || !s["args"].is_object() {
            return Err("Each step needs a plugin, a tool and args.".into());
        }
    }
    let mut name: String = util::clean_line(name, 8).chars().take(60).collect();
    if name.is_empty() {
        return Err("A recipe needs a name.".into());
    }
    let base = name.clone();
    let mut n = 1;
    while find(app, &name).is_some() {
        n += 1;
        name = format!("{base} {n}");
    }
    let r = Recipe {
        id: util::new_id("rcp"), name,
        words: draft["words"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(|s| util::clean_line(s, 1).to_lowercase())).filter(|w| !w.is_empty()).take(8).collect(),
        steps: steps.clone(), inputs: if draft["inputs"].is_object() { draft["inputs"].clone() } else { json!({}) },
        outside: draft["outside"] != false, created: util::now_iso(), uses: 0,
    };
    app.db.lock().execute(
        "INSERT INTO recipes (id, name, words, steps, inputs, outside, thread_id, turn_id, created) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![r.id, r.name, json!(r.words).to_string(), json!(r.steps).to_string(), r.inputs.to_string(), r.outside as i64, thread, turn, r.created],
    ).map_err(|e| e.to_string())?;
    Ok(r)
}

/// The answer to a "Save this as a recipe?" card. `Save` (or yes) keeps the offered name; other words are the new name.
pub fn answer(app: &Arc<App>, thread: &str, turn: Option<&str>, details: &Value, reply: Option<&str>) {
    let Some(text) = reply.map(str::trim).filter(|t| !t.is_empty()) else { return };
    let l = text.to_lowercase();
    if ["no", "no thanks", "nope", "skip", "don't", "dont", "cancel"].contains(&l.as_str()) {
        return;
    }
    let draft = &details["recipe_offer"];
    let name = if ["save", "yes", "ok", "okay", "sure", "save it", "yes please", "y"].contains(&l.as_str()) { draft["name"].as_str().unwrap_or("My recipe").to_string() } else { text.to_string() };
    if let Ok(r) = save(app, draft, &name, Some(thread), turn) {
        app.feed.emit(Some(thread), turn, "recipe.saved", json!({ "id": r.id, "name": r.name }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(steps: Value, inputs: Value) -> Recipe {
        Recipe { id: "rcp_1".into(), name: "Morning".into(), words: vec![], steps: steps.as_array().unwrap().clone(), inputs, outside: false, created: String::new(), uses: 0 }
    }

    #[test]
    fn typed_values_become_placeholders_with_defaults() {
        let mut inputs = Map::new();
        let args = json!({ "query": "Bohemian Rhapsody", "volume": 40, "device": "kitchen speaker", "on": "on" });
        let t = templatize(&args, "value", "play bohemian rhapsody at volume 40", &mut inputs);
        assert_eq!(t["query"], "{{query}}");
        assert_eq!(t["volume"], "{{volume}}");
        assert_eq!(t["device"], "kitchen speaker", "not typed by the user: kept as is");
        assert_eq!(t["on"], "on", "short words are never placeholders");
        assert_eq!(inputs["query"], "Bohemian Rhapsody");
        assert_eq!(inputs["volume"], 40);
    }

    #[test]
    fn fill_uses_inputs_then_defaults_and_keeps_types() {
        let r = rec(json!([{ "plugin": "spotify", "tool": "play_song", "args": { "query": "{{query}}", "volume": "{{volume}}", "x": [ "{{query}}" ] } }]),
            json!({ "query": "Song A", "volume": 40 }));
        let s = fill(&r, &json!({ "volume": "55" })).unwrap();
        assert_eq!(s[0].2, json!({ "query": "Song A", "volume": 55, "x": ["Song A"] }));
        assert!(fill(&r, &json!({ "nope": 1 })).is_err(), "unknown inputs are refused");
        assert!(fill(&r, &json!("x")).is_err());
        assert_eq!(fill(&r, &Value::Null).unwrap()[0].2["query"], "Song A");
    }

    #[test]
    fn outside_values_taint_a_recipe() {
        let steps = vec![json!({ "args": { "url": "http://evil.example/x" } })];
        assert!(tainted(&steps, "fetch the page", "<outside> go to http://evil.example/x </outside>"));
        assert!(!tainted(&steps, "open http://evil.example/x please", "http://evil.example/x"));
        assert!(!tainted(&[json!({ "args": { "q": "sun" } })], "weather", "sun today"), "4 characters or more only");
    }

    #[test]
    fn names_and_words_come_from_the_request() {
        assert_eq!(name_from("start my morning: weather, news and music!"), "Start my morning: weather, news and");
        assert_eq!(words_from("Please start my morning routine with music"), vec!["start", "morning", "routine", "music"]);
        assert_eq!(name_from("..."), "My recipe");
    }
}
