use super::*;

pub(super) fn mobility_login_event(
    ip: &str,
    ip_location: Option<&str>,
    happened_at: Option<&str>,
) -> Value {
    json!({
        "version": 1,
        "kind": "login",
        "happenedAt": happened_at.map(ToString::to_string).unwrap_or_else(time_utils::now_iso),
        "source": "login",
        "toIp": ip,
        "toIpLocation": ip_location.filter(|value| !value.trim().is_empty()),
    })
}

pub(super) fn mobility_drift_event(
    source: &str,
    from_ip: &str,
    from_ip_location: Option<&str>,
    to_ip: &str,
    to_ip_location: Option<&str>,
) -> Value {
    json!({
        "version": 1,
        "kind": "drift",
        "happenedAt": time_utils::now_iso(),
        "source": normalize_drift_source(source),
        "fromIp": from_ip,
        "fromIpLocation": from_ip_location.filter(|value| !value.trim().is_empty()),
        "toIp": to_ip,
        "toIpLocation": to_ip_location.filter(|value| !value.trim().is_empty()),
    })
}

pub(super) fn mobility_summary(events: &[Value]) -> Value {
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
            .and_then(Value::as_str),
    })
}

pub(super) fn normalize_active_ip_source(value: &str) -> &str {
    match value {
        "login" | "proxy-session" | "fnos-token" | "session-refresh" | "browser-session" => value,
        _ => "session-refresh",
    }
}

pub(super) fn normalize_drift_source(value: &str) -> &str {
    match value {
        "proxy-session" | "fnos-token" | "session-refresh" | "browser-session" => value,
        _ => "session-refresh",
    }
}

pub(super) fn parse_iso_unix(value: Option<&str>) -> Option<i64> {
    value
        .and_then(time_utils::parse_iso_ms)
        .map(|ms| ms.div_euclid(1000))
}

pub(super) fn resolve_proxy_session_ttl(expire_at: Option<i64>) -> Option<i64> {
    let remaining = expire_at? - now_seconds();
    (remaining > 0).then_some(remaining)
}

pub(super) fn now_seconds() -> i64 {
    time_utils::now_ms().div_euclid(1000)
}
