//! Export everything as a zip of markdown + JSON (docs/04 User control), and undo of logged
//! actions through each tool's declared `undo` tool (docs/03: the core never guesses).

use crate::{App, engine, memory, runner, util};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// ---------------- zip (stored, no compression) ----------------

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

pub struct Zip {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Default for Zip {
    fn default() -> Self {
        Self::new()
    }
}

impl Zip {
    pub fn new() -> Zip {
        Zip { out: vec![], central: vec![], count: 0 }
    }

    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let off = self.out.len() as u32;
        let n = name.as_bytes();
        let le16 = |v: u16| v.to_le_bytes();
        let le32 = |v: u32| v.to_le_bytes();
        // Local file header.
        self.out.extend_from_slice(&le32(0x0403_4b50));
        for v in [20u16, 0x0800, 0, 0, 0x21] { self.out.extend_from_slice(&le16(v)); } // version, utf-8 flag, stored, time, date
        for v in [crc, data.len() as u32, data.len() as u32] { self.out.extend_from_slice(&le32(v)); }
        self.out.extend_from_slice(&le16(n.len() as u16));
        self.out.extend_from_slice(&le16(0));
        self.out.extend_from_slice(n);
        self.out.extend_from_slice(data);
        // Central directory entry.
        self.central.extend_from_slice(&le32(0x0201_4b50));
        for v in [20u16, 20, 0x0800, 0, 0, 0x21] { self.central.extend_from_slice(&le16(v)); }
        for v in [crc, data.len() as u32, data.len() as u32] { self.central.extend_from_slice(&le32(v)); }
        for v in [n.len() as u16, 0, 0, 0, 0] { self.central.extend_from_slice(&le16(v)); }
        self.central.extend_from_slice(&le32(0));
        self.central.extend_from_slice(&le32(off));
        self.central.extend_from_slice(n);
        self.count += 1;
    }

    pub fn finish(mut self) -> Vec<u8> {
        let cd_off = self.out.len() as u32;
        let cd_len = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        for v in [0u16, 0, self.count, self.count] { self.out.extend_from_slice(&v.to_le_bytes()); }
        self.out.extend_from_slice(&cd_len.to_le_bytes());
        self.out.extend_from_slice(&cd_off.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out
    }
}

fn table(app: &App, sql: &str) -> Value {
    let c = app.db.lock();
    let Ok(mut st) = c.prepare(sql) else { return json!([]) };
    let cols: Vec<String> = st.column_names().map(|s| s.to_string()).collect();
    let rows = st.query_map([], |r| {
        let mut o = serde_json::Map::new();
        for (i, name) in cols.iter().enumerate() {
            let v: rusqlite::types::Value = r.get(i)?;
            let j = match v {
                rusqlite::types::Value::Null => Value::Null,
                rusqlite::types::Value::Integer(n) => json!(n),
                rusqlite::types::Value::Real(f) => json!(f),
                rusqlite::types::Value::Text(t) => serde_json::from_str::<Value>(&t).ok().filter(|x| x.is_object() || x.is_array()).unwrap_or(Value::String(t)),
                rusqlite::types::Value::Blob(_) => Value::Null,
            };
            o.insert(name.clone(), j);
        }
        Ok(Value::Object(o))
    }).map(|x| x.flatten().collect::<Vec<_>>()).unwrap_or_default();
    Value::Array(rows)
}

/// Everything the user owns. Secret values and token hashes are not included.
pub fn export_zip(app: &App) -> Vec<u8> {
    let mut z = Zip::new();
    let entries = memory::all(&app.db);
    for e in &entries {
        let md = format!("# {}\n\n{}\n\n{}\n\n---\n```json\n{}\n```\n", e.title, e.summary, e.detail, serde_json::to_string_pretty(e).unwrap_or_default());
        z.add(&format!("memory/{}/{}.md", e.topic_path, e.id), md.as_bytes());
    }
    z.add("memory/core.md", memory::read_core(&app.cfg.data_dir).as_bytes());
    z.add("memory/index.md", memory::index_for(&app.db, &memory::all_current(&app.db), None).join("\n").as_bytes());
    for (name, sql) in [
        ("threads", "SELECT id, title, kind, created, archived FROM threads"),
        ("messages", "SELECT id, thread_id, turn_id, role, trust, body, created FROM messages ORDER BY n"),
        ("turns", "SELECT id, thread_id, source, status, created, finished, error_code, tokens_used FROM turns"),
        ("actions", "SELECT id, thread_id, turn_id, plugin, tool, args, result, ok, outcome, decision, approval_id, undoable, created FROM actions ORDER BY n"),
        ("approvals", "SELECT id, thread_id, kind, risk, question, details, status, answer, created, expires_at FROM approvals"),
        ("inbox", "SELECT id, text, topic, trust, sources, created, status FROM inbox"),
        ("memory_history", "SELECT entry_id, version, snapshot, changed, by FROM memory_history"),
        ("jobs", "SELECT id, kind, text, schedule, timezone, next_run, last_run, outside, status, created FROM jobs"),
        ("lists", "SELECT id, name, status, created, updated FROM lists"),
        ("list_items", "SELECT id, list_id, text, quantity, note, due, done, outside, position, created, updated FROM list_items ORDER BY list_id, position"),
        ("plugins", "SELECT id, status, version, hash, source, created_by FROM plugin_state"),
        ("dreaming_runs", "SELECT id, dry_run, status, started, finished, changes, questions FROM dreaming_runs"),
        ("secret_names", "SELECT name, updated FROM secrets"),
    ] {
        z.add(&format!("json/{name}.json"), serde_json::to_string_pretty(&table(app, sql)).unwrap_or_default().as_bytes());
    }
    z.add("README.md", format!("Export made {}.\nMemory is in memory/ as markdown. Everything else is JSON in json/.\n", util::now_iso()).as_bytes());
    z.finish()
}

// ---------------- undo ----------------

/// Undo an action by calling the tool named in its `undo` entry with `{args, result}`.
#[allow(clippy::type_complexity)]
pub async fn undo(app: &Arc<App>, action_id: &str) -> Result<Value, String> {
    let row: Option<(String, String, String, String, Option<String>, i64, String)> = app.db.lock().query_row(
        "SELECT thread_id, plugin, tool, args, result, undoable, outcome FROM actions WHERE id = ?1", [action_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    ).optional().ok().flatten();
    let (thread, plugin, tool, args, result, undoable, outcome) = row.ok_or("no such action")?;
    if undoable == 0 || outcome != "ok" {
        return Err("this action cannot be undone".into());
    }
    let m = app.manifest(&plugin).ok_or("plugin is gone")?;
    let undo_tool = m.tool(&tool).and_then(|d| d.undo.clone()).ok_or("this tool has no undo")?;
    let undo_args = json!({ "args": serde_json::from_str::<Value>(&args).unwrap_or(Value::Null),
                            "result": result.and_then(|r| serde_json::from_str::<Value>(&r).ok()).unwrap_or(Value::Null) });
    let new_id = engine::log_action(app, &thread, None, &plugin, &undo_tool, &undo_args, None, None, "running", &format!("undo:{action_id}"), None, false);
    let out = match plugin.as_str() {
        "spotify" => crate::spotify::call(app, &undo_tool, &undo_args).await,
        "email" => crate::email::call(app, &undo_tool, &undo_args).await,
        "scheduler" => crate::scheduler::tool(app, &thread, false, &undo_tool, &undo_args),
        "calendar" => crate::calendar::tool(app, false, &undo_tool, &undo_args),
        "lists" => crate::lists::tool(app, false, &undo_tool, &undo_args),
        "location" => crate::location::tool(app, &undo_tool, &undo_args),
        _ => runner::run(app, &m, &undo_tool, undo_args, false, None, Arc::new(AtomicBool::new(false))).await,
    };
    let ok = out.is_ok();
    let body = match &out { Ok(v) => v.clone(), Err(e) => json!({ "error": e }) };
    engine::finish_action(app, &new_id, Some(&body), Some(ok), if ok { "ok" } else { "failed" });
    if ok {
        let _ = app.db.lock().execute("UPDATE actions SET undoable = 0, outcome = 'undone' WHERE id = ?1", params![action_id]);
    }
    app.feed.emit(Some(&thread), None, "action.logged", json!({ "id": new_id, "summary": format!("undo of {action_id} {}", if ok { "done" } else { "failed" }), "undoable": false }));
    out.map(|v| json!({ "undone": action_id, "undo_action": new_id, "result": v }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_known_value() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }
}
