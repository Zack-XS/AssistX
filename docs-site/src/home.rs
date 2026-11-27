//! The AssistX home page (`out/index.html`): top nav (Home, Docs, GitHub),
//! a short intro with the download button, real app screenshots, what it does,
//! how to run it, and contributors, releases, pull requests and commits.
//!
//! Screenshots live in `docs-site/assets/screens/light/` and `dark/`, with the
//! same file names. See the README for how to retake them.

use crate::github::{Data, REPO_URL};

pub const GH_MARK: &str = r#"<svg viewBox="0 0 16 16" width="18" height="18" aria-hidden="true" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>"#;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn icon(icons: &std::collections::HashMap<String, String>, name: &str) -> String {
    icons.get(name).cloned().unwrap_or_default()
}

/// `screens`: file names found in assets/screens (already copied to out/assets/screens).
pub fn page(gh: &Data, screens: &[String], icons: &std::collections::HashMap<String, String>) -> String {
    let latest = gh.releases.first();
    let download_url = latest
        .and_then(|r| r.apk.clone())
        .unwrap_or_else(|| format!("{REPO_URL}/releases"));
    let download_note = match latest {
        Some(r) if r.apk.is_some() => format!("Android · {}", esc(&r.tag)),
        Some(r) => format!("Latest: {}", esc(&r.tag)),
        None => "Android · from GitHub".to_string(),
    };

    let mut s = String::new();
    s.push_str(&format!(
        r##"<!doctype html>
<html lang="en" data-root="">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="description" content="AssistX: a personal assistant that runs on your own server.">
<title>AssistX</title>
<link rel="stylesheet" href="assets/tokens.css">
<link rel="stylesheet" href="assets/main.css">
{boot}
</head>
<body class="hm">
<a class="skip" href="#top">Skip to content</a>
{nav}
<main id="top">
"##,
        boot = crate::template::THEME_BOOT,
        nav = crate::template::site_nav("", "home", &icon(icons, "sun"), &icon(icons, "moon"), None),
    ));

    // Hero: a plain line, two buttons, and a real screen.
    s.push_str(&format!(
        r##"<section class="hm-hero">
  <div class="hm-hero-text">
    <h1>A personal assistant that runs on your own server.</h1>
    <p class="hm-lede">AssistX keeps your calendar, alarms, lists and notes. You can ask it to change them, and it remembers what you tell it. You pick the model and keep the data.</p>
    <div class="hm-ctas">
      <a class="hm-btn hm-primary" href="{dl}" target="_blank" rel="noopener noreferrer">Download for Android</a>
      <a class="hm-btn hm-secondary" href="docs.html">Read the docs</a>
    </div>
    <p class="hm-kicker">{note} · Open source</p>
  </div>
  <div class="hm-hero-art">{art}</div>
</section>
"##,
        dl = esc(&download_url),
        note = download_note,
        art = if has(screens, "chat.png") { shot("chat.png", "The Assistant screen answering a question about tomorrow", "eager") } else { String::new() },
    ));

    // Screens: real screenshots of the app page, in a fixed order.
    let shown: Vec<&(&str, &str, &str)> = SCREENS.iter().filter(|(f, _, _)| has(screens, f)).collect();
    if !shown.is_empty() {
        s.push_str(r#"<section class="hm-sec"><h2>The app</h2><div class="hm-screens">"#);
        for (f, cap, alt) in shown {
            s.push_str(&format!(r#"<figure>{}<figcaption>{cap}</figcaption></figure>"#, shot(f, alt, "lazy")));
        }
        s.push_str(r#"</div><p class="hm-note">Screenshots of the app with sample data.</p></section>"#);
    }

    // What it does: a plain list.
    s.push_str(r#"<section class="hm-sec"><h2>What it does</h2><ul class="hm-list">"#);
    for line in [
        "Remembers what you tell it. You can read, edit or delete anything it keeps.",
        "Handles your calendar, alarms, lists, mail, music and files.",
        "Sends a daily report and reminds you by time or by place.",
        "Can write new plugins for itself, after you say yes.",
        "Asks before doing anything risky, and logs every action.",
        "Runs on your server with any model. The database is encrypted.",
    ] {
        s.push_str(&format!("<li>{line}</li>"));
    }
    s.push_str("</ul></section>");

    // Run it yourself: a plain code block.
    s.push_str(&format!(
        r##"<section class="hm-sec hm-split">
  <div><h2>Run it yourself</h2><p class="hm-lede">The server is one Rust program and one encrypted database file. It works with a local model through Ollama or with a cloud model. The tests need no API key.</p>
  <p><a href="getting-started/run-server.html">Run the server</a><br><a href="getting-started/run-android.html">Run the Android app</a></p></div>
  <pre class="hm-code"><code>git clone {REPO_URL}.git
cd AssistX/server
cargo test -j1

AGENT_MODEL_BASE_URL=http://127.0.0.1:11434/v1 \
  AGENT_MODEL_NAME=llama3.1 cargo run -j1

# make a token for the phone
cargo run -j1 -- token create client phone</code></pre>
</section>
"##
    ));

    // Community: leaderboard + logs
    s.push_str(r#"<section class="hm-sec"><h2>Development</h2><div class="hm-community">"#);

    // Leaderboard
    s.push_str(r#"<div class="hm-panel hm-board"><h3>Contributors</h3>"#);
    if gh.contributors.is_empty() {
        s.push_str(r#"<p class="hm-empty">No contributors loaded yet.</p>"#);
    } else {
        s.push_str(r#"<ol class="hm-leader">"#);
        for c in gh.contributors.iter().take(10) {
            let avatar = if c.avatar.starts_with("https://avatars.githubusercontent.com/") {
                format!(r#"<img src="{}&s=64" alt="" width="32" height="32" loading="lazy">"#, esc(&c.avatar))
            } else {
                format!(r#"<span class="hm-av">{}</span>"#, esc(&c.login.chars().next().unwrap_or('?').to_uppercase().to_string()))
            };
            s.push_str(&format!(
                r#"<li class="hm-lrow">{avatar}<a href="{}" target="_blank" rel="noopener noreferrer">{}</a><span class="hm-cnt">{} {}</span></li>"#,
                esc(&c.url),
                esc(&c.login),
                c.commits,
                if c.commits == 1 { "commit" } else { "commits" }
            ));
        }
        s.push_str("</ol>");
    }
    s.push_str(&format!(
        r#"<a class="hm-more" href="{REPO_URL}/blob/production/docs/contributing/README.md" target="_blank" rel="noopener noreferrer">How to contribute</a></div>"#
    ));

    // Releases
    s.push_str(r#"<div class="hm-panel"><h3>Releases</h3>"#);
    if gh.releases.is_empty() {
        s.push_str(r#"<p class="hm-empty">No releases yet. The first one ships from the <code>production</code> branch.</p>"#);
    } else {
        s.push_str(r#"<ul class="hm-log">"#);
        for r in gh.releases.iter().take(5) {
            let notes: String = r.notes.iter().map(|n| format!("<li>{}</li>", esc(n))).collect();
            let title = if r.name.is_empty() { &r.tag } else { &r.name };
            s.push_str(&format!(
                r#"<li><a class="hm-lt" href="{}" target="_blank" rel="noopener noreferrer"><span class="hm-tag">{}</span>{}</a><span class="hm-date">{}</span>{}</li>"#,
                esc(&r.url),
                esc(&r.tag),
                esc(title),
                esc(&r.date),
                if notes.is_empty() { String::new() } else { format!(r#"<ul class="hm-notes">{notes}</ul>"#) }
            ));
        }
        s.push_str("</ul>");
    }
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/releases" target="_blank" rel="noopener noreferrer">All releases</a></div>"#));

    // Pull requests
    s.push_str(r#"<div class="hm-panel"><h3>Pull requests</h3>"#);
    if gh.pulls.is_empty() {
        s.push_str(r#"<p class="hm-empty">No pull requests yet.</p>"#);
    } else {
        s.push_str(r#"<ul class="hm-log">"#);
        for p in gh.pulls.iter().take(8) {
            let st = match p.state.as_str() { "merged" | "open" | "closed" => p.state.as_str(), _ => "open" };
            s.push_str(&format!(
                r#"<li><a class="hm-lt" href="{}" target="_blank" rel="noopener noreferrer"><span class="hm-state hm-{st}">{st}</span>{}</a><span class="hm-date">#{} · {} · {}</span></li>"#,
                esc(&p.url),
                esc(&p.title),
                p.number,
                esc(&p.user),
                esc(&p.date)
            ));
        }
        s.push_str("</ul>");
    }
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/pulls" target="_blank" rel="noopener noreferrer">All pull requests</a></div>"#));

    // Recent pushes
    s.push_str(r#"<div class="hm-panel"><h3>Recent commits</h3>"#);
    if gh.commits.is_empty() {
        s.push_str(r#"<p class="hm-empty">No commits loaded yet.</p>"#);
    } else {
        s.push_str(r#"<ul class="hm-log">"#);
        for c in gh.commits.iter().take(6) {
            s.push_str(&format!(
                r#"<li><a class="hm-lt" href="{}" target="_blank" rel="noopener noreferrer"><code class="hm-sha">{}</code>{}</a><span class="hm-date">{} · {}</span></li>"#,
                esc(&c.url),
                esc(&c.sha),
                esc(&c.message),
                esc(&c.author),
                esc(&c.date)
            ));
        }
        s.push_str("</ul>");
    }
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/commits" target="_blank" rel="noopener noreferrer">All commits</a></div>"#));

    s.push_str("</div>");
    if !gh.fetched_at.is_empty() {
        s.push_str(&format!(r#"<p class="hm-fetched">GitHub data from {}.</p>"#, esc(&gh.fetched_at)));
    }
    s.push_str("</section>");

    // Footer
    s.push_str(&format!(
        r##"</main>
<footer class="hm-foot"><span>AssistX</span><a href="docs.html">Docs</a><a href="roadmap/index.html">Roadmap</a><a href="security/reporting.html">Report a security problem</a><a href="{REPO_URL}" target="_blank" rel="noopener noreferrer">{GH_MARK} GitHub</a></footer>
<script src="assets/app.js" defer></script>
</body>
</html>
"##
    ));
    s
}

/// The screens shown under "The app": file, caption, alt text.
const SCREENS: &[(&str, &str, &str)] = &[
    ("home.png", "Home", "Home screen with today's tasks and what is coming up"),
    ("calendar.png", "Calendar", "Plan screen with the month calendar and today's events"),
    ("list.png", "Lists", "A shopping list with two items done"),
    ("alarms.png", "Alarms", "Alarms screen with the next alarm and two weekly alarms"),
];

fn has(screens: &[String], f: &str) -> bool {
    screens.iter().any(|s| s == f)
}

/// One screenshot in a plain rounded frame. Both the light and the dark file
/// are in the page; CSS shows the one that matches the site theme.
fn shot(f: &str, alt: &str, loading: &str) -> String {
    format!(
        r#"<div class="hm-shot"><img class="for-light" src="assets/screens/light/{f}" alt="{alt}" width="390" height="844" loading="{loading}"><img class="for-dark" src="assets/screens/dark/{f}" alt="{alt}" width="390" height="844" loading="lazy"></div>"#,
        f = esc(f),
        alt = esc(alt),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{parse, Data};
    use std::collections::HashMap;

    #[test]
    fn empty_data_shows_empty_states_and_nav() {
        let p = page(&Data::default(), &[], &HashMap::new());
        assert!(p.contains(">Home</a>") && p.contains(">Docs</a>") && p.contains("<span>GitHub</span>"));
        assert!(p.contains("No releases yet") && p.contains("No pull requests yet"));
        assert!(p.contains("https://github.com/Zack-XS/AssistX/releases"), "download falls back to releases");
        assert!(p.contains("hm-code") && !p.contains("hm-shot"), "no screens, no frames");
    }

    #[test]
    fn filled_data_shows_leaderboard_logs_and_apk() {
        let d = parse(r#"{"contributors":[{"login":"a","avatar":"https://avatars.githubusercontent.com/u/1?v=4","url":"https://github.com/a","commits":4},{"login":"b<x>","avatar":"","url":"https://github.com/b","commits":1}],
          "releases":[{"tag":"v2","name":"","date":"2026-11-20","url":"https://github.com/r/v2","body":"- One","apk":"https://github.com/r/app.apk"}],
          "pulls":[{"number":7,"title":"fix: y","state":"merged","user":"a","date":"2026-11-21","url":"https://github.com/p/7"}],
          "commits":[]}"#);
        let p = page(&d, &["chat.png".into(), "home.png".into()], &HashMap::new());
        assert!(p.contains("4 commits") && p.contains("1 commit<"));
        assert!(p.contains("b&lt;x&gt;"), "escaped");
        assert!(p.contains("https://github.com/r/app.apk") && p.contains("Android · v2"));
        assert!(p.contains("hm-merged") && p.contains("#7"));
        assert!(p.contains("assets/screens/light/chat.png") && p.contains("assets/screens/dark/home.png"));
        assert!(!p.contains("calendar.png"), "missing screens are skipped");
    }
}
