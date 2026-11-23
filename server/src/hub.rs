//! Accounts (docs/15). One server, many people, each with their own assistant.
//!
//! The server token (the "server password") only lets a phone talk to this server. To get in, a
//! person signs in to an account with a username and passphrase. Each account is a complete,
//! separate `App`: its own encrypted database (keyed with its own data key), memory, mind,
//! jobs, settings, models, tokens, logs and folders. Nothing is shared between accounts.
//!
//! An account's data key exists only wrapped: once by the passphrase (Argon2id), once by the
//! recovery key shown at sign-up. Signing in unwraps it into memory; it is never written to disk
//! unwrapped. It stays there until the server restarts or the account is locked, so the mind,
//! reminders and reports keep working while the person is away. After a restart nothing of an
//! account runs until its owner signs in once. The routes are in `hub_http.rs`.

use crate::{App, config::Config, model, util, vault};
use axum::Router;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

pub type Models = dyn Fn(&Config) -> Result<Arc<dyn model::Adapter>, String> + Send + Sync;

pub struct Hub {
    pub cfg: Config,
    pub(crate) sdb: Mutex<Connection>,
    pub(crate) apps: RwLock<HashMap<String, Arc<App>>>,
    pub(crate) routers: RwLock<HashMap<String, Router>>,
    /// username -> (failures in a row, locked until)
    fails: Mutex<HashMap<String, (u32, i64)>>,
    models: Box<Models>,
    /// Wrong tokens per caller address: (misses in a row, last miss, locked until, how many times locked).
    guesses: Mutex<HashMap<String, (u32, i64, i64, u32)>>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
  id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE, status TEXT NOT NULL, created TEXT NOT NULL, last_login TEXT,
  salt TEXT, wrapped_pass TEXT, wrapped_recovery TEXT, tail_hash TEXT
);
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS devices (
  id TEXT PRIMARY KEY, account_id TEXT NOT NULL, label TEXT, wrapped TEXT NOT NULL, created TEXT NOT NULL, last_seen TEXT, revoked INTEGER NOT NULL DEFAULT 0
);
"#;

type Row = (String, String, Option<String>, Option<String>, Option<String>, Option<String>);

fn valid_username(u: &str) -> bool {
    (3..=32).contains(&u.len()) && u.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

impl Hub {
    pub fn open(cfg: Config, models: Box<Models>) -> Result<Arc<Hub>, String> {
        std::fs::create_dir_all(cfg.data_dir.join("accounts")).map_err(|e| format!("data dir: {e}"))?;
        let path = cfg.data_dir.join("server.db");
        let c = Connection::open(&path).map_err(|e| format!("server database: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        c.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        let mut cfg = cfg;
        cfg.server_dir = cfg.data_dir.clone();
        Ok(Arc::new(Hub { cfg, sdb: Mutex::new(c), apps: Default::default(), routers: Default::default(), fails: Default::default(), models, guesses: Default::default() }))
    }

    /// The server token: made once. It keeps the old owner token's text when there was one (so a phone that
    /// knew it can still connect); otherwise a new one is written to `server.token` (mode 0600). Returns where, when new.
    pub fn ensure_server_token(&self) -> Result<Option<PathBuf>, String> {
        let have: Option<String> = self.sdb.lock().unwrap().query_row("SELECT value FROM meta WHERE key = 'token_hash'", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if have.is_some() {
            return Ok(None);
        }
        let (token, path) = match std::fs::read_to_string(self.cfg.data_dir.join("owner.token")) {
            Ok(t) if t.trim().len() >= 32 => (t.trim().to_string(), None),
            _ => {
                let t = format!("srv_{}", util::random_hex(24));
                let p = self.cfg.data_dir.join("server.token");
                std::fs::write(&p, &t).map_err(|e| e.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
                }
                (t, Some(p))
            }
        };
        self.sdb.lock().unwrap().execute("INSERT OR REPLACE INTO meta (key, value) VALUES ('token_hash', ?1)", [util::sha256_hex(token.as_bytes())]).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Change the server password (the server token) in `<data dir>/server.db`. The old one stops working at once;
    /// the plain `server.token` file, if any, is removed. Used by `agent-core server-password`, which reads it from stdin.
    pub fn set_server_password(data_dir: &FsPath, new: &str) -> Result<(), String> {
        let new = new.trim();
        if new.chars().count() < 12 {
            return Err("use at least 12 characters".into());
        }
        let c = Connection::open(data_dir.join("server.db")).map_err(|e| format!("server database: {e}"))?;
        c.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        c.execute("INSERT OR REPLACE INTO meta (key, value) VALUES ('token_hash', ?1)", [util::sha256_hex(new.as_bytes())]).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(data_dir.join("server.token"));
        Ok(())
    }

    /// Seconds a caller must wait, if too many wrong tokens came from it. Five misses in a row lock that address for
    /// 30 seconds, then 1, 2, 4 minutes, up to 15. A right token clears it; a quiet quarter hour clears it too.
    pub fn blocked_for(&self, who: &str) -> Option<i64> {
        let now = util::unix_now();
        self.guesses.lock().unwrap().get(who).filter(|g| g.2 > now).map(|g| g.2 - now)
    }

    pub(crate) fn token_missed(&self, who: &str) {
        let now = util::unix_now();
        let mut g = self.guesses.lock().unwrap();
        if g.len() > 5000 {
            g.retain(|_, v| v.2 > now || now - v.1 < 900); // forget old callers
        }
        let e = g.entry(who.to_string()).or_insert((0, now, 0, 0));
        if now - e.1 > 900 && e.2 <= now {
            *e = (0, now, 0, 0);
        }
        e.0 += 1;
        e.1 = now;
        if e.0 >= 5 {
            e.2 = now + (30i64 << e.3.min(5)).min(900);
            e.3 += 1;
            e.0 = 0;
        }
    }

    pub(crate) fn token_ok_from(&self, who: &str) {
        self.guesses.lock().unwrap().remove(who);
    }

    pub(crate) fn server_token_ok(&self, token: &str) -> bool {
        let h: Option<String> = self.sdb.lock().unwrap().query_row("SELECT value FROM meta WHERE key = 'token_hash'", [], |r| r.get(0)).ok();
        h.is_some_and(|h| h == util::sha256_hex(token.as_bytes()))
    }

    fn row(&self, username: &str) -> Option<Row> {
        self.sdb.lock().unwrap().query_row("SELECT id, status, salt, wrapped_pass, wrapped_recovery, tail_hash FROM accounts WHERE username = ?1", [username],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).ok()
    }

    pub(crate) fn is_active(&self, id: &str) -> bool {
        self.sdb.lock().unwrap().query_row("SELECT 1 FROM accounts WHERE id = ?1 AND status = 'active'", [id], |_| Ok(())).is_ok()
    }

    /// The admin makes a place for a person. The person then sets their own passphrase; the admin never learns it.
    pub fn invite(&self, username: &str) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        if !valid_username(&username) {
            return Err("a username is 3 to 32 letters, digits, dash or underscore".into());
        }
        if self.row(&username).is_some() {
            return Err("that username is taken".into());
        }
        let id = util::random_hex(5);
        self.sdb.lock().unwrap().execute("INSERT INTO accounts (id, username, status, created) VALUES (?1, ?2, 'invited', ?3)", params![id, username, util::now_iso()]).map_err(|e| e.to_string())?;
        Ok(json!({ "id": id, "username": username, "status": "invited" }))
    }

    pub fn list(&self) -> Vec<Value> {
        let unlocked: Vec<String> = self.apps.read().unwrap().keys().cloned().collect();
        let c = self.sdb.lock().unwrap();
        let mut st = c.prepare("SELECT id, username, status, created, last_login FROM accounts ORDER BY created").expect("prepare");
        st.query_map([], |r| {
            let id: String = r.get(0)?;
            Ok(json!({ "id": id, "username": r.get::<_, String>(1)?, "status": r.get::<_, String>(2)?, "created": r.get::<_, String>(3)?,
                "last_login": r.get::<_, Option<String>>(4)?, "unlocked": unlocked.contains(&id),
                "phones": c.query_row("SELECT COUNT(*) FROM devices WHERE account_id = ?1 AND revoked = 0", [&id], |x| x.get::<_, i64>(0)).unwrap_or(0) }))
        }).map(|x| x.flatten().collect()).unwrap_or_default()
    }

    fn throttle(&self, username: &str) -> Result<(), String> {
        let now = util::unix_now();
        if let Some((n, until)) = self.fails.lock().unwrap().get(username)
            && *n >= 5
            && *until > now
        {
            return Err(format!("too many wrong tries; wait {} seconds", until - now));
        }
        Ok(())
    }

    fn failed(&self, username: &str) {
        let now = util::unix_now();
        let mut f = self.fails.lock().unwrap();
        let e = f.entry(username.to_string()).or_insert((0, 0));
        e.0 += 1;
        e.1 = now + 60 * (e.0 as i64 - 4).clamp(1, 30); // the 5th miss: a minute, then longer
    }

    /// Step 1 of sign-up. Makes the keys and returns the recovery key, once. Nothing is active until `confirm`.
    pub fn signup(&self, username: &str, passphrase: &str) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        let (id, status, ..) = self.row(&username).ok_or("no account is waiting for that username; ask the server's admin")?;
        if status == "active" || status == "disabled" {
            return Err("that account is already set up".into());
        }
        if passphrase.chars().count() < 10 {
            return Err("use a passphrase of at least 10 characters; a few random words is best".into());
        }
        let (data_key, recovery) = (vault::random_key(), vault::random_key());
        let salt = util::random_hex(16);
        let pass_key = vault::kdf(passphrase, salt.as_bytes())?;
        let text = vault::recovery_text(&recovery);
        self.sdb.lock().unwrap().execute(
            "UPDATE accounts SET status = 'pending', salt = ?1, wrapped_pass = ?2, wrapped_recovery = ?3, tail_hash = ?4 WHERE id = ?5",
            params![salt, vault::wrap(&pass_key, &data_key, &id)?, vault::wrap(&recovery, &data_key, &id)?, util::sha256_hex(vault::recovery_tail(&text).as_bytes()), id],
        ).map_err(|e| e.to_string())?;
        Ok(json!({ "recovery_key": text, "warning": "Copy this key and keep it safe. If you forget your passphrase and lose this key, your data is gone for good: nobody, not even the server's admin, can recover it." }))
    }

    /// Step 2: the person types the last 8 characters of the recovery key back. Then the account is real.
    pub fn confirm(&self, username: &str, passphrase: &str, tail: &str, label: &str, enroll: bool) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        self.throttle(&username)?;
        let (id, status, salt, wp, _, th) = self.row(&username).ok_or("no such account")?;
        if status != "pending" {
            return Err("this account is not waiting for confirmation".into());
        }
        let key = self.open_with_passphrase(&id, passphrase, salt.as_deref(), wp.as_deref()).inspect_err(|_| self.failed(&username))?;
        if th.as_deref() != Some(util::sha256_hex(vault::recovery_tail(tail).as_bytes()).as_str()) {
            self.failed(&username);
            return Err("those are not the last 8 characters of the recovery key; check you copied it".into());
        }
        self.sdb.lock().unwrap().execute("UPDATE accounts SET status = 'active' WHERE id = ?1", [&id]).map_err(|e| e.to_string())?;
        self.finish_login(&id, &username, &key, label, enroll)
    }

    fn open_with_passphrase(&self, id: &str, passphrase: &str, salt: Option<&str>, wrapped: Option<&str>) -> Result<[u8; 32], String> {
        const WRONG: &str = "wrong username or passphrase";
        let (salt, wrapped) = (salt.ok_or(WRONG)?, wrapped.ok_or(WRONG)?);
        let pk = vault::kdf(passphrase, salt.as_bytes())?;
        vault::unwrap(&pk, wrapped, id).map_err(|_| WRONG.to_string())?.try_into().map_err(|_| WRONG.to_string())
    }

    pub fn login(&self, username: &str, passphrase: &str, label: &str, enroll: bool) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        self.throttle(&username)?;
        let Some((id, status, salt, wp, ..)) = self.row(&username) else {
            self.failed(&username);
            let _ = vault::kdf(passphrase, b"0123456789abcdef"); // takes as long whether or not the name exists
            return Err("wrong username or passphrase".into());
        };
        if status != "active" {
            return Err(if status == "disabled" { "this account is switched off".into() } else { "this account is not set up yet; finish sign-up first".to_string() });
        }
        let key = self.open_with_passphrase(&id, passphrase, salt.as_deref(), wp.as_deref()).inspect_err(|_| self.failed(&username))?;
        self.fails.lock().unwrap().remove(&username);
        self.finish_login(&id, &username, &key, label, enroll)
    }

    fn finish_login(&self, id: &str, username: &str, key: &[u8; 32], label: &str, enroll: bool) -> Result<Value, String> {
        let app = self.unlock(id, key)?;
        let device = if enroll { Some(self.enroll_device(id, key, label)?) } else { None };
        let (tid, secret) = crate::auth::create_token(&app.db, crate::auth::Scope::Owner, if label.is_empty() { "app" } else { label });
        let _ = self.sdb.lock().unwrap().execute("UPDATE accounts SET last_login = ?1 WHERE id = ?2", params![util::now_iso(), id]);
        Ok(json!({ "account": id, "username": username, "token": format!("{id}.{secret}"), "token_id": tid, "device": device }))
    }

    // ---------------- remembered phones ----------------
    // A signed-in phone can be remembered: the data key is wrapped once more, by a random device key that only the
    // phone keeps (never stored here). After a server restart the phone sends that key and the account unlocks by
    // itself, so nobody types a passphrase again. A copy of the server's disk is still useless without the phone's key.

    fn enroll_device(&self, account: &str, key: &[u8; 32], label: &str) -> Result<Value, String> {
        let (did, dkey) = (util::random_hex(8), vault::random_key());
        let wrapped = vault::wrap(&dkey, key, &format!("{did}:{account}"))?;
        self.sdb.lock().unwrap().execute("INSERT INTO devices (id, account_id, label, wrapped, created) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![did, account, util::clean_line(label, 4), wrapped, util::now_iso()]).map_err(|e| e.to_string())?;
        Ok(json!({ "id": did, "key": vault::to_hex(&dkey) }))
    }

    fn device_key(&self, device_id: &str, key_hex: &str) -> Result<(String, [u8; 32]), String> {
        const NO: &str = "this phone is not remembered; sign in again";
        let (account, wrapped): (String, String) = self.sdb.lock().unwrap().query_row("SELECT account_id, wrapped FROM devices WHERE id = ?1 AND revoked = 0", [device_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|_| NO.to_string())?;
        let dkey: [u8; 32] = vault::from_hex(key_hex).and_then(|v| v.try_into().ok()).ok_or(NO)?;
        let raw = vault::unwrap(&dkey, &wrapped, &format!("{device_id}:{account}")).map_err(|_| NO.to_string())?;
        Ok((account, raw.try_into().map_err(|_| NO.to_string())?))
    }

    /// A remembered phone unlocks its account after a restart. No passphrase, no server password.
    pub fn unlock_device(&self, device_id: &str, key_hex: &str) -> Result<Value, String> {
        let (account, key) = self.device_key(device_id, key_hex)?;
        if !self.is_active(&account) {
            return Err("this account is switched off".into());
        }
        self.unlock(&account, &key)?;
        let _ = self.sdb.lock().unwrap().execute("UPDATE devices SET last_seen = ?1 WHERE id = ?2", params![util::now_iso(), device_id]);
        Ok(json!({ "ok": true, "account": account }))
    }

    /// Sign out for good on this phone: the remembered key stops working.
    pub fn forget_device(&self, device_id: &str, key_hex: &str) -> Result<Value, String> {
        self.device_key(device_id, key_hex)?;
        self.sdb.lock().unwrap().execute("UPDATE devices SET revoked = 1 WHERE id = ?1", [device_id]).map_err(|e| e.to_string())?;
        Ok(json!({ "ok": true }))
    }

    /// Admin or recovery: no remembered phone of this account can unlock it any more.
    pub fn revoke_devices(&self, account: &str) -> usize {
        self.sdb.lock().unwrap().execute("UPDATE devices SET revoked = 1 WHERE account_id = ?1 AND revoked = 0", [account]).unwrap_or(0)
    }

    /// Lost passphrase: the recovery key opens the data key, and a new passphrase wraps it again.
    pub fn recover(&self, username: &str, recovery_key: &str, new_passphrase: &str) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        self.throttle(&username)?;
        let (id, status, _, _, wr, _) = self.row(&username).ok_or("wrong username or recovery key")?;
        if status != "active" {
            return Err("this account is not set up".into());
        }
        if new_passphrase.chars().count() < 10 {
            return Err("use a passphrase of at least 10 characters".into());
        }
        let rk = vault::recovery_parse(recovery_key).ok_or("that is not a recovery key")?;
        let data = vault::unwrap(&rk, wr.as_deref().unwrap_or(""), &id).map_err(|_| {
            self.failed(&username);
            "wrong username or recovery key".to_string()
        })?;
        let key: [u8; 32] = data.try_into().map_err(|_| "damaged key".to_string())?;
        self.rewrap(&id, &key, new_passphrase)?;
        self.revoke_devices(&id); // a lost passphrase may mean a lost phone: remembered phones must sign in again
        self.fails.lock().unwrap().remove(&username);
        Ok(json!({ "ok": true }))
    }

    pub fn change_passphrase(&self, username: &str, old: &str, new: &str) -> Result<Value, String> {
        let username = username.trim().to_lowercase();
        self.throttle(&username)?;
        let (id, status, salt, wp, ..) = self.row(&username).ok_or("wrong username or passphrase")?;
        if status != "active" || new.chars().count() < 10 {
            return Err("use a new passphrase of at least 10 characters".into());
        }
        let key = self.open_with_passphrase(&id, old, salt.as_deref(), wp.as_deref()).inspect_err(|_| self.failed(&username))?;
        self.rewrap(&id, &key, new)?;
        Ok(json!({ "ok": true }))
    }

    fn rewrap(&self, id: &str, key: &[u8; 32], passphrase: &str) -> Result<(), String> {
        let salt = util::random_hex(16);
        let pk = vault::kdf(passphrase, salt.as_bytes())?;
        self.sdb.lock().unwrap().execute("UPDATE accounts SET salt = ?1, wrapped_pass = ?2 WHERE id = ?3", params![salt, vault::wrap(&pk, key, id)?, id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn unlock(&self, id: &str, key: &[u8; 32]) -> Result<Arc<App>, String> {
        if let Some(a) = self.apps.read().unwrap().get(id) {
            return Ok(a.clone());
        }
        let model = (self.models)(&self.cfg)?;
        let app = build_account(&self.cfg, id, key, model)?;
        app.spawn_housekeeping();
        self.routers.write().unwrap().insert(id.to_string(), crate::api::router(app.clone()));
        self.apps.write().unwrap().insert(id.to_string(), app.clone());
        Ok(app)
    }

    /// Forget the key and stop the account's background work. Its data stays encrypted on disk.
    pub fn lock(&self, id: &str) -> bool {
        self.routers.write().unwrap().remove(id);
        match self.apps.write().unwrap().remove(id) {
            Some(a) => {
                a.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.lock(id);
        let _ = self.sdb.lock().unwrap().execute("DELETE FROM devices WHERE account_id = ?1", [id]);
        let n = self.sdb.lock().unwrap().execute("DELETE FROM accounts WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("no such account".into());
        }
        if id.chars().all(|c| c.is_ascii_hexdigit()) {
            let _ = std::fs::remove_dir_all(self.cfg.data_dir.join("accounts").join(id));
        }
        Ok(())
    }

    pub fn app_of(&self, id: &str) -> Option<Arc<App>> {
        self.apps.read().unwrap().get(id).cloned()
    }
}

/// Make one account's assistant: its own folder, plugin copies, encrypted database and settings.
pub fn build_account(base: &Config, id: &str, key: &[u8; 32], model: Arc<dyn model::Adapter>) -> Result<Arc<App>, String> {
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("bad account id".into());
    }
    let dir = base.data_dir.join("accounts").join(id);
    for sub in ["files", "work", "plugins"] {
        std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("account folder: {e}"))?;
    }
    sync_plugins(&base.plugins_dir, &dir.join("plugins")).map_err(|e| format!("plugins: {e}"))?;
    let mut cfg = base.clone();
    cfg.server_dir = base.data_dir.clone();
    cfg.data_dir = dir.clone();
    cfg.files_root = dir.join("files");
    cfg.term_cwd = dir.join("work");
    cfg.plugins_dir = dir.join("plugins");
    let db = crate::db::Db::open_keyed(&dir.join("core.db"), &vault::to_hex(key)).map_err(|e| format!("account database: {e}"))?;
    let routed = cfg.model.kind == "openai";
    let app = App::new(cfg, db, model);
    app.routed.store(routed, std::sync::atomic::Ordering::Relaxed);
    crate::scheduler::ensure_system_jobs(&app);
    crate::scheduler::ensure_reports(&app);
    crate::mind::ensure(&app);
    crate::scheduler::follow_zone(&app);
    Ok(app)
}

fn copy_dir(from: &FsPath, to: &FsPath) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, name) = (e.path(), e.file_name());
        if name == "state" {
            continue; // a plugin's state is never copied between accounts
        }
        if p.is_dir() { copy_dir(&p, &to.join(&name))? } else { std::fs::copy(&p, to.join(&name)).map(|_| ())? }
    }
    Ok(())
}

/// Give each account its own copy of the shipped plugins (a plugin keeps its `state/` beside its code, which
/// must never be shared). The shipped ones are copied again when the program is updated; plugins the account
/// built itself are left alone. `.shipped.json` lists which are shipped.
pub fn sync_plugins(src: &FsPath, dst: &FsPath) -> std::io::Result<()> {
    let marker = dst.join(".shipped.json");
    let mut shipped: Vec<String> = std::fs::read_to_string(&marker).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !e.path().is_dir() {
            continue;
        }
        let target = dst.join(&name);
        if !target.exists() || shipped.contains(&name) {
            // Keep the account's own state folder through an update.
            let keep = target.join("state");
            let saved = keep.exists().then(|| dst.join(format!(".state-keep-{name}")));
            if let Some(s) = &saved {
                let _ = std::fs::rename(&keep, s);
            }
            let _ = std::fs::remove_dir_all(&target);
            copy_dir(&e.path(), &target)?;
            if let Some(s) = &saved {
                let _ = std::fs::rename(s, target.join("state"));
            }
            if !shipped.contains(&name) {
                shipped.push(name);
            }
        }
    }
    std::fs::write(marker, serde_json::to_string(&shipped).unwrap_or_default())
}
