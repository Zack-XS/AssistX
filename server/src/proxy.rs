//! Local allowlist proxy (docs/03 Sandbox, docs/06 T2/T13). One proxy per tool run.
//!
//! - Allows only the hosts in the plugin's `needs.network` (or any public host for `*public*`).
//! - Does its own DNS lookup and refuses loopback, private and link-local answers, then connects
//!   to the address it checked, so a name cannot be re-pointed between check and connect.
//! - Plain HTTP can be held to GET/HEAD. HTTPS (CONNECT) is checked by host only.

use crate::plugins::web::is_public;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug)]
pub struct Allow {
    /// Exact host names. `*public*` = any public host.
    pub hosts: Vec<String>,
    pub get_only: bool,
}

impl Allow {
    pub fn none() -> Allow {
        Allow { hosts: vec![], get_only: true }
    }
    pub fn permits(&self, host: &str) -> bool {
        let h = host.trim_end_matches('.').to_ascii_lowercase();
        self.hosts.iter().any(|a| a == "*public*" || a.eq_ignore_ascii_case(&h))
    }
}

pub struct Proxy {
    pub port: u16,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start(allow: Allow) -> std::io::Result<Proxy> {
    let l = TcpListener::bind("127.0.0.1:0").await?;
    let port = l.local_addr()?.port();
    let allow = Arc::new(allow);
    let task = tokio::spawn(async move {
        while let Ok((s, _)) = l.accept().await {
            let a = allow.clone();
            tokio::spawn(async move {
                let _ = handle(s, &a).await;
            });
        }
    });
    Ok(Proxy { port, task })
}

/// Same proxy on a Unix socket. The Linux sandbox has no network, but a socket file in its
/// scratch folder reaches this proxy, so the allowlist and DNS checks stay the only way out.
#[cfg(unix)]
pub fn start_unix(allow: Allow, path: &std::path::Path) -> std::io::Result<Proxy> {
    let l = tokio::net::UnixListener::bind(path)?;
    let allow = Arc::new(allow);
    let task = tokio::spawn(async move {
        while let Ok((s, _)) = l.accept().await {
            let a = allow.clone();
            tokio::spawn(async move {
                let _ = handle(s, &a).await;
            });
        }
    });
    Ok(Proxy { port: 0, task })
}

/// Resolve and pick a public address. If any answer is non-public, refuse the whole name.
pub async fn resolve_public(host: &str, port: u16) -> Result<SocketAddr, &'static str> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await.map_err(|_| "cannot resolve host")?.collect();
    if addrs.is_empty() || addrs.iter().any(|a| !is_public(a.ip())) {
        return Err("private address blocked");
    }
    Ok(addrs[0])
}

async fn deny<S: AsyncWrite + Unpin>(s: &mut S, code: u16, why: &str) -> std::io::Result<()> {
    let body = format!("blocked by proxy: {why}\n");
    s.write_all(format!("HTTP/1.1 {code} Blocked\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await
}

async fn handle<S: AsyncRead + AsyncWrite + Unpin>(mut s: S, allow: &Allow) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let head_end = loop {
        let n = s.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 16_384 {
            return deny(&mut s, 431, "header too large").await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let first = head.lines().next().unwrap_or_default().to_string();
    let mut parts = first.split_whitespace();
    let (method, target, version) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or("HTTP/1.1"));

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(target, 443);
        if !allow.permits(&host) {
            return deny(&mut s, 403, "host not in needs").await;
        }
        if allow.get_only && port != 443 {
            return deny(&mut s, 403, "only https on 443").await;
        }
        let addr = match resolve_public(&host, port).await {
            Ok(a) => a,
            Err(e) => return deny(&mut s, 403, e).await,
        };
        let mut up = TcpStream::connect(addr).await?;
        s.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        if head_end < buf.len() {
            up.write_all(&buf[head_end..]).await?;
        }
        let _ = tokio::io::copy_bidirectional(&mut s, &mut up).await;
        return Ok(());
    }

    // Plain HTTP with an absolute URI.
    let Some(rest) = target.strip_prefix("http://") else { return deny(&mut s, 400, "absolute http:// URI required").await };
    if allow.get_only && !(method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")) {
        return deny(&mut s, 403, "only GET and HEAD").await;
    }
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.contains('@') {
        return deny(&mut s, 403, "credentials in URL").await;
    }
    let (host, port) = split_host_port(authority, 80);
    if !allow.permits(&host) {
        return deny(&mut s, 403, "host not in needs").await;
    }
    let addr = match resolve_public(&host, port).await {
        Ok(a) => a,
        Err(e) => return deny(&mut s, 403, e).await,
    };
    let mut up = TcpStream::connect(addr).await?;
    // Rewrite to origin form; drop proxy headers; one request per connection.
    let mut out = format!("{method} {path} {version}\r\n");
    for line in head.lines().skip(1) {
        let l = line.to_ascii_lowercase();
        if line.is_empty() || l.starts_with("proxy-") || l.starts_with("connection:") {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str("Connection: close\r\n\r\n");
    up.write_all(out.as_bytes()).await?;
    if head_end < buf.len() {
        up.write_all(&buf[head_end..]).await?;
    }
    let _ = tokio::io::copy_bidirectional(&mut s, &mut up).await;
    Ok(())
}

fn split_host_port(a: &str, default: u16) -> (String, u16) {
    if let Some(rest) = a.strip_prefix('[') {
        let end = rest.find(']').unwrap_or(rest.len());
        let port = rest[end..].strip_prefix("]:").and_then(|p| p.parse().ok()).unwrap_or(default);
        return (rest[..end].to_string(), port);
    }
    match a.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h.to_string(), p.parse().unwrap_or(default)),
        _ => (a.to_string(), default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_rules() {
        let a = Allow { hosts: vec!["api.open-meteo.com".into()], get_only: true };
        assert!(a.permits("API.open-meteo.com."));
        assert!(!a.permits("evil.com"));
        assert!(Allow { hosts: vec!["*public*".into()], get_only: true }.permits("x.org"));
        assert!(!Allow::none().permits("x.org"));
        assert_eq!(split_host_port("a.com:8080", 80), ("a.com".into(), 8080));
        assert_eq!(split_host_port("[::1]:9", 80), ("::1".into(), 9));
    }

    #[tokio::test]
    async fn refuses_loopback_and_unlisted() {
        let p = start(Allow { hosts: vec!["localhost".into(), "*public*".into()], get_only: true }).await.unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", p.port)).await.unwrap();
        s.write_all(b"CONNECT localhost:443 HTTP/1.1\r\nHost: localhost\r\n\r\n").await.unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).await.unwrap();
        assert!(r.starts_with("HTTP/1.1 403") && r.contains("private address"), "{r}");

        let p2 = start(Allow { hosts: vec!["only.example".into()], get_only: true }).await.unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", p2.port)).await.unwrap();
        s.write_all(b"POST http://only.example/x HTTP/1.1\r\nHost: only.example\r\n\r\n").await.unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).await.unwrap();
        assert!(r.contains("only GET"), "{r}");
    }

    #[tokio::test]
    async fn socket_bridge_reaches_the_proxy() {
        let dir = std::env::temp_dir().join(format!("px{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("proxy.sock");
        let _p = start_unix(Allow { hosts: vec!["localhost".into()], get_only: true }, &sock).unwrap();
        let url = crate::plugins::start_bridge(sock.to_str().unwrap()).unwrap();
        let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
        let r = tokio::task::spawn_blocking(move || {
            use std::io::{Read, Write};
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(b"CONNECT localhost:443 HTTP/1.1\r\n\r\n").unwrap();
            let mut r = String::new();
            s.read_to_string(&mut r).unwrap();
            r
        })
        .await
        .unwrap();
        assert!(r.starts_with("HTTP/1.1 403") && r.contains("private address"), "{r}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
