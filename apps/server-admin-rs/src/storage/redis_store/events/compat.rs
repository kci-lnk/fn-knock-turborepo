use super::*;

pub(in crate::storage::redis_store) fn system_event_data_key(id: &str) -> String {
    format!("{EVENTS_DATA_PREFIX}{id}")
}

pub(in crate::storage::redis_store) fn system_event_stream_id_key(id: &str) -> String {
    format!("{EVENTS_STREAM_ID_PREFIX}{id}")
}
pub(in crate::storage::redis_store) fn system_event_matches_filters(
    event: &Value,
    search: &str,
    event_type: Option<&str>,
    level: Option<&str>,
    source: Option<&str>,
) -> bool {
    if event_type.is_some_and(|value| event.get("type").and_then(Value::as_str) != Some(value)) {
        return false;
    }
    if level.is_some_and(|value| event.get("level").and_then(Value::as_str) != Some(value)) {
        return false;
    }
    if source.is_some_and(|value| event.get("source").and_then(Value::as_str) != Some(value)) {
        return false;
    }

    let keyword = search.trim().to_lowercase();
    if keyword.is_empty() {
        return true;
    }

    let mut haystack = String::new();
    for key in ["id", "type", "source", "level", "happened_at", "dedupe_key"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            haystack.push_str(value);
            haystack.push(' ');
        }
    }
    if let Some(subject) = event.get("subject").and_then(Value::as_object) {
        for key in ["kind", "id"] {
            if let Some(value) = subject.get(key).and_then(Value::as_str) {
                haystack.push_str(value);
                haystack.push(' ');
            }
        }
    }

    if let Some(tags) = event.get("tags").and_then(Value::as_array) {
        for tag in tags.iter().filter_map(Value::as_str) {
            haystack.push_str(tag);
            haystack.push(' ');
        }
    }
    if let Some(payload) = event.get("payload") {
        haystack.push_str(&serde_json::to_string(payload).unwrap_or_default());
    }

    haystack.to_lowercase().contains(&keyword)
}
