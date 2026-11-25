//! The one HTML template that wraps every page, plus the sidebar renderer.

use crate::model::{PageRef, Section};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Runs in `<head>` before the CSS paints, so a saved light/dark choice shows
/// at once with no flash. Shared by the home page and every docs page.
/// For `file://`, where each page has its own storage, the choice also rides
/// along in `?theme=` on links and in `window.name`.
pub const THEME_BOOT: &str = r#"<script>(function(){var v,m=/[?&]theme=(light|dark)/.exec(location.search)||/ax-theme=(light|dark)/.exec(window.name||'');try{v=localStorage.getItem('assistx-theme')||localStorage.getItem('assistx-docs-theme');}catch(e){}if((location.protocol==='file:'&&m)||(v!=='light'&&v!=='dark'))v=m&&m[1];var r=document.documentElement;if(v==='light'||v==='dark')r.setAttribute('data-theme',v);r.classList.add('js');if(!(window.matchMedia&&matchMedia('(prefers-reduced-motion: reduce)').matches))r.classList.add('motion');})();</script>"#;

/// The one top bar used by the home page and every docs page, so the two feel
/// like one site. `current` is "home" or "docs". `menu_svg` adds the phone
/// menu button that opens the docs sidebar.
pub fn site_nav(root: &str, current: &str, sun_svg: &str, moon_svg: &str, menu_svg: Option<&str>) -> String {
    let home_cur = if current == "home" { r#" aria-current="page""# } else { "" };
    let docs_cur = if current == "docs" { r#" aria-current="page""# } else { "" };
    let menu = match menu_svg {
        Some(svg) => format!(r#"<button class="iconbtn menu-btn" type="button" aria-label="Open navigation"><span class="icon">{svg}</span></button>"#),
        None => String::new(),
    };
    format!(
        r#"<header class="hm-nav">
  {menu}<a class="hm-logo" href="{root}index.html"><span class="hm-mark" aria-hidden="true"></span>AssistX</a>
  <nav aria-label="Main">
    <a href="{root}index.html"{home_cur}>Home</a>
    <a href="{root}docs.html"{docs_cur}>Docs</a>
    <a class="gh" href="{repo}" target="_blank" rel="noopener noreferrer" aria-label="GitHub">{gh}<span>GitHub</span></a>
  </nav>
  <button class="iconbtn theme-btn" type="button" aria-label="Switch light or dark"><span class="icon sun">{sun_svg}</span><span class="icon moon">{moon_svg}</span></button>
</header>"#,
        repo = crate::github::REPO_URL,
        gh = crate::home::GH_MARK,
    )
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
{boot}
</head>
<body class="docs">
<a class="skip" href="#main">Skip to content</a>
{nav}
<aside class="sidebar" id="sidebar">
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
        boot = THEME_BOOT,
        nav = site_nav(root, "docs", sun_svg, moon_svg, Some(menu_svg)),
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
        r#"<div class="section"><a class="pl" href="{root}index.html">← AssistX home</a><a class="{cls}" href="{root}docs.html">Docs home</a></div>"#
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
