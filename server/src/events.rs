//! Event feed (docs/01, Event format). Stored for replay, pushed live over a broadcast channel.

use crate::db::Db;
use crate::util;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub id: String,
    pub seq: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub ts: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub data: Value,
    /// Global order; `id` is made from it.
    #[serde(skip)]
    pub n: i64,
}

pub fn event_id(n: i64) -> String {
    format!("evt_{n:016}")
}

/// Parse `evt_0000000000001042` back to 1042.
pub fn parse_event_id(id: &str) -> Option<i64> {
    id.strip_prefix("evt_")?.parse().ok()
}

pub struct Feed {
    db: Arc<Db>,
    tx: broadcast::Sender<Arc<Event>>,
}

impl Feed {
    pub fn new(db: Arc<Db>) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Feed { db, tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Event>> {
        self.tx.subscribe()
    }

    /// Store then broadcast. `seq` grows by one per thread.
    pub fn emit(&self, thread_id: Option<&str>, turn_id: Option<&str>, kind: &str, data: Value) -> Arc<Event> {
        let ts = util::unix_now();
        let ev = {
            let mut c = self.db.lock();
            let tx = c.transaction().expect("begin");
            let seq: i64 = match thread_id {
                Some(t) => {
                    let s: i64 = tx
                        .query_row("SELECT next_seq FROM threads WHERE id = ?1", [t], |r| r.get(0))
                        .unwrap_or(1);
                    let _ = tx.execute("UPDATE threads SET next_seq = next_seq + 1 WHERE id = ?1", [t]);
                    s
                }
                None => 0,
            };
            tx.execute(
                "INSERT INTO events (seq, type, ts, thread_id, turn_id, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![seq, kind, ts, thread_id, turn_id, data.to_string()],
            )
            .expect("insert event");
            let n = tx.last_insert_rowid();
            tx.commit().expect("commit");
            Event {
                id: event_id(n),
                seq,
                kind: kind.into(),
                ts: util::iso(ts),
                thread_id: thread_id.map(String::from),
                turn_id: turn_id.map(String::from),
                data,
                n,
            }
        };
        let ev = Arc::new(ev);
        let _ = self.tx.send(ev.clone());
        ev
    }

    /// Events after `after_n`, oldest first, optionally for one thread.
    pub fn replay(&self, after_n: i64, thread_id: Option<&str>, limit: i64) -> Vec<Event> {
        let c = self.db.lock();
        let mut st = c
            .prepare(
                "SELECT n, seq, type, ts, thread_id, turn_id, data FROM events
                 WHERE n > ?1 AND (?2 IS NULL OR thread_id = ?2) ORDER BY n LIMIT ?3",
            )
            .expect("prepare");
        st.query_map(params![after_n, thread_id, limit], |r| {
            let n: i64 = r.get(0)?;
            let data: String = r.get(6)?;
            Ok(Event {
                id: event_id(n),
                seq: r.get(1)?,
                kind: r.get(2)?,
                ts: util::iso(r.get(3)?),
                thread_id: r.get(4)?,
                turn_id: r.get(5)?,
                data: serde_json::from_str(&data).unwrap_or(Value::Null),
                n,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    /// Drop events older than the replay window.
    pub fn prune(&self, retention_secs: i64) {
        let cutoff = util::unix_now() - retention_secs;
        let _ = self.db.lock().execute("DELETE FROM events WHERE ts < ?1", [cutoff]);
    }
}
