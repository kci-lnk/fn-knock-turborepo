use std::time::Duration;

use anyhow::Context;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use serde::Deserialize;
use serde_json::{Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    http_utils::{is_private_or_local_ip, normalize_ip},
    i18n::Translator,
    response,
    state::AppState,
};

mod analytics;

const GATEWAY_LOG_ANALYTICS_TIMEOUT: Duration = Duration::from_secs(120);

use analytics::{
    hydrate_analytics_response, release_geo_refresh, spawn_geo_refresh, try_acquire_geo_refresh,
    with_geo_refresh_lease,
};

fn gateway_logs_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.gatewayLogs.{key}"))
}

#[derive(Deserialize)]
struct GatewayLogQuery {
    date: Option<String>,
    pagination: Option<String>,
    page: Option<String>,
    limit: Option<String>,
    cursor: Option<String>,
    search: Option<String>,
    status: Option<String>,
    logged_in: Option<String>,
    credential: Option<String>,
    waf_status: Option<String>,
    trace_id: Option<String>,
}

#[derive(Deserialize)]
struct GatewayLogAnalyticsQuery {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct GatewayLoggingConfigBody {
    enabled: bool,
    #[serde(default)]
    record_localhost: bool,
    max_days: i64,
}

const GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES: usize = 1024 * 1024;

pub fn gateway_logs_routes() -> Router<AppState> {
    let routes: Router<AppState> = gateway_logs_openapi_routes().into();
    routes.layer(DefaultBodyLimit::max(GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES))
}

pub(crate) fn gateway_logs_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_config))
        .routes(routes!(update_config))
        .routes(routes!(directory))
        .routes(routes!(dates))
        .routes(routes!(entries))
        .routes(routes!(delete_entries))
        .routes(routes!(analytics))
        .routes(routes!(refresh_analytics_geo))
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/config", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_config", responses((status = 200, description = "Gateway logging configuration")))]
async fn get_config(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let settings = match gateway_logging_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway logging config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_logs_text(&translator, "configLoadFailed"),
            );
        }
    };
    let runtime = match state
        .gateway
        .client
        .get_logging_config()
        .await
        .and_then(go_backend_data)
    {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway logging runtime config");
            Value::Null
        }
    };
    response::ok(gateway_logging_config_response(settings, &runtime)).into_response()
}

#[utoipa::path(post, path = "/api/admin/gateway-logs/config", tag = "gateway-logs", operation_id = "post_api_admin_gateway_logs_config", request_body = GatewayLoggingConfigBody, responses((status = 200, description = "Updated gateway logging configuration")))]
async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<GatewayLoggingConfigBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let settings = GatewayLoggingSettings {
        enabled: body.enabled,
        record_localhost: body.record_localhost,
        max_days: normalize_gateway_logging_max_days(body.max_days),
    };
    let mut config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read config before gateway logging update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_logs_text(&translator, "configLoadFailed"),
            );
        }
    };
    ensure_object(&mut config).insert(
        "gateway_logging".to_string(),
        json!({
            "enabled": settings.enabled,
            "record_localhost": settings.record_localhost,
            "max_days": settings.max_days
        }),
    );
    if let Err(error) = state.storage.store.save_config(&config).await {
        tracing::warn!(%error, "failed to save gateway logging config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            gateway_logs_text(&translator, "configSaveFailed"),
        );
    }

    match state
        .gateway
        .client
        .set_gateway_logging_config(&json!({
            "enabled": settings.enabled,
            "record_localhost": settings.record_localhost,
            "max_days": settings.max_days
        }))
        .await
        .and_then(go_backend_data)
    {
        Ok(data) => response::ok(gateway_logging_config_response(settings, &data)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to sync gateway logging config to Go backend");
            response::error(
                StatusCode::BAD_GATEWAY,
                gateway_logs_text(&translator, "configSyncFailed"),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/directory", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_directory", responses((status = 200, description = "Gateway log directory")))]
async fn directory(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    go_data_response(
        &translator,
        state
            .gateway
            .client
            .get_logging_directory()
            .await
            .and_then(go_backend_data),
        "readDirectoryFailed",
    )
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/dates", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_dates", responses((status = 200, description = "Available gateway log dates")))]
async fn dates(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    go_data_response(
        &translator,
        state
            .gateway
            .client
            .get_log_dates()
            .await
            .and_then(go_backend_data),
        "readDatesFailed",
    )
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/entries", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_entries", responses((status = 200, description = "Gateway log entries")))]
async fn entries(State(state): State<AppState>, Query(query): Query<GatewayLogQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let trace_id = query
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if trace_id.is_some_and(|value| !crate::trace_id::is_valid_trace_id(value)) {
        return response::error(StatusCode::BAD_REQUEST, "invalid trace_id");
    }
    let waf_status = normalize_waf_status_filter(query.waf_status.as_deref());
    let result = if let Some(trace_id) = trace_id {
        find_gateway_log_entry(&state, trace_id).await
    } else if let Some(waf_status) = waf_status {
        get_entries_with_waf_filter(&state, query, waf_status).await
    } else {
        go_log_entries(&state, &query, true).await
    };

    match result {
        Ok(data) => response::ok(hydrate_entries_response(data)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway log entries");
            response::error(
                StatusCode::BAD_REQUEST,
                gateway_logs_text(&translator, "readEntriesFailed"),
            )
        }
    }
}

async fn find_gateway_log_entry(state: &AppState, trace_id: &str) -> anyhow::Result<Value> {
    let value = state
        .gateway
        .client
        .find_log_entry_by_trace_id(trace_id)
        .await?;
    let data = go_backend_data(value)?;
    let item = data
        .get("found")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| data.get("entry").cloned())
        .flatten()
        .filter(|entry| !entry.is_null());
    let items = item.into_iter().collect::<Vec<_>>();
    Ok(json!({
        "date": "",
        "logs_dir": "",
        "available_dates": [],
        "pagination": "cursor",
        "page": 1,
        "limit": 1,
        "total": items.len(),
        "cursor": "",
        "next_cursor": "",
        "has_more": false,
        "items": items,
    }))
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/analytics", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_analytics", responses((status = 200, description = "Gateway log analytics")))]
async fn analytics(
    State(state): State<AppState>,
    Query(query): Query<GatewayLogAnalyticsQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let result = state
        .gateway
        .client
        .analyze_log_entries_with_timeout(
            crate::grpc_proto::GatewayLogAnalyticsQuery {
                from_date: query.from.unwrap_or_default(),
                to_date: query.to.unwrap_or_default(),
            },
            GATEWAY_LOG_ANALYTICS_TIMEOUT,
        )
        .await
        .and_then(go_backend_data);

    match result {
        Ok(data) => response::ok(hydrate_analytics_response(&state, data).await).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to analyze gateway log entries");
            response::error(
                StatusCode::BAD_REQUEST,
                gateway_logs_text(&translator, "readEntriesFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/gateway-logs/analytics", tag = "gateway-logs", operation_id = "post_api_admin_gateway_logs_analytics", responses((status = 200, description = "Gateway analytics refresh state")))]
async fn refresh_analytics_geo(
    State(state): State<AppState>,
    Query(query): Query<GatewayLogAnalyticsQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let lock_id = match try_acquire_geo_refresh(&state).await {
        Ok(Some(lock_id)) => lock_id,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                gateway_logs_text(&translator, "geoRefreshActive"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to acquire gateway analytics geo refresh lock");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_logs_text(&translator, "geoRefreshFailed"),
            );
        }
    };
    let result = with_geo_refresh_lease(&state, &lock_id, async {
        state
            .gateway
            .client
            .analyze_log_entries_with_timeout(
                crate::grpc_proto::GatewayLogAnalyticsQuery {
                    from_date: query.from.unwrap_or_default(),
                    to_date: query.to.unwrap_or_default(),
                },
                GATEWAY_LOG_ANALYTICS_TIMEOUT,
            )
            .await
            .and_then(go_backend_data)
    })
    .await;

    let data = match result {
        Ok(data) => data,
        Err(error) => {
            release_geo_refresh(&state, &lock_id).await;
            tracing::warn!(%error, "failed to prepare gateway analytics geo refresh");
            return response::error(
                StatusCode::BAD_REQUEST,
                gateway_logs_text(&translator, "geoRefreshFailed"),
            );
        }
    };

    spawn_geo_refresh(&state, data, lock_id);
    response::ok(json!({ "refreshing": true })).into_response()
}

#[utoipa::path(delete, path = "/api/admin/gateway-logs/entries", tag = "gateway-logs", operation_id = "delete_api_admin_gateway_logs_entries", responses((status = 200, description = "Deleted gateway log entries")))]
async fn delete_entries(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                gateway_logs_text(&translator, "invalidJsonObject"),
            );
        }
    };
    let date = parsed
        .get("date")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    go_data_response(
        &translator,
        state
            .gateway
            .client
            .delete_log_date(date)
            .await
            .and_then(go_backend_data),
        "deleteEntriesFailed",
    )
}

async fn get_entries_with_waf_filter(
    state: &AppState,
    mut query: GatewayLogQuery,
    waf_status: &'static str,
) -> anyhow::Result<Value> {
    let limit = normalize_positive_integer(query.limit.as_deref(), 20, 200);
    let initial_cursor = normalize_optional_cursor(query.cursor.as_deref());
    let mut items = Vec::<Value>::new();
    let mut base_data: Option<Value> = None;
    let mut raw_cursor = initial_cursor.map(|value| value.to_string());
    let mut next_cursor = String::new();
    let mut has_more = false;
    query.pagination = Some("cursor".to_string());
    query.waf_status = None;

    for scans in 0..200 {
        let remaining = (limit - items.len() as i64).max(1);
        query.limit = Some(remaining.to_string());
        query.cursor = raw_cursor.clone();
        let data = go_log_entries(state, &query, false).await?;
        if base_data.is_none() {
            base_data = Some(data.clone());
        }

        for entry in data
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            if gateway_log_matches_waf_status(&entry, waf_status) {
                items.push(entry);
            }
        }

        if items.len() as i64 >= limit {
            let candidate_next_cursor = data
                .get("next_cursor")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !data
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || candidate_next_cursor.is_empty()
            {
                break;
            }

            next_cursor = candidate_next_cursor.clone();
            let mut lookahead_cursor = Some(candidate_next_cursor);
            for _lookups in (scans + 1)..200 {
                query.cursor = lookahead_cursor.clone();
                query.limit = Some(limit.to_string());
                let lookahead_data = go_log_entries(state, &query, false).await?;
                if lookahead_data
                    .get("items")
                    .and_then(Value::as_array)
                    .is_some_and(|entries| {
                        entries
                            .iter()
                            .any(|entry| gateway_log_matches_waf_status(entry, waf_status))
                    })
                {
                    has_more = true;
                    break;
                }
                if !lookahead_data
                    .get("has_more")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    break;
                }
                let cursor = lookahead_data
                    .get("next_cursor")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if cursor.is_empty() {
                    break;
                }
                lookahead_cursor = Some(cursor.to_string());
            }
            break;
        }

        if !data
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            break;
        }
        let cursor = data
            .get("next_cursor")
            .and_then(Value::as_str)
            .unwrap_or("");
        if cursor.is_empty() {
            break;
        }
        raw_cursor = Some(cursor.to_string());
    }

    let response_cursor = initial_cursor
        .map(|value| value.to_string())
        .or_else(|| {
            base_data
                .as_ref()
                .and_then(|data| data.get("cursor").and_then(Value::as_str))
                .map(ToString::to_string)
        })
        .unwrap_or_default();

    let Some(mut base_data) = base_data else {
        return Ok(json!({
            "date": query.date.unwrap_or_default(),
            "logs_dir": "",
            "available_dates": [],
            "pagination": "cursor",
            "page": 1,
            "limit": limit,
            "total": 0,
            "cursor": response_cursor,
            "next_cursor": "",
            "has_more": false,
            "items": []
        }));
    };

    let object = ensure_object(&mut base_data);
    object.insert(
        "pagination".to_string(),
        Value::String("cursor".to_string()),
    );
    object.insert("page".to_string(), json!(1));
    object.insert("limit".to_string(), json!(limit));
    object.insert(
        "total".to_string(),
        json!(items.len() as i64 + if has_more { 1 } else { 0 }),
    );
    object.insert("cursor".to_string(), Value::String(response_cursor));
    object.insert("next_cursor".to_string(), Value::String(next_cursor));
    object.insert("has_more".to_string(), Value::Bool(has_more));
    object.insert("items".to_string(), Value::Array(items));
    Ok(base_data)
}

async fn go_log_entries(
    state: &AppState,
    query: &GatewayLogQuery,
    _include_waf_status: bool,
) -> anyhow::Result<Value> {
    let pagination = query
        .pagination
        .clone()
        .unwrap_or_else(|| "page".to_string());
    let page = if pagination.trim().eq_ignore_ascii_case("cursor") {
        0
    } else {
        parse_gateway_log_positive_i32(query.page.as_deref(), 1, "page")?
    };
    let limit = parse_gateway_log_positive_i32(query.limit.as_deref(), 20, "limit")?;
    let rpc_query = crate::grpc_proto::GatewayLogQuery {
        date: query.date.clone().unwrap_or_default(),
        page,
        limit,
        search: query.search.clone().unwrap_or_default(),
        status: query.status.clone().unwrap_or_default(),
        logged_in: query.logged_in.clone().unwrap_or_default(),
        credential: query.credential.clone().unwrap_or_default(),
        cursor: query.cursor.clone().unwrap_or_default(),
        pagination,
    };
    state
        .gateway
        .client
        .query_log_entries(rpc_query)
        .await
        .and_then(go_backend_data)
}

fn go_backend_data(value: Value) -> anyhow::Result<Value> {
    if !value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        anyhow::bail!(
            "{}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Go backend request failed")
        );
    }
    Ok(value.get("data").cloned().unwrap_or(Value::Null))
}

fn parse_gateway_log_positive_i32(
    value: Option<&str>,
    fallback: i32,
    field: &str,
) -> anyhow::Result<i32> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(fallback);
    };
    let parsed = raw
        .parse::<i32>()
        .with_context(|| format!("{field} must be a positive integer"))?;
    if parsed <= 0 {
        anyhow::bail!("{field} must be a positive integer");
    }
    Ok(parsed)
}

fn go_data_response(
    translator: &Translator,
    result: anyhow::Result<Value>,
    fallback_key: &str,
) -> Response {
    match result {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "gateway logging Go backend request failed");
            response::error(
                StatusCode::BAD_GATEWAY,
                gateway_logs_text(translator, fallback_key),
            )
        }
    }
}

#[derive(Clone, Copy)]
struct GatewayLoggingSettings {
    enabled: bool,
    record_localhost: bool,
    max_days: i64,
}

async fn gateway_logging_settings(
    state: &AppState,
) -> crate::storage::StorageResult<GatewayLoggingSettings> {
    let config = state.storage.store.get_config().await?;
    let raw = config
        .get("gateway_logging")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Ok(GatewayLoggingSettings {
        enabled: raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        record_localhost: raw
            .get("record_localhost")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        max_days: raw
            .get("max_days")
            .and_then(Value::as_i64)
            .map(normalize_gateway_logging_max_days)
            .unwrap_or(7),
    })
}

#[cfg(test)]
fn gateway_log_query_string(query: &GatewayLogQuery, include_waf_status: bool) -> Option<String> {
    let output = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        append_if_some(&mut serializer, "date", query.date.as_deref());
        append_if_some(&mut serializer, "pagination", query.pagination.as_deref());
        append_if_present(&mut serializer, "page", query.page.as_deref());
        append_if_present(&mut serializer, "limit", query.limit.as_deref());
        append_if_some(&mut serializer, "cursor", query.cursor.as_deref());
        append_if_some(&mut serializer, "search", query.search.as_deref());
        append_if_some(&mut serializer, "status", query.status.as_deref());
        append_if_some(&mut serializer, "logged_in", query.logged_in.as_deref());
        append_if_some(&mut serializer, "credential", query.credential.as_deref());
        if include_waf_status {
            append_if_some(&mut serializer, "waf_status", query.waf_status.as_deref());
        }
        append_if_some(&mut serializer, "trace_id", query.trace_id.as_deref());
        serializer.finish()
    };
    (!output.is_empty()).then_some(output)
}

#[cfg(test)]
fn append_if_some(
    serializer: &mut url::form_urlencoded::Serializer<'_, String>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value
        && !value.is_empty()
    {
        serializer.append_pair(key, value);
    }
}

#[cfg(test)]
fn append_if_present(
    serializer: &mut url::form_urlencoded::Serializer<'_, String>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        serializer.append_pair(key, value);
    }
}

fn gateway_logging_config_response(settings: GatewayLoggingSettings, runtime: &Value) -> Value {
    json!({
        "enabled": settings.enabled,
        "record_localhost": settings.record_localhost,
        "max_days": settings.max_days,
        "logs_dir": runtime.get("logs_dir").and_then(Value::as_str).unwrap_or(""),
        "dropped_entries": runtime_u64_field(runtime, "dropped_entries"),
        "queue_size": runtime_i64_field(runtime, "queue_size"),
        "queue_depth": runtime_i64_field(runtime, "queue_depth")
    })
}

fn runtime_u64_field(value: &Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(|raw| {
            raw.as_u64()
                .or_else(|| raw.as_i64().and_then(|signed| u64::try_from(signed).ok()))
        })
        .unwrap_or(0)
}

fn runtime_i64_field(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|raw| {
            raw.as_i64().or_else(|| {
                raw.as_u64()
                    .and_then(|unsigned| i64::try_from(unsigned).ok())
            })
        })
        .unwrap_or(0)
}

fn hydrate_entries_response(mut data: Value) -> Value {
    if let Some(items) = data.get_mut("items").and_then(Value::as_array_mut) {
        for entry in items {
            hydrate_gateway_log_entry(entry);
        }
    }
    data
}

fn hydrate_gateway_log_entry(entry: &mut Value) {
    let client_ip = infer_gateway_log_client_ip(entry);
    if let Some(object) = entry.as_object_mut() {
        object.insert("client_ip".to_string(), Value::String(client_ip));
    }
}

fn infer_gateway_log_client_ip(entry: &Value) -> String {
    if let Some(client_ip) = entry
        .get("client_ip")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = normalize_ip(client_ip);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    let provider_candidates = split_forwarded_ips(entry.get("eo_connecting_ip"))
        .into_iter()
        .chain(split_forwarded_ips(entry.get("ali_real_client_ip")))
        .collect::<Vec<_>>();
    let provider_ip = pick_preferred_ip(&provider_candidates);
    if !provider_ip.is_empty() {
        return provider_ip;
    }

    let remote_raw = entry.get("remote_ip").and_then(Value::as_str).unwrap_or("");
    let remote_ip = normalize_ip(remote_raw);
    let proxy_header_candidates = split_forwarded_ips(entry.get("x_forwarded_for"))
        .into_iter()
        .chain(split_forwarded_ips(entry.get("x_real_ip")))
        .collect::<Vec<_>>();
    if !remote_ip.is_empty() && is_private_or_local_ip(&remote_ip) {
        let proxy_header_ip = pick_preferred_ip(&proxy_header_candidates);
        if !proxy_header_ip.is_empty() {
            return proxy_header_ip;
        }
    }

    remote_ip
}

fn split_forwarded_ips(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .split(',')
        .map(|item| normalize_ip(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

fn pick_preferred_ip(candidates: &[String]) -> String {
    candidates
        .iter()
        .find(|ip| !is_private_or_local_ip(ip))
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or_default()
}

fn gateway_log_has_waf_signal(entry: &Value) -> bool {
    entry.get("waf_trace_id").is_some_and(js_truthy_value)
        || entry.get("waf_bundle").is_some_and(js_truthy_value)
        || entry.get("waf_action").is_some_and(js_truthy_value)
        || entry
            .get("waf_blocked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || entry
            .get("waf_rule_ids")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
}

fn js_truthy_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn gateway_log_matches_waf_status(entry: &Value, status: &str) -> bool {
    let has_waf = gateway_log_has_waf_signal(entry);
    match status {
        "has_waf" => has_waf,
        "none" => !has_waf,
        _ => true,
    }
}

fn normalize_waf_status_filter(value: Option<&str>) -> Option<&'static str> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "has_waf" => Some("has_waf"),
        "none" => Some("none"),
        _ => None,
    }
}

fn normalize_positive_integer(value: Option<&str>, fallback: i64, max: i64) -> i64 {
    value
        .and_then(|value| crate::node_compat::parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .map(|value| value.min(max))
        .unwrap_or(fallback)
}

fn normalize_optional_cursor(value: Option<&str>) -> Option<i64> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    crate::node_compat::parse_i64_prefix(value).filter(|value| *value >= 0)
}

fn normalize_gateway_logging_max_days(value: i64) -> i64 {
    value.max(1)
}

use crate::json_utils::ensure_object;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
        routing::{delete, post},
    };
    use tower::ServiceExt;

    #[test]
    fn infers_client_ip_from_provider_and_forwarded_headers() {
        assert_eq!(
            infer_gateway_log_client_ip(&json!({
                "remote_ip": "10.0.0.2",
                "x_forwarded_for": "198.51.100.10, 10.0.0.3"
            })),
            "198.51.100.10"
        );
        assert_eq!(
            infer_gateway_log_client_ip(&json!({
                "remote_ip": "10.0.0.2",
                "eo_connecting_ip": "203.0.113.5"
            })),
            "203.0.113.5"
        );
        assert_eq!(
            infer_gateway_log_client_ip(&json!({
                "client_ip": "",
                "remote_ip": "10.0.0.2",
                "eo_connecting_ip": "203.0.113.6"
            })),
            "203.0.113.6"
        );
        assert_eq!(
            infer_gateway_log_client_ip(&json!({
                "client_ip": "   ",
                "remote_ip": "10.0.0.2",
                "eo_connecting_ip": "203.0.113.6"
            })),
            "203.0.113.6"
        );
        assert_eq!(
            infer_gateway_log_client_ip(&json!({
                "client_ip": "not-an-ip",
                "remote_ip": "also-invalid"
            })),
            ""
        );
    }

    #[test]
    fn detects_waf_status_signal() {
        assert!(gateway_log_matches_waf_status(
            &json!({ "waf_trace_id": "abc" }),
            "has_waf"
        ));
        assert!(gateway_log_matches_waf_status(
            &json!({ "status": 200 }),
            "none"
        ));
        assert!(!gateway_log_matches_waf_status(
            &json!({ "waf_rule_ids": [1] }),
            "none"
        ));
        assert!(gateway_log_matches_waf_status(
            &json!({ "waf_trace_id": 1 }),
            "has_waf"
        ));
        assert!(gateway_log_matches_waf_status(
            &json!({ "waf_action": {} }),
            "has_waf"
        ));
        assert!(gateway_log_matches_waf_status(
            &json!({ "waf_trace_id": 0, "waf_bundle": "", "waf_action": false }),
            "none"
        ));
    }

    #[test]
    fn query_number_parsers_match_node_parse_int_edges() {
        assert_eq!(normalize_positive_integer(None, 20, 200), 20);
        assert_eq!(normalize_positive_integer(Some("2x"), 20, 200), 2);
        assert_eq!(normalize_positive_integer(Some("  +3.9"), 20, 200), 3);
        assert_eq!(normalize_positive_integer(Some("-1"), 20, 200), 20);
        assert_eq!(normalize_positive_integer(Some("999"), 20, 200), 200);

        assert_eq!(normalize_optional_cursor(None), None);
        assert_eq!(normalize_optional_cursor(Some("")), None);
        assert_eq!(normalize_optional_cursor(Some("2x")), Some(2));
        assert_eq!(normalize_optional_cursor(Some("  +3.9")), Some(3));
        assert_eq!(normalize_optional_cursor(Some("-1")), None);
    }

    #[test]
    fn gateway_logging_max_days_matches_node_bounds() {
        assert_eq!(normalize_gateway_logging_max_days(-5), 1);
        assert_eq!(normalize_gateway_logging_max_days(0), 1);
        assert_eq!(normalize_gateway_logging_max_days(7), 7);
        assert_eq!(normalize_gateway_logging_max_days(999), 999);
    }

    #[test]
    fn gateway_logging_config_response_merges_runtime_metrics() {
        let payload = gateway_logging_config_response(
            GatewayLoggingSettings {
                enabled: true,
                record_localhost: true,
                max_days: 14,
            },
            &json!({
                "enabled": false,
                "max_days": 1,
                "logs_dir": "/runtime/logs",
                "dropped_entries": 5,
                "queue_size": 4096,
                "queue_depth": 12
            }),
        );

        assert_eq!(
            payload,
            json!({
                "enabled": true,
                "record_localhost": true,
                "max_days": 14,
                "logs_dir": "/runtime/logs",
                "dropped_entries": 5,
                "queue_size": 4096,
                "queue_depth": 12
            })
        );
    }

    #[test]
    fn gateway_logging_config_response_defaults_runtime_metrics() {
        let payload = gateway_logging_config_response(
            GatewayLoggingSettings {
                enabled: false,
                record_localhost: false,
                max_days: 7,
            },
            &Value::Null,
        );

        assert_eq!(payload["logs_dir"], "");
        assert_eq!(payload["record_localhost"], false);
        assert_eq!(payload["dropped_entries"], 0);
        assert_eq!(payload["queue_size"], 0);
        assert_eq!(payload["queue_depth"], 0);
    }

    #[tokio::test]
    async fn gateway_logging_json_body_limit_rejects_oversized_config() {
        async fn accept_config(Json(_body): Json<GatewayLoggingConfigBody>) -> StatusCode {
            StatusCode::NO_CONTENT
        }

        let app = Router::new()
            .route("/test", post(accept_config))
            .layer(DefaultBodyLimit::max(GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES));
        let payload = json!({
            "enabled": true,
            "max_days": 7,
            "padding": "x".repeat(GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES)
        })
        .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/test")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn gateway_logging_bytes_body_limit_rejects_oversized_delete() {
        async fn accept_bytes(_body: Bytes) -> StatusCode {
            StatusCode::NO_CONTENT
        }

        let app = Router::new()
            .route("/test", delete(accept_bytes))
            .layer(DefaultBodyLimit::max(GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES));
        let payload = vec![b' '; GATEWAY_LOGS_JSON_BODY_LIMIT_BYTES + 1];

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri("/test")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn localizes_gateway_log_route_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            gateway_logs_text(&translator, "configLoadFailed"),
            "读取请求日志配置失败"
        );
        assert_eq!(
            gateway_logs_text(&translator, "invalidJsonObject"),
            "请求体不是有效的 JSON 对象"
        );
        assert_eq!(
            gateway_logs_text(&translator, "readEntriesFailed"),
            "读取请求日志失败"
        );
    }

    #[test]
    fn builds_gateway_log_query_string() {
        let query = GatewayLogQuery {
            date: Some("2026-07-05".to_string()),
            pagination: Some("cursor".to_string()),
            page: None,
            limit: Some("50".to_string()),
            cursor: Some("10".to_string()),
            search: Some("hello world".to_string()),
            status: None,
            logged_in: Some("true".to_string()),
            credential: None,
            waf_status: Some("has_waf".to_string()),
            trace_id: Some("trc_3f93d40a-89ea-4dbe-a04f-67692778d973".to_string()),
        };
        let output = gateway_log_query_string(&query, true).unwrap();
        assert!(output.contains("date=2026-07-05"));
        assert!(output.contains("search=hello+world"));
        assert!(output.contains("waf_status=has_waf"));
        assert!(
            !gateway_log_query_string(&query, false)
                .unwrap()
                .contains("waf_status")
        );
    }

    #[test]
    fn gateway_log_query_keeps_empty_page_limit_like_node() {
        let query = GatewayLogQuery {
            date: Some(String::new()),
            pagination: None,
            page: Some(String::new()),
            limit: Some(String::new()),
            cursor: Some(String::new()),
            search: Some("  ".to_string()),
            status: None,
            logged_in: None,
            credential: None,
            waf_status: None,
            trace_id: None,
        };
        let output = gateway_log_query_string(&query, true).unwrap();
        assert!(output.contains("page="));
        assert!(output.contains("limit="));
        assert!(!output.contains("date="));
        assert!(!output.contains("cursor="));
        assert!(output.contains("search=++"));
    }
}
