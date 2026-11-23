//! Markdown -> HTML rendering and extraction, using `pulldown-cmark`.
//!
//! All functions take the page **body** (front matter already stripped).
//! Links that point at a `.md` file inside `docs/` are rewritten to `.html`
//! (`README.md` -> `index.html`); links that escape `docs/` (runtime/source
//! files) are left untouched; absolute links are left untouched.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::util::{rewrite_md_link, slugify};

fn opts() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// Render markdown body to an HTML fragment, rewriting in-docs links and
/// adding `id` attributes to headings (GitHub-style slugs).
pub fn render_html(body: &str, base: &str) -> String {
    let parser = Parser::new_ext(body, opts());
    let mapped = parser.map(|ev| rewrite_event(ev, base));
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, mapped);
    add_heading_ids(&html)
}

/// Rewrite a single event's link/image destination if it is an in-docs `.md`
/// link.
fn rewrite_event<'a>(ev: Event<'a>, base: &str) -> Event<'a> {
    match ev {
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            let new_dest: CowStr<'a> = match rewrite_md_link(base, &dest_url) {
                Some(s) => CowStr::from(s),
                None => dest_url,
            };
            Event::Start(Tag::Link { link_type, dest_url: new_dest, title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            let new_dest: CowStr<'a> = match rewrite_md_link(base, &dest_url) {
                Some(s) => CowStr::from(s),
                None => dest_url,
            };
            Event::Start(Tag::Image { link_type, dest_url: new_dest, title, id })
        }
        other => other,
    }
}

/// Every heading as `(level, text)` in document order.
pub fn headings(body: &str) -> Vec<(u8, String)> {
    let parser = Parser::new_ext(body, opts());
    let mut out = Vec::new();
    let mut cur: Option<(u8, String)> = None;
    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                cur = Some((level_num(level), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(t) = cur.take() {
                    out.push(t);
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, s)) = cur.as_mut() {
                    s.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, s)) = cur.as_mut() {
                    s.push(' ');
                }
            }
            _ => {}
        }
    }
    out
}

/// First level-1 heading text (the page title), if any.
pub fn first_h1(body: &str) -> Option<String> {
    let parser = Parser::new_ext(body, opts());
    let mut cur: Option<String> = None;
    let mut in_h1 = false;
    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                if level == HeadingLevel::H1 {
                    in_h1 = true;
                    cur = Some(String::new());
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_h1 {
                    return cur.take();
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if in_h1 {
                    if let Some(s) = cur.as_mut() {
                        s.push_str(&t);
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_h1 {
                    if let Some(s) = cur.as_mut() {
                        s.push(' ');
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Every link/image destination string in the body (for the broken-link check).
pub fn link_dests(body: &str) -> Vec<String> {
    let parser = Parser::new_ext(body, opts());
    let mut out = Vec::new();
    for ev in parser {
        match ev {
            Event::Start(Tag::Link { dest_url, .. }) | Event::Start(Tag::Image { dest_url, .. }) => {
                out.push(dest_url.to_string());
            }
            _ => {}
        }
    }
    out
}

// --- heading id injection (post-process the rendered HTML) ---

fn add_heading_ids(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 256);
    let mut s = html;
    loop {
        match s.find("<h") {
            None => {
                out.push_str(s);
                break;
            }
            Some(pos) => {
                let after = &s[pos..];
                if let Some(level) = heading_open_level(after) {
                    out.push_str(&s[..pos]);
                    let open_end = pos + 4; // length of "<hN>"
                    let close = format!("</h{level}>");
                    match s[open_end..].find(&close) {
                        Some(rel) => {
                            let inner = &s[open_end..open_end + rel];
                            let id = slugify(&strip_tags(inner));
                            out.push_str(&format!("<h{level} id=\"{id}\">"));
                            out.push_str(inner);
                            out.push_str(&close);
                            s = &s[open_end + rel + close.len()..];
                        }
                        None => {
                            out.push_str(after);
                            break;
                        }
                    }
                } else {
                    // Not a heading open (e.g. "<hr"); emit "<h" and keep going.
                    out.push_str(&s[..pos + 2]);
                    s = &s[pos + 2..];
                }
            }
        }
    }
    out
}

fn heading_open_level(s: &str) -> Option<u8> {
    let b = s.as_bytes();
    if b.len() < 4 || b[0] != b'<' || b[1] != b'h' || b[3] != b'>' {
        return None;
    }
    let n = b[2];
    (b'1'..=b'6').contains(&n).then_some(n - b'0')
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
