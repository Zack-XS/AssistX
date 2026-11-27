//! Which model answers (docs/12).
//!
//! A list of model profiles: the small and medium local models on this server (Ollama), a
//! heavy local reasoning model (with a warning), and cloud models (NVIDIA NIM, Google Gemini,
//! OVH AI Endpoints). Keys live in the secrets table, never in files.
//!
//! In `auto` mode each turn starts on the small local model for very light messages ("thanks",
//! "hi") and on the medium one otherwise. A model may call `use_bigger_model`: small to medium
//! happens at once; medium to cloud asks the user first (by default). A fixed mode sends every
//! turn to one profile.

use crate::model::Adapter;
use crate::openai_compat::OpenAiCompat;
use crate::{App, store};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
    /// Name in the secrets table holding the key, if this profile needs one.
    pub key_secret: Option<String>,
    /// The key may also come from AGENT_MODEL_API_KEY (the old one-model setting).
    pub key_from_env: bool,
    /// The key is optional (the service works without one, slower).
    pub key_optional: bool,
    pub local: bool,
    /// light | default | strong | cloud
    pub tier: &'static str,
    pub warning: Option<&'static str>,
}

fn local_base() -> String {
    std::env::var("AGENT_LOCAL_MODEL_BASE").unwrap_or_else(|_| "http://127.0.0.1:11434/v1".into())
}

/// The built-in list. Model names can be changed per profile with PATCH /v1/models.
pub fn builtins(app: &App) -> Vec<Profile> {
    catalog(app, false)
}

/// Every profile; `all` = also the cloud ones not added yet.
fn catalog(app: &App, all: bool) -> Vec<Profile> {
    let lb = local_base();
    let p = |id: &str, label: &str, base: &str, model: &str, key: Option<&str>, local: bool, tier: &'static str| Profile {
        id: id.into(), label: label.into(), base_url: base.into(), model: model.into(), key_secret: key.map(String::from),
        key_from_env: false, key_optional: false, local, tier, warning: None,
    };
    let mut v = vec![
        p("local-light", "Gemma 4 E4B, on your server (light: quick commands, summaries, sorting, simple decisions)", &lb, "assistant-gemma4", None, true, "light"),
        p("local", "Qwen 3.5 9B, on your server (medium: general reasoning, complex instructions, everyday planning)", &lb, "assistant-qwen9b", None, true, "default"),
        Profile { warning: Some("Bigger than the server's 8 GB graphics card, so part of it runs on the processor: answers take longer and it uses about 24 GB of memory while it runs."),
            ..p("local-smart", "Qwen3.6 35B-A3B, on your server (smart: deep reasoning, hard decisions, complicated plans)", &lb, "assistant-qwen35b", None, true, "strong") },
        Profile { key_from_env: true, ..p("nim", "NVIDIA NIM (cloud)", "https://integrate.api.nvidia.com/v1", "nvidia/nemotron-3-ultra-550b-a55b", Some("NIM_API_KEY"), false, "cloud") },
        p("gemini", "Google Gemini (cloud)", "https://generativelanguage.googleapis.com/v1beta/openai", "gemini-3.5-flash", Some("GEMINI_API_KEY"), false, "cloud"),
        Profile { key_optional: true, ..p("ovh", "OVH AI Endpoints (cloud, works without a key at a low rate)", "https://oai.endpoints.kepler.ai.cloud.ovh.net/v1", "gpt-oss-120b", Some("OVH_AI_KEY"), false, "cloud") },
    ];
    // Proton Lumo through a local browser bridge, only when one is set (src/lumo.rs).
    if let Some(u) = crate::lumo::bridge_url() {
        v.push(Profile { key_optional: true, warning: Some("Slow: each answer is typed into Lumo's web page by a browser on your server. Tools work through written calls, so they are less reliable than on the other models."),
            ..p("lumo", "Proton Lumo, your own account through a browser bridge on your server", &u, "lumo", None, false, "cloud") });
    }
    // The old single-model setting stays usable as its own profile.
    let m = &app.cfg.model;
    if m.kind == "openai" {
        v.push(Profile { key_from_env: true, ..p("env", "Set by AGENT_MODEL_* on the server", &m.base_url, &m.name, None, m.is_local(), if m.is_local() { "default" } else { "cloud" }) });
    }
    // The owner's changes (another model name on a profile).
    let s = settings(app);
    // NIM, Gemini and OVH are off the list until the owner adds them (`PATCH /v1/models {add}`).
    // One with its key already saved counts as added, so a working setup keeps working.
    v.retain(|p| all || !ADDABLE.contains(&p.id.as_str()) || is_added(&s, &p.id)
        || (!in_list(&s, "removed", &p.id) && p.key_secret.as_deref().is_some_and(|k| store::secret_get(app, k).is_some())));
    for prof in &mut v {
        if let Some(name) = s["set"][&prof.id]["model"].as_str().filter(|n| !n.trim().is_empty()) {
            prof.model = name.trim().to_string();
        }
    }
    v
}

pub fn settings(app: &App) -> Value {
    let raw: Option<String> = app.db.lock().query_row("SELECT value FROM settings WHERE key = 'models'", [], |r| r.get(0)).ok();
    let mut v: Value = raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or(json!({}));
    let dflt = std::env::var("AGENT_MODEL_MODE").unwrap_or_else(|_| "auto-mix".into());
    if !v["mode"].is_string() { v["mode"] = json!(dflt); }
    if v["mode"] == "auto" { v["mode"] = json!("auto-mix"); } // the old name
    if !v["cloud"].is_string() { v["cloud"] = json!("lumo"); }
    if !v["added"].is_array() { v["added"] = json!([]); }
    if !v["cloud_upgrade"].is_string() { v["cloud_upgrade"] = json!("ask"); }
    if !v["tasks"].is_object() { v["tasks"] = json!({}); }
    v
}

/// Cloud profiles hidden until the owner adds them.
pub const ADDABLE: &[&str] = &["nim", "gemini", "ovh"];

fn in_list(s: &Value, list: &str, id: &str) -> bool {
    s[list].as_array().is_some_and(|a| a.iter().any(|x| x == id))
}

fn is_added(s: &Value, id: &str) -> bool {
    in_list(s, "added", id)
}

/// The automatic modes. Local: Gemma, then Qwen 9B, then Qwen 35B, never the cloud.
/// Mix: the same, then the cloud model (asks first by default).
/// Smart: Lumo for background work and longer chats, Qwen 9B when Lumo is busy or not set up.
pub const MODES: &[(&str, &str)] = &[
    ("auto-local", "Auto (Local): Gemma 4 E4B, then Qwen 9B, then Qwen 35B. Never leaves your server."),
    ("auto-mix", "Auto (Mix): the local ones first, then a cloud model when one is really needed (asks you first)."),
    ("auto-smart", "Auto (Smart): Lumo for memory sorting, notices, automations and longer chats; Qwen 9B when Lumo is busy or off. Quick commands stay on Gemma."),
];

/// Jobs that can have their own model, to make them faster or better. Plugins are `plugin:<id>`.
pub const TASKS: &[(&str, &str)] = &[
    ("dreaming", "Sorting memory (dreaming)"),
    ("background", "Scheduled jobs and outside events"),
    ("build", "Building new plugins"),
];

fn is_auto(mode: &str) -> bool {
    mode.starts_with("auto")
}

/// The model set for one job (`dreaming`, `background`, `build`, `plugin:spotify`), if any.
pub fn task_profile(app: &App, task: &str) -> Option<String> {
    let s = settings(app);
    s["tasks"][task].as_str().filter(|id| get(app, id).is_some()).map(String::from)
}

pub fn save_settings(app: &App, v: &Value) {
    let _ = app.db.lock().execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('models', ?1)", [v.to_string()]);
}

pub fn profile_ids(app: &App) -> Vec<String> {
    builtins(app).into_iter().map(|p| p.id).collect()
}

pub fn get(app: &App, id: &str) -> Option<Profile> {
    builtins(app).into_iter().find(|p| p.id == id)
}

fn key(app: &App, p: &Profile) -> Option<String> {
    p.key_secret.as_deref().and_then(|s| store::secret_get(app, s))
        .or_else(|| if p.key_from_env { app.cfg.model.api_key.clone() } else { None })
}

/// Can this profile be used now (it has the key it needs)?
pub fn ready(app: &App, p: &Profile) -> bool {
    p.local || p.key_optional || key(app, p).is_some()
}

/// Every key a profile may use, so logs and tool output can be scrubbed of them.
pub fn all_keys(app: &App) -> Vec<String> {
    builtins(app).iter().filter_map(|p| key(app, p)).collect()
}

/// Is this a valid choice of who answers: `auto-local`, `auto-mix` or a profile id?
pub fn valid_mode(app: &App, m: &str) -> bool {
    MODES.iter().any(|x| x.0 == m) || get(app, m).is_some()
}

/// Who answers in this chat: the chat's own pick, else the default from settings.
pub fn mode_for(app: &App, thread: &str) -> String {
    let own: Option<String> = app.db.lock().query_row("SELECT model FROM threads WHERE id = ?1", [thread], |r| r.get(0)).ok().flatten();
    own.filter(|m| valid_mode(app, m)).unwrap_or_else(|| settings(app)["mode"].as_str().unwrap_or("auto-mix").to_string())
}

/// Which profile a new turn starts on. `light_text` is the user's message when it is one;
/// `background` is a turn nobody is waiting on (a job, a trigger, dreaming, a build).
pub fn pick(app: &App, mode: &str, light_text: Option<&str>, background: bool) -> String {
    if !is_auto(mode) && get(app, mode).is_some() {
        return mode.to_string();
    }
    let main = || if mode == "auto-smart" { smart(app) } else { "local".into() };
    if background {
        return task_profile(app, "background").unwrap_or_else(main);
    }
    match light_text {
        // Short chat and short "do this" commands: Gemma answers in a second or two.
        Some(t) if is_light(t) || is_command(t) => "local-light".into(),
        _ => main(),
    }
}

/// Auto (Smart)'s main model: Lumo when its bridge is set up, else Qwen 9B. While Lumo is
/// limited or its bridge is down, the Lumo adapter itself hands the turn to Qwen 9B.
fn smart(app: &App) -> String {
    if get(app, "lumo").is_some() { "lumo".into() } else { "local".into() }
}

const ACT: &[&str] = &["play", "pause", "skip", "next", "previous", "volume", "remind", "alarm", "timer", "set", "add", "cancel", "delete",
    "move", "copy", "turn", "start", "stop", "open", "send", "call", "text", "name", "save", "note", "list", "show", "read", "find"];
const THINK: &[&str] = &["plan", "why", "explain", "compare", "should", "decide", "help", "think", "advice", "best", "write", "draft", "idea", "ideas"];

fn words(t: &str) -> Vec<String> {
    t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// A short "do this" ("play Espresso", "set an alarm for 7", "remind me to call Sam at 5"):
/// tool work, no thinking needed.
pub fn is_command(text: &str) -> bool {
    let w = words(text);
    !w.is_empty() && w.len() <= 20 && w.iter().any(|x| ACT.contains(&x.as_str())) && !w.iter().any(|x| THINK.contains(&x.as_str()))
}

/// Very light: a short message that asks for nothing to be done ("thanks", "hi", "ok cool").
pub fn is_light(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    let words = t.split_whitespace().count();
    if words == 0 || words > 8 || t.len() > 60 {
        return false;
    }
    const WORK: &[&str] = &["remind", "alarm", "play", "set", "add", "schedule", "calendar", "find", "search", "read", "file", "folder",
        "computer", "where", "location", "music", "spotify", "song", "mail", "send", "delete", "move", "copy", "write", "make", "create",
        "build", "code", "why", "how", "explain", "plan", "weather", "news", "run", "open", "list", "show", "what", "when", "who", "change",
        "cancel", "turn", "start", "stop", "pause", "next", "volume", "remember", "forget", "note", "buy", "book", "call", "text", "message"];
    !t.split(|c: char| !c.is_alphanumeric()).any(|w| WORK.contains(&w))
}

/// The next profile up from this one, and whether moving there needs a yes.
/// Gemma, then Qwen 9B, then Qwen 35B, all on this server and at once. In Auto (Mix) the
/// cloud comes last and asks first (by default); Auto (Local) and a fixed model stop there.
pub fn next_up(app: &App, from: &str, mode: &str) -> Option<(Profile, bool)> {
    let s = settings(app);
    let cur = get(app, from)?;
    let local = |id: &str| get(app, id).map(|p| (p, false));
    match cur.tier {
        "light" => local("local"),
        "default" => local("local-smart"),
        "strong" => {
            if s["cloud_upgrade"] == "never" || mode != "auto-mix" {
                return None;
            }
            let cloud = get(app, s["cloud"].as_str().unwrap_or("nim")).filter(|p| !p.local)?;
            Some((cloud, s["cloud_upgrade"] != "auto"))
        }
        _ => None,
    }
}

#[derive(Default)]
pub struct Router {
    cache: Mutex<HashMap<String, Arc<dyn Adapter>>>,
}

static WARM_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WARM_LAST: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Keep the light model (Gemma) loaded whenever no other model is, so short messages answer at
/// once. A bigger model pushes it out of the graphics card when it needs the room (Ollama does
/// that by itself); once that one goes idle and unloads, Gemma comes back. Checked every 30 s,
/// off the timer thread. Off with `PATCH /v1/models {"keep_light_loaded": false}`.
pub fn keep_light_warm(app: &App) {
    let now = crate::util::unix_now();
    if !app.is_routed() || now - WARM_LAST.load(std::sync::atomic::Ordering::Relaxed) < 30 {
        return;
    }
    WARM_LAST.store(now, std::sync::atomic::Ordering::Relaxed);
    if settings(app)["keep_light_loaded"] == false {
        return;
    }
    let Some(p) = get(app, "local-light").filter(|p| p.local) else { return };
    if WARM_BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let base = p.base_url.trim_end_matches('/').trim_end_matches("/v1").to_string();
    let model = p.model.clone();
    std::thread::spawn(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(120))).http_status_as_error(false).build().into();
        let loaded = agent.get(&format!("{base}/api/ps")).call().ok()
            .and_then(|mut r| r.body_mut().read_to_string().ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| v["models"].as_array().map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect::<Vec<_>>()).unwrap_or_default());
        // Nothing loaded (and Ollama answered): load Gemma and keep it until something needs the room.
        if loaded.is_some_and(|l| l.is_empty()) {
            let body = json!({ "model": model, "keep_alive": -1, "prompt": "", "stream": false }).to_string();
            let _ = agent.post(&format!("{base}/api/generate")).header("Content-Type", "application/json").send(body);
        }
        WARM_BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// "Never send to the cloud" (security review #7, #25): `local_only` in the account's model settings, or
/// `AGENT_LOCAL_ONLY=1` for the whole server. Checked here, where every model call gets its adapter, so no mode,
/// chat setting, job setting or `use_bigger_model` can get around it.
pub fn local_only(app: &App) -> bool {
    std::env::var("AGENT_LOCAL_ONLY").is_ok_and(|v| v == "1") || settings(app)["local_only"] == true
}

/// The profile a call may really use: a cloud one is swapped for the local default while `local_only` is on.
pub fn allowed_profile(app: &App, id: &str) -> String {
    match get(app, id) {
        Some(p) if !p.local && local_only(app) => {
            let local = builtins(app).into_iter().find(|l| l.local && l.id == "local").or_else(|| builtins(app).into_iter().find(|l| l.local));
            let to = local.map(|l| l.id).unwrap_or_else(|| "local".into());
            eprintln!("security: local only is on; a call for cloud model '{id}' uses '{to}' instead");
            to
        }
        _ => id.to_string(),
    }
}

impl Router {
    /// The adapter for a profile. Tests and demos (`mock`) always get the one mock model.
    pub fn adapter(&self, app: &App, id: &str) -> Result<Arc<dyn Adapter>, String> {
        if app.cfg.model.kind == "mock" {
            return Ok(app.model.clone());
        }
        let id = allowed_profile(app, id);
        let p = get(app, &id).ok_or_else(|| format!("no model profile '{id}'"))?;
        let k = key(app, &p);
        if !p.local && !p.key_optional && k.is_none() {
            return Err(format!("{} has no key yet. Add the secret {} in Settings, Intelligence.", p.label, p.key_secret.as_deref().unwrap_or("?")));
        }
        let cache_key = format!("{}|{}|{}|{}", p.id, p.base_url, p.model, crate::util::sha256_hex(k.as_deref().unwrap_or("").as_bytes()));
        let mut c = self.cache.lock().unwrap();
        if let Some(a) = c.get(&cache_key) {
            return Ok(a.clone());
        }
        let a: Arc<dyn Adapter> = if p.id == "lumo" {
            // While Lumo rests after a limit, the medium local model answers.
            let fallback = get(app, "local").filter(|l| l.local).map(|l| Arc::new(OpenAiCompat::new(&l.base_url, &l.model, None)) as Arc<dyn Adapter>);
            Arc::new(crate::lumo::Lumo::new(&p.base_url, fallback))
        } else {
            Arc::new(OpenAiCompat::new(&p.base_url, &p.model, k))
        };
        c.retain(|key, _| !key.starts_with(&format!("{}|", p.id)));
        c.insert(cache_key, a.clone());
        Ok(a)
    }
}

/// For GET /v1/models.
pub fn describe(app: &App) -> Value {
    let s = settings(app);
    let list: Vec<Value> = builtins(app).iter().map(|p| json!({
        "id": p.id, "label": p.label, "model": p.model, "base_url": p.base_url, "local": p.local, "tier": p.tier,
        "warning": p.warning, "ready": ready(app, p), "key_secret": p.key_secret, "key_optional": p.key_optional,
        "removable": ADDABLE.contains(&p.id.as_str()),
    })).collect();
    // Cloud profiles not on the list yet; the page shows an Add button for each.
    let shown: Vec<String> = builtins(app).into_iter().map(|p| p.id).collect();
    let can_add: Vec<Value> = catalog(app, true).into_iter().filter(|p| ADDABLE.contains(&p.id.as_str()) && !shown.contains(&p.id))
        .map(|p| json!({ "id": p.id, "label": p.label, "key_secret": p.key_secret, "key_optional": p.key_optional })).collect();
    let tasks: Vec<Value> = TASKS.iter().map(|(id, label)| json!({ "id": id, "label": label, "model": s["tasks"][id] })).collect();
    json!({ "mode": s["mode"], "cloud": s["cloud"], "cloud_upgrade": s["cloud_upgrade"], "local_only": local_only(app), "keep_light_loaded": s["keep_light_loaded"] != false, "profiles": list, "can_add": can_add,
        "modes": MODES.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
        "tasks": tasks, "plugin_models": s["tasks"].as_object().map(|o| o.iter().filter(|(k, _)| k.starts_with("plugin:"))
            .map(|(k, v)| (k.trim_start_matches("plugin:").to_string(), v.clone())).collect::<serde_json::Map<_, _>>()).unwrap_or_default() })
}

/// PATCH /v1/models: `{add?, remove?, mode?, cloud?, cloud_upgrade?, set?: {profile: {model}}, tasks?: {job: profile | null}}`.
/// A job is `dreaming`, `background`, `build`, or `plugin:<id>` (for example `plugin:spotify`).
pub fn update(app: &App, b: &Value) -> Result<Value, String> {
    let mut s = settings(app);
    // `{add: "nim"}` puts a hidden cloud profile on the list; `{remove: "nim"}` takes it off again.
    for (field, on) in [("add", true), ("remove", false)] {
        let Some(id) = b[field].as_str() else { continue };
        if !ADDABLE.contains(&id) {
            return Err(format!("only {} can be added or removed", ADDABLE.join(", ")));
        }
        for (list, keep) in [("added", on), ("removed", !on)] {
            let mut l: Vec<Value> = s[list].as_array().cloned().unwrap_or_default();
            l.retain(|x| x != id);
            if keep { l.push(json!(id)); }
            s[list] = json!(l);
        }
        if !on {
            if s["cloud"] == id { s["cloud"] = json!("lumo"); }
            if s["mode"] == id { s["mode"] = json!("auto-mix"); }
            if let Some(t) = s["tasks"].as_object_mut() { t.retain(|_, v| v != id); }
        }
        // Saved now, so the checks below see the new list.
        save_settings(app, &s);
    }
    if let Some(m) = b["mode"].as_str() {
        let m = if m == "auto" { "auto-mix" } else { m };
        if !MODES.iter().any(|x| x.0 == m) && get(app, m).is_none() {
            return Err(format!("mode is auto-local, auto-mix, auto-smart or a profile id ({})", builtins(app).iter().map(|p| p.id.clone()).collect::<Vec<_>>().join(", ")));
        }
        s["mode"] = json!(m);
    }
    if let Some(o) = b["tasks"].as_object() {
        for (task, v) in o {
            let known = TASKS.iter().any(|t| t.0 == task) || task.strip_prefix("plugin:").is_some_and(|p| app.manifest(p).is_some());
            if !known {
                return Err(format!("no job '{task}'; use dreaming, background, build or plugin:<id>"));
            }
            match v.as_str() {
                None | Some("") => { s["tasks"].as_object_mut().map(|t| t.remove(task)); }
                Some(id) if get(app, id).is_some() => s["tasks"][task] = json!(id),
                Some(id) => return Err(format!("no profile '{id}'")),
            }
        }
    }
    if let Some(c) = b["cloud"].as_str() {
        if !get(app, c).is_some_and(|p| !p.local) {
            return Err("cloud must be a cloud profile: nim, gemini, ovh or lumo".into());
        }
        s["cloud"] = json!(c);
    }
    if let Some(u) = b["cloud_upgrade"].as_str() {
        if !["ask", "auto", "never"].contains(&u) {
            return Err("cloud_upgrade is ask, auto or never".into());
        }
        s["cloud_upgrade"] = json!(u);
    }
    if let Some(k) = b["keep_light_loaded"].as_bool() {
        s["keep_light_loaded"] = json!(k);
    }
    if let Some(k) = b["local_only"].as_bool() {
        s["local_only"] = json!(k);
    }
    if let Some(o) = b["set"].as_object() {
        for (id, v) in o {
            get(app, id).ok_or_else(|| format!("no profile '{id}'"))?;
            let name = v["model"].as_str().unwrap_or("").trim();
            if name.len() > 120 || name.contains(char::is_whitespace) {
                return Err("model names have no spaces and at most 120 characters".into());
            }
            s["set"][id] = json!({ "model": name });
        }
    }
    save_settings(app, &s);
    Ok(describe(app))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_tiny_chit_chat_is_light() {
        for t in ["thanks!", "hi there", "ok cool", "good night"] {
            assert!(is_light(t), "{t}");
        }
        for t in ["play some music", "what time is it", "remind me at 5", "", "can you help me plan a trip to Rome next spring with my family"] {
            assert!(!is_light(t), "{t}");
        }
    }

    #[test]
    fn short_do_this_messages_are_commands() {
        for t in ["play Espresso", "set an alarm for 7", "remind me to call Sam at 5", "pause the music"] {
            assert!(is_command(t), "{t}");
        }
        for t in ["help me plan my week", "why is the sky blue", "should I buy a car or lease one", "thanks"] {
            assert!(!is_command(t), "{t}");
        }
    }
}
