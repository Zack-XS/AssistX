//! Schedule import: paste a schedule, see what was understood, add it on top of the calendar you already have.
//!
//! Nothing is replaced or removed. `preview` asks the model to read the text into events (it only writes JSON, it
//! has no tools), then every event is checked by the calendar's own rules, and marked when it already exists or clashes
//! with something. `import` adds the events the person ticked; `undo` removes exactly those again.

use crate::{App, calendar, model};
use serde_json::{Value, json};
use std::sync::Arc;

const MAX_TEXT: usize = 8000;

fn system(app: &App) -> String {
    format!("You turn a pasted schedule into calendar events. Reply with ONLY a JSON array: no words before or after, no code fence. \
Each item: {{\"title\": text, \"start\": \"YYYY-MM-DD HH:MM\" (24 hour; just \"YYYY-MM-DD\" for an all-day event), \"end\": \"YYYY-MM-DD HH:MM\" or null, \"all_day\": true or false, \
\"repeat\": \"none\" | \"day\" | \"weekdays\" | \"week\" | \"month\" | \"year\", \"until\": \"YYYY-MM-DD\" or null, \"location\": text or null}}. \
{} Use real dates. For something that repeats, start on its next occurrence from today. \"Mon to Fri\" is one item with repeat weekdays. \"Mon/Wed/Fri\" is one item per day, each with repeat week, each starting on that weekday's next occurrence. \
\"Every day\" is repeat day. A time range gives start and end. No time means all_day true. Keep titles short, as written. Never invent an event. Leave out any line that is not an event.", crate::scheduler::now_line(app))
}

/// The JSON array in a model's reply, however it is wrapped.
fn find_array(text: &str) -> Option<Vec<Value>> {
    let (a, b) = (text.find('[')?, text.rfind(']')?);
    (b > a).then(|| serde_json::from_str::<Vec<Value>>(&text[a..=b]).ok()).flatten()
}

pub fn preview(app: &Arc<App>, text: &str) -> Result<Value, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("paste your schedule first".into());
    }
    if text.chars().count() > MAX_TEXT {
        return Err("that is too long for one go: paste about 60 lines at a time".into());
    }
    let adapter = app.models.adapter(app, "local")?;
    let req = model::Request { purpose: Default::default(), system: system(app), messages: vec![json!({ "role": "user", "content": text })], tools: vec![] };
    let reply = adapter.chat(&req).map_err(|e| format!("could not read it: {e}"))?;
    let items = find_array(reply.text.as_deref().unwrap_or("")).ok_or("I could not read that as a schedule. Try fewer lines, one event per line, with days and times.")?;
    let mut out = vec![];
    for it in items.iter().take(120) {
        let a = json!({ "title": it["title"], "start": it["start"], "end": it["end"], "all_day": it["all_day"], "repeat": it["repeat"], "until": it["until"], "location": it["location"] });
        // Nulls mean "not given", the way the calendar's own tools read them.
        let a = Value::Object(a.as_object().cloned().unwrap_or_default().into_iter().filter(|(_, v)| !v.is_null()).collect());
        match calendar::check(app, &a) {
            Ok(v) => {
                let (ev, first) = (&v["event"], v["first"].as_str().unwrap_or(""));
                out.push(json!({ "ok": true, "title": ev["title"], "start": ev["start"], "end": ev["end"], "all_day": ev["all_day"], "repeat": ev["repeat"], "until": ev["until"], "location": ev["location"],
                    "first": first, "duplicate": calendar::exists_like(app, ev), "clash": calendar::clash(app, ev, first) }));
            }
            Err(e) => out.push(json!({ "ok": false, "title": it["title"], "error": e })),
        }
    }
    Ok(json!({ "events": out, "note": "Nothing has been added. Tick what you want and add it." }))
}

/// Add the events on top of the calendar. `remind_minutes` adds one reminder before each (None: no reminder).
pub fn import(app: &App, events: &Value, remind_minutes: Option<i64>) -> Result<Value, String> {
    let list = events.as_array().ok_or("events must be a list")?;
    if list.len() > 120 {
        return Err("at most 120 events at once".into());
    }
    let (mut ids, mut failed, mut duplicates) = (vec![], vec![], 0);
    for e in list {
        let mut a = json!({ "title": e["title"], "start": e["start"], "end": e["end"], "all_day": e["all_day"], "repeat": e["repeat"], "until": e["until"], "location": e["location"] });
        a = Value::Object(a.as_object().cloned().unwrap_or_default().into_iter().filter(|(_, v)| !v.is_null()).collect());
        if let Some(m) = remind_minutes {
            a["remind"] = json!([m]);
        }
        match calendar::check(app, &a) {
            Ok(v) if calendar::exists_like(app, &v["event"]) => { let _ = v; duplicates += 1 }
            Ok(_) => match calendar::add(app, &a, false) {
                Ok(done) => ids.push(done["event"]["id"].clone()),
                Err(err) => failed.push(json!({ "title": e["title"], "error": err })),
            },
            Err(err) => failed.push(json!({ "title": e["title"], "error": err })),
        }
    }
    Ok(json!({ "added": ids.len(), "ids": ids, "duplicates_skipped": duplicates, "failed": failed }))
}

/// Remove exactly the events an import added.
pub fn undo(app: &App, ids: &Value) -> Value {
    let n = ids.as_array().into_iter().flatten().filter_map(|i| i.as_str()).filter(|i| calendar::delete(app, i).is_ok()).count();
    json!({ "removed": n })
}
