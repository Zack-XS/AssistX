//! Roadmap visual: a self-contained HTML timeline the builder appends under
//! each `docs/roadmap/*.md` page. The markdown stays plain; this reads the
//! page's own content (build-step table, gap headings, decision table, or the
//! index links) and turns it into a graphic using the design tokens.
//!
//! No `regex` crate: scanners are hand-written, matching the rest of the crate.

// --- inline icons (Lucide style, 24x24, stroke = currentColor) ---

const CHECK_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>"#;

const PENDING_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m13 6 6 6-6 6"/></svg>"#;

const QUESTION_SVG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M9.1 9a3 3 0 1 1 5.8 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>"#;

/// Append-only HTML for a roadmap page, based on its file stem. `body` is the
/// page's raw markdown (front matter already stripped). None for pages that get
/// no visual.
pub fn roadmap_visual(stem: &str, body: &str) -> Option<String> {
    match stem {
        "done" => done_visual(body),
        "next" => next_visual(body),
        "open-questions" => open_visual(body),
        "README" => readme_visual(body),
        _ => None,
    }
}

// --- parsers (hand-written, no deps) ---

#[derive(Default)]
struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Every markdown table in `body`, in order. A table is a header row, a
/// `|---|` separator, then data rows.
fn tables(body: &str) -> Vec<Table> {
    let mut out = Vec::new();
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut in_table = false;
    let mut had_sep = false;

    let flush = |header: &mut Option<Vec<String>>, rows: &mut Vec<Vec<String>>, out: &mut Vec<Table>| {
        if let Some(h) = header.take() {
            out.push(Table {
                header: h,
                rows: std::mem::take(rows),
            });
        }
    };

    for line in body.lines() {
        let t = line.trim();
        if is_table_row(t) {
            let cells = split_row(t);
            if is_separator(&cells) {
                had_sep = true;
                in_table = true;
                continue;
            }
            if !had_sep {
                // header row (before any separator seen)
                header = Some(cells);
            } else {
                in_table = true;
                rows.push(cells);
            }
        } else if in_table || had_sep {
            flush(&mut header, &mut rows, &mut out);
            in_table = false;
            had_sep = false;
        }
    }
    flush(&mut header, &mut rows, &mut out);
    out
}

fn is_table_row(s: &str) -> bool {
    s.starts_with('|') && s.ends_with('|') && s.matches('|').count() >= 2
}

fn split_row(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(clean_cell)
        .collect()
}

fn is_separator(cells: &[String]) -> bool {
    cells.iter().all(|c| {
        c.contains('-') && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
    })
}

fn clean_cell(s: &str) -> String {
    s.trim()
        .replace("**", "")
        .replace('`', "")
        .trim()
        .to_string()
}

/// Every `## ` heading text, in order. Skips H1 and deeper headings.
fn h2_list(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| {
            let t = l.trim();
            // skip deeper headings like "###"
            if t.starts_with("###") {
                return None;
            }
            t.strip_prefix("## ").map(|s| s.trim().to_string())
        })
        .collect()
}

/// List items of the form `- [Label](url): desc` (the roadmap index).
fn list_links(body: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("- ") else { continue };
        if !rest.starts_with('[') {
            continue;
        }
        let inner = &rest[1..];
        let Some(ce) = inner.find(']') else { continue };
        let label = inner[..ce].trim().to_string();
        let after = &inner[ce + 1..];
        let Some(oe) = after.find('(') else { continue };
        let after = &after[oe + 1..];
        let Some(cpe) = after.find(')') else { continue };
        let url = after[..cpe].trim().to_string();
        let mut desc = after[cpe + 1..].trim().to_string();
        if let Some(d) = desc.strip_prefix(':') {
            desc = d.trim().to_string();
        }
        out.push(Link { label, url, desc });
    }
    out
}

struct Link {
    label: String,
    url: String,
    desc: String,
}

/// First short segment of a "What" cell: text before the first `(` or `,`.
fn first_segment(s: &str) -> String {
    let s = s.replace("**", "").replace('`', "");
    let cut = s.find(|c| c == '(' || c == ',').unwrap_or(s.len());
    s[..cut].trim().to_string()
}

/// Count `- ` bullets under the `## Still open` heading.
fn count_still_open(body: &str) -> usize {
    let mut in_sec = false;
    let mut n = 0;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("## ") {
            in_sec = t.starts_with("## Still open");
            continue;
        }
        if in_sec && t.starts_with("- ") {
            n += 1;
        }
    }
    n
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn open_section(title: &str) -> String {
    format!(r#"<section class="showcase roadmap"><h2 class="show-title">{title}</h2>"#)
}

// --- per-page visuals ---

fn done_visual(body: &str) -> Option<String> {
    let table = tables(body)
        .into_iter()
        .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("state")))?;
    let step_col = header_index(&table, "step");
    let what_col = header_index(&table, "what");
    let state_col = header_index(&table, "state");

    let mut s = open_section("Live: build timeline");
    s.push_str(r#"<div class="rm-timeline"><div class="rm-line"></div>"#);
    for row in &table.rows {
        let step = step_col.and_then(|i| row.get(i)).map(|x| x.as_str()).unwrap_or("");
        let what = what_col.and_then(|i| row.get(i)).map(|x| x.as_str()).unwrap_or("");
        let state = state_col.and_then(|i| row.get(i)).map(|x| x.as_str()).unwrap_or("");
        let done = state.to_lowercase().contains("done");
        let label = first_segment(what);
        let cls = if done { "rm-done" } else { "rm-next" };
        let icon = if done { CHECK_SVG } else { PENDING_SVG };
        s.push_str(&format!(
            r#"<div class="rm-node {cls}"><div class="rm-dot">{icon}</div><div class="rm-step">{}</div><div class="rm-label">{}</div></div>"#,
            esc(step),
            esc(&label)
        ));
    }
    s.push_str("</div>");

    // "Also built since" strip.
    if let Some(line) = body.lines().find(|l| l.contains("Also built since")) {
        if let Some(rest) = line.split(':').nth(1) {
            let rest = rest.split(". ").next().unwrap_or(rest);
            let items: Vec<&str> = rest
                .split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty() && !x.starts_with('['))
                .collect();
            if !items.is_empty() {
                s.push_str(r#"<p class="demo-label">Also built since</p><div class="rm-pills">"#);
                for it in items {
                    s.push_str(&format!(r#"<span class="rm-pill">{}</span>"#, esc(it)));
                }
                s.push_str("</div>");
            }
        }
    }

    s.push_str(r#"<p class="demo-label">All 8 build steps are done. Safety came in at step 4, with the first risky tool.</p>"#);
    s.push_str("</section>");
    Some(s)
}

fn next_visual(body: &str) -> Option<String> {
    let heads = h2_list(body);
    if heads.is_empty() {
        return None;
    }
    let mut s = open_section("Live: what's next");
    s.push_str(r#"<div class="rm-next">"#);
    for h in &heads {
        let (title, sub) = split_heading(h);
        s.push_str(&format!(
            r#"<div class="rm-milestone"><div class="rm-dot rm-pending">{PENDING_SVG}</div><div class="rm-mtitle">{}</div><div class="rm-msub">{}</div></div>"#,
            esc(&title),
            esc(&sub)
        ));
    }
    s.push_str("</div>");
    s.push_str(r#"<p class="demo-label">Upcoming work. Dashed and lighter to show it is not built yet.</p>"#);
    s.push_str("</section>");
    Some(s)
}

fn open_visual(body: &str) -> Option<String> {
    let table = tables(body)
        .into_iter()
        .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("status")))?;
    let q_col = header_index(&table, "question");
    let st_col = header_index(&table, "status");
    let still_open = count_still_open(body);

    let mut s = open_section("Live: decisions");
    s.push_str(r#"<div class="rm-legend">"#);
    s.push_str(&legend_item("rm-decided", CHECK_SVG, "Decided"));
    s.push_str(&legend_item("rm-proposed", PENDING_SVG, "Proposed"));
    s.push_str(&legend_item("rm-open", QUESTION_SVG, "Open"));
    s.push_str("</div>");

    s.push_str(r#"<ul class="rm-decisions">"#);
    for row in &table.rows {
        let num = row.first().map(|x| x.as_str()).unwrap_or("");
        let q = q_col.and_then(|i| row.get(i)).map(|x| x.as_str()).unwrap_or("");
        let st = st_col.and_then(|i| row.get(i)).map(|x| x.as_str()).unwrap_or("");
        let (cls, icon, label) = match st.trim().to_lowercase().as_str() {
            "decided" => ("rm-decided", CHECK_SVG, "Decided"),
            "proposed" => ("rm-proposed", PENDING_SVG, "Proposed"),
            _ => ("rm-open", QUESTION_SVG, "Open"),
        };
        s.push_str(&format!(
            r#"<li class="rm-dec {cls}"><span class="rm-ico">{icon}</span><span class="rm-q">{} {}</span><span class="rm-tag">{label}</span></li>"#,
            esc(num),
            esc(q)
        ));
    }
    s.push_str("</ul>");

    if still_open > 0 {
        s.push_str(&format!(
            r#"<p class="demo-label">Plus {still_open} more still-open question(s) listed below the table.</p>"#
        ));
    }
    s.push_str("</section>");
    Some(s)
}

fn readme_visual(body: &str) -> Option<String> {
    let links = list_links(body);
    if links.is_empty() {
        return None;
    }
    let mut s = open_section("Live: roadmap at a glance");
    s.push_str(r#"<div class="rm-overview">"#);
    for l in &links {
        let (cls, icon) = match l.url.as_str() {
            u if u.contains("done") => ("rm-done", CHECK_SVG),
            u if u.contains("next") => ("rm-next", PENDING_SVG),
            u if u.contains("open") => ("rm-open", QUESTION_SVG),
            _ => ("rm-next", PENDING_SVG),
        };
        let href = l.url.replace(".md", ".html");
        s.push_str(&format!(
            r#"<a class="rm-phase {cls}" href="{href}"><span class="rm-pico">{icon}</span><span class="rm-ptitle">{}</span><span class="rm-psub">{}</span></a>"#,
            esc(&l.label),
            esc(&l.desc)
        ));
    }
    s.push_str("</div>");
    s.push_str(r#"<p class="demo-label">Done is built. Next is known gaps. Open is decisions still needed.</p>"#);
    s.push_str("</section>");
    Some(s)
}

// --- helpers ---

fn header_index(table: &Table, name: &str) -> Option<usize> {
    table
        .header
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
}

fn split_heading(h: &str) -> (String, String) {
    match h.find(':') {
        Some(i) => (h[..i].trim().to_string(), h[i + 1..].trim().to_string()),
        None => (h.trim().to_string(), String::new()),
    }
}

fn legend_item(cls: &str, icon: &str, label: &str) -> String {
    format!(r#"<span class="rm-li {cls}">{icon}<span>{label}</span></span>"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_done_step_table() {
        let body = "## Build order status\n\n| Step | What | State |\n|---|---|---|\n| 1 | Turn engine, adapter | Done |\n| 2 | Memory DB | Done |\n";
        let t = tables(body)
            .into_iter()
            .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("state")))
            .expect("table");
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][1], "Turn engine, adapter");
        let v = roadmap_visual("done", body).expect("visual");
        assert!(v.contains("rm-timeline"));
        assert!(v.contains("rm-done"));
        assert!(v.contains("Turn engine"));
    }

    #[test]
    fn parses_next_headings() {
        let body = "# Next\n\n## Phone capture: not built yet\n\n- a\n\n## Computers: not yet\n";
        let v = roadmap_visual("next", body).expect("visual");
        assert!(v.contains("rm-milestone"));
        assert!(v.contains("Phone capture"));
        assert!(v.contains("not built yet"));
        assert!(v.contains("rm-pending"));
    }

    #[test]
    fn parses_open_decisions() {
        let body = "## Decisions\n\n| # | Question | Status | Answer | Reason |\n|---|---|---|---|---|\n| 1 | Language | **Decided** | Rust | Speed |\n| 5 | Project name | **Open** | — | Owner |\n\n## Still open\n\n- Plugin signing\n- Backup format\n";
        let v = roadmap_visual("open-questions", body).expect("visual");
        assert!(v.contains("rm-decided"));
        assert!(v.contains("rm-open"));
        assert!(v.contains("Language"));
        // still-open count surfaces
        assert!(v.contains("2 more still-open"));
    }

    #[test]
    fn parses_readme_links() {
        let body = "# Roadmap\n\n- [Done](done.md): what is built.\n- [Next](next.md): gaps.\n- [Open questions](open-questions.md): decisions.\n";
        let v = roadmap_visual("README", body).expect("visual");
        assert!(v.contains("rm-phase"));
        assert!(v.contains("done.html"));
        assert!(v.contains("rm-done"));
        assert!(v.contains("rm-open"));
    }

    #[test]
    fn no_visual_for_unknown_stem() {
        assert!(roadmap_visual("nope", "body").is_none());
    }

    #[test]
    fn first_segment_cuts_at_paren_or_comma() {
        assert_eq!(first_segment("Turn engine, model adapter"), "Turn engine");
        assert_eq!(first_segment("Index + load / unload (plugins)"), "Index + load / unload");
        assert_eq!(first_segment("Scheduler"), "Scheduler");
    }

    #[test]
    fn separator_row_detected() {
        assert!(is_separator(&["---".to_string(), ":--:".to_string()]));
        assert!(!is_separator(&["Step".to_string(), "What".to_string()]));
    }
}
