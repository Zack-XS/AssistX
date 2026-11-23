//! The one HTML template that wraps every page, plus the sidebar renderer.

use crate::model::{PageRef, Section};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A full HTML page. `root` is the path from this page to the site root
/// (e.g. "" for index.html, "../" for a page one folder deep).
pub fn page(
    title: &str,
    description: &str,
    root: &str,
    sidebar: &str,
    body: &str,
    menu_svg: &str,
    sun_svg: &str,
    moon_svg: &str,
) -> String {
    let t = esc(title);
    let d = esc(description);
    format!(
        r##"<!doctype html>
<html lang="en" data-root="{root}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="description" content="{d}">
<title>{t} · AssistX docs</title>
<link rel="stylesheet" href="{root}assets/tokens.css">
<link rel="stylesheet" href="{root}assets/main.css">
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="topbar">
  <button class="menu-btn" type="button" aria-label="Open navigation"><span class="icon">{menu_svg}</span> Menu</button>
  <a class="brand" href="{root}index.html">AssistX docs</a>
</header>
<aside class="sidebar" id="sidebar">
  <div class="sidebar-head">
    <a class="brand" href="{root}index.html">AssistX docs</a>
    <button class="iconbtn theme-btn" type="button" aria-label="Toggle theme">
      <span class="icon sun">{sun_svg}</span><span class="icon moon">{moon_svg}</span>
    </button>
  </div>
  <div class="search">
    <label class="skip" for="q">Search</label>
    <input id="q" type="search" placeholder="Search docs…" autocomplete="off" aria-label="Search docs">
    <ul id="results" hidden></ul>
  </div>
  <nav class="nav">{sidebar}</nav>
</aside>
<div class="scrim" id="scrim"></div>
<main class="main" id="main">
  <article class="prose">
{body}
  </article>
</main>
<script src="{root}assets/app.js" defer></script>
</body>
</html>
"##,
        t = t,
        d = d,
        root = root,
        menu_svg = menu_svg,
        sun_svg = sun_svg,
        moon_svg = moon_svg,
        sidebar = sidebar,
        body = body,
    )
}

/// Render the sidebar. `active` is the output-rel of the current page (so its
/// link is marked active). `home_active` marks the home link.
pub fn render_sidebar(
    sections: &[Section],
    about: &[PageRef],
    active: &str,
    root: &str,
    home_active: bool,
) -> String {
    let mut out = String::new();
    // Home link
    let cls = if home_active { "pl active" } else { "pl" };
    out.push_str(&format!(
        r#"<div class="section"><a class="{cls}" href="{root}index.html">Home</a></div>"#
    ));
    for s in sections {
        let sec_cls = if active == s.index_rel { "stitle active" } else { "stitle" };
        out.push_str("<div class=\"section\">");
        out.push_str(&format!(
            r#"<span class="{sec_cls}"><a href="{root}{idx}">{title}</a></span>"#,
            idx = s.index_rel,
            title = esc(&s.title)
        ));
        for p in &s.pages {
            let pcls = if active == p.output_rel { "pl active" } else { "pl" };
            out.push_str(&format!(
                r#"<a class="{pcls}" href="{root}{href}">{name}</a>"#,
                href = p.output_rel,
                name = esc(&p.title)
            ));
        }
        out.push_str("</div>");
    }
    if !about.is_empty() {
        out.push_str("<div class=\"section\"><span class=\"stitle\">About these docs</span>");
        for p in about {
            let pcls = if active == p.output_rel { "pl active" } else { "pl" };
            out.push_str(&format!(
                r#"<a class="{pcls}" href="{root}{href}">{name}</a>"#,
                href = p.output_rel,
                name = esc(&p.title)
            ));
        }
        out.push_str("</div>");
    }
    out
}
