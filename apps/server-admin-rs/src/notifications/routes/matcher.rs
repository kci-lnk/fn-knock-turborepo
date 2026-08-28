use super::*;

pub(super) fn event_matches_notification_rule(event: &Value, rule: &Value) -> bool {
    if !rule.get("enabled").and_then(Value::as_bool).unwrap_or(true) {
        return false;
    }
    if event.get("type").and_then(Value::as_str) != rule.get("event_type").and_then(Value::as_str) {
        return false;
    }
    if let Some(levels) = rule.get("event_level_filter").and_then(Value::as_array)
        && !levels.is_empty()
    {
        let event_level = event.get("level").and_then(Value::as_str).unwrap_or("");
        if !levels
            .iter()
            .any(|level| level.as_str() == Some(event_level))
        {
            return false;
        }
    }
    if let Some(sources) = rule.get("event_source_filter").and_then(Value::as_array)
        && !sources.is_empty()
    {
        let event_source = event.get("source").and_then(Value::as_str).unwrap_or("");
        if !sources
            .iter()
            .any(|source| source.as_str() == Some(event_source))
        {
            return false;
        }
    }
    true
}

pub(super) fn build_notification_group_key(event: &Value, group_by: &str) -> String {
    match group_by {
        "IP" => payload_group_key(event, &["ip", "to_ip", "from_ip"], "IP", "missing:ip"),
        "SESSION" => payload_group_key(event, &["session_id"], "SESSION", "missing:session"),
        "SUBJECT" => event
            .get("subject")
            .and_then(|subject| subject.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "missing:subject".to_string()),
        "HOSTNAME" => payload_group_key(event, &["hostname"], "RESOURCE", "missing:hostname"),
        "PROVIDER" => payload_group_key(event, &["provider"], "DDNS", "missing:provider"),
        _ => "global".to_string(),
    }
}

pub(super) fn payload_group_key(
    event: &Value,
    keys: &[&str],
    subject_kind: &str,
    missing: &str,
) -> String {
    let payload = payload_text(event, keys);
    if !payload.is_empty() {
        return payload;
    }
    subject_id_for_kind(event, subject_kind).unwrap_or_else(|| missing.to_string())
}

pub(super) fn payload_text(event: &Value, keys: &[&str]) -> String {
    let Some(payload) = event.get("payload").and_then(Value::as_object) else {
        return String::new();
    };
    for key in keys {
        let Some(value) = payload.get(*key) else {
            continue;
        };
        if value.is_null() || value.as_str() == Some("") {
            continue;
        }
        return value_to_trimmed_string(value);
    }
    String::new()
}

pub(super) fn subject_id_for_kind(event: &Value, kind: &str) -> Option<String> {
    let subject = event.get("subject")?;
    if subject.get("kind").and_then(Value::as_str) != Some(kind) {
        return None;
    }
    subject
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn build_notification_message(
    event: &Value,
    rule: &Value,
    matched_count: i64,
    group_key: &str,
    translator: &Translator,
) -> Value {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("event");
    let details = build_notification_details(event, rule, matched_count, translator);
    let title = brand_notification_title(
        &build_notification_title(event, matched_count, translator),
        translator,
    );
    let happened_at = event
        .get("happened_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let event_id = event.get("id").and_then(Value::as_str).unwrap_or("");
    let window_seconds = rule
        .get("window_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(60);
    let rule_id = rule
        .get("id")
        .map(value_to_trimmed_string)
        .unwrap_or_default();
    let rule_name = rule
        .get("name")
        .map(value_to_trimmed_string)
        .unwrap_or_default();
    json!({
        "title": title,
        "summary": details.summary,
        "body_text": details.body_text,
        "body_markdown": details.body_markdown,
        "severity": notification_severity(event.get("level").and_then(Value::as_str)),
        "facts": details.facts,
        "actions": [],
        "mentions": [],
        "dedupe_key": format!("{rule_id}:{group_key}"),
        "occurred_at": if happened_at.is_empty() { time_utils::now_iso() } else { happened_at.to_string() },
        "event_id": event_id,
        "metadata": {
            "event_type": event_type,
            "event_level": event.get("level").cloned().unwrap_or_else(|| json!("INFO")),
            "event_source": event.get("source").cloned().unwrap_or_else(|| json!("SERVER_ADMIN")),
            "rule_id": if rule_id.is_empty() { Value::Null } else { json!(rule_id) },
            "rule_name": if rule_name.is_empty() { Value::Null } else { json!(rule_name) },
            "group_key": group_key,
            "matched_count": matched_count,
            "window_seconds": window_seconds,
            "threshold_count": rule.get("threshold_count").cloned().unwrap_or_else(|| json!(1)),
            "locale": translator.locale()
        }
    })
}

pub(super) fn sanitize_notification_message(message: &Value) -> Value {
    let mut sanitized = message.clone();
    let Some(object) = sanitized.as_object_mut() else {
        return sanitized;
    };

    object.remove("trace_id");
    object.remove("waf_trace_id");
    if let Some(metadata) = object.get_mut("metadata").and_then(Value::as_object_mut) {
        metadata.remove("trace_id");
        metadata.remove("waf_trace_id");
    }
    if let Some(facts) = object.get_mut("facts").and_then(Value::as_array_mut) {
        facts.retain(|fact| {
            !fact
                .get("value")
                .and_then(Value::as_str)
                .is_some_and(crate::trace_id::is_valid_trace_id)
        });
    }

    sanitized
}

pub(super) fn sanitize_notification_record(record: Value) -> Value {
    let mut sanitized = record;
    if let Some(message) = sanitized
        .get_mut("message_snapshot")
        .filter(|value| !value.is_null())
    {
        *message = sanitize_notification_message(message);
    }
    sanitized
}

pub(super) fn notification_severity(level: Option<&str>) -> &'static str {
    match level {
        Some("CRITICAL") => "critical",
        Some("ERROR") => "error",
        Some("WARN") => "warn",
        _ => "info",
    }
}
