//! Tokens, scopes and request checks (docs/01 Auth, docs/06 T6/T7).

use crate::db::Db;
use crate::util;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Client,
    Owner,
    Trigger,
    /// A phone or watch that may only upload captured text and read capture status.
    Device,
    /// A computer running the node program: it may only fetch work and send results.
    Node,
}

impl Scope {
    pub fn parse(s: &str) -> Option<Scope> {
        match s {
            "client" => Some(Scope::Client),
            "owner" => Some(Scope::Owner),
            "trigger" => Some(Scope::Trigger),
            "device" => Some(Scope::Device),
            "node" => Some(Scope::Node),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Client => "client",
            Scope::Owner => "owner",
            Scope::Trigger => "trigger",
            Scope::Device => "device",
            Scope::Node => "node",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Caller {
    pub token_id: String,
    pub scope: Scope,
    /// For trigger tokens: the one source this token may post to.
    pub label: String,
}

/// Older databases get the "last seen" columns (an existing column is not an error).
pub fn upgrade_tokens(c: &rusqlite::Connection) {
    for col in ["last_seen TEXT", "user_agent TEXT NOT NULL DEFAULT ''", "uses INTEGER NOT NULL DEFAULT 0"] {
        let _ = c.execute(&format!("ALTER TABLE tokens ADD COLUMN {col}"), []);
    }
}

/// Note that a token was just used: when, by what, how many requests. At most one write
/// every 20 seconds per token, so busy polling stays cheap.
pub fn touch_token(db: &Db, id: &str, user_agent: &str) {
    let now = util::now_iso();
    let cutoff = util::iso(util::unix_now() - 20);
    let ua: String = user_agent.chars().take(160).collect();
    let _ = db.lock().execute(
        "UPDATE tokens SET last_seen = ?1, user_agent = ?2, uses = uses + 1 WHERE id = ?3 AND (last_seen IS NULL OR last_seen < ?4)",
        params![now, ua, id, cutoff],
    );
}

/// Only the hash is stored. Plain text is shown once.
pub fn create_token(db: &Db, scope: Scope, label: &str) -> (String, String) {
    let plain = format!("tok_{}", util::random_hex(32));
    let id = util::new_id("tkn");
    db.lock()
        .execute(
            "INSERT INTO tokens (id, hash, scope, label, created) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, util::sha256_hex(plain.as_bytes()), scope.as_str(), label, util::now_iso()],
        )
        .expect("insert token");
    (id, plain)
}

pub fn check_token(db: &Db, plain: &str) -> Option<Caller> {
    // Lookup is by hash of the secret, so timing does not leak the stored value.
    let hash = util::sha256_hex(plain.as_bytes());
    db.lock()
        .query_row("SELECT id, scope, label FROM tokens WHERE hash = ?1 AND revoked = 0", [hash], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .optional()
        .ok()
        .flatten()
        .and_then(|(id, s, label)| Scope::parse(&s).map(|scope| Caller { token_id: id, scope, label }))
}

/// First run: make an owner token and write it to a 0600 file.
/// Later runs: the file is left alone; the DB holds only the hash.
pub fn ensure_owner_token(db: &Db, path: &Path) -> std::io::Result<Option<String>> {
    let has_owner: bool = db
        .lock()
        .query_row("SELECT EXISTS(SELECT 1 FROM tokens WHERE scope = 'owner' AND revoked = 0)", [], |r| r.get(0))
        .unwrap_or(false);
    if has_owner {
        return Ok(None);
    }
    let (_, plain) = create_token(db, Scope::Owner, "first-run owner");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    f.write_all(plain.as_bytes())?;
    Ok(Some(plain))
}

#[derive(Serialize)]
pub struct TokenInfo {
    pub id: String,
    pub scope: String,
    pub label: String,
    pub created: String,
    pub revoked: bool,
    pub last_seen: Option<String>,
    pub user_agent: String,
    pub uses: i64,
}

pub fn list_tokens(db: &Db) -> Vec<TokenInfo> {
    let c = db.lock();
    let mut st = c.prepare("SELECT id, scope, label, created, revoked, last_seen, user_agent, uses FROM tokens ORDER BY created").expect("prepare");
    st.query_map([], |r| {
        Ok(TokenInfo { id: r.get(0)?, scope: r.get(1)?, label: r.get(2)?, created: r.get(3)?, revoked: r.get::<_, i64>(4)? != 0, last_seen: r.get(5)?, user_agent: r.get(6)?, uses: r.get(7)? })
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

pub fn revoke_token(db: &Db, id: &str) -> bool {
    db.lock().execute("UPDATE tokens SET revoked = 1 WHERE id = ?1", [id]).map(|n| n > 0).unwrap_or(false)
}
