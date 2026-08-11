use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::time::{MissedTickBehavior, interval};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    i18n::Translator,
    response,
    state::AppState,
    store::{TrafficDeltaPoint, TrafficSnapshotRecord},
    time_utils,
};

const MAX_TRAFFIC_CHART_POINTS: usize = 300;

fn dashboard_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.dashboard.{key}"))
}

#[derive(Deserialize)]
struct DashboardStatsQuery {
    #[serde(rename = "rangeSec")]
    range_sec: Option<String>,
    #[serde(rename = "userId")]
    user_id: Option<String>,
    host: Option<String>,
}

#[derive(Deserialize)]
struct ActiveIpsQuery {
    host: Option<String>,
}

pub fn dashboard_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(stats))
        .routes(routes!(realtime))
        .routes(routes!(active_ips))
        .routes(routes!(get_dashboard_display, update_dashboard_display))
}

pub fn start_traffic_tasks(state: AppState) {
    let collect_state = state.clone();
    state.spawn_background("traffic-collector", run_traffic_collect_loop(collect_state));
    state.spawn_background("traffic-cleanup", run_traffic_cleanup_loop(state.clone()));
}

#[utoipa::path(
    get,
    path = "/api/admin/dashboard/stats",
    tag = "dashboard",
    operation_id = "get_api_admin_dashboard_stats",
    responses((status = 200, description = "Dashboard traffic statistics"))
)]
async fn stats(
    State(state): State<AppState>,
    Query(query): Query<DashboardStatsQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let user_id = dashboard_user_id(query.user_id.as_deref(), &state.settings.traffic_user_id);
    let host = normalize_traffic_host(query.host.as_deref().unwrap_or(""));
    let range_sec = crate::node_compat::parse_i64_or(query.range_sec.as_deref(), 3600)
        .clamp(60, 30 * 24 * 3600);
    let now_sec = now_unix_seconds();
    let from_sec = now_sec - range_sec;
    let host_ref = (!host.is_empty()).then_some(host.as_str());

    let result = tokio::try_join!(
        state
            .storage
            .store
            .list_traffic_points(&user_id, "in", from_sec, now_sec, host_ref),
        state
            .storage
            .store
            .list_traffic_points(&user_id, "out", from_sec, now_sec, host_ref),
        state
            .storage
            .store
            .list_error5xx_points(&user_id, from_sec, now_sec, host_ref),
        state
            .storage
            .store
            .list_error5xx_points(&user_id, now_sec - 24 * 3600, now_sec, host_ref),
        state.storage.store.list_error5xx_points(
            &user_id,
            now_sec - 7 * 24 * 3600,
            now_sec,
            host_ref
        )
    );

    let (in_points, out_points, err5xx_points, err5xx_1d_points, err5xx_1w_points) = match result {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load dashboard traffic metrics");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                dashboard_text(&translator, "statsLoadFailed"),
            );
        }
    };

    let current = fetch_traffic_stats(&state).await.ok().and_then(|value| {
        if host.is_empty() {
            return Some(value);
        }
        value
            .get("by_host")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| normalize_traffic_host_value(item.get("host")) == host)
                    .cloned()
            })
    });

    let traffic_echarts = json!({
        "tooltip": { "trigger": "axis" },
        "legend": { "data": [
            dashboard_text(&translator, "inbound"),
            dashboard_text(&translator, "outbound"),
        ] },
        "xAxis": { "type": "time" },
        "yAxis": { "type": "value" },
        "series": [
            {
                "name": dashboard_text(&translator, "inbound"),
                "type": "line",
                "showSymbol": false,
                "data": points_to_bps_data(&in_points, range_sec)
            },
            {
                "name": dashboard_text(&translator, "outbound"),
                "type": "line",
                "showSymbol": false,
                "data": points_to_bps_data(&out_points, range_sec)
            }
        ]
    });

    response::ok(json!({
        "rangeSec": range_sec,
        "now": {
            "online": current.as_ref().and_then(|value| value.get("active_conns")).map(normalize_i64_or_zero),
            "error5xxTotal": current.as_ref().and_then(|value| value.get("error_5xx")).map(normalize_f64_or_zero)
        },
        "totals": {
            "inBytes": sum_points(&in_points),
            "outBytes": sum_points(&out_points),
            "error5xx": sum_points(&err5xx_points)
        },
        "errors": {
            "error5xx1d": sum_points(&err5xx_1d_points),
            "error5xx1w": sum_points(&err5xx_1w_points)
        },
        "traffic": {
            "echarts": traffic_echarts
        }
    }))
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/admin/dashboard/realtime",
    tag = "dashboard",
    operation_id = "get_api_admin_dashboard_realtime",
    responses((status = 200, description = "Current dashboard traffic snapshot"))
)]
async fn realtime(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_realtime_payload(&state).await {
        Ok(Some(payload)) => response::ok(payload).into_response(),
        Ok(None) => response::error(
            StatusCode::BAD_GATEWAY,
            dashboard_text(&translator, "upstreamUnavailable"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to read realtime traffic stats");
            response::error(
                StatusCode::BAD_GATEWAY,
                dashboard_text(&translator, "upstreamUnavailable"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/dashboard/active-ips",
    tag = "dashboard",
    operation_id = "get_api_admin_dashboard_active_ips",
    responses((status = 200, description = "Active client addresses for one host"))
)]
async fn active_ips(
    State(state): State<AppState>,
    Query(query): Query<ActiveIpsQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let host = normalize_traffic_host(query.host.as_deref().unwrap_or(""));
    if host.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            dashboard_text(&translator, "hostRequired"),
        );
    }

    let (status, envelope) = match state.gateway.client.get_host_active_ips(host.clone()).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to read active IP stats from Go backend");
            return response::error(
                StatusCode::BAD_GATEWAY,
                dashboard_text(&translator, "upstreamUnavailable"),
            );
        }
    };

    if status.as_u16() == 404 || envelope_code(&envelope) == Some(404) {
        return response::ok(json!({
            "host": host,
            "window_seconds": 120,
            "items": [],
            "timestamp": time_utils::now_ms()
        }))
        .into_response();
    }

    if !status.is_success()
        || !envelope
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        let code = envelope_code(&envelope)
            .filter(|code| (400..=599).contains(code))
            .unwrap_or(502);
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_GATEWAY);
        let message = envelope
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| dashboard_text(&translator, "upstreamUnavailable"));
        return response::error(status, message);
    }

    let data = envelope.get("data").cloned().unwrap_or_else(|| json!({}));
    let mut items = data
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_active_ip_item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    items.sort_by(|left, right| {
        let left_seen = left
            .get("last_seen_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let right_seen = right
            .get("last_seen_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        right_seen.cmp(left_seen).then_with(|| {
            left.get("ip")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .cmp(right.get("ip").and_then(Value::as_str).unwrap_or_default())
        })
    });

    let response_host = {
        let normalized = normalize_traffic_host_value(data.get("host"));
        if normalized.is_empty() {
            host
        } else {
            normalized
        }
    };

    response::ok(json!({
        "host": response_host,
        "window_seconds": normalize_active_ip_window_seconds(data.get("window_seconds")),
        "items": items,
        "timestamp": time_utils::now_ms()
    }))
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/admin/config/dashboard_display",
    tag = "config",
    operation_id = "get_api_admin_config_dashboard_display",
    responses((status = 200, description = "Dashboard display configuration"))
)]
async fn get_dashboard_display(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_config().await {
        Ok(config) => response::ok(normalize_dashboard_display(config.get("dashboard_display")))
            .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load dashboard display config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                dashboard_text(&translator, "configLoadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/dashboard_display",
    tag = "config",
    operation_id = "post_api_admin_config_dashboard_display",
    responses((status = 200, description = "Updated dashboard display configuration"))
)]
async fn update_dashboard_display(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state
        .storage
        .store
        .merge_config_object_fields("dashboard_display", dashboard_display_update_fields(&body))
        .await
    {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to save dashboard display config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                dashboard_text(&translator, "displayConfigSaveFailed"),
            );
        }
    };
    let next = normalize_dashboard_display(config.get("dashboard_display"));

    response::ok(next).into_response()
}

async fn run_traffic_collect_loop(state: AppState) {
    let mut ticker = interval(state.settings.traffic_collect_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = state.shutdown.cancelled() => break,
            _ = ticker.tick() => {}
        }
        tokio::select! {
            _ = state.shutdown.cancelled() => break,
            result = collect_traffic_once(&state) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "traffic collect task failed");
                }
            }
        }
    }
}

async fn run_traffic_cleanup_loop(state: AppState) {
    let mut ticker = interval(state.settings.traffic_cleanup_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = state.shutdown.cancelled() => break,
            _ = ticker.tick() => {}
        }
        tokio::select! {
            _ = state.shutdown.cancelled() => break,
            result = cleanup_traffic_once(&state) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "traffic cleanup task failed");
                }
            }
        }
    }
}

async fn collect_traffic_once(state: &AppState) -> anyhow::Result<()> {
    let acquired = state
        .storage
        .store
        .set_lock_if_not_exists(
            "traffic-collect",
            state.settings.traffic_collect_lock_ttl_seconds,
        )
        .await?;
    if !acquired {
        return Ok(());
    }

    let snapshot = fetch_traffic_stats(state).await?;
    let records = build_snapshot_records(&snapshot);
    state
        .storage
        .store
        .record_traffic_snapshot(
            &state.settings.traffic_user_id,
            &records,
            now_unix_seconds(),
            state.settings.traffic_keep_seconds,
        )
        .await?;
    Ok(())
}

async fn cleanup_traffic_once(state: &AppState) -> anyhow::Result<()> {
    let acquired = state
        .storage
        .store
        .set_lock_if_not_exists(
            "traffic-cleanup",
            state.settings.traffic_cleanup_lock_ttl_seconds,
        )
        .await?;
    if !acquired {
        return Ok(());
    }
    let _ = state
        .storage
        .store
        .cleanup_traffic_metrics(state.settings.traffic_keep_seconds)
        .await?;
    Ok(())
}

async fn build_realtime_payload(state: &AppState) -> anyhow::Result<Option<Value>> {
    let data = fetch_traffic_stats(state).await?;
    Ok(Some(json!({
        "total_in": normalize_f64_or_zero(data.get("total_in").unwrap_or(&Value::Null)),
        "total_out": normalize_f64_or_zero(data.get("total_out").unwrap_or(&Value::Null)),
        "active_conns": normalize_i64_or_zero(data.get("active_conns").unwrap_or(&Value::Null)),
        "error_5xx": normalize_f64_or_zero(data.get("error_5xx").unwrap_or(&Value::Null)),
        "by_host": data.get("by_host")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(normalize_host_traffic_item).collect::<Vec<_>>())
            .unwrap_or_default(),
        "timestamp": time_utils::now_ms()
    })))
}

async fn fetch_traffic_stats(state: &AppState) -> anyhow::Result<Value> {
    let (status, envelope) = state.gateway.client.get_traffic_stats().await?;
    if status.as_u16() == 404 || envelope_code(&envelope) == Some(404) {
        return Ok(default_traffic_snapshot());
    }
    if !status.is_success() {
        anyhow::bail!("go traffic endpoint returned HTTP {status}");
    }
    if !envelope
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        anyhow::bail!(
            "go traffic endpoint failed: {}",
            envelope
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        );
    }
    envelope
        .get("data")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("go traffic endpoint returned no data"))
}

fn default_traffic_snapshot() -> Value {
    json!({
        "total_in": 0,
        "total_out": 0,
        "active_conns": 0,
        "error_5xx": 0,
        "by_host": []
    })
}

fn build_snapshot_records(snapshot: &Value) -> Vec<TrafficSnapshotRecord> {
    let mut records = vec![TrafficSnapshotRecord {
        host: None,
        total_in: normalize_f64_or_zero(snapshot.get("total_in").unwrap_or(&Value::Null)),
        total_out: normalize_f64_or_zero(snapshot.get("total_out").unwrap_or(&Value::Null)),
        error_5xx: normalize_f64_or_zero(snapshot.get("error_5xx").unwrap_or(&Value::Null)),
    }];

    let mut by_host = HashMap::<String, TrafficSnapshotRecord>::new();
    for item in snapshot
        .get("by_host")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let host = normalize_traffic_host_value(item.get("host"));
        if host.is_empty() {
            continue;
        }
        by_host.insert(
            host.clone(),
            TrafficSnapshotRecord {
                host: Some(host),
                total_in: normalize_f64_or_zero(item.get("total_in").unwrap_or(&Value::Null)),
                total_out: normalize_f64_or_zero(item.get("total_out").unwrap_or(&Value::Null)),
                error_5xx: normalize_f64_or_zero(item.get("error_5xx").unwrap_or(&Value::Null)),
            },
        );
    }
    records.extend(by_host.into_values());
    records
}

fn normalize_host_traffic_item(item: &Value) -> Option<Value> {
    let host = normalize_traffic_host_value(item.get("host"));
    if host.is_empty() {
        return None;
    }
    Some(json!({
        "host": host,
        "total_in": normalize_f64_or_zero(item.get("total_in").unwrap_or(&Value::Null)),
        "total_out": normalize_f64_or_zero(item.get("total_out").unwrap_or(&Value::Null)),
        "error_5xx": normalize_f64_or_zero(item.get("error_5xx").unwrap_or(&Value::Null)),
        "active_ip_count": normalize_active_ip_count(item.get("active_ip_count"))
    }))
}

fn normalize_active_ip_item(item: &Value) -> Option<Value> {
    let ip = js_string_nullish_empty(item.get("ip")).trim().to_string();
    let last_seen_at = to_iso_string_safe(item.get("last_seen_at"));
    if ip.is_empty() || last_seen_at.is_empty() {
        return None;
    }
    Some(json!({
        "ip": ip,
        "last_seen_at": last_seen_at,
        "active_conns": normalize_active_ip_count(item.get("active_conns"))
    }))
}

fn points_to_bps_data(points: &[TrafficDeltaPoint], range_sec: i64) -> Vec<Value> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut sorted = points.to_vec();
    sorted.sort_by_key(|point| point.ts);

    if sorted.len() <= MAX_TRAFFIC_CHART_POINTS {
        let mut last_ts = sorted[0].ts - 1;
        return sorted
            .into_iter()
            .map(|point| {
                let dt = (point.ts - last_ts).max(1) as f64;
                last_ts = point.ts;
                json!([point.ts * 1000, round3(point.delta / dt)])
            })
            .collect();
    }

    let bucket_sec = ((range_sec + MAX_TRAFFIC_CHART_POINTS as i64 - 1)
        / MAX_TRAFFIC_CHART_POINTS as i64)
        .max(1);
    let mut result = Vec::new();
    let mut current_bucket_ts = (sorted[0].ts / bucket_sec) * bucket_sec;
    let mut current_bucket_delta = 0.0;
    let mut has_data = false;

    for point in sorted {
        let bucket = (point.ts / bucket_sec) * bucket_sec;
        if bucket != current_bucket_ts {
            if has_data {
                result.push(json!([
                    current_bucket_ts * 1000,
                    round3(current_bucket_delta / bucket_sec as f64)
                ]));
            }
            current_bucket_ts = bucket;
            current_bucket_delta = 0.0;
            has_data = true;
        } else {
            has_data = true;
        }
        current_bucket_delta += point.delta;
    }

    if has_data {
        result.push(json!([
            current_bucket_ts * 1000,
            round3(current_bucket_delta / bucket_sec as f64)
        ]));
    }

    result
}

pub fn normalize_traffic_host(value: &str) -> String {
    let mut host = value.trim().to_lowercase();
    if host.is_empty() {
        return String::new();
    }

    if let Some(index) = host.find("://") {
        let scheme = &host[..index];
        if !scheme.is_empty()
            && scheme
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-'))
            && scheme
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            host = host[index + 3..].to_string();
        }
    }
    if let Some((prefix, _)) = host.split_once('/') {
        host = prefix.to_string();
    }
    host = host.trim_end_matches('.').to_string();
    if host.is_empty() {
        return String::new();
    }

    if host.starts_with('[') {
        if let Some(end) = host.find(']') {
            return host[..=end].to_string();
        }
        return host;
    }

    if let Some(last_colon) = host.rfind(':')
        && host[..last_colon].find(':').is_none()
    {
        let possible_port = &host[last_colon + 1..];
        if possible_port.chars().all(|ch| ch.is_ascii_digit()) {
            host.truncate(last_colon);
        }
    }

    host.trim().to_string()
}

fn normalize_date_time_display_mode(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("full") => "full",
        _ => "human_friendly",
    }
}

fn normalize_dashboard_display(value: Option<&Value>) -> Value {
    json!({
        "show_entry_status_module": value
            .and_then(|value| value.get("show_entry_status_module"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "date_time_display_mode": normalize_date_time_display_mode(
            value.and_then(|value| value.get("date_time_display_mode"))
        ),
        "sidebar_menu_order": normalize_sidebar_menu_order(
            value.and_then(|value| value.get("sidebar_menu_order"))
        )
    })
}

fn dashboard_display_update_fields(body: &Value) -> Map<String, Value> {
    let mut fields = Map::new();
    if let Some(show) = body
        .get("show_entry_status_module")
        .and_then(Value::as_bool)
    {
        fields.insert("show_entry_status_module".to_string(), Value::Bool(show));
    }
    if body.get("sidebar_menu_order").is_some_and(Value::is_array) {
        fields.insert(
            "sidebar_menu_order".to_string(),
            normalize_sidebar_menu_order(body.get("sidebar_menu_order")),
        );
    }
    if body.get("date_time_display_mode").is_some() {
        fields.insert(
            "date_time_display_mode".to_string(),
            json!(normalize_date_time_display_mode(
                body.get("date_time_display_mode")
            )),
        );
    }
    fields
}

pub(crate) const DEFAULT_SIDEBAR_MENU_ORDER: &[&str] = &[
    "dashboard",
    "route_mapping",
    "tunnel",
    "protocol_mapping",
    "sessions",
    "ip_whitelist",
    "ssl_certificate",
    "ddns",
    "wol",
    "auth",
    "ssh_security",
    "events",
    "gateway_request_logs",
    "waf_logs",
    "web_terminal",
    "system_settings",
];

fn normalize_sidebar_menu_order(value: Option<&Value>) -> Value {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();

    if let Some(items) = value.and_then(Value::as_array) {
        for item in items {
            let Some(id) = item.as_str() else {
                continue;
            };
            if DEFAULT_SIDEBAR_MENU_ORDER.contains(&id) && seen.insert(id) {
                normalized.push(Value::String(id.to_string()));
            }
        }
    }

    for id in DEFAULT_SIDEBAR_MENU_ORDER {
        if seen.insert(id) {
            normalized.push(Value::String((*id).to_string()));
        }
    }

    Value::Array(normalized)
}

fn envelope_code(envelope: &Value) -> Option<u16> {
    envelope
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
}

fn normalize_f64_or_zero(value: &Value) -> f64 {
    let number = js_number_with_nullish_fallback(Some(value), 0.0);
    if number.is_finite() && number > 0.0 {
        number
    } else {
        0.0
    }
}

fn normalize_i64_or_zero(value: &Value) -> i64 {
    let number = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value.floor() as i64))
        .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        .unwrap_or(0);
    number.max(0)
}

fn normalize_active_ip_window_seconds(value: Option<&Value>) -> i64 {
    let seconds = js_number_with_nullish_fallback(value, 120.0);
    if !seconds.is_finite() {
        return 120;
    }
    (seconds.floor() as i64).max(1)
}

fn to_iso_string_safe(value: Option<&Value>) -> String {
    let raw = value.and_then(Value::as_str).unwrap_or("").trim();
    if raw.is_empty() {
        return String::new();
    }
    OffsetDateTime::parse(raw, &Rfc3339)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn dashboard_user_id(query_user_id: Option<&str>, default_user_id: &str) -> String {
    query_user_id
        .filter(|value| !value.is_empty())
        .unwrap_or(default_user_id)
        .to_string()
}

fn normalize_traffic_host_value(value: Option<&Value>) -> String {
    normalize_traffic_host(&js_string_nullish_empty(value))
}

fn normalize_active_ip_count(value: Option<&Value>) -> i64 {
    let count = js_number_with_nullish_fallback(value, 0.0);
    if !count.is_finite() || count < 0.0 {
        return 0;
    }
    count.floor() as i64
}

fn js_number_with_nullish_fallback(value: Option<&Value>, fallback: f64) -> f64 {
    match value {
        None | Some(Value::Null) => fallback,
        Some(Value::Bool(value)) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Some(Value::Number(value)) => value.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Some(Value::Array(values)) => js_array_number(values),
        Some(Value::Object(_)) => f64::NAN,
    }
}

fn js_array_number(values: &[Value]) -> f64 {
    match values {
        [] => 0.0,
        [value] => {
            let text = js_string_for_array_item(value);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    }
}

fn js_string_nullish_empty(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(value) => js_string(value),
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(js_string_for_array_item)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_string_for_array_item(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        value => js_string(value),
    }
}

fn now_unix_seconds() -> i64 {
    time_utils::now_ms() / 1000
}

fn sum_points(points: &[TrafficDeltaPoint]) -> f64 {
    points.iter().map(|point| point.delta).sum()
}

fn round3(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_sidebar_menu_order_for_legacy_and_malformed_config() {
        assert_eq!(
            normalize_dashboard_display(Some(&json!({
                "show_entry_status_module": false
            }))),
            json!({
                "show_entry_status_module": false,
                "date_time_display_mode": "human_friendly",
                "sidebar_menu_order": DEFAULT_SIDEBAR_MENU_ORDER
            })
        );

        assert_eq!(
            normalize_dashboard_display(Some(&json!({
                "date_time_display_mode": "full"
            })))
            .get("date_time_display_mode"),
            Some(&json!("full"))
        );
        assert_eq!(
            normalize_dashboard_display(Some(&json!({
                "date_time_display_mode": "invalid"
            })))
            .get("date_time_display_mode"),
            Some(&json!("human_friendly"))
        );

        let normalized = normalize_sidebar_menu_order(Some(&json!([
            "events",
            "events",
            "unknown",
            42,
            "dashboard"
        ])));
        let items = normalized.as_array().expect("normalized menu order");
        assert_eq!(items.first(), Some(&json!("events")));
        assert_eq!(items.get(1), Some(&json!("dashboard")));
        assert_eq!(items.len(), DEFAULT_SIDEBAR_MENU_ORDER.len());
        assert_eq!(
            items
                .iter()
                .filter(|item| item.as_str() == Some("events"))
                .count(),
            1
        );
    }

    #[test]
    fn dashboard_display_partial_updates_only_write_submitted_fields() {
        let show_update =
            dashboard_display_update_fields(&json!({ "show_entry_status_module": true }));
        assert_eq!(show_update.len(), 1);
        assert_eq!(show_update["show_entry_status_module"], json!(true));

        let order_update = dashboard_display_update_fields(
            &json!({ "sidebar_menu_order": ["events", "dashboard"] }),
        );
        assert_eq!(order_update.len(), 1);
        assert_eq!(order_update["sidebar_menu_order"][0], json!("events"));
        assert_eq!(order_update["sidebar_menu_order"][1], json!("dashboard"));
        assert_eq!(
            order_update["sidebar_menu_order"].as_array().map(Vec::len),
            Some(DEFAULT_SIDEBAR_MENU_ORDER.len())
        );

        let mode_update =
            dashboard_display_update_fields(&json!({ "date_time_display_mode": "full" }));
        assert_eq!(mode_update.len(), 1);
        assert_eq!(mode_update["date_time_display_mode"], json!("full"));

        let invalid_mode_update =
            dashboard_display_update_fields(&json!({ "date_time_display_mode": "not-supported" }));
        assert_eq!(invalid_mode_update.len(), 1);
        assert_eq!(
            invalid_mode_update["date_time_display_mode"],
            json!("human_friendly")
        );
    }

    #[test]
    fn normalizes_traffic_hosts_like_node_dashboard() {
        assert_eq!(
            normalize_traffic_host("HTTPS://Example.COM:443/path?q=1."),
            "example.com"
        );
        assert_eq!(
            normalize_traffic_host("[2001:db8::1]:8443"),
            "[2001:db8::1]"
        );
        assert_eq!(normalize_traffic_host("example.com."), "example.com");
        assert_eq!(normalize_traffic_host(""), "");
    }

    #[test]
    fn converts_points_to_bps_with_first_point_interval_one() {
        let points = vec![
            TrafficDeltaPoint { ts: 10, delta: 5.0 },
            TrafficDeltaPoint {
                ts: 12,
                delta: 10.0,
            },
        ];
        assert_eq!(
            points_to_bps_data(&points, 60),
            vec![json!([10_000, 5.0]), json!([12_000, 5.0])]
        );
    }

    #[test]
    fn dashboard_query_parsers_match_node_truthiness_and_parse_int() {
        assert_eq!(dashboard_user_id(None, "global"), "global");
        assert_eq!(dashboard_user_id(Some(""), "global"), "global");
        assert_eq!(dashboard_user_id(Some("   "), "global"), "   ");
        assert_eq!(dashboard_user_id(Some(" user "), "global"), " user ");

        assert_eq!(crate::node_compat::parse_i64_or(None, 3600), 3600);
        assert_eq!(crate::node_compat::parse_i64_or(Some("30x"), 3600), 30);
        assert_eq!(crate::node_compat::parse_i64_or(Some("  +3.9"), 3600), 3);
        assert_eq!(crate::node_compat::parse_i64_or(Some("-1abc"), 3600), -1);
        assert_eq!(crate::node_compat::parse_i64_or(Some("0x10"), 3600), 0);
        assert_eq!(crate::node_compat::parse_i64_or(Some("abc"), 3600), 3600);
    }

    #[test]
    fn active_ip_normalizers_match_node_number_and_string_rules() {
        let item = normalize_active_ip_item(&json!({
            "ip": 123,
            "last_seen_at": "2026-07-06T19:14:08Z",
            "active_conns": "3.9"
        }))
        .expect("active IP item");
        assert_eq!(item.get("ip"), Some(&json!("123")));
        assert_eq!(item.get("active_conns"), Some(&json!(3)));

        assert_eq!(normalize_active_ip_count(Some(&json!(true))), 1);
        assert_eq!(normalize_active_ip_count(Some(&json!(""))), 0);
        assert_eq!(normalize_active_ip_count(Some(&json!("bad"))), 0);
        assert_eq!(normalize_active_ip_count(Some(&json!(-1))), 0);
        assert_eq!(normalize_active_ip_count(Some(&json!([5]))), 5);
        assert_eq!(normalize_active_ip_count(Some(&json!([1, 2]))), 0);

        assert_eq!(normalize_active_ip_window_seconds(None), 120);
        assert_eq!(normalize_active_ip_window_seconds(Some(&Value::Null)), 120);
        assert_eq!(normalize_active_ip_window_seconds(Some(&json!(""))), 1);
        assert_eq!(normalize_active_ip_window_seconds(Some(&json!("3.9"))), 3);
        assert_eq!(normalize_active_ip_window_seconds(Some(&json!("bad"))), 120);
    }

    #[test]
    fn host_traffic_normalizer_coerces_host_like_node_string() {
        let item = normalize_host_traffic_item(&json!({
            "host": 123,
            "total_in": "10.5",
            "total_out": true,
            "error_5xx": "bad",
            "active_ip_count": "2.9"
        }))
        .expect("host item");
        assert_eq!(item.get("host"), Some(&json!("123")));
        assert_eq!(item.get("total_in"), Some(&json!(10.5)));
        assert_eq!(item.get("total_out"), Some(&json!(1.0)));
        assert_eq!(item.get("error_5xx"), Some(&json!(0.0)));
        assert_eq!(item.get("active_ip_count"), Some(&json!(2)));
    }

    #[test]
    fn localizes_dashboard_route_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(dashboard_text(&translator, "inbound"), "入站");
        assert_eq!(
            dashboard_text(&translator, "statsLoadFailed"),
            "加载仪表盘统计失败"
        );
        assert_eq!(
            dashboard_text(&translator, "displayConfigSaveFailed"),
            "保存仪表盘展示配置失败"
        );
    }

    #[test]
    fn builds_snapshot_records_for_global_and_normalized_hosts() {
        let records = build_snapshot_records(&json!({
            "total_in": 100,
            "total_out": 40,
            "error_5xx": 2,
            "by_host": [
                { "host": "Example.COM:443", "total_in": 10, "total_out": 5, "error_5xx": 1 },
                { "host": "", "total_in": 999 }
            ]
        }));
        assert!(
            records
                .iter()
                .any(|record| record.host.is_none() && record.total_in == 100.0)
        );
        assert!(
            records
                .iter()
                .any(|record| record.host.as_deref() == Some("example.com"))
        );
    }
}
