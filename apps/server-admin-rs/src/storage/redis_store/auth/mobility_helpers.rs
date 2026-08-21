use super::*;

pub(in crate::storage::redis_store) fn limit_mobility_timeline_events(
    events: &mut Vec<Value>,
    max_events: usize,
) {
    if events.len() <= max_events {
        return;
    }
    let first_is_login = events
        .first()
        .and_then(|event| event.get("kind"))
        .and_then(Value::as_str)
        == Some("login");
    if first_is_login {
        let first = events.first().cloned();
        let tail_count = max_events.saturating_sub(1);
        let tail = events
            .iter()
            .skip(events.len().saturating_sub(tail_count))
            .cloned()
            .collect::<Vec<_>>();
        events.clear();
        if let Some(first) = first {
            events.push(first);
        }
        events.extend(tail);
    } else {
        let tail = events
            .iter()
            .skip(events.len().saturating_sub(max_events))
            .cloned()
            .collect::<Vec<_>>();
        *events = tail;
    }
}

pub(in crate::storage::redis_store) fn build_mobility_summary(events: &[Value]) -> Value {
    let drift_events = events
        .iter()
        .filter(|event| event.get("kind").and_then(Value::as_str) == Some("drift"))
        .collect::<Vec<_>>();
    let last_drift = drift_events.last().copied();
    json!({
        "hasHistory": !events.is_empty(),
        "driftCount": drift_events.len(),
        "lastDriftAt": last_drift
            .and_then(|event| event.get("happenedAt"))
            .and_then(Value::as_str),
        "lastDriftSource": last_drift
            .and_then(|event| event.get("source"))
            .and_then(Value::as_str)
    })
}

pub(in crate::storage::redis_store) fn next_mobility_summary_from_event(
    events: &[Value],
    stored_summary: Option<Value>,
    event: &Value,
    seed_login_event: Option<&Value>,
) -> Value {
    let baseline = stored_summary.unwrap_or_else(|| {
        if events.is_empty() {
            let seeded = seed_login_event.cloned().into_iter().collect::<Vec<_>>();
            build_mobility_summary(&seeded)
        } else {
            build_mobility_summary(events)
        }
    });

    if event.get("kind").and_then(Value::as_str) != Some("drift") {
        return baseline;
    }

    let drift_count = baseline
        .get("driftCount")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + 1;
    json!({
        "hasHistory": true,
        "driftCount": drift_count,
        "lastDriftAt": event
            .get("happenedAt")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "lastDriftSource": event
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("session-refresh")
    })
}
