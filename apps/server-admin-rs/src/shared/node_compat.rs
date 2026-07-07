use std::env;

use serde_json::Value;

pub(crate) fn parse_i64_prefix(value: &str) -> Option<i64> {
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }
    let mut end = 0;
    let mut has_digit = false;
    for (index, character) in chars {
        if character.is_ascii_digit() {
            has_digit = true;
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }
    value[..end].parse::<i64>().ok()
}

pub(crate) fn parse_i64_prefix_trim_start(value: &str) -> Option<i64> {
    parse_i64_prefix(value.trim_start())
}

pub(crate) fn parse_i64_or(value: Option<&str>, fallback: i64) -> i64 {
    value
        .and_then(parse_i64_prefix_trim_start)
        .unwrap_or(fallback)
}

pub(crate) fn env_i64(name: &str, fallback: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| parse_i64_prefix_trim_start(&value))
        .unwrap_or(fallback)
}

pub(crate) fn env_bool(name: &str, fallback: bool) -> bool {
    match env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        _ => fallback,
    }
}

pub(crate) fn floor_to_i64(value: f64) -> i64 {
    if value.is_finite() {
        value.floor() as i64
    } else {
        0
    }
}

pub(crate) fn parse_i64_from_json_like_node(value: &Value) -> Option<i64> {
    parse_i64_prefix_trim_start(&js_string_for_parse_int(value))
}

pub(crate) fn js_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(items) => items
            .iter()
            .map(js_array_item_string_for_parse_int)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub(crate) fn js_array_item_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Array(_) => js_string_for_parse_int(value),
        Value::Object(_) => "[object Object]".to_string(),
        _ => js_string_for_parse_int(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_i64_prefix_matches_node_parse_int_edges() {
        assert_eq!(parse_i64_prefix_trim_start("60s"), Some(60));
        assert_eq!(parse_i64_prefix_trim_start("  +3.9"), Some(3));
        assert_eq!(parse_i64_prefix_trim_start("-1x"), Some(-1));
        assert_eq!(parse_i64_prefix_trim_start("0x10"), Some(0));
        assert_eq!(parse_i64_prefix_trim_start("nope"), None);
        assert_eq!(parse_i64_prefix_trim_start("+"), None);
    }

    #[test]
    fn json_parse_int_stringification_matches_node_edges() {
        assert_eq!(parse_i64_from_json_like_node(&json!(["4.2"])), Some(4));
        assert_eq!(parse_i64_from_json_like_node(&json!([])), None);
        assert_eq!(parse_i64_from_json_like_node(&json!(["1", "2"])), Some(1));
        assert_eq!(parse_i64_from_json_like_node(&json!(true)), None);
        assert_eq!(parse_i64_from_json_like_node(&json!(null)), None);
    }
}
