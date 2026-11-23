//! Computers you connect (plugin `computers`, docs/11).
//!
//! A computer runs the small `agent-node` program with a `node` token. It keeps asking the
//! server for work (a long poll: one request waits up to 25 s), runs what it gets inside the
//! folders its owner allowed, and posts the answer back. The computer always dials out, so it
//! works behind any router, and the server never needs to reach in.
//!
//! The model asks through the `computers` plugin. Reads run by themselves; writes, copies,
//! moves and deletes ask the user first (the Safety Gate), and the computer checks every path
//! against its own allowed folders again before it touches anything.

use crate::{App, util};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{Notify, oneshot};

/// How long one poll waits for work.
pub const POLL_SECS: u64 = 25;
/// A computer that has not polled for this long is offline.
const ONLINE_SECS: i64 = 75;
/// How long a tool call waits for the computer's answer.
const ANSWER_SECS: u64 = 45;

pub const TOOLS: &[&str] = &["info", "list", "read", "stat", "search", "write", "mkdir", "copy", "move", "delete"];

struct Job {
    id: String,
    tool: String,
    args: Value,
}

#[derive(Default)]
struct Inner {
    queues: HashMap<String, VecDeque<Job>>,
    waiters: HashMap<String, (String, oneshot::Sender<Result<Value, String>>)>,
    /// token id -> (last poll, what the computer said about itself)
    seen: HashMap<String, (i64, Value)>,
}

#[derive(Default)]
pub struct Hub {
    inner: Mutex<Inner>,
    notify: Notify,
}

impl Hub {
    /// One poll from a computer: wait for work, up to POLL_SECS.
    pub async fn poll(&self, node: &str, info: Value) -> Option<Value> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(POLL_SECS);
        loop {
            let wait = self.notify.notified();
            {
                let mut g = self.inner.lock().unwrap();
                let keep = if info.is_object() { info.clone() } else { g.seen.get(node).map(|s| s.1.clone()).unwrap_or(Value::Null) };
                g.seen.insert(node.to_string(), (util::unix_now(), keep));
                // A connection test (`agent-node status`) only says hello; it never takes a job.
                if info["probe"] == true {
                    return None;
                }
                if let Some(j) = g.queues.get_mut(node).and_then(|q| q.pop_front()) {
                    return Some(json!({ "id": j.id, "tool": j.tool, "args": j.args }));
                }
            }
            if tokio::time::timeout_at(deadline, wait).await.is_err() {
                return None;
            }
        }
    }

    /// The computer's answer to a job. False when no one waits for it (late, or not this computer's).
    pub fn answer(&self, node: &str, job: &str, result: Result<Value, String>) -> bool {
        let mut g = self.inner.lock().unwrap();
        match g.waiters.remove(job) {
            Some((owner, tx)) if owner == node => tx.send(result).is_ok(),
            Some(other) => {
                g.waiters.insert(job.to_string(), other);
                false
            }
            None => false,
        }
    }

    pub fn last_seen(&self, node: &str) -> Option<(i64, Value)> {
        self.inner.lock().unwrap().seen.get(node).cloned()
    }

    fn enqueue(&self, node: &str, tool: &str, args: Value) -> (String, oneshot::Receiver<Result<Value, String>>) {
        let id = util::new_id("njob");
        let (tx, rx) = oneshot::channel();
        {
            let mut g = self.inner.lock().unwrap();
            g.waiters.insert(id.clone(), (node.to_string(), tx));
            g.queues.entry(node.to_string()).or_default().push_back(Job { id: id.clone(), tool: tool.into(), args });
        }
        self.notify.notify_waiters();
        (id, rx)
    }

    fn drop_job(&self, node: &str, id: &str) {
        let mut g = self.inner.lock().unwrap();
        g.waiters.remove(id);
        if let Some(q) = g.queues.get_mut(node) {
            q.retain(|j| j.id != id);
        }
    }
}

/// Every computer with a node token, and whether it is online now.
pub fn list(app: &App) -> Vec<Value> {
    let rows: Vec<(String, String, Option<String>)> = {
        let c = app.db.lock();
        let mut st = c.prepare("SELECT id, label, created FROM tokens WHERE scope = 'node' AND revoked = 0 ORDER BY created").expect("prepare");
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map(|x| x.flatten().collect()).unwrap_or_default()
    };
    let now = util::unix_now();
    rows.into_iter().map(|(id, label, created)| {
        let seen = app.nodes.last_seen(&id);
        let online = seen.as_ref().is_some_and(|s| now - s.0 < ONLINE_SECS);
        let info = seen.as_ref().map(|s| s.1.clone()).unwrap_or(Value::Null);
        json!({ "id": id, "name": label, "online": online, "last_seen": seen.map(|s| util::iso(s.0)), "created": created,
            "os": info["os"], "hostname": info["hostname"], "roots": info["roots"], "read_only": info["read_only"], "version": info["version"] })
    }).collect()
}

/// Find a computer by name (its token label) or id. With one computer, the name may be left out.
fn resolve(app: &App, name: Option<&str>) -> Result<Value, String> {
    let all = list(app);
    let pick = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => all.iter().find(|d| d["id"] == n || d["name"].as_str().is_some_and(|l| l.eq_ignore_ascii_case(n))).cloned(),
        None if all.len() == 1 => all.first().cloned(),
        None => return Err(format!("say which computer: {}", all.iter().filter_map(|d| d["name"].as_str()).collect::<Vec<_>>().join(", "))),
    };
    let d = pick.ok_or_else(|| if all.is_empty() { "No computer is connected. Add one under Settings, Computers.".to_string() } else { format!("no computer called that. Computers: {}", all.iter().filter_map(|d| d["name"].as_str()).collect::<Vec<_>>().join(", ")) })?;
    if d["online"] != true {
        return Err(format!("{} is offline (last seen {}). Is the computer on and the node program running?", d["name"].as_str().unwrap_or("That computer"), d["last_seen"].as_str().unwrap_or("never")));
    }
    Ok(d)
}

/// Run one tool on a computer and wait for its answer.
pub async fn call(app: &App, tool: &str, args: &Value) -> Result<Value, String> {
    if tool == "list_devices" {
        return Ok(json!({ "computers": list(app) }));
    }
    if !TOOLS.contains(&tool) {
        return Err(format!("no such tool '{tool}'"));
    }
    let d = resolve(app, args["computer"].as_str())?;
    let node = d["id"].as_str().unwrap_or("").to_string();
    let mut a = args.clone();
    if let Some(o) = a.as_object_mut() {
        o.remove("computer");
    }
    let (id, rx) = app.nodes.enqueue(&node, tool, a);
    match tokio::time::timeout(Duration::from_secs(ANSWER_SECS), rx).await {
        Ok(Ok(r)) => r.map(|mut v| { if let Some(o) = v.as_object_mut() { o.insert("computer".into(), d["name"].clone()); } v }),
        Ok(Err(_)) => Err("the computer dropped the job".into()),
        Err(_) => {
            app.nodes.drop_job(&node, &id);
            Err(format!("{} did not answer in {ANSWER_SECS} seconds", d["name"].as_str().unwrap_or("The computer")))
        }
    }
}
