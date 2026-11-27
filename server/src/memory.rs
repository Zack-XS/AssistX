//! Memory store (docs/04): inbox, memory entries with version history, FTS5 search,
//! core-set trust, tombstones. Dreaming (src/dreaming.rs) and the API write through here.

use crate::db::Db;
use crate::util;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS inbox (
  id TEXT PRIMARY KEY, owner_id TEXT NOT NULL DEFAULT 'owner', text TEXT NOT NULL, topic TEXT, importance REAL,
  people TEXT NOT NULL DEFAULT '[]', trust TEXT NOT NULL, sources TEXT NOT NULL DEFAULT '[]',
  thread_id TEXT, turn_id TEXT, created TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending'
);
CREATE TABLE IF NOT EXISTS memory (
  n INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, owner_id TEXT NOT NULL DEFAULT 'owner',
  title TEXT NOT NULL, summary TEXT NOT NULL, detail TEXT NOT NULL DEFAULT '', topic_path TEXT NOT NULL DEFAULT 'misc',
  tags TEXT NOT NULL DEFAULT '[]', people TEXT NOT NULL DEFAULT '[]', places TEXT NOT NULL DEFAULT '[]',
  time_range TEXT, importance REAL NOT NULL DEFAULT 0.5, confidence REAL NOT NULL DEFAULT 0.5,
  source_trust TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'current', sources TEXT NOT NULL DEFAULT '[]',
  supersedes TEXT NOT NULL DEFAULT '[]', created TEXT NOT NULL, updated INTEGER NOT NULL, last_used INTEGER,
  version INTEGER NOT NULL DEFAULT 1, embedding_model TEXT
);
CREATE TABLE IF NOT EXISTS memory_history (
  entry_id TEXT NOT NULL, version INTEGER NOT NULL, snapshot TEXT NOT NULL, changed TEXT NOT NULL, by TEXT NOT NULL,
  PRIMARY KEY (entry_id, version)
);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(entry_id UNINDEXED, title, summary, detail, topic_path, tags);
CREATE TABLE IF NOT EXISTS erased_sources (message_id TEXT PRIMARY KEY, created TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tombstones (id TEXT PRIMARY KEY, created TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS memory_folders (path TEXT PRIMARY KEY, description TEXT NOT NULL DEFAULT '', created TEXT NOT NULL);
"#;

/// Trust order, lowest first. A derived fact never has more trust than its sources (docs/04).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    Outside,
    Inferred,
    User,
}

impl Trust {
    pub fn as_str(self) -> &'static str {
        match self {
            Trust::Outside => "outside",
            Trust::Inferred => "inferred",
            Trust::User => "user",
        }
    }
    pub fn parse(s: &str) -> Trust {
        match s {
            "user" => Trust::User,
            "inferred" => Trust::Inferred,
            _ => Trust::Outside,
        }
    }
    /// Map a raw-log label to a fact trust level.
    pub fn from_message_label(label: &str) -> Trust {
        match label {
            "user" => Trust::User,
            "assistant" | "core" => Trust::Inferred,
            _ => Trust::Outside,
        }
    }
    pub fn start_confidence(self) -> f64 {
        match self {
            Trust::User => 0.9,
            Trust::Inferred => 0.6,
            Trust::Outside => 0.3,
        }
    }
}

/// Lowest trust among source messages, computed by the core. Unknown ids count as outside.
pub fn trust_of_sources(db: &Db, sources: &[String]) -> Option<Trust> {
    if sources.is_empty() {
        return None;
    }
    let c = db.lock();
    let mut lowest = Trust::User;
    for s in sources {
        let label: Option<String> = c.query_row("SELECT trust FROM messages WHERE id = ?1", [s], |r| r.get(0)).optional().ok().flatten();
        let t = label.map(|l| Trust::from_message_label(&l)).unwrap_or(Trust::Outside);
        lowest = lowest.min(t);
    }
    Some(lowest)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub topic_path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub people: Vec<String>,
    #[serde(default)]
    pub places: Vec<String>,
    #[serde(default)]
    pub time_range: Option<String>,
    #[serde(default)]
    pub importance: f64,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub source_trust: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub last_used: Option<i64>,
    #[serde(default)]
    pub version: i64,
}

impl Entry {
    pub fn sensitive(&self) -> bool {
        self.tags.iter().any(|t| t == "sensitive")
    }
}

const COLS: &str = "id, title, summary, detail, topic_path, tags, people, places, time_range, importance, confidence, source_trust, status, sources, supersedes, created, updated, last_used, version";

fn row_entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    let arr = |i: usize| -> rusqlite::Result<Vec<String>> {
        let s: String = r.get(i)?;
        Ok(serde_json::from_str(&s).unwrap_or_default())
    };
    Ok(Entry {
        id: r.get(0)?, title: r.get(1)?, summary: r.get(2)?, detail: r.get(3)?, topic_path: r.get(4)?,
        tags: arr(5)?, people: arr(6)?, places: arr(7)?, time_range: r.get(8)?, importance: r.get(9)?,
        confidence: r.get(10)?, source_trust: r.get(11)?, status: r.get(12)?, sources: arr(13)?, supersedes: arr(14)?,
        created: r.get(15)?, updated: r.get(16)?, last_used: r.get(17)?, version: r.get(18)?,
    })
}

pub fn get(db: &Db, id: &str) -> Option<Entry> {
    db.lock().query_row(&format!("SELECT {COLS} FROM memory WHERE id = ?1"), [id], row_entry).optional().ok().flatten()
}

pub fn all_current(db: &Db) -> Vec<Entry> {
    let c = db.lock();
    let mut st = c.prepare(&format!("SELECT {COLS} FROM memory WHERE status IN ('current', 'disputed') ORDER BY topic_path, n")).expect("prepare");
    st.query_map([], row_entry).map(|r| r.flatten().collect()).unwrap_or_default()
}

pub fn all(db: &Db) -> Vec<Entry> {
    let c = db.lock();
    let mut st = c.prepare(&format!("SELECT {COLS} FROM memory ORDER BY n")).expect("prepare");
    st.query_map([], row_entry).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// The five folders that always exist. The model and the user can add more (see `make_folder`).
pub const FOLDERS: [&str; 5] = ["People", "Places", "Questions", "Things", "Other"];
const MAX_DEPTH: usize = 4;
const MAX_CUSTOM_TOP: usize = 20;
const MAX_FOLDERS: usize = 300;

pub fn slug_of(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '-' };
        if c == '-' && (out.is_empty() || out.ends_with('-')) { continue; }
        out.push(c);
    }
    out.trim_end_matches('-').to_string()
}

fn folder_alias(seg: &str) -> Option<&'static str> {
    let k: String = seg.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    match k.as_str() {
        "people" | "person" | "family" | "friends" | "contacts" | "relationships" => Some("People"),
        "places" | "place" | "locations" | "location" => Some("Places"),
        "questions" | "question" | "openquestions" | "faq" => Some("Questions"),
        "things" | "thing" | "objects" | "items" | "stuff" => Some("Things"),
        "other" | "otherthings" | "misc" => Some("Other"),
        _ => None,
    }
}

/// "school" -> "School". Custom top-level folders look like the five built-in ones.
fn title_case(slug: &str) -> String {
    let mut c = slug.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// The first part of a stored topic path: the top-level folder it lives in.
pub fn top_of(topic: &str) -> String {
    let first = topic.split('/').map(str::trim).find(|p| !p.is_empty()).unwrap_or("Other");
    folder_alias(first).map(String::from).unwrap_or_else(|| first.to_string())
}

/// Turn any path into a stored folder path. The first part is one of the five folders or a
/// registered custom folder (`custom`); anything else goes under `Other/`. Later parts are
/// lower-case slugs, at most `MAX_DEPTH` parts in all.
pub fn clean_topic_in(t: &str, custom: &[String]) -> String {
    let parts: Vec<&str> = t.split('/').map(str::trim).filter(|p| !p.is_empty()).collect();
    let known = |p: &str| -> Option<String> {
        folder_alias(p).map(String::from).or_else(|| custom.iter().find(|c| slug_of(c) == slug_of(p)).cloned())
    };
    let (top, rest) = match parts.first().and_then(|p| known(p)) {
        Some(f) => (f, &parts[1..]),
        None => ("Other".to_string(), &parts[..]),
    };
    let mut out = vec![top];
    out.extend(rest.iter().map(|p| slug_of(p)).filter(|p| !p.is_empty()).take(MAX_DEPTH - 1));
    out.join("/")
}

#[cfg(test)]
fn clean_topic(t: &str) -> String {
    clean_topic_in(t, &[])
}

#[derive(Clone, Debug, Serialize)]
pub struct FolderRow {
    pub path: String,
    pub description: String,
}

/// Folders someone made on purpose (the model through memory-tools, or the user). The five
/// built-in ones are not stored here.
pub fn folder_rows(db: &Db) -> Vec<FolderRow> {
    let c = db.lock();
    c.prepare("SELECT path, description FROM memory_folders ORDER BY path")
        .and_then(|mut st| st.query_map([], |r| Ok(FolderRow { path: r.get(0)?, description: r.get(1)? })).map(|x| x.flatten().collect()))
        .unwrap_or_default()
}

pub fn custom_tops(rows: &[FolderRow]) -> Vec<String> {
    let mut v: Vec<String> = rows.iter().filter_map(|r| r.path.split('/').next()).filter(|t| folder_alias(t).is_none()).map(String::from).collect();
    v.sort();
    v.dedup();
    v
}

/// The entry's own name is not a folder: `People/sam` for an entry titled "Sam" is just `People`.
fn without_own_name(topic: String, title: &str) -> String {
    let mut parts: Vec<&str> = topic.split('/').collect();
    if parts.len() >= 2 && parts.last().is_some_and(|l| *l == slug_of(title)) {
        parts.pop();
    }
    parts.join("/")
}

fn topic_for(db: &Db, topic: &str, title: &str) -> String {
    without_own_name(clean_topic_in(topic, &custom_tops(&folder_rows(db))), title)
}

/// Make a folder (and its description). `People/family`, `School/math`, or a new top-level `School`.
pub fn make_folder(db: &Db, path: &str, description: &str) -> Result<String, String> {
    let rows = folder_rows(db);
    let custom = custom_tops(&rows);
    let first = path.split('/').map(str::trim).find(|p| !p.is_empty()).ok_or("A folder needs a name.")?;
    let is_new_top = folder_alias(first).is_none() && !custom.iter().any(|c| slug_of(c) == slug_of(first));
    if is_new_top {
        if custom.len() >= MAX_CUSTOM_TOP { return Err("Too many top-level folders. Use a subfolder.".into()); }
        let name = title_case(&slug_of(first));
        if name.is_empty() || name.len() > 24 { return Err("Folder names: letters and digits, up to 24 characters.".into()); }
    }
    if rows.len() >= MAX_FOLDERS { return Err("Too many folders.".into()); }
    let mut known = custom.clone();
    if is_new_top { known.push(title_case(&slug_of(first))); }
    let clean = clean_topic_in(path, &known);
    if clean.starts_with("Other/") && folder_alias(first).is_none() && !is_new_top {
        return Err("That path is not valid.".into());
    }
    let desc = util::clean_line(description, 20);
    if FOLDERS.contains(&clean.as_str()) {
        return Ok(clean); // the five built-in folders always exist
    }
    let c = db.lock();
    // A new top-level folder is stored too, so its subfolders are recognised later.
    let top = clean.split('/').next().unwrap_or("").to_string();
    if folder_alias(&top).is_none() {
        let _ = c.execute("INSERT OR IGNORE INTO memory_folders (path, description, created) VALUES (?1, '', ?2)", params![top, util::now_iso()]);
    }
    let _ = c.execute(
        "INSERT INTO memory_folders (path, description, created) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO UPDATE SET description = CASE WHEN ?2 = '' THEN description ELSE ?2 END",
        params![clean, desc, util::now_iso()],
    );
    Ok(clean)
}

/// The folder an entry should be filed in: make it if it is new, or fall back to `Other`.
pub fn make_folder_if_new(db: &Db, path: &str) -> String {
    make_folder(db, path, "").unwrap_or_else(|_| "Other".to_string())
}

/// Move a whole folder (its files and its subfolders) to a new path. Returns how many entries moved.
pub fn move_folder(db: &Db, from: &str, to: &str) -> Result<usize, String> {
    let rows = folder_rows(db);
    let custom = custom_tops(&rows);
    let from = clean_topic_in(from, &custom);
    if FOLDERS.contains(&from.as_str()) { return Err("The five built-in folders cannot be moved.".into()); }
    if let Some(top) = to.split('/').map(str::trim).find(|p| !p.is_empty()) {
        if folder_alias(top).is_none() && !custom.iter().any(|c| slug_of(c) == slug_of(top)) {
            make_folder(db, top, "")?;
        }
    }
    let rows = folder_rows(db);
    let to = clean_topic_in(to, &custom_tops(&rows));
    if to == from || to.starts_with(&format!("{from}/")) { return Err("A folder cannot move into itself.".into()); }
    let mut moved = 0;
    for e in all(db) {
        let rest = if e.topic_path == from { Some("") } else { e.topic_path.strip_prefix(&format!("{from}/")) };
        if let Some(rest) = rest {
            let dest = if rest.is_empty() { to.clone() } else { format!("{to}/{rest}") };
            if update(db, &e.id, &json!({ "topic_path": dest }), "assistant", None, None).is_ok() { moved += 1; }
        }
    }
    let c = db.lock();
    for r in &rows {
        let rest = if r.path == from { Some("") } else { r.path.strip_prefix(&format!("{from}/")) };
        if let Some(rest) = rest {
            let dest = if rest.is_empty() { to.clone() } else { format!("{to}/{rest}") };
            let _ = c.execute("DELETE FROM memory_folders WHERE path = ?1", [&r.path]);
            let _ = c.execute("INSERT OR REPLACE INTO memory_folders (path, description, created) VALUES (?1, ?2, ?3)", params![dest, r.description, util::now_iso()]);
        }
    }
    Ok(moved)
}

/// One-time tidy: put entries saved under older free-form topics into the folders.
pub fn normalize_topics(db: &Db) {
    for e in all(db) {
        let fixed = topic_for(db, &e.topic_path, &e.title);
        if fixed != e.topic_path {
            let _ = update(db, &e.id, &json!({ "topic_path": fixed }), "core", None, None);
        }
    }
}

fn reindex(c: &rusqlite::Connection, e: &Entry) {
    let _ = c.execute("DELETE FROM memory_fts WHERE entry_id = ?1", [&e.id]);
    if e.status != "erased" {
        let _ = c.execute(
            "INSERT INTO memory_fts (entry_id, title, summary, detail, topic_path, tags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![e.id, e.title, e.summary, e.detail, e.topic_path, e.tags.join(" ")],
        );
    }
}

fn snapshot(c: &rusqlite::Connection, e: &Entry, by: &str) {
    let _ = c.execute(
        "INSERT OR REPLACE INTO memory_history (entry_id, version, snapshot, changed, by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![e.id, e.version, serde_json::to_string(e).unwrap_or_default(), util::now_iso(), by],
    );
}

/// New entry. `trust` must come from `trust_of_sources` or a user edit, never from the model.
pub fn create(db: &Db, mut e: Entry, trust: Trust, by: &str) -> Entry {
    e.id = util::new_id("m");
    e.title = util::clean_line(&e.title, 12);
    e.summary = util::clean_line(&e.summary, 60);
    e.topic_path = topic_for(db, &e.topic_path, &e.title);
    e.source_trust = trust.as_str().into();
    if e.status.is_empty() {
        e.status = "current".into();
    }
    if e.confidence <= 0.0 {
        e.confidence = trust.start_confidence();
    }
    e.confidence = e.confidence.min(trust.start_confidence());
    e.importance = if e.importance <= 0.0 { 0.5 } else { e.importance.clamp(0.0, 1.0) };
    e.created = util::now_iso();
    e.updated = util::unix_now();
    e.version = 1;
    let c = db.lock();
    let _ = c.execute(
        &format!("INSERT INTO memory ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)"),
        params![e.id, e.title, e.summary, e.detail, e.topic_path, json!(e.tags).to_string(), json!(e.people).to_string(),
            json!(e.places).to_string(), e.time_range, e.importance, e.confidence, e.source_trust, e.status,
            json!(e.sources).to_string(), json!(e.supersedes).to_string(), e.created, e.updated, e.last_used, e.version],
    );
    reindex(&c, &e);
    snapshot(&c, &e, by);
    e
}

pub enum UpdateError {
    NotFound,
    /// Someone changed it after `expect_updated`; the newer version wins (docs/04 Concurrency).
    Stale,
}

/// Apply a JSON patch of allowed fields. Trust may only be lowered here, never raised.
pub fn update(db: &Db, id: &str, patch: &Value, by: &str, expect_updated: Option<i64>, new_trust: Option<Trust>) -> Result<Entry, UpdateError> {
    let mut e = get(db, id).ok_or(UpdateError::NotFound)?;
    if let Some(x) = expect_updated
        && e.updated > x
    {
        return Err(UpdateError::Stale);
    }
    let s = |k: &str| patch.get(k).and_then(Value::as_str).map(String::from);
    let list = |k: &str| patch.get(k).and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok());
    if let Some(v) = s("title") { e.title = util::clean_line(&v, 12); }
    if let Some(v) = s("summary") { e.summary = util::clean_line(&v, 60); }
    if let Some(v) = s("detail") { e.detail = v; }
    if let Some(v) = s("topic_path") { e.topic_path = topic_for(db, &v, &e.title); }
    if let Some(v) = s("time_range") { e.time_range = Some(v); }
    if let Some(v) = s("status").filter(|v| ["current", "outdated", "disputed"].contains(&v.as_str())) { e.status = v; }
    if let Some(v) = list("tags") { e.tags = v; }
    if let Some(v) = list("people") { e.people = v; }
    if let Some(v) = list("places") { e.places = v; }
    if let Some(v) = list("supersedes") { e.supersedes = v; }
    if let Some(v) = list("sources") {
        for src in v {
            if !e.sources.contains(&src) { e.sources.push(src); }
        }
    }
    if let Some(v) = patch.get("importance").and_then(Value::as_f64) { e.importance = v.clamp(0.0, 1.0); }
    if let Some(t) = new_trust {
        // A user edit sets trust to user; anything else can only lower it.
        e.source_trust = if by == "user" { t.as_str().into() } else { Trust::parse(&e.source_trust).min(t).as_str().into() };
    }
    let cap = Trust::parse(&e.source_trust).start_confidence();
    if let Some(v) = patch.get("confidence").and_then(Value::as_f64) { e.confidence = v.clamp(0.0, 1.0); }
    if by != "user" { e.confidence = e.confidence.min(cap); }
    e.version += 1;
    e.updated = util::unix_now();
    let c = db.lock();
    let _ = c.execute(
        "UPDATE memory SET title=?2, summary=?3, detail=?4, topic_path=?5, tags=?6, people=?7, places=?8, time_range=?9,
         importance=?10, confidence=?11, source_trust=?12, status=?13, sources=?14, supersedes=?15, updated=?16, version=?17 WHERE id=?1",
        params![e.id, e.title, e.summary, e.detail, e.topic_path, json!(e.tags).to_string(), json!(e.people).to_string(),
            json!(e.places).to_string(), e.time_range, e.importance, e.confidence, e.source_trust, e.status,
            json!(e.sources).to_string(), json!(e.supersedes).to_string(), e.updated, e.version],
    );
    reindex(&c, &e);
    snapshot(&c, &e, by);
    Ok(e)
}

pub fn touch(db: &Db, id: &str) {
    let _ = db.lock().execute("UPDATE memory SET last_used = ?1 WHERE id = ?2", params![util::unix_now(), id]);
}

pub fn history(db: &Db, id: &str) -> Vec<Value> {
    let c = db.lock();
    let mut st = c.prepare("SELECT version, snapshot, changed, by FROM memory_history WHERE entry_id = ?1 ORDER BY version").expect("prepare");
    st.query_map([id], |r| {
        let snap: String = r.get(1)?;
        Ok(json!({ "version": r.get::<_, i64>(0)?, "entry": serde_json::from_str::<Value>(&snap).unwrap_or(Value::Null),
                   "changed": r.get::<_, String>(2)?, "by": r.get::<_, String>(3)? }))
    }).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Erase: remove the entry, keep raw sources, but mark them so dreaming will not rebuild it (docs/04).
pub fn erase(db: &Db, id: &str) -> bool {
    let Some(e) = get(db, id) else { return false };
    let c = db.lock();
    for s in &e.sources {
        let _ = c.execute("INSERT OR IGNORE INTO erased_sources (message_id, created) VALUES (?1, ?2)", params![s, util::now_iso()]);
    }
    let _ = c.execute("DELETE FROM memory_fts WHERE entry_id = ?1", [id]);
    let _ = c.execute("DELETE FROM memory WHERE id = ?1", [id]);
    let _ = c.execute("DELETE FROM memory_history WHERE entry_id = ?1", [id]);
    true
}

/// Full-text search. Each word is quoted, so user text cannot inject FTS syntax.
pub fn search(db: &Db, query: &str, limit: i64) -> Vec<Entry> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .take(12)
        .map(|w| format!("\"{}\"", w.to_lowercase()))
        .collect();
    if terms.is_empty() {
        return vec![];
    }
    let q = terms.join(" OR ");
    let ids: Vec<String> = {
        let c = db.lock();
        let mut st = c.prepare("SELECT entry_id FROM memory_fts WHERE memory_fts MATCH ?1 ORDER BY bm25(memory_fts) LIMIT ?2").expect("prepare");
        st.query_map(params![q, limit], |r| r.get(0)).map(|r| r.flatten().collect()).unwrap_or_default()
    };
    ids.iter().filter_map(|id| get(db, id)).filter(|e| e.status != "outdated").collect()
}

// ---------------- inbox ----------------

#[derive(Clone, Debug, Serialize)]
pub struct Note {
    pub id: String,
    pub text: String,
    pub topic: Option<String>,
    pub importance: Option<f64>,
    pub people: Vec<String>,
    pub trust: String,
    pub sources: Vec<String>,
    pub created: String,
}

#[allow(clippy::too_many_arguments)]
pub fn save_note(db: &Db, text: &str, topic: Option<&str>, importance: Option<f64>, people: &[String],
                 sources: &[String], thread: &str, turn: &str) -> (String, Trust) {
    let trust = trust_of_sources(db, sources).unwrap_or(Trust::Outside);
    let id = util::new_id("note");
    let _ = db.lock().execute(
        "INSERT INTO inbox (id, text, topic, importance, people, trust, sources, thread_id, turn_id, created) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![id, util::scrub(text, &[]), topic.map(|t| clean_topic_in(t, &[])), importance.map(|i| i.clamp(0.0, 1.0)),
            json!(people).to_string(), trust.as_str(), json!(sources).to_string(), thread, turn, util::now_iso()],
    );
    (id, trust)
}

pub fn pending_notes(db: &Db, limit: i64) -> Vec<Note> {
    let c = db.lock();
    let mut st = c.prepare("SELECT id, text, topic, importance, people, trust, sources, created FROM inbox WHERE status = 'pending' ORDER BY created LIMIT ?1").expect("prepare");
    st.query_map([limit], |r| {
        let p: String = r.get(4)?;
        let s: String = r.get(6)?;
        Ok(Note { id: r.get(0)?, text: r.get(1)?, topic: r.get(2)?, importance: r.get(3)?,
            people: serde_json::from_str(&p).unwrap_or_default(), trust: r.get(5)?,
            sources: serde_json::from_str(&s).unwrap_or_default(), created: r.get(7)? })
    }).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Pending inbox notes that mention a query word. Notes are not in the search index until
/// dreaming sorts them, so `search_memory` adds these to its answer.
pub fn search_pending_notes(db: &Db, query: &str, limit: usize) -> Vec<Note> {
    const SKIP: &[&str] = &["the", "and", "what", "whats", "who", "how", "when", "where", "which", "does", "did", "are", "was", "for", "you", "your", "with", "about", "that", "this", "have", "has"];
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.chars().count() >= 3 && !SKIP.contains(&t.as_str()))
        .collect();
    if terms.is_empty() {
        return vec![];
    }
    pending_notes(db, 500)
        .into_iter()
        .rev()
        .filter(|n| {
            let hay = format!("{} {}", n.text, n.topic.clone().unwrap_or_default()).to_lowercase();
            terms.iter().any(|t| hay.contains(t.as_str()))
        })
        .take(limit)
        .collect()
}

/// The model may fix or drop its own unsorted notes. Only `pending` inbox rows; nothing already in memory.
pub fn update_pending_note(db: &Db, id: &str, text: &str) -> bool {
    db.lock().execute("UPDATE inbox SET text = ?1 WHERE id = ?2 AND status = 'pending'", params![util::scrub(text, &[]), id]).map(|n| n > 0).unwrap_or(false)
}

pub fn drop_pending_note(db: &Db, id: &str) -> bool {
    db.lock().execute("UPDATE inbox SET status = 'dropped' WHERE id = ?1 AND status = 'pending'", [id]).map(|n| n > 0).unwrap_or(false)
}

/// Light digest of recent user chats: date, how many messages, the user's first and last words.
/// Built from the user's own messages only, so it carries no outside text. No model call.
pub fn recent_chats(db: &Db, exclude_thread: &str, days: i64, limit: i64) -> Vec<String> {
    let cutoff = util::iso(util::unix_now() - days * 86_400);
    let c = db.lock();
    let threads: Vec<(String, String, i64)> = c
        .prepare("SELECT thread_id, MAX(created), COUNT(*) FROM messages \
                  WHERE thread_id IN (SELECT id FROM threads WHERE kind = 'user' AND archived = 0) AND thread_id != ?1 AND created >= ?2 \
                  GROUP BY thread_id ORDER BY MAX(created) DESC LIMIT ?3")
        .and_then(|mut st| st.query_map(params![exclude_thread, cutoff, limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map(|x| x.flatten().collect()))
        .unwrap_or_default();
    let said = |thread: &str, order: &str| -> String {
        let sql = format!("SELECT body FROM messages WHERE thread_id = ?1 AND role = 'user' ORDER BY n {order} LIMIT 1");
        let body: Option<String> = c.query_row(&sql, [thread], |r| r.get(0)).ok();
        body.and_then(|b| serde_json::from_str::<Value>(&b).ok())
            .and_then(|v| v["content"].as_str().map(|s| util::clean_line(s, 18)))
            .unwrap_or_default()
    };
    threads
        .into_iter()
        .map(|(t, last, n)| {
            let (first, latest) = (said(&t, "ASC"), said(&t, "DESC"));
            let date = last.get(..10).unwrap_or(&last).to_string();
            if first == latest { format!("- {date} ({n} messages): you said \"{first}\"") } else { format!("- {date} ({n} messages): started with \"{first}\", ended with \"{latest}\"") }
        })
        .collect()
}

pub fn mark_notes_done(db: &Db, ids: &[String]) {
    let c = db.lock();
    for id in ids {
        let _ = c.execute("UPDATE inbox SET status = 'done' WHERE id = ?1", [id]);
    }
}

// ---------------- core facts and index ----------------

pub const CORE_WORD_CAP: usize = 800;

/// Where older versions kept the basic facts as a plain file. Only read once, to move it into the database.
pub fn core_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("memory").join("core.md")
}

/// The basic facts. They live in the encrypted database (`meta.memory_core`), never in a plain file.
pub fn read_core(db: &crate::db::Db) -> String {
    db.lock().query_row("SELECT value FROM meta WHERE key = 'memory_core'", [], |r| r.get(0)).unwrap_or_default()
}

/// Older versions wrote plain copies of memory to `<data>/memory/`. Move the basic facts into the database (when the
/// database has none yet) and delete the whole folder: the database is the source, the files were only a copy.
pub fn remove_plain_copies(db: &crate::db::Db, data_dir: &std::path::Path) {
    let dir = data_dir.join("memory");
    if !dir.exists() {
        return;
    }
    if read_core(db).trim().is_empty()
        && let Ok(old) = std::fs::read_to_string(core_path(data_dir))
    {
        let _ = write_core(db, &old);
    }
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => eprintln!("security: removed the plain memory copy at {}", dir.display()),
        Err(e) => eprintln!("security: could not remove the plain memory copy at {}: {e}", dir.display()),
    }
}

/// Write core facts, cut at the hard cap.
pub fn write_core(db: &crate::db::Db, text: &str) -> std::io::Result<()> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let body = if words.len() > CORE_WORD_CAP {
        // Keep whole lines while under the cap.
        let mut out = String::new();
        let mut n = 0;
        for line in text.lines() {
            let w = line.split_whitespace().count();
            if n + w > CORE_WORD_CAP { break; }
            n += w;
            out.push_str(line);
            out.push('\n');
        }
        out
    } else {
        text.to_string()
    };
    db.lock()
        .execute("INSERT OR REPLACE INTO meta (key, value) VALUES ('memory_core', ?1)", [body])
        .map(|_| ())
        .map_err(std::io::Error::other)
}

pub const TOP_LEVEL_LINES: usize = 80;

pub use crate::folders::{confidence_report, find_by_title, folder_tree, index_for, index_lines, index_lines_with, sync_folders};

#[cfg(test)]
mod tests {
    use super::*;

    fn e(topic: &str, tags: &[&str]) -> Entry {
        Entry { id: "m_1".into(), summary: "secret clinic visit".into(), topic_path: topic.into(),
                tags: tags.iter().map(|s| s.to_string()).collect(), status: "current".into(), ..Default::default() }
    }

    fn titled(topic: &str, title: &str, summary: &str) -> Entry {
        Entry { id: format!("m_{title}"), title: title.into(), summary: summary.into(), topic_path: topic.into(), status: "current".into(), ..Default::default() }
    }

    #[test]
    fn sensitive_lines_are_vague() {
        let l = index_lines(&[e("Other", &["sensitive"])], Some("Other"));
        assert!(l[0].contains("personal notes (sensitive)") && !l[0].contains("clinic"));
        let top = index_lines(&[e("Other", &["sensitive"])], None);
        assert!(top.iter().any(|x| x.contains("+1 private")) && !top.join("").contains("clinic"));
    }

    #[test]
    fn topics_land_in_the_five_folders() {
        assert_eq!(clean_topic("people/Sam Lee"), "People/sam-lee");
        assert_eq!(clean_topic("Person"), "People");
        assert_eq!(clean_topic("travel/Porto"), "Other/travel/porto");
        assert_eq!(clean_topic(""), "Other");
        assert_eq!(top_of("places/home"), "Places");
        assert_eq!(clean_topic("health/dentist"), "Other/health/dentist");
    }

    #[test]
    fn index_shows_folders_with_names_and_each_folder_lists_its_files() {
        let all = vec![
            titled("People", "Sam", "Close friend from university, works in Lisbon"),
            titled("People", "Mia", "Sister, lives in Porto"),
            titled("Places", "Home", "Flat in Lisbon"),
        ];
        let top = index_lines(&all, None);
        assert_eq!(top.len(), 5, "always the five folders");
        assert!(top[0].starts_with("- People/ (2 files: Sam, Mia)") && top[0].contains("(load: dir:People)"));
        assert!(top[1].contains("Home") && top[2] == "- Questions/ (empty)");
        let people = index_lines(&all, Some("people"));
        assert_eq!(people.len(), 2);
        assert!(people[0].contains("Sam: Close friend") && people[0].contains("(load: mem:m_Sam)"));
    }

    #[test]
    fn big_folders_are_capped() {
        let many: Vec<Entry> = (0..120).map(|i| titled("Things", &format!("t{i}"), "a thing")).collect();
        let inner = index_lines(&many, Some("Things"));
        assert_eq!(inner.len(), TOP_LEVEL_LINES + 1);
        assert!(inner.last().unwrap().contains("40 more"));
    }

    #[test]
    fn trust_order() {
        assert!(Trust::Outside < Trust::Inferred && Trust::Inferred < Trust::User);
        assert_eq!(Trust::from_message_label("outside"), Trust::Outside);
        assert_eq!(Trust::from_message_label("weird"), Trust::Outside);
    }
}
