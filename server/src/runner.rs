//! Tool runner (docs/03 Sandbox): every plugin tool run is a separate, sandboxed process with
//! its own allowlist proxy, input/output schema checks, a hash check, limits and secret scrubbing.

use crate::manifest::Manifest;
use crate::{App, proxy, sandbox, schema, store, util};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum Backend {
    /// Handled inside the engine (scheduler, plugin-maker, memory-tools).
    Core,
    /// Rust tool code in this binary, run as `<exe> tool-run`.
    Rust,
    /// A script command declared in plugin.yaml.
    Script(Vec<String>),
}

pub const CORE_PLUGINS: &[&str] = &["scheduler", "calendar", "lists", "location", "health", "computers", "plugin-maker", "memory-tools", "spotify", "google-health", "hevy", "phone", "email"];

pub fn backend(app: &App, m: &Manifest) -> Option<Backend> {
    match m.runtime.builtin.as_deref() {
        Some("core") if CORE_PLUGINS.contains(&m.id.as_str()) => Some(Backend::Core),
        Some("rust") if app.registry.has(&m.id) => Some(Backend::Rust),
        Some(_) => None,
        None => m.runtime.command.clone().filter(|c| !c.is_empty()).map(Backend::Script),
    }
}

/// Expand `$AGENT_FILES_ROOT` and `$AGENT_TERM_CWD` in `needs.paths`.
fn rw_paths(app: &App, m: &Manifest) -> Vec<PathBuf> {
    m.needs
        .list("paths")
        .iter()
        .filter_map(|p| match p.as_str() {
            "$AGENT_FILES_ROOT" => Some(app.cfg.files_root.clone()),
            "$AGENT_TERM_CWD" => Some(app.cfg.term_cwd.clone()),
            s if s.starts_with('/') && !s.contains("..") => Some(PathBuf::from(s)),
            _ => None,
        })
        .collect()
}

/// Find each `needs.programs` entry on the host. Returns the folders to read from and put on PATH.
/// A program that is missing, or sits in the server's own data folder, is refused with a plain message.
pub fn resolve_programs(app: &App, m: &Manifest) -> Result<Vec<PathBuf>, String> {
    let mut search: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    search.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    let data = std::fs::canonicalize(&app.cfg.data_dir).unwrap_or_else(|_| app.cfg.data_dir.clone());
    let mut dirs: Vec<PathBuf> = vec![];
    for p in m.programs() {
        let found = if p.starts_with('/') { Some(PathBuf::from(&p)).filter(|f| f.is_file()) } else { search.iter().map(|d| d.join(&p)).find(|f| f.is_file()) };
        let f = found.ok_or_else(|| format!("tool_failed: the program '{p}' is not installed on this computer"))?;
        let canon = std::fs::canonicalize(&f).unwrap_or_else(|_| f.clone());
        if canon.starts_with(&data) {
            return Err(format!("tool_failed: the program '{p}' lives in the server's data folder"));
        }
        for d in [f.parent(), canon.parent()].into_iter().flatten() {
            if !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_path_buf());
            }
        }
    }
    Ok(dirs)
}

fn schema_file(m: &Manifest, tool: &str, dir: &str) -> Option<Value> {
    let decl = m.tool(tool)?;
    let _ = decl;
    let p = m.dir.join("schemas").join(format!("{tool}.{dir}.json"));
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok())
}

/// Run one tool. `m` may be a draft (then `allow` is usually `Allow::none()` and no hash check).
pub async fn run(app: &Arc<App>, m: &Manifest, tool: &str, args: Value, draft: bool, allow: Option<proxy::Allow>, kill: Arc<AtomicBool>) -> Result<Value, String> {
    let backend = backend(app, m).ok_or("this plugin has no runnable code")?;
    if backend == Backend::Core {
        return Err("core plugin tools are run by the engine".into());
    }
    if !draft {
        store::verify_hash(app, &m.id)?;
    }
    if let Some(s) = schema_file(m, tool, "in") {
        schema::validate(&s, &args).map_err(|e| format!("invalid_input: {e}"))?;
    }
    let prog_dirs = resolve_programs(app, m)?;
    let wall_secs = m.runtime.timeout_secs.unwrap_or(0).max(app.cfg.limits.tool_wall_secs).min(crate::manifest::MAX_TOOL_SECS.max(app.cfg.limits.tool_wall_secs));
    let allow = allow.unwrap_or_else(|| proxy::Allow { hosts: m.needs.network.clone(), get_only: false });
    let scratch = app.cfg.data_dir.join("run").join(util::random_hex(8));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    // Linux: no network in the sandbox, so the proxy listens on a socket file in the scratch folder.
    let sock = (app.cfg.sandbox == "linux" && !allow.hosts.is_empty()).then(|| scratch.join("proxy.sock"));
    let px = if allow.hosts.is_empty() {
        None
    } else if let Some(s) = &sock {
        Some(proxy::start_unix(allow, s).map_err(|e| e.to_string())?)
    } else {
        Some(proxy::start(allow).await.map_err(|e| e.to_string())?)
    };
    let port = px.as_ref().map(|p| p.port);
    let _ = std::fs::create_dir_all(m.dir.join("state"));
    let argv: Vec<String> = match &backend {
        Backend::Rust => vec![app.cfg.runner_exe.display().to_string(), "tool-run".into(), m.id.clone(), tool.into()],
        Backend::Script(c) => c.clone(),
        Backend::Core => unreachable!(),
    };
    let spec = sandbox::Spec {
        plugin_dir: m.dir.clone(),
        scratch: scratch.clone(),
        exe: app.cfg.runner_exe.clone(),
        rw_paths: rw_paths(app, m),
        proxy_port: port,
        proxy_sock: sock.clone(),
        apple_events: m.needs.list("apple_events"),
        ro_paths: prog_dirs.clone(),
        hide: {
            // The server's own data, plus key folders a wide work folder (the home) would expose.
            let mut h = vec![app.cfg.data_dir.clone()];
            if app.cfg.server_dir != app.cfg.data_dir {
                h.push(app.cfg.server_dir.clone()); // every account's folder, the account list
            }
            if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
                h.extend([".ssh", ".gnupg", ".aws", ".config/gh"].map(|d| home.join(d)));
            }
            h
        },
    };
    let mut cmd = sandbox::command(&app.cfg.sandbox, &spec, &argv).map_err(|e| format!("sandbox_denied: {e}"))?;
    cmd.env_clear()
        .current_dir(&m.dir)
        .env("PATH", {
            let mut p = String::from("/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin");
            for d in &prog_dirs {
                p.push(':');
                p.push_str(&d.display().to_string());
            }
            p
        })
        .env("HOME", &scratch)
        .env("TMPDIR", &scratch)
        .env("LANG", "en_US.UTF-8")
        .env("AGENT_FILES_ROOT", &app.cfg.files_root)
        .env("AGENT_TERM_ALLOW", app.cfg.term_allow.join(","))
        .env("AGENT_TERM_CWD", &app.cfg.term_cwd);
    if let Some(s) = &sock {
        cmd.env("AGENT_PROXY_SOCK", s);
    } else if let Some(p) = port {
        cmd.env("AGENT_PROXY", format!("http://127.0.0.1:{p}"));
    }
    // Secrets are injected by name only; their values never reach the model (docs/03).
    let mut secret_values = vec![];
    for name in &m.needs.secrets {
        if let Some(v) = store::secret_get(app, name) {
            cmd.env(format!("SECRET_{}", name.to_ascii_uppercase()), &v);
            secret_values.push(v);
        }
    }
    if let Some(k) = &app.cfg.model.api_key {
        secret_values.push(k.clone());
    }
    let limits = app.cfg.limits.clone();
    let input = json!({ "tool": tool, "args": args }).to_string();
    let res = tokio::task::spawn_blocking(move || -> Result<(String, String, bool), String> {
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("could not start tool: {e}"))?;
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(input.as_bytes());
        }
        let cap = limits.tool_output_bytes as u64;
        let drain = |r: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut b = vec![];
                if let Some(mut r) = r {
                    let _ = r.by_ref().take(cap).read_to_end(&mut b);
                    let _ = std::io::copy(&mut r, &mut std::io::sink());
                }
                String::from_utf8_lossy(&b).into_owned()
            })
        };
        let out = drain(child.stdout.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
        let err = drain(child.stderr.take().map(|x| Box::new(x) as Box<dyn Read + Send>));
        let t0 = Instant::now();
        let mut timed_out = false;
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            if kill.load(Ordering::SeqCst) || t0.elapsed() > Duration::from_secs(wall_secs) {
                timed_out = !kill.load(Ordering::SeqCst);
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok((out.join().unwrap_or_default(), err.join().unwrap_or_default(), timed_out))
    })
    .await
    .map_err(|e| e.to_string())?;
    drop(px);
    let _ = std::fs::remove_dir_all(&scratch);
    let (stdout, stderr, timed_out) = res?;
    if timed_out {
        return Err("tool_failed: timed out".into());
    }
    let line = stdout.lines().rev().find(|l| l.trim_start().starts_with('{')).unwrap_or("");
    let v: Value = serde_json::from_str(line).map_err(|_| {
        let e = util::scrub(&stderr, &secret_values);
        format!("tool_failed: no JSON result{}", if e.trim().is_empty() { String::new() } else { format!(" ({})", e.chars().take(300).collect::<String>()) })
    })?;
    if v["ok"] != true {
        return Err(util::scrub(v["error"].as_str().unwrap_or("tool failed"), &secret_values));
    }
    let result = v.get("result").cloned().unwrap_or(Value::Null);
    if let Some(s) = schema_file(m, tool, "out") {
        schema::validate(&s, &result).map_err(|e| format!("tool_failed: bad output: {e}"))?;
    }
    let clean = util::scrub(&result.to_string(), &secret_values);
    Ok(serde_json::from_str(&clean).unwrap_or(Value::String(clean)))
}
