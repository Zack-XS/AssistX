//! Remembering people, places and things, one folder each, and keeping every date in memory true.
//!
//! `People/mia/` holds `Mia profile` (who she is, groups, likes, birthday, plain facts) and
//! `Mia timeline` (dated things, newest last). The core writes both from a short, fixed call
//! (`remember`), so a small model cannot file "colleagues from theatre" as one blob with nobody in it.
//!
//! Dates: a date in memory always looks like `(Fri 2026-11-20, upcoming)`. The core works out the
//! weekday itself (models get it wrong) and flips `upcoming` to `today` to `done` as days pass
//! (`refresh_dates`), so something that already happened is marked done instead of staying "on Friday".

use crate::App;
use crate::memory::{self, Entry, Trust};
use crate::{scheduler, tz, util};
use serde_json::{Value, json};

const WD: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WD_FULL: [&str; 7] = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];

// ---------------- dates ----------------

/// Today as a day number in the user's own zone.
pub fn today(app: &App) -> i64 {
    day_of(&scheduler::local_str(app, util::unix_now())[..10]).unwrap_or(0)
}

/// `2026-11-20` -> day number (days since 1970-01-01, local).
pub fn day_of(date: &str) -> Option<i64> {
    if date.len() != 10 {
        return None;
    }
    Some(scheduler::parse_local(&format!("{date} 00:00"))?.div_euclid(86_400))
}

fn weekday(day: i64) -> usize {
    (day + 4).rem_euclid(7) as usize
}

fn iso(day: i64) -> String {
    let (y, m, d, _) = tz::civil(day * 86_400);
    format!("{y:04}-{m:02}-{d:02}")
}

fn status_for(day: i64, today: i64) -> &'static str {
    if day < today { "done" } else if day == today { "today" } else { "upcoming" }
}

/// `(Fri 2026-11-20, upcoming)`
pub fn marker(day: i64, today: i64) -> String {
    format!("({} {}, {})", WD[weekday(day)], iso(day), status_for(day, today))
}

/// One `Www YYYY-MM-DD, status` made right again; None when the text is not a date marker.
fn fix_marker(inner: &str, today: i64) -> Option<String> {
    let wd = inner.get(0..3)?;
    if !WD.contains(&wd) || inner.get(3..4)? != " " || inner.get(14..16)? != ", " {
        return None;
    }
    let date = inner.get(4..14)?;
    if !["upcoming", "today", "done"].contains(&inner.get(16..)?) {
        return None;
    }
    let day = day_of(date)?;
    Some(format!("{} {}, {}", WD[weekday(day)], date, status_for(day, today)))
}

/// Make every `(Www YYYY-MM-DD, status)` in the text true for `today`: right weekday, right status.
pub fn refresh_text(s: &str, today: i64) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('(') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(j) = after.find(')')
            && let Some(fixed) = fix_marker(&after[..j], today)
        {
            out.push('(');
            out.push_str(&fixed);
            out.push(')');
            rest = &after[j + 1..];
            continue;
        }
        out.push('(');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// "tomorrow", "on Friday", "next Monday" -> the same words followed by their exact date, once.
/// A bare weekday means its next turn (today counts); "last Friday" the one before; "next Friday"
/// the coming one, never today. Words already followed by a date are left alone.
pub fn annotate(app: &App, text: &str) -> String {
    annotate_at(text, today(app))
}

pub fn annotate_at(text: &str, today: i64) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut prev_word = String::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        let c = text[i..].chars().next().unwrap_or(' ');
        if !c.is_ascii_alphabetic() {
            out.push(c);
            if !c.is_whitespace() {
                prev_word.clear();
            }
            i += c.len_utf8();
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let word = &text[start..i];
        out.push_str(word);
        let lower = word.to_ascii_lowercase();
        let tail = text[i..].trim_start();
        let tail_lower = tail.to_ascii_lowercase();
        let followed = tail.starts_with('(') || tail_lower.starts_with("school") || tail_lower.starts_with("service");
        let wd = WD_FULL.iter().position(|w| *w == lower);
        let day = if followed || prev_word == "every" || prev_word == "each" || prev_word == "all" {
            None
        } else if let Some(w) = wd {
            let diff = (w as i64 - weekday(today) as i64).rem_euclid(7);
            Some(match prev_word.as_str() {
                "last" => if diff == 0 { -7 } else { diff - 7 },
                "next" => if diff == 0 { 7 } else { diff },
                _ => diff,
            } + today)
        } else {
            match lower.as_str() {
                "today" | "tonight" => Some(today),
                "tomorrow" => Some(today + 1),
                "yesterday" => Some(today - 1),
                _ => None,
            }
        };
        if let Some(d) = day {
            out.push(' ');
            out.push_str(&marker(d, today));
        }
        prev_word = lower;
    }
    out
}

/// Refresh the date markers in every current entry. Returns how many entries changed.
pub fn refresh_dates(app: &App) -> usize {
    let t = today(app);
    let mut changed = 0;
    for e in memory::all_current(&app.db) {
        let summary = refresh_text(&e.summary, t);
        let mut detail = refresh_text(&e.detail, t);
        let mut summary = summary;
        if let Some(name) = e.title.strip_suffix(" timeline") {
            summary = timeline_summary(name, &detail, t);
            detail = sort_timeline(&detail);
        }
        if summary != e.summary || detail != e.detail {
            let _ = memory::update(&app.db, &e.id, &json!({ "summary": summary, "detail": detail }), "core", None, None);
            changed += 1;
        }
    }
    if changed > 0 {
        app.force_index();
    }
    changed
}

// ---------------- the fixed call ----------------

fn strs(v: &Value, max: usize) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(|s| util::clean_line(s, 12))).filter(|s| !s.is_empty()).take(max).collect(),
        Value::String(s) => s.split(',').map(|x| util::clean_line(x, 12)).filter(|s| !s.is_empty()).take(max).collect(),
        _ => Vec::new(),
    }
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A line's words with any date marker, `(said ...)` or `(until ...)` taken off, for spotting repeats.
fn bare(line: &str) -> String {
    let mut s = line.trim_end();
    loop {
        let cut = match s.rfind(" (") {
            Some(i) if s.ends_with(')') && {
                let inner = s[i + 2..].trim_end_matches(')');
                fix_marker(inner, 0).is_some() || inner.starts_with("said ") || inner.starts_with("until ")
            } => i,
            _ => break,
        };
        s = &s[..cut];
    }
    norm(s.trim_start_matches("- "))
}

fn words(s: &str) -> std::collections::BTreeSet<String> {
    bare(s).split_whitespace().map(String::from).collect()
}

/// Facts that hold one value at a time: a newer one moves the older to `Before:`.
/// `age` ("I am 15", "is 14", "15 years old") and `grade` ("in grade 10").
fn slot_of(f: &str) -> Option<&'static str> {
    let t: Vec<String> = bare(f).split_whitespace().map(String::from).collect();
    let num = |s: &str| s.parse::<u32>().is_ok_and(|n| n <= 120);
    for (i, w) in t.iter().enumerate() {
        let prev = if i > 0 { t[i - 1].as_str() } else { "" };
        let next = t.get(i + 1).map(String::as_str).unwrap_or("");
        if num(w) && (["am", "is", "turned", "turns", "age", "aged"].contains(&prev) || ["years", "yrs", "year"].contains(&next)) && !["grade", "on", "at", "in"].contains(&next) {
            return Some("age");
        }
        if w == "grade" && (num(prev) || num(next)) {
            return Some("grade");
        }
    }
    None
}

/// Facts that go stale ("now", "currently", an age) get the day they were said, so a later reader
/// knows how old they are.
fn needs_said(f: &str) -> bool {
    let l = f.to_lowercase();
    if l.contains("(said ") || line_day(f).is_some() {
        return false;
    }
    let b = format!(" {} ", bare(f));
    slot_of(f).is_some() || l.starts_with("now:") || [" now ", " currently ", " nowadays ", " lately ", " these days ", " this week ", " this month ", " this year ", " at the moment "].iter().any(|w| b.contains(w))
}

fn top_folder(kind: &str) -> Option<&'static str> {
    match kind {
        "person" | "people" => Some("People"),
        "place" | "places" => Some("Places"),
        "thing" | "things" | "project" | "pet" => Some("Things"),
        _ => None,
    }
}

fn display_name(name: &str) -> String {
    let n = util::clean_line(name, 4);
    let mut c = n.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

const HEADS: [&str; 6] = ["Relation", "Owner", "Birthday", "Likes", "Dislikes", "Groups"];

fn union(old: &str, new: &[String]) -> String {
    let mut items: Vec<String> = old.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    for n in new {
        if !items.iter().any(|i| i.eq_ignore_ascii_case(n)) {
            items.push(n.clone());
        }
    }
    items.join(", ")
}

struct Profile<'a> {
    relation: Option<&'a str>,
    owner: Option<&'a str>,
    birthday: Option<&'a str>,
    likes: &'a [String],
    dislikes: &'a [String],
    groups: &'a [String],
    facts: &'a [String],
    /// Old facts that are no longer true; they move to `Before:`, never deleted.
    replaces: &'a [String],
    today: i64,
}

/// Merge new knowledge into a profile's text. Nothing is lost and nothing is said twice:
/// - Header lines (`Relation:`, `Owner:`, `Birthday:`, `Likes:`, `Dislikes:`, `Groups:`) once each;
///   a changed `Relation`, `Owner` or `Birthday` keeps the old value under `Before:`.
/// - Plain facts are `- ` lines under `Facts:`. A repeat is skipped; a fuller version of an old fact
///   ("colleague from my last year at theatre" over "colleague from theatre") takes its place.
/// - A newer age or grade, or a fact named in `replaces`, moves the old one to `Before:` with
///   `(until <date>)`. Stale-prone facts ("now", "currently", ages) get `(said <date>)`.
/// - Any line a person wrote by hand stays.
fn merge_profile(old: &str, p: &Profile) -> String {
    let mut head: Vec<(&str, String)> = HEADS.iter().map(|h| (*h, String::new())).collect();
    let mut facts: Vec<String> = Vec::new();
    let mut before: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    let mut in_before = false;
    for line in old.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()) {
        match line.trim() {
            "Facts:" => { in_before = false; continue }
            "Before:" => { in_before = true; continue }
            _ => {}
        }
        if line.starts_with("- ") {
            if in_before { before.push(line.to_string()) } else { facts.push(line.to_string()) }
        } else if let Some(h) = head.iter_mut().find(|(k, _)| line.strip_prefix(&format!("{k}: ")).is_some()) {
            h.1 = line[h.0.len() + 2..].to_string();
        } else {
            other.push(line.to_string());
        }
    }
    let until = format!("(until {})", iso(p.today));
    let retire = |before: &mut Vec<String>, text: &str| {
        let line = format!("- {} {until}", text.trim_start_matches("- "));
        if !before.iter().any(|b| bare(b) == bare(&line)) {
            before.push(line);
        }
    };
    for h in head.iter_mut() {
        let set = |v: Option<&str>, n: usize| v.filter(|r| !r.trim().is_empty()).map(|r| util::clean_line(r, n));
        let new = match h.0 {
            "Relation" => set(p.relation, 14),
            "Owner" => set(p.owner, 6),
            "Birthday" => set(p.birthday, 6),
            "Likes" => { h.1 = union(&h.1, p.likes); None }
            "Dislikes" => { h.1 = union(&h.1, p.dislikes); None }
            _ => { h.1 = union(&h.1, p.groups); None }
        };
        if let Some(n) = new {
            if !h.1.is_empty() && !h.1.eq_ignore_ascii_case(&n) {
                retire(&mut before, &format!("{} was: {}", h.0, h.1));
            }
            h.1 = n;
        }
    }
    for r in p.replaces {
        let key = words(r);
        if key.is_empty() {
            continue;
        }
        let (gone, keep): (Vec<String>, Vec<String>) = facts.drain(..).partition(|f| {
            let w = words(f);
            let shared = w.intersection(&key).count();
            bare(f).contains(&bare(r)) || (shared * 10 >= key.len() * 7 && shared * 10 >= w.len() * 5)
        });
        facts = keep;
        for g in gone {
            retire(&mut before, &g);
        }
    }
    for f in p.facts {
        let mut f = util::clean_line(f, 40);
        if f.is_empty() {
            continue;
        }
        if needs_said(&f) {
            f = format!("{f} (said {})", iso(p.today));
        }
        let new = words(&f);
        if facts.iter().any(|x| words(x).is_superset(&new)) {
            continue; // already known, maybe in fuller words
        }
        if let Some(slot) = slot_of(&f) {
            let (gone, keep): (Vec<String>, Vec<String>) = facts.drain(..).partition(|x| slot_of(x) == Some(slot));
            facts = keep;
            for g in gone {
                retire(&mut before, &g);
            }
        }
        let line = format!("- {f}");
        match facts.iter().position(|x| { let w = words(x); w.len() >= 2 && new.is_superset(&w) }) {
            Some(i) => facts[i] = line,
            None => facts.push(line),
        }
    }
    let mut out: Vec<String> = head.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}: {v}")).collect();
    if !facts.is_empty() {
        out.push("Facts:".into());
        out.extend(facts);
    }
    out.extend(other);
    if !before.is_empty() {
        out.push("Before:".into());
        out.extend(before);
    }
    out.join("\n")
}

fn profile_summary(name: &str, detail: &str) -> String {
    let get = |k: &str| detail.lines().find_map(|l| l.strip_prefix(&format!("{k}: "))).map(str::to_string);
    let mut s = name.to_string();
    if let Some(r) = get("Relation") {
        s.push_str(&format!(": {r}."));
    } else {
        s.push('.');
    }
    if let Some(o) = get("Owner") {
        s.push_str(&format!(" Belongs to {o}."));
    }
    if let Some(g) = get("Groups") {
        s.push_str(&format!(" Groups: {g}."));
    }
    if let Some(b) = get("Birthday") {
        s.push_str(&format!(" Birthday {b}."));
    }
    if let Some(l) = get("Likes") {
        s.push_str(&format!(" Likes {l}."));
    }
    let n = crate::links::facts_of(detail).len();
    if n > 0 {
        s.push_str(&format!(" {n} {}.", if n == 1 { "fact" } else { "facts" }));
    }
    s
}

/// The date inside a line's marker, if it has one.
fn line_day(line: &str) -> Option<i64> {
    let i = line.rfind(" (")?;
    let inner = line[i + 2..].trim_end_matches(')');
    day_of(inner.get(4..14)?)
}

fn sort_timeline(detail: &str) -> String {
    let mut lines: Vec<&str> = detail.lines().filter(|l| !l.trim().is_empty()).collect();
    lines.sort_by_key(|l| line_day(l).unwrap_or(i64::MAX));
    lines.join("\n")
}

fn timeline_summary(name: &str, detail: &str, today: i64) -> String {
    let days: Vec<i64> = detail.lines().filter_map(line_day).collect();
    let up = days.iter().filter(|d| **d >= today).count();
    let done = days.len() - up;
    let next = detail.lines().filter(|l| line_day(l).is_some_and(|d| d >= today)).min_by_key(|l| line_day(l)).map(|l| util::clean_line(l.trim_start_matches("- "), 16));
    let mut s = format!("Dated things for {name}: {up} coming, {done} done.");
    if let Some(n) = next {
        s.push_str(&format!(" Next: {n}"));
    }
    s
}

fn add_timeline_line(old: &str, text: &str, day: i64, today: i64) -> String {
    let line = format!("- {} {}", util::clean_line(text, 40), marker(day, today));
    let key = bare(&line);
    let dup = old.lines().any(|l| bare(l) == key && line_day(l) == Some(day));
    let mut all: Vec<String> = old.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    if !dup {
        all.push(line);
    }
    sort_timeline(&all.join("\n"))
}

/// Write or extend one entry in a folder, found by its title.
#[allow(clippy::too_many_arguments)]
fn upsert(app: &App, folder: &str, title: &str, summary: &str, detail: &str, sources: &[String], trust: Trust, by: &str, confidence: Option<f64>, people: &[String]) -> Option<Entry> {
    if let Some(old) = memory::find_by_title(&app.db, title, folder) {
        let mut patch = json!({ "summary": summary, "detail": detail, "sources": sources, "people": people });
        if let Some(c) = confidence {
            patch["confidence"] = json!(c);
        }
        return memory::update(&app.db, &old.id, &patch, by, None, Some(trust)).ok();
    }
    let e = Entry {
        title: title.into(), summary: summary.into(), detail: detail.into(), topic_path: folder.into(),
        people: people.to_vec(), importance: 0.6, confidence: confidence.unwrap_or(0.0), sources: sources.to_vec(), ..Default::default()
    };
    Some(memory::create(&app.db, e, trust, by))
}

fn old_detail(app: &App, folder: &str, title: &str) -> String {
    memory::find_by_title(&app.db, title, folder).map(|e| e.detail).unwrap_or_default()
}

/// Remember something about one person, place or thing. `a`:
/// `kind` (person | place | thing, default person), `name`, `relation`, `birthday`, `likes`, `dislikes`,
/// `groups`, `facts` (each a text, or `{text, date?, with?}`), `confidence`.
/// A fact with `date` (`2026-11-20`) goes to the timeline with its exact weekday and status; `with`
/// names other people who get the same line on their own timeline (and a folder if they have none).
pub fn remember(app: &App, a: &Value, sources: &[String], trust: Trust, by: &str) -> Result<Value, String> {
    let kind = a["kind"].as_str().unwrap_or("person").to_ascii_lowercase();
    let top = top_folder(&kind).ok_or("kind is person, place or thing")?;
    let name = display_name(a["name"].as_str().unwrap_or_default());
    if name.is_empty() {
        return Err("name is required: one person, place or thing, like \"Mia\". For several people make one call each.".into());
    }
    if name.contains(',') || name.to_lowercase().contains(" and ") {
        return Err("one name per call (not \"Mia and Alanna\"). Make one call for each person.".into());
    }
    let slug = memory::slug_of(&name);
    if slug.is_empty() {
        return Err("that name has no letters".into());
    }
    let folder = format!("{top}/{slug}");
    let t = today(app);
    let confidence = a["confidence"].as_f64().map(|c| c.clamp(0.0, 1.0));
    let (likes, dislikes, groups) = (strs(&a["likes"], 12), strs(&a["dislikes"], 12), strs(&a["groups"], 6));

    // Facts: dated ones go to the timeline, the rest to the profile.
    let mut plain: Vec<String> = Vec::new();
    let mut dated: Vec<(String, i64, Vec<String>)> = Vec::new();
    let facts: Vec<Value> = match &a["facts"] {
        Value::Array(l) => l.clone(),
        Value::String(s) if !s.trim().is_empty() => vec![json!(s)],
        _ => Vec::new(),
    };
    for f in facts.iter().take(30) {
        let (text, date, with) = match f {
            Value::String(s) => (s.clone(), None, Vec::new()),
            Value::Object(_) => (f["text"].as_str().unwrap_or_default().to_string(), f["date"].as_str().map(str::to_string), strs(&f["with"], 6)),
            _ => continue,
        };
        if text.trim().is_empty() {
            continue;
        }
        match date.filter(|d| !d.trim().is_empty()) {
            Some(d) => {
                let day = day_of(d.trim()).ok_or_else(|| format!("date '{d}' must look like 2026-11-20 (work it out from the Now line)"))?;
                dated.push((text, day, with));
            }
            None => plain.push(annotate_at(text.trim(), t)),
        }
    }

    // The folder, then the profile.
    let desc = match a["relation"].as_str().filter(|r| !r.trim().is_empty()) {
        Some(r) => format!("{name}: {}", util::clean_line(r, 10)),
        None => name.clone(),
    };
    memory::make_folder(&app.db, &folder, &desc)?;
    let ptitle = format!("{name} profile");
    let replaces = strs(&a["replaces"], 10);
    let owner = a["owner"].as_str().map(display_name);
    let merged = merge_profile(&old_detail(app, &folder, &ptitle), &Profile {
        relation: a["relation"].as_str(), owner: owner.as_deref(), birthday: a["birthday"].as_str(), likes: &likes, dislikes: &dislikes,
        groups: &groups, facts: &plain, replaces: &replaces, today: t,
    });
    let others: Vec<String> = dated.iter().flat_map(|d| d.2.clone()).collect();
    let prof = upsert(app, &folder, &ptitle, &profile_summary(&name, &merged), &merged, sources, trust, by, confidence, &others)
        .ok_or("could not save the profile")?;

    // The timeline.
    let mut timeline_id = None;
    if !dated.is_empty() {
        let ttitle = format!("{name} timeline");
        let mut detail = old_detail(app, &folder, &ttitle);
        for (text, day, _) in &dated {
            detail = add_timeline_line(&detail, text, *day, t);
        }
        let e = upsert(app, &folder, &ttitle, &timeline_summary(&name, &detail, t), &detail, sources, trust, by, confidence, &others);
        timeline_id = e.map(|e| e.id);
    }

    // The other people in the same facts get them too (and a folder, if they had none).
    let mut also: Vec<String> = Vec::new();
    for (text, day, with) in &dated {
        for w in with {
            let w = display_name(w);
            if w.is_empty() || w.eq_ignore_ascii_case(&name) {
                continue;
            }
            let fact = json!({ "text": text, "date": iso(*day) });
            let sub = json!({ "kind": "person", "name": w, "facts": [fact] });
            if remember(app, &sub, sources, trust, by).is_ok() && !also.contains(&w) {
                also.push(w);
            }
        }
    }

    // Groups list their members, so "theatre colleagues" is a folder with people in it.
    if top == "People" {
        for g in &groups {
            let gname = display_name(g);
            let gfolder = "People/groups";
            let _ = memory::make_folder(&app.db, gfolder, "Groups of people: theatre, family, school");
            let gtitle = format!("{gname} group");
            let mut members: Vec<String> = old_detail(app, gfolder, &gtitle).lines().filter(|l| l.starts_with("- ")).map(String::from).collect();
            if !members.iter().any(|m| norm(m) == norm(&name)) {
                members.push(format!("- {name}"));
            }
            members.sort();
            let list: Vec<&str> = members.iter().map(|m| m.trim_start_matches("- ")).collect();
            let detail = format!("Members:\n{}", members.join("\n"));
            let _ = upsert(app, gfolder, &gtitle, &format!("People in {gname}: {}.", list.join(", ")), &detail, sources, trust, by, confidence, &[]);
        }
    }

    app.force_index();
    // What this folder now links to, and a nudge when it links to nothing.
    let all = memory::all_current(&app.db);
    let linked = crate::links::labels(&all).remove(&folder).unwrap_or_default();
    let mut out = json!({
        "folder": folder, "profile": prof.id, "timeline": timeline_id, "facts_saved": plain.len() + dated.len(),
        "groups": groups, "also_added_to": also, "linked": linked,
        "note": "Saved in its own folder. Old values are kept under Before. Dates carry their exact weekday and flip to done by themselves.",
    });
    if linked.is_empty() && !crate::links::is_user_relation(a["relation"].as_str().unwrap_or_default()) {
        out["hint"] = json!(format!("{name} is not linked to anyone yet. If the chat says, call again with relation (who they are to the user) or owner (whose it is)."));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-16 is a Monday.
    fn d(s: &str) -> i64 {
        day_of(s).unwrap()
    }

    #[test]
    fn markers_get_the_right_weekday_and_status() {
        let t = d("2026-11-16");
        assert_eq!(weekday(t), 1);
        assert_eq!(marker(d("2026-11-20"), t), "(Fri 2026-11-20, upcoming)");
        assert_eq!(marker(t, t), "(Mon 2026-11-16, today)");
        assert_eq!(marker(d("2026-11-13"), t), "(Fri 2026-11-13, done)");
    }

    #[test]
    fn refresh_flips_upcoming_to_done_and_fixes_a_wrong_weekday() {
        let text = "- solo with Nathan (Mon 2026-11-20, upcoming) and (see notes)";
        assert_eq!(refresh_text(text, d("2026-11-16")), "- solo with Nathan (Fri 2026-11-20, upcoming) and (see notes)");
        assert_eq!(refresh_text(text, d("2026-11-21")), "- solo with Nathan (Fri 2026-11-20, done) and (see notes)");
        assert_eq!(refresh_text("no dates (here", d("2026-11-16")), "no dates (here");
    }

    #[test]
    fn words_like_friday_get_an_exact_date_once() {
        let t = d("2026-11-16"); // Monday
        let a = annotate_at("Zack has a solo with Nathan on Friday, rehearsal tomorrow", t);
        assert_eq!(a, "Zack has a solo with Nathan on Friday (Fri 2026-11-20, upcoming), rehearsal tomorrow (Tue 2026-11-17, upcoming)");
        assert_eq!(annotate_at(&a, t), a, "a second pass changes nothing");
        assert!(annotate_at("last Friday was fun", t).contains("(Fri 2026-11-13, done)"));
        assert_eq!(annotate_at("rehearsal every Friday", t), "rehearsal every Friday");
        assert!(annotate_at("next Monday", t).contains("(Mon 2026-11-23, upcoming)"));
    }

    #[test]
    fn profile_merge_keeps_one_header_and_no_repeats() {
        let none: Vec<String> = Vec::new();
        let t = d("2026-11-16");
        let first = merge_profile("", &Profile { relation: Some("colleague from theatre"), owner: None, birthday: None, likes: &["tap".to_string()], dislikes: &none, groups: &["Theatre".to_string()], facts: &["Plays a Who".to_string()], replaces: &none, today: t });
        let second = merge_profile(&first, &Profile { relation: None, owner: None, birthday: Some("March 14"), likes: &["TAP".to_string(), "jazz".to_string()], dislikes: &none, groups: &["Theatre".to_string()], facts: &["plays a who".to_string(), "Has a solo".to_string()], replaces: &none, today: t });
        assert_eq!(second, "Relation: colleague from theatre\nBirthday: March 14\nLikes: tap, jazz\nGroups: Theatre\nFacts:\n- Plays a Who\n- Has a solo");
        assert!(profile_summary("Mia", &second).starts_with("Mia: colleague from theatre. Groups: Theatre. Birthday March 14."));
    }

    fn facts(old: &str, new: &[&str], replaces: &[&str], relation: Option<&str>, t: i64) -> String {
        let none: Vec<String> = Vec::new();
        let f: Vec<String> = new.iter().map(|s| s.to_string()).collect();
        let r: Vec<String> = replaces.iter().map(|s| s.to_string()).collect();
        merge_profile(old, &Profile { relation, owner: None, birthday: None, likes: &none, dislikes: &none, groups: &none, facts: &f, replaces: &r, today: t })
    }

    #[test]
    fn nothing_is_lost_and_nothing_is_said_twice() {
        let t = d("2026-11-16");
        // A stale-prone fact gets the day it was said; a newer age moves the old one to Before.
        let a = facts("", &["I am 15", "Colleague from theatre"], &[], Some("friend"), t);
        assert!(a.contains("- I am 15 (said 2026-11-16)"), "{a}");
        let b = facts(&a, &["I am 16", "Colleague from my last year at theatre", "colleague from theatre"], &[], Some("best friend"), d("2027-03-01"));
        assert!(b.contains("Facts:\n- I am 16 (said 2027-03-01)") || b.contains("- I am 16 (said 2027-03-01)"), "{b}");
        assert!(b.contains("- Colleague from my last year at theatre") && !b.contains("- Colleague from theatre\n"), "fuller fact replaces the short one: {b}");
        assert!(b.contains("Before:\n") && b.contains("- I am 15 (said 2026-11-16) (until 2027-03-01)"), "{b}");
        assert!(b.contains("- Relation was: friend (until 2027-03-01)") && b.contains("Relation: best friend"), "{b}");
        // replaces: named facts retire, others stay; Before survives another merge.
        let c = facts(&b, &["Now: reading Dune"], &["reading The Count of Monte Cristo"], None, d("2027-03-02"));
        let c = facts(&facts(&c, &["Is reading The Count of Monte Cristo"], &[], None, d("2027-03-02")), &[], &["Is reading The Count of Monte Cristo"], None, d("2027-03-03"));
        assert!(c.contains("Before:") && c.contains("Count of Monte Cristo") && c.contains("(until 2027-03-03)"), "{c}");
        assert!(crate::links::facts_of(&c).iter().all(|f| !f.contains("Monte Cristo")), "{c}");
        assert!(crate::links::facts_of(&c).iter().any(|f| f.starts_with("Now: reading Dune")), "{c}");
        assert_eq!(facts(&c, &[], &[], None, d("2027-03-04")), c, "merging nothing changes nothing");
        assert_eq!(profile_summary("Max", "Facts:\n- One\nBefore:\n- Old (until 2027-01-01)"), "Max. 1 fact.");
    }

    #[test]
    fn timeline_sorts_by_date_and_counts_what_is_done() {
        let t = d("2026-11-16");
        let mut tl = add_timeline_line("", "Solo with Nathan", d("2026-11-20"), t);
        tl = add_timeline_line(&tl, "Costume parade", d("2026-11-13"), t);
        tl = add_timeline_line(&tl, "Solo with Nathan", d("2026-11-20"), t); // repeat: ignored
        assert_eq!(tl.lines().count(), 2);
        assert!(tl.lines().next().unwrap().contains("2026-11-13"));
        let s = timeline_summary("Zack", &refresh_text(&tl, t), t);
        assert!(s.contains("1 coming, 1 done") && s.contains("Next: Solo with Nathan"), "{s}");
    }
}

// ---------------- the mind map ----------------

/// What memory connects, as dots and lines for the Mind map screen: every person, place and thing
/// that has a folder, every group (linked to its members), and notes that name them. Lines come from
/// group member lists, the `people` and `places` fields, and facts shared between two people.
pub fn graph(db: &crate::db::Db) -> Value {
    use std::collections::BTreeMap;
    let entries = memory::all_current(db);
    let mut nodes: BTreeMap<String, Value> = BTreeMap::new();
    let mut links: BTreeMap<(String, String), String> = BTreeMap::new();
    let kind_of = |top: &str| match top { "People" => "person", "Places" => "place", _ => "thing" };
    let pretty = |slug: &str| display_name(&slug.replace('-', " "));
    // A node for a name, made when first mentioned (so a person named only in a fact still shows).
    let ensure = |nodes: &mut BTreeMap<String, Value>, top: &str, name: &str| -> String {
        let id = format!("{top}/{}", memory::slug_of(name));
        nodes.entry(id.clone()).or_insert_with(|| json!({ "id": id, "label": display_name(name), "kind": kind_of(top), "summary": "", "entry": null, "files": 0 }));
        id
    };
    let link = |links: &mut BTreeMap<(String, String), String>, a: &str, b: &str, why: &str| {
        if a != b {
            let key = if a < b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) };
            links.entry(key).or_insert_with(|| why.to_string());
        }
    };
    for e in &entries {
        let parts: Vec<&str> = e.topic_path.split('/').collect();
        let top = parts[0];
        if !matches!(top, "People" | "Places" | "Things") {
            continue;
        }
        match parts.get(1).copied() {
            Some("groups") => {
                let gname = e.title.strip_suffix(" group").unwrap_or(&e.title).to_string();
                let gid = format!("group/{}", memory::slug_of(&gname));
                nodes.insert(gid.clone(), json!({ "id": gid, "label": gname, "kind": "group", "summary": e.summary, "entry": e.id, "files": 1 }));
                for m in e.detail.lines().filter_map(|l| l.strip_prefix("- ")) {
                    let pid = ensure(&mut nodes, "People", m);
                    link(&mut links, &gid, &pid, "member");
                }
            }
            Some(slug) => {
                let id = format!("{top}/{slug}");
                let n = nodes.entry(id.clone()).or_insert_with(|| json!({ "id": id, "label": pretty(slug), "kind": kind_of(top), "summary": "", "entry": null, "files": 0 }));
                n["files"] = json!(n["files"].as_i64().unwrap_or(0) + 1);
                if let Some(name) = e.title.strip_suffix(" profile") {
                    n["label"] = json!(name);
                    n["summary"] = json!(e.summary);
                    n["entry"] = json!(e.id);
                }
                for w in &e.people {
                    let other = ensure(&mut nodes, "People", w);
                    link(&mut links, &id, &other, "shared plans");
                }
                for p in &e.places {
                    let other = ensure(&mut nodes, "Places", p);
                    link(&mut links, &id, &other, "place");
                }
            }
            None => {}
        }
    }
    // Other notes that name people or places.
    let mut notes: Vec<&Entry> = entries.iter().filter(|e| !matches!(e.topic_path.split('/').next(), Some("People" | "Places" | "Things")) && (!e.people.is_empty() || !e.places.is_empty())).collect();
    notes.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
    for e in notes.into_iter().take(30) {
        let id = format!("note/{}", e.id);
        nodes.insert(id.clone(), json!({ "id": id, "label": e.title, "kind": "note", "summary": e.summary, "entry": e.id, "files": 1 }));
        for w in &e.people {
            let other = ensure(&mut nodes, "People", w);
            link(&mut links, &id, &other, "mentions");
        }
        for p in &e.places {
            let other = ensure(&mut nodes, "Places", p);
            link(&mut links, &id, &other, "mentions");
        }
    }
    // Links the core works out: relations, owners, names in each other's notes (src/links.rs).
    for l in crate::links::compute(&entries).1 {
        if nodes.contains_key(&l.a) && nodes.contains_key(&l.b) {
            let key = if l.a < l.b { (l.a.clone(), l.b.clone()) } else { (l.b.clone(), l.a.clone()) };
            links.insert(key, l.why);
        }
    }
    let list: Vec<Value> = nodes.into_values().collect();
    let edges: Vec<Value> = links.into_iter().map(|((a, b), why)| json!({ "a": a, "b": b, "why": why })).collect();
    json!({ "nodes": list, "links": edges })
}
