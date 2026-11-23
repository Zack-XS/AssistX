//! Small JSON Schema subset for tool inputs and outputs (docs/03, Tools):
//! type, properties, required, additionalProperties: false, enum, items, min/maxLength, minimum/maximum.

use serde_json::Value;

pub fn validate(schema: &Value, v: &Value) -> Result<(), String> {
    check(schema, v, "$")
}

fn type_ok(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => true,
    }
}

fn check(s: &Value, v: &Value, at: &str) -> Result<(), String> {
    match &s["type"] {
        Value::String(t) if !type_ok(t, v) => return Err(format!("{at}: expected {t}")),
        Value::Array(ts) if !ts.iter().filter_map(Value::as_str).any(|t| type_ok(t, v)) => return Err(format!("{at}: wrong type")),
        _ => {}
    }
    if let Some(e) = s["enum"].as_array()
        && !e.contains(v)
    {
        return Err(format!("{at}: not one of the allowed values"));
    }
    if let Some(txt) = v.as_str() {
        let n = txt.chars().count() as u64;
        if s["maxLength"].as_u64().is_some_and(|m| n > m) {
            return Err(format!("{at}: too long"));
        }
        if s["minLength"].as_u64().is_some_and(|m| n < m) {
            return Err(format!("{at}: too short"));
        }
    }
    if let Some(x) = v.as_f64()
        && (s["minimum"].as_f64().is_some_and(|m| x < m) || s["maximum"].as_f64().is_some_and(|m| x > m)) {
            return Err(format!("{at}: out of range"));
        }
    if let Some(obj) = v.as_object() {
        for r in s["required"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !obj.contains_key(r) {
                return Err(format!("{at}: missing '{r}'"));
            }
        }
        let props = s["properties"].as_object();
        for (k, val) in obj {
            match props.and_then(|p| p.get(k)) {
                Some(ps) => check(ps, val, &format!("{at}.{k}"))?,
                None if s["additionalProperties"] == Value::Bool(false) => return Err(format!("{at}: unknown field '{k}'")),
                None => {}
            }
        }
    }
    if let (Some(items), Some(arr)) = (s.get("items"), v.as_array()) {
        for (i, x) in arr.iter().enumerate() {
            check(items, x, &format!("{at}[{i}]"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_basic_rules() {
        let s = json!({ "type": "object", "required": ["path"], "additionalProperties": false,
            "properties": { "path": { "type": "string", "maxLength": 5 }, "n": { "type": "integer", "minimum": 0 } } });
        assert!(validate(&s, &json!({ "path": "a.txt" })).is_ok());
        assert!(validate(&s, &json!({})).unwrap_err().contains("missing 'path'"));
        assert!(validate(&s, &json!({ "path": "toolong" })).is_err());
        assert!(validate(&s, &json!({ "path": "a", "x": 1 })).unwrap_err().contains("unknown field"));
        assert!(validate(&s, &json!({ "path": "a", "n": -1 })).is_err());
    }
}
