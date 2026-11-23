//! `check` subcommand: the five checks from the plan.
//!   1. missing front matter (title/description/order all present)
//!   2. front-matter `title` does not match the first `#` heading
//!   3. a page not listed in its folder's `README.md`
//!   4. a broken relative link (target file missing on disk)
//!   5. likely secrets (private IPs + leaked credential prefixes)
//!
//! No `regex` crate: scanners are hand-written to keep deps small.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::frontmatter;
use crate::markdown;
use crate::util;

/// One problem found by a check.
#[derive(Debug)]
pub struct Issue {
    pub file: String,
    pub problem: String,
}

/// Run all checks over `docs_dir`. `repo_root` is the repo root (docs/ sits in it).
/// Returns the list of issues found; empty means OK.
pub fn run(docs_dir: &Path, repo_root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Collect every .md file (sorted for stable output).
    let mut files: Vec<PathBuf> = Vec::new();
    collect_md(docs_dir, &mut files);
    files.sort();

    // repo-relative path for a file (e.g. "docs/plugins/calendar.md")
    let repo_rel = |p: &Path| -> String {
        p.strip_prefix(repo_root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| p.to_string_lossy().to_string())
    };

    // (repo_rel, fm, body) for every page that has front matter.
    let mut pages: Vec<(String, frontmatter::FrontMatter, String)> = Vec::new();
    let mut raw: Vec<(String, String)> = Vec::new(); // (repo_rel, full raw text)

    for f in &files {
        let rel = repo_rel(f);
        let text = fs::read_to_string(f).unwrap_or_default();
        raw.push((rel.clone(), text.clone()));
        let (fm_str, body) = frontmatter::split(&text);
        match fm_str.and_then(frontmatter::parse) {
            Some(fm) => pages.push((rel, fm, body.to_string())),
            None => issues.push(Issue {
                file: rel,
                problem: "missing or incomplete front matter (need title, description, order)"
                    .to_string(),
            }),
        }
    }

    // Group pages by folder (repo-relative directory).
    let mut by_folder: std::collections::BTreeMap<String, Vec<(String, frontmatter::FrontMatter, String)>> =
        std::collections::BTreeMap::new();
    for (rel, fm, body) in &pages {
        let folder = parent_dir(rel);
        by_folder.entry(folder).or_default().push((rel.clone(), fm.clone(), body.clone()));
    }

    // Check 2: title matches the first H1. Check 4: broken links. Check 5: secrets.
    for (rel, fm, body) in &pages {
        // 2. title vs heading
        match markdown::first_h1(body) {
            Some(h1) => {
                if h1.trim() != fm.title.trim() {
                    issues.push(Issue {
                        file: rel.clone(),
                        problem: format!(
                            "title {:?} does not match # heading {:?}",
                            fm.title, h1
                        ),
                    });
                }
            }
            None => issues.push(Issue {
                file: rel.clone(),
                problem: "no level-1 heading (# title) found".to_string(),
            }),
        }

        // 4. broken relative links
        let base = parent_dir(rel);
        for dest in markdown::link_dests(body) {
            if util::is_absolute(&dest) {
                continue;
            }
            let (path, _anchor) = util::split_anchor(&dest);
            if path.is_empty() {
                continue; // anchor-only
            }
            let segs = util::resolve(&base, path);
            if util::inside_docs(&segs) || segs.first().map(|s| s == "server").unwrap_or(false) {
                // resolve against repo root on disk
                let target: PathBuf = repo_root.join(util::join(&segs));
                if !target.exists() {
                    issues.push(Issue {
                        file: rel.clone(),
                        problem: format!("broken link: {dest} (resolves to {})", util::join(&segs)),
                    });
                }
            } else {
                // Any relative link should resolve to an existing file.
                let target: PathBuf = repo_root.join(util::join(&segs));
                if !target.exists() {
                    issues.push(Issue {
                        file: rel.clone(),
                        problem: format!("broken link: {dest} (resolves to {})", util::join(&segs)),
                    });
                }
            }
        }

        // 5. secrets
        let full = raw.iter().find(|(r, _)| r == rel).map(|(_, t)| t.clone()).unwrap_or_default();
        for s in scan_secrets(&full) {
            issues.push(Issue {
                file: rel.clone(),
                problem: format!("likely secret: {s}"),
            });
        }
    }

    // Check 3: every non-README page in a folder is listed in that folder's README.
    for (folder, members) in &by_folder {
        // find the folder's README
        let readme_rel = if folder == "docs" {
            "docs/README.md".to_string()
        } else {
            format!("{folder}/README.md")
        };
        let readme = members.iter().find(|(r, _, _)| r == &readme_rel);
        let required: Vec<String> = members
            .iter()
            .filter(|(r, _, _)| r != &readme_rel)
            .map(|(r, _, _)| basename(r).to_string())
            .collect();
        let listed: BTreeSet<String> = match readme {
            Some((_, _, body)) => {
                let mut set = BTreeSet::new();
                for dest in markdown::link_dests(body) {
                    if util::is_absolute(&dest) {
                        continue;
                    }
                    let (path, _) = util::split_anchor(&dest);
                    if path.is_empty() {
                        continue;
                    }
                    let segs = util::resolve(folder, path);
                    // direct child of this folder only (folder + one segment)
                    if direct_child_of(folder, &segs) {
                        set.insert(segs.last().unwrap().clone());
                    }
                }
                set
            }
            None => {
                issues.push(Issue {
                    file: readme_rel.clone(),
                    problem: "folder has no README.md".to_string(),
                });
                BTreeSet::new()
            }
        };
        for req in &required {
            if !listed.contains(req) {
                issues.push(Issue {
                    file: format!("{folder}/{req}"),
                    problem: format!("page not listed in folder README ({readme_rel})"),
                });
            }
        }
    }

    issues
}

/// True if `segs` is exactly `folder` plus one trailing segment.
fn direct_child_of(folder: &str, segs: &[String]) -> bool {
    let fparts: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() != fparts.len() + 1 {
        return false;
    }
    segs.iter()
        .take(fparts.len())
        .zip(fparts.iter())
        .all(|(a, b)| a == b)
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_md(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
}

fn parent_dir(repo_rel: &str) -> String {
    match repo_rel.rfind('/') {
        Some(i) => repo_rel[..i].to_string(),
        None => String::new(),
    }
}

fn basename(repo_rel: &str) -> &str {
    match repo_rel.rfind('/') {
        Some(i) => &repo_rel[i + 1..],
        None => repo_rel,
    }
}

/// Scan raw text for likely secrets. Returns short labels for each hit.
fn scan_secrets(text: &str) -> Vec<String> {
    let mut hits = Vec::new();
    scan_ips(text, &mut hits);
    scan_prefixed(text, &mut hits);
    hits
}

/// Private, non-loopback IPv4 addresses: 10/8, 172.16/12, 192.168/16, 169.254/16,
/// 100.64/10 (CGNAT/Tailscale). Loopback (127/8) and 0.0.0.0 are NOT flagged
/// (they are documented, non-secret addresses).
fn scan_ips(text: &str, hits: &mut Vec<String>) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        // boundary before: previous char must not be digit or dot
        if i > 0 {
            let p = bytes[i - 1];
            if p.is_ascii_digit() || p == b'.' {
                // skip the number
                while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                continue;
            }
        }
        if let Some((ip, end)) = read_ip(bytes, i) {
            if is_private(&ip) {
                hits.push(format!("private IP {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]));
            }
            i = end;
        } else {
            i += 1;
        }
    }
}

fn read_ip(b: &[u8], start: usize) -> Option<([u32; 4], usize)> {
    let mut octets = [0u32; 4];
    let mut i = start;
    for idx in 0..4 {
        // read 1-3 digits
        let mut val = 0u32;
        let mut digits = 0;
        while i < b.len() && b[i].is_ascii_digit() && digits < 3 {
            val = val * 10 + (b[i] - b'0') as u32;
            i += 1;
            digits += 1;
        }
        if digits == 0 || val > 255 {
            return None;
        }
        octets[idx] = val;
        if idx < 3 {
            if i >= b.len() || b[i] != b'.' {
                return None;
            }
            i += 1;
        }
    }
    // boundary after: next char must not be digit or dot
    if i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        return None;
    }
    Some((octets, i))
}

fn is_private(ip: &[u32; 4]) -> bool {
    match (ip[0], ip[1]) {
        (10, _) => true,
        (172, b) => (16..=31).contains(&b),
        (192, 168) => true,
        (169, 254) => true,
        (100, b) => (64..=127).contains(&b),
        _ => false,
    }
}

/// Known leaked-credential prefixes.
fn scan_prefixed(text: &str, hits: &mut Vec<String>) {
    let needles = [
        ("sk-", "sk-", 20usize, "OpenAI key"),
        ("AKIA", "AKIA", 16, "AWS access key"),
        ("ghp_", "ghp_", 20, "GitHub token"),
        ("gho_", "gho_", 20, "GitHub token"),
        ("ghu_", "ghu_", 20, "GitHub token"),
        ("ghs_", "ghs_", 20, "GitHub token"),
        ("ghr_", "ghr_", 20, "GitHub token"),
        ("AIza", "AIza", 35, "Google API key"),
    ];
    let slack = ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"];

    let bytes = text.as_bytes();
    for (needle, _label, min_len, name) in &needles {
        let mut from = 0;
        while let Some(pos) = text[from..].find(needle) {
            let abs = from + pos;
            let rest = &text[abs + needle.len()..];
            let count = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
                .count();
            if count >= *min_len {
                hits.push(format!("{name} ({needle}…)"));
            }
            from = abs + needle.len();
        }
        let _ = bytes;
    }
    for s in &slack {
        let mut from = 0;
        while let Some(pos) = text[from..].find(s) {
            let abs = from + pos;
            let rest = &text[abs + s.len()..];
            let count = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .count();
            if count >= 10 {
                hits.push(format!("Slack token ({s}…)"));
            }
            from = abs + s.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ips_flagged() {
        let mut hits = Vec::new();
        scan_ips("server at 10.0.0.5 and 192.168.1.1", &mut hits);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn loopback_not_flagged() {
        let mut hits = Vec::new();
        scan_ips("bind 127.0.0.1 and 0.0.0.0", &mut hits);
        assert!(hits.is_empty());
    }

    #[test]
    fn versions_not_flagged() {
        let mut hits = Vec::new();
        scan_ips("rustc 1.99.1 and v0.460.0", &mut hits);
        assert!(hits.is_empty());
    }

    #[test]
    fn openai_key_flagged() {
        let mut hits = Vec::new();
        scan_prefixed("key = sk-abcdefghijklmnopqrstuvwxyz123456", &mut hits);
        assert!(hits.iter().any(|h| h.contains("OpenAI")));
    }

    #[test]
    fn short_sk_not_flagged() {
        let mut hits = Vec::new();
        scan_prefixed("see sk-abc", &mut hits);
        assert!(hits.is_empty());
    }
}

#[cfg(test)]
mod golden {
    //! Golden tests: one fixture per check, asserting the exact issue text.
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn fresh_repo() -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("docs-site-golden-{}-{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("docs")).unwrap();
        root
    }

    fn write(root: &Path, rel: &str, content: impl AsRef<str>) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, content.as_ref().as_bytes()).unwrap();
    }

    fn page(title: &str, order: i64, body_extra: &str) -> String {
        format!(
            "---\ntitle: {title}\ndescription: d\norder: {order}\n---\n\n# {title}\n\n{body_extra}"
        )
    }

    fn issues(root: &Path) -> Vec<Issue> {
        run(&root.join("docs"), root)
    }

    fn has(issues: &[Issue], needle: &str) -> bool {
        issues.iter().any(|i| i.problem.contains(needle))
    }

    #[test]
    fn clean_tree_passes() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [What](overview/what-is.md)\n- [Migration](MIGRATION.md)\n");
        write(&root, "docs/MIGRATION.md", page("Migration", 99, "text"));
        write(&root, "docs/overview/README.md", "---\ntitle: Overview\ndescription: d\norder: 1\n---\n\n# Overview\n\n- [What](what-is.md)\n");
        write(&root, "docs/overview/what-is.md", page("What", 1, "body"));
        assert!(issues(&root).is_empty(), "{:?}", issues(&root));
    }

    #[test]
    fn missing_front_matter_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Migration](MIGRATION.md)\n");
        write(&root, "docs/MIGRATION.md", page("Migration", 99, "text"));
        write(&root, "docs/bad.md", "# No front matter here\n");
        let i = issues(&root);
        let bad = i.iter().find(|x| x.file == "docs/bad.md").expect("issue for bad.md");
        assert!(bad.problem.contains("front matter"), "{}", bad.problem);
    }

    #[test]
    fn title_mismatch_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Bad](bad.md)\n");
        write(&root, "docs/bad.md", "---\ntitle: One\ndescription: d\norder: 1\n---\n\n# Two\n\nbody");
        let i = issues(&root);
        assert!(has(&i, "does not match"), "{:?}", i);
    }

    #[test]
    fn page_not_listed_in_readme_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n");
        write(&root, "docs/overview/README.md", "---\ntitle: Overview\ndescription: d\norder: 1\n---\n\n# Overview\n");
        write(&root, "docs/overview/orphan.md", page("Orphan", 1, "body"));
        let i = issues(&root);
        assert!(i.iter().any(|x| x.file.contains("orphan") && has(std::slice::from_ref(x), "not listed")), "{:?}", i);
    }

    #[test]
    fn broken_link_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Bad](bad.md)\n");
        write(&root, "docs/bad.md", page("Bad", 1, "see [nowhere](missing.md)."));
        let i = issues(&root);
        assert!(has(&i, "broken link"), "{:?}", i);
    }

    #[test]
    fn private_ip_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Bad](bad.md)\n");
        write(&root, "docs/bad.md", page("Bad", 1, "server at 10.20.30.40"));
        let i = issues(&root);
        assert!(has(&i, "private IP"), "{:?}", i);
    }

    #[test]
    fn leaked_key_fails() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Bad](bad.md)\n");
        write(&root, "docs/bad.md", page("Bad", 1, "key = sk-abcdefghijklmnopqrstuvwxyz0123456789"));
        let i = issues(&root);
        assert!(has(&i, "OpenAI"), "{:?}", i);
    }

    #[test]
    fn loopback_is_not_a_secret() {
        let root = fresh_repo();
        write(&root, "docs/README.md", "---\ntitle: AssistX docs\ndescription: idx\norder: 0\n---\n\n# AssistX docs\n\n- [Bad](bad.md)\n");
        write(&root, "docs/bad.md", page("Bad", 1, "bind 127.0.0.1 and 0.0.0.0"));
        assert!(issues(&root).is_empty(), "{:?}", issues(&root));
    }
}
