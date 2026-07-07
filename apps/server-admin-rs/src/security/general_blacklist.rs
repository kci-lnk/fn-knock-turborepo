use axum::{
    Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{http_utils::normalize_ip, i18n::Translator, response, state::AppState};

const GENERAL_BLACKLIST_SOURCES: &[&str] = &["manual", "request_log", "active_ip", "waf_log"];

fn general_blacklist_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.generalBlacklist.{key}"))
}

fn general_blacklist_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.generalBlacklist.{key}"), params)
}

fn general_blacklist_error_response(
    translator: &Translator,
    status: StatusCode,
    message: impl AsRef<str>,
) -> Response {
    response::error(
        status,
        localize_general_blacklist_error(translator, message.as_ref()),
    )
}

fn localize_general_blacklist_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    match message {
        "Invalid request body" => general_blacklist_text(translator, "invalidRequestBody"),
        "Invalid IP" => general_blacklist_text(translator, "invalidIp"),
        "At least one valid IP is required" => {
            general_blacklist_text(translator, "atLeastOneValidIpRequired")
        }
        "Go backend request failed" => general_blacklist_text(translator, "backendRequestFailed"),
        "Go backend response missing data" => {
            general_blacklist_text(translator, "backendResponseMissingData")
        }
        _ => {
            if let Some(ip) = message.strip_prefix("Invalid IP: ") {
                return general_blacklist_text_params(
                    translator,
                    "invalidIpWithValue",
                    &[("ip", ip.trim().to_string())],
                );
            }
            message.to_string()
        }
    }
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<String>,
    limit: Option<String>,
    search: Option<String>,
}

pub fn general_blacklist_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/general-blacklist",
            get(list).post(add).delete(remove),
        )
        .route(
            "/api/admin/general-blacklist/",
            get(list).post(add).delete(remove),
        )
        .route("/api/admin/general-blacklist/status", post(status))
        .route("/api/admin/general-blacklist/{ip}", delete(remove_ip))
}

async fn list(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = match parse_positive_i32(query.page.as_deref(), 1, "page must be a positive integer")
    {
        Ok(page) => page,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    let limit = match parse_positive_i32(
        query.limit.as_deref(),
        20,
        "limit must be a positive integer",
    ) {
        Ok(limit) => limit,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    go_data_response(
        &translator,
        state
            .go_backend
            .list_general_blacklist(page, limit, query.search.unwrap_or_default())
            .await,
    )
}

async fn status(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = match parse_body(&body) {
        Ok(value) => value,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    let ips = normalize_status_ip_list(parsed.get("ips").cloned().unwrap_or(Value::Null));
    go_data_response(
        &translator,
        state.go_backend.check_general_blacklist(ips).await,
    )
}

async fn add(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = match parse_body(&body) {
        Ok(value) => value,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    let ips = match normalize_ip_list(parsed.get("ips").cloned().unwrap_or(Value::Null)) {
        Ok(ips) => ips,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    if ips.is_empty() {
        return general_blacklist_error_response(
            &translator,
            StatusCode::BAD_REQUEST,
            "At least one valid IP is required",
        );
    }
    let source = normalize_source(parsed.get("source").and_then(Value::as_str));
    let comment = parsed
        .get("comment")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    go_data_response(
        &translator,
        state
            .go_backend
            .add_general_blacklist(ips, source.to_string(), comment)
            .await,
    )
}

async fn remove(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = match parse_body(&body) {
        Ok(value) => value,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    let ips = match parse_delete_ips(parsed) {
        Ok(ips) => ips,
        Err(message) => {
            return general_blacklist_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };
    if ips.is_empty() {
        return general_blacklist_error_response(
            &translator,
            StatusCode::BAD_REQUEST,
            "At least one valid IP is required",
        );
    }
    go_data_response(
        &translator,
        state.go_backend.remove_general_blacklist(ips).await,
    )
}

async fn remove_ip(State(state): State<AppState>, Path(ip): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    let normalized = normalize_ip(&ip);
    if normalized.is_empty() {
        return general_blacklist_error_response(
            &translator,
            StatusCode::BAD_REQUEST,
            "Invalid IP",
        );
    }
    go_data_response(
        &translator,
        state
            .go_backend
            .remove_general_blacklist(vec![normalized])
            .await,
    )
}

fn go_data_response(translator: &Translator, result: anyhow::Result<Value>) -> Response {
    match result {
        Ok(value) => {
            if !value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let status = go_backend_response_status(&value);
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(|message| localize_general_blacklist_error(translator, message))
                    .unwrap_or_else(|| general_blacklist_text(translator, "backendRequestFailed"));
                return response::error(status, message);
            }
            match value.get("data") {
                Some(data) => response::ok(data.clone()).into_response(),
                None => response::error(
                    StatusCode::BAD_GATEWAY,
                    general_blacklist_text(translator, "backendResponseMissingData"),
                ),
            }
        }
        Err(error) => {
            tracing::warn!(%error, "general blacklist Go backend request failed");
            response::error(
                StatusCode::BAD_GATEWAY,
                general_blacklist_text(translator, "backendRequestFailed"),
            )
        }
    }
}

fn go_backend_response_status(value: &Value) -> StatusCode {
    value
        .get("code")
        .and_then(Value::as_u64)
        .filter(|code| (400..=599).contains(code))
        .and_then(|code| StatusCode::from_u16(code as u16).ok())
        .unwrap_or(StatusCode::BAD_GATEWAY)
}

fn parse_positive_i32(
    value: Option<&str>,
    fallback: i32,
    message: &'static str,
) -> Result<i32, &'static str> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(fallback);
    };
    let Ok(parsed) = raw.parse::<i32>() else {
        return Err(message);
    };
    if parsed <= 0 {
        return Err(message);
    }
    Ok(parsed)
}

fn parse_body(body: &[u8]) -> Result<Value, &'static str> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    let parsed: Value = serde_json::from_slice(body).map_err(|_| "Invalid request body")?;
    if let Some(inner) = parsed.as_str() {
        return serde_json::from_str(inner).map_err(|_| "Invalid request body");
    }
    Ok(parsed)
}

fn normalize_ip_list(value: Value) -> Result<Vec<String>, String> {
    let raw_items = value.as_array().cloned().unwrap_or_default();
    let mut seen = Vec::new();
    for item in raw_items {
        let Some(raw) = item.as_str() else {
            return Err("Invalid IP".to_string());
        };
        if raw.trim().is_empty() {
            return Err("Invalid IP".to_string());
        }
        let normalized = normalize_ip(raw);
        if normalized.is_empty() {
            return Err(format!("Invalid IP: {}", raw.trim()));
        }
        if !seen.iter().any(|value| value == &normalized) {
            seen.push(normalized);
        }
    }
    Ok(seen)
}

fn normalize_status_ip_list(value: Value) -> Vec<String> {
    let raw_items = value.as_array().cloned().unwrap_or_default();
    let mut seen = Vec::new();
    for item in raw_items {
        let Some(raw) = item.as_str() else {
            continue;
        };
        let normalized = normalize_ip(raw);
        if normalized.is_empty() {
            continue;
        }
        if !seen.iter().any(|value| value == &normalized) {
            seen.push(normalized);
        }
    }
    seen
}

fn parse_delete_ips(value: Value) -> Result<Vec<String>, String> {
    if value.is_array() {
        return normalize_ip_list(value);
    }
    if let Some(ips) = value.get("ips") {
        return normalize_ip_list(ips.clone());
    }
    Ok(Vec::new())
}

fn normalize_source(value: Option<&str>) -> &'static str {
    let candidate = value.unwrap_or("").trim();
    GENERAL_BLACKLIST_SOURCES
        .iter()
        .copied()
        .find(|source| *source == candidate)
        .unwrap_or("manual")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_and_deduplicates_ip_list() {
        assert_eq!(
            normalize_ip_list(json!([
                "203.0.113.10",
                "203.0.113.10:443",
                "[2001:db8::10]",
                "2001:db8::10"
            ]))
            .unwrap(),
            vec!["203.0.113.10".to_string(), "2001:db8::10".to_string()]
        );
    }

    #[test]
    fn rejects_invalid_blacklist_ips() {
        assert!(normalize_ip_list(json!(["203.0.113.10", "bad-ip"])).is_err());
        assert!(normalize_ip_list(json!(["203.0.113.10", ""])).is_err());
        assert!(normalize_ip_list(json!(["203.0.113.10", 42])).is_err());
    }

    #[test]
    fn status_parser_ignores_invalid_members() {
        assert_eq!(
            normalize_status_ip_list(json!(["203.0.113.10", "bad-ip", "", 42, "[2001:db8::10]"])),
            vec!["203.0.113.10".to_string(), "2001:db8::10".to_string()]
        );
    }

    #[test]
    fn delete_parser_accepts_array_or_object() {
        assert_eq!(
            parse_delete_ips(json!(["203.0.113.10"])).unwrap(),
            vec!["203.0.113.10".to_string()]
        );
        assert_eq!(
            parse_delete_ips(json!({ "ips": ["203.0.113.10"] })).unwrap(),
            vec!["203.0.113.10".to_string()]
        );
    }

    #[test]
    fn localizes_general_blacklist_route_errors() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            localize_general_blacklist_error(&translator, "Invalid request body"),
            "请求体不正确"
        );
        assert_eq!(
            localize_general_blacklist_error(&translator, "Invalid IP"),
            "IP 地址不正确"
        );
        assert_eq!(
            localize_general_blacklist_error(&translator, "Invalid IP: bad-ip"),
            "IP 地址不正确: bad-ip"
        );
        assert_eq!(
            localize_general_blacklist_error(&translator, "At least one valid IP is required"),
            "请至少提供一个有效 IP"
        );
        assert_eq!(
            localize_general_blacklist_error(&translator, "Go backend response missing data"),
            "通用黑名单后端响应缺少数据"
        );
    }

    #[test]
    fn go_backend_response_status_matches_node_rule() {
        assert_eq!(
            go_backend_response_status(&json!({ "code": 409 })),
            StatusCode::CONFLICT
        );
        assert_eq!(
            go_backend_response_status(&json!({ "code": 302 })),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            go_backend_response_status(&json!({ "code": 700 })),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn list_query_positive_integer_parser_matches_gateway_validation() {
        assert_eq!(parse_positive_i32(None, 1, "invalid").unwrap(), 1);
        assert_eq!(parse_positive_i32(Some(""), 1, "invalid").unwrap(), 1);
        assert_eq!(parse_positive_i32(Some(" 2 "), 1, "invalid").unwrap(), 2);
        assert!(parse_positive_i32(Some("0"), 1, "invalid").is_err());
        assert!(parse_positive_i32(Some("2x"), 1, "invalid").is_err());
    }

    #[test]
    fn source_defaults_to_manual() {
        assert_eq!(normalize_source(Some("waf_log")), "waf_log");
        assert_eq!(normalize_source(Some("unknown")), "manual");
    }
}
