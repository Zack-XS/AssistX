//! Alarm and reminder sounds (docs/01 Alarms).
//!
//! A few names are built in: the phone app has those sounds itself. The owner can add a custom
//! ringtone: a short audio file kept at `data/sounds/<name>.<ext>` and served back to the app.
//! These are ringtones only. They are never transcribed, never shown to the model, and have
//! nothing to do with phone capture, which still refuses all audio (docs/08).

use crate::App;
use serde_json::{Value, json};
use std::path::PathBuf;

pub const BUILTIN: &[&str] = &["default", "chime", "bell", "beep", "gentle", "radar"];
pub const MAX_BYTES: usize = 3 * 1024 * 1024;

fn dir(app: &App) -> PathBuf {
    app.cfg.data_dir.join("sounds")
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// What kind of audio file this is, from its first bytes. None = not a sound we accept.
pub fn sniff(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.len() < 12 {
        return None;
    }
    if b.starts_with(b"ID3") || (b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        Some(("mp3", "audio/mpeg"))
    } else if b.starts_with(b"OggS") {
        Some(("ogg", "audio/ogg"))
    } else if b.starts_with(b"RIFF") && &b[8..12] == b"WAVE" {
        Some(("wav", "audio/wav"))
    } else if &b[4..8] == b"ftyp" {
        Some(("m4a", "audio/mp4"))
    } else {
        None
    }
}

/// The uploaded file for a name, if any.
pub fn file(app: &App, name: &str) -> Option<(PathBuf, &'static str)> {
    if !valid_name(name) {
        return None;
    }
    ["mp3", "ogg", "wav", "m4a"].iter().find_map(|ext| {
        let p = dir(app).join(format!("{name}.{ext}"));
        p.exists().then(|| (p, match *ext { "mp3" => "audio/mpeg", "ogg" => "audio/ogg", "wav" => "audio/wav", _ => "audio/mp4" }))
    })
}

/// Address the app fetches a custom sound from (with its own token). Built-ins have none.
pub fn url(app: &App, name: &str) -> Option<String> {
    file(app, name).map(|_| format!("/v1/sounds/{name}"))
}

pub fn check(app: &App, name: &str) -> Result<(), String> {
    if BUILTIN.contains(&name) || file(app, name).is_some() {
        Ok(())
    } else {
        let names: Vec<String> = list(app).iter().filter_map(|s| s["name"].as_str().map(String::from)).collect();
        Err(format!("no sound called '{name}'. Sounds: {}", names.join(", ")))
    }
}

pub fn list(app: &App) -> Vec<Value> {
    let mut out: Vec<Value> = BUILTIN.iter().map(|n| json!({ "name": n, "builtin": true, "url": null })).collect();
    let mut custom: Vec<String> = std::fs::read_dir(dir(app)).into_iter().flatten().flatten()
        .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string()))
        .filter(|n| valid_name(n) && !BUILTIN.contains(&n.as_str())).collect();
    custom.sort();
    custom.dedup();
    for n in custom {
        let size = file(app, &n).and_then(|(p, _)| std::fs::metadata(p).ok()).map(|m| m.len());
        out.push(json!({ "name": n, "builtin": false, "url": format!("/v1/sounds/{n}"), "bytes": size }));
    }
    out
}

pub fn save(app: &App, name: &str, bytes: &[u8]) -> Result<Value, String> {
    if !valid_name(name) || BUILTIN.contains(&name) {
        return Err("name: 1 to 40 of a-z, 0-9, - and _, and not a built-in name".into());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!("a sound may be at most {} MB", MAX_BYTES / 1024 / 1024));
    }
    let (ext, _) = sniff(bytes).ok_or("not an mp3, ogg, wav or m4a file")?;
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    delete(app, name);
    std::fs::write(dir(app).join(format!("{name}.{ext}")), bytes).map_err(|e| e.to_string())?;
    Ok(json!({ "name": name, "url": format!("/v1/sounds/{name}"), "bytes": bytes.len(), "type": ext }))
}

pub fn delete(app: &App, name: &str) -> bool {
    let mut gone = false;
    while let Some((p, _)) = file(app, name) {
        gone = std::fs::remove_file(p).is_ok();
        if !gone {
            break;
        }
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_audio_files_and_safe_names_pass() {
        assert_eq!(sniff(b"ID3\x04\0\0\0\0\0\0\0\0\0").map(|s| s.0), Some("mp3"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt ").map(|s| s.0), Some("wav"));
        assert_eq!(sniff(b"OggS\0\0\0\0\0\0\0\0").map(|s| s.0), Some("ogg"));
        assert!(sniff(b"<html><script>").is_none() && sniff(b"#!/bin/sh\nrm -rf").is_none());
        assert!(valid_name("my-wake_up2") && !valid_name("../x") && !valid_name("Big") && !valid_name(""));
    }
}
