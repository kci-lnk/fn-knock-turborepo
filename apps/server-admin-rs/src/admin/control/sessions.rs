use std::{
    collections::{BTreeMap, HashSet},
    net::IpAddr,
};

use serde_json::{Map, Value, json};

use crate::{
    auth_mobility,
    auth_mobility_keys::{
        summary_key as auth_mobility_summary_key, timeline_key as auth_mobility_timeline_key,
    },
    http_utils,
    i18n::Translator,
    ip_location,
    state::AppState,
    store::{LoginSession, WhitelistRecord},
    time_utils, whitelist,
};

pub(super) async fn ensure_session_comment(
    state: &AppState,
    session_id: &str,
    mut data: Value,
    translator: &Translator,
) -> Value {
    let Some(object) = data.as_object_mut() else {
        return data;
    };
    if object.contains_key("comment") {
        let comment = normalize_auto_ip_grant_comment_value(
            object.get("comment").and_then(Value::as_str),
            translator,
        );
        if object.get("comment").and_then(Value::as_str) != Some(comment.as_str()) {
            object.insert("comment".to_string(), Value::String(comment));
        }
        return data;
    }

    let comment = match resolve_session_default_comment(state, session_id, &data, translator).await
    {
        Ok(Some(comment)) => comment,
        Ok(None) => return data,
        Err(error) => {
            tracing::warn!(%error, %session_id, "failed to resolve session default comment");
            return data;
        }
    };

    let mut updates = Map::new();
    updates.insert("comment".to_string(), Value::String(comment.clone()));
    match state.store.update_session_value(session_id, updates).await {
        Ok(Some(updated)) => updated,
        Ok(None) | Err(_) => {
            if let Some(object) = data.as_object_mut() {
                object.insert("comment".to_string(), Value::String(comment));
            }
            data
        }
    }
}

pub(super) async fn resolve_session_default_comment(
    state: &AppState,
    session_id: &str,
    session: &Value,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
    if let Some(record_id) = session
        .get("postLoginIpGrantRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(record) = state.store.get_whitelist_record(record_id).await?
        && record.status == "active"
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    if let Some(record_id) = auth_mobility::list_session_whitelist_record_ids(state, session_id)
        .await?
        .into_iter()
        .next()
        && let Some(record) = state.store.get_whitelist_record(&record_id).await?
        && record.status == "active"
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    if let Some(ip) = session
        .get("ip")
        .and_then(Value::as_str)
        .map(http_utils::normalize_ip)
        .filter(|value| !value.is_empty())
        && let Some(record) = latest_active_whitelist_record_by_ip(state, &ip).await?
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    Ok(None)
}

pub(super) async fn latest_active_whitelist_record_by_ip(
    state: &AppState,
    ip: &str,
) -> anyhow::Result<Option<WhitelistRecord>> {
    let target_ip = ip.parse::<IpAddr>().ok();
    let now = time_utils::now_ms().div_euclid(1000);
    let mut records = state
        .store
        .list_whitelist_records()
        .await?
        .into_iter()
        .filter(|record| record.status == "active")
        .filter(|record| record.expire_at.is_none_or(|expire_at| expire_at > now))
        .filter(|record| match record.target_type() {
            "ip" => record.ip == ip,
            "cidr" => target_ip.is_some_and(|target_ip| {
                record
                    .ip
                    .parse::<ipnet::IpNet>()
                    .is_ok_and(|network| network.contains(&target_ip))
            }),
            _ => false,
        })
        .collect::<Vec<_>>();
    records.sort_by_key(|record| std::cmp::Reverse(record.created_at));
    Ok(records.into_iter().next())
}

pub(super) async fn sync_session_whitelist_comments(
    state: &AppState,
    session_id: &str,
    session: &Value,
    comment: &str,
) -> anyhow::Result<()> {
    let mut record_ids = HashSet::new();
    if let Some(record_id) = session
        .get("postLoginIpGrantRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record_ids.insert(record_id.to_string());
    }
    for record_id in auth_mobility::list_session_whitelist_record_ids(state, session_id).await? {
        record_ids.insert(record_id);
    }

    let mut changed = false;
    for record_id in record_ids {
        changed |= state
            .store
            .update_whitelist_comment(&record_id, comment.to_string())
            .await?
            .is_some();
    }
    if changed {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(())
}

pub(super) async fn revoke_custom_post_login_ip_grant_for_session(
    state: &AppState,
    session: &LoginSession,
    config: &Value,
) -> anyhow::Result<bool> {
    if !should_revoke_custom_post_login_ip_grant(session, config) {
        return Ok(false);
    }
    if let Some(record_id) = session
        .post_login_ip_grant_record_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return whitelist::remove_whitelist_record_by_id(state, record_id).await;
    }
    whitelist::remove_whitelist_records_by_ip(state, &session.ip, Some("auto")).await
}

pub(super) fn should_revoke_custom_post_login_ip_grant(
    session: &LoginSession,
    config: &Value,
) -> bool {
    if session.grant_type.as_deref() == Some("login_ip_grant")
        && session.post_login_ip_grant_mode.as_deref() == Some("custom")
    {
        return true;
    }
    session
        .comment
        .as_deref()
        .is_some_and(auth_mobility::is_auto_ip_grant_comment)
        && config
            .pointer("/auth_credential_settings/post_login_ip_grant_mode")
            .and_then(Value::as_str)
            == Some("custom")
}

pub(super) fn normalize_auto_ip_grant_comment_value(
    value: Option<&str>,
    translator: &Translator,
) -> String {
    let trimmed = value.unwrap_or("").trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if auth_mobility::is_auto_ip_grant_comment(trimmed) {
        translator.t("auth.autoIpGrantComment")
    } else {
        trimmed.to_string()
    }
}

pub(super) fn session_record(id: String, data: Value) -> Value {
    match data {
        Value::Object(mut object) => {
            object.insert("id".to_string(), Value::String(id));
            Value::Object(object)
        }
        other => json!({ "id": id, "data": other }),
    }
}

pub(super) async fn session_record_with_mobility(
    state: &AppState,
    id: String,
    data: Value,
) -> Value {
    let mut record = session_record(id.clone(), data);
    let details = session_mobility_details_value(state, &id, Some(&record)).await;
    let fnos_attachments = list_session_attachments(state, &id, "fnos-token").await;
    let trim_media_attachments = list_session_attachments(state, &id, "trim-media-token").await;
    if let Some(object) = record.as_object_mut() {
        object.insert(
            "mobility".to_string(),
            details
                .get("summary")
                .cloned()
                .unwrap_or_else(default_mobility_summary),
        );
        object.insert(
            "fnosAttachments".to_string(),
            Value::Array(fnos_attachments),
        );
        object.insert(
            "trimMediaAttachments".to_string(),
            Value::Array(trim_media_attachments),
        );
    }
    hydrate_session_record_ip_location(state, &mut record).await;
    record
}

pub(super) async fn hydrate_session_record_ip_location(state: &AppState, record: &mut Value) {
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let ip = record
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() || ip.is_empty() {
        return;
    }

    match ip_location::register_usage(state, &ip, vec![format!("session|{id}")]).await {
        Ok(location) if !location.trim().is_empty() => {
            if let Some(object) = record.as_object_mut() {
                object.insert("ipLocation".to_string(), Value::String(location));
            }
        }
        Ok(_) => {}
        Err(error) => {
            tracing::debug!(%error, %id, %ip, "failed to hydrate auth session IP location")
        }
    }
}

pub(super) async fn list_session_attachments(
    state: &AppState,
    session_id: &str,
    subject_type: &str,
) -> Vec<Value> {
    match list_session_attachments_inner(state, session_id, subject_type).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, %session_id, %subject_type, "failed to list auth mobility session attachments");
            Vec::new()
        }
    }
}

pub(super) async fn list_session_attachments_inner(
    state: &AppState,
    session_id: &str,
    subject_type: &str,
) -> anyhow::Result<Vec<Value>> {
    let binding_prefix = format!("fn_knock:auth_mobility:binding:{subject_type}:");
    let attachment_keys = state
        .store
        .list_auth_mobility_session_binding_keys(session_id)
        .await?
        .into_iter()
        .filter(|key| key.starts_with(&binding_prefix))
        .collect::<Vec<_>>();
    if attachment_keys.is_empty() {
        return Ok(Vec::new());
    }

    let mut stale_keys = Vec::new();
    let mut attachments = Vec::new();
    for storage_key in attachment_keys {
        let Some(binding) = state.store.get_json_value(&storage_key).await? else {
            stale_keys.push(storage_key);
            continue;
        };
        if let Some(attachment) =
            session_attachment_from_binding(&binding, session_id, subject_type)
        {
            attachments.push(attachment);
        } else {
            stale_keys.push(storage_key);
        }
    }
    if !stale_keys.is_empty() {
        state
            .store
            .remove_auth_mobility_session_bindings(session_id, &stale_keys)
            .await?;
    }

    attachments.sort_by(|left, right| {
        let left_ms = left
            .get("lastSeenAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        let right_ms = right
            .get("lastSeenAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        right_ms.cmp(&left_ms)
    });
    Ok(attachments)
}

pub(super) fn session_attachment_from_binding(
    binding: &Value,
    session_id: &str,
    subject_type: &str,
) -> Option<Value> {
    if binding.get("subjectType").and_then(Value::as_str) != Some(subject_type)
        || binding.get("ownerSessionId").and_then(Value::as_str) != Some(session_id)
    {
        return None;
    }

    let expire_at = binding
        .get("expireAt")
        .and_then(Value::as_i64)
        .map(|seconds| Value::String(time_utils::iso_from_ms(seconds.saturating_mul(1000))))
        .unwrap_or(Value::Null);
    Some(json!({
        "subjectHash": binding.get("subjectHash").and_then(Value::as_str).unwrap_or(""),
        "currentIp": binding.get("currentIp").and_then(Value::as_str).unwrap_or(""),
        "createdAt": binding.get("createdAt").and_then(Value::as_str).unwrap_or(""),
        "lastSeenAt": binding.get("lastSeenAt").and_then(Value::as_str).unwrap_or(""),
        "expiresAt": expire_at,
    }))
}

pub(super) async fn session_mobility_details_value(
    state: &AppState,
    session_id: &str,
    fallback_session: Option<&Value>,
) -> Value {
    let mut events = state
        .store
        .get_json_value(&auth_mobility_timeline_key(session_id))
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        let left_ms = left
            .get("happenedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        let right_ms = right
            .get("happenedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        left_ms.cmp(&right_ms)
    });
    if events.is_empty()
        && let Some(session) = fallback_session
        && let Some(login_event) = build_mobility_login_event(session)
    {
        events.push(login_event);
    }

    let stored_summary = state
        .store
        .get_json_value(&auth_mobility_summary_key(session_id))
        .await
        .ok()
        .flatten()
        .filter(valid_mobility_summary);
    let summary = stored_summary.unwrap_or_else(|| build_mobility_summary(&events));
    json!({
        "summary": summary,
        "events": events,
    })
}

pub(super) async fn hydrate_mobility_event_ip_locations(
    state: &AppState,
    session_id: &str,
    details: &mut Value,
) {
    let Some(events) = details.get_mut("events").and_then(Value::as_array_mut) else {
        return;
    };
    if events.is_empty() {
        return;
    }

    let mut seen = HashSet::new();
    let mut ips = Vec::new();
    for event in events.iter() {
        for ip_key in ["toIp", "fromIp"] {
            let ip = event
                .get(ip_key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let normalized_ip = http_utils::normalize_ip(ip);
            if !normalized_ip.is_empty() && seen.insert(normalized_ip.clone()) {
                ips.push(normalized_ip);
            }
        }
    }

    let reference = format!("session-timeline|{session_id}");
    let mut locations = BTreeMap::new();
    for ip in ips {
        match ip_location::register_usage(state, &ip, vec![reference.clone()]).await {
            Ok(location) if !location.trim().is_empty() => {
                locations.insert(ip, location);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(%error, %session_id, %ip, "failed to hydrate auth mobility event IP location");
            }
        }
    }
    apply_mobility_event_ip_locations(events, &locations);
}

pub(super) fn apply_mobility_event_ip_locations(
    events: &mut [Value],
    locations: &BTreeMap<String, String>,
) {
    if locations.is_empty() {
        return;
    }
    for event in events {
        let Some(object) = event.as_object_mut() else {
            continue;
        };
        for (ip_key, location_key) in [("toIp", "toIpLocation"), ("fromIp", "fromIpLocation")] {
            let ip = object.get(ip_key).and_then(Value::as_str).unwrap_or("");
            let normalized_ip = http_utils::normalize_ip(ip);
            if let Some(location) = locations.get(&normalized_ip) {
                object.insert(location_key.to_string(), Value::String(location.clone()));
            }
        }
    }
}

pub(super) fn build_mobility_login_event(session: &Value) -> Option<Value> {
    let ip = session.get("ip").and_then(Value::as_str)?.trim();
    if ip.is_empty() {
        return None;
    }
    let mut event = Map::new();
    event.insert("version".to_string(), Value::Number(1.into()));
    event.insert("kind".to_string(), Value::String("login".to_string()));
    let happened_at = session
        .get("loginTime")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(time_utils::now_iso);
    event.insert("happenedAt".to_string(), Value::String(happened_at));
    event.insert("source".to_string(), Value::String("login".to_string()));
    event.insert("toIp".to_string(), Value::String(ip.to_string()));
    if let Some(location) = session
        .get("ipLocation")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        event.insert(
            "toIpLocation".to_string(),
            Value::String(location.to_string()),
        );
    }
    Some(Value::Object(event))
}

pub(super) fn build_mobility_summary(events: &[Value]) -> Value {
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

pub(super) fn default_mobility_summary() -> Value {
    json!({
        "hasHistory": false,
        "driftCount": 0,
        "lastDriftAt": null,
        "lastDriftSource": null,
    })
}

pub(super) fn valid_mobility_summary(value: &Value) -> bool {
    value.get("hasHistory").and_then(Value::as_bool).is_some()
        && value.get("driftCount").and_then(Value::as_i64).is_some()
}
