use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    http_utils::normalize_ip, i18n::Translator, ip_location, response, state::AppState, time_utils,
};

const SYSTEM_EVENT_TYPES: &[&str] = &[
    "FN_EVENT_AUTH_LOGIN_SUCCESS",
    "FN_EVENT_AUTH_LOGOUT",
    "FN_EVENT_AUTH_LOGIN_FAILURE",
    "FN_EVENT_AUTH_SESSION_IP_DRIFT",
    "FN_EVENT_SECURITY_SCANNER_BLOCKED",
    "FN_EVENT_DDNS_UPDATE_COMPLETED",
    "FN_EVENT_GATEWAY_THROTTLE_BLOCKED",
    "FN_EVENT_WAF_BLOCKED",
    "FN_EVENT_SSH_LOGIN_SUCCESS",
    "FN_EVENT_SSH_LOGIN_FAILURE",
    "FN_EVENT_SSH_IP_BLOCKED",
    "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE",
    "FN_EVENT_SYSTEM_CPU_ALERT",
    "FN_EVENT_SYSTEM_CPU_RECOVERED",
    "FN_EVENT_SYSTEM_MEMORY_ALERT",
    "FN_EVENT_SYSTEM_MEMORY_RECOVERED",
    "FN_EVENT_TUNNEL_FRP_CONNECTED",
    "FN_EVENT_TUNNEL_FRP_DISCONNECTED",
    "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED",
    "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED",
];
const SYSTEM_EVENT_LEVELS: &[&str] = &["INFO", "WARN", "ERROR", "CRITICAL"];
const SYSTEM_EVENT_SOURCES: &[&str] = &["SERVER_ADMIN", "GO_REAUTH_PROXY", "SYSTEM_MONITOR"];
const SYSTEM_EVENT_SUBJECT_KINDS: &[&str] =
    &["IP", "SESSION", "DDNS", "RESOURCE", "APPLICATION", "TUNNEL"];
const APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

fn system_event_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.systemEvents.routes.{key}"))
}

#[derive(Deserialize)]
struct InternalSystemEventBody {
    #[serde(rename = "type")]
    event_type: String,
    source: String,
    level: Option<String>,
    happened_at: Option<String>,
    dedupe_key: Option<String>,
    dedupe_ttl_seconds: Option<f64>,
    subject: Option<Value>,
    tags: Option<Vec<String>>,
    payload: Value,
}

#[derive(Deserialize)]
struct EventListQuery {
    page: Option<String>,
    limit: Option<String>,
    search: Option<String>,
    #[serde(rename = "type")]
    event_type: Option<String>,
    level: Option<String>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct DeleteEventsBody {
    ids: Vec<String>,
}

pub fn internal_system_event_routes() -> Router<AppState> {
    Router::new()
        .route("/api/internal/system-events", post(publish_internal_event))
        .route("/api/internal/system-events/", post(publish_internal_event))
}

pub fn admin_event_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/events", get(list_events).delete(delete_events))
        .route("/api/admin/events/", get(list_events).delete(delete_events))
        .route("/api/admin/events/clear", delete(clear_events))
}

pub async fn publish_waf_blocked_event(state: &AppState, event: &Value) -> anyhow::Result<bool> {
    let Some(body) = waf_blocked_body(event) else {
        return Ok(false);
    };
    publish_system_event_body(state, body).await
}

fn waf_blocked_body(event: &Value) -> Option<InternalSystemEventBody> {
    let trace_id = event.get("trace_id").and_then(Value::as_str).unwrap_or("");
    if trace_id.is_empty() {
        return None;
    }
    let ip = event
        .get("client_ip")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            event
                .get("remote_addr")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("unknown");
    let blocked_at = event
        .get("time")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut payload = Map::new();
    payload.insert("ip".to_string(), Value::String(ip.to_string()));
    payload.insert("trace_id".to_string(), Value::String(trace_id.to_string()));
    if let Some(blocked_at) = blocked_at.clone() {
        payload.insert("blocked_at".to_string(), Value::String(blocked_at));
    }
    payload.insert(
        "mode".to_string(),
        Value::String(
            event
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    payload.insert(
        "action".to_string(),
        Value::String(
            event
                .get("action")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("deny")
                .to_string(),
        ),
    );
    for key in [
        "host",
        "path",
        "request_uri",
        "route_type",
        "route_key",
        "bundle_id",
    ] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            if !value.is_empty() {
                payload.insert(key.to_string(), Value::String(value.to_string()));
            }
        }
    }
    if let Some(status) = event
        .get("status")
        .and_then(Value::as_i64)
        .filter(|value| *value != 0)
    {
        payload.insert("status".to_string(), Value::Number(status.into()));
    }
    let rule_ids = event
        .get("rule_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_i64)
                .map(|value| Value::Number(value.into()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    payload.insert("rule_ids".to_string(), Value::Array(rule_ids));

    Some(InternalSystemEventBody {
        event_type: "FN_EVENT_WAF_BLOCKED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: blocked_at,
        dedupe_key: Some(format!("waf:{trace_id}")),
        dedupe_ttl_seconds: Some((24 * 60 * 60) as f64),
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: Some(vec!["waf".to_string(), "gateway".to_string()]),
        payload: Value::Object(payload),
    })
}

pub async fn publish_ddns_update_completed_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    let target_id = payload
        .get("target_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let success = payload
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let body = InternalSystemEventBody {
        event_type: "FN_EVENT_DDNS_UPDATE_COMPLETED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some(if success { "INFO" } else { "ERROR" }.to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "DDNS", "id": target_id })),
        tags: None,
        payload,
    };
    publish_system_event_body(state, body).await
}

pub async fn publish_app_update_available_event(
    state: &AppState,
    local_version: &str,
    latest_version: &str,
    force_update: bool,
    release_notes: &str,
    check_reason: &str,
) -> anyhow::Result<bool> {
    let body = app_update_available_body(
        local_version,
        latest_version,
        force_update,
        release_notes,
        check_reason,
    );
    publish_system_event_body(state, body).await
}

fn app_update_available_body(
    local_version: &str,
    latest_version: &str,
    force_update: bool,
    release_notes: &str,
    check_reason: &str,
) -> InternalSystemEventBody {
    let mut payload = Map::new();
    payload.insert(
        "local_version".to_string(),
        Value::String(local_version.to_string()),
    );
    payload.insert(
        "latest_version".to_string(),
        Value::String(latest_version.to_string()),
    );
    payload.insert("force_update".to_string(), Value::Bool(force_update));
    let release_notes = release_notes.trim();
    if !release_notes.is_empty() {
        payload.insert(
            "release_notes".to_string(),
            Value::String(release_notes.to_string()),
        );
    }
    let check_reason = check_reason.trim();
    if !check_reason.is_empty() {
        payload.insert(
            "check_reason".to_string(),
            Value::String(check_reason.to_string()),
        );
    }

    InternalSystemEventBody {
        event_type: "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("INFO".to_string()),
        happened_at: None,
        dedupe_key: Some(format!("system:app-update:{latest_version}")),
        dedupe_ttl_seconds: Some(APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS as f64),
        subject: Some(json!({ "kind": "APPLICATION", "id": "fn-knock" })),
        tags: None,
        payload: Value::Object(payload),
    }
}

pub async fn publish_auth_login_success_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    publish_system_event_body(state, auth_login_success_body(payload)).await
}

fn auth_login_success_body(payload: Value) -> InternalSystemEventBody {
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    InternalSystemEventBody {
        event_type: "FN_EVENT_AUTH_LOGIN_SUCCESS".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("INFO".to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "SESSION", "id": session_id })),
        tags: None,
        payload: omit_js_falsy_object_fields(
            payload,
            &[
                "auth_provider_name",
                "linked_totp_name",
                "session_comment",
                "ip_location",
            ],
        ),
    }
}

pub async fn publish_auth_logout_event(state: &AppState, payload: Value) -> anyhow::Result<bool> {
    publish_system_event_body(state, auth_logout_body(payload)).await
}

fn auth_logout_body(payload: Value) -> InternalSystemEventBody {
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    InternalSystemEventBody {
        event_type: "FN_EVENT_AUTH_LOGOUT".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("INFO".to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "SESSION", "id": session_id })),
        tags: None,
        payload: omit_js_falsy_object_fields(
            payload,
            &[
                "linked_totp_name",
                "session_comment",
                "ip_location",
                "login_time",
            ],
        ),
    }
}

pub async fn publish_auth_login_failure_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    publish_system_event_body(state, auth_login_failure_body(payload)).await
}

fn auth_login_failure_body(payload: Value) -> InternalSystemEventBody {
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    InternalSystemEventBody {
        event_type: "FN_EVENT_AUTH_LOGIN_FAILURE".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: None,
        payload: omit_js_falsy_object_fields(
            payload,
            &[
                "blocked_until",
                "method",
                "credential_name",
                "linked_totp_name",
                "user_agent",
            ],
        ),
    }
}

pub async fn publish_auth_session_ip_drift_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    publish_system_event_body(state, auth_session_ip_drift_body(payload)).await
}

fn auth_session_ip_drift_body(payload: Value) -> InternalSystemEventBody {
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    InternalSystemEventBody {
        event_type: "FN_EVENT_AUTH_SESSION_IP_DRIFT".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "SESSION", "id": session_id })),
        tags: None,
        payload: omit_js_falsy_object_fields(
            payload,
            &[
                "linked_totp_name",
                "session_comment",
                "from_ip_location",
                "to_ip_location",
                "login_time",
            ],
        ),
    }
}

pub async fn publish_tunnel_connectivity_event(
    state: &AppState,
    tunnel: &str,
    connected: bool,
    pid: Option<u32>,
    message: Option<&str>,
    instance_id: Option<&str>,
    instance_name: Option<&str>,
    is_primary: Option<bool>,
) -> anyhow::Result<bool> {
    publish_system_event_body(
        state,
        tunnel_connectivity_body(
            tunnel,
            connected,
            pid,
            message,
            instance_id,
            instance_name,
            is_primary,
        ),
    )
    .await
}

fn tunnel_connectivity_body(
    tunnel: &str,
    connected: bool,
    pid: Option<u32>,
    message: Option<&str>,
    instance_id: Option<&str>,
    instance_name: Option<&str>,
    is_primary: Option<bool>,
) -> InternalSystemEventBody {
    let tunnel = if tunnel == "frp" {
        "frp"
    } else {
        "cloudflared"
    };
    let event_type = match (tunnel, connected) {
        ("frp", true) => "FN_EVENT_TUNNEL_FRP_CONNECTED",
        ("frp", false) => "FN_EVENT_TUNNEL_FRP_DISCONNECTED",
        ("cloudflared", true) => "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED",
        _ => "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED",
    };
    let mut payload = Map::new();
    payload.insert("tunnel".to_string(), Value::String(tunnel.to_string()));
    payload.insert(
        "status".to_string(),
        Value::String(if connected {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        }),
    );
    if let Some(pid) = pid.filter(|value| *value > 0) {
        payload.insert("pid".to_string(), Value::Number(pid.into()));
    }
    if let Some(value) = instance_id.map(str::trim).filter(|value| !value.is_empty()) {
        payload.insert("instance_id".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = instance_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload.insert(
            "instance_name".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = is_primary {
        payload.insert("is_primary".to_string(), Value::Bool(value));
    }
    if let Some(value) = message.map(str::trim).filter(|value| !value.is_empty()) {
        payload.insert("message".to_string(), Value::String(value.to_string()));
    }

    let subject_id = if tunnel == "frp" {
        instance_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("frp:{value}"))
            .unwrap_or_else(|| tunnel.to_string())
    } else {
        tunnel.to_string()
    };

    InternalSystemEventBody {
        event_type: event_type.to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some(if connected { "INFO" } else { "ERROR" }.to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "TUNNEL", "id": subject_id })),
        tags: None,
        payload: Value::Object(payload),
    }
}

pub async fn publish_scanner_blocked_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let body = InternalSystemEventBody {
        event_type: "FN_EVENT_SECURITY_SCANNER_BLOCKED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: None,
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: None,
        payload,
    };
    publish_system_event_body(state, body).await
}

pub async fn publish_ssh_login_success_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let body = InternalSystemEventBody {
        event_type: "FN_EVENT_SSH_LOGIN_SUCCESS".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("INFO".to_string()),
        happened_at: payload
            .get("log_time")
            .and_then(Value::as_str)
            .map(str::to_string),
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: Some(vec!["ssh".to_string(), "login".to_string()]),
        payload: without_null_object_fields(payload),
    };
    publish_system_event_body(state, body).await
}

pub async fn publish_ssh_login_failure_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let body = InternalSystemEventBody {
        event_type: "FN_EVENT_SSH_LOGIN_FAILURE".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: payload
            .get("log_time")
            .and_then(Value::as_str)
            .map(str::to_string),
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: Some(vec!["ssh".to_string(), "login".to_string()]),
        payload: without_null_object_fields(payload),
    };
    publish_system_event_body(state, body).await
}

pub async fn publish_ssh_ip_blocked_event(
    state: &AppState,
    payload: Value,
) -> anyhow::Result<bool> {
    let ip = payload
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let body = InternalSystemEventBody {
        event_type: "FN_EVENT_SSH_IP_BLOCKED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some("WARN".to_string()),
        happened_at: payload
            .get("blocked_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: Some(json!({ "kind": "IP", "id": ip })),
        tags: Some(vec!["ssh".to_string(), "firewall".to_string()]),
        payload: without_null_object_fields(payload),
    };
    publish_system_event_body(state, body).await
}

pub async fn publish_resource_alert_event(
    state: &AppState,
    metric: &str,
    hostname: &str,
    recovered: bool,
    dedupe_key: String,
    dedupe_ttl_seconds: i64,
    payload: Value,
) -> anyhow::Result<bool> {
    let event_type = match (metric, recovered) {
        ("cpu", true) => "FN_EVENT_SYSTEM_CPU_RECOVERED",
        ("cpu", false) => "FN_EVENT_SYSTEM_CPU_ALERT",
        ("memory", true) => "FN_EVENT_SYSTEM_MEMORY_RECOVERED",
        _ => "FN_EVENT_SYSTEM_MEMORY_ALERT",
    };
    let body = InternalSystemEventBody {
        event_type: event_type.to_string(),
        source: "SYSTEM_MONITOR".to_string(),
        level: Some(if recovered { "INFO" } else { "WARN" }.to_string()),
        happened_at: None,
        dedupe_key: Some(dedupe_key),
        dedupe_ttl_seconds: Some(dedupe_ttl_seconds as f64),
        subject: Some(json!({ "kind": "RESOURCE", "id": format!("{hostname}:{metric}") })),
        tags: None,
        payload,
    };
    publish_system_event_body(state, body).await
}

fn without_null_object_fields(value: Value) -> Value {
    let Value::Object(mut object) = value else {
        return value;
    };
    object.retain(|_, value| !value.is_null());
    Value::Object(object)
}

fn omit_js_falsy_object_fields(value: Value, fields: &[&str]) -> Value {
    let Value::Object(mut object) = value else {
        return value;
    };
    for field in fields {
        if object.get(*field).is_some_and(is_js_falsy_json_value) {
            object.remove(*field);
        }
    }
    Value::Object(object)
}

fn is_js_falsy_json_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(value) => !*value,
        Value::Number(number) => {
            number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
        }
        Value::String(value) => value.is_empty(),
        Value::Array(_) | Value::Object(_) => false,
    }
}

async fn publish_system_event_body(
    state: &AppState,
    body: InternalSystemEventBody,
) -> anyhow::Result<bool> {
    let subject = normalize_subject(body.subject.clone())
        .map_err(|key| anyhow::anyhow!(system_event_route_text(&Translator::new("zh-CN"), key)))?;
    let event_config = load_event_system_config(state).await?;
    if !event_config.enabled || !is_event_type_enabled(&event_config, &body.event_type) {
        return Ok(false);
    }

    let dedupe_key = body
        .dedupe_key
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned();
    let dedupe_ttl_seconds = normalize_dedupe_ttl_seconds(body.dedupe_ttl_seconds);
    let acquired_dedupe = if let Some(key) = dedupe_key.as_deref() {
        dedupe_ttl_seconds > 0
            && state
                .redis
                .acquire_system_event_dedupe(key, dedupe_ttl_seconds)
                .await?
    } else {
        false
    };
    if dedupe_key.is_some() && dedupe_ttl_seconds > 0 && !acquired_dedupe {
        return Ok(false);
    }

    let event = build_event_envelope(body, subject, dedupe_key);
    if let Err(error) = state
        .redis
        .append_system_event(&event, event_config.retention_days)
        .await
    {
        if acquired_dedupe {
            if let Some(key) = event.get("dedupe_key").and_then(Value::as_str) {
                let _ = state.redis.release_system_event_dedupe(key).await;
            }
        }
        return Err(error.into());
    }
    Ok(true)
}

async fn publish_internal_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<InternalSystemEventBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if has_forbidden_internal_event_headers(&headers) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": translator.t("server.notFound") })),
        )
            .into_response();
    }

    if !is_allowed(SYSTEM_EVENT_TYPES, &body.event_type) {
        return response::error(
            StatusCode::BAD_REQUEST,
            system_event_route_text(&translator, "unsupportedSystemEventType"),
        );
    }
    if !is_allowed(SYSTEM_EVENT_SOURCES, &body.source) {
        return response::error(
            StatusCode::BAD_REQUEST,
            system_event_route_text(&translator, "unsupportedSystemEventSource"),
        );
    }
    if let Some(level) = body.level.as_deref().filter(|value| !value.is_empty()) {
        if !is_allowed(SYSTEM_EVENT_LEVELS, level) {
            return response::error(
                StatusCode::BAD_REQUEST,
                system_event_route_text(&translator, "unsupportedSystemEventLevel"),
            );
        }
    }
    let subject = match normalize_subject(body.subject.clone()) {
        Ok(subject) => subject,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                system_event_route_text(&translator, key),
            );
        }
    };
    apply_internal_event_route_truthiness(&mut body);

    let event_config = match load_event_system_config(&state).await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load event system config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_event_route_text(&translator, "loadConfigFailed"),
            );
        }
    };
    if !event_config.enabled || !is_event_type_enabled(&event_config, &body.event_type) {
        return Json(json!({ "success": true, "skipped": true, "data": Value::Null }))
            .into_response();
    }

    let dedupe_key = body
        .dedupe_key
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned();
    let dedupe_ttl_seconds = normalize_dedupe_ttl_seconds(body.dedupe_ttl_seconds);
    let acquired_dedupe = if let Some(key) = dedupe_key.as_deref() {
        if dedupe_ttl_seconds > 0 {
            match state
                .redis
                .acquire_system_event_dedupe(key, dedupe_ttl_seconds)
                .await
            {
                Ok(true) => true,
                Ok(false) => {
                    return Json(json!({ "success": true, "skipped": true, "data": Value::Null }))
                        .into_response();
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to acquire system event dedupe key");
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        system_event_route_text(&translator, "writeEventFailed"),
                    );
                }
            }
        } else {
            false
        }
    } else {
        false
    };

    let event = build_event_envelope(body, subject, dedupe_key);
    match state
        .redis
        .append_system_event(&event, event_config.retention_days)
        .await
    {
        Ok(()) => {
            let mut event = event;
            hydrate_system_event_ip_locations(&state, std::slice::from_mut(&mut event)).await;
            Json(json!({ "success": true, "skipped": false, "data": event })).into_response()
        }
        Err(error) => {
            if acquired_dedupe {
                if let Some(key) = event.get("dedupe_key").and_then(Value::as_str) {
                    let _ = state.redis.release_system_event_dedupe(key).await;
                }
            }
            tracing::warn!(%error, "failed to append system event");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_event_route_text(&translator, "writeEventFailed"),
            )
        }
    }
}

async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventListQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let event_type = match normalize_optional_filter(
        query.event_type,
        SYSTEM_EVENT_TYPES,
        "unsupportedEventType",
    ) {
        Ok(value) => value,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                system_event_route_text(&translator, key),
            );
        }
    };
    let level = match normalize_optional_filter(
        query.level,
        SYSTEM_EVENT_LEVELS,
        "unsupportedEventLevel",
    ) {
        Ok(value) => value,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                system_event_route_text(&translator, key),
            );
        }
    };
    let source = match normalize_optional_filter(
        query.source,
        SYSTEM_EVENT_SOURCES,
        "unsupportedEventSource",
    ) {
        Ok(value) => value,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                system_event_route_text(&translator, key),
            );
        }
    };

    match state
        .redis
        .list_system_events(
            parse_positive_int(query.page.as_deref(), 1),
            parse_positive_int(query.limit.as_deref(), 20).min(100),
            query.search.as_deref().unwrap_or(""),
            event_type.as_deref(),
            level.as_deref(),
            source.as_deref(),
        )
        .await
    {
        Ok(mut result) => {
            if let Some(events) = result.get_mut("events").and_then(Value::as_array_mut) {
                hydrate_system_event_ip_locations(&state, events).await;
            }
            response::ok(result).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list system events");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_event_route_text(&translator, "listEventsFailed"),
            )
        }
    }
}

async fn delete_events(
    State(state): State<AppState>,
    Json(body): Json<DeleteEventsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.delete_system_events(&body.ids).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete system events");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_event_route_text(&translator, "deleteEventsFailed"),
            )
        }
    }
}

async fn clear_events(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.clear_system_events().await {
        Ok(deleted_count) => {
            response::ok(json!({ "deleted_count": deleted_count })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to clear system events");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_event_route_text(&translator, "clearEventsFailed"),
            )
        }
    }
}

fn build_event_envelope(
    body: InternalSystemEventBody,
    subject: Option<Value>,
    dedupe_key: Option<String>,
) -> Value {
    let mut event = Map::new();
    event.insert(
        "id".to_string(),
        Value::String(format!("evt_{}", hex::encode(rand::random::<[u8; 12]>()))),
    );
    event.insert("type".to_string(), Value::String(body.event_type.clone()));
    event.insert("source".to_string(), Value::String(body.source));
    event.insert(
        "level".to_string(),
        Value::String(
            body.level
                .unwrap_or_else(|| default_event_level(&body.event_type).to_string()),
        ),
    );
    event.insert(
        "happened_at".to_string(),
        Value::String(body.happened_at.unwrap_or_else(time_utils::now_iso)),
    );
    if let Some(dedupe_key) = dedupe_key {
        event.insert("dedupe_key".to_string(), Value::String(dedupe_key));
    }
    if let Some(subject) = subject {
        event.insert("subject".to_string(), subject);
    }
    let tags = body
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(Value::String)
        .collect::<Vec<_>>();
    if !tags.is_empty() {
        event.insert("tags".to_string(), Value::Array(tags));
    }
    event.insert("payload".to_string(), body.payload);
    Value::Object(event)
}

fn has_forbidden_internal_event_headers(headers: &HeaderMap) -> bool {
    headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.trim().is_empty())
        || header_has_node_truthy_value(headers, "x-forwarded-host")
        || header_has_node_truthy_value(headers, "x-forwarded-for")
        || header_has_node_truthy_value(headers, "x-real-ip")
        || header_has_node_truthy_value(headers, "x-forwarded-proto")
}

fn header_has_node_truthy_value(headers: &HeaderMap, name: &'static str) -> bool {
    headers.get(name).is_some_and(|value| {
        value
            .to_str()
            .map(|value| !value.is_empty())
            .unwrap_or(true)
    })
}

fn apply_internal_event_route_truthiness(body: &mut InternalSystemEventBody) {
    if body.level.as_deref() == Some("") {
        body.level = None;
    }
    if body.happened_at.as_deref() == Some("") {
        body.happened_at = None;
    }
    if body.dedupe_key.as_deref() == Some("") {
        body.dedupe_key = None;
    }
}

fn normalize_dedupe_ttl_seconds(value: Option<f64>) -> i64 {
    value
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.ceil() as i64)
        .unwrap_or_default()
}

fn normalize_subject(value: Option<Value>) -> Result<Option<Value>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Err("unsupportedSubjectKind");
    };
    let kind = object.get("kind").and_then(Value::as_str).unwrap_or("");
    if !is_allowed(SYSTEM_EVENT_SUBJECT_KINDS, kind) {
        return Err("unsupportedSubjectKind");
    }
    let id = object.get("id").and_then(Value::as_str).unwrap_or("");
    Ok(Some(json!({ "kind": kind, "id": id })))
}

#[derive(Debug)]
struct EventSystemConfig {
    enabled: bool,
    retention_days: i64,
    rules: Map<String, Value>,
}

async fn load_event_system_config(state: &AppState) -> redis::RedisResult<EventSystemConfig> {
    let config = state.redis.get_config().await?;
    let event_system = config
        .get("event_system")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let rules = event_system
        .get("rules")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(EventSystemConfig {
        enabled: event_system
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        retention_days: event_system
            .get("retention_days")
            .and_then(Value::as_i64)
            .unwrap_or(30)
            .clamp(1, 90),
        rules,
    })
}

fn is_event_type_enabled(config: &EventSystemConfig, event_type: &str) -> bool {
    if matches!(
        event_type,
        "FN_EVENT_AUTH_LOGIN_SUCCESS" | "FN_EVENT_AUTH_LOGOUT"
    ) {
        return true;
    }
    let Some(rule_key) = event_rule_key(event_type) else {
        return false;
    };
    config
        .rules
        .get(rule_key)
        .and_then(Value::as_object)
        .and_then(|rule| rule.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn event_rule_key(event_type: &str) -> Option<&'static str> {
    match event_type {
        "FN_EVENT_AUTH_LOGIN_FAILURE" => Some("login_failure"),
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => Some("ip_drift"),
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => Some("scanner_blocked"),
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => Some("ddns_update"),
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => Some("gateway_throttle_block"),
        "FN_EVENT_WAF_BLOCKED" => Some("waf_blocked"),
        "FN_EVENT_SSH_LOGIN_SUCCESS" => Some("ssh_login_success"),
        "FN_EVENT_SSH_LOGIN_FAILURE" => Some("ssh_login_failure"),
        "FN_EVENT_SSH_IP_BLOCKED" => Some("ssh_ip_blocked"),
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => Some("app_update_available"),
        "FN_EVENT_SYSTEM_CPU_ALERT" | "FN_EVENT_SYSTEM_CPU_RECOVERED" => Some("cpu_alert"),
        "FN_EVENT_SYSTEM_MEMORY_ALERT" | "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => Some("memory_alert"),
        "FN_EVENT_TUNNEL_FRP_CONNECTED" | "FN_EVENT_TUNNEL_FRP_DISCONNECTED" => Some("frp_tunnel"),
        "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED" | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => {
            Some("cloudflared_tunnel")
        }
        _ => None,
    }
}

fn default_event_level(event_type: &str) -> &'static str {
    match event_type {
        "FN_EVENT_AUTH_LOGIN_SUCCESS"
        | "FN_EVENT_AUTH_LOGOUT"
        | "FN_EVENT_DDNS_UPDATE_COMPLETED"
        | "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"
        | "FN_EVENT_SYSTEM_CPU_RECOVERED"
        | "FN_EVENT_SYSTEM_MEMORY_RECOVERED"
        | "FN_EVENT_TUNNEL_FRP_CONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
        | "FN_EVENT_SSH_LOGIN_SUCCESS" => "INFO",
        "FN_EVENT_SYSTEM_CPU_ALERT"
        | "FN_EVENT_SYSTEM_MEMORY_ALERT"
        | "FN_EVENT_AUTH_LOGIN_FAILURE"
        | "FN_EVENT_AUTH_SESSION_IP_DRIFT"
        | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
        | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
        | "FN_EVENT_WAF_BLOCKED"
        | "FN_EVENT_SSH_LOGIN_FAILURE"
        | "FN_EVENT_SSH_IP_BLOCKED" => "WARN",
        "FN_EVENT_TUNNEL_FRP_DISCONNECTED" | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => "WARN",
        _ => "ERROR",
    }
}

fn normalize_optional_filter(
    value: Option<String>,
    allowed: &[&str],
    error_key: &'static str,
) -> Result<Option<String>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    if is_allowed(allowed, normalized) {
        return Ok(Some(normalized.to_string()));
    }
    Err(error_key)
}

fn parse_positive_int(value: Option<&str>, fallback: i64) -> i64 {
    value
        .and_then(|value| parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn parse_i64_prefix(value: &str) -> Option<i64> {
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }
    let mut end = 0;
    let mut has_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }
    value[..end].parse::<i64>().ok()
}

async fn hydrate_system_event_ip_locations(state: &AppState, events: &mut [Value]) {
    if events.is_empty() {
        return;
    }

    let mut refs_by_ip = std::collections::BTreeMap::<String, Vec<String>>::new();
    for event in events.iter() {
        let id = event.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        for &(ip_key, _) in system_event_ip_fields(event.get("type").and_then(Value::as_str)) {
            let raw_ip = event
                .get("payload")
                .and_then(Value::as_object)
                .and_then(|payload| payload.get(ip_key))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if raw_ip.is_empty() {
                continue;
            }
            let normalized_ip = normalize_ip(raw_ip);
            if normalized_ip.is_empty() {
                continue;
            }
            let refs = refs_by_ip.entry(normalized_ip).or_default();
            let reference = format!("system-event|{id}");
            if !refs.iter().any(|item| item == &reference) {
                refs.push(reference);
            }
        }
    }

    let mut locations = std::collections::BTreeMap::<String, String>::new();
    for (ip, refs) in refs_by_ip {
        match ip_location::register_usage(state, &ip, refs).await {
            Ok(location) if !location.trim().is_empty() => {
                locations.insert(ip, location);
            }
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, ip, "failed to hydrate system event IP location"),
        }
    }
    if locations.is_empty() {
        return;
    }

    for event in events {
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let Some(payload) = event.get_mut("payload").and_then(Value::as_object_mut) else {
            continue;
        };
        for &(ip_key, location_key) in system_event_ip_fields(Some(&event_type)) {
            if payload
                .get(location_key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                let ip = payload.get(ip_key).and_then(Value::as_str).unwrap_or("");
                let normalized_ip = normalize_ip(ip);
                if let Some(location) = locations.get(&normalized_ip) {
                    payload.insert(location_key.to_string(), Value::String(location.clone()));
                }
            }
        }
    }
}

fn system_event_ip_fields(event_type: Option<&str>) -> &'static [(&'static str, &'static str)] {
    match event_type.unwrap_or("") {
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => {
            &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
        }
        "FN_EVENT_AUTH_LOGIN_SUCCESS"
        | "FN_EVENT_AUTH_LOGOUT"
        | "FN_EVENT_AUTH_LOGIN_FAILURE"
        | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
        | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
        | "FN_EVENT_WAF_BLOCKED"
        | "FN_EVENT_SSH_LOGIN_SUCCESS"
        | "FN_EVENT_SSH_LOGIN_FAILURE"
        | "FN_EVENT_SSH_IP_BLOCKED" => &[("ip", "ip_location")],
        _ => &[],
    }
}

fn is_allowed(allowed: &[&str], value: &str) -> bool {
    allowed.contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_event_rules_and_default_levels() {
        assert_eq!(
            event_rule_key("FN_EVENT_GATEWAY_THROTTLE_BLOCKED"),
            Some("gateway_throttle_block")
        );
        assert_eq!(default_event_level("FN_EVENT_AUTH_LOGIN_SUCCESS"), "INFO");
        assert_eq!(default_event_level("FN_EVENT_WAF_BLOCKED"), "WARN");
        assert_eq!(
            default_event_level("FN_EVENT_TUNNEL_FRP_DISCONNECTED"),
            "WARN"
        );
    }

    #[test]
    fn internal_header_guard_matches_node_truthiness() {
        let mut headers = HeaderMap::new();
        assert!(!has_forbidden_internal_event_headers(&headers));

        headers.insert("x-forwarded-for", axum::http::HeaderValue::from_static(""));
        assert!(!has_forbidden_internal_event_headers(&headers));

        headers.insert("x-forwarded-for", axum::http::HeaderValue::from_static(" "));
        assert!(has_forbidden_internal_event_headers(&headers));

        headers.clear();
        headers.insert("origin", axum::http::HeaderValue::from_static("   "));
        assert!(!has_forbidden_internal_event_headers(&headers));

        headers.insert(
            "origin",
            axum::http::HeaderValue::from_static("https://example.com"),
        );
        assert!(has_forbidden_internal_event_headers(&headers));
    }

    #[test]
    fn validates_subject_kind() {
        assert!(normalize_subject(Some(json!({ "kind": "IP", "id": "1.2.3.4" }))).is_ok());
        assert!(normalize_subject(Some(json!({ "kind": "NOPE", "id": "x" }))).is_err());
        assert!(normalize_subject(Some(json!({ "kind": " IP ", "id": "x" }))).is_err());
        assert_eq!(
            normalize_subject(Some(json!({ "kind": "IP", "id": " 1.2.3.4 " }))).unwrap(),
            Some(json!({ "kind": "IP", "id": " 1.2.3.4 " }))
        );
    }

    #[test]
    fn event_list_page_parser_matches_node_parse_int_edges() {
        assert_eq!(parse_positive_int(None, 1), 1);
        assert_eq!(parse_positive_int(Some("2x"), 1), 2);
        assert_eq!(parse_positive_int(Some("  +3.9"), 1), 3);
        assert_eq!(parse_positive_int(Some("-1"), 1), 1);
        assert_eq!(parse_positive_int(Some(""), 20), 20);
    }

    #[test]
    fn system_event_ip_field_mapping_matches_node_hydration() {
        assert_eq!(
            system_event_ip_fields(Some("FN_EVENT_AUTH_SESSION_IP_DRIFT")),
            &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
        );
        assert_eq!(
            system_event_ip_fields(Some("FN_EVENT_WAF_BLOCKED")),
            &[("ip", "ip_location")]
        );
        assert!(system_event_ip_fields(Some("FN_EVENT_DDNS_UPDATE_COMPLETED")).is_empty());
    }

    #[test]
    fn builds_event_envelope_with_node_manager_nullish_semantics() {
        let body = InternalSystemEventBody {
            event_type: "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED".to_string(),
            source: "SERVER_ADMIN".to_string(),
            level: Some(String::new()),
            happened_at: Some("   ".to_string()),
            dedupe_key: None,
            dedupe_ttl_seconds: None,
            subject: None,
            tags: Some(vec![String::new(), " tag ".to_string()]),
            payload: json!({ "message": "edge" }),
        };

        let event = build_event_envelope(body, None, Some("  dedupe  ".to_string()));

        assert_eq!(event.get("level"), Some(&json!("")));
        assert_eq!(event.get("happened_at"), Some(&json!("   ")));
        assert_eq!(event.get("dedupe_key"), Some(&json!("  dedupe  ")));
        assert_eq!(event.get("tags"), Some(&json!(["", " tag "])));
    }

    #[test]
    fn applies_internal_route_truthiness_before_manager_publish() {
        let mut body = InternalSystemEventBody {
            event_type: "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED".to_string(),
            source: "SERVER_ADMIN".to_string(),
            level: Some(String::new()),
            happened_at: Some(String::new()),
            dedupe_key: Some(String::new()),
            dedupe_ttl_seconds: Some(60.0),
            subject: None,
            tags: Some(vec![String::new()]),
            payload: json!({}),
        };

        apply_internal_event_route_truthiness(&mut body);
        let event = build_event_envelope(body, None, None);

        assert_eq!(event.get("level"), Some(&json!("WARN")));
        assert_ne!(event.get("happened_at"), Some(&json!("")));
        assert!(event.get("dedupe_key").is_none());
        assert_eq!(event.get("tags"), Some(&json!([""])));
    }

    #[test]
    fn honors_event_rule_defaults() {
        let config = EventSystemConfig {
            enabled: true,
            retention_days: 30,
            rules: Map::new(),
        };
        assert!(is_event_type_enabled(
            &config,
            "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
        ));
        assert!(is_event_type_enabled(&config, "FN_EVENT_AUTH_LOGOUT"));
    }

    #[test]
    fn builds_app_update_available_event_like_node() {
        let body = app_update_available_body("1.8.6", "1.9.0", true, "Release notes", "startup");

        assert_eq!(body.event_type, "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE");
        assert_eq!(body.source, "SERVER_ADMIN");
        assert_eq!(body.level.as_deref(), Some("INFO"));
        assert_eq!(body.dedupe_key.as_deref(), Some("system:app-update:1.9.0"));
        assert_eq!(
            body.dedupe_ttl_seconds,
            Some(APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS as f64)
        );
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "APPLICATION", "id": "fn-knock" }))
        );
        assert_eq!(body.payload.get("local_version"), Some(&json!("1.8.6")));
        assert_eq!(body.payload.get("latest_version"), Some(&json!("1.9.0")));
        assert_eq!(body.payload.get("force_update"), Some(&json!(true)));
        assert_eq!(
            body.payload.get("release_notes"),
            Some(&json!("Release notes"))
        );
        assert_eq!(body.payload.get("check_reason"), Some(&json!("startup")));

        let body = app_update_available_body("1.8.6", "1.9.0", false, "  ", "");
        assert!(body.payload.get("release_notes").is_none());
        assert!(body.payload.get("check_reason").is_none());
    }

    #[test]
    fn dedupe_ttl_seconds_matches_node_number_ceiling() {
        assert_eq!(normalize_dedupe_ttl_seconds(Some(1.2)), 2);
        assert_eq!(normalize_dedupe_ttl_seconds(Some(1.0)), 1);
        assert_eq!(normalize_dedupe_ttl_seconds(Some(0.0)), 0);
        assert_eq!(normalize_dedupe_ttl_seconds(Some(f64::NAN)), 0);
        assert_eq!(normalize_dedupe_ttl_seconds(None), 0);
    }

    #[test]
    fn builds_auth_login_failure_event_like_node() {
        let body = auth_login_failure_body(json!({
            "ip": "203.0.113.10",
            "attempts": 3,
            "retry_after_seconds": 8,
            "blocked_until": "2026-01-01T00:00:00Z",
            "method": "PASSKEY",
            "credential_name": "MacBook",
            "linked_totp_name": Value::Null,
            "user_agent": "Browser",
        }));

        assert_eq!(body.event_type, "FN_EVENT_AUTH_LOGIN_FAILURE");
        assert_eq!(body.source, "SERVER_ADMIN");
        assert_eq!(body.level.as_deref(), Some("WARN"));
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "IP", "id": "203.0.113.10" }))
        );
        assert_eq!(body.payload.get("attempts"), Some(&json!(3)));
        assert_eq!(body.payload.get("retry_after_seconds"), Some(&json!(8)));
        assert_eq!(body.payload.get("method"), Some(&json!("PASSKEY")));
        assert_eq!(body.payload.get("credential_name"), Some(&json!("MacBook")));
        assert!(body.payload.get("linked_totp_name").is_none());
    }

    #[test]
    fn builds_auth_login_success_event_like_node_optional_payload_shape() {
        let body = auth_login_success_body(json!({
            "session_id": " session-1 ",
            "auth_method": "TOTP",
            "auth_provider_name": "",
            "credential_id": "cred-1",
            "credential_name": "Token",
            "linked_totp_name": Value::Null,
            "session_comment": "",
            "grant_type": "browser_session",
            "post_login_ip_grant_mode": Value::Null,
            "whitelist_record_id": Value::Null,
            "ip": "203.0.113.10",
            "ip_location": "",
            "user_agent": "Browser",
            "remember_me": false,
            "expires_at": "2026-01-01T00:00:00Z",
        }));

        assert_eq!(body.event_type, "FN_EVENT_AUTH_LOGIN_SUCCESS");
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "SESSION", "id": "session-1" }))
        );
        assert!(body.payload.get("auth_provider_name").is_none());
        assert!(body.payload.get("linked_totp_name").is_none());
        assert!(body.payload.get("session_comment").is_none());
        assert!(body.payload.get("ip_location").is_none());
        assert_eq!(
            body.payload.get("post_login_ip_grant_mode"),
            Some(&Value::Null)
        );
        assert_eq!(body.payload.get("whitelist_record_id"), Some(&Value::Null));
        assert_eq!(body.payload.get("remember_me"), Some(&json!(false)));
    }

    #[test]
    fn builds_auth_logout_and_drift_events_like_node_optional_payload_shape() {
        let logout = auth_logout_body(json!({
            "session_id": "session-1",
            "auth_method": "TOTP",
            "credential_id": "cred-1",
            "credential_name": "Token",
            "linked_totp_name": "",
            "session_comment": Value::Null,
            "ip": "203.0.113.10",
            "ip_location": "",
            "user_agent": "Browser",
            "login_time": "",
            "logout_source": "admin_session_delete",
        }));
        assert!(logout.payload.get("linked_totp_name").is_none());
        assert!(logout.payload.get("session_comment").is_none());
        assert!(logout.payload.get("ip_location").is_none());
        assert!(logout.payload.get("login_time").is_none());

        let drift = auth_session_ip_drift_body(json!({
            "session_id": "session-1",
            "auth_method": "TOTP",
            "credential_id": "cred-1",
            "credential_name": "Token",
            "linked_totp_name": "",
            "session_comment": "",
            "drift_source": "proxy-session",
            "from_ip": "203.0.113.10",
            "from_ip_location": "",
            "to_ip": "203.0.113.11",
            "to_ip_location": Value::Null,
            "login_time": "",
        }));
        assert!(drift.payload.get("linked_totp_name").is_none());
        assert!(drift.payload.get("session_comment").is_none());
        assert!(drift.payload.get("from_ip_location").is_none());
        assert!(drift.payload.get("to_ip_location").is_none());
        assert!(drift.payload.get("login_time").is_none());
    }

    #[test]
    fn builds_waf_blocked_event_like_node_helper_truthiness() {
        let body = waf_blocked_body(&json!({
            "trace_id": " trace-1 ",
            "client_ip": "",
            "remote_addr": " 203.0.113.9 ",
            "mode": "block",
            "action": "",
            "status": 0,
            "host": "   ",
            "path": "",
            "rule_ids": [1001, "skip"]
        }))
        .unwrap();

        assert_eq!(body.event_type, "FN_EVENT_WAF_BLOCKED");
        assert_eq!(body.level.as_deref(), Some("WARN"));
        assert_eq!(body.happened_at, None);
        assert_eq!(body.dedupe_key.as_deref(), Some("waf: trace-1 "));
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "IP", "id": " 203.0.113.9 " }))
        );
        assert_eq!(body.payload.get("ip"), Some(&json!(" 203.0.113.9 ")));
        assert_eq!(body.payload.get("trace_id"), Some(&json!(" trace-1 ")));
        assert_eq!(body.payload.get("action"), Some(&json!("deny")));
        assert_eq!(body.payload.get("host"), Some(&json!("   ")));
        assert!(body.payload.get("path").is_none());
        assert!(body.payload.get("status").is_none());
        assert!(body.payload.get("blocked_at").is_none());
        assert_eq!(body.payload.get("rule_ids"), Some(&json!([1001])));
    }

    #[test]
    fn builds_tunnel_connectivity_event_like_node() {
        let body = tunnel_connectivity_body(
            "frp",
            false,
            Some(1234),
            Some("Primary: session shutdown"),
            Some("primary"),
            Some("Primary"),
            Some(true),
        );

        assert_eq!(body.event_type, "FN_EVENT_TUNNEL_FRP_DISCONNECTED");
        assert_eq!(body.source, "SERVER_ADMIN");
        assert_eq!(body.level.as_deref(), Some("ERROR"));
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "TUNNEL", "id": "frp:primary" }))
        );
        assert_eq!(body.payload.get("tunnel"), Some(&json!("frp")));
        assert_eq!(body.payload.get("status"), Some(&json!("disconnected")));
        assert_eq!(body.payload.get("pid"), Some(&json!(1234)));
        assert_eq!(body.payload.get("instance_id"), Some(&json!("primary")));
        assert_eq!(body.payload.get("instance_name"), Some(&json!("Primary")));
        assert_eq!(body.payload.get("is_primary"), Some(&json!(true)));
        assert_eq!(
            body.payload.get("message"),
            Some(&json!("Primary: session shutdown"))
        );

        let body = tunnel_connectivity_body("cloudflared", true, None, None, None, None, None);
        assert_eq!(body.event_type, "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED");
        assert_eq!(body.level.as_deref(), Some("INFO"));
        assert_eq!(
            body.subject,
            Some(json!({ "kind": "TUNNEL", "id": "cloudflared" }))
        );
        assert!(body.payload.get("pid").is_none());
    }

    #[test]
    fn localizes_system_event_route_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            system_event_route_text(&zh, "unsupportedEventLevel"),
            "不支持的事件级别"
        );
        assert_eq!(
            system_event_route_text(&zh, "clearEventsFailed"),
            "清空系统事件失败"
        );
    }
}
