//! Links between the people, places and things in memory (docs/04, Links).
//!
//! The core works links out from what is already saved, so a small model that forgets to fill a
//! field still gets them, and nothing can drift: there is no second copy to keep in step.
//!
//! - Relation: a profile's `Relation:` is who they are to the user, so `Anna: mom` links Anna to the
//!   user's own folder. A relation that names someone else (`dog, Alla's`) links to that one instead.
//! - Owner: an `Owner:` line, or a fact like "My dog" with no relation, links to its owner / the user.
//! - Fields: the `people` and `places` of any entry in the folder.
//! - Mentions: a saved line that names another known person, place or thing ("hang out with Nathan").
//!
//! One link per pair; the strongest reason wins (relation > owner > field > mention).

use crate::memory::{self, Entry};
use std::collections::BTreeMap;

/// A person, place or thing with its own folder (`People/anna`).
#[derive(Clone, Debug)]
pub struct Ent {
    pub id: String,
    pub name: String,
    pub relation: Option<String>,
    pub owner: Option<String>,
    pub is_user: bool,
    /// Every line saved in the folder, lower case, for spotting names.
    text: String,
    /// Plain `- ` facts of the profile.
    facts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub a: String,
    pub b: String,
    pub why: String,
    rank: u8,
}

/// `People/anna` for an entry in that folder (or below it); None for flat notes and groups.
pub fn folder_key(topic: &str) -> Option<String> {
    let mut p = topic.split('/');
    let top = p.next()?;
    let slug = p.next()?;
    (matches!(top, "People" | "Places" | "Things") && slug != "groups" && !slug.is_empty()).then(|| format!("{top}/{slug}"))
}

fn header<'a>(detail: &'a str, key: &str) -> Option<&'a str> {
    detail.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix(": ")).map(str::trim).filter(|v| !v.is_empty())
}

/// Is this relation the user themself? ("me", "the user", "myself", "the user (the person writing)")
pub fn is_user_relation(r: &str) -> bool {
    let r = r.to_lowercase();
    let r = r.trim();
    r == "me" || r == "myself" || r == "self" || r.starts_with("me,") || r.starts_with("me ") || r.contains("the user") || r.contains("that's me")
}

pub fn entities(entries: &[Entry]) -> Vec<Ent> {
    let mut by: BTreeMap<String, Ent> = BTreeMap::new();
    for e in entries {
        let Some(id) = folder_key(&e.topic_path) else { continue };
        let slug = id.split('/').nth(1).unwrap_or_default().to_string();
        let ent = by.entry(id.clone()).or_insert_with(|| Ent {
            id: id.clone(), name: pretty(&slug), relation: None, owner: None, is_user: false, text: String::new(), facts: Vec::new(),
        });
        ent.text.push('\n');
        ent.text.push_str(&e.detail.to_lowercase());
        ent.text.push('\n');
        ent.text.push_str(&e.summary.to_lowercase());
        if let Some(name) = e.title.strip_suffix(" profile") {
            ent.name = name.to_string();
            ent.relation = header(&e.detail, "Relation").map(String::from);
            ent.owner = header(&e.detail, "Owner").map(String::from);
            ent.is_user = ent.relation.as_deref().is_some_and(is_user_relation);
            ent.facts = facts_of(&e.detail);
        }
    }
    by.into_values().collect()
}

/// The `- ` lines under `Facts:` (not the ones kept under `Before:`).
pub fn facts_of(detail: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut before = false;
    for l in detail.lines() {
        let t = l.trim();
        if t == "Before:" {
            before = true;
        } else if t == "Facts:" {
            before = false;
        } else if let Some(f) = t.strip_prefix("- ")
            && !before
        {
            out.push(f.to_string());
        }
    }
    out
}

fn pretty(slug: &str) -> String {
    let s = slug.replace('-', " ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Does `hay` (lower case) hold `name` as whole words?
fn names(hay: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    if n.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
        return false;
    }
    let mut from = 0;
    while let Some(i) = hay[from..].find(&n) {
        let s = from + i;
        let e = s + n.len();
        let ok_l = hay[..s].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let ok_r = hay[e..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if ok_l && ok_r {
            return true;
        }
        from = s + n.len().max(1);
        if from >= hay.len() {
            break;
        }
    }
    false
}

/// "my dog" -> "dog"; "Zack's sister" -> "sister" (when Zack is the user); else the relation as is.
fn short_relation(r: &str, user: Option<&str>) -> String {
    let mut s = r.trim().to_string();
    let low = s.to_lowercase();
    if let Some(rest) = low.strip_prefix("my ") {
        s = s[s.len() - rest.len()..].to_string();
    } else if let Some(u) = user {
        let pref = format!("{}'s ", u.to_lowercase());
        if let Some(rest) = low.strip_prefix(&pref) {
            s = s[s.len() - rest.len()..].to_string();
        }
    }
    s
}

pub fn compute(entries: &[Entry]) -> (Vec<Ent>, Vec<Link>) {
    let ents = entities(entries);
    let user = ents.iter().find(|e| e.is_user).cloned();
    let mut links: BTreeMap<(String, String), Link> = BTreeMap::new();
    let mut add = |a: &str, b: &str, why: String, rank: u8| {
        if a == b {
            return;
        }
        let key = if a < b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) };
        let l = Link { a: a.to_string(), b: b.to_string(), why, rank };
        match links.get(&key) {
            Some(old) if old.rank >= rank => {}
            _ => {
                links.insert(key, l);
            }
        }
    };
    let find = |name: &str| -> Option<&Ent> {
        let slug = memory::slug_of(name);
        ents.iter().find(|e| e.id.ends_with(&format!("/{slug}")) || e.name.eq_ignore_ascii_case(name))
    };

    for e in &ents {
        if e.is_user {
            continue;
        }
        // Relation: to the one it names, else to the user.
        if let Some(r) = &e.relation {
            let named = ents.iter().find(|o| o.id != e.id && names(&r.to_lowercase(), &o.name) && !(o.is_user && r.to_lowercase().starts_with("my ")));
            match (named, &user) {
                (Some(o), _) if !o.is_user => add(&e.id, &o.id, format!("{}: {}", e.name, r), 4),
                (_, Some(u)) => add(&e.id, &u.id, format!("{}: {}", e.name, short_relation(r, Some(&u.name))), 4),
                _ => {}
            }
        }
        // Owner line, then "My dog" style facts.
        if let Some(o) = e.owner.as_deref().and_then(find) {
            add(&e.id, &o.id, format!("{}: belongs to {}", e.name, o.name), 3);
        } else if e.relation.is_none()
            && let Some(u) = &user
            && let Some(f) = e.facts.iter().find(|f| f.to_lowercase().starts_with("my "))
        {
            add(&e.id, &u.id, format!("{}: {}", e.name, short_relation(f, None).trim_end_matches('.')), 3);
        }
    }
    // Fields on any entry in the folder.
    for x in entries {
        let Some(id) = folder_key(&x.topic_path) else { continue };
        for n in x.people.iter().chain(x.places.iter()) {
            if let Some(o) = find(n) {
                add(&id, &o.id, format!("together in {}", x.title), 2);
            }
        }
    }
    // Mentions.
    for e in &ents {
        for o in &ents {
            if o.id != e.id && names(&e.text, &o.name) {
                add(&e.id, &o.id, format!("named in {}'s notes", e.name), 1);
            }
        }
    }
    (ents, links.into_values().collect())
}

/// Lines for a loaded entry: who and what its folder links to, with the path to load.
pub fn lines_for(db: &crate::db::Db, e: &Entry) -> String {
    // Only trusted entries name links in the prompt; outside text never writes words there.
    let all: Vec<Entry> = memory::all_current(db).into_iter().filter(|x| x.source_trust != "outside").collect();
    let (ents, links) = compute(&all);
    let mine: Vec<String> = match folder_key(&e.topic_path) {
        Some(id) => links.iter().filter_map(|l| {
            let other = if l.a == id { &l.b } else if l.b == id { &l.a } else { return None };
            let o = ents.iter().find(|x| &x.id == other)?;
            Some(format!("- {} (load dir:{}) — {}", o.name, o.id, l.why))
        }).collect(),
        None => {
            let hay = format!("{}\n{}", e.summary, e.detail).to_lowercase();
            ents.iter().filter(|o| names(&hay, &o.name) || e.people.iter().chain(e.places.iter()).any(|n| n.eq_ignore_ascii_case(&o.name)))
                .map(|o| format!("- {} (load dir:{})", o.name, o.id)).collect()
        }
    };
    if mine.is_empty() { String::new() } else { format!("\nLinked:\n{}", mine.into_iter().take(25).collect::<Vec<_>>().join("\n")) }
}

/// Short link labels per folder, for search results.
pub fn labels(entries: &[Entry]) -> BTreeMap<String, Vec<String>> {
    let (ents, links) = compute(entries);
    let name = |id: &str| ents.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in links {
        out.entry(l.a.clone()).or_default().push(format!("{} ({})", name(&l.b), l.why));
        out.entry(l.b.clone()).or_default().push(format!("{} ({})", name(&l.a), l.why));
    }
    out
}

/// People, places and things with no link to anything (the user's own folder never counts).
pub fn orphans(entries: &[Entry]) -> Vec<String> {
    let (ents, links) = compute(entries);
    ents.iter().filter(|e| !e.is_user && !links.iter().any(|l| l.a == e.id || l.b == e.id)).map(|e| e.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prof(folder: &str, name: &str, detail: &str) -> Entry {
        Entry { id: format!("m_{name}"), title: format!("{name} profile"), summary: name.into(), detail: detail.into(), topic_path: folder.into(), status: "current".into(), ..Default::default() }
    }

    fn has(links: &[Link], a: &str, b: &str) -> Option<String> {
        links.iter().find(|l| (l.a == a && l.b == b) || (l.a == b && l.b == a)).map(|l| l.why.clone())
    }

    #[test]
    fn relations_owners_and_mentions_become_links() {
        let es = vec![
            prof("People/zack", "Zack", "Relation: me, the user\nFacts:\n- Wants to hang out with Nathan and Mia"),
            prof("People/anna", "Anna", "Relation: mom\nBirthday: August 9"),
            prof("People/alla", "Alla", "Relation: Zack's sister"),
            prof("People/nathan", "Nathan", "Groups: Mainstage Theatre"),
            prof("People/mia", "Mia", "Facts:\n- Colleague from theatre"),
            prof("Things/frankie", "Frankie", "Facts:\n- My dog"),
            prof("Things/morningstar", "Morningstar", "Relation: dog, Alla's"),
            prof("Things/kane", "Kane", "Facts:\n- A dog"),
            prof("Things/beethoven", "Beethoven", "Owner: Zoey\nFacts:\n- A dog"),
            prof("People/zoey", "Zoey", "Relation: sister"),
        ];
        let (_, l) = compute(&es);
        assert_eq!(has(&l, "People/anna", "People/zack").as_deref(), Some("Anna: mom"));
        assert_eq!(has(&l, "People/alla", "People/zack").as_deref(), Some("Alla: sister"));
        assert_eq!(has(&l, "Things/frankie", "People/zack").as_deref(), Some("Frankie: dog"));
        assert_eq!(has(&l, "Things/morningstar", "People/alla").as_deref(), Some("Morningstar: dog, Alla's"));
        assert!(has(&l, "Things/morningstar", "People/zack").is_none());
        assert_eq!(has(&l, "Things/beethoven", "People/zoey").as_deref(), Some("Beethoven: belongs to Zoey"));
        assert!(has(&l, "People/zack", "People/nathan").unwrap().contains("named in"));
        assert_eq!(orphans(&es), vec!["Kane".to_string()]);
    }

    #[test]
    fn names_match_whole_words_only() {
        assert!(names("hang out with mia sometime", "Mia"));
        assert!(!names("a miami trip", "Mia"));
        assert!(!names("we ate", "Al"), "names under 3 letters never match");
    }
}
