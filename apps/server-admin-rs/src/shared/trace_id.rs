use serde_json::Value;

pub(crate) const TRACE_ID_PREFIX: &str = "trc_";
pub(crate) const LEGACY_WAF_TRACE_ID_PREFIX: &str = "waf_";
pub(crate) const TRACE_ID_PATTERN: &str =
    r"^(?:trc|waf)_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$";

pub(crate) fn is_valid_trace_id(value: &str) -> bool {
    let value = value.trim();
    let suffix = value
        .strip_prefix(TRACE_ID_PREFIX)
        .or_else(|| value.strip_prefix(LEGACY_WAF_TRACE_ID_PREFIX));
    suffix.is_some_and(|suffix| {
        suffix.len() == 36
            && suffix.as_bytes().get(14) == Some(&b'4')
            && suffix
                .as_bytes()
                .get(19)
                .is_some_and(|value| matches!(value, b'8' | b'9' | b'a' | b'b'))
            && uuid::Uuid::parse_str(suffix).is_ok_and(|uuid| uuid.to_string() == suffix)
    })
}

pub(crate) fn event_trace_id(event: &Value) -> Option<&str> {
    event
        .get("trace_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| is_valid_trace_id(value))
        .or_else(|| {
            event
                .get("waf_trace_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| is_valid_trace_id(value))
        })
        .or_else(|| {
            event
                .pointer("/payload/trace_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| is_valid_trace_id(value))
        })
        .or_else(|| {
            event
                .pointer("/payload/waf_trace_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| is_valid_trace_id(value))
        })
}

pub(crate) fn record_trace_id(record: &Value) -> Option<&str> {
    record
        .get("trace_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| is_valid_trace_id(value))
        .or_else(|| {
            record
                .pointer("/message_snapshot/trace_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| is_valid_trace_id(value))
        })
        .or_else(|| {
            record
                .pointer("/message_snapshot/metadata/trace_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| is_valid_trace_id(value))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_waf_trace_id_remains_queryable() {
        assert!(is_valid_trace_id(
            "waf_3f93d40a-89ea-4dbe-a04f-67692778d973"
        ));
        assert!(!is_valid_trace_id("trc_not-a-uuid"));
        assert!(!is_valid_trace_id(
            "trc_3F93D40A-89EA-4DBE-A04F-67692778D973"
        ));
        let event = serde_json::json!({
            "payload": { "waf_trace_id": "waf_3f93d40a-89ea-4dbe-a04f-67692778d973" }
        });
        assert_eq!(
            event_trace_id(&event),
            Some("waf_3f93d40a-89ea-4dbe-a04f-67692778d973")
        );
    }
}
