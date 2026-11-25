//! private-data-check: scans every file git would commit for private data.
//!
//! Run from anywhere in the repo:
//!   cargo run --manifest-path tools/private-data-check/Cargo.toml
//!
//! Exit code 0 = clean, 1 = findings, 2 = could not run.
//! Rules: docs/security/private-data.md. Allow list: `.private-data-allow`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

#[derive(Debug, PartialEq)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub kind: &'static str,
    pub text: String,
}

/// File names that must never be committed, whatever they hold.
const BLOCKED_NAMES: &[&str] = &[".env", ".token", "auth.json", "owner.token", "db.key", "local.properties"];
const BLOCKED_SUFFIXES: &[&str] = &[".keystore", ".jks", ".pem", ".p12", ".key", ".token", ".db", ".sqlite"];
const BLOCKED_DIRS: &[&str] = &["server/data/", "data/", "server/docs-retired/"];

/// Token shapes from common providers.
const TOKEN_PREFIXES: &[(&str, usize, &str)] = &[
    ("sk-", 20, "api key (sk-)"),
    ("ghp_", 30, "GitHub token"),
    ("gho_", 30, "GitHub token"),
    ("github_pat_", 30, "GitHub token"),
    ("xoxb-", 20, "Slack token"),
    ("xoxp-", 20, "Slack token"),
    ("AKIA", 16, "AWS key id"),
    ("AIza", 35, "Google API key"),
    ("nvapi-", 30, "NVIDIA API key"),
];

/// Email domains that are fine to show.
const SAFE_EMAIL_DOMAINS: &[&str] = &[
    "example.com", "example.org", "example.net", "example", "test", "localhost",
    "users.noreply.github.com", "noreply.github.com", "anthropic.com",
];

fn main() -> ExitCode {
    let root = match repo_root() {
        Some(r) => r,
        None => {
            eprintln!("private-data-check: not inside a git repo");
            return ExitCode::from(2);
        }
    };
    let files = match candidate_files(&root) {
        Some(f) => f,
        None => {
            eprintln!("private-data-check: git ls-files failed");
            return ExitCode::from(2);
        }
    };
    let allow = std::fs::read_to_string(root.join(".private-data-allow")).unwrap_or_default();
    let allow = parse_allow(&allow);

    let mut findings = Vec::new();
    for rel in &files {
        if rel == ".private-data-allow" {
            continue; // it quotes the allowed values
        }
        if let Some(kind) = blocked_file(rel) {
            findings.push(Finding { path: rel.clone(), line: 0, kind, text: String::new() });
            continue;
        }
        let Ok(bytes) = std::fs::read(root.join(rel)) else { continue };
        if bytes.contains(&0) {
            continue; // binary
        }
        let text = String::from_utf8_lossy(&bytes);
        findings.extend(scan_text(rel, &text));
    }
    findings.retain(|f| !is_allowed(&allow, f));

    eprintln!("private-data-check: {} files scanned, {} findings", files.len(), findings.len());
    for f in &findings {
        if f.line == 0 {
            println!("{}: {}", f.path, f.kind);
        } else {
            println!("{}:{}: {}: {}", f.path, f.line, f.kind, f.text);
        }
    }
    if findings.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn repo_root() -> Option<PathBuf> {
    let out = Command::new("git").args(["rev-parse", "--show-toplevel"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// Tracked files plus new files git would pick up (not ignored).
fn candidate_files(root: &Path) -> Option<Vec<String>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut v: Vec<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .filter(|p| root.join(p).is_file())
        .collect();
    v.sort();
    v.dedup();
    Some(v)
}

pub fn blocked_file(rel: &str) -> Option<&'static str> {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if BLOCKED_DIRS.iter().any(|d| rel.starts_with(d)) {
        return Some("private folder (never commit)");
    }
    if BLOCKED_NAMES.contains(&name) || name.starts_with(".env.") {
        return Some("private file (never commit)");
    }
    if BLOCKED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return Some("key, token or database file (never commit)");
    }
    None
}

pub fn scan_text(path: &str, text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let mut push = |kind: &'static str, t: &str| {
            out.push(Finding { path: path.to_string(), line: n, kind, text: t.to_string() });
        };
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            push("private key", "-----BEGIN ... PRIVATE KEY");
        }
        // SVG path data packs numbers like "3.5.7.7"; those are shapes, not IPs.
        let svg_path = path.ends_with(".svg") || line.contains(" d=\"") || line.contains("<path");
        for ip in if svg_path { Vec::new() } else { find_ipv4(line) } {
            if let Some(kind) = ip_kind(&ip) {
                push(kind, &ip);
            }
        }
        for email in find_emails(line) {
            push("email address", &email);
        }
        for (tok, kind) in find_tokens(line) {
            push(kind, &tok);
        }
    }
    out
}

/// Returns a finding kind for an IP that must not be committed.
pub fn ip_kind(ip: &str) -> Option<&'static str> {
    let o: Vec<u8> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
    if o.len() != 4 {
        return None;
    }
    let safe = o[0] == 0
        || o[0] == 127
        || o[0] == 10
        || (o[0] == 192 && o[1] == 168)
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 169 && o[1] == 254)
        || (o[0] == 192 && o[1] == 0 && o[2] == 2) // docs range
        || (o[0] == 198 && o[1] == 51 && o[2] == 100) // docs range
        || (o[0] == 203 && o[1] == 0 && o[2] == 113) // docs range
        || o[0] >= 224 // multicast, reserved, broadcast
        || o == [1, 1, 1, 1]
        || o == [8, 8, 8, 8];
    if safe {
        return None;
    }
    if o[0] == 100 && (64..=127).contains(&o[1]) {
        return Some("Tailscale IP");
    }
    Some("public IP address")
}

/// Dotted quads with each part 0-255, not part of a longer number or version.
pub fn find_ipv4(line: &str) -> Vec<String> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'.' || b[i - 1] == b'-' || b[i - 1] == b'_')) {
            let start = i;
            let mut parts = 0;
            let mut ok = true;
            loop {
                let ps = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                let len = i - ps;
                if len == 0 || len > 3 || line[ps..i].parse::<u16>().map_or(true, |v| v > 255) {
                    ok = false;
                    break;
                }
                parts += 1;
                if parts == 4 || i >= b.len() || b[i] != b'.' {
                    break;
                }
                i += 1;
            }
            let trailing_bad = i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || (b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()));
            if ok && parts == 4 && !trailing_bad {
                out.push(line[start..i].to_string());
            }
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

pub fn find_emails(line: &str) -> Vec<String> {
    let is_local = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let is_dom = |c: char| c.is_ascii_alphanumeric() || ".-".contains(c);
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut out = Vec::new();
    for (k, &(at, c)) in chars.iter().enumerate() {
        if c != '@' {
            continue;
        }
        let mut s = k;
        while s > 0 && is_local(chars[s - 1].1) {
            s -= 1;
        }
        let mut e = k + 1;
        while e < chars.len() && is_dom(chars[e].1) {
            e += 1;
        }
        if s == k || e == k + 1 {
            continue;
        }
        let local = &line[chars[s].0..at];
        if ["this", "super", "return", "break", "continue"].contains(&local) {
            continue; // Kotlin label, like this@MainActivity
        }
        let end = if e < chars.len() { chars[e].0 } else { line.len() };
        let domain = line[at + 1..end].trim_end_matches('.').to_ascii_lowercase();
        if !domain.contains('.') || domain.split('.').last().map_or(true, |t| t.len() < 2 || !t.chars().all(|c| c.is_ascii_alphabetic())) {
            continue; // not a real domain: npm scope, decorator, version
        }
        if SAFE_EMAIL_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}"))) {
            continue;
        }
        out.push(format!("{local}@{domain}"));
    }
    out
}

pub fn find_tokens(line: &str) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for &(prefix, min_len, kind) in TOKEN_PREFIXES {
        let mut from = 0;
        while let Some(p) = line[from..].find(prefix) {
            let s = from + p;
            let before_ok = s == 0 || !line.as_bytes()[s - 1].is_ascii_alphanumeric();
            let e = line[s..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .map_or(line.len(), |x| s + x);
            let tok = &line[s..e];
            let body = &tok[prefix.len()..];
            // Real tokens are long and mix letters and digits.
            if before_ok
                && tok.len() >= min_len
                && body.chars().any(|c| c.is_ascii_digit())
                && body.chars().any(|c| c.is_ascii_alphabetic())
                && !body.chars().all(|c| c == 'x' || c == 'X' || c == '-' || c == '_')
            {
                out.push((format!("{}…", &tok[..prefix.len() + 4.min(body.len())]), kind));
            }
            from = e.max(s + 1);
        }
    }
    out
}

/// Allow list lines: `path` (whole file) or `path: text` (one value in that file).
/// Lines starting with `#` are comments. Every entry needs a reason comment above it.
pub fn parse_allow(s: &str) -> Vec<(String, Option<String>)> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| match l.split_once(": ") {
            Some((p, t)) => (p.trim().to_string(), Some(t.trim().to_string())),
            None => (l.to_string(), None),
        })
        .collect()
}

fn is_allowed(allow: &[(String, Option<String>)], f: &Finding) -> bool {
    allow.iter().any(|(p, t)| {
        let path_ok = if let Some(dir) = p.strip_suffix('/') { f.path.starts_with(&format!("{dir}/")) } else { &f.path == p };
        path_ok && t.as_ref().map_or(true, |t| f.text.contains(t.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_public_and_tailscale_ips() {
        assert_eq!(find_ipv4("ssh root@45.12.88.7 now"), vec!["45.12.88.7"]);
        assert_eq!(ip_kind("45.12.88.7"), Some("public IP address"));
        assert_eq!(ip_kind("100.101.5.9"), Some("Tailscale IP"));
    }

    #[test]
    fn skips_local_doc_and_version_numbers() {
        for ip in ["127.0.0.1", "10.0.2.2", "192.168.1.4", "172.20.0.1", "192.0.2.10", "0.0.0.0"] {
            assert_eq!(ip_kind(ip), None, "{ip}");
        }
        assert!(find_ipv4("version 0.1.462673").is_empty());
        assert!(find_ipv4("v1.2.3.4.5").is_empty());
        assert!(find_ipv4("lib-1.2.3.4").is_empty());
        assert!(find_ipv4("999.1.1.1").is_empty());
    }

    #[test]
    fn finds_emails_but_not_safe_ones() {
        assert_eq!(find_emails("mail me: jo.doe@gmail.com."), vec!["jo.doe@gmail.com"]);
        assert!(find_emails("user@example.com").is_empty());
        assert!(find_emails("123+bot@users.noreply.github.com").is_empty());
        assert!(find_emails("npm i @types/node").is_empty());
        assert!(find_emails("#[derive] @Composable fun").is_empty());
        assert!(find_emails("this@MainActivity.haptic()").is_empty());
    }

    #[test]
    fn finds_tokens_not_placeholders() {
        assert_eq!(find_tokens("key=ghp_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7").len(), 1);
        assert_eq!(find_tokens("AKIAIOSFODNN7EXAMPLE").len(), 1);
        assert!(find_tokens("sk-xxxxxxxxxxxxxxxxxxxxxxxx").is_empty());
        assert!(find_tokens("task-runner-thing").is_empty());
        assert!(find_tokens("disk-usage-report-for-today").is_empty());
    }

    #[test]
    fn finds_private_keys() {
        let f = scan_text("a.txt", "x\n-----BEGIN OPENSSH PRIVATE KEY-----\n");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].line, 2);
    }

    #[test]
    fn blocks_private_files() {
        assert!(blocked_file("server/data/core.db").is_some());
        assert!(blocked_file("server/.token").is_some());
        assert!(blocked_file("android/release.keystore").is_some());
        assert!(blocked_file(".env.local").is_some());
        assert!(blocked_file("server/src/auth.rs").is_none());
        assert!(blocked_file("docs/design/tokens.md").is_none());
    }

    #[test]
    fn allow_list_matches_path_and_text() {
        let allow = parse_allow("# reason\nserver/src/redact.rs\ndocs/a.md: 45.12.88.7\nserver/tests/fixtures/\n");
        let f = |p: &str, t: &str| Finding { path: p.into(), line: 1, kind: "x", text: t.into() };
        assert!(is_allowed(&allow, &f("server/src/redact.rs", "anything")));
        assert!(is_allowed(&allow, &f("docs/a.md", "45.12.88.7")));
        assert!(!is_allowed(&allow, &f("docs/a.md", "45.12.88.8")));
        assert!(is_allowed(&allow, &f("server/tests/fixtures/k.txt", "x")));
        assert!(!is_allowed(&allow, &f("server/tests/other.rs", "x")));
    }
}
