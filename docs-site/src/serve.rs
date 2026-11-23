//! `serve` subcommand: a tiny std-only HTTP/1.1 server for `docs-site/out/`.
//! Local only (127.0.0.1). No extra crates.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use std::time::Duration;

pub fn run(out_dir: &Path, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    eprintln!("docs-site serving on http://127.0.0.1:{port}  (Ctrl-C to stop)");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = handle(&mut stream, out_dir);
    }
    Ok(())
}

fn handle(stream: &mut std::net::TcpStream, out_dir: &Path) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req
        .lines()
        .next()
        .and_then(|l| {
            let mut parts = l.split_whitespace();
            parts.next(); // method
            parts.next().map(|p| p.to_string())
        })
        .unwrap_or_else(|| "/".to_string());

    // strip query
    let path = path.split('?').next().unwrap_or("/").to_string();
    let decoded = url_decode(&path);

    let mut file_path = PathBuf::from(out_dir);
    // safe join: drop leading slash, reject '..'
    let rel = decoded.trim_start_matches('/');
    if rel.contains("..") {
        return send(stream, 400, "text/plain", b"bad request");
    }
    file_path.push(rel);

    // directory index
    if file_path.is_dir() {
        file_path.push("index.html");
    }
    if !file_path.exists() {
        return send(stream, 404, "text/plain", b"not found");
    }

    let body = std::fs::read(&file_path)?;
    let ct = content_type(&file_path);
    send(stream, 200, ct, &body)
}

fn send(stream: &mut std::net::TcpStream, code: u16, ct: &str, body: &[u8]) -> std::io::Result<()> {
    let status = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {code} {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn url_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent() {
        assert_eq!(url_decode("/a%20b%2Fc"), "/a b/c");
    }
}
