//! Small path helpers: normalise `.`/`..` segments and decide whether a
//! resolved relative link stays inside `docs/`.

/// Normalise a path with `.` and `..` segments into a slash-joined list.
/// `base` is the source file's directory (slash-separated, e.g. "docs/plugins").
/// `rel` is the link destination (e.g. "../security/safety-rules.md").
pub fn resolve(base: &str, rel: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in base.split('/') {
        if !seg.is_empty() {
            out.push(seg.to_string());
        }
    }
    // Leading slash = absolute from repo root; drop the base.
    if rel.starts_with('/') {
        out.clear();
    }
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s.to_string()),
        }
    }
    out
}

/// True if a resolved path stays inside `docs/`.
pub fn inside_docs(segs: &[String]) -> bool {
    segs.first().map(|s| s == "docs").unwrap_or(false)
}

/// Join normalised segments into a slash path.
pub fn join(segs: &[String]) -> String {
    segs.join("/")
}

/// Split a URL into `(path, anchor)`. Anchor-only ("#x") -> ("", "#x").
pub fn split_anchor(url: &str) -> (&str, &str) {
    match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    }
}

/// True if a URL is absolute (not a relative link we touch).
pub fn is_absolute(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
        || url.starts_with("data:")
        || url.starts_with("//")
}

/// Rewrite a relative `.md` link to `.html` for the built site.
/// Keeps the relative path string (input and output trees are isomorphic),
/// turns `README.md` into `index.html`, and keeps any `#anchor`.
/// Returns None if the link should not be rewritten (absolute, anchor-only,
/// non-`.md`, or a source-file link that escapes `docs/`).
pub fn rewrite_md_link(base: &str, url: &str) -> Option<String> {
    if is_absolute(url) {
        return None;
    }
    let (path, anchor) = split_anchor(url);
    if path.is_empty() {
        return None; // anchor-only
    }
    if !path.ends_with(".md") {
        return None; // e.g. runtime .yaml / source files: leave as-is
    }
    let target = resolve(base, path);
    if !inside_docs(&target) {
        return None; // points outside docs/ (a runtime/source file): leave as-is
    }
    let new_path = if target.join("/") == "docs/README.md" {
        // The docs index is docs.html at the site root.
        match path.strip_suffix("README.md") {
            Some(prefix) => format!("{prefix}docs.html"),
            None => "docs.html".to_string(),
        }
    } else if path == "README.md" {
        "index.html".to_string()
    } else if let Some(stripped) = path.strip_suffix("/README.md") {
        format!("{stripped}/index.html")
    } else if let Some(stripped) = path.strip_suffix(".md") {
        format!("{stripped}.html")
    } else {
        path.to_string()
    };
    Some(format!("{new_path}{anchor}"))
}

/// GitHub-style slug for a heading: lowercase, runs of non-[a-z0-9] -> '-'.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true; // suppress leading dashes
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}
