//! Core of a local, model-agnostic personal assistant backend. No UI. See docs/.
#![forbid(unsafe_code)]

pub mod ambient;
pub mod audit;
pub mod autoreport;
pub mod eval;
pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod dreaming;
pub mod embed;
pub mod engine;
pub mod events;
pub mod export;
pub mod folders;
pub mod gate;
pub mod manifest;
pub mod memory;
pub mod mind;
pub mod model;
pub mod oauth;
pub mod openai_compat;
pub mod plugins;
pub mod power;
pub mod prompt;
pub mod calendar;
pub mod lists;
pub mod lumo;
pub mod location;
pub mod health;
pub mod detect;
pub mod phone;
pub mod hub;
pub mod hub_http;
pub mod nodes;
pub mod models;
pub mod proxy;
pub mod runner;
pub mod schedule_import;
pub mod sandbox;
pub mod remember;
pub mod links;
pub mod recipes;
pub mod reports;
pub mod scheduler;
pub mod signals;
pub mod sounds;
pub mod spotify;
pub mod google_health;
pub mod hevy;
pub mod email;
pub mod schema;
pub mod status;
pub mod store;
pub mod summary;
pub mod travel;
pub mod tz;
pub mod usage;
pub mod util;
pub mod vault;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{Notify, mpsc, oneshot};

/// Tables owned by modules other than db/memory.
pub fn extra_schema() -> &'static str {
    r#"
CREATE TABLE IF NOT EXISTS oauth_pending (state TEXT PRIMARY KEY, provider TEXT NOT NULL, verifier TEXT NOT NULL, redirect_uri TEXT NOT NULL, client_id TEXT NOT NULL, created INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS plugin_state (
  id TEXT PRIMARY KEY, status TEXT NOT NULL, version TEXT NOT NULL, hash TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'local', url TEXT, sha256 TEXT, installed_at TEXT, signed_by TEXT,
  created_by TEXT NOT NULL DEFAULT 'human'
);
CREATE TABLE IF NOT EXISTS secrets (name TEXT PRIMARY KEY, value TEXT NOT NULL, updated TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS user_autonomy (plugin TEXT NOT NULL, tool TEXT NOT NULL, level TEXT NOT NULL, spend_cap REAL, PRIMARY KEY (plugin, tool));
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS memory_allow (entry_id TEXT NOT NULL, thread_id TEXT NOT NULL, PRIMARY KEY (entry_id, thread_id));
CREATE TABLE IF NOT EXISTS build_log (n INTEGER PRIMARY KEY AUTOINCREMENT, plugin TEXT NOT NULL, url TEXT NOT NULL, status INTEGER, ts INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS jobs (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, text TEXT NOT NULL, schedule TEXT NOT NULL, timezone TEXT NOT NULL,
  next_run INTEGER, last_run INTEGER, outside INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'active',
  created TEXT NOT NULL, created_by TEXT NOT NULL, origin_thread TEXT
);
CREATE TABLE IF NOT EXISTS notices (n INTEGER PRIMARY KEY AUTOINCREMENT, priority TEXT NOT NULL, text TEXT NOT NULL, ts INTEGER NOT NULL, delivered INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS dreaming_runs (
  id TEXT PRIMARY KEY, dry_run INTEGER NOT NULL, status TEXT NOT NULL, started TEXT NOT NULL, finished TEXT,
  changes TEXT NOT NULL DEFAULT '[]', questions TEXT NOT NULL DEFAULT '[]', snapshot_at INTEGER NOT NULL, turn_id TEXT
);
"#
}

/// Stops a turn between steps (docs/01, Sending a message).
#[derive(Default)]
pub struct Cancel {
    flag: AtomicBool,
    notify: Notify,
}

impl Cancel {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
    pub async fn cancelled(&self) {
        loop {
            let n = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            n.await;
        }
    }
}

/// What a user said on an approval card.
#[derive(Debug, Clone)]
pub enum Answer {
    Yes,
    No(Option<String>),
    Edit(serde_json::Value),
    /// Typed or chosen reply to an `ask_user` question card.
    Reply(String),
    Expired,
}

pub struct App {
    pub cfg: config::Config,
    pub db: Arc<db::Db>,
    pub feed: events::Feed,
    pub registry: plugins::Registry,
    /// Live, valid manifests by id (status comes from `plugin_state`).
    manifests: RwLock<BTreeMap<String, manifest::Manifest>>,
    manifest_problems: RwLock<Vec<String>>,
    pub model: Arc<dyn model::Adapter>,
    pub prompts: prompt::Prompts,
    workers: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
    running: Mutex<HashMap<String, (String, Arc<Cancel>)>>,
    queued: Mutex<HashMap<String, usize>>,
    waiters: Mutex<HashMap<String, oneshot::Sender<Answer>>>,
    /// thread id -> loaded item ids (`plugin:x`, `mem:y`), most recently used last.
    loaded: Mutex<HashMap<String, Vec<String>>>,
    rate: Mutex<HashMap<String, (i64, u32)>>,
    /// Memory part of the index: (built at, lines, soft dirty).
    index_cache: Mutex<(i64, String, bool)>,
    /// Memory changed: the folder copy on disk needs a refresh (done by the core timer).
    files_dirty: AtomicBool,
    /// Last known "what is playing" line and when it was seen (plugin `spotify`). Never fetched per turn.
    pub music: Mutex<Option<(i64, String)>>,
    /// Connected computers and their work queues (plugin `computers`).
    pub nodes: nodes::Hub,
    /// Picks a model per turn (docs/12). Off in tests, which give the App one mock model.
    pub models: models::Router,
    pub routed: AtomicBool,
    /// The master switch, 0 = not read yet, 1 = on, 2 = off (src/power.rs).
    pub power_state: std::sync::atomic::AtomicU8,
    /// When each periodic check last ran (per account, so accounts never skip each other's turn): see `due`.
    clock: Mutex<HashMap<&'static str, i64>>,
    /// What the health watch saw down at its last check (src/status.rs).
    pub down: Mutex<Option<std::collections::BTreeSet<String>>>,
    /// Set when the account is locked or removed: its background loop ends.
    pub shutdown: AtomicBool,
}

pub const MAX_LOADED_ITEMS: usize = 8;
/// New memory entries reach the index at most this often (docs/02), unless forced.
pub const INDEX_REBUILD_SECS: i64 = 600;

impl App {
    pub fn new(cfg: config::Config, db: db::Db, model: Arc<dyn model::Adapter>) -> Arc<App> {
        let db = Arc::new(db);
        let registry = plugins::Registry::new(&cfg);
        let prompts = prompt::Prompts::load(&cfg.prompts_dir);
        let app = Arc::new(App {
            feed: events::Feed::new(db.clone()),
            db,
            registry,
            manifests: Default::default(),
            manifest_problems: Default::default(),
            model,
            prompts,
            cfg,
            workers: Default::default(),
            running: Default::default(),
            queued: Default::default(),
            waiters: Default::default(),
            loaded: Default::default(),
            rate: Default::default(),
            index_cache: Mutex::new((0, String::new(), true)),
            files_dirty: AtomicBool::new(true),
            music: Mutex::new(None),
            nodes: Default::default(),
            models: Default::default(),
            routed: AtomicBool::new(false),
            power_state: std::sync::atomic::AtomicU8::new(0),
            clock: Default::default(),
            down: Mutex::new(None),
            shutdown: AtomicBool::new(false),
        });
        memory::normalize_topics(&app.db);
        // Which model wrote each reply (shown under a message on the page).
        let _ = app.db.lock().execute("ALTER TABLE messages ADD COLUMN model TEXT", []);
        scheduler::migrate(&app.db);
        usage::migrate(&app.db);
        // A chat may pick its own model (docs/12); empty = the default.
        let _ = app.db.lock().execute("ALTER TABLE threads ADD COLUMN model TEXT", []);
        // Tokens a cloud model used in a turn; only these count toward the daily cap.
        let _ = app.db.lock().execute("ALTER TABLE turns ADD COLUMN cloud_tokens INTEGER NOT NULL DEFAULT 0", []);
        // A restart ends every turn that was still going: nothing runs it any more, and a client
        // waiting on it must not wait forever. Its cards stay, so a late yes still works.
        let _ = app.db.lock().execute(
            "UPDATE turns SET status = 'error', error_code = 'server_restarted', error_message = 'The server restarted before this finished. Please ask again.', finished = ?1 WHERE status IN ('queued', 'running', 'waiting')",
            [util::now_iso()],
        );
        app.reload_plugins();
        app
    }

    /// Model keys that must never appear in logs, history or tool output.
    pub fn secret_words(&self) -> Vec<String> {
        let mut k: Vec<String> = self.cfg.model.api_key.iter().cloned().collect();
        if self.is_routed() {
            k.extend(models::all_keys(self));
        }
        k
    }

    pub fn is_routed(&self) -> bool {
        self.routed.load(Ordering::Relaxed)
    }

    /// Re-read plugins from disk and reconcile with `plugin_state`.
    pub fn reload_plugins(&self) {
        let (found, problems) = store::reconcile(self);
        *self.manifests.write().unwrap() = found;
        *self.manifest_problems.write().unwrap() = problems;
        self.force_index();
    }

    pub fn manifest(&self, id: &str) -> Option<manifest::Manifest> {
        self.manifests.read().unwrap().get(id).cloned()
    }

    pub fn manifests(&self) -> BTreeMap<String, manifest::Manifest> {
        self.manifests.read().unwrap().clone()
    }

    pub fn manifest_problems(&self) -> Vec<String> {
        self.manifest_problems.read().unwrap().clone()
    }

    /// Active, valid and backed by code (core, Rust or a script command).
    pub fn plugin_usable(&self, id: &str) -> bool {
        let Some(m) = self.manifest(id) else { return false };
        store::status(self, id).as_deref() == Some("active") && runner::backend(self, &m).is_some()
    }

    pub fn loaded_items(&self, thread: &str) -> Vec<String> {
        self.loaded.lock().unwrap().get(thread).cloned().unwrap_or_default()
    }

    pub fn loaded_plugins(&self, thread: &str) -> Vec<String> {
        self.loaded_items(thread).iter().filter_map(|i| i.strip_prefix("plugin:").map(String::from)).collect()
    }

    /// Load or touch an item. Returns items pushed out, least recently used first (docs/02 budget).
    pub fn mark_loaded(&self, thread: &str, item: &str, pinned: &HashSet<String>) -> Vec<String> {
        let mut map = self.loaded.lock().unwrap();
        let list = map.entry(thread.to_string()).or_default();
        list.retain(|p| p != item);
        list.push(item.to_string());
        let mut evicted = vec![];
        while list.len() > MAX_LOADED_ITEMS {
            match list.iter().position(|p| !pinned.contains(p) && p != item) {
                Some(i) => evicted.push(list.remove(i)),
                None => break,
            }
        }
        evicted
    }

    pub fn unload(&self, thread: &str, item: &str) -> bool {
        let mut map = self.loaded.lock().unwrap();
        map.get_mut(thread)
            .map(|l| {
                let before = l.len();
                l.retain(|p| p != item);
                l.len() != before
            })
            .unwrap_or(false)
    }

    /// Drop an item from every thread (plugin disabled, entry forgotten).
    pub fn unload_everywhere(&self, item: &str) {
        for l in self.loaded.lock().unwrap().values_mut() {
            l.retain(|p| p != item);
        }
    }

    /// New entries: picked up at the next batched rebuild.
    pub fn soft_dirty_index(&self) {
        self.index_cache.lock().unwrap().2 = true;
        self.files_dirty.store(true, Ordering::Relaxed);
    }

    /// Refresh the readable copy of memory (`data/memory/<Folder>/*.md`) if anything changed.
    pub fn sync_memory_files(&self) {
        if self.files_dirty.swap(false, Ordering::Relaxed) {
            memory::sync_folders(&self.cfg.data_dir, &memory::all_current(&self.db), &memory::folder_rows(&self.db));
            // The basic-facts file is built from the entries by code, so keep it in step with them.
            let _ = memory::write_core(&self.cfg.data_dir, &dreaming::build_core(self));
        }
    }

    /// Dreaming or a plugin change: rebuild on next use.
    pub fn force_index(&self) {
        self.files_dirty.store(true, Ordering::Relaxed);
        let mut c = self.index_cache.lock().unwrap();
        c.0 = 0;
        c.2 = true;
    }

    pub fn memory_index(&self) -> String {
        let now = util::unix_now();
        let mut c = self.index_cache.lock().unwrap();
        if c.2 && (c.0 == 0 || now - c.0 >= INDEX_REBUILD_SECS) {
            let entries = memory::all_current(&self.db);
            c.1 = memory::index_for(&self.db, &entries, None).join("\n");
            c.0 = now;
            c.2 = false;
        }
        c.1.clone()
    }

    pub fn enqueue(self: &Arc<Self>, thread: &str, turn_id: &str) -> bool {
        let busy = self.running.lock().unwrap().contains_key(thread)
            || self.queued.lock().unwrap().get(thread).copied().unwrap_or(0) > 0;
        *self.queued.lock().unwrap().entry(thread.to_string()).or_default() += 1;
        let mut workers = self.workers.lock().unwrap();
        let tx = workers.entry(thread.to_string()).or_insert_with(|| {
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            let app = self.clone();
            let thread = thread.to_string();
            tokio::spawn(async move {
                while let Some(turn) = rx.recv().await {
                    if let Some(q) = app.queued.lock().unwrap().get_mut(&thread) {
                        *q = q.saturating_sub(1);
                    }
                    engine::run_turn(app.clone(), &thread, &turn).await;
                }
            });
            tx
        });
        let _ = tx.send(turn_id.to_string());
        busy
    }

    pub fn set_running(&self, thread: &str, turn: &str) -> Arc<Cancel> {
        let c = Arc::new(Cancel::default());
        self.running.lock().unwrap().insert(thread.to_string(), (turn.to_string(), c.clone()));
        c
    }

    pub fn clear_running(&self, thread: &str) {
        self.running.lock().unwrap().remove(thread);
    }

    pub fn is_busy(&self, thread: &str) -> bool {
        self.running.lock().unwrap().contains_key(thread) || self.queued.lock().unwrap().get(thread).copied().unwrap_or(0) > 0
    }

    pub fn cancel_running(&self, thread: &str) -> Option<String> {
        let r = self.running.lock().unwrap();
        r.get(thread).map(|(id, c)| {
            c.cancel();
            id.clone()
        })
    }

    pub fn add_waiter(&self, approval: &str) -> oneshot::Receiver<Answer> {
        let (tx, rx) = oneshot::channel();
        self.waiters.lock().unwrap().insert(approval.to_string(), tx);
        rx
    }

    pub fn take_waiter(&self, approval: &str) -> Option<oneshot::Sender<Answer>> {
        self.waiters.lock().unwrap().remove(approval)
    }

    /// Per-key, per-minute limit (docs/01, Rate limits).
    pub fn rate_ok(&self, key: &str, per_minute: u32) -> bool {
        let minute = util::unix_now() / 60;
        let mut m = self.rate.lock().unwrap();
        let e = m.entry(key.to_string()).or_insert((minute, 0));
        if e.0 != minute {
            *e = (minute, 0);
        }
        e.1 += 1;
        e.1 <= per_minute
    }

    /// Create a system or user thread if missing (system threads have fixed ids).
    pub fn ensure_thread(&self, id: &str, kind: &str, title: &str) {
        let _ = self.db.lock().execute(
            "INSERT OR IGNORE INTO threads (id, title, kind, created) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, title, kind, util::now_iso()],
        );
    }

    /// Queue a new turn of any source. Returns its id.
    pub fn start_turn(self: &Arc<Self>, thread: &str, source: &str, input: serde_json::Value) -> String {
        let turn = util::new_id("turn");
        let _ = self.db.lock().execute(
            "INSERT INTO turns (id, thread_id, source, status, input, created) VALUES (?1, ?2, ?3, 'queued', ?4, ?5)",
            rusqlite::params![turn, thread, source, input.to_string(), util::now_iso()],
        );
        if self.enqueue(thread, &turn) {
            self.feed.emit(Some(thread), Some(&turn), "turn.queued", serde_json::json!({}));
        }
        turn
    }

    /// Background jobs: expire approvals, run due jobs, prune events.
    /// True at most once every `secs` seconds for this `key`. The first call is due.
    pub fn due(&self, key: &'static str, secs: i64) -> bool {
        let now = util::unix_now();
        let mut c = self.clock.lock().unwrap();
        if c.get(key).is_some_and(|t| now - t < secs) {
            return false;
        }
        c.insert(key, now);
        true
    }

    /// The last mark for a key without changing it (0 = never).
    pub fn mark(&self, key: &'static str) -> i64 {
        self.clock.lock().unwrap().get(key).copied().unwrap_or(0)
    }

    pub fn set_mark(&self, key: &'static str, v: i64) -> i64 {
        self.clock.lock().unwrap().insert(key, v).unwrap_or(-1)
    }

    pub fn spawn_housekeeping(self: &Arc<Self>) {
        let app = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
            let mut n: u64 = 0;
            loop {
                tick.tick().await;
                if app.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                engine::expire_approvals(&app);
                scheduler::tick(&app);
                n += 1;
                if n.is_multiple_of(240) {
                    app.feed.prune(app.cfg.limits.event_retention_secs);
                }
            }
        });
    }
}

/// The model adapter the config asks for (each account gets its own).
pub fn make_model(cfg: &config::Config) -> Result<Arc<dyn model::Adapter>, String> {
    Ok(match cfg.model.kind.as_str() {
        "mock" => {
            let path = std::env::var("AGENT_MOCK_SCRIPT").unwrap_or_default();
            if path.is_empty() { Arc::new(model::Mock::new(&[])) } else { Arc::new(model::Mock::from_file(&path)?) }
        }
        "openai" => Arc::new(openai_compat::OpenAiCompat::new(&cfg.model.base_url, &cfg.model.name, cfg.model.api_key.clone())),
        other => return Err(format!("unknown AGENT_MODEL_KIND '{other}' (use openai or mock)")),
    })
}

/// Build the app from config: open the DB, make the first owner token, pick the model.
pub fn build(cfg: config::Config) -> Result<(Arc<App>, Option<String>), String> {
    std::fs::create_dir_all(&cfg.data_dir).map_err(|e| format!("data dir: {e}"))?;
    let db = db::Db::open(&cfg.data_dir.join("core.db")).map_err(|e| format!("database: {e}"))?;
    let model = make_model(&cfg)?;
    let token_path = cfg.data_dir.join("owner.token");
    let routed = cfg.model.kind == "openai";
    let app = App::new(cfg, db, model);
    app.routed.store(routed, Ordering::Relaxed);
    let made = auth::ensure_owner_token(&app.db, &token_path).map_err(|e| format!("owner token: {e}"))?;
    scheduler::ensure_system_jobs(&app);
    scheduler::ensure_reports(&app);
    mind::ensure(&app);
    Ok((app, made.map(|_| token_path.display().to_string())))
}
