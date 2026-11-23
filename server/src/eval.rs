//! Model tests (docs/12, "Model tests"). Fixed requests are sent to each local model profile in
//! throwaway chats, and each answer is scored: did the model pick the right tool, did it end on a
//! FAILED tool result, did it write a reply, and how many steps, seconds and tokens it took.
//!
//! The cases are the smoke test's ten requests plus `evals/cases.json` (or `AGENT_EVAL_CASES`).
//! Cloud profiles run only when asked (`cloud: true`, they cost money). The chats are titled
//! `eval: ...`, archived after, and the reminders they made are cancelled; their failures never
//! count toward automatic reports. Results go to `data/evals/<time>.json`; `GET /v1/evals` lists them.
//!
//! Runs inside the server (an account's key lives only there): `POST /v1/evals/run`, or
//! `agent-core eval`, which asks a running server to do it, or runs on its own in one-owner mode.

use crate::{App, autoreport, util};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    pub id: String,
    #[serde(default = "general")]
    pub skill: String,
    pub text: String,
    /// The right pick: `plugin.tool`, `plugin` (any of its tools) or a core tool (`search_memory`).
    /// Any one of them counts. `[]`: no tool should run. Left out: not scored.
    #[serde(default)]
    pub expect: Option<Vec<String>>,
    /// A failed tool here only warns (it needs a sign-in or a network the server may not have).
    #[serde(default)]
    pub may_fail: bool,
    /// Cases with the same chat name share one chat, in order ("cancel that reminder").
    #[serde(default)]
    pub chat: Option<String>,
}

fn general() -> String {
    "general".into()
}

/// The smoke test's ten requests (`assistantx-smoke`), with the tools a good answer uses.
pub fn smoke_cases() -> Vec<Case> {
    let c = |id: &str, skill: &str, text: &str, expect: Option<&[&str]>, may_fail: bool| Case {
        id: id.into(), skill: skill.into(), text: text.into(), expect: expect.map(|e| e.iter().map(|s| s.to_string()).collect()),
        may_fail, chat: Some("smoke".into()),
    };
    vec![
        c("smoke-weather", "weather", "What is the weather in Berlin right now?", Some(&["weather.now"]), false),
        c("smoke-music-now", "music", "What is playing on my music right now?", Some(&["spotify.now"]), true),
        c("smoke-reminder-set", "reminders", "Set a reminder in 5 minutes to stretch.", Some(&["scheduler.create_reminder", "scheduler.create_alarm", "scheduler.create_alarms"]), false),
        c("smoke-reminder-cancel", "reminders", "Cancel that reminder.", Some(&["scheduler.cancel_job"]), false),
        c("smoke-reminder-list", "reminders", "List my reminders.", Some(&["scheduler.list_jobs"]), false),
        c("smoke-calendar", "calendar", "What is on my calendar for the next days?", Some(&["calendar.list_events", "calendar.find_events"]), false),
        c("smoke-memory-search", "memory", "Search your memory for anything about my favourite things.", Some(&["search_memory"]), false),
        c("smoke-time", "chat", "What time is it for me?", None, false),
        c("smoke-about-me", "memory", "What do you remember about me?", Some(&["search_memory", "load"]), false),
        c("smoke-thanks", "chat", "Thank you, that is all.", Some(&[]), false),
    ]
}

pub fn load_cases(path: &std::path::Path) -> Result<Vec<Case>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let list = if v.is_array() { v } else { v["cases"].clone() };
    serde_json::from_value(list).map_err(|e| format!("{}: {e}", path.display()))
}

/// The smoke cases plus the repo's file. A broken file is reported, not fatal.
pub fn default_cases() -> (Vec<Case>, Option<String>) {
    let path = std::env::var("AGENT_EVAL_CASES").unwrap_or_else(|_| "evals/cases.json".into());
    let mut v = smoke_cases();
    match load_cases(std::path::Path::new(&path)) {
        Ok(more) => {
            v.extend(more);
            (v, None)
        }
        Err(e) => (v, Some(e)),
    }
}

// ---------------- scoring ----------------

/// What happened in one turn.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Outcome {
    /// `plugin.tool` for plugin tools, the name for core tools (`load`, `search_memory`).
    pub called: Vec<String>,
    /// One per tool result: did it start with `[core] RESULT: FAILED`.
    pub failed: Vec<bool>,
    pub reply: String,
    /// done | asked (a tool waited for a yes) | error | cancelled | timeout
    pub status: String,
    pub error: Option<String>,
    pub steps: i64,
    pub secs: f64,
    pub tokens: i64,
}

/// The tool names the model called, from the assistant messages of one turn.
pub fn tool_names(messages: &[Value]) -> Vec<String> {
    let mut out = vec![];
    for m in messages.iter().filter(|m| m["role"] == "assistant") {
        for c in m["tool_calls"].as_array().into_iter().flatten() {
            let name = c["function"]["name"].as_str().unwrap_or_default();
            let args: Value = match &c["function"]["arguments"] {
                Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                v => v.clone(),
            };
            if name == "run_plugin_tool" {
                let p = args["plugin"].as_str().unwrap_or_default().trim_start_matches("plugin:");
                out.push(format!("{p}.{}", args["tool"].as_str().unwrap_or_default()));
            } else {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Does a called tool match one expected name? `scheduler` matches any scheduler tool.
pub fn matches(expect: &str, called: &str) -> bool {
    expect == called || (!expect.contains('.') && called.strip_prefix(expect).is_some_and(|r| r.starts_with('.')))
}

/// The score of one case: `{right_tool, no_failed, replied, pass}` (`right_tool` is null when not scored).
pub fn score(case: &Case, o: &Outcome) -> Value {
    let right_tool = case.expect.as_ref().map(|e| {
        if e.is_empty() {
            // Nothing should run; reading its own memory directory (`load`) is fine.
            o.called.iter().all(|c| c == "load" || c == "unload")
        } else {
            o.called.iter().any(|c| e.iter().any(|x| matches(x, c)))
        }
    });
    let ended_failed = o.failed.last().copied().unwrap_or(false);
    let no_failed = !ended_failed || case.may_fail;
    let finished = matches!(o.status.as_str(), "done" | "asked");
    let replied = o.status == "asked" || !o.reply.trim().is_empty();
    let pass = finished && replied && no_failed && right_tool != Some(false);
    json!({ "right_tool": right_tool, "no_failed": no_failed, "failed_results": o.failed.iter().filter(|f| **f).count(), "replied": replied, "finished": finished, "pass": pass })
}

#[derive(Default)]
struct Tally {
    cases: usize,
    pass: usize,
    right: usize,
    scored: usize,
    no_failed: usize,
    replied: usize,
    steps: i64,
    secs: f64,
    tokens: i64,
}

impl Tally {
    fn add(&mut self, r: &Value) {
        let s = &r["score"];
        self.cases += 1;
        self.pass += (s["pass"] == true) as usize;
        if let Some(b) = s["right_tool"].as_bool() {
            self.scored += 1;
            self.right += b as usize;
        }
        self.no_failed += (s["no_failed"] == true) as usize;
        self.replied += (s["replied"] == true) as usize;
        self.steps += r["steps"].as_i64().unwrap_or(0);
        self.secs += r["secs"].as_f64().unwrap_or(0.0);
        self.tokens += r["tokens"].as_i64().unwrap_or(0);
    }

    fn json(&self) -> Value {
        let n = self.cases.max(1) as f64;
        json!({ "cases": self.cases, "pass": self.pass, "right_tool": self.right, "scored": self.scored, "no_failed": self.no_failed,
            "replied": self.replied, "avg_steps": (self.steps as f64 / n * 10.0).round() / 10.0, "avg_secs": (self.secs / n * 10.0).round() / 10.0, "tokens": self.tokens })
    }
}

/// Per model and per (model, skill) totals from the case results.
pub fn summarize(results: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut by_model: BTreeMap<String, Tally> = BTreeMap::new();
    let mut by_skill: BTreeMap<(String, String), Tally> = BTreeMap::new();
    let mut order: Vec<String> = vec![];
    for r in results {
        let p = r["profile"].as_str().unwrap_or("").to_string();
        if !order.contains(&p) {
            order.push(p.clone());
        }
        by_model.entry(p.clone()).or_default().add(r);
        by_skill.entry((p, r["skill"].as_str().unwrap_or("").to_string())).or_default().add(r);
    }
    let models = order.iter().map(|p| { let mut v = by_model[p].json(); v["profile"] = json!(p); v }).collect();
    let skills = order.iter().flat_map(|p| by_skill.iter().filter(move |((q, _), _)| q == p)
        .map(|((p, s), t)| { let mut v = t.json(); v["profile"] = json!(p); v["skill"] = json!(s); v })).collect();
    (models, skills)
}

fn row(name: &str, t: &Value) -> String {
    format!("{name:<28} {:>5} {:>5} {:>10} {:>8} {:>8} {:>6} {:>7} {:>8}",
        t["cases"], t["pass"], format!("{}/{}", t["right_tool"], t["scored"]), format!("{}/{}", t["no_failed"], t["cases"]),
        format!("{}/{}", t["replied"], t["cases"]), t["avg_steps"], t["avg_secs"], t["tokens"])
}

/// Two plain tables: one line per model, then one per model and skill.
pub fn table(models: &[Value], skills: &[Value]) -> String {
    let head = format!("{:<28} {:>5} {:>5} {:>10} {:>8} {:>8} {:>6} {:>7} {:>8}", "model", "cases", "pass", "right-tool", "no-fail", "replied", "steps", "secs", "tokens");
    let mut out = vec![head.clone()];
    out.extend(models.iter().map(|m| row(m["profile"].as_str().unwrap_or(""), m)));
    out.push(String::new());
    out.push(head.replacen("model", "model / skill", 1).replacen("              ", "", 1));
    out.extend(skills.iter().map(|s| row(&format!("{} / {}", s["profile"].as_str().unwrap_or(""), s["skill"].as_str().unwrap_or("")), s)));
    out.join("\n")
}

// ---------------- running ----------------

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Opts {
    /// Also run the cloud profiles that have a key (they cost money).
    #[serde(default)]
    pub cloud: bool,
    /// Only these profile ids.
    #[serde(default)]
    pub profiles: Option<Vec<String>>,
    /// Only these case ids, or skills.
    #[serde(default)]
    pub only: Option<Vec<String>>,
    /// Seconds one turn may take (default 240).
    #[serde(default)]
    pub wait_secs: Option<u64>,
    /// Cases to use instead of the default ones (tests).
    #[serde(default)]
    pub cases: Option<Vec<Case>>,
}

/// Runs in progress, per data folder (one per account): the progress record.
static RUNNING: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);

fn key(app: &App) -> String {
    app.cfg.data_dir.display().to_string()
}

pub fn progress(app: &App) -> Option<Value> {
    RUNNING.lock().unwrap().as_ref().and_then(|m| m.get(&key(app)).cloned())
}

fn set_progress(app: &App, v: Option<Value>) {
    let mut g = RUNNING.lock().unwrap();
    let m = g.get_or_insert_with(HashMap::new);
    match v {
        Some(v) => { m.insert(key(app), v); }
        None => { m.remove(&key(app)); }
    }
}

/// Which profiles run: every local one (and the cloud ones with a key when `cloud`). Not routed: the one model.
pub fn profiles(app: &App, opts: &Opts) -> Vec<(String, String)> {
    if !app.is_routed() {
        return vec![("default".into(), app.model.name())];
    }
    crate::models::builtins(app).into_iter()
        .filter(|p| p.local || (opts.cloud && crate::models::ready(app, p)))
        .filter(|p| opts.profiles.as_ref().is_none_or(|l| l.contains(&p.id)))
        .map(|p| (p.id, p.label.split(',').next().unwrap_or("").trim().to_string()))
        .collect()
}

fn dir(app: &App) -> std::path::PathBuf {
    app.cfg.data_dir.join("evals")
}

/// Start a run in the background. Returns its id and plan, or an error when one is running already.
pub fn start(app: &Arc<App>, opts: Opts) -> Result<Value, String> {
    if progress(app).is_some() {
        return Err("a model test is running already".into());
    }
    let (mut cases, note) = match opts.cases.clone() {
        Some(c) => (c, None),
        None => default_cases(),
    };
    if let Some(only) = &opts.only {
        cases.retain(|c| only.contains(&c.id) || only.contains(&c.skill));
    }
    let profs = profiles(app, &opts);
    if cases.is_empty() || profs.is_empty() {
        return Err("nothing to test: no cases or no model profiles".into());
    }
    let id = util::now_iso().replace(':', "-");
    let plan = json!({ "id": id, "running": true, "started": util::now_iso(), "profiles": profs.iter().map(|p| &p.0).collect::<Vec<_>>(),
        "cases": cases.len(), "done": 0, "total": cases.len() * profs.len(), "cases_note": note });
    set_progress(app, Some(plan.clone()));
    let app2 = app.clone();
    tokio::spawn(async move {
        let out = run(&app2, &id, profs, cases, opts, note).await;
        if let Err(e) = save(&app2, &id, &out) {
            eprintln!("model test: could not save the result: {e}");
        }
        set_progress(&app2, None);
    });
    Ok(plan)
}

async fn run(app: &Arc<App>, id: &str, profs: Vec<(String, String)>, cases: Vec<Case>, opts: Opts, note: Option<String>) -> Value {
    let wait = Duration::from_secs(opts.wait_secs.unwrap_or(240).clamp(5, 1800));
    let started = util::now_iso();
    let mut results = vec![];
    let mut done = 0;
    for (profile, label) in &profs {
        let mut chats: HashMap<String, String> = HashMap::new();
        for case in &cases {
            let chat = case.chat.clone().unwrap_or_else(|| case.id.clone());
            let thread = chats.entry(chat.clone()).or_insert_with(|| {
                let t = util::new_id("thr");
                let model = if app.is_routed() { Some(profile.clone()) } else { None };
                let _ = app.db.lock().execute("INSERT INTO threads (id, title, created, model) VALUES (?1, ?2, ?3, ?4)",
                    params![t, format!("{} {profile} {chat}", autoreport::EVAL_TITLE), util::now_iso(), model]);
                t
            }).clone();
            let o = one(app, &thread, &case.text, wait).await;
            let s = score(case, &o);
            results.push(json!({ "profile": profile, "label": label, "case": case.id, "skill": case.skill, "text": case.text, "expect": case.expect,
                "called": o.called, "status": o.status, "error": o.error, "reply": o.reply.chars().take(400).collect::<String>(),
                "steps": o.steps, "secs": (o.secs * 10.0).round() / 10.0, "tokens": o.tokens, "score": s }));
            done += 1;
            if let Some(mut p) = progress(app) {
                p["done"] = json!(done);
                set_progress(app, Some(p));
            }
        }
        cleanup(app, &chats.into_values().collect::<Vec<_>>());
    }
    let (models, skills) = summarize(&results);
    let table = table(&models, &skills);
    json!({ "id": id, "started": started, "finished": util::now_iso(), "cloud": opts.cloud, "cases_note": note,
        "models": models, "skills": skills, "results": results, "table": table })
}

/// One request in its chat, waited for. A tool that waits for a yes is marked `asked` and the turn is stopped
/// (a cancelled wait voids its card), so nothing runs without the owner.
async fn one(app: &Arc<App>, thread: &str, text: &str, wait: Duration) -> Outcome {
    let t0 = Instant::now();
    let turn = app.start_turn(thread, "message", json!({ "text": text }));
    let mut asked = false;
    let status = loop {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let st: String = app.db.lock().query_row("SELECT status FROM turns WHERE id = ?1", [&turn], |r| r.get(0)).unwrap_or_default();
        match st.as_str() {
            "done" | "error" | "cancelled" => break st,
            "waiting" if !asked => {
                asked = true;
                app.cancel_running(thread);
            }
            _ if t0.elapsed() > wait => {
                app.cancel_running(thread);
                // Give it a moment to stop, then move on.
                for _ in 0..50 {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    if app.db.lock().query_row("SELECT status FROM turns WHERE id = ?1", [&turn], |r| r.get::<_, String>(0)).is_ok_and(|s| s != "running" && s != "queued" && s != "waiting") {
                        break;
                    }
                }
                break "timeout".into();
            }
            _ => {}
        }
    };
    let secs = t0.elapsed().as_secs_f64();
    let (tokens, error): (i64, Option<String>) = app.db.lock().query_row("SELECT tokens_used, error_code FROM turns WHERE id = ?1", [&turn], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((0, None));
    let steps: i64 = app.db.lock().query_row("SELECT COUNT(*) FROM model_calls WHERE turn_id = ?1", [&turn], |r| r.get(0)).unwrap_or(0);
    let msgs: Vec<Value> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT body FROM messages WHERE turn_id = ?1 ORDER BY n").expect("prepare");
        st.query_map([&turn], |r| r.get::<_, String>(0)).map(|x| x.flatten().filter_map(|s| serde_json::from_str(&s).ok()).collect()).unwrap_or_default()
    };
    let failed = msgs.iter().filter(|m| m["role"] == "tool").map(|m| m["content"].as_str().unwrap_or("").contains("[core] RESULT: FAILED")).collect();
    let reply = msgs.iter().rev().filter(|m| m["role"] == "assistant").find_map(|m| m["content"].as_str().filter(|t| !t.trim().is_empty())).unwrap_or("").to_string();
    let status = if asked { "asked".to_string() } else { status };
    Outcome { called: tool_names(&msgs), failed, reply, status, error, steps, secs, tokens }
}

/// Archive the chats and cancel what they scheduled.
fn cleanup(app: &App, threads: &[String]) {
    for t in threads {
        let jobs: Vec<String> = {
            let c = app.db.lock();
            let mut st = c.prepare("SELECT id FROM jobs WHERE origin_thread = ?1 AND status = 'active'").expect("prepare");
            st.query_map([t], |r| r.get(0)).map(|x| x.flatten().collect()).unwrap_or_default()
        };
        for j in jobs {
            crate::scheduler::cancel_job(app, &j);
        }
        let _ = app.db.lock().execute("UPDATE threads SET archived = 1 WHERE id = ?1", [t]);
    }
}

fn save(app: &App, id: &str, v: &Value) -> Result<(), String> {
    let d = dir(app);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(format!("{id}.json"));
    std::fs::write(&path, util::scrub(&serde_json::to_string_pretty(v).map_err(|e| e.to_string())?, &app.secret_words())).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Newest first, without the per-case results. The run in progress, if any, is `running`.
pub fn list(app: &App) -> Value {
    let mut items: Vec<Value> = std::fs::read_dir(dir(app)).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|r| json!({ "id": r["id"], "started": r["started"], "finished": r["finished"], "cloud": r["cloud"], "models": r["models"], "skills": r["skills"], "table": r["table"] }))
        .collect();
    items.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    json!({ "running": progress(app), "items": items })
}

/// One run in full; while it runs, its progress.
pub fn get(app: &App, id: &str) -> Option<Value> {
    if !valid_id(id) {
        return None;
    }
    if let Some(p) = progress(app).filter(|p| p["id"] == id) {
        return Some(p);
    }
    std::fs::read_to_string(dir(app).join(format!("{id}.json"))).ok().and_then(|s| serde_json::from_str(&s).ok())
}

// ---------------- the command line ----------------

/// `agent-core eval [--cloud] [--profiles a,b] [--only id,skill] [--wait-secs N] [--server URL --token-file F]`.
/// With `--server` (or `AGENT_EVAL_SERVER`) it asks that running server to run the tests and waits for the
/// table; the token is read from the file (or `AGENT_EVAL_TOKEN`) and never printed. Without, it runs here
/// on the one-owner data folder.
pub fn cli(args: &[String]) -> i32 {
    let mut opts = Opts::default();
    let mut server = std::env::var("AGENT_EVAL_SERVER").ok();
    let mut token_file: Option<String> = None;
    let mut i = 0;
    let list = |s: &str| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
    while i < args.len() {
        let next = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--cloud" => { opts.cloud = true; i += 1; continue; }
            "--profiles" => opts.profiles = Some(list(&next)),
            "--only" => opts.only = Some(list(&next)),
            "--wait-secs" => opts.wait_secs = next.parse().ok(),
            "--server" => server = Some(next),
            "--token-file" => token_file = Some(next),
            other => {
                eprintln!("unknown option '{other}'. Use: agent-core eval [--cloud] [--profiles a,b] [--only ids-or-skills] [--wait-secs N] [--server URL --token-file F]");
                return 2;
            }
        }
        i += 2;
    }
    match server {
        Some(url) => remote(&url, token_file, &opts),
        None => local(opts),
    }
}

fn local(opts: Opts) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    rt.block_on(async {
        let (app, _) = match crate::build(crate::config::Config::from_env()) {
            Ok(x) => x,
            Err(e) => { eprintln!("{e}"); return 1; }
        };
        let plan = match start(&app, opts) {
            Ok(p) => p,
            Err(e) => { eprintln!("{e}"); return 1; }
        };
        let id = plan["id"].as_str().unwrap_or_default().to_string();
        eprintln!("testing {} on {} ({} cases each)", id, plan["profiles"], plan["cases"]);
        while progress(&app).is_some() {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        match get(&app, &id) {
            Some(r) => { println!("{}\nsaved: data/evals/{id}.json", r["table"].as_str().unwrap_or("")); 0 }
            None => { eprintln!("the run left no result"); 1 }
        }
    })
}

fn remote(url: &str, token_file: Option<String>, opts: &Opts) -> i32 {
    let token = match token_file.map(|f| std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))).transpose() {
        Ok(Some(t)) => t.trim().to_string(),
        Ok(None) => std::env::var("AGENT_EVAL_TOKEN").unwrap_or_default(),
        Err(e) => { eprintln!("{e}"); return 2; }
    };
    if token.is_empty() {
        eprintln!("no token: pass --token-file or set AGENT_EVAL_TOKEN");
        return 2;
    }
    let base = url.trim_end_matches('/').to_string();
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).http_status_as_error(false).build().into();
    let auth = format!("Bearer {token}");
    let call = |method: &str, path: &str, body: Option<Value>| -> Result<(u16, Value), String> {
        let u = format!("{base}{path}");
        let resp = if method == "POST" {
            agent.post(&u).header("Authorization", &auth).header("Content-Type", "application/json").send(body.unwrap_or(json!({})).to_string())
        } else {
            agent.get(&u).header("Authorization", &auth).call()
        };
        let mut r = resp.map_err(|e| format!("cannot reach {base}: {e}"))?;
        let code = r.status().as_u16();
        let text = r.body_mut().with_config().limit(50_000_000).read_to_string().unwrap_or_default();
        Ok((code, serde_json::from_str(&text).unwrap_or(Value::String(text))))
    };
    let body = json!({ "cloud": opts.cloud, "profiles": opts.profiles, "only": opts.only, "wait_secs": opts.wait_secs });
    let plan = match call("POST", "/v1/evals/run", Some(body)) {
        Ok((202, v)) => v,
        Ok((423, _)) => { eprintln!("the account is locked (the server restarted): open the app once so it unlocks, then try again"); return 3; }
        Ok((code, v)) => { eprintln!("the server said {code}: {}", v["error"]["message"].as_str().map(String::from).unwrap_or_else(|| v.to_string())); return 1; }
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    let id = plan["id"].as_str().unwrap_or_default().to_string();
    eprintln!("testing {id} on {} ({} cases each)", plan["profiles"], plan["cases"]);
    if let Some(n) = plan["cases_note"].as_str() {
        eprintln!("note: {n}");
    }
    let mut last = -1;
    loop {
        std::thread::sleep(Duration::from_secs(5));
        match call("GET", &format!("/v1/evals/{id}"), None) {
            Ok((200, v)) if v["running"] == true => {
                let d = v["done"].as_i64().unwrap_or(0);
                if d != last {
                    eprintln!("  {d}/{}", v["total"]);
                    last = d;
                }
            }
            Ok((200, v)) => {
                println!("{}", v["table"].as_str().unwrap_or(""));
                for r in v["results"].as_array().into_iter().flatten().filter(|r| r["score"]["pass"] != true) {
                    println!("miss {} {}: called {} status {} -> {}", r["profile"].as_str().unwrap_or(""), r["case"].as_str().unwrap_or(""), r["called"], r["status"].as_str().unwrap_or(""),
                        r["reply"].as_str().unwrap_or("").replace('\n', " ").chars().take(120).collect::<String>());
                }
                println!("saved on the server: evals/{id}.json (GET /v1/evals/{id})");
                return 0;
            }
            Ok((code, _)) if code == 423 => { eprintln!("the account was locked during the run"); return 3; }
            Ok((code, v)) => { eprintln!("the server said {code}: {v}"); return 1; }
            Err(e) => { eprintln!("{e}"); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(expect: Option<&[&str]>, may_fail: bool) -> Case {
        Case { id: "c".into(), skill: "s".into(), text: "t".into(), expect: expect.map(|e| e.iter().map(|s| s.to_string()).collect()), may_fail, chat: None }
    }

    fn out(called: &[&str], failed: &[bool], reply: &str, status: &str) -> Outcome {
        Outcome { called: called.iter().map(|s| s.to_string()).collect(), failed: failed.to_vec(), reply: reply.into(), status: status.into(), ..Default::default() }
    }

    #[test]
    fn names_come_from_tool_calls() {
        let msgs = vec![
            json!({ "role": "assistant", "content": null, "tool_calls": [
                { "function": { "name": "load", "arguments": "{\"id\":\"plugin:weather\"}" } },
                { "function": { "name": "run_plugin_tool", "arguments": "{\"plugin\":\"plugin:weather\",\"tool\":\"now\",\"args\":{}}" } }] }),
            json!({ "role": "tool", "content": "[core] RESULT: DONE" }),
            json!({ "role": "assistant", "tool_calls": [{ "function": { "name": "search_memory", "arguments": { "query": "x" } } }] }),
        ];
        assert_eq!(tool_names(&msgs), vec!["load", "weather.now", "search_memory"]);
    }

    #[test]
    fn right_tool_failed_and_reply_are_scored() {
        let weather = case(Some(&["weather.now"]), false);
        let good = score(&weather, &out(&["load", "weather.now"], &[false], "12 C and sunny", "done"));
        assert_eq!(good["right_tool"], true);
        assert_eq!(good["pass"], true);
        // Wrong tool.
        let s = score(&weather, &out(&["web.fetch"], &[false], "it is warm", "done"));
        assert_eq!((s["right_tool"].clone(), s["pass"].clone()), (json!(false), json!(false)));
        // Ends on a FAILED result: fails, unless the case may fail. A failure fixed later is fine.
        assert_eq!(score(&weather, &out(&["weather.now"], &[false, true], "sorry", "done"))["no_failed"], false);
        assert_eq!(score(&weather, &out(&["weather.now", "weather.now"], &[true, false], "12 C", "done"))["pass"], true);
        assert_eq!(score(&case(Some(&["spotify.now"]), true), &out(&["spotify.now"], &[true], "not signed in", "done"))["pass"], true);
        // Empty reply, error.
        assert_eq!(score(&weather, &out(&["weather.now"], &[false], "  ", "done"))["replied"], false);
        assert_eq!(score(&weather, &out(&["weather.now"], &[false], "x", "error"))["pass"], false);
        // A plugin name matches any of its tools; a waiting card counts as answered.
        assert_eq!(score(&case(Some(&["scheduler"]), false), &out(&["scheduler.cancel_job"], &[], "", "asked"))["pass"], true);
        // `[]`: nothing should run (its own memory directory is fine). Missing: not scored.
        let none = case(Some(&[]), false);
        assert_eq!(score(&none, &out(&["load"], &[], "You're welcome", "done"))["pass"], true);
        assert_eq!(score(&none, &out(&["weather.now"], &[], "You're welcome", "done"))["right_tool"], false);
        assert!(score(&case(None, false), &out(&["web.fetch"], &[], "10:00", "done"))["right_tool"].is_null());
    }

    #[test]
    fn tables_add_up_per_model_and_skill() {
        let r = |p: &str, skill: &str, pass: bool, right: Option<bool>| json!({ "profile": p, "skill": skill, "steps": 2, "secs": 3.0, "tokens": 100,
            "score": { "pass": pass, "right_tool": right, "no_failed": true, "replied": true } });
        let results = vec![r("local-light", "weather", true, Some(true)), r("local-light", "chat", false, None), r("local", "weather", false, Some(false))];
        let (models, skills) = summarize(&results);
        assert_eq!(models[0]["profile"], "local-light");
        assert_eq!((models[0]["cases"].clone(), models[0]["pass"].clone(), models[0]["right_tool"].clone(), models[0]["scored"].clone()), (json!(2), json!(1), json!(1), json!(1)));
        assert_eq!(models[1]["tokens"], 100);
        assert_eq!(skills.len(), 3);
        let t = table(&models, &skills);
        assert!(t.contains("local-light / weather") && t.contains("1/1"), "{t}");
    }

    #[test]
    fn shipped_cases_file_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("evals/cases.json");
        let cases = load_cases(&path).unwrap();
        assert!(cases.len() >= 5);
        let mut ids: Vec<String> = cases.iter().chain(smoke_cases().iter()).map(|c| c.id.clone()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "case ids are unique");
        assert!(!valid_id("../x") && valid_id("2026-11-17T19-30-00Z"));
    }
}
