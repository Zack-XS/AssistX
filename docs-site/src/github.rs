//! GitHub activity for the home page: contributors, releases, pull requests
//! and recent pushes.
//!
//! `fetch` asks the `gh` CLI (already signed in on the dev machine or in CI)
//! and writes `docs-site/github.json`. `build` only reads that file, so the
//! built site holds no token and works offline. No HTTP crate: the all-Rust
//! rule is kept, and `gh` does the network part.

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

/// The public repo the home page talks about.
pub const REPO: &str = "Zack-XS/AssistX";
pub const REPO_URL: &str = "https://github.com/Zack-XS/AssistX";

#[derive(Default)]
pub struct Data {
    pub fetched_at: String,
    pub contributors: Vec<Contributor>,
    pub releases: Vec<Release>,
    pub pulls: Vec<Pull>,
    pub commits: Vec<Commit>,
}

pub struct Contributor {
    pub login: String,
    pub avatar: String,
    pub url: String,
    pub commits: u64,
}

pub struct Release {
    pub tag: String,
    pub name: String,
    pub date: String,
    pub url: String,
    pub notes: Vec<String>,
    pub apk: Option<String>,
}

pub struct Pull {
    pub number: u64,
    pub title: String,
    pub state: String, // merged | open | closed
    pub user: String,
    pub date: String,
    pub url: String,
}

pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub url: String,
}

fn gh(path: &str) -> Result<Value, String> {
    let out = Command::new("gh")
        .args(["api", "-H", "Accept: application/vnd.github+json", path])
        .output()
        .map_err(|e| format!("could not run gh ({e}); install the GitHub CLI and run `gh auth login`"))?;
    if !out.status.success() {
        return Err(format!("gh api {path}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("gh api {path}: bad JSON: {e}"))
}

/// `fetch` subcommand: ask GitHub, keep only the fields the page shows.
pub fn fetch(out_file: &Path) -> Result<String, String> {
    let contributors = gh(&format!("repos/{REPO}/contributors?per_page=50"))?;
    let releases = gh(&format!("repos/{REPO}/releases?per_page=10"))?;
    let pulls = gh(&format!("repos/{REPO}/pulls?state=all&per_page=15"))?;
    let commits = gh(&format!("repos/{REPO}/commits?per_page=8"))?;

    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let arr = |v: &Value| v.as_array().cloned().unwrap_or_default();

    let contributors: Vec<Value> = arr(&contributors)
        .iter()
        .filter(|c| s(c, "type") != "Bot")
        .map(|c| json!({"login": s(c, "login"), "avatar": s(c, "avatar_url"), "url": s(c, "html_url"), "commits": c.get("contributions").and_then(Value::as_u64).unwrap_or(0)}))
        .collect();
    let releases: Vec<Value> = arr(&releases)
        .iter()
        .filter(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false))
        .map(|r| {
            let apk = arr(r.get("assets").unwrap_or(&Value::Null))
                .iter()
                .find(|a| s(a, "name").ends_with(".apk"))
                .map(|a| s(a, "browser_download_url"));
            json!({"tag": s(r, "tag_name"), "name": s(r, "name"), "date": s(r, "published_at"), "url": s(r, "html_url"), "body": s(r, "body"), "apk": apk})
        })
        .collect();
    let pulls: Vec<Value> = arr(&pulls)
        .iter()
        .map(|p| {
            let merged = p.get("merged_at").map(|m| !m.is_null()).unwrap_or(false);
            let state = if merged { "merged".to_string() } else { s(p, "state") };
            let user = p.get("user").map(|u| s(u, "login")).unwrap_or_default();
            json!({"number": p.get("number").and_then(Value::as_u64).unwrap_or(0), "title": s(p, "title"), "state": state, "user": user, "date": s(p, "updated_at"), "url": s(p, "html_url")})
        })
        .collect();
    let commits: Vec<Value> = arr(&commits)
        .iter()
        .map(|c| {
            let commit = c.get("commit").cloned().unwrap_or(Value::Null);
            let author = c
                .get("author")
                .filter(|a| !a.is_null())
                .map(|a| s(a, "login"))
                .unwrap_or_else(|| commit.get("author").map(|a| s(a, "name")).unwrap_or_default());
            let date = commit.get("author").map(|a| s(a, "date")).unwrap_or_default();
            json!({"sha": s(c, "sha"), "message": s(&commit, "message"), "author": author, "date": date, "url": s(c, "html_url")})
        })
        .collect();

    let fetched_at = Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M UTC"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let doc = json!({"repo": REPO, "fetched_at": fetched_at, "contributors": contributors, "releases": releases, "pulls": pulls, "commits": commits});
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(out_file, text).map_err(|e| format!("write {}: {e}", out_file.display()))?;
    Ok(format!(
        "{} contributors, {} releases, {} pull requests, {} commits",
        contributors.len(),
        releases.len(),
        pulls.len(),
        commits.len()
    ))
}

/// Only GitHub links and avatars are allowed onto the page.
fn safe_url(u: &str) -> bool {
    u.starts_with("https://github.com/") || u.starts_with("https://avatars.githubusercontent.com/")
}

/// Read `github.json`. Missing or bad file gives empty data (the page shows empty states).
pub fn load(file: &Path) -> Data {
    let Ok(text) = std::fs::read_to_string(file) else { return Data::default() };
    parse(&text)
}

pub fn parse(text: &str) -> Data {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Data::default() };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let arr = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let day = |d: String| d.chars().take(10).collect::<String>();

    let mut contributors: Vec<Contributor> = arr("contributors")
        .iter()
        .map(|c| Contributor {
            login: s(c, "login"),
            avatar: s(c, "avatar"),
            url: s(c, "url"),
            commits: c.get("commits").and_then(Value::as_u64).unwrap_or(0),
        })
        .filter(|c| !c.login.is_empty() && safe_url(&c.url))
        .collect();
    contributors.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.login.cmp(&b.login)));

    let releases = arr("releases")
        .iter()
        .map(|r| Release {
            tag: s(r, "tag"),
            name: s(r, "name"),
            date: day(s(r, "date")),
            url: s(r, "url"),
            notes: s(r, "body")
                .lines()
                .map(|l| l.trim().trim_start_matches(['-', '*']).trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .take(4)
                .collect(),
            apk: r.get("apk").and_then(Value::as_str).filter(|u| safe_url(u)).map(str::to_string),
        })
        .filter(|r| safe_url(&r.url))
        .collect();

    let pulls = arr("pulls")
        .iter()
        .map(|p| Pull {
            number: p.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: s(p, "title"),
            state: s(p, "state"),
            user: s(p, "user"),
            date: day(s(p, "date")),
            url: s(p, "url"),
        })
        .filter(|p| safe_url(&p.url))
        .collect();

    let commits = arr("commits")
        .iter()
        .map(|c| Commit {
            sha: s(c, "sha").chars().take(7).collect(),
            message: s(c, "message").lines().next().unwrap_or("").to_string(),
            author: s(c, "author"),
            date: day(s(c, "date")),
            url: s(c, "url"),
        })
        .filter(|c| safe_url(&c.url))
        .collect();

    Data { fetched_at: s(&v, "fetched_at"), contributors, releases, pulls, commits }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_sorts_and_drops_unsafe_links() {
        let j = r###"{"fetched_at":"2026-11-25 20:00 UTC",
          "contributors":[{"login":"a","avatar":"https://avatars.githubusercontent.com/u/1","url":"https://github.com/a","commits":3},
                          {"login":"b","avatar":"","url":"https://github.com/b","commits":9},
                          {"login":"evil","avatar":"","url":"javascript:alert(1)","commits":99}],
          "releases":[{"tag":"v1","name":"One","date":"2026-11-20T10:00:00Z","url":"https://github.com/x/releases/v1","body":"## Changes\n- Fix a\n- Add b","apk":"https://github.com/x/app.apk"}],
          "pulls":[{"number":4,"title":"feat: x","state":"merged","user":"a","date":"2026-11-21T00:00:00Z","url":"https://github.com/x/pull/4"}],
          "commits":[{"sha":"abcdef123","message":"first line\nmore","author":"a","date":"2026-11-22T00:00:00Z","url":"https://github.com/x/commit/abc"}]}"###;
        let d = parse(j);
        assert_eq!(d.contributors.len(), 2);
        assert_eq!(d.contributors[0].login, "b");
        assert_eq!(d.releases[0].notes, vec!["Fix a", "Add b"]);
        assert_eq!(d.releases[0].date, "2026-11-20");
        assert!(d.releases[0].apk.is_some());
        assert_eq!(d.pulls[0].state, "merged");
        assert_eq!(d.commits[0].sha, "abcdef1");
        assert_eq!(d.commits[0].message, "first line");
    }

    #[test]
    fn bad_or_missing_file_is_empty() {
        assert!(parse("not json").contributors.is_empty());
        assert!(load(Path::new("/nope/github.json")).releases.is_empty());
    }
}
