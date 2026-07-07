use super::*;

pub(super) fn coalesce_success_login_logs(entries: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::new();
    let mut latest_by_key: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if entry.get("outcome").and_then(Value::as_str) != Some("success") {
            result.push(entry);
            continue;
        }

        let key = success_coalesce_key(&entry);
        let existing_index = latest_by_key.get(&key).copied();
        let should_start_new = existing_index.is_none_or(|index| {
            (entry_time_ms(&result[index]) - entry_time_ms(&entry)).abs()
                > SUCCESS_LOG_COALESCE_WINDOW_MS
        });

        if should_start_new {
            let mut next = entry;
            let repeat_count = positive_i64_from_value(next.get("repeat_count")).unwrap_or(1);
            let related_ports = entry_ports(&next);
            if let Some(object) = next.as_object_mut() {
                object.insert("repeat_count".to_string(), json!(repeat_count.max(1)));
                object.insert("related_ports".to_string(), json!(related_ports));
            }
            result.push(next);
            latest_by_key.insert(key, result.len() - 1);
            continue;
        }

        if let Some(index) = existing_index {
            let incoming_repeat = positive_i64_from_value(entry.get("repeat_count"))
                .unwrap_or(1)
                .max(1);
            let incoming_ports = entry_ports(&entry);
            let incoming_raw = entry
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Some(existing) = result.get_mut(index)
                && let Some(object) = existing.as_object_mut()
            {
                let repeat = positive_i64_from_value(object.get("repeat_count"))
                    .unwrap_or(1)
                    .max(1)
                    + incoming_repeat;
                object.insert("repeat_count".to_string(), json!(repeat));
                let mut merged_ports = object
                    .get("related_ports")
                    .and_then(Value::as_array)
                    .map(|values| merge_port_values(values.iter()))
                    .unwrap_or_default();
                merged_ports.extend(incoming_ports);
                merged_ports.sort_unstable();
                merged_ports.dedup();
                object.insert("related_ports".to_string(), json!(merged_ports));
                if !incoming_raw.is_empty() {
                    let existing_raw = object
                        .get("raw")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if !existing_raw.contains(&incoming_raw) {
                        object.insert(
                            "raw".to_string(),
                            Value::String(if existing_raw.is_empty() {
                                incoming_raw
                            } else {
                                format!("{existing_raw}\n{incoming_raw}")
                            }),
                        );
                    }
                }
            }
        }
    }

    result
}

pub(super) fn success_coalesce_key(entry: &Value) -> String {
    [
        entry.get("source").and_then(Value::as_str).unwrap_or(""),
        entry.get("outcome").and_then(Value::as_str).unwrap_or(""),
        entry.get("username").and_then(Value::as_str).unwrap_or(""),
        entry.get("ip").and_then(Value::as_str).unwrap_or(""),
        entry
            .get("auth_method")
            .and_then(Value::as_str)
            .unwrap_or(""),
    ]
    .join("|")
}

pub(super) fn entry_time_ms(entry: &Value) -> i64 {
    iso_score(entry.get("happened_at").and_then(Value::as_str))
}

pub(super) fn entry_ports(entry: &Value) -> Vec<i64> {
    let related = entry
        .get("related_ports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let port = entry.get("port").into_iter();
    merge_port_values(related.chain(port))
}

pub(super) fn merge_port_values<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<i64> {
    let mut ports = values
        .into_iter()
        .filter_map(parse_i64_from_json_like_node)
        .filter(|port| *port > 0 && *port <= 65535)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    ports
}

pub(super) async fn hydrate_ip_location_records<F>(
    state: &AppState,
    items: &mut [Value],
    mut reference: F,
) where
    F: FnMut(&Value) -> Option<String>,
{
    for item in items {
        let ip = item
            .get("ip")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if ip.is_empty() {
            continue;
        }
        let refs = reference(item).into_iter().collect::<Vec<_>>();
        match ip_location::register_usage(state, &ip, refs).await {
            Ok(location) if !location.trim().is_empty() => {
                if let Some(object) = item.as_object_mut() {
                    object.insert("ipLocation".to_string(), Value::String(location));
                }
            }
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, ip, "failed to hydrate SSH IP location"),
        }
    }
}
