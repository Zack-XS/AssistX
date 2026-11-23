//! Memory as a folder tree (docs/02, section 4): the index the model sees, folder and
//! confidence reports, and the readable copy on disk. Entries and the database stay in memory.rs.

use crate::db::Db;
use crate::memory::{
    Entry, FOLDERS, FolderRow, TOP_LEVEL_LINES, all_current, clean_topic_in, custom_tops, folder_rows, slug_of, top_of,
};
use crate::util;
use serde_json::{Value, json};

fn date_of(e: &Entry) -> String {
    util::iso(e.updated).get(..10).unwrap_or("").to_string()
}

fn in_folder(e: &Entry, folder: &str) -> bool {
    e.topic_path == folder || e.topic_path.starts_with(&format!("{folder}/"))
}

fn files_word(n: usize) -> &'static str {
    if n == 1 { "file" } else { "files" }
}

/// One file in a folder: its name, a short description, and how to read it.
fn entry_line(e: &Entry) -> String {
    let flag = match e.status.as_str() {
        "disputed" => " [disputed]",
        _ => "",
    };
    if e.sensitive() {
        return format!("- private note: personal notes (sensitive){flag} [{}] (load: mem:{})", date_of(e), e.id);
    }
    format!("- {}: {}{flag} [{}] (load: mem:{})", util::clean_line(&e.title, 8), util::clean_line(&e.summary, 20), date_of(e), e.id)
}

/// Direct subfolders of `folder`, from files and from folders made on purpose.
fn child_folders(entries: &[Entry], folders: &[FolderRow], folder: &str) -> Vec<String> {
    let prefix = format!("{folder}/");
    let mut kids: Vec<String> = entries
        .iter()
        .map(|e| e.topic_path.as_str())
        .chain(folders.iter().map(|f| f.path.as_str()))
        .filter_map(|p| p.strip_prefix(&prefix))
        .filter_map(|rest| rest.split('/').next())
        .map(String::from)
        .collect();
    kids.sort();
    kids.dedup();
    kids
}

fn describe(folders: &[FolderRow], path: &str) -> String {
    folders
        .iter()
        .find(|f| f.path == path && !f.description.is_empty())
        .map(|f| format!(" \u{2014} {}", f.description))
        .unwrap_or_default()
}

/// One top-level folder: how many files, its subfolders, and a few names, so the model can see
/// who or what is in it without reading any profile.
fn top_line(entries: &[Entry], folders: &[FolderRow], top: &str) -> String {
    let inside: Vec<&Entry> = entries.iter().filter(|e| in_folder(e, top)).collect();
    let kids = child_folders(entries, folders, top);
    let desc = describe(folders, top);
    if inside.is_empty() && kids.is_empty() {
        return format!("- {top}/{desc} (empty)");
    }
    let names: Vec<String> = inside.iter().filter(|e| !e.sensitive()).take(6).map(|e| util::clean_line(&e.title, 4)).collect();
    let private = inside.iter().filter(|e| e.sensitive()).count();
    let mut shown = names.join(", ");
    if private > 0 {
        if !shown.is_empty() {
            shown.push_str(", ");
        }
        shown.push_str(&format!("+{private} private"));
    }
    let more = inside.len().saturating_sub(names.len() + private);
    if more > 0 {
        shown.push_str(&format!(", +{more} more"));
    }
    let n = inside.len();
    let sub = if kids.is_empty() {
        String::new()
    } else {
        format!(" in {} folder{} [{}]", kids.len(), if kids.len() == 1 { "" } else { "s" }, kids.join(", "))
    };
    let tail = if shown.is_empty() { String::new() } else { format!(": {shown}") };
    format!("- {top}/{desc} ({n} {}{sub}{tail}) (load: dir:{top})", files_word(n))
}

/// Memory part of the index. Without `dir`: the five folders and any folders made on purpose,
/// each with a count and a few names. With `dir` (for example `People` or `School/math`): the
/// subfolders and the files directly in it, each with a short description.
pub fn index_lines_with(entries: &[Entry], folders: &[FolderRow], dir: Option<&str>) -> Vec<String> {
    match dir {
        None => {
            let mut tops: Vec<String> = FOLDERS.iter().map(|f| f.to_string()).collect();
            let mut extra: Vec<String> = entries
                .iter()
                .map(|e| top_of(&e.topic_path))
                .chain(folders.iter().map(|f| top_of(&f.path)))
                .filter(|t| !tops.contains(t))
                .collect();
            extra.sort();
            extra.dedup();
            tops.extend(extra);
            tops.iter().map(|t| top_line(entries, folders, t)).collect()
        }
        Some(d) => {
            let want = clean_topic_in(d, &custom_tops(folders));
            let mut lines: Vec<String> = child_folders(entries, folders, &want)
                .into_iter()
                .map(|k| {
                    let path = format!("{want}/{k}");
                    let n = entries.iter().filter(|e| in_folder(e, &path)).count();
                    format!("- {k}/{} ({n} {}) (load: dir:{path})", describe(folders, &path), files_word(n))
                })
                .collect();
            lines.extend(entries.iter().filter(|e| e.topic_path == want).map(entry_line));
            let total = lines.len();
            lines.truncate(TOP_LEVEL_LINES);
            if total > TOP_LEVEL_LINES {
                lines.push(format!("- ...and {} more (use search_memory to find them)", total - TOP_LEVEL_LINES));
            }
            lines
        }
    }
}

/// Same, with no folders made on purpose. For tests and the export.
pub fn index_lines(entries: &[Entry], dir: Option<&str>) -> Vec<String> {
    index_lines_with(entries, &[], dir)
}

/// Same, reading the folders made on purpose from the database.
pub fn index_for(db: &Db, entries: &[Entry], dir: Option<&str>) -> Vec<String> {
    index_lines_with(entries, &folder_rows(db), dir)
}

/// Every folder with counts, its description, and the average confidence of the files directly in it.
pub fn folder_tree(db: &Db) -> Vec<Value> {
    let entries = all_current(db);
    let folders = folder_rows(db);
    let mut paths: Vec<String> = FOLDERS.iter().map(|f| f.to_string()).collect();
    for p in entries.iter().map(|e| e.topic_path.clone()).chain(folders.iter().map(|f| f.path.clone())) {
        let mut acc = String::new();
        for seg in p.split('/') {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(seg);
            if !paths.contains(&acc) {
                paths.push(acc.clone());
            }
        }
    }
    paths.sort_by_key(|p| (FOLDERS.iter().position(|f| *f == p.split('/').next().unwrap_or("")).unwrap_or(99), p.to_lowercase()));
    paths
        .into_iter()
        .map(|p| {
            let direct: Vec<&Entry> = entries.iter().filter(|e| e.topic_path == p).collect();
            let avg = if direct.is_empty() {
                Value::Null
            } else {
                json!((direct.iter().map(|e| e.confidence).sum::<f64>() / direct.len() as f64 * 100.0).round() / 100.0)
            };
            json!({
                "folder": p,
                "files": direct.len(),
                "files_including_subfolders": entries.iter().filter(|e| in_folder(e, &p)).count(),
                "description": folders.iter().find(|f| f.path == p).map(|f| f.description.clone()).unwrap_or_default(),
                "average_confidence": avg,
            })
        })
        .collect()
}

/// How sure the memory is about each entry, weakest first. `folder` and `below` narrow the list.
pub fn confidence_report(db: &Db, folder: Option<&str>, below: Option<f64>) -> Value {
    let custom = custom_tops(&folder_rows(db));
    let want = folder.filter(|f| !f.trim().is_empty()).map(|f| clean_topic_in(f, &custom));
    let mut rows: Vec<Entry> = all_current(db)
        .into_iter()
        .filter(|e| want.as_ref().is_none_or(|w| in_folder(e, w)))
        .filter(|e| below.is_none_or(|b| e.confidence < b))
        .collect();
    rows.sort_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));
    let level = |c: f64| if c >= 0.8 { "high" } else if c >= 0.5 { "medium" } else { "low" };
    json!({
        "count": rows.len(),
        "entries": rows.iter().take(60).map(|e| json!({
            "id": format!("mem:{}", e.id), "title": e.title, "folder": e.topic_path,
            "confidence": e.confidence, "level": level(e.confidence), "trust": e.source_trust,
            "importance": e.importance, "status": e.status, "sources": e.sources.len(), "updated": date_of(e),
        })).collect::<Vec<_>>(),
        "note": "0.8 and up is solid, 0.5 to 0.8 is likely, under 0.5 should be checked with the user."
    })
}

/// A current entry with this title in this folder, if any, so a repeat becomes an update.
pub fn find_by_title(db: &Db, title: &str, folder: &str) -> Option<Entry> {
    let want = slug_of(title);
    all_current(db).into_iter().find(|e| e.topic_path == folder && slug_of(&e.title) == want)
}

// ---------------- folders on disk ----------------

fn file_name_for(e: &Entry) -> String {
    let mut slug = slug_of(&e.title);
    slug.truncate(40);
    let slug = slug.trim_end_matches('-').to_string();
    let slug = if slug.is_empty() { "entry".to_string() } else { slug };
    format!("{slug}-{}.md", e.id.trim_start_matches("m_").chars().take(8).collect::<String>())
}

fn file_text(e: &Entry) -> String {
    let body = if e.detail.trim().is_empty() { e.summary.clone() } else { e.detail.clone() };
    let body = if e.source_trust == "outside" { util::wrap_outside(&format!("mem:{}", e.id), &body) } else { body };
    format!(
        "# {}\ndescription: {}\ntopic: {}\ntrust: {}\nconfidence: {:.2}\nupdated: {}\nid: {}\n\n{}\n",
        e.title, e.summary, e.topic_path, e.source_trust, e.confidence, date_of(e), e.id, body
    )
}

fn prune(dir: &std::path::Path, keep: &std::collections::HashSet<std::path::PathBuf>, keep_dirs: &std::collections::HashSet<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for item in rd.flatten() {
        let p = item.path();
        if p.is_dir() {
            prune(&p, keep, keep_dirs);
            // A folder nobody made and nothing lives in goes away.
            if !keep_dirs.contains(&p) && std::fs::read_dir(&p).map(|mut r| r.next().is_none()).unwrap_or(false) {
                let _ = std::fs::remove_dir(&p);
            }
        } else if p.extension().is_some_and(|x| x == "md") && !keep.contains(&p) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Keep `<data>/memory/<Folder>/.../<name>.md` in step with the database: one file per entry,
/// the description on top, subfolders as real folders. The database stays the source of truth;
/// these files are a readable copy (edits to them are not read back).
pub fn sync_folders(data_dir: &std::path::Path, entries: &[Entry], folders: &[FolderRow]) {
    let base = data_dir.join("memory");
    let mut keep: std::collections::HashSet<std::path::PathBuf> = Default::default();
    let safe = |p: &str| p.split('/').filter(|s| !s.is_empty() && *s != "." && *s != "..").collect::<std::path::PathBuf>();
    let mut keep_dirs: std::collections::HashSet<std::path::PathBuf> = Default::default();
    for f in FOLDERS {
        let _ = std::fs::create_dir_all(base.join(f));
        keep_dirs.insert(base.join(f));
    }
    for f in folders {
        let dir = base.join(safe(&f.path));
        let _ = std::fs::create_dir_all(&dir);
        keep_dirs.extend(dir.ancestors().take_while(|a| *a != base.as_path()).map(|a| a.to_path_buf()));
    }
    for e in entries {
        let dir = base.join(safe(&e.topic_path));
        let _ = std::fs::create_dir_all(&dir);
        keep_dirs.extend(dir.ancestors().take_while(|a| *a != base.as_path()).map(|a| a.to_path_buf()));
        let path = dir.join(file_name_for(e));
        let text = file_text(e);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            let _ = std::fs::write(&path, text);
        }
        keep.insert(path);
    }
    // core.md sits in `base` itself; only files inside folders are tidied.
    if let Ok(rd) = std::fs::read_dir(&base) {
        for item in rd.flatten().filter(|i| i.path().is_dir()) {
            prune(&item.path(), &keep, &keep_dirs);
        }
    }
}
