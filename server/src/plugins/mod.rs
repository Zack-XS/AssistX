//! Plugin registry. A plugin is a named group of tools that take and return JSON (see docs/03-plugins.md).

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub mod files;
pub mod terminal;
pub mod weather;
pub mod web;

/// User-facing failure (HTTP 400).
#[derive(Debug)]
pub struct ToolError(pub String);

impl<S: Into<String>> From<S> for ToolError {
    fn from(s: S) -> Self {
        ToolError(s.into())
    }
}

pub type ToolResult = Result<Value, ToolError>;

#[derive(Serialize)]
pub struct ActionInfo {
    pub desc: &'static str,
    /// argument name -> short description
    pub params: BTreeMap<&'static str, &'static str>,
}

pub fn info(desc: &'static str, params: &[(&'static str, &'static str)]) -> ActionInfo {
    ActionInfo { desc, params: params.iter().copied().collect() }
}

pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn actions(&self) -> BTreeMap<&'static str, ActionInfo>;
    /// `None` means the action does not exist (HTTP 404).
    fn call(&self, action: &str, args: &Value) -> Option<ToolResult>;
}

pub struct Registry {
    plugins: BTreeMap<&'static str, Box<dyn Plugin>>,
}

impl Registry {
    /// Built-in Rust tool code. A plugin is usable only if it also has a valid `plugins/<id>/plugin.yaml`.
    pub fn new(cfg: &crate::config::Config) -> Self {
        let all: Vec<Box<dyn Plugin>> = vec![
            Box::new(files::Files::new(&cfg.files_root)),
            Box::new(terminal::Terminal::new(&cfg.term_allow, &cfg.term_cwd)),
            Box::new(web::Web),
            Box::new(weather::Weather),
        ];
        Registry { plugins: all.into_iter().map(|s| (s.name(), s)).collect() }
    }

    pub fn has(&self, plugin: &str) -> bool {
        self.plugins.contains_key(plugin)
    }

    pub fn actions(&self, plugin: &str) -> BTreeMap<&'static str, ActionInfo> {
        self.plugins.get(plugin).map(|p| p.actions()).unwrap_or_default()
    }

    pub fn call(&self, plugin: &str, tool: &str, args: &Value) -> Option<ToolResult> {
        self.plugins.get(plugin)?.call(tool, args)
    }
}

// ---- argument helpers shared by plugins ----

/// HTTP client for plugin code. Inside a sandboxed run, `AGENT_PROXY` points at the run's
/// allowlist proxy and is the only network route.
static BRIDGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The run's proxy address: the env var (macOS) or the local bridge (Linux).
pub fn proxy_url() -> Option<String> {
    BRIDGE.get().cloned().or_else(|| std::env::var("AGENT_PROXY").ok())
}

/// Linux sandbox: no network, but a socket file reaches the run's proxy. Listen on loopback inside
/// the sandbox and pass each connection to that socket. Returns the proxy URL.
#[cfg(unix)]
pub fn start_bridge(sock: &str) -> std::io::Result<String> {
    use std::os::unix::net::UnixStream;
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://127.0.0.1:{}", l.local_addr()?.port());
    let sock = sock.to_string();
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let Ok(u) = UnixStream::connect(&sock) else { continue };
            let (Ok(mut c2), Ok(mut u2)) = (c.try_clone(), u.try_clone()) else { continue };
            let (mut c, mut u) = (c, u);
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut c2, &mut u2);
                let _ = u2.shutdown(std::net::Shutdown::Write);
            });
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut u, &mut c);
                let _ = c.shutdown(std::net::Shutdown::Write);
            });
        }
    });
    Ok(url)
}

pub fn http_agent(timeout_secs: u64, max_redirects: u32) -> ureq::Agent {
    let proxy = proxy_url().and_then(|p| ureq::Proxy::new(&p).ok());
    ureq::Agent::config_builder()
        .max_redirects(max_redirects)
        .timeout_global(Some(std::time::Duration::from_secs(timeout_secs)))
        .proxy(proxy)
        .build()
        .into()
}

/// Entry point of `agent-core tool-run <plugin> <tool>`: the child process the runner starts
/// inside the sandbox. Reads `{"args": ...}` on stdin, writes `{"ok":..}` on stdout.
pub fn tool_run_main(plugin: &str, tool: &str) -> i32 {
    use std::io::Read;
    #[cfg(unix)]
    if let Ok(sock) = std::env::var("AGENT_PROXY_SOCK") {
        match start_bridge(&sock) {
            Ok(u) => {
                let _ = BRIDGE.set(u);
            }
            Err(e) => {
                println!("{}", serde_json::json!({ "ok": false, "error": format!("no network route: {e}") }));
                return 0;
            }
        }
    }
    let mut input = String::new();
    let _ = std::io::stdin().take(10_000_000).read_to_string(&mut input);
    let args = serde_json::from_str::<Value>(&input).ok().and_then(|v| v.get("args").cloned()).unwrap_or(Value::Null);
    let reg = Registry::new(&crate::config::Config::from_env());
    let out = match reg.call(plugin, tool, &args) {
        Some(Ok(v)) => serde_json::json!({ "ok": true, "result": v }),
        Some(Err(e)) => serde_json::json!({ "ok": false, "error": e.0 }),
        None => serde_json::json!({ "ok": false, "error": "no such tool" }),
    };
    println!("{out}");
    0
}

pub fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError(format!("missing string arg '{key}'")))
}

pub fn opt_str<'a>(args: &'a Value, key: &str, default: &'a str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or(default)
}
