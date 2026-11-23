//! Adapter for any OpenAI-compatible chat server. Retries rate limits and server errors with a
//! short wait, and turns the final error into a plain sentence the user can act on.

use crate::model::{Adapter, Reply, Request, parse_openai, read_sse};
use serde_json::{Value, json};
use std::time::Duration;

type Resp = ureq::http::Response<ureq::Body>;

pub struct OpenAiCompat {
    base_url: String,
    model: String,
    api_key: Option<String>,
    agent: ureq::Agent,
    /// Waits between tries. Three waits = four tries in all.
    retry_delays: Vec<Duration>,
}

impl OpenAiCompat {
    pub fn new(base_url: &str, model: &str, api_key: Option<String>) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(180)))
            .http_status_as_error(false)
            .build()
            .into();
        OpenAiCompat {
            base_url: base_url.trim_end_matches('/').into(),
            model: model.into(),
            api_key,
            agent,
            retry_delays: vec![Duration::from_secs(2), Duration::from_secs(6), Duration::from_secs(15)],
        }
    }

    pub fn with_retry_delays(mut self, delays: Vec<Duration>) -> Self {
        self.retry_delays = delays;
        self
    }

    /// Send the request. A 429, a 5xx or a network failure is tried again after a wait (the
    /// server's `Retry-After` wins, up to 30 seconds). `on_note` says what is happening.
    fn post(&self, body: &Value, on_note: &mut dyn FnMut(&str)) -> Result<Resp, String> {
        let mut attempt = 0;
        loop {
            let mut call = self.agent.post(&format!("{}/chat/completions", self.base_url)).header("Content-Type", "application/json");
            if let Some(k) = &self.api_key {
                call = call.header("Authorization", &format!("Bearer {k}"));
            }
            let (why, hint, last) = match call.send(body.to_string()) {
                Ok(resp) => {
                    let st = resp.status().as_u16();
                    if st != 429 && !(500..600).contains(&st) {
                        return Ok(resp);
                    }
                    let hint = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok());
                    (if st == 429 { "rate limited".to_string() } else { format!("server error {st}") }, hint, Ok(resp))
                }
                Err(e) => ("no answer".to_string(), None, Err(format!("I could not reach the model provider ({e}). Check the network and the server address."))),
            };
            if attempt >= self.retry_delays.len() {
                return last;
            }
            let wait = hint.map(|s| Duration::from_secs(s.min(30))).unwrap_or(self.retry_delays[attempt]);
            on_note(&format!("The model provider said: {why}. Trying again in {}s ({} of {}).", wait.as_secs(), attempt + 1, self.retry_delays.len()));
            std::thread::sleep(wait);
            attempt += 1;
        }
    }

    fn body(&self, req: &Request, stream: bool) -> Value {
        let mut messages = vec![json!({ "role": "system", "content": req.system })];
        // Some servers (Ollama) refuse a message whose content is null. An empty reply is "".
        messages.extend(req.messages.iter().cloned().map(|mut m| {
            if m["content"].is_null() {
                m["content"] = json!("");
            }
            m
        }));
        let mut body = json!({ "model": self.model, "messages": messages });
        if stream {
            body["stream"] = json!(true);
            body["stream_options"] = json!({ "include_usage": true });
        }
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(req.tools.clone());
        }
        body
    }

    /// A failed answer in words the user can act on.
    fn explain(&self, status: u16, text: &str) -> String {
        match status {
            429 => "The model provider is rate limiting requests (HTTP 429). I waited and tried again, but it is still busy. Wait a minute and send your message again.".into(),
            500..=599 => format!("The model provider is having trouble (HTTP {status}). I tried again a few times. Try again in a moment."),
            401 | 403 => format!("The model provider refused the API key (HTTP {status}). Check the key in the server settings."),
            404 | 410 => format!("The model '{}' is not available (HTTP {status}). It may have been retired. Choose another model.", self.model),
            _ => format!("model returned HTTP {status}: {}", text.chars().take(300).collect::<String>()),
        }
    }

    fn read_failure(&self, mut resp: Resp) -> String {
        let status = resp.status().as_u16();
        let text = resp.body_mut().with_config().limit(1_000_000).read_to_string().unwrap_or_default();
        self.explain(status, &text)
    }
}

impl Adapter for OpenAiCompat {
    fn name(&self) -> String {
        format!("openai-compatible:{}", self.model)
    }

    fn chat(&self, req: &Request) -> Result<Reply, String> {
        let mut resp = self.post(&self.body(req, false), &mut |_| {})?;
        if resp.status().as_u16() >= 400 {
            return Err(self.read_failure(resp));
        }
        let text = resp.body_mut().with_config().limit(10_000_000).read_to_string().map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("model reply is not JSON: {e}"))?;
        parse_openai(&v)
    }

    fn chat_stream(&self, req: &Request, on_text: &mut dyn FnMut(&str) -> bool, on_note: &mut dyn FnMut(&str)) -> Result<Reply, String> {
        let mut resp = self.post(&self.body(req, true), on_note)?;
        let status = resp.status().as_u16();
        if status == 400 || status == 422 {
            // This server may not support streaming. Ask the plain way.
            return self.chat(req);
        }
        if status >= 400 {
            return Err(self.read_failure(resp));
        }
        read_sse(std::io::BufReader::new(resp.body_mut().as_reader()), on_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A server that answers each connection with the next canned response.
    fn serve(responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for r in responses {
                let Ok((mut s, _)) = listener.accept() else { return };
                let mut buf = vec![0u8; 65536];
                let mut got = 0;
                // Read the request head and body.
                loop {
                    let n = s.read(&mut buf[got..]).unwrap_or(0);
                    got += n;
                    let head = String::from_utf8_lossy(&buf[..got]).to_string();
                    if let Some(i) = head.find("\r\n\r\n") {
                        let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                        if got >= i + 4 + len || n == 0 { break; }
                    } else if n == 0 { break; }
                }
                let _ = s.write_all(r.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    fn reply(status: &str, headers: &str, body: &str) -> String {
        format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len())
    }

    fn req() -> Request {
        Request { purpose: Default::default(), system: "s".into(), messages: vec![json!({ "role": "user", "content": "hi" })], tools: vec![] }
    }

    fn fast(url: &str) -> OpenAiCompat {
        OpenAiCompat::new(url, "m", None).with_retry_delays(vec![Duration::ZERO; 3])
    }

    const OK: &str = r#"{"choices":[{"message":{"content":"hello"}}],"usage":{"total_tokens":3}}"#;

    #[test]
    fn empty_replies_go_out_as_empty_text_not_null() {
        let r = Request { purpose: Default::default(), system: "s".into(), messages: vec![json!({ "role": "user", "content": "hi" }), json!({ "role": "assistant", "content": null })], tools: vec![] };
        let b = fast("http://x").body(&r, false);
        assert_eq!(b["messages"][2]["content"], "", "Ollama answers 400 to a null content");
    }

    #[test]
    fn rate_limit_then_success_is_retried() {
        let url = serve(vec![
            reply("429 Too Many Requests", "Retry-After: 0\r\n", "slow down"),
            reply("503 Service Unavailable", "", "busy"),
            reply("200 OK", "Content-Type: application/json\r\n", OK),
        ]);
        let mut notes = vec![];
        let r = fast(&url).post(&fast(&url).body(&req(), false), &mut |n| notes.push(n.to_string())).unwrap();
        assert_eq!(r.status().as_u16(), 200);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].contains("rate limited") && notes[1].contains("server error 503"));
    }

    #[test]
    fn a_rate_limit_that_never_ends_gives_a_friendly_error() {
        let url = serve(vec![reply("429 Too Many Requests", "", "x"); 4]);
        let err = fast(&url).chat(&req()).unwrap_err();
        assert!(err.contains("rate limiting") && err.contains("send your message again"), "{err}");
    }

    #[test]
    fn a_bad_key_is_not_retried_and_says_so() {
        let url = serve(vec![reply("401 Unauthorized", "", "no")]);
        let err = fast(&url).chat(&req()).unwrap_err();
        assert!(err.contains("refused the API key"), "{err}");
    }

    #[test]
    fn a_retired_model_is_named() {
        let url = serve(vec![reply("410 Gone", "", "gone")]);
        let err = fast(&url).chat(&req()).unwrap_err();
        assert!(err.contains("'m' is not available"), "{err}");
    }
}
