//! Settings. Read from env vars at start; a few can be changed live with `PATCH /v1/config`
//! (stored in the `settings` table, see api.rs).

use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub port: u16,
    /// Address to listen on. Default 127.0.0.1. Anything else is opt-in (`AGENT_BIND`) and should sit behind a private tunnel.
    pub bind: String,
    /// Extra `Host` header values accepted besides 127.0.0.1:<port>, such as a tailnet name (`AGENT_ALLOWED_HOSTS`).
    pub allowed_hosts: Vec<String>,
    pub data_dir: PathBuf,
    /// The server's own folder: the app update files, the account list. Equal to `data_dir` unless accounts are on (docs/15),
    /// where each account has its own `data_dir` below it. Tools never see this folder.
    pub server_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub prompts_dir: PathBuf,
    /// Exact browser origins allowed to call the API. Empty = none.
    pub allowed_origins: Vec<String>,
    pub model: ModelConfig,
    pub limits: Limits,
    pub files_root: PathBuf,
    pub term_allow: Vec<String>,
    pub term_cwd: PathBuf,
    /// Binary that runs one tool inside the sandbox (`<exe> tool-run`). Defaults to this program.
    pub runner_exe: PathBuf,
    /// `macos` (sandbox-exec), or `off` (only with AGENT_UNSAFE_NO_SANDBOX=1).
    pub sandbox: String,
    /// IANA zone for schedules and quiet hours, e.g. "Europe/Berlin".
    pub timezone: String,
    /// "HH:MM-HH:MM" local time. Normal notices wait until it ends.
    pub quiet_hours: Option<(u32, u32)>,
    pub notices_per_day: u32,
    /// Local hour the nightly dreaming job runs.
    pub dreaming_hour: u32,
    /// Loading a `sensitive` entry for a cloud model: ask | never | allow (docs/04).
    pub sensitive_to_cloud: String,
    /// Run dreaming by itself when notes are waiting and the last run is older than this many minutes. 0 = off.
    pub dream_every_mins: u32,
    /// Memory switches: the recent-chats directory line, and the basic-facts directory line.
    pub memory_recent: bool,
    pub memory_core: bool,
    /// Spotify's web service and sign-in addresses. Only tests and mocks change them.
    pub spotify_base: String,
    pub oauth_base: String,
    /// OpenStreetMap Nominatim, for naming places (plugin `location`).
    pub geocode_base: String,
    /// Google sign-in, token and Health API addresses; Hevy API address. Only tests and mocks change them.
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_health_base: String,
    pub gmail_base: String,
    pub hevy_base: String,
    /// A text-to-speech service on this same computer (`AGENT_TTS_URL`, for example http://127.0.0.1:5002). Empty = off. Only a loopback address is ever used.
    pub tts_url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelConfig {
    /// `openai` (any OpenAI-compatible server) or `mock` (tests and demos).
    pub kind: String,
    pub base_url: String,
    pub name: String,
    /// Never serialized out (GET /v1/config hides secrets).
    #[serde(skip)]
    pub api_key: Option<String>,
}

impl ModelConfig {
    /// A model on this machine never sees data leave it.
    pub fn is_local(&self) -> bool {
        self.kind == "mock" || ["://127.", "://localhost", "://[::1]"].iter().any(|p| self.base_url.contains(p))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Limits {
    pub max_steps_per_turn: u32,
    pub max_tool_calls_per_turn: u32,
    pub max_tokens_per_turn: u64,
    pub max_tokens_per_day: u64,
    pub max_tokens_per_dreaming: u64,
    pub max_approvals_per_turn: u32,
    pub max_approvals_per_hour: u32,
    pub approval_wait_secs: u64,
    pub approval_expiry_secs: i64,
    pub event_retention_secs: i64,
    pub messages_per_minute: u32,
    pub triggers_per_minute: u32,
    pub history_messages: usize,
    /// Sum up the older part of a chat once what the model would see passes about this many tokens (0 = never).
    pub summary_at_tokens: usize,
    /// Newest messages kept word for word when a chat is summed up.
    pub summary_keep: usize,
    pub tool_wall_secs: u64,
    pub tool_cpu_secs: u64,
    pub tool_output_bytes: usize,
    pub build_fetches_per_minute: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_steps_per_turn: 25,
            max_tool_calls_per_turn: 20,
            max_tokens_per_turn: 200_000,
            max_tokens_per_day: 2_000_000,
            max_tokens_per_dreaming: 300_000,
            max_approvals_per_turn: 5,
            max_approvals_per_hour: 20,
            approval_wait_secs: 600,
            approval_expiry_secs: 24 * 3600,
            event_retention_secs: 7 * 24 * 3600,
            messages_per_minute: 60,
            triggers_per_minute: 10,
            history_messages: 40,
            summary_at_tokens: 12_000,
            summary_keep: 12,
            tool_wall_secs: 30,
            tool_cpu_secs: 20,
            tool_output_bytes: 200_000,
            build_fetches_per_minute: 20,
        }
    }
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

/// A secret from systemd's credentials folder (`LoadCredential=name:/path` in the unit), so keys need not sit in an
/// environment file next to the data (security review #9). `None` when not running that way or not set.
pub fn credential(name: &str) -> Option<String> {
    let dir = env("CREDENTIALS_DIRECTORY")?;
    std::fs::read_to_string(std::path::Path::new(&dir).join(name)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn list(k: &str, default: &str) -> Vec<String> {
    env(k).unwrap_or_else(|| default.into()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

pub const DEFAULT_TERM_ALLOW: &str = "ls,cat,pwd,echo,date,whoami,uname,df,du,head,tail,wc,grep,which";

/// "22:00-07:00" -> (1320, 420) minutes after midnight.
pub fn parse_quiet(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('-')?;
    let m = |t: &str| -> Option<u32> {
        let (h, mi) = t.trim().split_once(':')?;
        let (h, mi): (u32, u32) = (h.parse().ok()?, mi.parse().ok()?);
        (h < 24 && mi < 60).then_some(h * 60 + mi)
    };
    Some((m(a)?, m(b)?))
}

impl Config {
    pub fn from_env() -> Self {
        let home = PathBuf::from(env("HOME").unwrap_or_else(|| ".".into()));
        let mut limits = Limits::default();
        if let Some(v) = env("AGENT_APPROVAL_WAIT_SECS").and_then(|v| v.parse().ok()) {
            limits.approval_wait_secs = v;
        }
        // The daily token cap guards cloud spend. 0 = no cap (for example with only local models).
        if let Some(v) = env("AGENT_MAX_TOKENS_PER_DAY").and_then(|v| v.parse().ok()) {
            limits.max_tokens_per_day = v;
        }
        // Long chats are summed up (src/summary.rs). 0 = never.
        if let Some(v) = env("AGENT_SUMMARY_AT_TOKENS").and_then(|v| v.parse().ok()) {
            limits.summary_at_tokens = v;
        }
        let sandbox = if env("AGENT_UNSAFE_NO_SANDBOX").as_deref() == Some("1") {
            "off".to_string()
        } else if let Some(kind) = crate::sandbox::probe() {
            kind.to_string()
        } else {
            "unavailable".to_string()
        };
        Config {
            bind: env("AGENT_BIND").unwrap_or_else(|| "127.0.0.1".into()),
            allowed_hosts: list("AGENT_ALLOWED_HOSTS", ""),
            port: env("AGENT_PORT").and_then(|p| p.parse().ok()).unwrap_or(8787),
            data_dir: PathBuf::from(env("AGENT_DATA_DIR").unwrap_or_else(|| "data".into())),
            server_dir: PathBuf::from(env("AGENT_DATA_DIR").unwrap_or_else(|| "data".into())),
            plugins_dir: PathBuf::from(env("AGENT_PLUGINS_DIR").unwrap_or_else(|| "plugins".into())),
            prompts_dir: PathBuf::from(env("AGENT_PROMPTS_DIR").unwrap_or_else(|| "prompts".into())),
            allowed_origins: list("AGENT_ALLOWED_ORIGINS", ""),
            model: ModelConfig {
                kind: env("AGENT_MODEL_KIND").unwrap_or_else(|| "openai".into()),
                base_url: env("AGENT_MODEL_BASE_URL").unwrap_or_else(|| "http://127.0.0.1:11434/v1".into()),
                name: env("AGENT_MODEL_NAME").unwrap_or_else(|| "llama3.1".into()),
                api_key: env("AGENT_MODEL_API_KEY").or_else(|| credential("model_api_key")),
            },
            limits,
            files_root: env("AGENT_FILES_ROOT").map(PathBuf::from).unwrap_or_else(|| home.join("Documents")),
            term_allow: list("AGENT_TERM_ALLOW", DEFAULT_TERM_ALLOW),
            term_cwd: env("AGENT_TERM_CWD").map(PathBuf::from).unwrap_or_else(|| home.clone()),
            runner_exe: env("AGENT_RUNNER_EXE").map(PathBuf::from).or_else(|| std::env::current_exe().ok()).unwrap_or_default(),
            sandbox,
            timezone: env("AGENT_TIMEZONE").or_else(crate::tz::system_zone).unwrap_or_else(|| "UTC".into()),
            quiet_hours: parse_quiet(&env("AGENT_QUIET_HOURS").unwrap_or_else(|| "22:00-07:00".into())),
            notices_per_day: env("AGENT_NOTICES_PER_DAY").and_then(|v| v.parse().ok()).unwrap_or(10),
            dreaming_hour: env("AGENT_DREAMING_HOUR").and_then(|v| v.parse().ok()).filter(|h| *h < 24).unwrap_or(3),
            sensitive_to_cloud: env("AGENT_SENSITIVE_TO_CLOUD").filter(|v| ["ask", "never", "allow"].contains(&v.as_str())).unwrap_or_else(|| "ask".into()),
            dream_every_mins: env("AGENT_DREAM_EVERY_MINS").and_then(|v| v.parse().ok()).unwrap_or(120),
            memory_recent: env("AGENT_MEMORY_RECENT").as_deref() != Some("off"),
            memory_core: env("AGENT_MEMORY_CORE").as_deref() != Some("off"),
            spotify_base: env("AGENT_SPOTIFY_API").unwrap_or_else(|| "https://api.spotify.com".into()),
            oauth_base: env("AGENT_OAUTH_BASE").unwrap_or_else(|| "https://accounts.spotify.com".into()),
            geocode_base: env("AGENT_GEOCODE_BASE").unwrap_or_else(|| "https://nominatim.openstreetmap.org".into()),
            google_auth_url: env("AGENT_GOOGLE_AUTH_URL").unwrap_or_else(|| "https://accounts.google.com/o/oauth2/v2/auth".into()),
            google_token_url: env("AGENT_GOOGLE_TOKEN_URL").unwrap_or_else(|| "https://oauth2.googleapis.com/token".into()),
            google_health_base: env("AGENT_GOOGLE_HEALTH_API").unwrap_or_else(|| "https://health.googleapis.com".into()),
            gmail_base: env("AGENT_GMAIL_API").unwrap_or_else(|| "https://gmail.googleapis.com".into()),
            hevy_base: env("AGENT_HEVY_API").unwrap_or_else(|| "https://api.hevyapp.com".into()),
            tts_url: env("AGENT_TTS_URL").unwrap_or_default(),
        }
    }

    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// Where the server listens.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind, self.port)
    }

    /// Is this `Host` header one we answer to? Exact match with 127.0.0.1:<port>, or with an
    /// allowed host (with or without the port). This is what stops DNS rebinding.
    pub fn host_ok(&self, header: &str) -> bool {
        header == self.host()
            || self.allowed_hosts.iter().any(|a| {
                header == a || (!a.contains(':') && header.rsplit_once(':').is_some_and(|(name, port)| name == a && port == self.port.to_string()))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(hosts: &[&str]) -> Config {
        let mut c = Config::from_env();
        c.port = 8787;
        c.allowed_hosts = hosts.iter().map(|s| s.to_string()).collect();
        c
    }

    #[test]
    fn host_header_must_be_local_or_listed() {
        let c = cfg(&["phone-mac.tail1234.ts.net", "100.64.0.7:8787"]);
        assert!(c.host_ok("127.0.0.1:8787"));
        assert!(c.host_ok("phone-mac.tail1234.ts.net:8787") && c.host_ok("phone-mac.tail1234.ts.net"));
        assert!(c.host_ok("100.64.0.7:8787"));
        assert!(!c.host_ok("evil.example:8787") && !c.host_ok("phone-mac.tail1234.ts.net:9999") && !c.host_ok("localhost:8787"));
    }
}
