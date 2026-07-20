use serde_json::{Value, json};

use super::{
    AUTH_MAX_TTL_SECONDS, AUTH_POST_LOGIN_IP_GRANT_TTL_SECONDS_DEFAULT,
    AUTH_REMEMBER_ME_TTL_SECONDS_DEFAULT, AUTH_SESSION_IP_MOBILITY_WINDOW_SECONDS_DEFAULT,
    AUTH_SESSION_TTL_SECONDS_DEFAULT,
};

pub(super) fn auth_credential_settings_from_config(config: &Value) -> Value {
    normalize_auth_credential_settings(
        config
            .get("auth_credential_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
        legacy_auto_add_whitelist_on_login(config),
    )
}

pub(super) fn normalize_auth_credential_settings(
    value: Value,
    legacy_auto_add_whitelist_on_login: Option<bool>,
) -> Value {
    let session_ttl = bounded_int_like_node(
        &value,
        "session_ttl_seconds",
        AUTH_SESSION_TTL_SECONDS_DEFAULT,
        60,
        AUTH_MAX_TTL_SECONDS,
    );
    let remember_ttl = bounded_int_like_node(
        &value,
        "remember_me_ttl_seconds",
        AUTH_REMEMBER_ME_TTL_SECONDS_DEFAULT,
        session_ttl,
        AUTH_MAX_TTL_SECONDS,
    );
    let ip_grant_mode = match value
        .get("post_login_ip_grant_mode")
        .and_then(Value::as_str)
    {
        Some("disabled") => "disabled",
        Some("custom") => "custom",
        Some("follow_session") => "follow_session",
        _ if legacy_auto_add_whitelist_on_login == Some(false) => "disabled",
        _ => "follow_session",
    };
    let post_login_ip_grant_ttl_seconds = (ip_grant_mode == "custom").then(|| {
        bounded_int_like_node(
            &value,
            "post_login_ip_grant_ttl_seconds",
            AUTH_POST_LOGIN_IP_GRANT_TTL_SECONDS_DEFAULT,
            60,
            AUTH_MAX_TTL_SECONDS,
        )
    });
    json!({
        "session_ttl_seconds": session_ttl,
        "remember_me_ttl_seconds": remember_ttl,
        "post_login_ip_grant_mode": ip_grant_mode,
        "post_login_ip_grant_ttl_seconds": post_login_ip_grant_ttl_seconds,
        "session_ip_mobility_enabled": value
            .get("session_ip_mobility_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "session_ip_mobility_window_seconds": bounded_int_like_node(
            &value,
            "session_ip_mobility_window_seconds",
            AUTH_SESSION_IP_MOBILITY_WINDOW_SECONDS_DEFAULT,
            60,
            24 * 3600,
        ),
        "passkey_bind_prompt_enabled": value
            .get("passkey_bind_prompt_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    })
}

pub(super) fn is_allowed_auth_credential_setting(key: &str) -> bool {
    matches!(
        key,
        "session_ttl_seconds"
            | "remember_me_ttl_seconds"
            | "post_login_ip_grant_mode"
            | "post_login_ip_grant_ttl_seconds"
            | "session_ip_mobility_enabled"
            | "session_ip_mobility_window_seconds"
            | "passkey_bind_prompt_enabled"
    )
}

pub(super) fn legacy_auto_add_whitelist_on_login(config: &Value) -> Option<bool> {
    config
        .pointer("/subdomain_mode/auto_add_whitelist_on_login")
        .and_then(Value::as_bool)
}

pub(super) fn bounded_int_like_node(
    value: &Value,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    value
        .get(key)
        .and_then(parse_int_like_node)
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn parse_int_like_node(value: &Value) -> Option<i64> {
    let raw = match value {
        Value::Null => return None,
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => return None,
    };
    crate::node_compat::parse_i64_prefix_trim_start(&raw)
}

pub(super) fn session_ip_mobility_settings_changed(previous: &Value, next: &Value) -> bool {
    previous
        .get("session_ip_mobility_enabled")
        .and_then(Value::as_bool)
        != next
            .get("session_ip_mobility_enabled")
            .and_then(Value::as_bool)
        || previous
            .get("session_ip_mobility_window_seconds")
            .and_then(Value::as_i64)
            != next
                .get("session_ip_mobility_window_seconds")
                .and_then(Value::as_i64)
}

pub(super) fn stream_access_grant_settings_changed(previous: &Value, next: &Value) -> bool {
    previous
        .get("post_login_ip_grant_mode")
        .and_then(Value::as_str)
        != next.get("post_login_ip_grant_mode").and_then(Value::as_str)
        || previous
            .get("post_login_ip_grant_ttl_seconds")
            .and_then(Value::as_i64)
            != next
                .get("post_login_ip_grant_ttl_seconds")
                .and_then(Value::as_i64)
}

pub(super) fn node_totp_bind_comment(comment: Option<String>) -> String {
    match comment {
        Some(value) if !value.is_empty() => value,
        _ => "New Token".to_string(),
    }
}

pub(super) use crate::json_utils::ensure_object;
