//! Terminal. Allowlisted programs only, no shell, 20 second limit.

use super::{ActionInfo, Plugin, ToolResult, info, str_arg};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_secs(20);
/// Just under the runner's 30 second wall, so the shell's own message wins.
const SHELL_LIMIT: Duration = Duration::from_secs(28);
const MAX_OUT: u64 = 20_000;
pub struct Terminal {
    allow: Vec<String>,
    cwd: String,
}

impl Terminal {
    pub fn new(allow: &[String], cwd: &std::path::Path) -> Self {
        Terminal { allow: allow.to_vec(), cwd: cwd.display().to_string() }
    }

    fn run(&self, args: &Value) -> ToolResult {
        let argv: Vec<&str> = str_arg(args, "command")?.split_whitespace().collect();
        let prog = *argv.first().ok_or("empty command")?;
        if !self.allow.iter().any(|a| a == prog) {
            return Err(format!("not allowed: {prog}. Use the shell tool instead (it asks the user with a warning).").into());
        }
        let mut c = Command::new(prog);
        c.args(&argv[1..]);
        self.spawn(c, LIMIT)
    }

    /// Any command line, through `/bin/sh -c`, in the work folder. The core always asks first
    /// with a loud warning (tag `unrestricted`), and the sandbox still applies.
    fn shell(&self, args: &Value) -> ToolResult {
        let cmd = str_arg(args, "command")?;
        if cmd.trim().is_empty() || cmd.len() > 4000 {
            return Err("command must be 1 to 4000 characters".into());
        }
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(cmd);
        self.spawn(c, SHELL_LIMIT)
    }

    fn spawn(&self, mut cmd: Command, limit: Duration) -> ToolResult {
        let mut child = cmd
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        // Drain pipes on threads so a chatty child cannot block on a full pipe.
        let drain = |mut r: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut b = vec![];
                let _ = r.by_ref().take(MAX_OUT).read_to_end(&mut b);
                let _ = std::io::copy(&mut r, &mut std::io::sink());
                String::from_utf8_lossy(&b).into_owned()
            })
        };
        let out = drain(Box::new(child.stdout.take().ok_or("no stdout")?));
        let err = drain(Box::new(child.stderr.take().ok_or("no stderr")?));
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break s;
            }
            if start.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timed out".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        Ok(json!({
            "code": status.code(),
            "stdout": out.join().unwrap_or_default(),
            "stderr": err.join().unwrap_or_default(),
        }))
    }
}

impl Plugin for Terminal {
    fn name(&self) -> &'static str {
        "terminal"
    }
    fn actions(&self) -> BTreeMap<&'static str, ActionInfo> {
        BTreeMap::from([
            ("run", info("Run an allowlisted command (no shell, no quotes)", &[("command", "e.g. 'ls -la'")])),
            ("shell", info("Run any command line through a shell; always asks the user with a warning", &[("command", "e.g. 'git status && ls | head'")])),
            ("allowed", info("List allowed programs", &[])),
        ])
    }
    fn call(&self, action: &str, args: &Value) -> Option<ToolResult> {
        Some(match action {
            "run" => self.run(args),
            "shell" => self.shell(args),
            "allowed" => Ok(json!({ "allowed": self.allow, "cwd": self.cwd })),
            _ => return None,
        })
    }
}
