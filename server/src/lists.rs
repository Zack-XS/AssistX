//! The user's lists (plugin `lists`, docs/03): shopping, to-do, or any name.
//!
//! Lists and their items live in the database. A list is found by its id or by its name
//! (`"shopping list"`, `"Shopping"` and `"groceries"` are the same list). Items may carry a
//! quantity, a note and a due date, and are ticked off rather than removed. Items added in a
//! turn that read outside content are marked `outside`. Open lists are shown in the index as
//! counts, wrapped as outside content (item words may have come from a mail or a page).

use crate::{App, scheduler, util};
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::sync::Arc;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS lists (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active',
  created TEXT NOT NULL, updated TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS list_items (
  id TEXT PRIMARY KEY, list_id TEXT NOT NULL, text TEXT NOT NULL, quantity TEXT, note TEXT, due TEXT,
  done INTEGER NOT NULL DEFAULT 0, outside INTEGER NOT NULL DEFAULT 0, position INTEGER NOT NULL DEFAULT 0,
  created TEXT NOT NULL, updated TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS list_items_list ON list_items(list_id, done, position);
"#;

const MAX_LISTS: i64 = 100;
const MAX_ITEMS: i64 = 500;
const MAX_ADD: usize = 100;

#[derive(Clone, Debug)]
pub struct List {
    pub id: String,
    pub name: String,
    pub status: String,
}

fn list_row(r: &rusqlite::Row) -> rusqlite::Result<List> {
    Ok(List { id: r.get(0)?, name: r.get(1)?, status: r.get(2)? })
}

const ITEM_COLS: &str = "id, list_id, text, quantity, note, due, done, outside, position";

fn item_row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({ "id": r.get::<_, String>(0)?, "list_id": r.get::<_, String>(1)?, "text": r.get::<_, String>(2)?,
        "quantity": r.get::<_, Option<String>>(3)?, "note": r.get::<_, Option<String>>(4)?, "due": r.get::<_, Option<String>>(5)?,
        "done": r.get::<_, i64>(6)? != 0, "outside": r.get::<_, i64>(7)? != 0, "position": r.get::<_, i64>(8)? }))
}

// ---------------- names ----------------

/// Compare names loosely: lower case, letters and digits only, no trailing "list".
fn norm(name: &str) -> String {
    let s: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    match s.strip_suffix("list") {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => s,
    }
}

/// Other words people use for the two common lists.
fn alias(n: &str) -> Option<&'static str> {
    match n {
        "grocery" | "groceries" | "shop" | "shoppinglist" => Some("shopping"),
        "todos" | "todolist" | "tasks" | "task" => Some("todo"),
        _ => None,
    }
}

/// The name a new list gets: `to-do` for the to-do list, else the user's words with a capital first letter and no "list".
fn display_name(name: &str) -> String {
    let n = norm(name);
    if n == "todo" || alias(&n) == Some("todo") {
        return "To-do".into();
    }
    if alias(&n) == Some("shopping") {
        return "Shopping".into();
    }
    let t = name.trim();
    let t = if t.to_lowercase().ends_with(" list") && t.len() > 5 { &t[..t.len() - 5] } else { t };
    let t: String = t.trim().chars().take(60).collect();
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => t,
    }
}

fn active_lists(app: &App) -> Vec<List> {
    let c = app.db.lock();
    let mut st = c.prepare("SELECT id, name, status FROM lists WHERE status = 'active' ORDER BY created").expect("prepare");
    st.query_map([], list_row).map(|x| x.flatten().collect()).unwrap_or_default()
}

fn get_list(app: &App, id: &str) -> Option<List> {
    app.db.lock().query_row("SELECT id, name, status FROM lists WHERE id = ?1", [id], list_row).optional().ok().flatten()
}

/// An active list by id or by name.
pub fn find(app: &App, key: &str) -> Option<List> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    if let Some(l) = get_list(app, key).filter(|l| l.status == "active") {
        return Some(l);
    }
    let all = active_lists(app);
    let n = norm(key);
    if let Some(l) = all.iter().find(|l| norm(&l.name) == n) {
        return Some(l.clone());
    }
    let a = alias(&n)?;
    all.into_iter().find(|l| norm(&l.name) == a || alias(&norm(&l.name)) == Some(a))
}

fn create(app: &App, name: &str) -> Result<List, String> {
    let name = display_name(name);
    if name.is_empty() {
        return Err("a list needs a name, e.g. Shopping".into());
    }
    if let Some(l) = find(app, &name) {
        return Err(format!("there is already a list called {}", l.name));
    }
    let n: i64 = app.db.lock().query_row("SELECT COUNT(*) FROM lists WHERE status = 'active'", [], |r| r.get(0)).unwrap_or(0);
    if n >= MAX_LISTS {
        return Err(format!("at most {MAX_LISTS} lists; delete one first"));
    }
    let l = List { id: util::new_id("lst"), name, status: "active".into() };
    let now = util::now_iso();
    app.db.lock().execute("INSERT INTO lists (id, name, status, created, updated) VALUES (?1, ?2, 'active', ?3, ?3)", params![l.id, l.name, now])
        .map_err(|e| e.to_string())?;
    Ok(l)
}

fn touch(app: &App, id: &str) {
    let _ = app.db.lock().execute("UPDATE lists SET updated = ?2 WHERE id = ?1", params![id, util::now_iso()]);
}

fn items(app: &App, list_id: &str) -> Vec<Value> {
    let c = app.db.lock();
    let mut st = c.prepare(&format!("SELECT {ITEM_COLS} FROM list_items WHERE list_id = ?1 ORDER BY done, position")).expect("prepare");
    st.query_map([list_id], item_row).map(|x| x.flatten().collect()).unwrap_or_default()
}

fn counts(app: &App, list_id: &str) -> (i64, i64) {
    app.db.lock().query_row("SELECT COALESCE(SUM(done = 0), 0), COALESCE(SUM(done = 1), 0) FROM list_items WHERE list_id = ?1", [list_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or((0, 0))
}

fn summary(app: &App, l: &List) -> Value {
    let (open, done) = counts(app, &l.id);
    json!({ "id": l.id, "name": l.name, "open": open, "done": done })
}

// ---------------- fields ----------------

fn text_field(v: &Value, key: &str, max: usize) -> Option<String> {
    v[key].as_str().map(|s| s.trim().chars().take(max).collect::<String>()).filter(|s| !s.is_empty())
}

/// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`, checked.
fn check_due(s: &str) -> Result<String, String> {
    let s = s.trim();
    let full = if s.len() == 10 { format!("{s} 00:00") } else { s.to_string() };
    if !(s.len() == 10 || s.len() == 16) || scheduler::parse_local(&full).is_none() {
        return Err(format!("due '{s}' is not a date; use YYYY-MM-DD or YYYY-MM-DD HH:MM"));
    }
    Ok(s.to_string())
}

/// One item from a tool or API call: a plain string, or `{text, quantity?, note?, due?}`.
struct NewItem {
    text: String,
    quantity: Option<String>,
    note: Option<String>,
    due: Option<String>,
}

fn new_item(v: &Value) -> Result<NewItem, String> {
    match v {
        Value::String(s) => {
            let text: String = s.trim().chars().take(200).collect();
            if text.is_empty() { Err("an item is empty".into()) } else { Ok(NewItem { text, quantity: None, note: None, due: None }) }
        }
        Value::Object(_) => {
            let text = text_field(v, "text", 200).or_else(|| text_field(v, "name", 200)).ok_or("each item needs text")?;
            let quantity = match &v["quantity"] {
                Value::Number(n) => Some(n.to_string()),
                _ => text_field(v, "quantity", 40),
            };
            let due = match text_field(v, "due", 20) { Some(d) => Some(check_due(&d)?), None => None };
            Ok(NewItem { text, quantity, note: text_field(v, "note", 500), due })
        }
        _ => Err("items are words, e.g. [\"milk\", {\"text\": \"eggs\", \"quantity\": \"12\"}]".into()),
    }
}

/// Item ids or words in a list: an exact id, the same words, or words that match only one item.
fn pick(all: &[Value], keys: &[String]) -> (Vec<Value>, Vec<String>) {
    let mut found: Vec<Value> = vec![];
    let mut missing = vec![];
    for k in keys {
        let kn = norm(k);
        let hit = all.iter().find(|i| i["id"] == k.as_str()).or_else(|| all.iter().find(|i| norm(i["text"].as_str().unwrap_or("")) == kn)).or_else(|| {
            let lk = k.to_lowercase();
            let some: Vec<&Value> = all.iter().filter(|i| !lk.is_empty() && i["text"].as_str().unwrap_or("").to_lowercase().contains(&lk)).collect();
            (some.len() == 1).then(|| some[0])
        });
        match hit {
            Some(i) if !found.iter().any(|f| f["id"] == i["id"]) => found.push(i.clone()),
            Some(_) => {}
            None => missing.push(k.clone()),
        }
    }
    (found, missing)
}

fn keys(a: &Value) -> Vec<String> {
    let v = if a.get("items").is_some() { &a["items"] } else if a.get("item").is_some() { &a["item"] } else { &a["ids"] };
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(x) => x.iter().filter_map(|i| i.as_str().map(String::from).or_else(|| i["id"].as_str().or(i["text"].as_str()).map(String::from))).collect(),
        _ => vec![],
    }
}

fn need_list(app: &App, a: &Value) -> Result<List, String> {
    let key = a["list"].as_str().or(a["id"].as_str()).or(a["list_id"].as_str()).or(a["name"].as_str()).unwrap_or_default();
    if key.trim().is_empty() {
        return Err("say which list, e.g. {\"list\": \"shopping\"}".into());
    }
    find(app, key).ok_or_else(|| {
        let names: Vec<String> = active_lists(app).into_iter().map(|l| l.name).collect();
        if names.is_empty() { format!("no list called '{key}'; there are no lists yet") } else { format!("no list called '{key}'; the lists are: {}", names.join(", ")) }
    })
}

// ---------------- actions ----------------

pub fn list_all(app: &App) -> Value {
    json!({ "lists": active_lists(app).iter().map(|l| summary(app, l)).collect::<Vec<_>>() })
}

pub fn show(app: &App, l: &List) -> Value {
    json!({ "list": summary(app, l), "items": items(app, &l.id) })
}

/// Add items, making the list when it is new. An open item with the same words is not added twice; a ticked one is
/// opened again.
pub fn add_items(app: &App, list_key: &str, raw: &Value, outside: bool) -> Result<Value, String> {
    let list = if raw.is_null() { vec![] } else if let Value::Array(a) = raw { a.clone() } else { vec![raw.clone()] };
    if list.is_empty() {
        return Err("items is required, e.g. [\"milk\", \"eggs\"]".into());
    }
    if list.len() > MAX_ADD {
        return Err(format!("at most {MAX_ADD} items at once"));
    }
    let new: Vec<NewItem> = list.iter().map(new_item).collect::<Result<_, _>>()?;
    let (l, made) = match find(app, list_key) {
        Some(l) => (l, false),
        None => (create(app, list_key)?, true),
    };
    let have = items(app, &l.id);
    if have.len() as i64 + new.len() as i64 > MAX_ITEMS {
        return Err(format!("a list holds at most {MAX_ITEMS} items; clear the ticked ones first"));
    }
    let mut pos: i64 = have.iter().filter_map(|i| i["position"].as_i64()).max().unwrap_or(0);
    let (mut ids, mut reopened, mut already, mut added) = (vec![], vec![], vec![], vec![]);
    let now = util::now_iso();
    let c = app.db.lock();
    for it in new {
        if let Some(old) = have.iter().find(|i| norm(i["text"].as_str().unwrap_or("")) == norm(&it.text)) {
            let id = old["id"].as_str().unwrap_or_default().to_string();
            if old["done"] == true {
                let _ = c.execute("UPDATE list_items SET done = 0, updated = ?2 WHERE id = ?1", params![id, now]);
                reopened.push(id);
                added.push(it.text);
            } else {
                already.push(it.text);
            }
            continue;
        }
        pos += 1;
        let id = util::new_id("itm");
        c.execute(
            "INSERT INTO list_items (id, list_id, text, quantity, note, due, done, outside, position, created, updated) VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8,?9,?9)",
            params![id, l.id, it.text, it.quantity, it.note, it.due, outside as i64, pos, now],
        ).map_err(|e| e.to_string())?;
        ids.push(id);
        added.push(it.text);
    }
    drop(c);
    touch(app, &l.id);
    app.force_index();
    let (open, _) = counts(app, &l.id);
    Ok(json!({ "list": l.name, "list_id": l.id, "created_list": made, "added": added, "already_there": already, "ids": ids, "reopened": reopened, "open": open }))
}

fn set_done(app: &App, l: &List, ids: &[String], done: bool) -> usize {
    let c = app.db.lock();
    let now = util::now_iso();
    let n = ids.iter().filter(|id| c.execute("UPDATE list_items SET done = ?3, updated = ?4 WHERE id = ?1 AND list_id = ?2", params![id, l.id, done as i64, now]).unwrap_or(0) > 0).count();
    drop(c);
    touch(app, &l.id);
    app.force_index();
    n
}

/// Tick or untick items. Called as an undo (`{args, result}`), it takes the exact ids from the result.
fn tick(app: &App, a: &Value, done: bool) -> Result<Value, String> {
    if let Some(ids) = a["result"]["ids"].as_array() {
        let l = get_list(app, a["result"]["list_id"].as_str().unwrap_or_default()).ok_or("the list is gone")?;
        let ids: Vec<String> = ids.iter().filter_map(|i| i.as_str().map(String::from)).collect();
        let n = set_done(app, &l, &ids, done);
        return Ok(json!({ "list": l.name, "list_id": l.id, "ids": ids, "changed": n }));
    }
    let l = need_list(app, a)?;
    let want = keys(a);
    if want.is_empty() {
        return Err("items is required: the words or ids of the items".into());
    }
    let all = items(app, &l.id);
    let pool: Vec<Value> = all.iter().filter(|i| i["done"] != done).cloned().collect();
    let (mut hit, missing) = pick(&pool, &want);
    // Already in the wanted state counts as done, not as missing.
    let (same, missing) = pick(&all.iter().filter(|i| i["done"] == done).cloned().collect::<Vec<_>>(), &missing);
    if hit.is_empty() && same.is_empty() {
        let open: Vec<&str> = all.iter().filter(|i| i["done"] != done).filter_map(|i| i["text"].as_str()).take(30).collect();
        return Err(format!("no item on {} matches {}; {} items are: {}", l.name, missing.join(", "), if done { "its open" } else { "its ticked" }, if open.is_empty() { "none".into() } else { open.join(", ") }));
    }
    let ids: Vec<String> = hit.iter().filter_map(|i| i["id"].as_str().map(String::from)).collect();
    set_done(app, &l, &ids, done);
    for i in &mut hit {
        i["done"] = json!(done);
    }
    let (open, _) = counts(app, &l.id);
    let mut out = json!({ "list": l.name, "list_id": l.id, "ids": ids,
        "already": same.iter().map(|i| i["text"].clone()).collect::<Vec<_>>(), "not_found": missing, "open": open });
    out[if done { "checked" } else { "unchecked" }] = hit.iter().map(|i| i["text"].clone()).collect();
    Ok(out)
}

fn delete_rows(app: &App, l: &List, rows: &[Value]) -> Vec<Value> {
    let c = app.db.lock();
    let gone: Vec<Value> = rows.iter().filter(|r| c.execute("DELETE FROM list_items WHERE id = ?1 AND list_id = ?2", params![r["id"].as_str(), l.id]).unwrap_or(0) > 0).cloned().collect();
    drop(c);
    touch(app, &l.id);
    app.force_index();
    gone
}

fn remove(app: &App, a: &Value) -> Result<Value, String> {
    let l = need_list(app, a)?;
    let want = keys(a);
    if want.is_empty() {
        return Err("items is required: the words or ids of the items".into());
    }
    let (hit, missing) = pick(&items(app, &l.id), &want);
    if hit.is_empty() {
        return Err(format!("no item on {} matches {}", l.name, missing.join(", ")));
    }
    let gone = delete_rows(app, &l, &hit);
    Ok(json!({ "list": l.name, "list_id": l.id, "removed": gone, "not_found": missing }))
}

fn clear_checked(app: &App, l: &List) -> Value {
    let done: Vec<Value> = items(app, &l.id).into_iter().filter(|i| i["done"] == true).collect();
    let gone = delete_rows(app, l, &done);
    json!({ "list": l.name, "list_id": l.id, "removed": gone, "cleared": gone.len() })
}

/// Undo of remove and clear: put the rows back as they were.
fn restore_items(app: &App, result: &Value) -> Result<Value, String> {
    let rows = result["removed"].as_array().ok_or("nothing to restore")?;
    let l = get_list(app, result["list_id"].as_str().unwrap_or_default()).ok_or("the list is gone")?;
    let c = app.db.lock();
    let now = util::now_iso();
    let mut n = 0;
    for r in rows {
        n += c.execute(
            "INSERT OR IGNORE INTO list_items (id, list_id, text, quantity, note, due, done, outside, position, created, updated) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            params![r["id"].as_str(), l.id, r["text"].as_str().unwrap_or(""), r["quantity"].as_str(), r["note"].as_str(), r["due"].as_str(),
                (r["done"] == true) as i64, (r["outside"] == true) as i64, r["position"].as_i64().unwrap_or(0), now],
        ).unwrap_or(0);
    }
    drop(c);
    touch(app, &l.id);
    app.force_index();
    Ok(json!({ "list": l.name, "restored": n }))
}

/// Undo of add_items: the new items go, the reopened ones are ticked again.
fn undo_add(app: &App, result: &Value) -> Result<Value, String> {
    let l = get_list(app, result["list_id"].as_str().unwrap_or_default()).ok_or("the list is gone")?;
    let strs = |k: &str| -> Vec<String> { result[k].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(String::from)).collect() };
    let c = app.db.lock();
    let n = strs("ids").iter().filter(|id| c.execute("DELETE FROM list_items WHERE id = ?1 AND list_id = ?2", params![id, l.id]).unwrap_or(0) > 0).count();
    drop(c);
    set_done(app, &l, &strs("reopened"), true);
    if result["created_list"] == true && items(app, &l.id).is_empty() {
        let _ = app.db.lock().execute("UPDATE lists SET status = 'deleted', updated = ?2 WHERE id = ?1", params![l.id, util::now_iso()]);
        app.force_index();
    }
    Ok(json!({ "list": l.name, "removed": n }))
}

pub fn rename(app: &App, l: &List, name: &str) -> Result<Value, String> {
    let new = display_name(name);
    if new.is_empty() {
        return Err("name is required".into());
    }
    if let Some(other) = find(app, &new).filter(|o| o.id != l.id) {
        return Err(format!("there is already a list called {}", other.name));
    }
    app.db.lock().execute("UPDATE lists SET name = ?2, updated = ?3 WHERE id = ?1", params![l.id, new, util::now_iso()]).map_err(|e| e.to_string())?;
    app.force_index();
    Ok(json!({ "list": new, "list_id": l.id, "previous": { "id": l.id, "name": l.name } }))
}

/// Delete a list. Its items are kept with it, so undo brings everything back.
pub fn delete(app: &App, l: &List) -> Value {
    let (open, done) = counts(app, &l.id);
    let _ = app.db.lock().execute("UPDATE lists SET status = 'deleted', updated = ?2 WHERE id = ?1", params![l.id, util::now_iso()]);
    app.force_index();
    json!({ "deleted": l.id, "list": l.name, "items": open + done, "previous": { "id": l.id, "name": l.name } })
}

/// Undo of rename and delete.
fn restore_list(app: &App, prev: &Value) -> Result<Value, String> {
    let id = prev["id"].as_str().ok_or("nothing to restore")?;
    let l = get_list(app, id).ok_or("no such list")?;
    let name = prev["name"].as_str().unwrap_or(&l.name).to_string();
    if let Some(other) = find(app, &name).filter(|o| o.id != l.id) {
        return Err(format!("another list is now called {}; rename it first", other.name));
    }
    app.db.lock().execute("UPDATE lists SET name = ?2, status = 'active', updated = ?3 WHERE id = ?1", params![id, name, util::now_iso()]).map_err(|e| e.to_string())?;
    app.force_index();
    Ok(json!({ "restored": id, "list": name }))
}

pub fn update_item(app: &App, l: &List, item: &str, b: &Value) -> Result<Value, String> {
    let row = items(app, &l.id).into_iter().find(|i| i["id"] == item).ok_or("no such item")?;
    let text = if b.get("text").is_some() { text_field(b, "text", 200).ok_or("text cannot be empty")? } else { row["text"].as_str().unwrap_or("").to_string() };
    let opt = |k: &str, max: usize| -> Option<String> {
        if b.get(k).is_some() { match &b[k] { Value::Number(n) => Some(n.to_string()), _ => text_field(b, k, max) } } else { row[k].as_str().map(String::from) }
    };
    let due = match opt("due", 20) { Some(d) => Some(check_due(&d)?), None => None };
    let done = b["done"].as_bool().unwrap_or(row["done"] == true);
    app.db.lock().execute(
        "UPDATE list_items SET text = ?2, quantity = ?3, note = ?4, due = ?5, done = ?6, updated = ?7 WHERE id = ?1",
        params![item, text, opt("quantity", 40), opt("note", 500), due, done as i64, util::now_iso()],
    ).map_err(|e| e.to_string())?;
    touch(app, &l.id);
    app.force_index();
    items(app, &l.id).into_iter().find(|i| i["id"] == item).ok_or_else(|| "no such item".into())
}

/// The list as plain text, for sharing: open items first, then the ticked ones.
pub fn as_text(app: &App, l: &List) -> String {
    let all = items(app, &l.id);
    let line = |i: &Value| {
        let mut s = format!("- [{}] {}", if i["done"] == true { "x" } else { " " }, i["text"].as_str().unwrap_or(""));
        if let Some(q) = i["quantity"].as_str() { s.push_str(&format!(" ({q})")); }
        if let Some(d) = i["due"].as_str() { s.push_str(&format!(", by {d}")); }
        if let Some(n) = i["note"].as_str() { s.push_str(&format!(" - {n}")); }
        s
    };
    let mut out = vec![l.name.clone()];
    out.extend(all.iter().filter(|i| i["done"] != true).map(line));
    let done: Vec<String> = all.iter().filter(|i| i["done"] == true).map(line).collect();
    if !done.is_empty() {
        out.push(String::new());
        out.push("Done:".into());
        out.extend(done);
    }
    out.join("\n") + "\n"
}

/// Lists with open items for the index: names and counts only, as outside content.
pub fn index_line(app: &App) -> Option<String> {
    let lines: Vec<String> = active_lists(app).iter().filter_map(|l| {
        let (open, _) = counts(app, &l.id);
        (open > 0).then(|| format!("- {}: {open} open", l.name))
    }).take(10).collect();
    if lines.is_empty() {
        return None;
    }
    Some(util::wrap_outside("lists:open", &lines.join("\n")))
}

/// `lists` plugin tools. `outside`: the turn read outside content, so new items are marked.
pub fn tool(app: &App, outside: bool, tool: &str, a: &Value) -> Result<Value, String> {
    match tool {
        "list_lists" => Ok(list_all(app)),
        "show_list" => {
            let l = need_list(app, a)?;
            let mut v = show(app, &l);
            if a["include_done"] == false
                && let Some(x) = v["items"].as_array_mut()
            {
                x.retain(|i| i["done"] != true);
            }
            Ok(v)
        }
        "add_items" => {
            let key = a["list"].as_str().or(a["name"].as_str()).unwrap_or_default();
            if key.trim().is_empty() {
                return Err("say which list, e.g. {\"list\": \"shopping\", \"items\": [\"milk\"]}".into());
            }
            let raw = if a.get("items").is_some() { &a["items"] } else if a.get("item").is_some() { &a["item"] } else { &a["text"] };
            add_items(app, key, raw, outside)
        }
        "check_items" => tick(app, a, true),
        "uncheck_items" => tick(app, a, false),
        "remove_items" => remove(app, a),
        "clear_checked" => need_list(app, a).map(|l| clear_checked(app, &l)),
        "rename_list" => {
            let l = need_list(app, &json!({ "list": a["list"].as_str().or(a["id"].as_str()) }))?;
            rename(app, &l, a["name"].as_str().or(a["new_name"].as_str()).unwrap_or_default())
        }
        "delete_list" => need_list(app, a).map(|l| delete(app, &l)),
        "undo_add_items" => undo_add(app, &a["result"]),
        "restore_items" => restore_items(app, &a["result"]),
        "restore_list" => restore_list(app, &a["result"]["previous"]),
        _ => Err("no such tool".into()),
    }
}

// ---------------- routes (docs/01) ----------------

type St = State<Arc<App>>;

fn err(code: StatusCode, msg: impl Into<String>) -> Response {
    let c = if code == StatusCode::NOT_FOUND { "not_found" } else { "invalid_input" };
    (code, Json(json!({ "error": { "code": c, "message": msg.into() } }))).into_response()
}

fn by_id(app: &App, id: &str) -> Result<List, Response> {
    get_list(app, id).filter(|l| l.status == "active").ok_or_else(|| err(StatusCode::NOT_FOUND, "List not found."))
}

fn done(r: Result<Value, String>, code: StatusCode) -> Response {
    match r {
        Ok(v) => (code, Json(v)).into_response(),
        Err(e) if e.starts_with("no such") => err(StatusCode::NOT_FOUND, e),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

async fn r_list(State(app): St) -> Response {
    Json(list_all(&app)).into_response()
}

/// `{name, items?}`: a new list, with items if given.
async fn r_create(State(app): St, Json(b): Json<Value>) -> Response {
    let name = b["name"].as_str().unwrap_or_default();
    let l = match create(&app, name) { Ok(l) => l, Err(e) => return err(StatusCode::BAD_REQUEST, e) };
    if b["items"].as_array().is_some_and(|x| !x.is_empty())
        && let Err(e) = add_items(&app, &l.id, &b["items"], false)
    {
        return err(StatusCode::BAD_REQUEST, e);
    }
    app.force_index();
    (StatusCode::CREATED, Json(show(&app, &l))).into_response()
}

async fn r_get(State(app): St, Path(id): Path<String>) -> Response {
    match by_id(&app, &id) { Ok(l) => Json(show(&app, &l)).into_response(), Err(r) => r }
}

/// `{name}`: rename.
async fn r_patch(State(app): St, Path(id): Path<String>, Json(b): Json<Value>) -> Response {
    let l = match by_id(&app, &id) { Ok(l) => l, Err(r) => return r };
    match rename(&app, &l, b["name"].as_str().unwrap_or_default()) {
        Ok(_) => Json(show(&app, &by_id(&app, &id).unwrap_or(l))).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

async fn r_delete(State(app): St, Path(id): Path<String>) -> Response {
    match by_id(&app, &id) { Ok(l) => { delete(&app, &l); Json(json!({ "deleted": id })).into_response() } Err(r) => r }
}

/// `{text, quantity?, note?, due?}` or `{items: [...]}`.
async fn r_add(State(app): St, Path(id): Path<String>, Json(b): Json<Value>) -> Response {
    if let Err(r) = by_id(&app, &id) {
        return r;
    }
    let raw = if b.get("items").is_some() { b["items"].clone() } else { b.clone() };
    done(add_items(&app, &id, &raw, false), StatusCode::CREATED)
}

/// `{done?, text?, quantity?, note?, due?}`. An empty string clears quantity, note or due.
async fn r_item_patch(State(app): St, Path((id, item)): Path<(String, String)>, Json(b): Json<Value>) -> Response {
    match by_id(&app, &id) { Ok(l) => done(update_item(&app, &l, &item, &b), StatusCode::OK), Err(r) => r }
}

async fn r_item_delete(State(app): St, Path((id, item)): Path<(String, String)>) -> Response {
    let l = match by_id(&app, &id) { Ok(l) => l, Err(r) => return r };
    let row: Vec<Value> = items(&app, &l.id).into_iter().filter(|i| i["id"] == item.as_str()).collect();
    if row.is_empty() {
        return err(StatusCode::NOT_FOUND, "Item not found.");
    }
    delete_rows(&app, &l, &row);
    Json(json!({ "deleted": item })).into_response()
}

/// Remove the ticked items.
async fn r_clear(State(app): St, Path(id): Path<String>) -> Response {
    match by_id(&app, &id) { Ok(l) => Json(clear_checked(&app, &l)).into_response(), Err(r) => r }
}

/// Plain text for sharing.
async fn r_text(State(app): St, Path(id): Path<String>) -> Response {
    match by_id(&app, &id) {
        Ok(l) => ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], as_text(&app, &l)).into_response(),
        Err(r) => r,
    }
}

/// `/v1/lists` routes; the API guard (Host, Origin, token) wraps them with the rest.
pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/v1/lists", get(r_list).post(r_create))
        .route("/v1/lists/{id}", get(r_get).patch(r_patch).delete(r_delete))
        .route("/v1/lists/{id}/items", post(r_add))
        .route("/v1/lists/{id}/items/{item}", patch(r_item_patch).delete(r_item_delete))
        .route("/v1/lists/{id}/clear", post(r_clear))
        .route("/v1/lists/{id}/text", get(r_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_loosely() {
        assert_eq!(norm("Shopping list"), "shopping");
        assert_eq!(norm("To-do"), "todo");
        assert_eq!(norm("list"), "list");
        assert_eq!(display_name("to do list"), "To-do");
        assert_eq!(display_name("groceries"), "Shopping");
        assert_eq!(display_name("camping gear list"), "Camping gear");
        assert!(check_due("2026-11-20").is_ok() && check_due("2026-11-20 18:00").is_ok());
        assert!(check_due("Friday").is_err() && check_due("2026-13-01").is_err());
    }

    #[test]
    fn picks_by_id_words_or_one_partial_match() {
        let all = vec![json!({ "id": "a", "text": "Oat milk" }), json!({ "id": "b", "text": "Eggs" }), json!({ "id": "c", "text": "Egg noodles" })];
        let (hit, miss) = pick(&all, &["oat milk".into(), "b".into(), "milk".into(), "egg".into(), "bread".into()]);
        assert_eq!(hit.iter().map(|i| i["id"].as_str().unwrap()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(miss, ["egg", "bread"], "egg matches two items, so it is not guessed");
    }
}
