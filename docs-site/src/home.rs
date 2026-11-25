//! The AssistX home page (`out/index.html`): top nav (Home, Docs, GitHub),
//! hero with a GitHub download button, app screens, a terminal, why AssistX,
//! a contributor leaderboard, and release, pull request and push logs.
//!
//! Real screenshots: drop PNG/JPG/WebP files into `docs-site/assets/screens/`.
//! When there are none, the page draws the screens from the design language.

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
<meta name="description" content="AssistX: an AI buddy that runs your life, on your own server.">
<title>AssistX · an AI buddy that runs your life</title>
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

    // Hero
    s.push_str(&format!(
        r##"<section class="hm-hero">
  <div class="hm-hero-text">
    <h1>An AI buddy<br><span>that runs your life.</span></h1>
    <p class="hm-lede">AssistX remembers you, acts for you, starts work on its own and learns new skills. It runs on your own server, with safety in code, not in a prompt.</p>
    <div class="hm-ctas">
      <a class="hm-btn hm-primary" href="{dl}" target="_blank" rel="noopener noreferrer">Download for Android</a>
      <a class="hm-btn hm-secondary" href="docs.html">Read the docs</a>
    </div>
    <p class="hm-kicker">{note} · Open source · Rust</p>
  </div>
  <div class="hm-hero-art">{phone}</div>
</section>
"##,
        dl = esc(&download_url),
        note = download_note,
        phone = phone_chat(),
    ));

    // Why
    s.push_str(r#"<section class="hm-sec"><h2>Why AssistX</h2><div class="hm-why">"#);
    for (title, text) in [
        ("Knows you", "A real memory you own, tidied each night."),
        ("Does things", "Calendar, alarms, lists, mail, music, files and your phone."),
        ("Thinks ahead", "Check-ins, a morning brief, reminders by time or place."),
        ("Learns skills", "Writes its own plugins, with your yes."),
        ("Safe in code", "Risky acts need a tap. Every act is logged."),
        ("Yours", "Your server, any model, encrypted at rest."),
    ] {
        s.push_str(&format!(
            r#"<div class="hm-card"><h3>{title}</h3><p>{text}</p></div>"#
        ));
    }
    s.push_str("</div></section>");

    // Screens
    s.push_str(r#"<section class="hm-sec"><h2>Screens</h2><div class="hm-screens">"#);
    if screens.is_empty() {
        s.push_str(&format!(
            r#"<figure>{}<figcaption>Chat that acts</figcaption></figure><figure>{}<figcaption>Your day at a glance</figcaption></figure><figure>{}<figcaption>Talk hands-free</figcaption></figure>"#,
            phone_chat(),
            phone_today(),
            phone_voice()
        ));
    } else {
        for f in screens {
            let name = f.rsplit_once('.').map(|(a, _)| a).unwrap_or(f).replace(['-', '_'], " ");
            s.push_str(&format!(
                r#"<figure><div class="hm-phone hm-shot"><img src="assets/screens/{f}" alt="{alt}" loading="lazy"></div><figcaption>{alt}</figcaption></figure>"#,
                f = esc(f),
                alt = esc(&name)
            ));
        }
    }
    s.push_str("</div></section>");

    // Terminal
    s.push_str(&format!(
        r##"<section class="hm-sec hm-split">
  <div><h2>Run it yourself</h2><p class="hm-lede">One Rust server and one encrypted database file. Point it at any model: a local one with Ollama, or a cloud one. The tests need no API key.</p>
  <p><a href="getting-started/run-server.html">Run the server →</a><br><a href="getting-started/run-android.html">Run the Android app →</a></p></div>
  <div class="hm-term" role="img" aria-label="Terminal: clone, test and run the AssistX server">
    <div class="hm-term-bar"><b>Terminal</b></div>
    <pre><code><span class="l"><i>$</i> git clone {REPO_URL}.git</span><span class="l"><i>$</i> cd AssistX/server</span><span class="l"><i>$</i> cargo test -j1 <em># unit + golden tests, no key needed</em></span><span class="l"><i>$</i> AGENT_MODEL_BASE_URL=http://127.0.0.1:11434/v1 \</span><span class="l">  AGENT_MODEL_NAME=llama3.1 cargo run -j1</span><span class="l"><i>$</i> cargo run -j1 -- token create client phone <em># pair the app</em></span><span class="l hm-cursor"><i>$</i> </span></code></pre>
  </div>
</section>
"##
    ));

    // Community: leaderboard + logs
    s.push_str(r#"<section class="hm-sec"><h2>Community</h2><div class="hm-community">"#);

    // Leaderboard
    s.push_str(r#"<div class="hm-panel hm-board"><h3>Contributors</h3>"#);
    if gh.contributors.is_empty() {
        s.push_str(r#"<p class="hm-empty">No contributors loaded yet.</p>"#);
    } else {
        let top = gh.contributors[0].commits.max(1);
        s.push_str(r#"<ol class="hm-leader">"#);
        for (i, c) in gh.contributors.iter().take(10).enumerate() {
            let medal = match i { 0 => " hm-gold", 1 => " hm-silver", 2 => " hm-bronze", _ => "" };
            let avatar = if c.avatar.starts_with("https://avatars.githubusercontent.com/") {
                format!(r#"<img src="{}&s=64" alt="" width="32" height="32" loading="lazy">"#, esc(&c.avatar))
            } else {
                format!(r#"<span class="hm-av">{}</span>"#, esc(&c.login.chars().next().unwrap_or('?').to_uppercase().to_string()))
            };
            s.push_str(&format!(
                r#"<li class="hm-lrow{medal}"><span class="hm-rank">{}</span>{avatar}<a href="{}" target="_blank" rel="noopener noreferrer">{}</a><span class="hm-bar"><span style="width:{}%"></span></span><span class="hm-cnt">{} {}</span></li>"#,
                i + 1,
                esc(&c.url),
                esc(&c.login),
                (c.commits * 100 / top).max(4),
                c.commits,
                if c.commits == 1 { "commit" } else { "commits" }
            ));
        }
        s.push_str("</ol>");
    }
    s.push_str(&format!(
        r#"<a class="hm-more" href="{REPO_URL}/blob/production/docs/contributing/README.md" target="_blank" rel="noopener noreferrer">Join them: how to contribute →</a></div>"#
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
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/releases" target="_blank" rel="noopener noreferrer">All releases →</a></div>"#));

    // Pull requests
    s.push_str(r#"<div class="hm-panel"><h3>Pull requests</h3>"#);
    if gh.pulls.is_empty() {
        s.push_str(r#"<p class="hm-empty">No pull requests yet. Yours could be the first.</p>"#);
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
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/pulls" target="_blank" rel="noopener noreferrer">All pull requests →</a></div>"#));

    // Recent pushes
    s.push_str(r#"<div class="hm-panel"><h3>Recent pushes</h3>"#);
    if gh.commits.is_empty() {
        s.push_str(r#"<p class="hm-empty">No pushes loaded yet.</p>"#);
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
    s.push_str(&format!(r#"<a class="hm-more" href="{REPO_URL}/commits" target="_blank" rel="noopener noreferrer">All commits →</a></div>"#));

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

fn phone(inner: &str, label: &str) -> String {
    format!(r#"<div class="hm-phone" role="img" aria-label="{label}"><div class="hm-notch"></div><div class="hm-screen">{inner}</div></div>"#)
}

fn phone_chat() -> String {
    phone(
        r#"<div class="hm-sbar"><b>Assistant</b><span class="hm-live"></span></div>
<div class="hm-msg hm-me">Move my dentist to Friday and remind me the night before.</div>
<div class="hm-msg hm-ai">Dentist is Tue 10:00. I can move it to Fri 10:00 and set a reminder Thu 20:00.</div>
<div class="hm-approve"><b>Move “Dentist” to Fri 10:00?</b><span class="hm-row"><span class="hm-b hm-ok">Allow</span><span class="hm-b">No</span></span></div>
<div class="hm-msg hm-ai">Done. Reminder set for Thursday night.</div>
<div class="hm-compose"><span>Message</span><i></i></div>"#,
        "App screen: a chat where the assistant moves an appointment after you allow it",
    )
}

fn phone_today() -> String {
    phone(
        r#"<div class="hm-sbar"><b>Today</b><span>Wed</span></div>
<div class="hm-tcard"><small>Morning brief</small><b>Sunny, 18°. Two meetings, gym at 6.</b></div>
<div class="hm-tcard"><small>Next</small><b>10:00 Design review</b><span class="hm-pbar"><span></span></span></div>
<div class="hm-tcard"><small>Shopping</small><b>Milk · Eggs · Coffee</b></div>
<div class="hm-tcard"><small>Noticed</small><b>You usually leave at 8:40. Traffic is light.</b></div>
<div class="hm-tabs"><i class="on"></i><i></i><i></i><i></i></div>"#,
        "App screen: today, with the morning brief, the next meeting and a list",
    )
}

fn phone_voice() -> String {
    phone(
        r#"<div class="hm-sbar"><b>Voice</b><span>Live</span></div>
<div class="hm-orb"><span></span></div>
<p class="hm-heard">“What's on tomorrow?”</p>
<p class="hm-said">Two things: standup at 9 and lunch with Sam at 12:30.</p>
<div class="hm-tabs"><i></i><i></i><i class="on"></i><i></i></div>"#,
        "App screen: voice mode, listening and answering",
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
        assert!(p.contains("hm-term") && p.contains("hm-phone"));
    }

    #[test]
    fn filled_data_shows_leaderboard_logs_and_apk() {
        let d = parse(r#"{"contributors":[{"login":"a","avatar":"https://avatars.githubusercontent.com/u/1?v=4","url":"https://github.com/a","commits":4},{"login":"b<x>","avatar":"","url":"https://github.com/b","commits":1}],
          "releases":[{"tag":"v2","name":"","date":"2026-11-20","url":"https://github.com/r/v2","body":"- One","apk":"https://github.com/r/app.apk"}],
          "pulls":[{"number":7,"title":"fix: y","state":"merged","user":"a","date":"2026-11-21","url":"https://github.com/p/7"}],
          "commits":[]}"#);
        let p = page(&d, &["chat.png".into()], &HashMap::new());
        assert!(p.contains("hm-gold") && p.contains("4 commits") && p.contains("1 commit<"));
        assert!(p.contains("b&lt;x&gt;"), "escaped");
        assert!(p.contains("https://github.com/r/app.apk") && p.contains("Android · v2"));
        assert!(p.contains("hm-merged") && p.contains("#7"));
        assert!(p.contains("assets/screens/chat.png"));
    }
}
