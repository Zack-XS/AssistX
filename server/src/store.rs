//! Plugin lifecycle (docs/03): live / draft / versions, core-kept status and hash, approval
//! cards built from files, install from a path, build-mode fetches, and secrets by name.

use crate::manifest::{self, Manifest};
use crate::{App, gate, proxy, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const MAX_DRAFT_FILES: usize = 60;
const MAX_DRAFT_FILE_BYTES: usize = 200_000;

pub fn live_dir(app: &App, id: &str) -> PathBuf {
    app.cfg.plugins_dir.join(id)
}
pub fn draft_dir(app: &App, id: &str) -> PathBuf {
    app.cfg.plugins_dir.join(".drafts").join(id)
}
fn versions_dir(app: &App, id: &str) -> PathBuf {
    app.cfg.plugins_dir.join(".versions").join(id)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// All files of a plugin folder except `state/`, sorted by relative path.
fn files_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            if rel == "state" || rel.starts_with("state/") || rel.ends_with(".DS_Store") {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() => out.push((rel, std::fs::read(&p).unwrap_or_default())),
                _ => {}
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn hash_dir(dir: &Path) -> String {
    let mut buf = vec![];
    for (rel, data) in files_of(dir) {
        buf.extend_from_slice(rel.as_bytes());
        buf.push(0);
        buf.extend_from_slice(util::sha256_hex(&data).as_bytes());
        buf.push(0);
    }
    util::sha256_hex(&buf)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    for (rel, data) in files_of(from) {
        let dst = to.join(&rel);
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(dst, data)?;
    }
    Ok(())
}

/// Remove everything in `dir` except `state/`.
fn clear_except_state(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name() == "state" {
            continue;
        }
        let p = e.path();
        let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
    }
}

// ---------------- core-kept state ----------------

pub fn status(app: &App, id: &str) -> Option<String> {
    app.db.lock().query_row("SELECT status FROM plugin_state WHERE id = ?1", [id], |r| r.get(0)).optional().ok().flatten()
}

pub fn state_json(app: &App, id: &str) -> Value {
    app.db
        .lock()
        .query_row("SELECT status, version, hash, source, url, sha256, installed_at, signed_by, created_by FROM plugin_state WHERE id = ?1", [id], |r| {
            Ok(json!({ "status": r.get::<_, String>(0)?, "version": r.get::<_, String>(1)?, "hash": r.get::<_, String>(2)?,
                "provenance": { "source": r.get::<_, String>(3)?, "url": r.get::<_, Option<String>>(4)?, "sha256": r.get::<_, Option<String>>(5)?,
                    "installed_at": r.get::<_, Option<String>>(6)?, "signed_by": r.get::<_, Option<String>>(7)? },
                "created_by": r.get::<_, String>(8)? }))
        })
        .optional()
        .ok()
        .flatten()
        .unwrap_or(Value::Null)
}

#[allow(clippy::too_many_arguments)]
fn upsert_state(app: &App, id: &str, status: &str, version: &str, hash: &str, source: &str, sha: Option<&str>, created_by: &str) {
    let _ = app.db.lock().execute(
        "INSERT INTO plugin_state (id, status, version, hash, source, sha256, installed_at, created_by) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(id) DO UPDATE SET status=?2, version=?3, hash=?4, source=?5, sha256=COALESCE(?6, sha256), installed_at=?7, created_by=?8",
        params![id, status, version, hash, source, sha, util::now_iso(), created_by],
    );
}

/// Startup / reload: read live folders. Plugins shipped in the repo (`source local`, made by a
/// human) are trusted as they are on disk; anything else must match its approved hash.
pub fn reconcile(app: &App) -> (BTreeMap<String, Manifest>, Vec<String>) {
    let (found, mut problems) = manifest::load_all(&app.cfg.plugins_dir);
    for (id, m) in &found {
        let h = hash_dir(&m.dir);
        let row: Option<(String, String, String)> = app.db.lock()
            .query_row("SELECT hash, source, created_by FROM plugin_state WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional().ok().flatten();
        match row {
            None => upsert_state(app, id, "active", &m.version, &h, "local", None, "human"),
            Some((old, source, by)) if old != h => {
                if source == "local" && by == "human" {
                    let _ = app.db.lock().execute("UPDATE plugin_state SET hash = ?1, version = ?2 WHERE id = ?3", params![h, m.version, id]);
                    problems.push(format!("{id}: files changed on disk; re-hashed (shipped plugin)"));
                } else {
                    problems.push(format!("{id}: plugin_tampered (files differ from the approved version)"));
                }
            }
            _ => {}
        }
    }
    (found, problems)
}

/// Checked before every tool run (docs/03). A mismatch blocks with `plugin_tampered`.
pub fn verify_hash(app: &App, id: &str) -> Result<(), String> {
    let stored: Option<String> = app.db.lock().query_row("SELECT hash FROM plugin_state WHERE id = ?1", [id], |r| r.get(0)).optional().ok().flatten();
    let now = hash_dir(&live_dir(app, id));
    if stored.as_deref() == Some(now.as_str()) { Ok(()) } else { Err("plugin_tampered: plugin files changed since approval".into()) }
}

// ---------------- secrets (by name; values never leave the core) ----------------

pub fn secret_get(app: &App, name: &str) -> Option<String> {
    app.db.lock().query_row("SELECT value FROM secrets WHERE name = ?1", [name], |r| r.get(0)).optional().ok().flatten()
}
pub fn secret_set(app: &App, name: &str, value: &str) -> bool {
    valid_secret_name(name) && app.db.lock().execute("INSERT OR REPLACE INTO secrets (name, value, updated) VALUES (?1, ?2, ?3)", params![name, value, util::now_iso()]).is_ok()
}
pub fn secret_delete(app: &App, name: &str) -> bool {
    app.db.lock().execute("DELETE FROM secrets WHERE name = ?1", [name]).map(|n| n > 0).unwrap_or(false)
}
pub fn secret_names(app: &App) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT name, updated FROM secrets ORDER BY name").expect("prepare");
    st.query_map([], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "updated": r.get::<_, String>(1)? }))).map(|x| x.flatten().collect()).unwrap_or_default()
}
pub fn valid_secret_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------- drafts ----------------

fn safe_rel(rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("path must stay inside the plugin folder".into());
    }
    if rel == "state" || rel.starts_with("state/") {
        return Err("state/ is plugin data, not code".into());
    }
    Ok(p.to_path_buf())
}

/// Start a draft (copy of live if one exists, so the live version keeps working).
fn ensure_draft(app: &App, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("plugin id: lowercase letters, digits and - only".into());
    }
    let d = draft_dir(app, id);
    if !d.exists() {
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let live = live_dir(app, id);
        if live.join("plugin.yaml").exists() {
            copy_tree(&live, &d).map_err(|e| e.to_string())?;
        }
    }
    Ok(d)
}

pub fn draft_write(app: &App, id: &str, rel: &str, content: &str) -> Result<Value, String> {
    let d = ensure_draft(app, id)?;
    let rel = safe_rel(rel)?;
    if content.len() > MAX_DRAFT_FILE_BYTES {
        return Err("file too big".into());
    }
    if !d.join(&rel).exists() && files_of(&d).len() >= MAX_DRAFT_FILES {
        return Err("too many files in draft".into());
    }
    let dst = d.join(&rel);
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&dst, content).map_err(|e| e.to_string())?;
    Ok(json!({ "written": rel, "bytes": content.len() }))
}

pub fn draft_read(app: &App, id: &str, rel: &str) -> Result<Value, String> {
    let p = draft_dir(app, id).join(safe_rel(rel)?);
    std::fs::read_to_string(&p).map(|c| json!({ "path": rel, "content": c })).map_err(|_| "no such file in draft".into())
}

pub fn draft_list(app: &App, id: &str) -> Value {
    json!({ "files": files_of(&draft_dir(app, id)).into_iter().map(|(r, d)| json!({ "path": r, "bytes": d.len() })).collect::<Vec<_>>() })
}

pub fn draft_manifest(app: &App, id: &str) -> Result<Manifest, String> {
    manifest::load_one(&draft_dir(app, id), id)
}

/// Run the draft's `tests/*.json` cases in the sandbox with no network.
/// A case: `{"tool": "...", "args": {...}, "expect_ok": true}`. Tests are a weak signal (docs/03).
pub async fn draft_test(app: &Arc<App>, id: &str) -> Value {
    let m = match draft_manifest(app, id) {
        Ok(m) => m,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    if crate::runner::backend(app, &m).is_none() {
        return json!({ "ok": false, "error": "runtime.command is missing" });
    }
    let mut results = vec![];
    let mut all_ok = true;
    let tests = files_of(&m.dir).into_iter().filter(|(r, _)| r.starts_with("tests/") && r.ends_with(".json")).collect::<Vec<_>>();
    for (rel, data) in tests.iter().take(20) {
        let case: Value = serde_json::from_slice(data).unwrap_or(Value::Null);
        let tool = case["tool"].as_str().unwrap_or_default().to_string();
        if m.tool(&tool).is_none() {
            all_ok = false;
            results.push(json!({ "test": rel, "ok": false, "error": "unknown tool" }));
            continue;
        }
        let expect = case["expect_ok"].as_bool().unwrap_or(true);
        let kill = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = crate::runner::run(app, &m, &tool, case["args"].clone(), true, Some(proxy::Allow::none()), kill).await;
        let passed = r.is_ok() == expect;
        all_ok &= passed;
        results.push(json!({ "test": rel, "tool": tool, "args": case["args"], "passed": passed,
            "result": r.as_ref().ok(), "error": r.as_ref().err() }));
    }
    json!({ "ok": all_ok, "tests": results, "note": if tests.is_empty() { "no tests/ in draft" } else { "" } })
}

/// Approval card for a draft, built from the files (docs/03, Approval card).
pub fn card(app: &App, id: &str, fetched: &[String], outside_source: bool, tests: Option<Value>) -> Result<(Value, gate::Risk), String> {
    let new = draft_manifest(app, id)?;
    let old = app.manifest(id);
    let old_hosts: Vec<String> = old.as_ref().map(|m| m.needs.network.clone()).unwrap_or_default();
    let old_secrets: Vec<String> = old.as_ref().map(|m| m.needs.secrets.clone()).unwrap_or_default();
    let added_hosts: Vec<&String> = new.needs.network.iter().filter(|h| !old_hosts.contains(h)).collect();
    let added_secrets: Vec<&String> = new.needs.secrets.iter().filter(|s| !old_secrets.contains(s)).collect();
    let auto_writes: Vec<&str> = new.tools.iter()
        .filter(|t| t.autonomy.as_deref() == Some("auto") && t.tags.iter().any(|g| g != "reads"))
        .map(|t| t.name.as_str()).collect();
    let tag_changes: Vec<Value> = new.tools.iter().filter_map(|t| {
        let o = old.as_ref()?.tool(&t.name)?;
        (o.tags != t.tags || o.autonomy != t.autonomy).then(|| json!({ "tool": t.name, "from": { "tags": o.tags, "autonomy": o.autonomy }, "to": { "tags": t.tags, "autonomy": t.autonomy } }))
    }).collect();
    let old_programs = old.as_ref().map(|m| m.programs()).unwrap_or_default();
    let programs = new.programs();
    let added_programs: Vec<&String> = programs.iter().filter(|p| !old_programs.contains(p)).collect();
    let shell_programs: Vec<&String> = programs.iter().filter(|p| crate::manifest::is_shell_program(p)).collect();
    let timeout_secs = new.runtime.timeout_secs;
    let longer = timeout_secs.is_some_and(|t| t > app.cfg.limits.tool_wall_secs && Some(t) != old.as_ref().and_then(|m| m.runtime.timeout_secs));
    let mentions_others: Vec<String> = app.manifests().keys().filter(|k| *k != id && new.guide.contains(&format!("{k}."))).cloned().collect();
    let high = !added_hosts.is_empty() || !added_secrets.is_empty() || !auto_writes.is_empty() || !tag_changes.is_empty() || !added_programs.is_empty() || longer || outside_source || !fetched.is_empty();
    let risk = if high { gate::Risk::High } else { gate::Risk::Medium };
    let show_guide = outside_source || !fetched.is_empty();
    Ok((json!({
        "plugin": id, "version": new.version, "update_of": old.as_ref().map(|m| m.version.clone()),
        "purpose": util::clean_line(&new.summary, 25),
        "network_hosts": new.needs.network, "added_hosts": added_hosts,
        "secrets": new.needs.secrets, "added_secrets": added_secrets,
        "extra_needs": new.needs.other,
        "programs": programs, "added_programs": added_programs,
        "programs_warning": if shell_programs.is_empty() { Value::Null } else { Value::String(format!("{} can run any command line, so this plugin can do almost anything the sandbox allows.", shell_programs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))) },
        "timeout_secs": timeout_secs,
        "tools": new.tools.iter().map(|t| json!({ "name": t.name, "tags": t.tags, "requested": t.autonomy.as_deref().unwrap_or("ask"),
            "highlight": auto_writes.contains(&t.name.as_str()) })).collect::<Vec<_>>(),
        "tag_changes": tag_changes,
        "fetched_urls": fetched,
        "guide": if show_guide { Value::String(new.guide.clone()) } else { Value::Null },
        "guide_one_tap": !show_guide,
        "tests": tests,
        "warning_mentions_other_plugins": mentions_others,
        "hash": hash_dir(&new.dir),
    }), risk))
}

/// Raise the plugin approval card. Returns the approval id.
#[allow(clippy::too_many_arguments)]
pub fn submit(app: &App, thread: &str, turn: &str, id: &str, fetched: &[String], outside_source: bool, tests: Option<Value>, reason: Option<String>) -> Result<String, String> {
    // A guide installed from outside, or written after reading web pages, is outside content (docs/02).
    if outside_source || !fetched.is_empty() {
        let _ = std::fs::write(draft_dir(app, id).join(".outside"), "guide is outside content\n");
    }
    let (card, risk) = card(app, id, fetched, outside_source, tests)?;
    let kind = if app.manifest(id).is_some() { "plugin_update" } else { "plugin_install" };
    let question = format!("{} plugin \"{id}\" v{}?", if kind == "plugin_update" { "Update" } else { "Install" }, card["version"].as_str().unwrap_or("?"));
    let hash = card["hash"].as_str().unwrap_or_default().to_string();
    Ok(crate::engine::create_approval_raw(app, thread, turn, kind, risk, &question, reason, card, &hash))
}

/// Promote the draft to live. `expect_hash` binds an approval to the exact files it showed.
pub fn approve(app: &App, id: &str, expect_hash: Option<&str>, created_by: &str) -> Result<Value, String> {
    let d = draft_dir(app, id);
    if !d.join("plugin.yaml").exists() {
        // No draft: re-enable a disabled plugin.
        if status(app, id).as_deref() == Some("disabled") {
            let _ = app.db.lock().execute("UPDATE plugin_state SET status = 'active' WHERE id = ?1", [id]);
            app.reload_plugins();
            return Ok(json!({ "id": id, "status": "active" }));
        }
        return Err("no draft to approve".into());
    }
    let m = draft_manifest(app, id)?;
    let h = hash_dir(&d);
    if let Some(x) = expect_hash
        && x != h
    {
        return Err("the draft changed after the card was made; raise a new approval".into());
    }
    let live = live_dir(app, id);
    if let Some(old) = app.manifest(id) {
        let v = versions_dir(app, id).join(&old.version);
        let _ = std::fs::remove_dir_all(&v);
        copy_tree(&live, &v).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&live).map_err(|e| e.to_string())?;
    clear_except_state(&live);
    copy_tree(&d, &live).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&d);
    let source: String = app.db.lock().query_row("SELECT source FROM plugin_state WHERE id = ?1", [id], |r| r.get(0)).optional().ok().flatten()
        .or_else(|| std::fs::read_to_string(app.cfg.plugins_dir.join(".drafts").join(format!("{id}.source"))).ok())
        .unwrap_or_else(|| "local".into());
    let _ = std::fs::remove_file(app.cfg.plugins_dir.join(".drafts").join(format!("{id}.source")));
    upsert_state(app, id, "active", &m.version, &hash_dir(&live), &source, None, created_by);
    app.reload_plugins();
    app.feed.emit(None, None, "plugin.loaded", json!({ "plugin": id, "reason": "approved", "version": m.version }));
    Ok(json!({ "id": id, "status": "active", "version": m.version }))
}

pub fn reject(app: &App, id: &str) -> bool {
    let _ = std::fs::remove_file(app.cfg.plugins_dir.join(".drafts").join(format!("{id}.source")));
    std::fs::remove_dir_all(draft_dir(app, id)).is_ok()
}

pub fn disable(app: &App, id: &str) -> bool {
    let n = app.db.lock().execute("UPDATE plugin_state SET status = 'disabled' WHERE id = ?1", [id]).unwrap_or(0);
    app.unload_everywhere(&format!("plugin:{id}"));
    app.reload_plugins();
    n > 0
}

pub fn versions(app: &App, id: &str) -> Value {
    let mut v: Vec<String> = std::fs::read_dir(versions_dir(app, id)).into_iter().flatten().flatten()
        .filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    json!({ "live": app.manifest(id).map(|m| m.version), "draft": draft_manifest(app, id).ok().map(|m| m.version), "older": v })
}

pub fn rollback(app: &App, id: &str, version: &str) -> Result<Value, String> {
    if !version.chars().all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)) {
        return Err("bad version".into());
    }
    let src = versions_dir(app, id).join(version);
    if !src.join("plugin.yaml").exists() {
        return Err("no such version".into());
    }
    let d = draft_dir(app, id);
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(&src, &d).map_err(|e| e.to_string())?;
    let by: String = app.db.lock().query_row("SELECT created_by FROM plugin_state WHERE id = ?1", [id], |r| r.get(0)).optional().ok().flatten().unwrap_or_else(|| "human".into());
    approve(app, id, None, &by)
}

pub fn uninstall(app: &App, id: &str, remove_state: bool) -> bool {
    let live = live_dir(app, id);
    if !live.exists() {
        return false;
    }
    if remove_state {
        let _ = std::fs::remove_dir_all(&live);
    } else {
        clear_except_state(&live);
    }
    let _ = app.db.lock().execute("DELETE FROM plugin_state WHERE id = ?1", [id]);
    app.unload_everywhere(&format!("plugin:{id}"));
    app.reload_plugins();
    true
}

/// Install from a local folder. It arrives as a draft and needs approval at `high` risk.
pub fn install_from_path(app: &App, path: &str) -> Result<String, String> {
    let src = PathBuf::from(path);
    let probe: Value = serde_yaml::from_str::<serde_yaml::Value>(&std::fs::read_to_string(src.join("plugin.yaml")).map_err(|e| format!("plugin.yaml: {e}"))?)
        .ok().and_then(|y| serde_json::to_value(y).ok()).unwrap_or(Value::Null);
    let id = probe["id"].as_str().filter(|i| valid_id(i)).ok_or("plugin.yaml needs a valid id")?.to_string();
    let d = draft_dir(app, &id);
    let _ = std::fs::remove_dir_all(&d);
    copy_tree(&src, &d).map_err(|e| e.to_string())?;
    manifest::load_one(&d, &id).inspect_err(|_| { let _ = std::fs::remove_dir_all(&d); })?;
    let _ = std::fs::write(app.cfg.plugins_dir.join(".drafts").join(format!("{id}.source")), "path");
    Ok(id)
}

/// Line diff between two versions of one file set (live vs draft by default).
pub fn diff(app: &App, id: &str, from: Option<&str>, to: Option<&str>) -> Result<Value, String> {
    let pick = |v: Option<&str>, default_draft: bool| -> Result<PathBuf, String> {
        match v {
            Some("live") => Ok(live_dir(app, id)),
            Some("draft") => Ok(draft_dir(app, id)),
            Some(x) if x.chars().all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)) => Ok(versions_dir(app, id).join(x)),
            Some(_) => Err("bad version".into()),
            None => Ok(if default_draft { draft_dir(app, id) } else { live_dir(app, id) }),
        }
    };
    let (a, b) = (files_of(&pick(from, false)?), files_of(&pick(to, true)?));
    let am: BTreeMap<_, _> = a.into_iter().collect();
    let bm: BTreeMap<_, _> = b.into_iter().collect();
    let mut out = vec![];
    for k in am.keys().chain(bm.keys()).collect::<std::collections::BTreeSet<_>>() {
        let (x, y) = (am.get(k), bm.get(k));
        if x == y {
            continue;
        }
        let xs = x.map(|d| String::from_utf8_lossy(d).into_owned()).unwrap_or_default();
        let ys = y.map(|d| String::from_utf8_lossy(d).into_owned()).unwrap_or_default();
        let xl: Vec<&str> = xs.lines().collect();
        let yl: Vec<&str> = ys.lines().collect();
        let mut lines = vec![];
        for l in &xl {
            if !yl.contains(l) { lines.push(format!("-{l}")); }
        }
        for l in &yl {
            if !xl.contains(l) { lines.push(format!("+{l}")); }
        }
        out.push(json!({ "file": k, "change": match (x, y) { (None, _) => "added", (_, None) => "removed", _ => "changed" }, "lines": lines }));
    }
    Ok(json!({ "files": out }))
}

// ---------------- build mode research (docs/03, Build mode) ----------------

/// Read-only GET to any public host, through a GET-only proxy. No secrets, no body,
/// URL at most 200 characters, rate limited, logged for the approval card.
pub async fn build_fetch(app: &Arc<App>, plugin: &str, url: &str) -> Result<String, String> {
    if url.len() > 200 {
        return Err("URL longer than 200 characters".into());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.split('/').nth(2).is_some_and(|a| a.contains('@')) {
        return Err("need a plain http(s) URL".into());
    }
    if !app.rate_ok(&format!("build:{plugin}"), app.cfg.limits.build_fetches_per_minute) {
        return Err("rate_limited: too many fetches, wait a minute".into());
    }
    let px = proxy::start(proxy::Allow { hosts: vec!["*public*".into()], get_only: true }).await.map_err(|e| e.to_string())?;
    let (u, port) = (url.to_string(), px.port);
    let res = tokio::task::spawn_blocking(move || -> Result<(u16, String), String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .proxy(ureq::Proxy::new(&format!("http://127.0.0.1:{port}")).ok())
            .build()
            .into();
        let mut r = agent.get(&u).header("User-Agent", "agent-core-build/0.1").call().map_err(|e| e.to_string())?;
        let status = r.status().as_u16();
        let body = r.body_mut().with_config().limit(1_000_000).read_to_string().unwrap_or_default();
        Ok((status, body))
    }).await.map_err(|e| e.to_string())?;
    drop(px);
    let status = res.as_ref().map(|r| r.0 as i64).unwrap_or(0);
    let _ = app.db.lock().execute("INSERT INTO build_log (plugin, url, status, ts) VALUES (?1, ?2, ?3, ?4)", params![plugin, url, status, util::unix_now()]);
    let (status, body) = res?;
    let text = crate::plugins::web::to_text(&body);
    Ok(format!("HTTP {status}\n{}", text.chars().take(30_000).collect::<String>()))
}

pub fn fetched_urls(app: &App, plugin: &str) -> Vec<String> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT DISTINCT url FROM build_log WHERE plugin = ?1 ORDER BY n").expect("prepare");
    st.query_map([plugin], |r| r.get(0)).map(|x| x.flatten().collect()).unwrap_or_default()
}
