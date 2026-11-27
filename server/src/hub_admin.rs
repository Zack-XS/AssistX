//! The admin login for the server's dashboard (`/admin`, docs/architecture/accounts.md, Admin login).
//!
//! Managing accounts needs two things: the Server Access Code (which every person who signs in also has) and the
//! admin login, one username and password kept only as an Argon2id hash in `server.db`. Signing in gives a session
//! token held only in memory: it ends after 30 idle minutes, 12 hours at most, on sign-out, or on a restart.
//!
//! The first admin login is made on `/admin` with a one-time **setup code**. The server makes one at start while no
//! admin exists, prints it to its log and writes it to `<data dir>/admin-setup.code` (mode 0600); it is removed once
//! used. `agent-core admin-password` (username and password on stdin) sets or resets the login from the server itself.

use crate::hub::Hub;
use crate::{util, vault};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::{Path as FsPath, PathBuf};

/// An admin password needs at least this many characters.
pub const ADMIN_PASS_MIN: usize = 12;
const IDLE_SECS: i64 = 30 * 60;
const MAX_SECS: i64 = 12 * 3600;
const SETUP_FILE: &str = "admin-setup.code";

fn meta(c: &Connection, k: &str) -> Option<String> {
    c.query_row("SELECT value FROM meta WHERE key = ?1", [k], |r| r.get(0)).optional().ok().flatten()
}

/// Same length, every byte looked at: no early exit that tells a guesser how close they were.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn check_new(username: &str, password: &str) -> Result<(), String> {
    let ok_name = (3..=32).contains(&username.len()) && username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !ok_name {
        return Err("the admin username needs 3 to 32 small letters, digits, - or _".into());
    }
    if password.chars().count() < ADMIN_PASS_MIN {
        return Err(format!("the admin password needs at least {ADMIN_PASS_MIN} characters"));
    }
    Ok(())
}

fn store(c: &Connection, username: &str, password: &str) -> Result<(), String> {
    check_new(username, password)?;
    let salt = vault::random_key();
    let hash = vault::to_hex(&vault::kdf(password, &salt)?);
    for (k, v) in [("admin_user", username.to_string()), ("admin_salt", vault::to_hex(&salt)), ("admin_hash", hash)] {
        c.execute("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)", params![k, v]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Hub {
    pub fn admin_exists(&self) -> bool {
        meta(&self.sdb.lock().unwrap(), "admin_hash").is_some()
    }

    /// While no admin login exists: make a fresh setup code, keep its hash, write it to a 0600 file. Returns the code.
    pub fn ensure_admin_setup(&self) -> Option<(String, PathBuf)> {
        if self.admin_exists() {
            let _ = std::fs::remove_file(self.cfg.data_dir.join(SETUP_FILE));
            return None;
        }
        let code = crate::hub::invite_code();
        let path = self.cfg.data_dir.join(SETUP_FILE);
        let _ = std::fs::write(&path, format!("{code}\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        *self.admin_setup.lock().unwrap() = Some(crate::hub::code_hash(&code));
        Some((code, path))
    }

    /// First run: the setup code, then the admin's own username and password. Signs the admin in.
    pub fn admin_setup(&self, code: &str, username: &str, password: &str) -> Result<Value, String> {
        let key = "admin:setup".to_string();
        self.throttle(&key)?;
        if self.admin_exists() {
            return Err("an admin login exists already; sign in instead".into());
        }
        let want = self.admin_setup.lock().unwrap().clone().ok_or("no setup code is active; restart the server to get one")?;
        if !same(&crate::hub::code_hash(code), &want) {
            self.failed(&key);
            return Err("wrong setup code".into());
        }
        store(&self.sdb.lock().unwrap(), username, password)?;
        *self.admin_setup.lock().unwrap() = None;
        let _ = std::fs::remove_file(self.cfg.data_dir.join(SETUP_FILE));
        self.log_admin("admin_setup", "", username);
        Ok(self.new_session(username))
    }

    pub fn admin_login(&self, username: &str, password: &str) -> Result<Value, String> {
        let key = format!("admin:{username}");
        self.throttle(&key)?;
        let (user, salt, hash) = {
            let c = self.sdb.lock().unwrap();
            (meta(&c, "admin_user"), meta(&c, "admin_salt"), meta(&c, "admin_hash"))
        };
        let (Some(user), Some(salt), Some(hash)) = (user, salt, hash) else { return Err("no admin login yet; use the setup code".into()) };
        let salt = vault::from_hex(&salt).ok_or("admin login is damaged")?;
        // Always run the slow hash, so a wrong name takes as long as a wrong password.
        let got = vault::to_hex(&vault::kdf(password, &salt)?);
        if !(same(username, &user) & same(&got, &hash)) {
            self.failed(&key);
            return Err("wrong admin username or password".into());
        }
        self.log_admin("admin_login", "", username);
        Ok(self.new_session(username))
    }

    pub fn admin_change_password(&self, old: &str, new: &str) -> Result<Value, String> {
        let user = meta(&self.sdb.lock().unwrap(), "admin_user").unwrap_or_default();
        self.admin_login(&user, old).map_err(|_| "the current admin password is wrong".to_string())?;
        store(&self.sdb.lock().unwrap(), &user, new)?;
        self.admin_sessions.lock().unwrap().clear(); // every other admin session ends
        self.log_admin("admin_password", "", &user);
        Ok(self.new_session(&user))
    }

    fn new_session(&self, username: &str) -> Value {
        let token = format!("adm_{}", util::random_hex(32));
        let now = util::unix_now();
        let mut s = self.admin_sessions.lock().unwrap();
        s.retain(|_, v| now - v.1 < IDLE_SECS && now - v.0 < MAX_SECS);
        s.insert(util::sha256_hex(token.as_bytes()), (now, now));
        json!({ "session": token, "username": username, "idle_minutes": IDLE_SECS / 60 })
    }

    /// A live session: refreshes its idle timer.
    pub fn admin_session_ok(&self, token: &str) -> bool {
        let now = util::unix_now();
        let mut s = self.admin_sessions.lock().unwrap();
        match s.get_mut(&util::sha256_hex(token.as_bytes())) {
            Some(v) if now - v.1 < IDLE_SECS && now - v.0 < MAX_SECS => {
                v.1 = now;
                true
            }
            _ => false,
        }
    }

    pub fn admin_logout(&self, token: &str) {
        self.admin_sessions.lock().unwrap().remove(&util::sha256_hex(token.as_bytes()));
    }

    pub fn admin_name(&self) -> String {
        meta(&self.sdb.lock().unwrap(), "admin_user").unwrap_or_default()
    }

    /// `agent-core admin-password`: set or reset the admin login from the server itself.
    pub fn set_admin_password(data_dir: &FsPath, username: &str, password: &str) -> Result<(), String> {
        let c = Connection::open(data_dir.join("server.db")).map_err(|e| format!("server database: {e}"))?;
        crate::hub::open_schema(&c)?;
        store(&c, username.trim(), password)?;
        let _ = std::fs::remove_file(data_dir.join(SETUP_FILE));
        Ok(())
    }
}
