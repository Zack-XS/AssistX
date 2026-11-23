//! `plugin.yaml` + `guide.md` loader (docs/03-plugins.md).
//!
//! Fields the core keeps itself (`status`, `provenance`, `created_by`) are ignored when read.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub const KNOWN_TAGS: &[&str] = &["reads", "writes", "sends", "spends", "deletes", "shares", "unrestricted", "always_ask"];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Needs {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<String>,
    /// Extra keys (paths, programs, platform) are kept as-is for the approval card.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolDecl {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Requested level: auto | ask | never. Missing = ask.
    #[serde(default)]
    pub autonomy: Option<String>,
    #[serde(default)]
    pub undo: Option<String>,
    #[serde(default)]
    pub cost: Option<serde_yaml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Runtime {
    /// `rust` (tool code in this binary) or `core` (handled by the engine, e.g. scheduler).
    #[serde(default)]
    pub builtin: Option<String>,
    /// Script tools: argv run in the plugin folder, inside the sandbox.
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub deps: Option<String>,
    /// Wall-time limit per tool run, in seconds (at most `MAX_TOOL_SECS`).
    /// Asking for more than the core's default is shown on the approval card.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Longest wall time a plugin may ask for per tool run.
pub const MAX_TOOL_SECS: u64 = 300;
/// Programs that run any command line: asking for one is shown as a warning on the card.
pub const SHELL_PROGRAMS: &[&str] = &["sh", "bash", "zsh", "dash", "fish", "ksh", "csh", "tcsh", "env", "xargs", "sudo", "su", "osascript", "python", "python3", "node", "perl", "ruby", "php", "lua", "deno", "bun"];

pub fn is_shell_program(p: &str) -> bool {
    let base = p.rsplit('/').next().unwrap_or(p);
    SHELL_PROGRAMS.contains(&base) || base.starts_with("python3.")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub requires_core: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub needs: Needs,
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
    #[serde(default)]
    pub runtime: Runtime,
    /// Words in the user's own message that load this plugin before the model's first step (saves a
    /// round trip for quick commands). Used only for plugins the user did not get from outside.
    #[serde(default)]
    pub words: Vec<String>,
    /// guide.md text, loaded on demand into the prompt.
    #[serde(skip)]
    pub guide: String,
    #[serde(skip)]
    pub dir: std::path::PathBuf,
}

impl Needs {
    /// A list-valued extra key such as `paths`, `programs`, `apple_events`.
    pub fn list(&self, key: &str) -> Vec<String> {
        self.other.get(key).and_then(|v| v.as_sequence()).map(|s| s.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    }
}

/// Load and check one plugin folder. `expect_id` must match `id`.
pub fn load_one(dir: &Path, expect_id: &str) -> Result<Manifest, String> {
    let yaml = std::fs::read_to_string(dir.join("plugin.yaml")).map_err(|e| format!("{expect_id}: plugin.yaml: {e}"))?;
    let mut m: Manifest = serde_yaml::from_str(&yaml).map_err(|e| format!("{expect_id}: {e}"))?;
    if m.id != expect_id {
        return Err(format!("{expect_id}: id '{}' does not match folder", m.id));
    }
    m.validate()?;
    if let Some(req) = &m.requires_core
        && !core_version_ok(req)
    {
        return Err(format!("{expect_id}: needs core {req}, this is {}", env!("CARGO_PKG_VERSION")));
    }
    m.guide = std::fs::read_to_string(dir.join("guide.md")).unwrap_or_default();
    m.dir = dir.to_path_buf();
    Ok(m)
}

/// Supports ">=x.y" and "x.y" (major.minor) checks against this core.
pub fn core_version_ok(req: &str) -> bool {
    let parse = |s: &str| -> Option<(u32, u32)> {
        let mut it = s.trim().split('.');
        Some((it.next()?.parse().ok()?, it.next().unwrap_or("0").parse().ok()?))
    };
    let Some(have) = parse(env!("CARGO_PKG_VERSION")) else { return false };
    match req.trim().strip_prefix(">=") {
        Some(v) => parse(v).is_some_and(|need| have >= need),
        None => parse(req).is_some_and(|need| have.0 == need.0 && have >= need),
    }
}

impl Manifest {
    /// Real programs the plugin asks to run (`needs.programs`): names found on the host or absolute paths.
    pub fn programs(&self) -> Vec<String> {
        self.needs.list("programs")
    }

    pub fn tool(&self, name: &str) -> Option<&ToolDecl> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Rules from docs/03 the core must enforce at install time.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty() && self.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(format!("bad plugin id '{}'", self.id));
        }
        for p in self.programs() {
            let name_ok = !p.is_empty() && p.len() <= 120 && !p.contains("..") && p.chars().all(|c| c.is_ascii_alphanumeric() || "._+-/".contains(c));
            let shape_ok = if p.contains('/') { p.starts_with('/') && !p.ends_with('/') } else { !p.starts_with('-') };
            if !name_ok || !shape_ok {
                return Err(format!("{}: bad program '{p}' in needs.programs (a plain name or an absolute path)", self.id));
            }
        }
        if let Some(t) = self.runtime.timeout_secs
            && !(1..=MAX_TOOL_SECS).contains(&t)
        {
            return Err(format!("{}: runtime.timeout_secs must be 1 to {MAX_TOOL_SECS}", self.id));
        }
        for t in &self.tools {
            if let Some(bad) = t.tags.iter().find(|g| !KNOWN_TAGS.contains(&g.as_str())) {
                return Err(format!("{}.{}: unknown tag '{bad}'", self.id, t.name));
            }
            if t.tags.iter().any(|g| g == "spends") && t.cost.is_none() {
                return Err(format!("{}.{}: a `spends` tool must declare `cost`", self.id, t.name));
            }
            if let Some(a) = &t.autonomy
                && !["auto", "ask", "never"].contains(&a.as_str())
            {
                return Err(format!("{}.{}: bad autonomy '{a}'", self.id, t.name));
            }
        }
        Ok(())
    }

    pub fn card(&self) -> Value {
        serde_json::json!({
            "id": self.id, "version": self.version, "summary": self.summary,
            "needs": self.needs, "tools": self.tools,
        })
    }
}

/// Load every live `plugins/<id>/plugin.yaml`. Bad ones are skipped with a reason.
pub fn load_all(dir: &Path) -> (BTreeMap<String, Manifest>, Vec<String>) {
    let (mut ok, mut problems) = (BTreeMap::new(), vec![]);
    let Ok(entries) = std::fs::read_dir(dir) else {
        problems.push(format!("plugins dir not found: {}", dir.display()));
        return (ok, problems);
    };
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') || !d.join("plugin.yaml").exists() {
            continue; // .drafts, .versions
        }
        match load_one(&d, &name) {
            Ok(m) => {
                ok.insert(m.id.clone(), m);
            }
            Err(e) => problems.push(e),
        }
    }
    (ok, problems)
}

#[cfg(test)]
mod programs_tests {
    use super::*;

    fn m(extra: &str) -> Result<Manifest, String> {
        let y = format!("id: demo\nversion: 0.1.0\nsummary: x\nneeds:\n  programs: {extra}\nruntime:\n  command: [\"true\"]\n  timeout_secs: 120\n");
        let m: Manifest = serde_yaml::from_str(&y).map_err(|e| e.to_string())?;
        m.validate().map(|_| m)
    }

    #[test]
    fn programs_are_checked() {
        assert_eq!(m("[git, /opt/homebrew/bin/ffmpeg]").unwrap().programs().len(), 2);
        assert!(m("[\"../x\"]").is_err());
        assert!(m("[\"rm -rf\"]").is_err());
        assert!(m("[\"-rf\"]").is_err());
        assert!(m("[\"bin/x\"]").is_err());
        assert!(is_shell_program("/bin/bash") && is_shell_program("python3.12") && !is_shell_program("git"));
    }

    #[test]
    fn timeout_is_capped() {
        let y = "id: demo\nversion: 0.1.0\nsummary: x\nruntime:\n  timeout_secs: 9999\n";
        assert!(serde_yaml::from_str::<Manifest>(y).unwrap().validate().is_err());
    }
}
