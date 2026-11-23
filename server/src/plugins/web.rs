//! Web tools. Fetch public pages as text or links. Private and loopback hosts are blocked.

use super::{ActionInfo, Plugin, ToolError, ToolResult, info, str_arg};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::net::{IpAddr, ToSocketAddrs};


const MAX_BODY: u64 = 1_000_000;

pub struct Web;

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private() || v.is_loopback() || v.is_link_local() || v.is_unspecified()
                || v.is_broadcast() || v.octets()[0] == 0 || v.octets()[0] >= 224)
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            !(v.is_loopback() || v.is_unspecified() || (s[0] & 0xfe00) == 0xfc00 // unique-local
                || (s[0] & 0xffc0) == 0xfe80 // link-local
                || v.to_ipv4_mapped().is_some_and(|m| !is_public(IpAddr::V4(m))))
        }
    }
}

/// Only http(s), no credentials. When a proxy is set (sandboxed run) the proxy does the
/// DNS check against the address it connects to; otherwise check here.
pub fn check_public(url: &str) -> Result<(), ToolError> {
    let (scheme, rest) = url.split_once("://").ok_or("need an http(s) url")?;
    let port = if scheme == "https" { 443 } else if scheme == "http" { 80 } else { return Err("need an http(s) url".into()) };
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains('@') {
        return Err("credentials in url not allowed".into());
    }
    if super::proxy_url().is_some() {
        return Ok(());
    }
    let hostport = if auth.contains(':') && !auth.starts_with('[') { auth.to_string() } else { format!("{auth}:{port}") };
    let addrs: Vec<_> = hostport.to_socket_addrs().map_err(|_| "cannot resolve host")?.collect();
    if addrs.is_empty() || addrs.iter().any(|a| !is_public(a.ip())) {
        return Err("private address blocked".into());
    }
    Ok(())
}

/// Redirects are off, so a public page cannot bounce us to an internal host.
fn get(url: &str) -> Result<String, ToolError> {
    check_public(url)?;
    let agent = super::http_agent(15, 0);
    let mut resp = agent.get(url).header("User-Agent", "agent-core/0.1").call().map_err(|e| e.to_string())?;
    resp.body_mut().with_config().limit(MAX_BODY).read_to_string().map_err(|e| e.to_string().into())
}

pub fn to_text(html: &str) -> String {
    let (mut out, mut i, lower) = (String::new(), 0, html.to_lowercase());
    while i < html.len() {
        if !html.is_char_boundary(i) {
            i += 1;
            continue;
        }
        if html[i..].starts_with('<') {
            // Skip whole script/style blocks.
            for tag in ["script", "style"] {
                if lower[i..].starts_with(&format!("<{tag}")) {
                    i = lower[i..].find(&format!("</{tag}")).map_or(html.len(), |e| i + e);
                }
            }
            i = html[i..].find('>').map_or(html.len(), |e| i + e + 1);
            out.push(' ');
        } else {
            let next = html[i..].find('<').map_or(html.len(), |e| i + e);
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn links(base: &str, html: &str) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    for part in html.split("href=").skip(1) {
        let Some(q) = part.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        if let Some(end) = part[1..].find(q) {
            let h = &part[1..1 + end];
            if h.starts_with("http://") || h.starts_with("https://") {
                found.insert(h.to_string());
            } else if let Some(path) = h.strip_prefix('/').filter(|_| !h.starts_with("//")) {
                let origin: String = base.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
                found.insert(format!("{origin}/{path}"));
            }
        }
    }
    found.into_iter().take(200).collect()
}

impl Plugin for Web {
    fn name(&self) -> &'static str {
        "web"
    }
    fn actions(&self) -> BTreeMap<&'static str, ActionInfo> {
        BTreeMap::from([
            ("fetch", info("Fetch a page as plain text", &[("url", "http(s) url")])),
            ("links", info("List absolute links on a page", &[("url", "http(s) url")])),
        ])
    }
    fn call(&self, action: &str, args: &Value) -> Option<ToolResult> {
        let run = |f: fn(&str, &str) -> Value| -> ToolResult {
            let url = str_arg(args, "url")?;
            Ok(f(url, &get(url)?))
        };
        Some(match action {
            "fetch" => run(|u, h| json!({ "url": u, "text": to_text(h).chars().take(50_000).collect::<String>() })),
            "links" => run(|u, h| json!({ "links": links(u, h) })),
            _ => return None,
        })
    }
}
