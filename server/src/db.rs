//! SQLite store. One file in the data dir.
//!
//! Gaps vs docs/04: no SQLCipher yet, and the raw log is the `messages` table.

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Db(Mutex<Connection>);

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tokens (
  id TEXT PRIMARY KEY, hash TEXT NOT NULL UNIQUE, scope TEXT NOT NULL,
  label TEXT NOT NULL DEFAULT '', created TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS threads (
  id TEXT PRIMARY KEY, owner_id TEXT NOT NULL DEFAULT 'owner', title TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'user', created TEXT NOT NULL, archived INTEGER NOT NULL DEFAULT 0,
  next_seq INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS turns (
  id TEXT PRIMARY KEY, thread_id TEXT NOT NULL REFERENCES threads(id), source TEXT NOT NULL,
  status TEXT NOT NULL, client_msg_id TEXT, input TEXT NOT NULL DEFAULT '{}',
  created TEXT NOT NULL, finished TEXT, error_code TEXT, error_message TEXT, tokens_used INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS turns_client_msg ON turns(thread_id, client_msg_id) WHERE client_msg_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS messages (
  n INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, owner_id TEXT NOT NULL DEFAULT 'owner',
  thread_id TEXT NOT NULL REFERENCES threads(id), turn_id TEXT,
  role TEXT NOT NULL, trust TEXT NOT NULL, body TEXT NOT NULL, created TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_thread ON messages(thread_id, n);
CREATE TABLE IF NOT EXISTS events (
  n INTEGER PRIMARY KEY AUTOINCREMENT, seq INTEGER NOT NULL, type TEXT NOT NULL, ts INTEGER NOT NULL,
  thread_id TEXT, turn_id TEXT, data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS events_thread ON events(thread_id, n);
CREATE TABLE IF NOT EXISTS approvals (
  id TEXT PRIMARY KEY, thread_id TEXT NOT NULL, turn_id TEXT NOT NULL, kind TEXT NOT NULL,
  risk TEXT NOT NULL, question TEXT NOT NULL, reason TEXT, details TEXT NOT NULL, call_hash TEXT NOT NULL,
  status TEXT NOT NULL, answer TEXT, answered_by TEXT, created INTEGER NOT NULL, expires_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS approvals_status ON approvals(status, created);
CREATE TABLE IF NOT EXISTS actions (
  n INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, thread_id TEXT NOT NULL, turn_id TEXT,
  plugin TEXT NOT NULL, tool TEXT NOT NULL, args TEXT NOT NULL, result TEXT, ok INTEGER,
  outcome TEXT NOT NULL, decision TEXT NOT NULL, approval_id TEXT, undoable INTEGER NOT NULL DEFAULT 0,
  created TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS actions_thread ON actions(thread_id, n);
"#;

impl Db {
    /// Open the database file, encrypted with SQLCipher (docs/06). The key comes from
    /// `AGENT_DB_KEY` (any passphrase) or else from `db.key` next to the file (32 random bytes,
    /// mode 0600, made on first run). A plain database from an older version is encrypted in
    /// place; the plain copy is kept as `core.db.plain-backup` for the owner to delete.
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        let key = db_key(path).map_err(rusqlite::Error::InvalidParameterName)?;
        // A plain SQLite file starts with this header; an encrypted one looks like noise.
        let plain = { use std::io::Read; let mut h = [0u8; 16]; std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut h)).is_ok() && &h == b"SQLite format 3\0" };
        if plain && opens_with(path, None) {
            encrypt_in_place(path, &key).map_err(|e| rusqlite::Error::InvalidParameterName(format!("encrypting the old database: {e}")))?;
        }
        let c = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        set_key(&c, &key)?;
        // A wrong key only shows on the first read.
        c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .map_err(|_| rusqlite::Error::InvalidParameterName("the database key is wrong (AGENT_DB_KEY or db.key)".into()))?;
        Self::setup(c)
    }

    /// An account's database: encrypted with its own 32 byte key (hex), never with the server's `db.key` (docs/15).
    pub fn open_keyed(path: &Path, key_hex: &str) -> rusqlite::Result<Db> {
        if key_hex.len() != 64 || !key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(rusqlite::Error::InvalidParameterName("bad account key".into()));
        }
        let c = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        set_key(&c, &format!("\"x'{key_hex}'\""))?;
        c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).map_err(|_| rusqlite::Error::InvalidParameterName("the account key is wrong".into()))?;
        Self::setup(c)
    }

    pub fn open_memory() -> rusqlite::Result<Db> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(c: Connection) -> rusqlite::Result<Db> {
        c.execute_batch(SCHEMA)?;
        c.execute_batch(crate::memory::SCHEMA)?;
        c.execute_batch(crate::extra_schema())?;
        c.execute_batch(crate::ambient::SCHEMA)?;
        c.execute_batch(crate::calendar::SCHEMA)?;
        c.execute_batch(crate::lists::SCHEMA)?;
        let _ = c.execute("ALTER TABLE calendar_events ADD COLUMN skip TEXT NOT NULL DEFAULT '[]'", []); // older databases
        c.execute_batch(crate::location::SCHEMA)?;
        c.execute_batch(crate::health::SCHEMA)?;
        c.execute_batch(crate::phone::SCHEMA)?;
        c.execute_batch(crate::audit::SCHEMA)?;
        c.execute_batch(crate::mind::SCHEMA)?;
        c.execute_batch(crate::signals::SCHEMA)?;
        c.execute_batch(crate::travel::SCHEMA)?;
        c.execute_batch(crate::detect::SCHEMA)?;
        c.execute_batch(crate::embed::SCHEMA)?;
        c.execute_batch(crate::recipes::SCHEMA)?;
        c.execute_batch(crate::summary::SCHEMA)?;
        crate::auth::upgrade_tokens(&c);
        Ok(Db(Mutex::new(c)))
    }
}

/// The key as SQLCipher wants it: a passphrase from the environment, or raw hex from `db.key`.
fn db_key(path: &Path) -> Result<String, String> {
    if let Ok(k) = std::env::var("AGENT_DB_KEY")
        && !k.trim().is_empty()
    {
        return Ok(format!("'{}'", k.trim().replace('\'', "''")));
    }
    let file = path.with_file_name("db.key");
    let hex = match std::fs::read_to_string(&file) {
        Ok(s) if s.trim().len() == 64 && s.trim().chars().all(|c| c.is_ascii_hexdigit()) => s.trim().to_string(),
        Ok(_) => return Err(format!("{} is not a 64 character hex key", file.display())),
        Err(_) => {
            let hex = crate::util::random_hex(32);
            write_private(&file, &hex).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            hex
        }
    };
    Ok(format!("\"x'{hex}'\""))
}

fn write_private(file: &Path, text: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(file)?;
        f.write_all(text.as_bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(file, text)
    }
}

fn set_key(c: &Connection, key: &str) -> rusqlite::Result<()> {
    c.execute_batch(&format!("PRAGMA key = {key};"))
}

/// Can this file be read with this key (`None` = as a plain database)?
fn opens_with(path: &Path, key: Option<&str>) -> bool {
    let Ok(c) = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else { return false };
    if let Some(k) = key
        && set_key(&c, k).is_err()
    {
        return false;
    }
    c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_ok()
}

/// Copy a plain database into an encrypted one, then swap them. The plain file is kept.
fn encrypt_in_place(path: &Path, key: &str) -> Result<(), String> {
    let tmp = path.with_extension("db.encrypting");
    let _ = std::fs::remove_file(&tmp);
    {
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        // Fold the write-ahead log into the main file first, so nothing is left behind in it.
        c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())).map_err(|e| e.to_string())?;
        let t = tmp.display().to_string().replace('\'', "''");
        c.execute_batch(&format!("ATTACH DATABASE '{t}' AS enc KEY {key}; SELECT sqlcipher_export('enc'); DETACH DATABASE enc;"))
            .map_err(|e| e.to_string())?;
    }
    if !opens_with(&tmp, Some(key)) {
        let _ = std::fs::remove_file(&tmp);
        return Err("the encrypted copy could not be read back".into());
    }
    let backup = path.with_extension("db.plain-backup");
    std::fs::rename(path, &backup).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(0o600));
    }
    for side in ["db-wal", "db-shm"] {
        let _ = std::fs::remove_file(path.with_extension(side));
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    eprintln!("note: the database is now encrypted. The old plain copy is {}; delete it once all is well.", backup.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_encrypted_and_an_old_plain_one_is_converted() {
        let dir = std::env::temp_dir().join(format!("dbenc{}", crate::util::random_hex(4)));
        std::fs::create_dir_all(&dir).unwrap();
        // An old plain database with a row in it.
        let p = dir.join("core.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch("CREATE TABLE old (x TEXT); INSERT INTO old VALUES ('kept');").unwrap();
        }
        let db = Db::open(&p).unwrap();
        let x: String = db.lock().query_row("SELECT x FROM old", [], |r| r.get(0)).unwrap();
        assert_eq!(x, "kept", "data survives");
        drop(db);
        assert!(!opens_with(&p, None), "the file can no longer be read without the key");
        assert!(std::fs::read(&p).unwrap().windows(4).all(|w| w != b"kept"), "no plain text in the file");
        assert!(p.with_extension("db.plain-backup").exists());
        let key = std::fs::read_to_string(dir.join("db.key")).unwrap();
        assert_eq!(key.len(), 64);
        // Opens again with the same key; a wrong key is refused, not silently replaced.
        assert!(Db::open(&p).is_ok());
        std::fs::write(dir.join("db.key"), "0".repeat(64)).unwrap();
        assert!(Db::open(&p).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

impl Db {
    /// Short critical sections only; never hold this across an `.await`.
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
