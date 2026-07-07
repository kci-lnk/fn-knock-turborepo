use super::*;

pub(super) fn is_waf_blocking_event(event: &Value) -> bool {
    let action = event
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    if matches!(action.as_str(), "block" | "deny") {
        return true;
    }
    if matches!(action.as_str(), "detect" | "log") {
        return false;
    }
    event
        .get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.eq_ignore_ascii_case("blocking"))
        && event.get("status").and_then(Value::as_i64).is_some()
}

pub(super) async fn query_waf_logs(state: &AppState, query: &WafLogQuery) -> anyhow::Result<Value> {
    let date = normalize_date(query.date.as_deref()).map_err(anyhow::Error::msg)?;
    let available_dates = state.redis.list_waf_log_dates(&today()).await?;
    let limit = normalize_limit(query.limit.as_deref());
    let cursor = normalize_cursor(query.cursor.as_deref());

    if let Some(trace_id) = query
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let event = get_waf_log_event(state, trace_id).await?;
        let items = event
            .filter(|event| event_matches(event, query))
            .into_iter()
            .collect::<Vec<_>>();
        return Ok(json!({
            "date": date,
            "available_dates": available_dates,
            "cursor": cursor.to_string(),
            "next_cursor": "",
            "has_more": false,
            "limit": limit,
            "total": items.len(),
            "items": items,
        }));
    }

    let page = if has_log_filters(query) {
        query_filtered(state, &date, query, cursor, limit).await?
    } else {
        query_unfiltered(state, &date, cursor, limit).await?
    };

    Ok(json!({
        "date": date,
        "available_dates": available_dates,
        "cursor": cursor.to_string(),
        "next_cursor": page.next_cursor,
        "has_more": page.has_more,
        "limit": limit,
        "total": page.total,
        "items": page.items,
    }))
}

pub(super) struct WafLogPage {
    items: Vec<Value>,
    next_cursor: String,
    has_more: bool,
    total: i64,
}

pub(super) async fn query_unfiltered(
    state: &AppState,
    date: &str,
    cursor: i64,
    limit: i64,
) -> anyhow::Result<WafLogPage> {
    let original_total = state.redis.waf_log_date_total(date).await?;
    let mut events = Vec::<Value>::new();
    let mut stale_ids = Vec::<String>::new();
    let mut offset = cursor;

    while events.len() < (limit + 1) as usize {
        let ids = state
            .redis
            .waf_log_ids_desc(
                date,
                offset as isize,
                offset as isize + UNFILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
            )
            .await?;
        if ids.is_empty() {
            break;
        }
        offset += ids.len() as i64;
        let batch = events_by_ids(state, &ids).await?;
        events.extend(batch.events);
        stale_ids.extend(batch.stale_ids);
    }

    state
        .redis
        .remove_waf_log_stale_ids(date, &stale_ids)
        .await?;

    let has_more = events.len() > limit as usize;
    let items = events
        .into_iter()
        .take(limit.max(0) as usize)
        .collect::<Vec<_>>();
    let next_cursor = cursor + items.len() as i64;

    Ok(WafLogPage {
        next_cursor: if has_more {
            next_cursor.to_string()
        } else {
            String::new()
        },
        has_more,
        total: (original_total - stale_ids.len() as i64).max(0),
        items,
    })
}

pub(super) async fn query_filtered(
    state: &AppState,
    date: &str,
    query: &WafLogQuery,
    cursor: i64,
    limit: i64,
) -> anyhow::Result<WafLogPage> {
    let mut offset = 0_i64;
    let mut matched_total = 0_i64;
    let mut items = Vec::<Value>::new();
    let mut stale_ids = Vec::<String>::new();

    loop {
        let ids = state
            .redis
            .waf_log_ids_desc(
                date,
                offset as isize,
                offset as isize + FILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
            )
            .await?;
        if ids.is_empty() {
            break;
        }
        offset += ids.len() as i64;
        let batch = events_by_ids(state, &ids).await?;
        stale_ids.extend(batch.stale_ids);

        for event in batch.events {
            if !event_matches(&event, query) {
                continue;
            }
            if matched_total >= cursor && items.len() < limit as usize {
                items.push(event);
            }
            matched_total += 1;
        }
    }

    state
        .redis
        .remove_waf_log_stale_ids(date, &stale_ids)
        .await?;
    let next_cursor = cursor + items.len() as i64;
    let has_more = next_cursor < matched_total;
    Ok(WafLogPage {
        next_cursor: if has_more {
            next_cursor.to_string()
        } else {
            String::new()
        },
        has_more,
        total: matched_total,
        items,
    })
}

pub(super) struct EventBatch {
    events: Vec<Value>,
    stale_ids: Vec<String>,
}

pub(super) async fn events_by_ids(state: &AppState, ids: &[String]) -> anyhow::Result<EventBatch> {
    let raws = state.redis.waf_log_events_by_ids(ids).await?;
    let mut events = Vec::new();
    let mut stale_ids = Vec::new();
    for (id, raw) in ids.iter().zip(raws) {
        match raw.and_then(sanitize_event) {
            Some(event) => events.push(event),
            None => stale_ids.push(id.clone()),
        }
    }
    Ok(EventBatch { events, stale_ids })
}

pub(super) async fn get_waf_log_event(
    state: &AppState,
    trace_id: &str,
) -> anyhow::Result<Option<Value>> {
    let trace_id = trace_id.trim();
    if trace_id.is_empty() {
        return Ok(None);
    }
    Ok(state
        .redis
        .get_waf_log_event(trace_id)
        .await?
        .and_then(sanitize_event))
}

pub(super) fn sanitize_event(mut event: Value) -> Option<Value> {
    if event
        .get("trace_id")
        .and_then(Value::as_str)?
        .trim()
        .is_empty()
    {
        return None;
    }

    let original_rules = event
        .get("rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let initialization_rule_ids = original_rules
        .iter()
        .filter(|rule| is_initialization_rule(rule))
        .filter_map(|rule| rule.get("id").and_then(Value::as_i64))
        .collect::<std::collections::HashSet<_>>();
    let rules = original_rules
        .into_iter()
        .filter(|rule| !is_initialization_rule(rule))
        .collect::<Vec<_>>();
    let rule_ids = event.get("rule_ids").and_then(Value::as_array).map(|ids| {
        ids.iter()
            .filter(|id| {
                id.as_i64()
                    .is_none_or(|id| !initialization_rule_ids.contains(&id))
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    let interruption_rule_id = event
        .pointer("/interruption/rule_id")
        .and_then(Value::as_i64);
    let remove_interruption =
        interruption_rule_id.is_some_and(|id| initialization_rule_ids.contains(&id));
    let has_rule_signal = !rules.is_empty() || rule_ids.as_ref().is_some_and(|ids| !ids.is_empty());
    let has_blocking_signal = is_blocking_action(event.get("action"))
        || (event.get("interruption").is_some() && !remove_interruption);
    if !has_rule_signal && !has_blocking_signal {
        return None;
    }

    let object = event.as_object_mut()?;
    if !rules.is_empty() || object.contains_key("rules") {
        object.insert("rules".to_string(), Value::Array(rules));
    }
    if let Some(rule_ids) = rule_ids {
        object.insert("rule_ids".to_string(), Value::Array(rule_ids));
    }
    if remove_interruption {
        object.remove("interruption");
    }
    Some(event)
}

pub(super) fn event_matches(event: &Value, query: &WafLogQuery) -> bool {
    let host = query.host.as_deref().unwrap_or("").trim().to_lowercase();
    if !host.is_empty()
        && event
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
            != host
    {
        return false;
    }

    let client_ip = query.client_ip.as_deref().unwrap_or("").trim();
    if !client_ip.is_empty() && event.get("client_ip").and_then(Value::as_str) != Some(client_ip) {
        return false;
    }

    let route_type = query.route_type.as_deref().unwrap_or("").trim();
    if !route_type.is_empty() && event.get("route_type").and_then(Value::as_str) != Some(route_type)
    {
        return false;
    }

    let mode = query.mode.as_deref().unwrap_or("").trim();
    if !mode.is_empty() && event.get("mode").and_then(Value::as_str) != Some(mode) {
        return false;
    }

    let raw_rule_id = query.rule_id.as_deref().unwrap_or("").trim();
    if !raw_rule_id.is_empty() {
        let Some(rule_id) = crate::node_compat::parse_i64_prefix(raw_rule_id.trim_start()) else {
            return false;
        };
        let matches_rule = event
            .get("rule_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(rule_id)));
        if !matches_rule {
            return false;
        }
    }

    let search = query.search.as_deref().unwrap_or("").trim().to_lowercase();
    if !search.is_empty() {
        let mut haystack = Vec::new();
        for key in [
            "trace_id",
            "host",
            "path",
            "request_uri",
            "client_ip",
            "route_key",
            "upstream",
            "bundle_id",
        ] {
            if let Some(value) = event.get(key).and_then(Value::as_str) {
                haystack.push(value.to_string());
            }
        }
        if let Some(ids) = event.get("rule_ids").and_then(Value::as_array) {
            haystack.extend(ids.iter().map(|id| id.to_string()));
        }
        if !haystack
            .iter()
            .any(|value| value.to_lowercase().contains(&search))
        {
            return false;
        }
    }

    true
}

pub(super) fn has_log_filters(query: &WafLogQuery) -> bool {
    [
        query.search.as_deref(),
        query.host.as_deref(),
        query.client_ip.as_deref(),
        query.rule_id.as_deref(),
        query.route_type.as_deref(),
        query.mode.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !value.trim().is_empty())
}

pub(super) fn is_initialization_rule(rule: &Value) -> bool {
    rule_basename(rule.get("file")).eq_ignore_ascii_case(INITIALIZATION_RULE_FILENAME)
}

pub(super) fn rule_basename(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

pub(super) fn is_blocking_action(value: Option<&Value>) -> bool {
    matches!(
        value
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "block" | "deny"
    )
}

pub(super) fn normalize_date(value: Option<&str>) -> Result<String, &'static str> {
    let raw = value.unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(today());
    }
    if is_date(raw) {
        Ok(raw.to_string())
    } else {
        Err("invalid date, expected YYYY-MM-DD")
    }
}

pub(super) fn normalize_limit(value: Option<&str>) -> i64 {
    value
        .and_then(|value| crate::node_compat::parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(50)
        .min(200)
}

pub(super) fn normalize_cursor(value: Option<&str>) -> i64 {
    value
        .and_then(|value| crate::node_compat::parse_i64_prefix(value.trim_start()))
        .filter(|value| *value >= 0)
        .unwrap_or(0)
}

pub(super) fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub(super) fn today() -> String {
    time_utils::local_date_from_ms(time_utils::now_ms())
}
