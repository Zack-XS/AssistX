//! Search by meaning (docs/04, Search). Each memory entry gets a vector from a local embedding
//! model (any OpenAI-compatible `/v1/embeddings`, for example Ollama with `nomic-embed-text`).
//! Vectors are f32 little-endian BLOBs in `memory_vectors`; cosine is plain Rust.
//! `search_memory` merges FTS5 and vector hits with reciprocal rank fusion.
//!
//! Off when `AGENT_EMBED_MODEL` is unset. When the embedding server does not answer, search
//! falls back to FTS5 alone and the core tries again a few minutes later.

use crate::App;
use crate::db::Db;
use crate::memory::{self, Entry};
use crate::util;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS memory_vectors (
  entry_id TEXT PRIMARY KEY, model TEXT NOT NULL, dim INTEGER NOT NULL, vec BLOB NOT NULL, text_hash TEXT NOT NULL
);
"#;

/// Entries sent per embedding request, and batches per background pass.
const BATCH: usize = 32;
const BATCHES_PER_PASS: usize = 8;
/// After a failed call, leave the embedding server alone for this long.
const BACKOFF_SECS: i64 = 300;
/// RRF constant (the usual 60).
const RRF_K: f64 = 60.0;
/// Vector hits below this cosine are noise, not meaning.
const MIN_COSINE: f32 = 0.45;

/// Turns texts into vectors. `query` says whether the texts are search queries or documents.
pub trait Embedder: Send + Sync {
    fn model(&self) -> String;
    fn embed(&self, texts: &[String], query: bool) -> Result<Vec<Vec<f32>>, String>;
}

/// An OpenAI-compatible embeddings endpoint on this computer.
pub struct HttpEmbedder {
    url: String,
    model: String,
    agent: ureq::Agent,
}

impl HttpEmbedder {
    pub fn new(base_url: &str, model: &str) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).build().into();
        HttpEmbedder { url: format!("{}/embeddings", base_url.trim_end_matches('/')), model: model.to_string(), agent }
    }
}

/// Models trained with task prefixes (nomic) want them; others do not.
fn prefixed(model: &str, text: &str, query: bool) -> String {
    if model.contains("nomic") {
        format!("{}{text}", if query { "search_query: " } else { "search_document: " })
    } else {
        text.to_string()
    }
}

impl Embedder for HttpEmbedder {
    fn model(&self) -> String {
        self.model.clone()
    }
    fn embed(&self, texts: &[String], query: bool) -> Result<Vec<Vec<f32>>, String> {
        let input: Vec<String> = texts.iter().map(|t| prefixed(&self.model, t, query)).collect();
        let body = json!({ "model": self.model, "input": input });
        let mut resp = self.agent.post(&self.url).header("Content-Type", "application/json").send(body.to_string()).map_err(|e| format!("embedding server: {e}"))?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().map_err(|e| format!("embedding server: {e}"))?;
        if status != 200 {
            return Err(format!("embedding server answered {status}"));
        }
        parse_embeddings(&text, texts.len())
    }
}

/// `{"data":[{"index":0,"embedding":[...]},...]}` in input order.
pub fn parse_embeddings(text: &str, n: usize) -> Result<Vec<Vec<f32>>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("embedding reply: {e}"))?;
    let mut rows: Vec<(usize, Vec<f32>)> = v["data"].as_array().ok_or("embedding reply has no data")?.iter().enumerate()
        .map(|(i, d)| (d["index"].as_u64().map(|x| x as usize).unwrap_or(i),
            d["embedding"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect()).unwrap_or_default()))
        .collect();
    rows.sort_by_key(|r| r.0);
    if rows.len() != n || rows.iter().any(|r| r.1.is_empty()) {
        return Err("embedding reply has the wrong number of vectors".into());
    }
    Ok(rows.into_iter().map(|r| r.1).collect())
}

/// Only a loopback address: memory text never leaves this computer for embedding.
fn loopback(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or("");
    let host = rest.split(['/', '?']).next().unwrap_or("");
    let name = if host.starts_with('[') { format!("{}]", host.split(']').next().unwrap_or("")) } else { host.split(':').next().unwrap_or("").to_string() };
    name == "localhost" || name == "[::1]" || (name.starts_with("127.") && name.split('.').all(|p| p.parse::<u8>().is_ok()))
}

/// The embedder from `AGENT_EMBED_MODEL` and `AGENT_EMBED_URL` (default the local Ollama), or none.
pub fn global() -> Option<Arc<dyn Embedder>> {
    static E: OnceLock<Option<Arc<dyn Embedder>>> = OnceLock::new();
    E.get_or_init(|| {
        let model = std::env::var("AGENT_EMBED_MODEL").ok().map(|m| m.trim().to_string()).filter(|m| !m.is_empty())?;
        let url = std::env::var("AGENT_EMBED_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| "http://127.0.0.1:11434/v1".into());
        if !loopback(url.trim()) {
            eprintln!("AGENT_EMBED_URL must be a loopback http address; search by meaning is off");
            return None;
        }
        Some(Arc::new(HttpEmbedder::new(url.trim(), &model)) as Arc<dyn Embedder>)
    }).clone()
}

// ---------------- vectors ----------------

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na.sqrt() * nb.sqrt()) }
}

/// What gets embedded for an entry. Sensitive entries too: the vector stays on this computer.
fn entry_text(e: &Entry) -> String {
    let detail: String = e.detail.chars().take(1500).collect();
    format!("{}\n{}\n{}\n{} {}", e.title, e.summary, detail, e.topic_path, e.tags.join(" "))
}

fn text_hash(text: &str) -> String {
    util::sha256_hex(text.as_bytes())[..16].to_string()
}

/// Entries whose vector is missing, from another model, or older than their text.
pub fn stale(db: &Db, model: &str, limit: usize) -> Vec<(Entry, String)> {
    let have: HashMap<String, (String, String)> = {
        let c = db.lock();
        c.prepare("SELECT entry_id, model, text_hash FROM memory_vectors")
            .and_then(|mut st| st.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?)))).map(|x| x.flatten().collect()))
            .unwrap_or_default()
    };
    memory::all_current(db)
        .into_iter()
        .filter_map(|e| {
            let text = entry_text(&e);
            let h = text_hash(&text);
            match have.get(&e.id) {
                Some((m, old)) if m == model && *old == h => None,
                _ => Some((e, text)),
            }
        })
        .take(limit)
        .collect()
}

/// Embed up to one pass worth of stale entries. Returns how many were stored.
pub fn embed_pending(db: &Db, emb: &dyn Embedder) -> Result<usize, String> {
    let model = emb.model();
    // Vectors of entries that no longer exist (erased, forgotten) go.
    let _ = db.lock().execute("DELETE FROM memory_vectors WHERE entry_id NOT IN (SELECT id FROM memory)", []);
    let todo = stale(db, &model, BATCH * BATCHES_PER_PASS);
    let mut done = 0;
    for chunk in todo.chunks(BATCH) {
        let texts: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();
        let vecs = emb.embed(&texts, false)?;
        let c = db.lock();
        for ((e, t), v) in chunk.iter().zip(vecs) {
            let _ = c.execute(
                "INSERT OR REPLACE INTO memory_vectors (entry_id, model, dim, vec, text_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![e.id, model, v.len() as i64, to_blob(&v), text_hash(t)],
            );
            done += 1;
        }
    }
    Ok(done)
}

/// Entries ranked by cosine to the query vector (only vectors of the current model).
pub fn nearest(db: &Db, model: &str, q: &[f32], limit: usize) -> Vec<(String, f32)> {
    let rows: Vec<(String, Vec<u8>)> = {
        let c = db.lock();
        c.prepare("SELECT v.entry_id, v.vec FROM memory_vectors v JOIN memory m ON m.id = v.entry_id WHERE v.model = ?1 AND v.dim = ?2 AND m.status IN ('current', 'disputed')")
            .and_then(|mut st| st.query_map(params![model, q.len() as i64], |r| Ok((r.get(0)?, r.get(1)?))).map(|x| x.flatten().collect()))
            .unwrap_or_default()
    };
    let mut scored: Vec<(String, f32)> = rows.into_iter().map(|(id, b)| (id, cosine(q, &from_blob(&b)))).filter(|(_, s)| *s >= MIN_COSINE).collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

/// Reciprocal rank fusion of ranked id lists.
pub fn rrf(lists: &[Vec<String>], limit: usize) -> Vec<String> {
    let mut score: HashMap<&str, f64> = HashMap::new();
    let mut first: HashMap<&str, usize> = HashMap::new();
    let mut n = 0;
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            *score.entry(id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
            first.entry(id).or_insert_with(|| { n += 1; n });
        }
    }
    let mut ids: Vec<&str> = score.keys().copied().collect();
    ids.sort_by(|a, b| score[b].partial_cmp(&score[a]).unwrap_or(std::cmp::Ordering::Equal).then(first[a].cmp(&first[b])));
    ids.into_iter().take(limit).map(String::from).collect()
}

/// FTS5 plus vectors, merged with RRF. With no embedder (or a failed query) this is FTS5 alone.
pub fn hybrid(db: &Db, emb: Option<&dyn Embedder>, query: &str, limit: i64) -> Result<Vec<Entry>, (Vec<Entry>, String)> {
    let fts = memory::search(db, query, limit * 3);
    let Some(emb) = emb else { return Ok(fts.into_iter().take(limit as usize).collect()) };
    if query.trim().is_empty() {
        return Ok(fts);
    }
    let q = match emb.embed(&[query.chars().take(1000).collect()], true) {
        Ok(mut v) if !v.is_empty() => v.remove(0),
        Ok(_) => return Err((fts.into_iter().take(limit as usize).collect(), "empty embedding".into())),
        Err(e) => return Err((fts.into_iter().take(limit as usize).collect(), e)),
    };
    let near = nearest(db, &emb.model(), &q, (limit * 3) as usize);
    let mut by_id: HashMap<String, Entry> = fts.iter().map(|e| (e.id.clone(), e.clone())).collect();
    let fused = rrf(&[fts.iter().map(|e| e.id.clone()).collect(), near.iter().map(|n| n.0.clone()).collect()], limit as usize);
    Ok(fused.into_iter().filter_map(|id| by_id.remove(&id).or_else(|| memory::get(db, &id))).filter(|e| e.status != "outdated").collect())
}

// ---------------- in the app ----------------

fn backing_off(app: &App) -> bool {
    util::unix_now() < app.mark("embed_backoff")
}

/// `search_memory` for the model and the API. Blocking: call from a blocking thread.
pub fn search(app: &App, query: &str, limit: i64) -> Vec<Entry> {
    let emb = global().filter(|_| !backing_off(app));
    match hybrid(&app.db, emb.as_deref(), query, limit) {
        Ok(v) => v,
        Err((v, _)) => {
            app.set_mark("embed_backoff", util::unix_now() + BACKOFF_SECS);
            v
        }
    }
}

/// Called by the core timer: embed new and changed entries (and backfill old ones) in the background.
pub fn maybe_work(app: &Arc<App>) {
    let Some(emb) = global() else { return };
    if backing_off(app) || app.mark("embed_busy") == 1 {
        return;
    }
    app.set_mark("embed_busy", 1);
    let app = app.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = embed_pending(&app.db, emb.as_ref()) {
            eprintln!("search by meaning: {e} (trying again in {} minutes)", BACKOFF_SECS / 60);
            app.set_mark("embed_backoff", util::unix_now() + BACKOFF_SECS);
        }
        app.set_mark("embed_busy", 0);
    });
}

/// For `GET /v1/status`-style reports: model, how many entries have a vector, how many wait.
pub fn status(app: &App) -> Value {
    let Some(emb) = global() else { return json!({ "on": false }) };
    let model = emb.model();
    let have: i64 = app.db.lock().query_row("SELECT COUNT(*) FROM memory_vectors WHERE model = ?1", [&model], |r| r.get(0)).unwrap_or(0);
    json!({ "on": true, "model": model, "vectors": have, "waiting": stale(&app.db, &model, 10_000).len(), "backing_off": backing_off(app) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Trust, create};
    use std::sync::Mutex;

    /// Words to fixed axes, so "car" and "vehicle" land near each other without a model.
    struct Fake {
        model: String,
        calls: Mutex<usize>,
        fail: bool,
    }

    impl Fake {
        fn new(model: &str) -> Self {
            Fake { model: model.into(), calls: Mutex::new(0), fail: false }
        }
    }

    impl Embedder for Fake {
        fn model(&self) -> String {
            self.model.clone()
        }
        fn embed(&self, texts: &[String], _query: bool) -> Result<Vec<Vec<f32>>, String> {
            *self.calls.lock().unwrap() += 1;
            if self.fail {
                return Err("down".into());
            }
            Ok(texts.iter().map(|t| {
                let t = t.to_lowercase();
                let axis = |words: &[&str]| if words.iter().any(|w| t.contains(w)) { 1.0 } else { 0.0 };
                vec![axis(&["car", "vehicle", "drive"]), axis(&["dog", "puppy", "pet"]), axis(&["piano", "music"]), 0.1]
            }).collect())
        }
    }

    fn entry(db: &Db, title: &str, summary: &str) -> Entry {
        create(db, Entry { title: title.into(), summary: summary.into(), topic_path: "Things".into(), ..Default::default() }, Trust::User, "test")
    }

    #[test]
    fn blobs_round_trip_and_cosine() {
        let v = vec![0.5f32, -1.25, 3.0];
        assert_eq!(from_blob(&to_blob(&v)), v);
        assert!((cosine(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0, "different sizes never match");
    }

    #[test]
    fn rrf_rewards_agreement() {
        let a = vec!["x".to_string(), "y".into(), "z".into()];
        let b = vec!["z".to_string(), "w".into()];
        let out = rrf(&[a, b], 10);
        assert_eq!(out[0], "z", "in both lists beats first in one");
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn parses_openai_reply_in_index_order() {
        let v = parse_embeddings(r#"{"data":[{"index":1,"embedding":[2.0]},{"index":0,"embedding":[1.0]}]}"#, 2).unwrap();
        assert_eq!(v, vec![vec![1.0], vec![2.0]]);
        assert!(parse_embeddings(r#"{"data":[]}"#, 1).is_err());
    }

    #[test]
    fn only_loopback_addresses() {
        assert!(loopback("http://127.0.0.1:11434/v1") && loopback("http://localhost:11434/v1") && loopback("http://[::1]:11434/v1"));
        assert!(!loopback("https://api.example.com/v1") && !loopback("http://192.168.1.4:11434/v1") && !loopback("http://localhost.evil.com/v1"));
    }

    #[test]
    fn hybrid_finds_by_meaning_and_backfills_and_reembeds() {
        let db = Db::open_memory().unwrap();
        let car = entry(&db, "Red hatchback", "The user drives a red car to work");
        let dog = entry(&db, "Rex", "Their puppy, a beagle");
        let fake = Fake::new("fake-1");
        assert_eq!(embed_pending(&db, &fake).unwrap(), 2, "backfill both");
        assert_eq!(embed_pending(&db, &fake).unwrap(), 0, "nothing left");
        // "vehicle" is not a word in either entry: FTS alone finds nothing, the vector finds the car.
        assert!(memory::search(&db, "vehicle", 10).is_empty());
        let hits = hybrid(&db, Some(&fake as &dyn Embedder), "vehicle", 10).ok().unwrap();
        assert_eq!(hits.first().map(|e| e.id.clone()), Some(car.id.clone()));
        assert!(!hits.iter().any(|e| e.id == dog.id), "unrelated entries stay out");
        // A changed entry is embedded again; a new model re-embeds everything.
        memory::update(&db, &dog.id, &json!({ "summary": "Their pet dog moved to a farm" }), "user", None, None).ok().unwrap();
        assert_eq!(embed_pending(&db, &fake).unwrap(), 1);
        assert_eq!(embed_pending(&db, &Fake::new("fake-2")).unwrap(), 2);
        // An erased entry loses its vector.
        memory::erase(&db, &car.id);
        embed_pending(&db, &Fake::new("fake-2")).unwrap();
        let n: i64 = db.lock().query_row("SELECT COUNT(*) FROM memory_vectors", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_down_embedder_falls_back_to_fts() {
        let db = Db::open_memory().unwrap();
        let e = entry(&db, "Piano lessons", "Tuesdays at five");
        let down = Fake { fail: true, ..Fake::new("x") };
        assert!(embed_pending(&db, &down).is_err());
        let (hits, _) = hybrid(&db, Some(&down as &dyn Embedder), "piano", 5).err().unwrap();
        assert_eq!(hits[0].id, e.id);
        assert_eq!(hybrid(&db, None, "piano", 5).ok().unwrap()[0].id, e.id);
    }
}
