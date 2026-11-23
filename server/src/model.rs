//! Model adapters (docs/05 #4). First: any OpenAI-compatible server (also covers Ollama, vLLM).
//! `Mock` replays a script, for golden conversations and demos with no model.

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
    /// Provider data that must come back with this call next time (Gemini's `extra_content`
    /// with its thought signature). Stored in history as is; never shown to the model as text.
    pub extra: Option<Value>,
}

#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub text: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tokens: u64,
    /// The model's own reasoning text, when the provider sends it. Shown to clients; never stored as history.
    pub thinking: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Request {
    pub system: String,
    /// What the turn is for, so an adapter may tune itself: `Purpose::Chat` (a person is waiting),
    /// `Purpose::Deep` (dreaming), `Purpose::Background` (jobs, triggers, everything else).
    pub purpose: Purpose,
    /// OpenAI chat format: {role, content, tool_calls?, tool_call_id?}
    pub messages: Vec<Value>,
    /// OpenAI function tool definitions.
    pub tools: Vec<Value>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Purpose {
    #[default]
    Background,
    Chat,
    Deep,
}

/// Blocking on purpose; the engine runs it on a blocking thread.
pub trait Adapter: Send + Sync {
    fn name(&self) -> String;
    fn chat(&self, req: &Request) -> Result<Reply, String>;
    /// Like `chat`, but calls `on_text` with each piece of reply text as it arrives.
    /// `on_text` returns false to stop (the turn was cancelled). Adapters that cannot
    /// stream use this default: one call, no pieces.
    /// `on_note` reports waits, such as a retry after a rate limit.
    fn chat_stream(&self, req: &Request, on_text: &mut dyn FnMut(&str) -> bool, on_note: &mut dyn FnMut(&str)) -> Result<Reply, String> {
        let _ = (on_text, on_note);
        self.chat(req)
    }
}

/// The assistant message to store in history, in the same chat format.
pub fn reply_to_message(r: &Reply) -> Value {
    let mut m = json!({ "role": "assistant", "content": r.text });
    if !r.tool_calls.is_empty() {
        m["tool_calls"] = Value::Array(
            r.tool_calls
                .iter()
                .map(|c| {
                    let mut t = json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.args.to_string() } });
                    if let Some(x) = &c.extra {
                        t["extra_content"] = x.clone();
                    }
                    t
                })
                .collect(),
        );
    }
    m
}

/// Read an OpenAI-style event stream: text pieces go to `on_text`; tool calls are put together
/// from their fragments and returned once the stream ends.
pub fn read_sse<R: std::io::BufRead>(mut r: R, on_text: &mut dyn FnMut(&str) -> bool) -> Result<Reply, String> {
    let mut text = String::new();
    let mut thinking = String::new();
    let mut calls: std::collections::BTreeMap<usize, (String, String, String, Option<Value>)> = Default::default();
    let mut tokens = 0u64;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(|e| format!("model stream failed: {e}"))? == 0 {
            break;
        }
        let Some(data) = line.trim().strip_prefix("data:") else { continue };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(format!("model stream error: {}", e.to_string().chars().take(300).collect::<String>()));
        }
        if let Some(t) = v["usage"]["total_tokens"].as_u64() {
            tokens = t;
        }
        let delta = &v["choices"][0]["delta"];
        if let Some(c) = delta["content"].as_str().filter(|c| !c.is_empty()) {
            text.push_str(c);
            if !on_text(c) {
                return Err("cancelled".into());
            }
        }
        if let Some(c) = delta["reasoning_content"].as_str().or_else(|| delta["reasoning"].as_str()) {
            thinking.push_str(c);
        }
        for tc in delta["tool_calls"].as_array().into_iter().flatten() {
            let e = calls.entry(tc["index"].as_u64().unwrap_or(0) as usize).or_default();
            if let Some(id) = tc["id"].as_str().filter(|s| !s.is_empty()) {
                e.0 = id.into();
            }
            if let Some(n) = tc["function"]["name"].as_str().filter(|_| e.1.is_empty()) {
                e.1 = n.into();
            }
            if let Some(a) = tc["function"]["arguments"].as_str() {
                e.2.push_str(a);
            }
            if let Some(x) = tc.get("extra_content").filter(|x| !x.is_null()) {
                e.3 = Some(x.clone());
            }
        }
    }
    let tool_calls: Vec<ToolCall> = calls
        .into_iter()
        .enumerate()
        .map(|(i, (_, (id, name, raw, extra)))| ToolCall {
            id: if id.is_empty() { format!("call_{i}") } else { id },
            name,
            args: if raw.trim().is_empty() { json!({}) } else { serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "_unparsed": raw })) },
            extra,
        })
        .collect();
    if tokens == 0 {
        // The server sent no usage numbers; estimate so budgets still count.
        tokens = (text.len() + thinking.len() + tool_calls.iter().map(|c| c.args.to_string().len()).sum::<usize>()) as u64 / 4;
    }
    Ok(Reply {
        text: Some(text).filter(|s| !s.is_empty()),
        tool_calls,
        tokens,
        thinking: Some(thinking).filter(|s| !s.is_empty()),
    })
}

pub fn parse_openai(v: &Value) -> Result<Reply, String> {
    let msg = &v["choices"][0]["message"];
    if msg.is_null() {
        return Err("model reply has no choices".into());
    }
    let mut tool_calls = vec![];
    for (i, c) in msg["tool_calls"].as_array().into_iter().flatten().enumerate() {
        let name = c["function"]["name"].as_str().unwrap_or_default().to_string();
        let raw = &c["function"]["arguments"];
        // Some servers send a JSON string, some an object.
        let args = match raw {
            Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({ "_unparsed": s })),
            Value::Object(_) => raw.clone(),
            _ => json!({}),
        };
        let id = c["id"].as_str().map(String::from).unwrap_or_else(|| format!("call_{i}"));
        tool_calls.push(ToolCall { id, name, args, extra: c.get("extra_content").filter(|x| !x.is_null()).cloned() });
    }
    Ok(Reply {
        text: msg["content"].as_str().filter(|s| !s.is_empty()).map(String::from),
        tool_calls,
        tokens: v["usage"]["total_tokens"].as_u64().unwrap_or(0),
        thinking: msg["reasoning_content"].as_str().or_else(|| msg["reasoning"].as_str()).filter(|s| !s.is_empty()).map(String::from),
    })
}

// ---------------- Mock ----------------

/// Script entries: `{"text": "..."}` and/or `{"tool_calls": [{"name": "...", "args": {...}}]}`.
pub struct Mock {
    script: Mutex<VecDeque<Reply>>,
    /// Per script step: `"stream": true` makes the mock send its text word by word.
    stream: Mutex<VecDeque<bool>>,
    /// Per script step: `"error": "..."` makes the model call fail with that text.
    errors: Mutex<VecDeque<Option<String>>>,
    seen: Mutex<Vec<Request>>,
}

impl Mock {
    pub fn new(script: &[Value]) -> Self {
        let mut q = VecDeque::new();
        let mut flags = VecDeque::new();
        let mut errors = VecDeque::new();
        for (step, s) in script.iter().enumerate() {
            flags.push_back(s["stream"].as_bool().unwrap_or(false));
            errors.push_back(s["error"].as_str().map(String::from));
            let calls = s["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, c)| ToolCall {
                    id: format!("call_{step}_{i}"),
                    name: c["name"].as_str().unwrap_or_default().into(),
                    args: c.get("args").cloned().unwrap_or(json!({})),
                    extra: c.get("extra").cloned(),
                })
                .collect();
            q.push_back(Reply { text: s["text"].as_str().map(String::from), tool_calls: calls, tokens: 10, thinking: None });
        }
        Mock { script: Mutex::new(q), stream: Mutex::new(flags), errors: Mutex::new(errors), seen: Mutex::new(vec![]) }
    }

    pub fn from_file(path: &str) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("{path}: {e}"))?;
        Ok(Mock::new(v.as_array().map(|a| a.as_slice()).unwrap_or(&[])))
    }

    /// Add more script steps (tests use this between turns).
    pub fn push(&self, script: &[Value]) {
        let more = Mock::new(script);
        self.script.lock().unwrap().extend(more.script.into_inner().unwrap());
        self.stream.lock().unwrap().extend(more.stream.into_inner().unwrap());
        self.errors.lock().unwrap().extend(more.errors.into_inner().unwrap());
    }

    /// Every request the engine sent, for tests to inspect.
    pub fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
}

impl Adapter for Mock {
    fn name(&self) -> String {
        "mock".into()
    }
    fn chat(&self, req: &Request) -> Result<Reply, String> {
        self.seen.lock().unwrap().push(req.clone());
        if let Some(e) = self.errors.lock().unwrap().pop_front().flatten() {
            self.script.lock().unwrap().pop_front();
            return Err(e);
        }
        Ok(self.script.lock().unwrap().pop_front().unwrap_or(Reply {
            text: Some("(mock model: script finished)".into()),
            tool_calls: vec![],
            tokens: 1,
            thinking: None,
        }))
    }
    fn chat_stream(&self, req: &Request, on_text: &mut dyn FnMut(&str) -> bool, _on_note: &mut dyn FnMut(&str)) -> Result<Reply, String> {
        let flag = self.stream.lock().unwrap().pop_front().unwrap_or(false);
        let reply = self.chat(req)?;
        if flag {
            for piece in reply.text.as_deref().unwrap_or_default().split_inclusive(' ') {
                if !on_text(piece) {
                    return Err("cancelled".into());
                }
            }
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_stream_of_text_and_tool_call_fragments() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"load\",\"arguments\":\"{\\\"id\\\":\"}}]}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"core\\\"}\"}}]}}]}\n\n\
                   data: {\"usage\":{\"total_tokens\":7},\"choices\":[]}\n\n\
                   data: [DONE]\n\n";
        let mut pieces = vec![];
        let r = read_sse(std::io::Cursor::new(sse), &mut |t| { pieces.push(t.to_string()); true }).unwrap();
        assert_eq!(pieces, ["Hel", "lo"]);
        assert_eq!(r.text.as_deref(), Some("Hello"));
        assert_eq!(r.tool_calls[0].name, "load");
        assert_eq!(r.tool_calls[0].args["id"], "core");
        assert_eq!(r.tokens, 7);
    }

    #[test]
    fn a_stream_stops_when_the_callback_says_so() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n";
        assert_eq!(read_sse(std::io::Cursor::new(sse), &mut |_| false).unwrap_err(), "cancelled");
    }

    #[test]
    fn gemini_thought_signatures_go_back_with_their_call() {
        let v = json!({ "choices": [{ "message": { "content": null, "tool_calls": [{ "id": "c1", "type": "function",
            "function": { "name": "load", "arguments": "{\"id\":\"plugin:x\"}" }, "extra_content": { "google": { "thought_signature": "SIG" } } }] } }] });
        let r = parse_openai(&v).unwrap();
        let m = reply_to_message(&r);
        assert_eq!(m["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "SIG");
        let sse = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c2\",\"function\":{\"name\":\"load\",\"arguments\":\"{}\"},\"extra_content\":{\"google\":{\"thought_signature\":\"S2\"}}}]}}]}\n\ndata: [DONE]\n";
        let r = read_sse(std::io::Cursor::new(sse), &mut |_| true).unwrap();
        assert_eq!(reply_to_message(&r)["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "S2");
    }

    #[test]
    fn parses_string_and_object_arguments() {
        let v = json!({"choices":[{"message":{"content":"","tool_calls":[
            {"id":"a","function":{"name":"load","arguments":"{\"id\":\"plugin:files\"}"}},
            {"id":"b","function":{"name":"unload","arguments":{"id":"plugin:web"}}}
        ]}}],"usage":{"total_tokens":42}});
        let r = parse_openai(&v).unwrap();
        assert_eq!(r.text, None);
        assert_eq!(r.tool_calls[0].args["id"], "plugin:files");
        assert_eq!(r.tool_calls[1].args["id"], "plugin:web");
        assert_eq!(r.tokens, 42);
    }
}
