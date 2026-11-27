//! Proton Lumo through a local browser bridge (docs/12).
//!
//! Lumo has no public API yet. A small bridge program (outside this repo, for example
//! Lumo-Api-V2) runs one headless browser signed in to the owner's own Lumo account and takes
//! `POST / {"prompt": "..."}`, answering with the reply as plain text. This adapter talks to it.
//!
//! - The bridge address must be loopback (`AGENT_LUMO_URL`, for example `http://127.0.0.1:3333`).
//! - Lumo has no tool calls, so the whole request (system text, tools, chat) is written out as
//!   one prompt and tool calls come back as `CALL {"name": ..., "arguments": {...}}` lines.
//! - One request at a time. When Lumo limits the account, the adapter waits out a cool-down
//!   (`AGENT_LUMO_COOLDOWN_MINS`, default 30) and the local model answers meanwhile. It never
//!   changes address, account or browser to get around a limit.

use crate::model::{Adapter, Purpose, Reply, Request, ToolCall};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Lumo {
    url: String,
    /// `AGENT_LUMO_TOKEN`: the bridge's bearer token, if it has one.
    token: Option<String>,
    agent: ureq::Agent,
    /// Unix time until which Lumo is not asked (it limited us, or the bridge was down).
    blocked_until: AtomicI64,
    cooldown_secs: i64,
    /// The local model that answers while Lumo cools down.
    fallback: Option<Arc<dyn Adapter>>,
    one_at_a_time: Mutex<()>,
}

/// `AGENT_LUMO_URL`, only when it is a loopback `http` address.
pub fn bridge_url() -> Option<String> {
    let u = std::env::var("AGENT_LUMO_URL").ok()?;
    let u = u.trim().trim_end_matches('/').to_string();
    is_loopback(&u).then_some(u)
}

fn is_loopback(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("http://") else { return false };
    let host = rest.split('/').next().unwrap_or("");
    let host = if host.starts_with('[') { host.split(']').next().unwrap_or("").trim_start_matches('[') } else { host.split(':').next().unwrap_or("") };
    host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

impl Lumo {
    pub fn new(url: &str, fallback: Option<Arc<dyn Adapter>>) -> Self {
        let secs = std::env::var("AGENT_LUMO_TIMEOUT_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
        let cool = std::env::var("AGENT_LUMO_COOLDOWN_MINS").ok().and_then(|v| v.parse::<i64>().ok()).unwrap_or(30);
        Lumo {
            url: url.into(),
            token: std::env::var("AGENT_LUMO_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).or_else(|| crate::config::credential("lumo_token")),
            agent: ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs))).http_status_as_error(false).build().into(),
            blocked_until: AtomicI64::new(0),
            cooldown_secs: cool.max(1) * 60,
            fallback,
            one_at_a_time: Mutex::new(()),
        }
    }

    /// Ask the bridge once. `Err((limited, why))`.
    fn ask(&self, prompt: &str, purpose: Purpose) -> Result<String, (bool, String)> {
        let (model, speed) = lumo_mode(purpose);
        let body = serde_json::json!({ "prompt": prompt, "model": model, "speed": speed }).to_string();
        let mut tries = 0;
        loop {
            let mut call = self.agent.post(&self.url).header("Content-Type", "application/json");
            if let Some(t) = &self.token {
                call = call.header("Authorization", &format!("Bearer {t}"));
            }
            let resp = call.send(body.as_str());
            let (st, text) = match resp {
                Ok(mut r) => (r.status().as_u16(), r.body_mut().read_to_string().unwrap_or_default()),
                Err(e) => return Err((false, format!("the Lumo bridge at {} did not answer ({e})", self.url))),
            };
            // The bridge says "Model busy" while another prompt runs; wait a little.
            if st != 200 && text.contains("busy") && tries < 3 {
                tries += 1;
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            if st == 429 || is_limit_text(&text) {
                return Err((true, "Lumo says this account has reached its limit for now".into()));
            }
            if st != 200 {
                return Err((false, format!("the Lumo bridge failed ({st}): {}", text.chars().take(200).collect::<String>())));
            }
            if text.trim().is_empty() {
                return Err((false, "Lumo sent an empty reply".into()));
            }
            return Ok(text);
        }
    }

    fn fall_back(&self, req: &Request, why: &str, on_text: &mut dyn FnMut(&str) -> bool, on_note: &mut dyn FnMut(&str)) -> Result<Reply, String> {
        match &self.fallback {
            Some(f) => {
                on_note(&format!("{why}. Your local model answers instead."));
                f.chat_stream(req, on_text, on_note)
            }
            None => Err(format!("{why}. Try again later or pick another model.")),
        }
    }
}

impl Adapter for Lumo {
    fn name(&self) -> String {
        "lumo".into()
    }

    fn chat(&self, req: &Request) -> Result<Reply, String> {
        self.chat_stream(req, &mut |_| true, &mut |_| {})
    }

    fn chat_stream(&self, req: &Request, on_text: &mut dyn FnMut(&str) -> bool, on_note: &mut dyn FnMut(&str)) -> Result<Reply, String> {
        let now = crate::util::unix_now();
        let until = self.blocked_until.load(Ordering::Relaxed);
        if until > now {
            return self.fall_back(req, &format!("Lumo is resting for about {} more minutes", (until - now + 59) / 60), on_text, on_note);
        }
        let prompt = flatten(req);
        let got = {
            let _one = self.one_at_a_time.lock().unwrap_or_else(|e| e.into_inner());
            self.ask(&prompt, req.purpose)
        };
        match got {
            Ok(text) => {
                let mut r = parse_reply(&text, &req.tools);
                r.tokens = ((prompt.len() + text.len()) / 4) as u64;
                Ok(r)
            }
            Err((limited, why)) => {
                // A limit waits out the full cool-down; a bridge that is down is tried again in 2 minutes.
                let wait = if limited { self.cooldown_secs } else { 120 };
                self.blocked_until.store(crate::util::unix_now() + wait, Ordering::Relaxed);
                self.fall_back(req, &why, on_text, on_note)
            }
        }
    }
}

/// Lumo's model and speed for each kind of work: a person waiting gets Lite in Fast mode,
/// dreaming gets Max in Thinking mode, other background work Lite in Thinking mode.
pub fn lumo_mode(purpose: Purpose) -> (&'static str, &'static str) {
    match purpose {
        Purpose::Chat => ("Lumo 2.0 Lite", "fast"),
        Purpose::Deep => ("Lumo 2.0 Max", "thinking"),
        Purpose::Background => ("Lumo 2.0 Lite", "thinking"),
    }
}

fn is_limit_text(t: &str) -> bool {
    let t = t.to_lowercase();
    // Only short texts: a real answer may talk about limits.
    t.len() < 400
        && ["rate limit", "too many requests", "reached your limit", "reached the limit", "usage limit", "limit reached", "try again later"]
            .iter()
            .any(|p| t.contains(p))
}

fn content_text(m: &Value) -> String {
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// The whole request as one prompt for a chat box.
pub fn flatten(req: &Request) -> String {
    let mut out = String::new();
    out.push_str("You are the brain of the user's own personal assistant program, which runs on their server. ");
    out.push_str("You are not chatting in Lumo's normal way here: the program reads your reply. ");
    out.push_str("The tools listed below are real. When you write a CALL line, the program runs that tool for you and sends you the result. ");
    out.push_str("So you CAN do what the tools do (set alarms, play music, read files...). Never say you cannot do something a tool below does: call it.\n\n## Instructions\n");
    out.push_str(req.system.trim());
    out.push('\n');
    if !req.tools.is_empty() {
        out.push_str("\n## Tools\nTo use a tool, write one line per call, exactly like this and nothing else on the line:\n");
        out.push_str("CALL {\"name\": \"tool_name\", \"arguments\": {...}}\n");
        out.push_str("After tool calls, stop and wait: the results come in the next message. Without a tool call, just reply in plain words.\n");
        for t in &req.tools {
            let f = &t["function"];
            let name = f["name"].as_str().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            out.push_str(&format!("- {name}: {}\n  arguments: {}\n", f["description"].as_str().unwrap_or("").trim(), f["parameters"]));
        }
    }
    out.push_str("\n## Conversation\n");
    for m in &req.messages {
        let text = content_text(m);
        match m["role"].as_str().unwrap_or("") {
            "user" => out.push_str(&format!("\nUser: {text}\n")),
            "assistant" => {
                out.push_str("\nAssistant:");
                if !text.trim().is_empty() {
                    out.push_str(&format!(" {text}"));
                }
                out.push('\n');
                for c in m["tool_calls"].as_array().into_iter().flatten() {
                    let args: Value = c["function"]["arguments"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
                    out.push_str(&format!("CALL {}\n", serde_json::json!({ "name": c["function"]["name"], "arguments": args })));
                }
            }
            "tool" => out.push_str(&format!("\nTool result ({}):\n{text}\n", m["tool_call_id"].as_str().unwrap_or("?"))),
            "system" => out.push_str(&format!("\nNote: {text}\n")),
            _ => {}
        }
    }
    out.push_str("\nWrite the next Assistant message now (only that message).\n");
    out
}

/// Text and `CALL` lines back into a reply. Calls to tools that were not offered are dropped.
pub fn parse_reply(text: &str, tools: &[Value]) -> Reply {
    let known = |n: &str| tools.iter().any(|t| t["function"]["name"] == n);
    let mut calls = Vec::new();
    let mut words = Vec::new();
    for line in text.lines() {
        let l = line.trim().trim_start_matches('`').trim_end_matches('`').trim();
        let parsed = l.strip_prefix("CALL").map(str::trim_start).and_then(|j| serde_json::Deserializer::from_str(j).into_iter::<Value>().next()?.ok());
        match parsed {
            Some(v) if v["name"].as_str().is_some_and(known) => {
                let args = if v["arguments"].is_object() { v["arguments"].clone() } else { serde_json::json!({}) };
                calls.push(ToolCall { id: format!("call_{}", crate::util::random_hex(6)), name: v["name"].as_str().unwrap_or("").into(), args, extra: None });
            }
            Some(_) => {}
            None => words.push(line),
        }
    }
    let text = words.join("\n").trim().to_string();
    Reply { text: (!text.is_empty()).then_some(text), tool_calls: calls, tokens: 0, thinking: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tools() -> Vec<Value> {
        vec![json!({ "type": "function", "function": { "name": "create_alarm", "description": "Set an alarm", "parameters": { "type": "object" } } })]
    }

    #[test]
    fn only_loopback_bridges() {
        assert!(is_loopback("http://127.0.0.1:3333"));
        assert!(is_loopback("http://localhost:3333/"));
        assert!(is_loopback("http://[::1]:3333"));
        assert!(!is_loopback("http://192.168.1.5:3333"));
        assert!(!is_loopback("https://127.0.0.1:3333"));
        assert!(!is_loopback("http://127.0.0.1.evil.com"));
    }

    #[test]
    fn prompt_holds_system_tools_and_chat() {
        let req = Request {
            purpose: Default::default(),
            system: "Be brief.".into(),
            tools: tools(),
            messages: vec![
                json!({ "role": "user", "content": "wake me at 7" }),
                json!({ "role": "assistant", "content": null, "tool_calls": [{ "id": "c1", "type": "function", "function": { "name": "create_alarm", "arguments": "{\"time\":\"07:00\"}" } }] }),
                json!({ "role": "tool", "tool_call_id": "c1", "content": "[core] RESULT: DONE" }),
            ],
        };
        let p = flatten(&req);
        assert!(p.contains("Be brief."));
        assert!(p.contains("- create_alarm: Set an alarm"));
        assert!(p.contains("User: wake me at 7"));
        let call = p.lines().find(|l| l.starts_with("CALL {") && l.contains("create_alarm")).unwrap();
        assert!(call.contains("\"name\":\"create_alarm\"") && call.contains("\"time\":\"07:00\""), "{call}");
        assert!(p.contains("Tool result (c1):\n[core] RESULT: DONE"));
    }

    #[test]
    fn calls_and_words_come_back_apart() {
        let r = parse_reply("Setting it now.\nCALL {\"name\": \"create_alarm\", \"arguments\": {\"time\": \"07:00\"}}\n`CALL {\"name\": \"rm_rf\", \"arguments\": {}}`", &tools());
        assert_eq!(r.text.as_deref(), Some("Setting it now."));
        assert_eq!(r.tool_calls.len(), 1);
        assert_eq!(r.tool_calls[0].name, "create_alarm");
        assert_eq!(r.tool_calls[0].args["time"], "07:00");
        let plain = parse_reply("Hi! How can I help?", &tools());
        assert!(plain.tool_calls.is_empty());
        assert_eq!(plain.text.as_deref(), Some("Hi! How can I help?"));
    }

    #[test]
    fn chats_are_fast_and_dreaming_gets_max() {
        assert_eq!(lumo_mode(Purpose::Chat), ("Lumo 2.0 Lite", "fast"));
        assert_eq!(lumo_mode(Purpose::Deep), ("Lumo 2.0 Max", "thinking"));
        assert_eq!(lumo_mode(Purpose::Background), ("Lumo 2.0 Lite", "thinking"));
    }

    #[test]
    fn limit_messages_are_spotted() {
        assert!(is_limit_text("Error: You've reached your limit. Try again later."));
        assert!(!is_limit_text("Paris is the capital of France."));
    }

    struct Local;
    impl Adapter for Local {
        fn name(&self) -> String { "local".into() }
        fn chat(&self, _: &Request) -> Result<Reply, String> {
            Ok(Reply { text: Some("from local".into()), ..Default::default() })
        }
    }

    #[test]
    fn a_bridge_that_is_down_hands_over_to_the_local_model() {
        // Port 9 on loopback: nothing listens there.
        let l = Lumo::new("http://127.0.0.1:9", Some(Arc::new(Local)));
        let req = Request { purpose: Default::default(), system: String::new(), messages: vec![json!({ "role": "user", "content": "hi" })], tools: vec![] };
        let mut notes = Vec::new();
        let r = l.chat_stream(&req, &mut |_| true, &mut |n| notes.push(n.to_string())).unwrap();
        assert_eq!(r.text.as_deref(), Some("from local"));
        assert!(notes[0].contains("Lumo bridge"));
        // Now it rests: the next turn goes straight to the local model.
        let r = l.chat(&req).unwrap();
        assert_eq!(r.text.as_deref(), Some("from local"));
        assert!(l.blocked_until.load(Ordering::Relaxed) > crate::util::unix_now());
    }
}
