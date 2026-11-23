//! Safety Gate rules (docs/00 Safety Gate, docs/01 Autonomy levels and Approvals).
//! Pure functions, so they are easy to test. The engine applies them to every tool call.

use crate::manifest::ToolDecl;
use crate::util;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Autonomy {
    Auto,
    Ask,
    Never,
}

impl Autonomy {
    pub fn parse(s: &str) -> Option<Autonomy> {
        match s {
            "auto" => Some(Autonomy::Auto),
            "ask" => Some(Autonomy::Ask),
            "never" => Some(Autonomy::Never),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
        }
    }
}

const FLOOR_TAGS: &[&str] = &["sends", "spends", "deletes", "shares"];
/// A tool that can do anything (a shell). It always needs a yes; no setting can turn that off.
pub const UNRESTRICTED: &str = "unrestricted";
/// A tool whose every call needs a yes (sending mail). Like `unrestricted`, no setting can make it auto,
/// but its card is built for the call (it is not a shell).
pub const ALWAYS_ASK: &str = "always_ask";

/// Whether no setting may make this tool `auto`.
pub fn always_asks(d: &ToolDecl) -> bool {
    d.tags.iter().any(|t| t == UNRESTRICTED || t == ALWAYS_ASK)
}

/// Effective level, in the order of docs/01:
/// plugin `never` is final, then user config, then plugin request, then the core floor.
/// `user` is the per-tool user setting. Only a user setting may go below the floor, and never
/// for an `unrestricted` tool.
pub fn effective(decl: Option<&ToolDecl>, user: Option<Autonomy>) -> Autonomy {
    let Some(d) = decl else { return user.filter(|u| *u == Autonomy::Never).unwrap_or(Autonomy::Ask) };
    let requested = d.autonomy.as_deref().and_then(Autonomy::parse).unwrap_or(Autonomy::Ask);
    if requested == Autonomy::Never {
        return Autonomy::Never;
    }
    if always_asks(d) {
        return user.unwrap_or(requested).max(Autonomy::Ask);
    }
    if let Some(u) = user {
        return u;
    }
    let floored = d.tags.is_empty() || d.tags.iter().any(|t| FLOOR_TAGS.contains(&t.as_str()));
    if floored { requested.max(Autonomy::Ask) } else { requested }
}

/// Risk comes from tags, never from the model. Highest tag wins. No tags = medium.
pub fn risk(decl: Option<&ToolDecl>) -> Risk {
    let tags = decl.map(|d| d.tags.as_slice()).unwrap_or(&[]);
    if tags.is_empty() {
        return Risk::Medium;
    }
    tags.iter()
        .map(|t| match t.as_str() {
            "reads" | "writes" => Risk::Low,
            "sends" => Risk::Medium,
            _ => Risk::High,
        })
        .max()
        .unwrap_or(Risk::Medium)
}

/// A "yes" is bound to this exact tool, these args and this plugin version.
pub fn call_hash(plugin: &str, version: &str, tool: &str, args: &Value) -> String {
    util::sha256_hex(format!("{plugin}\n{version}\n{tool}\n{}", util::canonical_json(args)).as_bytes())
}

/// Card question, built by the core from the real call, not from model text.
pub fn question(plugin: &str, tool: &str, decl: Option<&ToolDecl>) -> String {
    let tags = decl.map(|d| d.tags.join(", ")).filter(|t| !t.is_empty()).unwrap_or_else(|| "no tags".into());
    let undo = if decl.and_then(|d| d.undo.as_ref()).is_some() { "can be undone" } else { "cannot be undone" };
    format!("Allow {plugin} to run \"{tool}\" ({tags}; {undo})?")
}

/// Card question for an `unrestricted` tool: a loud warning with the exact command, so the
/// user reads what will run, not the model's summary of it.
pub fn danger_question(plugin: &str, command: &str) -> String {
    let shown: String = command.chars().take(600).collect();
    let cut = if command.chars().count() > 600 { " [cut]" } else { "" };
    format!(
        "WARNING - FULL COMMAND ACCESS. Allow {plugin} to run this exact command?\n\n    {shown}{cut}\n\n\
         It runs as you, with a shell, in your work folder. It can change or delete any file there, and this cannot be undone. \
         Only say yes if you understand every part of it. If a page, mail or file suggested it, say no."
    )
}

/// The command an unrestricted call will run, from its args.
pub fn command_of(args: &Value) -> String {
    args["command"].as_str().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(tags: &[&str], autonomy: Option<&str>) -> ToolDecl {
        ToolDecl {
            name: "t".into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            autonomy: autonomy.map(String::from),
            undo: None,
            cost: None,
        }
    }

    #[test]
    fn floor_tags_force_ask() {
        assert_eq!(effective(Some(&decl(&["sends"], Some("auto"))), None), Autonomy::Ask);
        assert_eq!(effective(Some(&decl(&["reads", "deletes"], Some("auto"))), None), Autonomy::Ask);
        assert_eq!(effective(Some(&decl(&["reads"], Some("auto"))), None), Autonomy::Auto);
    }

    #[test]
    fn no_tags_or_unknown_tool_is_ask() {
        assert_eq!(effective(Some(&decl(&[], Some("auto"))), None), Autonomy::Ask);
        assert_eq!(effective(None, None), Autonomy::Ask);
        assert_eq!(effective(None, Some(Autonomy::Auto)), Autonomy::Ask);
        assert_eq!(effective(None, Some(Autonomy::Never)), Autonomy::Never);
    }

    #[test]
    fn plugin_never_is_final_and_user_wins_otherwise() {
        assert_eq!(effective(Some(&decl(&["reads"], Some("never"))), Some(Autonomy::Auto)), Autonomy::Never);
        assert_eq!(effective(Some(&decl(&["writes"], Some("ask"))), Some(Autonomy::Auto)), Autonomy::Auto);
        assert_eq!(effective(Some(&decl(&["reads"], Some("auto"))), Some(Autonomy::Never)), Autonomy::Never);
    }

    #[test]
    fn unrestricted_always_asks_whatever_the_settings_say() {
        let d = decl(&["writes", UNRESTRICTED], Some("auto"));
        assert_eq!(effective(Some(&d), None), Autonomy::Ask);
        assert_eq!(effective(Some(&d), Some(Autonomy::Auto)), Autonomy::Ask, "the user cannot turn the yes off");
        assert_eq!(effective(Some(&d), Some(Autonomy::Never)), Autonomy::Never, "but may block it");
        assert!(danger_question("terminal", "rm -rf build").contains("rm -rf build"));
    }

    #[test]
    fn risk_from_tags() {
        assert_eq!(risk(Some(&decl(&["reads", "writes"], None))), Risk::Low);
        assert_eq!(risk(Some(&decl(&["writes", "sends"], None))), Risk::Medium);
        assert_eq!(risk(Some(&decl(&["sends", "spends"], None))), Risk::High);
        assert_eq!(risk(Some(&decl(&[], None))), Risk::Medium);
    }

    #[test]
    fn hash_changes_with_args_and_version() {
        let a = serde_json::json!({"path": "a.txt"});
        let b = serde_json::json!({"path": "b.txt"});
        assert_ne!(call_hash("files", "0.1.0", "write", &a), call_hash("files", "0.1.0", "write", &b));
        assert_ne!(call_hash("files", "0.1.0", "write", &a), call_hash("files", "0.2.0", "write", &a));
    }
}
