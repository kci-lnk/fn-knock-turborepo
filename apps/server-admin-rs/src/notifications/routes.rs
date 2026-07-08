use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

mod definitions;
mod delivery;
mod details;
mod matcher;
mod provider_service;
mod providers;
mod rule_service;
mod runtime;
mod store;
mod templates;
mod utils;

use definitions::*;
use delivery::*;
use details::*;
use matcher::*;
use provider_service::*;
use providers::*;
use rule_service::*;
use runtime::*;
use store::*;
use templates::*;
use utils::*;

#[cfg(test)]
mod tests;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use lettre::{
    Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::Mailbox,
    transport::smtp::{authentication::Credentials, client::Tls},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::time;

use crate::{i18n::Translator, response, state::AppState, time_utils};

const PROVIDERS_INDEX_KEY: &str = "fn_knock:notifications:providers:index";
const PROVIDERS_DATA_PREFIX: &str = "fn_knock:notifications:providers:data:";
const RULES_INDEX_KEY: &str = "fn_knock:notifications:rules:index";
const RULES_DATA_PREFIX: &str = "fn_knock:notifications:rules:data:";
const TRIGGERS_INDEX_KEY: &str = "fn_knock:notifications:triggers:index";
const TRIGGERS_DATA_PREFIX: &str = "fn_knock:notifications:triggers:data:";
const DELIVERIES_INDEX_KEY: &str = "fn_knock:notifications:deliveries:index";
const DELIVERIES_DATA_PREFIX: &str = "fn_knock:notifications:deliveries:data:";
const DELIVERIES_READY_KEY: &str = "fn_knock:notifications:deliveries:ready";
const HISTORY_RETENTION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const DISPATCH_INTERVAL: Duration = Duration::from_millis(3000);
const DELIVERY_INTERVAL: Duration = Duration::from_millis(1500);
const STREAM_BATCH_SIZE: usize = 50;
const DELIVERY_BATCH_SIZE: usize = 10;
const DISPATCH_LEASE_TTL_SECONDS: usize = 15;
const APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH: usize = 360;
const DEFAULT_NOTIFICATION_MESSAGE_TITLE: &str = "fn-knock 通知";

const PROVIDER_TYPES: &[&str] = &[
    "wxpusher",
    "serverchan",
    "pushplus",
    "wecom",
    "dingtalk",
    "feishu",
    "email",
    "webhook",
    "pushdeer",
    "magicpush",
    "bark",
    "telegram",
];
const NOTIFICATION_MESSAGE_LOCALES: [&str; 5] = ["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"];
const NOTIFICATION_TEST_SERVICE_KEYS: &[&str] = &[
    "unsupportedProviderType",
    "testSendFailed",
    "testSendSuccess",
];
const NOTIFICATION_PROVIDER_ERROR_KEYS: &[&str] = &[
    "missingWebhookUrl",
    "missingSendKey",
    "missingToken",
    "missingPushKey",
    "missingBaseUrl",
    "missingDeviceKey",
    "missingBotToken",
    "missingChatId",
    "missingAppToken",
    "recipientRequired",
    "invalidTopicIds",
    "missingSmtpHost",
    "invalidEmailAddress",
    "invalidFromAddress",
    "smtpConnectionTimeout",
    "missingUrl",
    "requestFailed",
];
const GROUP_BY_VALUES: &[&str] = &["GLOBAL", "IP", "SESSION", "SUBJECT", "HOSTNAME", "PROVIDER"];
const TRIGGER_STATUSES: &[&str] = &["created", "fanout_done", "partially_failed", "completed"];
const DELIVERY_STATUSES: &[&str] = &[
    "queued", "sending", "success", "failed", "gave_up", "skipped",
];
const MESSAGE_TEMPLATE_MODES: &[&str] = &["default", "custom"];
const TEMPLATE_OVERRIDE_MODES: &[&str] = &["inherit", "custom"];
const SYSTEM_EVENT_LEVELS: &[&str] = &["INFO", "WARN", "ERROR", "CRITICAL"];
const SYSTEM_EVENT_SOURCES: &[&str] = &["SERVER_ADMIN", "GO_REAUTH_PROXY", "SYSTEM_MONITOR"];
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

#[derive(Deserialize)]
struct PageQuery {
    page: Option<String>,
    limit: Option<String>,
    rule_id: Option<String>,
    provider_id: Option<String>,
    trigger_id: Option<String>,
    status: Option<String>,
}

#[derive(Clone)]
struct SchemaField {
    key: &'static str,
    label: &'static str,
    field_type: &'static str,
    required: bool,
    sensitive: bool,
    placeholder: Option<&'static str>,
    default_value: Option<Value>,
    min: Option<i64>,
    max: Option<i64>,
    options: Vec<(&'static str, &'static str)>,
}

impl SchemaField {
    fn placeholder(mut self, value: &'static str) -> Self {
        if !value.is_empty() {
            self.placeholder = Some(value);
        }
        self
    }

    fn bounds(mut self, min: i64, max: i64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    fn min(mut self, min: i64) -> Self {
        self.min = Some(min);
        self
    }
}

#[derive(Clone)]
struct ProviderDefinition {
    provider_type: &'static str,
    label: &'static str,
    description: &'static str,
    connection_schema: Vec<SchemaField>,
    target_schema: Vec<SchemaField>,
    sensitive_fields: Vec<&'static str>,
    supports_markdown: bool,
    supports_actions: bool,
    supports_mentions: bool,
    supports_provider_dedupe_key: bool,
}

pub fn notification_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/notifications/providers/catalog",
            get(provider_catalog),
        )
        .route(
            "/api/admin/notifications/providers",
            get(list_providers).post(create_provider),
        )
        .route(
            "/api/admin/notifications/providers/test",
            post(test_provider_draft),
        )
        .route(
            "/api/admin/notifications/providers/{id}",
            get(get_provider)
                .patch(update_provider)
                .delete(delete_provider),
        )
        .route(
            "/api/admin/notifications/providers/{id}/test",
            post(test_provider),
        )
        .route(
            "/api/admin/notifications/rules",
            get(list_rules).post(create_rule),
        )
        .route(
            "/api/admin/notifications/rules/{id}",
            patch(update_rule).delete(delete_rule),
        )
        .route("/api/admin/notifications/triggers", get(list_triggers))
        .route(
            "/api/admin/notifications/deliveries",
            get(list_deliveries).delete(clear_deliveries),
        )
}

pub fn start_notification_tasks(state: AppState) {
    tokio::spawn(notification_dispatch_loop(state.clone()));
    tokio::spawn(notification_delivery_loop(state));
}

async fn notification_dispatch_loop(state: AppState) {
    let mut interval = time::interval(DISPATCH_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(error) = notification_dispatch_tick(&state).await {
            tracing::warn!(%error, "notification dispatch tick failed");
        }
    }
}

async fn notification_delivery_loop(state: AppState) {
    let mut interval = time::interval(DELIVERY_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(error) = process_ready_deliveries(&state, DELIVERY_BATCH_SIZE).await {
            tracing::warn!(%error, "notification delivery tick failed");
        }
    }
}

async fn provider_catalog(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let providers = PROVIDER_TYPES
        .iter()
        .filter_map(|provider_type| provider_definition(provider_type))
        .map(|definition| provider_definition_view(&definition, &translator))
        .collect::<Vec<_>>();
    response::ok(json!({ "providers": providers })).into_response()
}

async fn list_providers(State(state): State<AppState>) -> Response {
    match load_providers(&state).await {
        Ok(providers) => {
            let views = providers
                .iter()
                .map(mask_provider)
                .collect::<Result<Vec<_>, _>>();
            match views {
                Ok(views) => response::ok(json!({ "providers": views })).into_response(),
                Err(message) => response::error(StatusCode::BAD_REQUEST, message),
            }
        }
        Err(error) => internal_error(&state, "failed to list notification providers", error).await,
    }
}

async fn get_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_provider(&state, &id).await {
        Ok(Some(provider)) => match reveal_provider(&provider) {
            Ok(view) => response::ok(view).into_response(),
            Err(message) => response::error(StatusCode::BAD_REQUEST, message),
        },
        Ok(None) => response::error(
            StatusCode::BAD_REQUEST,
            notification_service_text(&translator, "providerNotFound", &[]),
        ),
        Err(error) => internal_error(&state, "failed to load notification provider", error).await,
    }
}

async fn create_provider(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    match create_provider_value(&state, body).await {
        Ok(provider) => response::ok(provider).into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to create notification provider", error).await
        }
    }
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    match update_provider_value(&state, &id, body).await {
        Ok(provider) => response::ok(provider).into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to update notification provider", error).await
        }
    }
}

async fn delete_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match delete_provider_value(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to delete notification provider", error).await
        }
    }
}

async fn test_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_provider(&state, &id).await {
        Ok(Some(provider)) => {
            match run_provider_test(&state, provider.clone(), &translator).await {
                Ok(test_result) => {
                    let test_result = localize_provider_test_result(test_result, &translator);
                    let mut tested_provider = provider;
                    apply_provider_test_result(&mut tested_provider, &test_result);
                    if let Err(error) = save_provider_raw(&state, &tested_provider).await {
                        return internal_error(
                            &state,
                            "failed to save notification provider test status",
                            error,
                        )
                        .await;
                    }
                    provider_test_response(test_result, Some(&tested_provider))
                }
                Err(message) => response::error(StatusCode::BAD_REQUEST, message),
            }
        }
        Ok(None) => response::error(
            StatusCode::BAD_REQUEST,
            notification_service_text(&translator, "providerNotFound", &[]),
        ),
        Err(error) => internal_error(&state, "failed to load notification provider", error).await,
    }
}

async fn test_provider_draft(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    match draft_provider_value(&state, body).await {
        Ok(provider) => match run_provider_test(&state, provider.clone(), &translator).await {
            Ok(test_result) => {
                let test_result = localize_provider_test_result(test_result, &translator);
                let mut tested_provider = provider;
                apply_provider_test_result(&mut tested_provider, &test_result);
                provider_test_response(test_result, Some(&tested_provider))
            }
            Err(message) => response::error(StatusCode::BAD_REQUEST, message),
        },
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(
                &state,
                "failed to build notification provider test draft",
                error,
            )
            .await
        }
    }
}

async fn list_rules(State(state): State<AppState>) -> Response {
    match load_rules(&state).await {
        Ok(rules) => response::ok(json!({ "rules": rules })).into_response(),
        Err(error) => internal_error(&state, "failed to list notification rules", error).await,
    }
}

async fn create_rule(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    match create_rule_value(&state, body).await {
        Ok(rule) => response::ok(rule).into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to create notification rule", error).await
        }
    }
}

async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    match update_rule_value(&state, &id, body).await {
        Ok(rule) => response::ok(rule).into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to update notification rule", error).await
        }
    }
}

async fn delete_rule(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match delete_rule_value(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(NotifyError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(NotifyError::Redis(error)) => {
            internal_error(&state, "failed to delete notification rule", error).await
        }
    }
}

async fn list_triggers(State(state): State<AppState>, Query(query): Query<PageQuery>) -> Response {
    let safe_status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| TRIGGER_STATUSES.contains(status));
    match list_history(
        &state,
        TRIGGERS_INDEX_KEY,
        TRIGGERS_DATA_PREFIX,
        parse_positive_int(query.page.as_deref(), 1, i64::MAX),
        parse_positive_int(query.limit.as_deref(), 20, 100),
        |item| {
            matches_optional_string(item, "rule_id", query.rule_id.as_deref())
                && matches_optional_string(item, "status", safe_status)
        },
    )
    .await
    {
        Ok((triggers, total)) => {
            response::ok(json!({ "triggers": triggers, "total": total })).into_response()
        }
        Err(error) => internal_error(&state, "failed to list notification triggers", error).await,
    }
}

async fn list_deliveries(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Response {
    let safe_status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| DELIVERY_STATUSES.contains(status));
    match list_history(
        &state,
        DELIVERIES_INDEX_KEY,
        DELIVERIES_DATA_PREFIX,
        parse_positive_int(query.page.as_deref(), 1, i64::MAX),
        parse_positive_int(query.limit.as_deref(), 20, 100),
        |item| {
            matches_optional_string(item, "rule_id", query.rule_id.as_deref())
                && matches_optional_string(item, "provider_id", query.provider_id.as_deref())
                && matches_optional_string(item, "trigger_id", query.trigger_id.as_deref())
                && matches_optional_string(item, "status", safe_status)
        },
    )
    .await
    {
        Ok((deliveries, total)) => {
            response::ok(json!({ "deliveries": deliveries, "total": total })).into_response()
        }
        Err(error) => internal_error(&state, "failed to list notification deliveries", error).await,
    }
}

async fn clear_deliveries(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = match parse_json_body(&body, &translator) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let status = parsed.get("status").and_then(Value::as_str).map(str::trim);
    if let Some(status) = status
        && !status.is_empty()
        && !DELIVERY_STATUSES.contains(&status)
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            notification_route_text(&translator, "unsupportedDeliveryStatus", &[]),
        );
    }

    match clear_delivery_values(
        &state,
        ClearDeliveryFilter {
            rule_id: trimmed_string(parsed.get("rule_id")),
            provider_id: trimmed_string(parsed.get("provider_id")),
            trigger_id: trimmed_string(parsed.get("trigger_id")),
            status: status.map(str::to_string).filter(|value| !value.is_empty()),
        },
    )
    .await
    {
        Ok(deleted_count) => {
            response::ok(json!({ "deleted_count": deleted_count })).into_response()
        }
        Err(error) => {
            internal_error(&state, "failed to clear notification deliveries", error).await
        }
    }
}

fn provider_test_response(result: ProviderTestResult, provider: Option<&Value>) -> Response {
    let provider_view = provider.and_then(|provider| mask_provider(provider).ok());
    Json(json!({
        "success": result.success,
        "message": result.message,
        "data": {
            "provider": provider_view.unwrap_or(Value::Null),
            "request_summary": result.request_summary.unwrap_or(Value::Null),
            "response_summary": result.response_summary.unwrap_or(Value::Null)
        }
    }))
    .into_response()
}
