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

    let mut s = open_section("Build timeline");
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
    let mut s = open_section("What's next");
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

    let mut s = open_section("Decisions");
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
    let mut s = open_section("Roadmap at a glance");
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


// --- the roadmap board (the Roadmap home page) ---

/// One `## ` section of next.md: its title, short note and bullet count.
struct Milestone {
    title: String,
    items: usize,
    first: String,
}

fn milestones(body: &str) -> Vec<Milestone> {
    let mut out: Vec<Milestone> = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("###") {
            continue;
        }
        if let Some(h) = t.strip_prefix("## ") {
            let (title, _) = split_heading(h);
            // Short road labels: drop "(...)" and "build order".
            let title = match title.find(" (") { Some(i) => title[..i].to_string(), None => title };
            let title = title.replace(" build order", "").trim().to_string();
            out.push(Milestone { title, items: 0, first: String::new() });
            continue;
        }
        let Some(m) = out.last_mut() else { continue };
        let item = t.strip_prefix("- ").or_else(|| {
            let d = t.find(". ")?;
            t[..d].chars().all(|c| c.is_ascii_digit()).then(|| &t[d + 2..])
        });
        if let Some(it) = item {
            m.items += 1;
            if m.first.is_empty() {
                m.first = plain_short(it, 90);
            }
        }
    }
    out
}

/// Markdown to short plain text: drop links, bold, code; cut at `max` chars.
fn plain_short(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '`' | '[' => {}
            ']' => {
                if chars.peek() == Some(&'(') {
                    for c2 in chars.by_ref() {
                        if c2 == ')' {
                            break;
                        }
                    }
                }
            }
            _ => out.push(c),
        }
    }
    let out = out.trim().to_string();
    if out.chars().count() <= max {
        return out;
    }
    let cut: String = out.chars().take(max).collect();
    let cut = cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut);
    format!("{}…", cut.trim_end_matches(|c: char| c == ',' || c == '.' || c == ':'))
}

struct Step {
    num: String,
    label: String,
    done: bool,
}

fn steps(done_body: &str) -> Vec<Step> {
    let Some(table) = tables(done_body)
        .into_iter()
        .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("state")))
    else {
        return Vec::new();
    };
    let sc = header_index(&table, "step");
    let wc = header_index(&table, "what");
    let st = header_index(&table, "state");
    let cell = |row: &Vec<String>, c: Option<usize>| c.and_then(|i| row.get(i)).cloned().unwrap_or_default();
    table
        .rows
        .iter()
        .map(|r| Step {
            num: cell(r, sc),
            label: first_segment(&cell(r, wc)),
            done: cell(r, st).to_lowercase().contains("done"),
        })
        .collect()
}

fn also_built(done_body: &str) -> Vec<String> {
    let Some(line) = done_body.lines().find(|l| l.contains("Also built since")) else { return Vec::new() };
    let Some(rest) = line.split(':').nth(1) else { return Vec::new() };
    let rest = rest.split(". ").next().unwrap_or(rest);
    rest.split(',')
        .map(|x| x.trim().trim_end_matches('.').to_string())
        .filter(|x| !x.is_empty() && !x.starts_with('['))
        .map(|x| {
            let mut c = x.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

struct Decision {
    num: String,
    question: String,
    status: &'static str,
}

fn decisions(open_body: &str) -> (Vec<Decision>, usize) {
    let mut out = Vec::new();
    if let Some(table) = tables(open_body)
        .into_iter()
        .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("status")))
    {
        let qc = header_index(&table, "question");
        let sc = header_index(&table, "status");
        for r in &table.rows {
            let st = sc.and_then(|i| r.get(i)).map(|x| x.trim().to_lowercase()).unwrap_or_default();
            out.push(Decision {
                num: r.first().cloned().unwrap_or_default(),
                question: qc.and_then(|i| r.get(i)).cloned().unwrap_or_default(),
                status: match st.as_str() {
                    "decided" => "decided",
                    "proposed" => "proposed",
                    _ => "open",
                },
            });
        }
    }
    (out, count_still_open(open_body))
}

/// Planned milestones from next.md's `| # | Milestone | What it means |` table.
/// Falls back to the `## ` sections when there is no such table.
fn planned(next_body: &str) -> Vec<(String, String)> {
    if let Some(t) = tables(next_body)
        .into_iter()
        .find(|t| t.header.iter().any(|h| h.eq_ignore_ascii_case("milestone")))
    {
        let mc = header_index(&t, "milestone");
        let wc = t.header.iter().position(|h| h.to_lowercase().starts_with("what"));
        return t
            .rows
            .iter()
            .map(|r| {
                let m = mc.and_then(|i| r.get(i)).cloned().unwrap_or_default();
                let w = wc.and_then(|i| r.get(i)).cloned().unwrap_or_default();
                (plain_short(&m, 60), plain_short(&w, 90))
            })
            .collect();
    }
    milestones(next_body)
        .into_iter()
        .map(|m| (m.title, m.first))
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Done,
    Now,
    Next,
    Later,
}

struct Stop {
    kind: Kind,
    tag: String,
    title: String,
    desc: String,
}

/// Height of one stop on the road, in px. CSS uses the same number.
const ROW: usize = 104;

/// A winding path through `pts` (x in 0-100, y in px): one S-curve per hop.
fn wind_path(pts: &[(f32, f32)]) -> String {
    let mut d = String::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        if i == 0 {
            d.push_str(&format!("M{x:.1} {y:.1}"));
            continue;
        }
        let (px, py) = pts[i - 1];
        let k = (y - py) * 0.62;
        d.push_str(&format!(" C{px:.1} {:.1} {x:.1} {:.1} {x:.1} {y:.1}", py + k, y - k));
    }
    d
}

/// The road as an SVG: a wide soft surface, the travelled part solid, the rest dashed.
fn road_svg(xs: &[f32], now: usize, h: usize, class: &str) -> String {
    let pts: Vec<(f32, f32)> = xs
        .iter()
        .enumerate()
        .map(|(i, &x)| (x, (i * ROW + ROW / 2) as f32))
        .collect();
    let all = wind_path(&pts);
    let done = wind_path(&pts[..=now.min(pts.len() - 1)]);
    let ahead = wind_path(&pts[now.min(pts.len() - 1)..]);
    format!(
        r#"<svg class="rm-svg {class}" viewBox="0 0 100 {h}" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path class="rm-surface" d="{all}" vector-effect="non-scaling-stroke"/><path class="rm-ahead" d="{ahead}" vector-effect="non-scaling-stroke"/><path class="rm-travelled" d="{done}" pathLength="1" vector-effect="non-scaling-stroke"/></svg>"#
    )
}

/// The whole Roadmap page: done, next and open questions in one place.
/// A hero with counts, a winding road (built -> you are here -> next -> later),
/// the choices still to make, then the full notes folded away.
/// `notes` is the rendered HTML of done.md, next.md and open-questions.md.
pub fn board(done_body: &str, next_body: &str, open_body: &str, notes: [&str; 3]) -> String {
    let steps = steps(done_body);
    let extra = also_built(done_body);
    let plan = planned(next_body);
    let (decs, still_open) = decisions(open_body);

    let built = steps.iter().filter(|s| s.done).count();
    let total = steps.len().max(1);
    let undecided = decs.iter().filter(|d| d.status != "decided").count() + still_open;
    let pct = built * 100 / total;

    // The stops, in road order.
    let mut stops: Vec<Stop> = Vec::new();
    for st in &steps {
        stops.push(Stop {
            kind: if st.done { Kind::Done } else { Kind::Next },
            tag: format!("Built · step {}", st.num),
            title: st.label.clone(),
            desc: String::new(),
        });
    }
    if !extra.is_empty() {
        stops.push(Stop { kind: Kind::Done, tag: "Built since".into(), title: extra.join(" · "), desc: String::new() });
    }
    let now = stops.len();
    stops.push(Stop {
        kind: Kind::Now,
        tag: "You are here".into(),
        title: "Docs, design language, docs site".into(),
        desc: "The ground work for everything ahead.".into(),
    });
    for (i, (t, d)) in plan.iter().enumerate() {
        stops.push(Stop { kind: Kind::Next, tag: format!("Next · {}", i + 1), title: t.clone(), desc: d.clone() });
    }
    stops.push(Stop {
        kind: Kind::Later,
        tag: "Later".into(),
        title: format!("{undecided} choices to make"),
        desc: "Open questions below decide the road after this.".into(),
    });

    let n = stops.len();
    let h = n * ROW;
    // Desktop swings wide with cards on the outside; phone swings small with cards on the right.
    let xs_wide: Vec<f32> = (0..n).map(|i| if i % 2 == 0 { 32.0 } else { 68.0 }).collect();
    let xs_narrow: Vec<f32> = (0..n).map(|i| if i % 2 == 0 { 9.0 } else { 23.0 }).collect();

    let mut s = String::new();
    s.push_str(r#"<div class="rm-board">"#);

    // Hero
    s.push_str(&format!(
        r##"<header class="rm-hero"><p class="rm-kicker">AssistX roadmap</p><h1 class="rm-h1">From a smart chat box to a buddy that runs your day.</h1>
<div class="rm-stats">
<a class="rm-stat rm-s-done" href="#rm-road"><span class="rm-num">{built}/{total}</span><span class="rm-cap">core steps built</span></a>
<a class="rm-stat rm-s-next" href="#rm-now"><span class="rm-num">{}</span><span class="rm-cap">milestones ahead</span></a>
<a class="rm-stat rm-s-open" href="#rm-decide"><span class="rm-num">{undecided}</span><span class="rm-cap">choices to make</span></a>
</div>
<div class="rm-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{pct}" aria-label="Core build progress"><span style="width:{pct}%"></span></div>
</header>"##,
        plan.len()
    ));

    // The winding road
    s.push_str(&format!(
        r#"<section class="rm-journey" id="rm-road" aria-label="Roadmap timeline" style="height:{h}px">"#
    ));
    s.push_str(&road_svg(&xs_wide, now, h, "rm-wide"));
    s.push_str(&road_svg(&xs_narrow, now, h, "rm-narrow"));
    s.push_str(r#"<ol class="rm-stops">"#);
    for (i, st) in stops.iter().enumerate() {
        let (cls, icon) = match st.kind {
            Kind::Done => ("rm-k-done", CHECK_SVG.to_string()),
            Kind::Now => ("rm-k-now", r#"<span class="rm-pulse"></span>"#.to_string()),
            Kind::Next => ("rm-k-next", format!(r#"<span class="rm-n">{}</span>"#, i - now)),
            Kind::Later => ("rm-k-later", QUESTION_SVG.to_string()),
        };
        let side = if i % 2 == 0 { "rm-left" } else { "rm-right" };
        let id = if st.kind == Kind::Now { r#" id="rm-now""# } else { "" };
        let desc = if st.desc.is_empty() { String::new() } else { format!(r#"<span class="rm-cd">{}</span>"#, esc(&st.desc)) };
        s.push_str(&format!(
            r#"<li class="rm-at {cls} {side}"{id} style="top:{}px;--xw:{}%;--xn:{}%;--d:{}"><span class="rm-pin">{icon}</span><span class="rm-card"><span class="rm-tag">{}</span><span class="rm-ct">{}</span>{desc}</span></li>"#,
            i * ROW + ROW / 2,
            xs_wide[i],
            xs_narrow[i],
            i,
            esc(&st.tag),
            esc(&st.title),
        ));
    }
    s.push_str("</ol></section>");

    // Choices to make
    s.push_str(&format!(
        r#"<section class="rm-decide" id="rm-decide"><h2>Choices to make <span class="rm-lc">{undecided}</span></h2><div class="rm-grid">"#
    ));
    for d in decs.iter().filter(|d| d.status != "decided") {
        s.push_str(&format!(
            r#"<div class="rm-q"><span class="rm-tagc rm-t-{}">{}</span><span class="rm-ct">{}</span><span class="rm-cm">#{}</span></div>"#,
            d.status,
            d.status,
            esc(&d.question),
            esc(&d.num)
        ));
    }
    if still_open > 0 {
        s.push_str(&format!(
            r#"<div class="rm-q rm-more"><span class="rm-ct">+{still_open} more open questions</span><span class="rm-cm">in the notes below</span></div>"#
        ));
    }
    s.push_str("</div></section>");

    // Full notes, folded
    s.push_str(r#"<section class="rm-notes"><h2>Full notes</h2>"#);
    for (label, html) in [("What is built", notes[0]), ("Next and known gaps", notes[1]), ("Open questions", notes[2])] {
        s.push_str(&format!(r#"<details class="rm-details"><summary>{label}</summary>{html}</details>"#));
    }
    s.push_str("</section></div>");
    s
}

/// Wraps a page's plain doc text so the visual leads and the text folds away.
pub fn details(body_html: &str) -> String {
    format!(r#"<details class="rm-details"><summary>Read the full notes</summary>{body_html}</details>"#)
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
    fn board_winds_and_combines_all_three() {
        let done = "| Step | What | State |\n|---|---|---|\n| 1 | Turn engine, adapter | Done |\n| 2 | Memory DB | Done |\n\nAlso built since: phone capture, location. See x.\n";
        let next = "# Next\n\n## Planned milestones\n\n| # | Milestone | What it means |\n|---|---|---|\n| 1 | Account system fixes | Close [gaps](a.md). |\n| 2 | Email bridges | More mail. |\n\n## Gaps\n\n- a\n";
        let open = "| # | Question | Status |\n|---|---|---|\n| 1 | Language | **Decided** |\n| 5 | Name | Open |\n\n## Still open\n\n- a\n";
        let b = board(done, next, open, ["<p>D</p>", "<p>N</p>", "<p>O</p>"]);
        // stops: 2 built + built-since + now + 2 next + later = 7
        assert_eq!(b.matches(r#"<li class="rm-at"#).count(), 7);
        assert!(b.contains("rm-k-now") && b.contains("rm-k-later"));
        assert!(b.contains("Account system fixes") && b.contains("Close gaps."));
        assert!(b.contains("Phone capture · Location"));
        assert!(b.contains(" C"), "road must curve");
        assert!(b.contains("rm-wide") && b.contains("rm-narrow"));
        assert!(b.contains("2/2") && b.contains(">2</span><span class=\"rm-cap\">choices"));
        assert!(b.contains("<p>D</p>") && b.contains("<p>N</p>") && b.contains("<p>O</p>"));
        assert!(!b.contains(">Language<"));
    }

    #[test]
    fn wind_path_alternates_with_curves() {
        let d = wind_path(&[(30.0, 50.0), (70.0, 150.0), (30.0, 250.0)]);
        assert!(d.starts_with("M30.0 50.0"));
        assert_eq!(d.matches(" C").count(), 2);
    }

    #[test]
    fn plain_short_strips_markdown_and_cuts() {
        assert_eq!(plain_short("See [Done](done.md) and **bold** `code`", 80), "See Done and bold code");
        assert!(plain_short("one two three four five six", 12).ends_with('…'));
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
