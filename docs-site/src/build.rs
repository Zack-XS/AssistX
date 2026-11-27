//! `build` subcommand: render every page under `docs/` into `docs-site/out/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::assets;
use crate::check;
use crate::frontmatter;
use crate::markdown;
use crate::model::{PageRef, Section};
use crate::roadmap;
use crate::showcase;
use crate::template;
use crate::theme;

struct Pg {
    fm: frontmatter::FrontMatter,
    body: String,
    output_rel: String,
    stem: String,
    folder: String,
    is_readme: bool,
}

/// Build the site. Runs `check` first (the plan: the check runs before each
/// build). Returns Err with a message if check fails or a file cannot be read.
pub fn run(docs_dir: &Path, repo_root: &Path, out_dir: &Path) -> Result<(), String> {
    let issues = check::run(docs_dir, repo_root);
    if !issues.is_empty() {
        let mut msg = String::from("check failed; build aborted:\n");
        for i in &issues {
            msg.push_str(&format!("  {}: {}\n", i.file, i.problem));
        }
        return Err(msg);
    }

    // Collect and parse pages.
    let mut files: Vec<PathBuf> = Vec::new();
    collect_md(docs_dir, &mut files);
    files.sort();

    let mut pages: Vec<Pg> = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).map_err(|e| format!("read {}: {e}", f.display()))?;
        let (fm_str, body) = frontmatter::split(&text);
        let fm = fm_str
            .and_then(frontmatter::parse)
            .ok_or_else(|| format!("front matter missing in {}", f.display()))?;
        let repo_rel = f
            .strip_prefix(repo_root)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| f.to_string_lossy().to_string());
        let output_rel = compute_output_rel(&repo_rel);
        let stem = f
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let folder = parent_dir(&repo_rel);
        let is_readme = basename(&repo_rel) == "README.md";
        pages.push(Pg {
            fm,
            body: body.to_string(),
            output_rel,
            stem,
            folder,
            is_readme,
        });
    }

    // Group by folder.
    let mut by_folder: BTreeMap<String, Vec<&Pg>> = BTreeMap::new();
    for p in &pages {
        by_folder.entry(p.folder.clone()).or_default().push(p);
    }

    // Sections: every folder that is a real subfolder of docs/ (not "docs" root).
    let mut sections: Vec<Section> = Vec::new();
    let mut about: Vec<PageRef> = Vec::new();
    for (folder, members) in &by_folder {
        if folder == "docs" {
            for p in members {
                if !p.is_readme {
                    about.push(PageRef {
                        title: p.fm.title.clone(),
                        order: p.fm.order,
                        output_rel: p.output_rel.clone(),
                    });
                }
            }
            continue;
        }
        let readme = members.iter().find(|p| p.is_readme);
        let title = readme.map(|r| r.fm.title.clone()).unwrap_or_else(|| folder.clone());
        let order = readme.map(|r| r.fm.order).unwrap_or(999);
        let index_rel = readme
            .map(|r| r.output_rel.clone())
            .unwrap_or_else(|| format!("{folder}/index.html").replace("docs/", ""));
        let mut secs_pages: Vec<PageRef> = members
            .iter()
            .filter(|p| !p.is_readme)
            .map(|p| PageRef {
                title: p.fm.title.clone(),
                order: p.fm.order,
                output_rel: p.output_rel.clone(),
            })
            .collect();
        secs_pages.sort_by_key(|p| p.order);
        if folder == "docs/roadmap" {
            // The roadmap is one combined page; its parts stay reachable by link only.
            secs_pages.clear();
        }
        sections.push(Section {
            title,
            order,
            index_rel,
            pages: secs_pages,
        });
    }
    sections.sort_by_key(|s| s.order);
    about.sort_by_key(|p| p.order);

    // Write static assets.
    prepare_out(out_dir)?;
    let assets_dir = out_dir.join("assets");
    fs::create_dir_all(assets_dir.join("fonts")).map_err(|e| e.to_string())?;
    fs::write(assets_dir.join("tokens.css"), theme::TOKENS_CSS).map_err(|e| e.to_string())?;
    fs::write(assets_dir.join("main.css"), theme::MAIN_CSS).map_err(|e| e.to_string())?;
    fs::write(assets_dir.join("app.js"), theme::APP_JS).map_err(|e| e.to_string())?;
    for (name, data) in assets::FONTS {
        fs::write(assets_dir.join("fonts").join(name), data).map_err(|e| e.to_string())?;
    }

    let icons = assets::load_icons();
    let menu_svg = icons.get("menu").cloned().unwrap_or_default();
    let sun_svg = icons.get("sun").cloned().unwrap_or_default();
    let moon_svg = icons.get("moon").cloned().unwrap_or_default();

    // Render every page.
    let mut search: Vec<serde_json::Value> = Vec::new();
    for p in &pages {
        let base = &p.folder; // repo-relative dir, used for link rewriting
        let body_html = markdown::render_html(&p.body, base);
        let mut html = body_html;
        if p.folder == "docs/design" {
            if let Some(show) = showcase::showcase_for(&p.stem, &icons) {
                html.push_str(&show);
            }
        }
        if p.folder == "docs/roadmap" {
            // The visual leads; the plain notes fold away under it.
            let body_of = |stem: &str| {
                pages
                    .iter()
                    .find(|q| q.folder == "docs/roadmap" && q.stem == stem)
                    .map(|q| q.body.as_str())
                    .unwrap_or("")
            };
            if p.is_readme {
                // One combined page: done, next and open questions together.
                let notes = ["done", "next", "open-questions"].map(|st| {
                    let full = markdown::render_html(body_of(st), base);
                    split_h1(&full).1.to_string()
                });
                html = roadmap::board(
                    body_of("done"),
                    body_of("next"),
                    body_of("open-questions"),
                    [&notes[0], &notes[1], &notes[2]],
                );
            } else if let Some(vis) = roadmap::roadmap_visual(&p.stem, &p.body) {
                let (title, rest) = split_h1(&html);
                html = format!("{title}{vis}{}", roadmap::details(rest));
            }
        }
        let depth = p.output_rel.matches('/').count();
        let root = "../".repeat(depth);
        let home_active = p.output_rel == "docs.html";
        let sidebar = template::render_sidebar(&sections, &about, &p.output_rel, &root, home_active);
        let page = template::page(
            &p.fm.title,
            &p.fm.description,
            &root,
            &sidebar,
            &html,
            &menu_svg,
            &sun_svg,
            &moon_svg,
        );
        let out_path = out_dir.join(&p.output_rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&out_path, page).map_err(|e| format!("write {}: {e}", out_path.display()))?;

        // search index
        let headings: String = markdown::headings(&p.body)
            .into_iter()
            .map(|(_, t)| t)
            .collect::<Vec<_>>()
            .join(" | ");
        search.push(json!({
            "title": p.fm.title,
            "description": p.fm.description,
            "headings": headings,
            "path": p.output_rel,
        }));
    }

    let search_json = serde_json::to_string_pretty(&search).map_err(|e| e.to_string())?;
    fs::write(assets_dir.join("search.json"), search_json).map_err(|e| e.to_string())?;

    // The home page at the site root, with GitHub activity from github.json
    // (filled by `fetch`) and any real screenshots in assets/screens/.
    let site_dir = out_dir.parent().unwrap_or(out_dir);
    let gh = crate::github::load(&site_dir.join("github.json"));
    // Screens come in two folders, light/ and dark/, with the same file names.
    // `screens` lists the names found in light/.
    let mut screens: Vec<String> = Vec::new();
    for theme in ["light", "dark"] {
        let Ok(entries) = fs::read_dir(site_dir.join("assets").join("screens").join(theme)) else { continue };
        let to = assets_dir.join("screens").join(theme);
        fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if [".png", ".jpg", ".jpeg", ".webp"].iter().any(|x| lower.ends_with(x)) {
                fs::copy(e.path(), to.join(&name)).map_err(|e| e.to_string())?;
                if theme == "light" {
                    screens.push(name);
                }
            }
        }
    }
    screens.sort();
    let home = crate::home::page(&gh, &screens, &icons);
    fs::write(out_dir.join("index.html"), home).map_err(|e| e.to_string())?;

    Ok(())
}

fn prepare_out(out_dir: &Path) -> Result<(), String> {
    // Clear a previous build, but keep nothing of it.
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).map_err(|e| format!("clear out: {e}"))?;
    }
    fs::create_dir_all(out_dir).map_err(|e| format!("create out: {e}"))?;
    Ok(())
}

fn compute_output_rel(repo_rel: &str) -> String {
    let rel = repo_rel.strip_prefix("docs/").unwrap_or(repo_rel);
    if rel == "README.md" {
        // The site root is the home page; the docs index lives at docs.html.
        "docs.html".to_string()
    } else if let Some(stripped) = rel.strip_suffix("/README.md") {
        format!("{stripped}/index.html")
    } else if let Some(stripped) = rel.strip_suffix(".md") {
        format!("{stripped}.html")
    } else {
        rel.to_string()
    }
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_md(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
}

fn parent_dir(repo_rel: &str) -> String {
    match repo_rel.rfind('/') {
        Some(i) => repo_rel[..i].to_string(),
        None => String::new(),
    }
}

fn basename(repo_rel: &str) -> &str {
    match repo_rel.rfind('/') {
        Some(i) => &repo_rel[i + 1..],
        None => repo_rel,
    }
}

/// Splits rendered HTML into its leading `<h1>...</h1>` (if any) and the rest.
fn split_h1(html: &str) -> (&str, &str) {
    let t = html.trim_start();
    if t.starts_with("<h1") {
        if let Some(e) = t.find("</h1>") {
            let cut = e + "</h1>".len();
            return (&t[..cut], &t[cut..]);
        }
    }
    ("", html)
}
