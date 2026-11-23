//! Hand-rolled front matter parser. The docs use three keys only:
//! `title`, `description`, `order`. Values are plain or quoted YAML scalars,
//! so a full YAML crate is not needed (keeps the dependency tree small).

/// Parsed front matter for one page.
#[derive(Debug, Clone)]
pub struct FrontMatter {
    pub title: String,
    pub description: String,
    pub order: i64,
}

/// Split a markdown file into `(front_matter, body)`.
/// Returns `(None, full_text)` if the file does not start with a `---` fence.
pub fn split<'a>(text: &'a str) -> (Option<&'a str>, &'a str) {
    let t = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"));
    match t {
        Some(rest) => match rest.find("\n---") {
            Some(end) => {
                let fm = &rest[..end];
                // skip the closing fence line (and an optional trailing \r)
                let after = &rest[end + 4..]; // past "\n---"
                let body = after.strip_prefix('\n').or_else(|| after.strip_prefix("\r\n")).unwrap_or(after);
                (Some(fm), body)
            }
            None => (None, text),
        },
        None => (None, text),
    }
}

/// Parse the three keys. Returns None if any key is missing or order is not a number.
pub fn parse(fm: &str) -> Option<FrontMatter> {
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut order: Option<i64> = None;
    for line in fm.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let Some((key, val)) = line.split_once(':') else { continue };
        let key = key.trim();
        let val = val.trim();
        let val = unquote(val);
        match key {
            "title" => title = Some(val),
            "description" => description = Some(val),
            "order" => order = val.parse().ok(),
            _ => {}
        }
    }
    Some(FrontMatter {
        title: title?,
        description: description?,
        order: order?,
    })
}

/// Strip matching single or double quotes around a scalar.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\'') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_keys() {
        let fm = "title: Tokens\ndescription: One line.\norder: 10";
        let p = parse(fm).unwrap();
        assert_eq!(p.title, "Tokens");
        assert_eq!(p.description, "One line.");
        assert_eq!(p.order, 10);
    }

    #[test]
    fn handles_quoted_values() {
        let fm = "title: 'Spacing and layout'\ndescription: \"A, b: c\"\norder: 4";
        let p = parse(fm).unwrap();
        assert_eq!(p.title, "Spacing and layout");
        assert_eq!(p.description, "A, b: c");
        assert_eq!(p.order, 4);
    }

    #[test]
    fn split_separates_front_matter_and_body() {
        let text = "---\ntitle: X\ndescription: Y\norder: 1\n---\n\n# X\n\nBody.";
        let (fm, body) = split(text);
        assert_eq!(fm.unwrap(), "title: X\ndescription: Y\norder: 1");
        assert_eq!(body, "\n# X\n\nBody.");
    }

    #[test]
    fn no_front_matter_returns_none() {
        let text = "# Just a heading\n";
        let (fm, body) = split(text);
        assert!(fm.is_none());
        assert_eq!(body, text);
    }
}
