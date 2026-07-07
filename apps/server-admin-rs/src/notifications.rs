use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use hmac::{Hmac, Mac};
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

type HmacSha256 = Hmac<Sha256>;

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
    loop {
        interval.tick().await;
        if let Err(error) = notification_dispatch_tick(&state).await {
            tracing::warn!(%error, "notification dispatch tick failed");
        }
    }
}

async fn notification_delivery_loop(state: AppState) {
    let mut interval = time::interval(DELIVERY_INTERVAL);
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

async fn notification_dispatch_tick(state: &AppState) -> anyhow::Result<()> {
    let token = create_runtime_token("dispatch");
    let acquired = state
        .redis
        .acquire_notification_runtime_lease("dispatch", &token, DISPATCH_LEASE_TTL_SECONDS)
        .await?;
    if !acquired {
        return Ok(());
    }

    let result = notification_dispatch_tick_locked(state).await;
    let release_result = state
        .redis
        .release_notification_runtime_lease("dispatch", &token)
        .await;
    if let Err(error) = release_result {
        tracing::warn!(%error, "failed to release notification dispatch lease");
    }
    result
}

async fn notification_dispatch_tick_locked(state: &AppState) -> anyhow::Result<()> {
    let mut last_stream_id = state.redis.get_notification_last_stream_id().await?;
    if last_stream_id.is_none() {
        let latest = state
            .redis
            .latest_system_event_stream_id()
            .await?
            .unwrap_or_else(|| "0-0".to_string());
        state.redis.set_notification_last_stream_id(&latest).await?;
        last_stream_id = Some(latest);
    }
    let Some(last_stream_id) = last_stream_id else {
        return Ok(());
    };

    let items = state
        .redis
        .read_system_event_stream_after(&last_stream_id, STREAM_BATCH_SIZE)
        .await?;
    for (stream_id, event) in items {
        if let Err(error) = handle_notification_event(state, &event).await {
            tracing::warn!(%error, stream_id, "failed to fan out notification event");
        }
        state
            .redis
            .set_notification_last_stream_id(&stream_id)
            .await?;
    }
    Ok(())
}

async fn handle_notification_event(state: &AppState, event: &Value) -> anyhow::Result<()> {
    let rules = load_rules(state).await?;
    let matching_rules = rules
        .into_iter()
        .filter(|rule| event_matches_notification_rule(event, rule))
        .collect::<Vec<_>>();
    if matching_rules.is_empty() {
        return Ok(());
    }

    for rule in matching_rules {
        fanout_notification_rule(state, event, rule).await?;
    }
    Ok(())
}

async fn fanout_notification_rule(
    state: &AppState,
    event: &Value,
    rule: Value,
) -> anyhow::Result<()> {
    let rule_id = rule.get("id").and_then(Value::as_str).unwrap_or_default();
    let event_id = event.get("id").and_then(Value::as_str).unwrap_or_default();
    if rule_id.is_empty() || event_id.is_empty() {
        return Ok(());
    }

    let trigger_id = create_stable_id("ntftrig", &[rule_id, event_id]);
    let mut trigger = load_trigger(state, &trigger_id).await?;
    let mut trigger_created = false;
    if trigger.is_none() {
        let group_by = rule
            .get("group_by")
            .and_then(Value::as_str)
            .unwrap_or("GLOBAL");
        let group_key = build_notification_group_key(event, group_by);
        let happened_at_ms = event
            .get("happened_at")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or_else(time_utils::now_ms);
        let window_seconds = rule
            .get("window_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(60)
            .max(1);
        let matched_count = state
            .redis
            .append_notification_window_hit(
                rule_id,
                &group_key,
                event_id,
                happened_at_ms,
                window_seconds,
            )
            .await?;
        let threshold_count = rule
            .get("threshold_count")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1);
        if matched_count < threshold_count {
            return Ok(());
        }
        if let Some(cooldown_until) = state
            .redis
            .get_notification_cooldown_until(rule_id, &group_key)
            .await?
            && time_utils::parse_iso_ms(&cooldown_until).unwrap_or_default() > time_utils::now_ms()
        {
            return Ok(());
        }

        let translator = Translator::from_state(state).await;
        let now = time_utils::now_iso();
        let draft = json!({
            "id": trigger_id,
            "rule_id": rule_id,
            "event_id": event_id,
            "group_key": group_key,
            "matched_count": matched_count,
            "message_snapshot": build_notification_message(event, &rule, matched_count, &group_key, &translator),
            "rule_snapshot": rule,
            "status": "created",
            "created_at": now
        });
        save_trigger_raw(state, &draft).await?;
        trigger = Some(draft);
        trigger_created = true;
    }

    let Some(trigger) = trigger else {
        return Ok(());
    };
    fanout_trigger_targets(state, event, &trigger, trigger_created).await?;
    refresh_trigger_status(
        state,
        trigger.get("id").and_then(Value::as_str).unwrap_or(""),
    )
    .await?;

    if trigger_created {
        let fanout_rule = trigger
            .get("rule_snapshot")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let cooldown_seconds = fanout_rule
            .get("cooldown_seconds")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if cooldown_seconds > 0 {
            let until = time_utils::iso_after_seconds(cooldown_seconds);
            let rule_id = fanout_rule
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let group_key = trigger
                .get("group_key")
                .and_then(Value::as_str)
                .unwrap_or("global");
            state
                .redis
                .set_notification_cooldown_until(rule_id, group_key, &until, cooldown_seconds)
                .await?;
        }
    }

    Ok(())
}

async fn fanout_trigger_targets(
    state: &AppState,
    event: &Value,
    trigger: &Value,
    trigger_created: bool,
) -> anyhow::Result<()> {
    let trigger_id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut fanout_rule = trigger
        .get("rule_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let targets = fanout_rule
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let translator = Translator::from_state(state).await;
    let message = trigger.get("message_snapshot").cloned().unwrap_or_else(|| {
        build_notification_message(event, &fanout_rule, 1, "global", &translator)
    });
    let trigger_created_at = trigger
        .get("created_at")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            message
                .get("occurred_at")
                .and_then(Value::as_str)
                .unwrap_or("")
        });
    let event_id = trigger
        .get("event_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let rule_id = trigger.get("rule_id").and_then(Value::as_str).unwrap_or("");

    for target in targets {
        let target_id = target.get("id").and_then(Value::as_str).unwrap_or_default();
        if target_id.is_empty() {
            continue;
        }
        let provider_id = target
            .get("provider_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let provider = if provider_id.is_empty() {
            None
        } else {
            load_provider(state, provider_id).await?
        };
        let delivery_id = create_stable_id("ntfdel", &[trigger_id, target_id]);
        if load_delivery(state, &delivery_id).await?.is_some() {
            continue;
        }

        let provider_enabled = provider
            .as_ref()
            .and_then(|provider| provider.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let target_enabled = target
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if provider.is_none() || !provider_enabled || !target_enabled {
            let reason = if provider.is_none() {
                "provider_missing"
            } else if !provider_enabled {
                "provider_disabled"
            } else {
                "target_disabled"
            };
            let skipped = build_delivery_value(DeliveryBuildArgs {
                id: delivery_id,
                trigger_id: trigger_id.to_string(),
                rule_id: rule_id.to_string(),
                target_id: target_id.to_string(),
                provider_id: provider_id.to_string(),
                event_id: event_id.to_string(),
                status: "skipped".to_string(),
                reason: Some(reason.to_string()),
                provider_type: provider
                    .as_ref()
                    .and_then(|provider| provider.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("webhook")
                    .to_string(),
                message_snapshot: message.clone(),
                target_snapshot: target.clone(),
                provider_snapshot: provider
                    .as_ref()
                    .and_then(|provider| mask_provider(provider).ok())
                    .unwrap_or_else(|| {
                        deleted_provider_snapshot(provider_id, trigger_created_at, &translator)
                    }),
                attempt_count: 0,
                triggered_at: trigger_created_at.to_string(),
                next_retry_at: None,
            });
            save_delivery_raw(state, &skipped).await?;
            continue;
        }

        let provider = provider.unwrap_or_else(|| json!({}));
        let delivery = build_delivery_value(DeliveryBuildArgs {
            id: delivery_id.clone(),
            trigger_id: trigger_id.to_string(),
            rule_id: rule_id.to_string(),
            target_id: target_id.to_string(),
            provider_id: provider_id.to_string(),
            event_id: event_id.to_string(),
            status: "queued".to_string(),
            reason: None,
            provider_type: provider
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("webhook")
                .to_string(),
            message_snapshot: message.clone(),
            target_snapshot: target.clone(),
            provider_snapshot: mask_provider(&provider).unwrap_or_else(|_| json!({})),
            attempt_count: 0,
            triggered_at: trigger_created_at.to_string(),
            next_retry_at: Some(trigger_created_at.to_string()),
        });
        save_delivery_raw(state, &delivery).await?;
        state
            .redis
            .enqueue_notification_delivery(&delivery_id, time_utils::now_ms())
            .await?;
    }

    if trigger_created {
        if let Some(object) = fanout_rule.as_object_mut() {
            object.insert(
                "last_triggered_at".to_string(),
                Value::String(trigger_created_at.to_string()),
            );
            object.insert(
                "updated_at".to_string(),
                Value::String(trigger_created_at.to_string()),
            );
        }
        if fanout_rule.get("id").and_then(Value::as_str).is_some() {
            save_rule_raw(state, &fanout_rule).await?;
        }
    }

    if let Some(latest) = load_trigger(state, trigger_id).await?
        && latest.get("status").and_then(Value::as_str) == Some("created")
    {
        let mut updated = latest.as_object().cloned().unwrap_or_default();
        updated.insert(
            "status".to_string(),
            Value::String("fanout_done".to_string()),
        );
        save_trigger_raw(state, &Value::Object(updated)).await?;
    }

    Ok(())
}

async fn process_ready_deliveries(state: &AppState, limit: usize) -> anyhow::Result<usize> {
    let ids = state
        .redis
        .pull_ready_notification_delivery_ids(limit, time_utils::now_ms())
        .await?;
    let count = ids.len();
    for id in ids {
        if let Err(error) = process_delivery(state, &id).await {
            tracing::warn!(%error, delivery_id = id, "failed to process notification delivery");
        }
    }
    Ok(count)
}

async fn process_delivery(state: &AppState, delivery_id: &str) -> anyhow::Result<()> {
    let Some(delivery) = load_delivery(state, delivery_id).await? else {
        return Ok(());
    };
    if is_terminal_delivery_status(delivery.get("status").and_then(Value::as_str)) {
        return Ok(());
    }

    let trigger_id = delivery
        .get("trigger_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let rule_id = delivery
        .get("rule_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let provider_id = delivery
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let trigger = load_trigger(state, trigger_id).await?;
    let rule = load_rule(state, rule_id).await?;
    let provider = load_provider(state, provider_id).await?;
    if trigger.is_none() || rule.is_none() || provider.is_none() {
        let mut updated = delivery.as_object().cloned().unwrap_or_default();
        updated.insert("status".to_string(), Value::String("gave_up".to_string()));
        updated.insert(
            "reason".to_string(),
            Value::String("missing_trigger_rule_or_provider".to_string()),
        );
        save_delivery_raw(state, &Value::Object(updated)).await?;
        if !trigger_id.is_empty() {
            refresh_trigger_status(state, trigger_id).await?;
        }
        return Ok(());
    }

    let trigger = trigger.unwrap_or_else(|| json!({}));
    let rule = rule.unwrap_or_else(|| json!({}));
    let provider = provider.unwrap_or_else(|| json!({}));
    let target_id = delivery
        .get("target_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let target = find_rule_target(&rule, target_id)
        .or_else(|| delivery.get("target_snapshot").cloned())
        .unwrap_or_else(|| json!({}));
    let policy = resolve_delivery_policy(target.get("delivery_policy"));
    let attempt_count = delivery
        .get("attempt_count")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + 1;
    let mut sending = delivery.as_object().cloned().unwrap_or_default();
    sending.insert("status".to_string(), Value::String("sending".to_string()));
    sending.insert("attempt_count".to_string(), json!(attempt_count));
    sending.insert("reason".to_string(), Value::Null);
    sending.insert("next_retry_at".to_string(), Value::Null);
    let sending = Value::Object(sending);
    save_delivery_raw(state, &sending).await?;

    let message = sending
        .get("message_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let translator = Translator::from_state(state).await;
    let result = match provider.get("type").and_then(Value::as_str) {
        Some("webhook") => {
            send_webhook_delivery(
                state,
                &provider,
                &target,
                &sending,
                &trigger,
                &rule,
                policy.timeout_seconds,
                &translator,
            )
            .await
        }
        Some(provider_type) if is_http_notification_provider(provider_type) => {
            send_http_notification_provider(
                state,
                &provider,
                &target,
                &message,
                policy.timeout_seconds,
            )
            .await
        }
        Some("email") => {
            send_email_notification(
                &provider,
                &target,
                &message,
                policy.timeout_seconds,
                &translator,
            )
            .await
        }
        Some(provider_type) => ProviderTestResult {
            success: false,
            message: format!("unsupported_provider:{provider_type}"),
            request_summary: None,
            response_summary: None,
        },
        None => ProviderTestResult {
            success: false,
            message: "unsupported_provider".to_string(),
            request_summary: None,
            response_summary: None,
        },
    };
    let result = localize_provider_test_result(result, &translator);

    let mut updated = sending.as_object().cloned().unwrap_or_default();
    let retryable = result
        .response_summary
        .as_ref()
        .and_then(|summary| summary.get("status"))
        .and_then(Value::as_i64)
        .is_some_and(|status| status >= 500 || status == 429);
    updated.insert(
        "request_summary".to_string(),
        result.request_summary.clone().unwrap_or(Value::Null),
    );
    updated.insert(
        "response_summary".to_string(),
        result.response_summary.clone().unwrap_or(Value::Null),
    );
    if result.success {
        updated.insert("status".to_string(), Value::String("success".to_string()));
        updated.insert("sent_at".to_string(), Value::String(time_utils::now_iso()));
        updated.insert("next_retry_at".to_string(), Value::Null);
        save_delivery_raw(state, &Value::Object(updated)).await?;
        refresh_trigger_status(state, trigger_id).await?;
        return Ok(());
    }

    if retryable && attempt_count < policy.max_attempts {
        let next_retry_at = time_utils::iso_after_seconds(policy.backoff_seconds);
        updated.insert("status".to_string(), Value::String("failed".to_string()));
        updated.insert("reason".to_string(), Value::String(result.message));
        updated.insert(
            "next_retry_at".to_string(),
            Value::String(next_retry_at.clone()),
        );
        save_delivery_raw(state, &Value::Object(updated)).await?;
        state
            .redis
            .enqueue_notification_delivery(
                delivery_id,
                time_utils::parse_iso_ms(&next_retry_at).unwrap_or_else(time_utils::now_ms),
            )
            .await?;
        return Ok(());
    }

    updated.insert("status".to_string(), Value::String("gave_up".to_string()));
    updated.insert("reason".to_string(), Value::String(result.message));
    updated.insert("next_retry_at".to_string(), Value::Null);
    save_delivery_raw(state, &Value::Object(updated)).await?;
    refresh_trigger_status(state, trigger_id).await?;
    Ok(())
}

async fn create_provider_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let provider_type = trimmed_string(body.get("type")).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(&provider_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let mut raw_config = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_config);
    let connection_config = normalize_schema_config(&raw_config, &definition.connection_schema)?;
    validate_required_fields(&connection_config, &definition.connection_schema)?;

    let existing = load_providers(state).await?;
    let names = existing
        .iter()
        .filter_map(|provider| provider.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let requested_name = trimmed_string(body.get("name"));
    let translator = Translator::from_state(state).await;
    let default_name_base = provider_definition_label(&definition, &translator);
    let name =
        requested_name.unwrap_or_else(|| build_next_sequential_name(&default_name_base, &names));
    let now = time_utils::now_iso();
    let provider = json!({
        "id": create_id("ntfprov"),
        "name": name,
        "type": definition.provider_type,
        "enabled": bool_field(&body, "enabled", true),
        "connection_config": Value::Object(connection_config),
        "created_at": now,
        "updated_at": now,
        "last_test_status": "idle",
        "last_error": Value::Null
    });
    save_provider_raw(state, &provider).await?;
    mask_provider(&provider).map_err(NotifyError::BadRequest)
}

async fn update_provider_value(state: &AppState, id: &str, body: Value) -> NotifyResult<Value> {
    let current = load_provider(state, id)
        .await?
        .ok_or_bad(notification_service_default_text("providerNotFound", &[]))?;
    let provider_type = current.get("type").and_then(Value::as_str).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(provider_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;

    let mut raw_patch = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_patch);
    drop_masked_sensitive_patch_values(&definition, &mut raw_patch);
    let patch = normalize_schema_patch(&raw_patch, &definition.connection_schema)?;
    let mut merged = current
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in patch {
        merged.insert(key, value);
    }
    apply_schema_defaults(&mut merged, &definition.connection_schema);
    validate_required_fields(&merged, &definition.connection_schema)?;

    let mut updated = current
        .as_object()
        .cloned()
        .ok_or_bad(notification_service_default_text(
            "invalidProviderRecord",
            &[],
        ))?;
    if let Some(name) = trimmed_string(body.get("name")) {
        updated.insert("name".to_string(), Value::String(name));
    }
    if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
        updated.insert("enabled".to_string(), Value::Bool(enabled));
    }
    updated.insert("connection_config".to_string(), Value::Object(merged));
    updated.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let updated = Value::Object(updated);
    save_provider_raw(state, &updated).await?;
    mask_provider(&updated).map_err(NotifyError::BadRequest)
}

async fn draft_provider_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let requested_id = trimmed_string(body.get("id"));
    let requested_type = trimmed_string(body.get("type")).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(&requested_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let existing = if let Some(id) = requested_id.as_deref() {
        Some(
            load_provider(state, id)
                .await?
                .ok_or_bad(notification_service_default_text("providerNotFound", &[]))?,
        )
    } else {
        None
    };
    if let Some(existing) = existing.as_ref()
        && existing.get("type").and_then(Value::as_str) != Some(definition.provider_type)
    {
        return Err(NotifyError::BadRequest(notification_service_default_text(
            "providerTypeMismatch",
            &[],
        )));
    }

    let mut raw_patch = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_patch);
    drop_masked_sensitive_patch_values(&definition, &mut raw_patch);
    let patch = normalize_schema_patch(&raw_patch, &definition.connection_schema)?;
    let mut connection_config = existing
        .as_ref()
        .and_then(|provider| provider.get("connection_config"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in patch {
        connection_config.insert(key, value);
    }
    apply_schema_defaults(&mut connection_config, &definition.connection_schema);
    validate_required_fields(&connection_config, &definition.connection_schema)?;

    let now = time_utils::now_iso();
    Ok(json!({
        "id": existing.as_ref().and_then(|provider| provider.get("id")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| create_id("ntfprovtest")),
        "name": trimmed_string(body.get("name"))
            .or_else(|| existing.as_ref().and_then(|provider| provider.get("name")).and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| notification_service_text(&translator, "providerTestName", &[("provider", definition.label.to_string())])),
        "type": definition.provider_type,
        "enabled": body.get("enabled").and_then(Value::as_bool)
            .or_else(|| existing.as_ref().and_then(|provider| provider.get("enabled")).and_then(Value::as_bool))
            .unwrap_or(true),
        "connection_config": Value::Object(connection_config),
        "created_at": existing.as_ref().and_then(|provider| provider.get("created_at")).and_then(Value::as_str).unwrap_or(&now),
        "updated_at": now,
        "last_test_at": existing.as_ref().and_then(|provider| provider.get("last_test_at")).cloned().unwrap_or(Value::Null),
        "last_test_status": existing.as_ref().and_then(|provider| provider.get("last_test_status")).cloned().unwrap_or(Value::Null),
        "last_error": existing.as_ref().and_then(|provider| provider.get("last_error")).cloned().unwrap_or(Value::Null)
    }))
}

async fn delete_provider_value(state: &AppState, id: &str) -> NotifyResult<()> {
    let rules = load_rules(state).await?;
    let referenced_by = rules.iter().find_map(|rule| {
        let targets = rule.get("targets").and_then(Value::as_array)?;
        let referenced = targets
            .iter()
            .any(|target| target.get("provider_id").and_then(Value::as_str) == Some(id));
        if referenced {
            rule.get("name").and_then(Value::as_str).map(str::to_string)
        } else {
            None
        }
    });
    if let Some(rule_name) = referenced_by {
        return Err(NotifyError::BadRequest(notification_service_default_text(
            "providerReferencedByRule",
            &[("rule", rule_name)],
        )));
    }
    let key = provider_key(id);
    state.redis.delete_keys(&[key]).await?;
    state
        .redis
        .zrem_string_member(PROVIDERS_INDEX_KEY, id)
        .await?;
    Ok(())
}

async fn create_rule_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let event_type = trimmed_string(body.get("event_type")).ok_or_bad(
        notification_service_text(&translator, "unsupportedEventType", &[]),
    )?;
    if !SYSTEM_EVENT_TYPES.contains(&event_type.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "unsupportedEventType",
            &[],
        )));
    }
    let group_by = trimmed_string(body.get("group_by")).ok_or_bad(notification_service_text(
        &translator,
        "invalidGroupBy",
        &[],
    ))?;
    if !GROUP_BY_VALUES.contains(&group_by.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidGroupBy",
            &[],
        )));
    }
    let message_template_mode =
        trimmed_string(body.get("message_template_mode")).unwrap_or_else(|| "default".to_string());
    if !MESSAGE_TEMPLATE_MODES.contains(&message_template_mode.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidMessageTemplateMode",
            &[],
        )));
    }
    let event_level_filter = unique_string_array(body.get("event_level_filter"));
    if !event_level_filter
        .iter()
        .all(|value| SYSTEM_EVENT_LEVELS.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventLevelFilter",
            &[],
        )));
    }
    let event_source_filter = unique_string_array(body.get("event_source_filter"));
    if !event_source_filter
        .iter()
        .all(|value| SYSTEM_EVENT_SOURCES.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventSourceFilter",
            &[],
        )));
    }

    let targets = normalize_rule_targets(state, body.get("targets"), &[], &translator).await?;
    if targets.is_empty() {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "targetRequired",
            &[],
        )));
    }
    let existing_rules = load_rules(state).await?;
    if existing_rules
        .iter()
        .any(|rule| rule.get("event_type").and_then(Value::as_str) == Some(&event_type))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "duplicateEventRule",
            &[],
        )));
    }

    let now = time_utils::now_iso();
    let mut rule = Map::new();
    rule.insert("id".to_string(), Value::String(create_id("ntfrule")));
    rule.insert(
        "name".to_string(),
        Value::String(build_notification_rule_name(&event_type, &translator)),
    );
    rule.insert(
        "enabled".to_string(),
        Value::Bool(bool_field(&body, "enabled", true)),
    );
    rule.insert("event_type".to_string(), Value::String(event_type));
    if !event_level_filter.is_empty() {
        rule.insert("event_level_filter".to_string(), json!(event_level_filter));
    }
    if !event_source_filter.is_empty() {
        rule.insert(
            "event_source_filter".to_string(),
            json!(event_source_filter),
        );
    }
    rule.insert(
        "window_seconds".to_string(),
        json!(number_field(&body, "window_seconds", 60, 1, 86400)),
    );
    rule.insert(
        "threshold_count".to_string(),
        json!(number_field(&body, "threshold_count", 1, 1, 9999)),
    );
    rule.insert("group_by".to_string(), Value::String(group_by));
    rule.insert(
        "cooldown_seconds".to_string(),
        json!(number_field(&body, "cooldown_seconds", 60, 0, 86400)),
    );
    rule.insert("targets".to_string(), Value::Array(targets));
    rule.insert(
        "message_template_mode".to_string(),
        Value::String(message_template_mode),
    );
    rule.insert(
        "message_template".to_string(),
        body.get("message_template").cloned().unwrap_or(Value::Null),
    );
    rule.insert("created_at".to_string(), Value::String(now.clone()));
    rule.insert("updated_at".to_string(), Value::String(now));
    rule.insert("last_triggered_at".to_string(), Value::Null);
    let rule = Value::Object(rule);
    save_rule_raw(state, &rule).await?;
    Ok(rule)
}

async fn update_rule_value(state: &AppState, id: &str, body: Value) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let current = load_rule(state, id)
        .await?
        .ok_or_bad(notification_service_text(&translator, "ruleNotFound", &[]))?;
    let current_object = current
        .as_object()
        .cloned()
        .ok_or_bad(notification_service_text(
            &translator,
            "invalidRuleRecord",
            &[],
        ))?;
    let event_type = trimmed_string(body.get("event_type")).unwrap_or_else(|| {
        current
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    if !SYSTEM_EVENT_TYPES.contains(&event_type.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "unsupportedEventType",
            &[],
        )));
    }
    let group_by = trimmed_string(body.get("group_by")).unwrap_or_else(|| {
        current
            .get("group_by")
            .and_then(Value::as_str)
            .unwrap_or("GLOBAL")
            .to_string()
    });
    if !GROUP_BY_VALUES.contains(&group_by.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidGroupBy",
            &[],
        )));
    }
    let message_template_mode =
        trimmed_string(body.get("message_template_mode")).unwrap_or_else(|| {
            current
                .get("message_template_mode")
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string()
        });
    if !MESSAGE_TEMPLATE_MODES.contains(&message_template_mode.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidMessageTemplateMode",
            &[],
        )));
    }
    let event_level_filter = if body.get("event_level_filter").is_some() {
        unique_string_array(body.get("event_level_filter"))
    } else {
        unique_string_array(current.get("event_level_filter"))
    };
    if !event_level_filter
        .iter()
        .all(|value| SYSTEM_EVENT_LEVELS.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventLevelFilter",
            &[],
        )));
    }
    let event_source_filter = if body.get("event_source_filter").is_some() {
        unique_string_array(body.get("event_source_filter"))
    } else {
        unique_string_array(current.get("event_source_filter"))
    };
    if !event_source_filter
        .iter()
        .all(|value| SYSTEM_EVENT_SOURCES.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventSourceFilter",
            &[],
        )));
    }
    let current_targets = current
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let targets = if body.get("targets").is_some() {
        normalize_rule_targets(state, body.get("targets"), &current_targets, &translator).await?
    } else {
        current_targets
    };
    if targets.is_empty() {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "targetRequired",
            &[],
        )));
    }
    if event_type
        != current
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        && load_rules(state).await?.iter().any(|rule| {
            rule.get("id").and_then(Value::as_str) != Some(id)
                && rule.get("event_type").and_then(Value::as_str) == Some(&event_type)
        })
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "duplicateEventRule",
            &[],
        )));
    }

    let mut updated = current_object;
    updated.insert(
        "name".to_string(),
        Value::String(build_notification_rule_name(&event_type, &translator)),
    );
    if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
        updated.insert("enabled".to_string(), Value::Bool(enabled));
    }
    updated.insert("event_type".to_string(), Value::String(event_type));
    if event_level_filter.is_empty() {
        updated.remove("event_level_filter");
    } else {
        updated.insert("event_level_filter".to_string(), json!(event_level_filter));
    }
    if event_source_filter.is_empty() {
        updated.remove("event_source_filter");
    } else {
        updated.insert(
            "event_source_filter".to_string(),
            json!(event_source_filter),
        );
    }
    if body.get("window_seconds").is_some() {
        updated.insert(
            "window_seconds".to_string(),
            json!(number_field(
                &body,
                "window_seconds",
                current
                    .get("window_seconds")
                    .and_then(Value::as_i64)
                    .unwrap_or(60),
                1,
                86400
            )),
        );
    }
    if body.get("threshold_count").is_some() {
        updated.insert(
            "threshold_count".to_string(),
            json!(number_field(
                &body,
                "threshold_count",
                current
                    .get("threshold_count")
                    .and_then(Value::as_i64)
                    .unwrap_or(1),
                1,
                9999
            )),
        );
    }
    updated.insert("group_by".to_string(), Value::String(group_by));
    if body.get("cooldown_seconds").is_some() {
        updated.insert(
            "cooldown_seconds".to_string(),
            json!(number_field(
                &body,
                "cooldown_seconds",
                current
                    .get("cooldown_seconds")
                    .and_then(Value::as_i64)
                    .unwrap_or(60),
                0,
                86400
            )),
        );
    }
    updated.insert("targets".to_string(), Value::Array(targets));
    updated.insert(
        "message_template_mode".to_string(),
        Value::String(message_template_mode),
    );
    if let Some(message_template) = body.get("message_template") {
        updated.insert("message_template".to_string(), message_template.clone());
    }
    updated.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let updated = Value::Object(updated);
    save_rule_raw(state, &updated).await?;
    Ok(updated)
}

async fn delete_rule_value(state: &AppState, id: &str) -> NotifyResult<()> {
    state.redis.delete_keys(&[rule_key(id)]).await?;
    state.redis.zrem_string_member(RULES_INDEX_KEY, id).await?;
    Ok(())
}

async fn normalize_rule_targets(
    state: &AppState,
    raw_targets: Option<&Value>,
    current_targets: &[Value],
    translator: &Translator,
) -> NotifyResult<Vec<Value>> {
    let Some(raw_targets) = raw_targets.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let providers = load_providers(state).await?;
    let provider_map = providers
        .into_iter()
        .filter_map(|provider| {
            provider
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), provider.clone()))
        })
        .collect::<HashMap<_, _>>();
    let current_map = current_targets
        .iter()
        .filter_map(|target| {
            target
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), target.clone()))
        })
        .collect::<HashMap<_, _>>();
    let mut targets = Vec::new();
    for raw_target in raw_targets {
        let provider_id = trimmed_string(raw_target.get("provider_id")).ok_or_bad(
            notification_service_text(translator, "ruleProviderMissing", &[]),
        )?;
        let provider = provider_map
            .get(&provider_id)
            .ok_or_bad(notification_service_text(
                translator,
                "ruleProviderMissing",
                &[],
            ))?;
        let provider_type =
            provider
                .get("type")
                .and_then(Value::as_str)
                .ok_or_bad(notification_service_text(
                    translator,
                    "unsupportedProviderType",
                    &[],
                ))?;
        let definition = provider_definition(provider_type).ok_or_bad(
            notification_service_text(translator, "unsupportedProviderType", &[]),
        )?;
        let existing = raw_target
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| current_map.get(id));
        let mut raw_config = object_field(raw_target, "target_config");
        normalize_provider_target_aliases(definition.provider_type, &mut raw_config);
        let target_config = normalize_schema_config(&raw_config, &definition.target_schema)?;
        validate_required_fields(&target_config, &definition.target_schema)?;
        let mode = trimmed_string(raw_target.get("template_override_mode"))
            .or_else(|| {
                existing
                    .and_then(|target| target.get("template_override_mode"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "inherit".to_string());
        if !TEMPLATE_OVERRIDE_MODES.contains(&mode.as_str()) {
            return Err(NotifyError::BadRequest(notification_service_text(
                translator,
                "invalidTemplateOverrideMode",
                &[],
            )));
        }
        let now = time_utils::now_iso();
        targets.push(json!({
            "id": raw_target.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| create_id("ntftarget")),
            "provider_id": provider_id,
            "enabled": raw_target.get("enabled").and_then(Value::as_bool)
                .or_else(|| existing.and_then(|target| target.get("enabled")).and_then(Value::as_bool))
                .unwrap_or(true),
            "target_config": Value::Object(target_config),
            "template_override_mode": mode,
            "template_override": raw_target.get("template_override")
                .cloned()
                .or_else(|| existing.and_then(|target| target.get("template_override")).cloned())
                .unwrap_or(Value::Null),
            "delivery_policy": raw_target.get("delivery_policy")
                .cloned()
                .or_else(|| existing.and_then(|target| target.get("delivery_policy")).cloned())
                .unwrap_or(Value::Null),
            "created_at": existing.and_then(|target| target.get("created_at")).and_then(Value::as_str).unwrap_or(&now),
            "updated_at": now
        }));
    }
    Ok(targets)
}

async fn load_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    load_indexed_values(state, PROVIDERS_INDEX_KEY, PROVIDERS_DATA_PREFIX).await
}

async fn load_provider(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&provider_key(id)).await
}

async fn save_provider_raw(state: &AppState, provider: &Value) -> redis::RedisResult<()> {
    let id = provider
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let score = iso_score_ms(
        provider
            .get("updated_at")
            .and_then(Value::as_str)
            .or_else(|| provider.get("created_at").and_then(Value::as_str)),
    );
    state
        .redis
        .set_json_value(&provider_key(id), provider)
        .await?;
    state
        .redis
        .zadd_string_member(PROVIDERS_INDEX_KEY, id, score)
        .await
}

async fn load_rules(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    load_indexed_values(state, RULES_INDEX_KEY, RULES_DATA_PREFIX).await
}

async fn load_rule(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&rule_key(id)).await
}

async fn save_rule_raw(state: &AppState, rule: &Value) -> redis::RedisResult<()> {
    let id = rule.get("id").and_then(Value::as_str).unwrap_or_default();
    let score = iso_score_ms(
        rule.get("updated_at")
            .and_then(Value::as_str)
            .or_else(|| rule.get("created_at").and_then(Value::as_str)),
    );
    state.redis.set_json_value(&rule_key(id), rule).await?;
    state
        .redis
        .zadd_string_member(RULES_INDEX_KEY, id, score)
        .await
}

async fn load_trigger(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .get_json_value(&format!("{TRIGGERS_DATA_PREFIX}{id}"))
        .await
}

async fn save_trigger_raw(state: &AppState, trigger: &Value) -> redis::RedisResult<()> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = trigger.get("created_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(created_at);
    let score = iso_score_ms(created_at);
    state
        .redis
        .set_json_value_ex(&format!("{TRIGGERS_DATA_PREFIX}{id}"), trigger, ttl)
        .await?;
    state
        .redis
        .zadd_string_member(TRIGGERS_INDEX_KEY, id, score)
        .await
}

async fn load_delivery(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&delivery_key(id)).await
}

async fn save_delivery_raw(state: &AppState, delivery: &Value) -> redis::RedisResult<()> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triggered_at = delivery.get("triggered_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(triggered_at);
    let score = iso_score_ms(triggered_at);
    state
        .redis
        .set_json_value_ex(&delivery_key(id), delivery, ttl)
        .await?;
    state
        .redis
        .zadd_string_member(DELIVERIES_INDEX_KEY, id, score)
        .await
}

fn history_ttl_seconds(happened_at: Option<&str>) -> usize {
    let expires_at = happened_at
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or_else(time_utils::now_ms)
        + HISTORY_RETENTION_TTL_SECONDS * 1000;
    (((expires_at - time_utils::now_ms()).max(1000) + 999) / 1000) as usize
}

fn event_matches_notification_rule(event: &Value, rule: &Value) -> bool {
    if !rule.get("enabled").and_then(Value::as_bool).unwrap_or(true) {
        return false;
    }
    if event.get("type").and_then(Value::as_str) != rule.get("event_type").and_then(Value::as_str) {
        return false;
    }
    if let Some(levels) = rule.get("event_level_filter").and_then(Value::as_array)
        && !levels.is_empty()
    {
        let event_level = event.get("level").and_then(Value::as_str).unwrap_or("");
        if !levels
            .iter()
            .any(|level| level.as_str() == Some(event_level))
        {
            return false;
        }
    }
    if let Some(sources) = rule.get("event_source_filter").and_then(Value::as_array)
        && !sources.is_empty()
    {
        let event_source = event.get("source").and_then(Value::as_str).unwrap_or("");
        if !sources
            .iter()
            .any(|source| source.as_str() == Some(event_source))
        {
            return false;
        }
    }
    true
}

fn build_notification_group_key(event: &Value, group_by: &str) -> String {
    match group_by {
        "IP" => payload_text(event, &["ip", "to_ip", "from_ip"])
            .or_else(|| subject_id_for_kind(event, "IP"))
            .unwrap_or_else(|| "missing:ip".to_string()),
        "SESSION" => payload_text(event, &["session_id"])
            .or_else(|| subject_id_for_kind(event, "SESSION"))
            .unwrap_or_else(|| "missing:session".to_string()),
        "SUBJECT" => event
            .get("subject")
            .and_then(|subject| subject.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "missing:subject".to_string()),
        "HOSTNAME" => payload_text(event, &["hostname"])
            .or_else(|| subject_id_for_kind(event, "RESOURCE"))
            .unwrap_or_else(|| "missing:hostname".to_string()),
        "PROVIDER" => payload_text(event, &["provider"])
            .or_else(|| subject_id_for_kind(event, "DDNS"))
            .unwrap_or_else(|| "missing:provider".to_string()),
        _ => "global".to_string(),
    }
}

fn payload_text(event: &Value, keys: &[&str]) -> Option<String> {
    let payload = event.get("payload").and_then(Value::as_object)?;
    for key in keys {
        let value = payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(value) = value {
            return Some(value.to_string());
        }
    }
    None
}

fn subject_id_for_kind(event: &Value, kind: &str) -> Option<String> {
    let subject = event.get("subject")?;
    if subject.get("kind").and_then(Value::as_str) != Some(kind) {
        return None;
    }
    subject
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_notification_message(
    event: &Value,
    rule: &Value,
    matched_count: i64,
    group_key: &str,
    translator: &Translator,
) -> Value {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("event");
    let details = build_notification_details(event, rule, matched_count, translator);
    let title = brand_notification_title(
        &build_notification_title(event, matched_count, translator),
        translator,
    );
    let happened_at = event
        .get("happened_at")
        .and_then(Value::as_str)
        .unwrap_or_else(|| "");
    let event_id = event.get("id").and_then(Value::as_str).unwrap_or("");
    let window_seconds = rule
        .get("window_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(60);
    let rule_id = rule
        .get("id")
        .map(value_to_trimmed_string)
        .unwrap_or_default();
    let rule_name = rule
        .get("name")
        .map(value_to_trimmed_string)
        .unwrap_or_default();
    json!({
        "title": title,
        "summary": details.summary,
        "body_text": details.body_text,
        "body_markdown": details.body_markdown,
        "severity": notification_severity(event.get("level").and_then(Value::as_str)),
        "facts": details.facts,
        "actions": [],
        "mentions": [],
        "dedupe_key": format!("{rule_id}:{group_key}"),
        "occurred_at": if happened_at.is_empty() { time_utils::now_iso() } else { happened_at.to_string() },
        "event_id": event_id,
        "metadata": {
            "event_type": event_type,
            "event_level": event.get("level").cloned().unwrap_or_else(|| json!("INFO")),
            "event_source": event.get("source").cloned().unwrap_or_else(|| json!("SERVER_ADMIN")),
            "rule_id": if rule_id.is_empty() { Value::Null } else { json!(rule_id) },
            "rule_name": if rule_name.is_empty() { Value::Null } else { json!(rule_name) },
            "group_key": group_key,
            "matched_count": matched_count,
            "window_seconds": window_seconds,
            "threshold_count": rule.get("threshold_count").cloned().unwrap_or_else(|| json!(1)),
            "locale": translator.locale()
        }
    })
}

fn notification_severity(level: Option<&str>) -> &'static str {
    match level {
        Some("CRITICAL") => "critical",
        Some("ERROR") => "error",
        Some("WARN") => "warn",
        _ => "info",
    }
}

struct NotificationDetails {
    summary: String,
    body_text: String,
    body_markdown: String,
    facts: Vec<Value>,
}

fn build_notification_details(
    event: &Value,
    rule: &Value,
    matched_count: i64,
    translator: &Translator,
) -> NotificationDetails {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    let window_seconds = rule
        .get("window_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(60);
    let aggregation =
        build_notification_aggregation_text(matched_count, window_seconds, translator);
    let mut facts = Vec::new();
    let mut summary = default_string(
        format_notification_summary(event, translator),
        &format_notification_event_label(event_type, translator),
    );
    let mut overview = summary.clone();
    let mut advice = String::new();

    match event_type {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => {
            let credential_name = default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            );
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let auth_method_raw = read_payload_value(event, "auth_method");
            let auth_provider_name = read_payload_value(event, "auth_provider_name");
            let auth_method = format_auth_method_label(&auth_method_raw, translator);
            let is_oidc_login = auth_method_raw == "OIDC";
            let login_method_text = if is_oidc_login && !auth_provider_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.loginViaProvider",
                    &[("provider", auth_provider_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.loginWithMethod",
                    &[(
                        "method",
                        default_string(
                            auth_method.clone(),
                            &notification_detail_text(translator, "unknownMethod", &[]),
                        ),
                    )],
                )
            };
            let login_auth_text = if is_oidc_login && !auth_provider_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.authViaProvider",
                    &[("provider", auth_provider_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.authWithMethod",
                    &[(
                        "method",
                        default_string(
                            auth_method.clone(),
                            &notification_detail_text(translator, "unknownMethod", &[]),
                        ),
                    )],
                )
            };
            let grant_type =
                format_grant_type_label(&read_payload_value(event, "grant_type"), translator);
            let remember_me =
                format_notification_bool(&read_payload_value(event, "remember_me"), translator);
            let expires_at = format_notification_datetime(&read_payload_value(event, "expires_at"));

            let base_summary = if is_oidc_login {
                let totp_part = if linked_totp_name.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "authLoginSuccess.linkedTotpPart",
                        &[("totp", linked_totp_name.clone())],
                    )
                };
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryOidc",
                    &[
                        ("credential", credential_name.clone()),
                        ("method", login_method_text),
                        ("ip", ip.clone()),
                        ("totpPart", totp_part),
                    ],
                )
            } else if !linked_totp_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryTotp",
                    &[
                        (
                            "method",
                            default_string(
                                auth_method.clone(),
                                &notification_template_text(translator, "credential", &[]),
                            ),
                        ),
                        ("credential", credential_name.clone()),
                        ("totp", linked_totp_name.clone()),
                        ("ip", ip.clone()),
                    ],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryCredential",
                    &[("credential", credential_name.clone()), ("ip", ip.clone())],
                )
            };
            summary = append_session_comment(base_summary, &session_comment, translator);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.locationPart",
                    &[("location", ip_location.clone())],
                )
            };
            let comment_part = session_comment_sentence(&session_comment, translator);
            overview = notification_detail_text(
                translator,
                "authLoginSuccess.overview",
                &[
                    ("auth", login_auth_text),
                    (
                        "grantType",
                        default_string(
                            grant_type.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    ("locationPart", location_part),
                    ("commentPart", comment_part),
                ],
            );
            advice = notification_detail_text(translator, "authLoginSuccess.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginProvider"),
                auth_provider_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "grantType"),
                grant_type,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "rememberLogin"),
                remember_me,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionExpiresAt"),
                expires_at,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_AUTH_LOGOUT" => {
            let credential_name = default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            );
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let auth_method =
                format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
            let logout_source =
                format_logout_source_label(&read_payload_value(event, "logout_source"), translator);

            let base_summary = if linked_totp_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLogout.summaryCredential",
                    &[("credential", credential_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLogout.summaryTotp",
                    &[
                        (
                            "method",
                            default_string(
                                auth_method.clone(),
                                &notification_template_text(translator, "credential", &[]),
                            ),
                        ),
                        ("credential", credential_name.clone()),
                        ("totp", linked_totp_name.clone()),
                    ],
                )
            };
            summary = append_session_comment(base_summary, &session_comment, translator);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "authLogout.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    (
                        "source",
                        default_string(
                            logout_source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "commentPart",
                        session_comment_sentence(&session_comment, translator),
                    ),
                ],
            );
            advice = notification_detail_text(translator, "authLogout.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "logoutSource"),
                logout_source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginTime"),
                format_notification_datetime(&read_payload_value(event, "login_time")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_AUTH_LOGIN_FAILURE" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let attempts = default_string(read_payload_value(event, "attempts"), "0");
            let retry_after = read_payload_value(event, "retry_after_seconds");
            let blocked_until =
                format_notification_datetime(&read_payload_value(event, "blocked_until"));
            let method = format_auth_method_label(&read_payload_value(event, "method"), translator);
            let credential_name = read_payload_value(event, "credential_name");
            let linked_totp_name = read_payload_value(event, "linked_totp_name");

            summary = notification_detail_text(
                translator,
                "authLoginFailure.summary",
                &[("ip", ip.clone()), ("attempts", attempts.clone())],
            );
            let retry_part = if retry_after.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginFailure.retryPart",
                    &[("seconds", retry_after.clone())],
                )
            };
            let blocked_part = if blocked_until.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginFailure.blockedPart",
                    &[("time", blocked_until.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "authLoginFailure.overview",
                &[
                    ("ip", ip.clone()),
                    ("retryPart", retry_part),
                    ("blockedPart", blocked_part),
                ],
            );
            advice = notification_detail_text(translator, "authLoginFailure.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                format_times(&attempts, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "retryWait"),
                format_seconds(&retry_after, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "limitUntil"),
                blocked_until,
            );
        }
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => {
            let credential_name = read_payload_value(event, "credential_name");
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let auth_method =
                format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
            let from_ip = default_string(
                read_payload_value(event, "from_ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let to_ip = default_string(
                read_payload_value(event, "to_ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let source =
                format_drift_source_label(&read_payload_value(event, "drift_source"), translator);
            let session_label = format_credential_context(
                event,
                &notification_detail_text(translator, "currentSession", &[]),
                translator,
            );

            summary = append_session_comment(
                notification_detail_text(
                    translator,
                    "authSessionIpDrift.summary",
                    &[
                        ("session", session_label.clone()),
                        ("fromIp", from_ip.clone()),
                        ("toIp", to_ip.clone()),
                    ],
                ),
                &session_comment,
                translator,
            );
            overview = notification_detail_text(
                translator,
                "authSessionIpDrift.overview",
                &[
                    ("session", session_label),
                    (
                        "source",
                        default_string(
                            source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "commentPart",
                        session_comment_sentence(&session_comment, translator),
                    ),
                ],
            );
            advice = notification_detail_text(translator, "authSessionIpDrift.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "originalIp"),
                from_ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "originalLocation"),
                read_payload_value(event, "from_ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentIp"),
                to_ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentLocation"),
                read_payload_value(event, "to_ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "driftSource"),
                source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginTime"),
                format_notification_datetime(&read_payload_value(event, "login_time")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let window_minutes = default_string(read_payload_value(event, "window_minutes"), "0");
            let hit_count = default_string(read_payload_value(event, "hit_count"), "0");
            let threshold = default_string(read_payload_value(event, "threshold"), "0");
            let scanner_paths = get_scanner_paths(event)
                .into_iter()
                .take(3)
                .collect::<Vec<_>>();

            summary = notification_detail_text(
                translator,
                "securityScannerBlocked.summary",
                &[("ip", ip.clone())],
            );
            let paths_part = if scanner_paths.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "securityScannerBlocked.pathsPart",
                    &[("paths", join_localized_list(&scanner_paths, translator))],
                )
            };
            overview = notification_detail_text(
                translator,
                "securityScannerBlocked.overview",
                &[
                    ("minutes", window_minutes.clone()),
                    ("hits", hit_count.clone()),
                    ("threshold", threshold.clone()),
                    ("pathsPart", paths_part),
                ],
            );
            advice = notification_detail_text(translator, "securityScannerBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                read_payload_value(event, "ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "hitCount"),
                format_times(&hit_count, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "observationWindow"),
                format_minutes(&window_minutes, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "triggerThreshold"),
                format_times(&threshold, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "recentPaths"),
                join_localized_list(&scanner_paths, translator),
            );
        }
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => {
            let target_name = default_string(
                read_payload_value(event, "target_name")
                    .if_empty(read_payload_value(event, "domain_summary")),
                &notification_detail_text(translator, "ddnsUpdateCompleted.defaultTarget", &[]),
            );
            let provider = default_string(
                read_payload_value(event, "provider"),
                &notification_detail_text(translator, "unknownProvider", &[]),
            );
            let success = read_payload_value(event, "success") == "true";
            let result_message = read_payload_value(event, "message");
            let trigger =
                format_ddns_trigger_label(&read_payload_value(event, "trigger"), translator);
            let update_scope = format_ddns_update_scope_label(
                &read_payload_value(event, "update_scope"),
                translator,
            );
            let ip_source =
                format_ddns_ip_source_label(&read_payload_value(event, "ip_source"), translator);
            let ipv4_change = format_ip_transition(
                &read_payload_value(event, "previous_ipv4"),
                &read_payload_value(event, "next_ipv4"),
            );
            let ipv6_change = format_ip_transition(
                &read_payload_value(event, "previous_ipv6"),
                &read_payload_value(event, "next_ipv6"),
            );

            summary = notification_detail_text(
                translator,
                if success {
                    "ddnsUpdateCompleted.summarySuccess"
                } else {
                    "ddnsUpdateCompleted.summaryFailure"
                },
                &[("target", target_name.clone())],
            );
            let result_part = if result_message.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "ddnsUpdateCompleted.resultPart",
                    &[("message", result_message.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "ddnsUpdateCompleted.overview",
                &[
                    (
                        "trigger",
                        default_string(
                            trigger.clone(),
                            &notification_detail_text(
                                translator,
                                "ddnsUpdateCompleted.currentTask",
                                &[],
                            ),
                        ),
                    ),
                    (
                        "scope",
                        default_string(
                            update_scope.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "ipSource",
                        default_string(
                            ip_source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    ("resultPart", result_part),
                ],
            );
            advice = notification_detail_text(
                translator,
                if success {
                    "ddnsUpdateCompleted.adviceSuccess"
                } else {
                    "ddnsUpdateCompleted.adviceFailure"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "target"),
                target_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "provider"),
                provider,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "targetType"),
                if read_payload_value(event, "is_primary") == "true" {
                    notification_detail_text(translator, "ddnsUpdateCompleted.primaryDomain", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "ddnsUpdateCompleted.additionalDomain",
                        &[],
                    )
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "trigger"),
                trigger,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "updateScope"),
                update_scope,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipSource"),
                ip_source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipv4Change"),
                ipv4_change,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipv6Change"),
                ipv6_change,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "result"),
                result_message,
            );
        }
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let block_seconds = default_string(read_payload_value(event, "block_seconds"), "0");
            let requests_per_second =
                default_string(read_payload_value(event, "requests_per_second"), "0");
            let burst = default_string(read_payload_value(event, "burst"), "0");
            let host = read_payload_value(event, "host");
            let path = read_payload_value(event, "path");

            summary = notification_detail_text(
                translator,
                "gatewayThrottleBlocked.summary",
                &[("ip", ip.clone()), ("seconds", block_seconds.clone())],
            );
            let target_part = if host.is_empty() && path.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "gatewayThrottleBlocked.targetPart",
                    &[("target", join_compact_parts(&[host.clone(), path.clone()]))],
                )
            };
            overview = notification_detail_text(
                translator,
                "gatewayThrottleBlocked.overview",
                &[
                    ("rate", requests_per_second.clone()),
                    ("burst", burst.clone()),
                    ("targetPart", target_part),
                ],
            );
            advice = notification_detail_text(translator, "gatewayThrottleBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockDuration"),
                format_seconds(&block_seconds, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedUntil"),
                format_notification_datetime(&read_payload_value(event, "blocked_until")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "rateLimit"),
                format_rate_per_second(&requests_per_second, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "burstCapacity"),
                burst,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "targetHost"),
                host,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestPath"),
                path,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "routeType"),
                read_payload_value(event, "route_type"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authRoute"),
                format_notification_bool(&read_payload_value(event, "is_auth_route"), translator),
            );
        }
        "FN_EVENT_WAF_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let host = read_payload_value(event, "host");
            let path = read_payload_value(event, "request_uri")
                .if_empty(read_payload_value(event, "path"));
            let rule_ids = read_payload_value(event, "rule_ids");
            let trace_id = read_payload_value(event, "trace_id");
            let action = read_payload_value(event, "action");
            let mode = read_payload_value(event, "mode");
            let action_label = format_waf_action_label(&action, translator);
            let mode_label = format_waf_mode_label(&mode, translator);
            let outcome_label = format_waf_outcome_label(&action, &mode, translator);
            let is_blocking = is_waf_blocking_action(&action, &mode);

            summary = notification_detail_text(
                translator,
                "wafBlocked.summary",
                &[("ip", ip.clone()), ("outcome", outcome_label.clone())],
            );
            let host_part = if host.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.hostPart",
                    &[("host", host.clone())],
                )
            };
            let path_part = if path.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.pathPart",
                    &[("path", path.clone())],
                )
            };
            let action_part = if action_label.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.actionPart",
                    &[("action", action_label.clone())],
                )
            };
            let mode_part = if mode_label.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.modePart",
                    &[("mode", mode_label.clone())],
                )
            };
            let rules_part = if rule_ids.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.rulesPart",
                    &[("rules", rule_ids.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "wafBlocked.overview",
                &[
                    ("outcome", outcome_label.clone()),
                    ("ip", ip.clone()),
                    ("hostPart", host_part),
                    ("pathPart", path_part),
                    ("actionPart", action_part),
                    ("modePart", mode_part),
                    ("rulesPart", rules_part),
                ],
            );
            advice = notification_detail_text(
                translator,
                if is_blocking {
                    "wafBlocked.adviceBlocked"
                } else {
                    "wafBlocked.adviceLogged"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "traceId"),
                trace_id,
            );
            push_notification_fact(&mut facts, "Host".to_string(), host);
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestAddress"),
                path,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "outcome"),
                outcome_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "wafAction"),
                action_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "wafMode"),
                mode_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ruleIds"),
                rule_ids,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ruleBundle"),
                read_payload_value(event, "bundle_id"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "statusCode"),
                read_payload_value(event, "status"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
        }
        "FN_EVENT_SSH_LOGIN_SUCCESS" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let username = default_string(
                read_payload_value(event, "username"),
                &notification_detail_text(translator, "unknownUser", &[]),
            );
            let auth_method = read_payload_value(event, "auth_method");

            summary = notification_detail_text(
                translator,
                "sshLoginSuccess.summary",
                &[("username", username.clone()), ("ip", ip.clone())],
            );
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            let auth_part = if auth_method.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "sshLoginSuccess.authPart",
                    &[("authMethod", auth_method.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshLoginSuccess.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    ("authPart", auth_part),
                ],
            );
            advice = notification_detail_text(translator, "sshLoginSuccess.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "user"),
                username,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "port"),
                read_payload_value(event, "port"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "logTime"),
                format_notification_datetime(&read_payload_value(event, "log_time")),
            );
        }
        "FN_EVENT_SSH_LOGIN_FAILURE" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let username = default_string(
                read_payload_value(event, "username"),
                &notification_detail_text(translator, "unknownUser", &[]),
            );
            let attempts = default_string(read_payload_value(event, "attempts"), "0");
            let threshold = default_string(read_payload_value(event, "threshold"), "0");
            let window_minutes = default_string(read_payload_value(event, "window_minutes"), "0");

            summary = notification_detail_text(
                translator,
                "sshLoginFailure.summary",
                &[("username", username.clone()), ("ip", ip.clone())],
            );
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "sshLoginFailure.locationPart",
                    &[("location", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshLoginFailure.overview",
                &[
                    ("minutes", window_minutes.clone()),
                    ("attempts", attempts.clone()),
                    ("threshold", threshold.clone()),
                    ("locationPart", location_part),
                ],
            );
            advice = notification_detail_text(translator, "sshLoginFailure.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "user"),
                username,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "invalidUser"),
                format_notification_bool(&read_payload_value(event, "invalid_user"), translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                read_payload_value(event, "auth_method"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "port"),
                read_payload_value(event, "port"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                attempts,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "threshold"),
                threshold,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "window"),
                format_minutes(&window_minutes, translator),
            );
        }
        "FN_EVENT_SSH_IP_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let reason = read_payload_value(event, "reason");
            let reason_label = if reason == "cidr_not_allowed" {
                notification_detail_text(translator, "sshIpBlocked.reasonCidrNotAllowed", &[])
            } else {
                notification_detail_text(translator, "sshIpBlocked.reasonFailedThreshold", &[])
            };

            summary =
                notification_detail_text(translator, "sshIpBlocked.summary", &[("ip", ip.clone())]);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshIpBlocked.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    ("reason", reason_label.clone()),
                ],
            );
            advice = notification_detail_text(translator, "sshIpBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedReason"),
                reason_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "relatedUser"),
                read_payload_value(event, "username"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                read_payload_value(event, "failed_count"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "window"),
                format_minutes(&read_payload_value(event, "window_minutes"), translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "threshold"),
                read_payload_value(event, "threshold"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedUntil"),
                format_notification_datetime(&read_payload_value(event, "blocked_until")),
            );
        }
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => {
            let local_version = default_string(
                read_payload_value(event, "local_version"),
                &notification_detail_text(
                    translator,
                    "appUpdateAvailable.currentVersionUnknown",
                    &[],
                ),
            );
            let latest_version = default_string(
                read_payload_value(event, "latest_version"),
                &notification_detail_text(
                    translator,
                    "appUpdateAvailable.targetVersionUnknown",
                    &[],
                ),
            );
            let force_update = read_payload_value(event, "force_update") == "true";
            let check_reason = format_update_check_reason_label(
                &read_payload_value(event, "check_reason"),
                translator,
            );
            let release_notes = truncate_notification_text(
                &read_payload_value(event, "release_notes"),
                APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH,
            );

            summary = notification_detail_text(
                translator,
                "appUpdateAvailable.summary",
                &[("version", latest_version.clone())],
            );
            let force_part = if force_update {
                notification_detail_text(translator, "appUpdateAvailable.forcePart", &[])
            } else {
                String::new()
            };
            overview = notification_detail_text(
                translator,
                "appUpdateAvailable.overview",
                &[
                    (
                        "reason",
                        default_string(
                            check_reason.clone(),
                            &notification_detail_text(
                                translator,
                                "appUpdateAvailable.currentCheck",
                                &[],
                            ),
                        ),
                    ),
                    ("localVersion", local_version.clone()),
                    ("latestVersion", latest_version.clone()),
                    ("forcePart", force_part),
                ],
            );
            advice = if release_notes.is_empty() {
                notification_detail_text(translator, "appUpdateAvailable.advice", &[])
            } else {
                notification_detail_text(
                    translator,
                    "appUpdateAvailable.releaseNotesAdvice",
                    &[("releaseNotes", release_notes.clone())],
                )
            };

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentVersion"),
                local_version,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "latestVersion"),
                latest_version,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "checkReason"),
                check_reason,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "forceUpdate"),
                if force_update {
                    notification_template_text(translator, "yes", &[])
                } else {
                    notification_template_text(translator, "no", &[])
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "releaseNotes"),
                release_notes,
            );
        }
        "FN_EVENT_SYSTEM_CPU_ALERT"
        | "FN_EVENT_SYSTEM_CPU_RECOVERED"
        | "FN_EVENT_SYSTEM_MEMORY_ALERT"
        | "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => {
            let is_cpu_event = event_type == "FN_EVENT_SYSTEM_CPU_ALERT"
                || event_type == "FN_EVENT_SYSTEM_CPU_RECOVERED";
            let recovered = event_type == "FN_EVENT_SYSTEM_CPU_RECOVERED"
                || event_type == "FN_EVENT_SYSTEM_MEMORY_RECOVERED";
            let metric_label = if is_cpu_event {
                "CPU".to_string()
            } else {
                notification_detail_text(translator, "memoryMetric", &[])
            };
            let hostname = default_string(
                read_payload_value(event, "hostname"),
                &notification_detail_text(translator, "unknownHost", &[]),
            );
            let usage_percent = default_string(read_payload_value(event, "usage_percent"), "0");
            let threshold_percent =
                default_string(read_payload_value(event, "threshold_percent"), "0");
            let recover_percent = default_string(read_payload_value(event, "recover_percent"), "0");

            summary = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredSummary"
                } else {
                    "systemMetric.alertSummary"
                },
                &[
                    ("hostname", hostname.clone()),
                    ("metric", metric_label.clone()),
                    ("usage", usage_percent.clone()),
                ],
            );
            overview = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredOverview"
                } else {
                    "systemMetric.alertOverview"
                },
                &[
                    ("hostname", hostname.clone()),
                    ("metric", metric_label),
                    ("usage", usage_percent.clone()),
                    ("recover", recover_percent.clone()),
                    ("threshold", threshold_percent.clone()),
                ],
            );
            advice = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredAdvice"
                } else {
                    "systemMetric.alertAdvice"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "hostname"),
                hostname,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentUsage"),
                format!("{usage_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "alertThreshold"),
                format!("{threshold_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "recoverThreshold"),
                format!("{recover_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sampleInterval"),
                format_seconds(
                    &read_payload_value(event, "sample_interval_seconds"),
                    translator,
                ),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sustainDuration"),
                format_seconds(&read_payload_value(event, "sustain_seconds"), translator),
            );
        }
        "FN_EVENT_TUNNEL_FRP_CONNECTED"
        | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => {
            let tunnel = tunnel_label(&read_payload_value(event, "tunnel"), event_type);
            let connected = read_payload_value(event, "status") == "connected";
            let runtime_message =
                truncate_notification_text(&read_payload_value(event, "message"), 200);
            let pid = read_payload_value(event, "pid");

            summary = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedSummary"
                } else {
                    "tunnel.disconnectedSummary"
                },
                &[("tunnel", tunnel.clone())],
            );
            let message_part = if runtime_message.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    if connected {
                        "tunnel.connectedMessagePart"
                    } else {
                        "tunnel.disconnectedMessagePart"
                    },
                    &[("message", runtime_message.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedOverview"
                } else {
                    "tunnel.disconnectedOverview"
                },
                &[("tunnel", tunnel.clone()), ("messagePart", message_part)],
            );
            advice = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedAdvice"
                } else {
                    "tunnel.disconnectedAdvice"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "tunnelType"),
                tunnel,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "connectionStatus"),
                if connected {
                    notification_detail_text(translator, "connected", &[])
                } else {
                    notification_detail_text(translator, "disconnected", &[])
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "processPid"),
                pid,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "runtimeFeedback"),
                runtime_message,
            );
        }
        _ => {}
    }

    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "eventType"),
        format_notification_event_label(event_type, translator),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "riskLevel"),
        format_notification_level_label(
            event.get("level").and_then(Value::as_str).unwrap_or("INFO"),
            translator,
        ),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "eventSource"),
        format_notification_source_label(
            event
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("SERVER_ADMIN"),
            translator,
        ),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "happenedAt"),
        format_notification_datetime(
            event
                .get("happened_at")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    );
    if matched_count > 1 {
        push_notification_fact(
            &mut facts,
            notification_fact_label(translator, "aggregationStats"),
            notification_detail_text(
                translator,
                "aggregationStatsValue",
                &[
                    ("count", matched_count.to_string()),
                    ("seconds", window_seconds.to_string()),
                ],
            ),
        );
    }

    NotificationDetails {
        summary: summary.trim().to_string(),
        body_text: build_notification_body_text(&overview, &aggregation, &advice),
        body_markdown: build_notification_body_markdown(
            &overview,
            &aggregation,
            &advice,
            translator,
        ),
        facts,
    }
}

fn build_notification_aggregation_text(
    matched_count: i64,
    window_seconds: i64,
    translator: &Translator,
) -> String {
    if matched_count <= 1 {
        return String::new();
    }
    notification_template_text(
        translator,
        "aggregationText",
        &[
            ("count", matched_count.to_string()),
            ("seconds", window_seconds.to_string()),
        ],
    )
}

fn build_notification_body_text(overview: &str, aggregation: &str, advice: &str) -> String {
    [overview, aggregation, advice]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_notification_body_markdown(
    overview: &str,
    aggregation: &str,
    advice: &str,
    translator: &Translator,
) -> String {
    let mut sections = Vec::new();
    if !overview.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.overview", &[]),
            overview.trim()
        ));
    }
    if !aggregation.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.aggregation", &[]),
            aggregation.trim()
        ));
    }
    if !advice.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.advice", &[]),
            advice.trim()
        ));
    }
    sections.join("\n\n")
}

fn read_payload_value(event: &Value, key: &str) -> String {
    event
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get(key))
        .map(value_to_notification_text)
        .unwrap_or_default()
}

fn value_to_notification_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(values) => values
            .iter()
            .map(value_to_notification_text)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn push_notification_fact(facts: &mut Vec<Value>, label: String, value: String) {
    let label = label.trim();
    let value = value.trim();
    if label.is_empty() && value.is_empty() {
        return;
    }
    facts.push(json!({ "label": label, "value": value }));
}

fn format_seconds(value: &str, translator: &Translator) -> String {
    format_unit("seconds", value, translator)
}

fn format_minutes(value: &str, translator: &Translator) -> String {
    format_unit("minutes", value, translator)
}

fn format_times(value: &str, translator: &Translator) -> String {
    format_unit("times", value, translator)
}

fn format_rate_per_second(value: &str, translator: &Translator) -> String {
    format_unit("ratePerSecond", value, translator)
}

fn format_unit(key: &str, value: &str, translator: &Translator) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    notification_detail_text(
        translator,
        &format!("units.{key}"),
        &[("count", value.to_string())],
    )
}

fn join_localized_list(values: &[String], translator: &Translator) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(&notification_detail_text(translator, "listSeparator", &[]))
}

fn join_compact_parts(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn format_ip_transition(previous_ip: &str, next_ip: &str) -> String {
    let previous_ip = previous_ip.trim();
    let next_ip = next_ip.trim();
    if !previous_ip.is_empty() && !next_ip.is_empty() {
        format!("{previous_ip} -> {next_ip}")
    } else if !previous_ip.is_empty() {
        previous_ip.to_string()
    } else {
        next_ip.to_string()
    }
}

fn read_session_comment(event: &Value, translator: &Translator) -> String {
    normalize_auto_ip_grant_comment(&read_payload_value(event, "session_comment"), translator)
}

fn normalize_auto_ip_grant_comment(value: &str, translator: &Translator) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let known = [
        "auth.autoIpGrantComment",
        "server.auth.autoIpGrantComment",
        "登录后自动授权",
        "登入後自動授權",
        "Automatically authorized after sign-in",
        "로그인 후 자동 승인됨",
        "ログイン後自動認証",
    ];
    if known.contains(&value) {
        translator.t("auth.autoIpGrantComment")
    } else {
        value.to_string()
    }
}

fn append_session_comment(text: String, session_comment: &str, translator: &Translator) -> String {
    if session_comment.trim().is_empty() {
        text
    } else {
        notification_template_text(
            translator,
            "appendSessionComment",
            &[
                ("text", text),
                (
                    "comment",
                    normalize_auto_ip_grant_comment(session_comment, translator),
                ),
            ],
        )
    }
}

fn session_comment_sentence(session_comment: &str, translator: &Translator) -> String {
    if session_comment.trim().is_empty() {
        String::new()
    } else {
        notification_detail_text(
            translator,
            "sessionCommentSentence",
            &[(
                "comment",
                normalize_auto_ip_grant_comment(session_comment, translator),
            )],
        )
    }
}

fn format_session_comment_compact(value: &str, translator: &Translator) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        notification_template_text(
            translator,
            "sessionCommentCompact",
            &[(
                "comment",
                normalize_auto_ip_grant_comment(value, translator),
            )],
        )
    }
}

fn format_credential_context(event: &Value, fallback: &str, translator: &Translator) -> String {
    let credential_name = read_payload_value(event, "credential_name");
    let linked_totp_name = read_payload_value(event, "linked_totp_name");
    let auth_method =
        format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
    if !linked_totp_name.is_empty() {
        return notification_template_text(
            translator,
            "credentialLinkedTotp",
            &[
                (
                    "authMethod",
                    default_string(
                        auth_method,
                        &notification_template_text(translator, "credential", &[]),
                    ),
                ),
                (
                    "credential",
                    default_string(
                        credential_name,
                        &notification_template_text(translator, "unknownCredential", &[]),
                    ),
                ),
                ("totp", linked_totp_name),
            ],
        );
    }
    if !credential_name.is_empty() {
        return notification_template_text(
            translator,
            "credentialName",
            &[("credential", credential_name)],
        );
    }
    fallback.to_string()
}

fn format_notification_bool(value: &str, translator: &Translator) -> String {
    match value.trim() {
        "true" => notification_template_text(translator, "yes", &[]),
        "false" => notification_template_text(translator, "no", &[]),
        other => other.to_string(),
    }
}

fn format_notification_datetime(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let Some(ms) = time_utils::parse_iso_ms(value) else {
        return value.to_string();
    };
    let Ok(utc) = ::time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000)) else {
        return value.to_string();
    };
    let local = ::time::UtcOffset::current_local_offset()
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    )
}

fn get_scanner_paths(event: &Value) -> Vec<String> {
    event
        .get("payload")
        .and_then(|payload| payload.get("hits"))
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| hit.get("path"))
                .map(value_to_notification_text)
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn format_notification_summary(event: &Value, translator: &Translator) -> String {
    match event.get("type").and_then(Value::as_str).unwrap_or("") {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => {
            let auth_method = read_payload_value(event, "auth_method");
            let auth_provider_name = read_payload_value(event, "auth_provider_name");
            if auth_method == "OIDC" && !auth_provider_name.is_empty() {
                return join_compact_parts(&[
                    notification_detail_text(
                        translator,
                        "authLoginSuccess.loginViaProvider",
                        &[("provider", auth_provider_name)],
                    ),
                    default_string(
                        read_payload_value(event, "credential_name"),
                        &notification_template_text(translator, "unknownCredential", &[]),
                    ),
                    format_session_comment_compact(
                        &read_session_comment(event, translator),
                        translator,
                    ),
                    read_payload_value(event, "ip"),
                ]);
            }
            join_compact_parts(&[
                default_string(
                    read_payload_value(event, "credential_name"),
                    &notification_template_text(translator, "unknownCredential", &[]),
                ),
                format_session_comment_compact(
                    &read_session_comment(event, translator),
                    translator,
                ),
                read_payload_value(event, "ip"),
            ])
        }
        "FN_EVENT_AUTH_LOGOUT" => join_compact_parts(&[
            default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            ),
            format_session_comment_compact(&read_session_comment(event, translator), translator),
            read_payload_value(event, "ip"),
        ]),
        "FN_EVENT_AUTH_LOGIN_FAILURE" => {
            let attempts = read_payload_value(event, "attempts");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if attempts.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "short.loginFailureAttempts",
                        &[("count", attempts)],
                    )
                },
            ])
        }
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => join_compact_parts(&[
            format_credential_context(event, "", translator),
            format_session_comment_compact(&read_session_comment(event, translator), translator),
            format_ip_transition(
                &read_payload_value(event, "from_ip"),
                &read_payload_value(event, "to_ip"),
            ),
        ]),
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => {
            let hit_count = read_payload_value(event, "hit_count");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if hit_count.is_empty() {
                    notification_detail_text(translator, "short.scanBlocked", &[])
                } else {
                    notification_detail_text(translator, "short.scanHits", &[("count", hit_count)])
                },
            ])
        }
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => join_compact_parts(&[
            read_payload_value(event, "target_name")
                .if_empty(read_payload_value(event, "domain_summary"))
                .if_empty(read_payload_value(event, "provider")),
            if read_payload_value(event, "success") == "true" {
                notification_detail_text(translator, "short.success", &[])
            } else {
                notification_detail_text(translator, "short.failure", &[])
            },
        ]),
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => {
            let seconds = read_payload_value(event, "block_seconds");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if seconds.is_empty() {
                    notification_detail_text(translator, "short.blockTriggered", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "short.blockSeconds",
                        &[("seconds", seconds)],
                    )
                },
            ])
        }
        "FN_EVENT_WAF_BLOCKED" => {
            let rule_ids = read_payload_value(event, "rule_ids");
            let outcome = format_waf_outcome_label(
                &read_payload_value(event, "action"),
                &read_payload_value(event, "mode"),
                translator,
            );
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                read_payload_value(event, "host"),
                format!("WAF {outcome}"),
                if rule_ids.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(translator, "short.rules", &[("rules", rule_ids)])
                },
            ])
        }
        "FN_EVENT_SSH_LOGIN_SUCCESS" => join_compact_parts(&[
            read_payload_value(event, "username"),
            read_payload_value(event, "ip"),
            notification_detail_text(translator, "short.sshLoginSuccess", &[]),
        ]),
        "FN_EVENT_SSH_LOGIN_FAILURE" => {
            let attempts = read_payload_value(event, "attempts");
            join_compact_parts(&[
                read_payload_value(event, "username"),
                read_payload_value(event, "ip"),
                if attempts.is_empty() {
                    notification_detail_text(translator, "short.sshLoginFailure", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "short.loginFailureAttempts",
                        &[("count", attempts)],
                    )
                },
            ])
        }
        "FN_EVENT_SSH_IP_BLOCKED" => join_compact_parts(&[
            read_payload_value(event, "ip"),
            if read_payload_value(event, "reason") == "cidr_not_allowed" {
                notification_detail_text(translator, "short.regionNotAllowed", &[])
            } else {
                notification_detail_text(translator, "short.failureThreshold", &[])
            },
        ]),
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => {
            let local_version = read_payload_value(event, "local_version");
            join_compact_parts(&[
                read_payload_value(event, "latest_version"),
                if local_version.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "short.currentVersion",
                        &[("version", local_version)],
                    )
                },
            ])
        }
        "FN_EVENT_SYSTEM_CPU_ALERT"
        | "FN_EVENT_SYSTEM_CPU_RECOVERED"
        | "FN_EVENT_SYSTEM_MEMORY_ALERT"
        | "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => join_compact_parts(&[
            read_payload_value(event, "hostname"),
            if read_payload_value(event, "usage_percent").is_empty() {
                String::new()
            } else {
                format!("{}%", read_payload_value(event, "usage_percent"))
            },
        ]),
        "FN_EVENT_TUNNEL_FRP_CONNECTED"
        | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => join_compact_parts(&[
            tunnel_label(
                &read_payload_value(event, "tunnel"),
                event.get("type").and_then(Value::as_str).unwrap_or(""),
            ),
            if read_payload_value(event, "status") == "connected" {
                notification_detail_text(translator, "connected", &[])
            } else {
                notification_detail_text(translator, "disconnected", &[])
            },
        ]),
        _ => String::new(),
    }
}

fn translate_notification_label(
    value: &str,
    labels: &[(&str, &str)],
    translator: &Translator,
) -> String {
    let value = value.trim();
    labels
        .iter()
        .find_map(|(candidate, key)| {
            (*candidate == value).then(|| notification_template_text(translator, key, &[]))
        })
        .unwrap_or_else(|| value.to_string())
}

fn format_auth_method_label(value: &str, translator: &Translator) -> String {
    match value.trim() {
        "TOTP" => "TOTP".to_string(),
        "PASSKEY" => "Passkey".to_string(),
        "OIDC" => notification_template_text(translator, "authMethods.oidc", &[]),
        other => other.to_string(),
    }
}

fn format_grant_type_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("browser_session", "grantTypes.browserSession"),
            ("login_ip_grant", "grantTypes.loginIpGrant"),
        ],
        translator,
    )
}

fn format_logout_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("user_logout", "logoutSources.userLogout"),
            ("admin_session_delete", "logoutSources.adminSessionDelete"),
        ],
        translator,
    )
}

fn format_drift_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("proxy-session", "driftSources.proxySession"),
            ("fnos-token", "driftSources.fnosToken"),
            ("session-refresh", "driftSources.sessionRefresh"),
            ("browser-session", "driftSources.browserSession"),
        ],
        translator,
    )
}

fn format_ddns_trigger_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("cron", "ddnsTriggers.cron"),
            ("enable", "ddnsTriggers.enable"),
            ("startup", "ddnsTriggers.startup"),
            ("manual_test", "ddnsTriggers.manualTest"),
        ],
        translator,
    )
}

fn format_ddns_update_scope_label(value: &str, translator: &Translator) -> String {
    if value.trim() == "dual_stack" {
        "IPv4 + IPv6".to_string()
    } else {
        translate_notification_label(
            value,
            &[
                ("ipv4_only", "ddnsUpdateScopes.ipv4Only"),
                ("ipv6_only", "ddnsUpdateScopes.ipv6Only"),
            ],
            translator,
        )
    }
}

fn format_ddns_ip_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("public", "ddnsIpSources.public"),
            ("interface", "ddnsIpSources.interface"),
            ("static", "ddnsIpSources.static"),
            ("domain", "ddnsIpSources.domain"),
        ],
        translator,
    )
}

fn format_update_check_reason_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("cron", "updateCheckReasons.cron"),
            ("manual", "updateCheckReasons.manual"),
            (
                "manual-check-and-download",
                "updateCheckReasons.manualCheckAndDownload",
            ),
            ("download-bootstrap", "updateCheckReasons.downloadBootstrap"),
        ],
        translator,
    )
}

fn format_waf_action_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("block", "wafActions.block"),
            ("deny", "wafActions.deny"),
            ("detect", "wafActions.detect"),
            ("log", "wafActions.log"),
            ("pass", "wafActions.pass"),
        ],
        translator,
    )
}

fn format_waf_mode_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("detection", "wafModes.detection"),
            ("blocking", "wafModes.blocking"),
            ("off", "wafModes.off"),
        ],
        translator,
    )
}

fn is_waf_blocking_action(action: &str, mode: &str) -> bool {
    let action = action.trim().to_ascii_lowercase();
    if action == "block" || action == "deny" {
        return true;
    }
    if matches!(action.as_str(), "detect" | "log" | "pass") {
        return false;
    }
    mode.trim().eq_ignore_ascii_case("blocking")
}

fn format_waf_outcome_label(action: &str, mode: &str, translator: &Translator) -> String {
    if is_waf_blocking_action(action, mode) {
        notification_template_text(translator, "wafOutcomeBlocked", &[])
    } else {
        let action_label = format_waf_action_label(action, translator);
        default_string(
            action_label,
            &notification_template_text(translator, "wafOutcomeLogged", &[]),
        )
    }
}

fn tunnel_label(value: &str, event_type: &str) -> String {
    match value.trim() {
        "frp" => "FRP".to_string(),
        "cloudflared" => "Cloudflared".to_string(),
        "" if event_type.contains("CLOUDFLARED") => "Cloudflared".to_string(),
        "" => "FRP".to_string(),
        other => other.to_string(),
    }
}

fn truncate_notification_text(value: &str, max_len: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        normalized
    } else {
        format!(
            "{}...",
            normalized.chars().take(max_len).collect::<String>().trim()
        )
    }
}

fn brand_notification_title(title: &str, translator: &Translator) -> String {
    let prefix = translator.t("server.notifications.brand.prefix");
    let default_title = translator.t("server.notifications.brand.defaultTitle");
    let title = title.trim();
    if title.is_empty() {
        default_title
    } else if title.starts_with(&prefix) {
        title.to_string()
    } else {
        format!("{prefix}{title}")
    }
}

struct DeliveryBuildArgs {
    id: String,
    trigger_id: String,
    rule_id: String,
    target_id: String,
    provider_id: String,
    event_id: String,
    status: String,
    reason: Option<String>,
    provider_type: String,
    message_snapshot: Value,
    target_snapshot: Value,
    provider_snapshot: Value,
    attempt_count: i64,
    triggered_at: String,
    next_retry_at: Option<String>,
}

fn build_delivery_value(args: DeliveryBuildArgs) -> Value {
    json!({
        "id": args.id,
        "trigger_id": args.trigger_id,
        "rule_id": args.rule_id,
        "target_id": args.target_id,
        "provider_id": args.provider_id,
        "event_id": args.event_id,
        "status": args.status,
        "reason": args.reason,
        "provider_type": args.provider_type,
        "message_snapshot": args.message_snapshot,
        "target_snapshot": args.target_snapshot,
        "provider_snapshot": args.provider_snapshot,
        "request_summary": Value::Null,
        "response_summary": Value::Null,
        "attempt_count": args.attempt_count,
        "triggered_at": args.triggered_at,
        "sent_at": Value::Null,
        "next_retry_at": args.next_retry_at
    })
}

fn deleted_provider_snapshot(provider_id: &str, timestamp: &str, translator: &Translator) -> Value {
    json!({
        "id": provider_id,
        "name": notification_service_text(translator, "deletedProvider", &[]),
        "type": "webhook",
        "enabled": false,
        "created_at": timestamp,
        "updated_at": timestamp,
        "connection_config_masked": {}
    })
}

fn is_terminal_delivery_status(status: Option<&str>) -> bool {
    matches!(status, Some("success" | "gave_up" | "skipped"))
}

async fn refresh_trigger_status(state: &AppState, trigger_id: &str) -> anyhow::Result<()> {
    if trigger_id.is_empty() {
        return Ok(());
    }
    let Some(trigger) = load_trigger(state, trigger_id).await? else {
        return Ok(());
    };
    let (deliveries, _) = list_history(
        state,
        DELIVERIES_INDEX_KEY,
        DELIVERIES_DATA_PREFIX,
        1,
        i64::MAX,
        |delivery| delivery.get("trigger_id").and_then(Value::as_str) == Some(trigger_id),
    )
    .await?;
    if deliveries.is_empty() {
        return Ok(());
    }
    if deliveries.iter().any(|delivery| {
        !is_terminal_delivery_status(delivery.get("status").and_then(Value::as_str))
    }) {
        return Ok(());
    }
    let all_succeeded = deliveries.iter().all(|delivery| {
        matches!(
            delivery.get("status").and_then(Value::as_str),
            Some("success" | "skipped")
        )
    });
    let mut updated = trigger.as_object().cloned().unwrap_or_default();
    updated.insert(
        "status".to_string(),
        Value::String(if all_succeeded {
            "completed".to_string()
        } else {
            "partially_failed".to_string()
        }),
    );
    save_trigger_raw(state, &Value::Object(updated)).await?;
    Ok(())
}

fn find_rule_target(rule: &Value, target_id: &str) -> Option<Value> {
    rule.get("targets")
        .and_then(Value::as_array)?
        .iter()
        .find(|target| target.get("id").and_then(Value::as_str) == Some(target_id))
        .cloned()
}

struct DeliveryPolicy {
    timeout_seconds: i64,
    max_attempts: i64,
    backoff_seconds: i64,
}

fn resolve_delivery_policy(value: Option<&Value>) -> DeliveryPolicy {
    let object = value.and_then(Value::as_object);
    DeliveryPolicy {
        timeout_seconds: object
            .and_then(|value| value.get("timeout_seconds"))
            .map(|value| value_to_i64(value, 5))
            .unwrap_or(5)
            .clamp(1, 30),
        max_attempts: object
            .and_then(|value| value.get("max_attempts"))
            .map(|value| value_to_i64(value, 3))
            .unwrap_or(3)
            .clamp(1, 10),
        backoff_seconds: object
            .and_then(|value| value.get("backoff_seconds"))
            .map(|value| value_to_i64(value, 30))
            .unwrap_or(30)
            .clamp(5, 3600),
    }
}

async fn load_indexed_values(
    state: &AppState,
    index_key: &str,
    data_prefix: &str,
) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(index_key).await?;
    let mut values = Vec::new();
    let mut stale_ids = Vec::new();
    for id in ids {
        match state
            .redis
            .get_json_value(&format!("{data_prefix}{id}"))
            .await?
        {
            Some(value) => values.push(value),
            None => stale_ids.push(id),
        }
    }
    for id in stale_ids {
        state.redis.zrem_string_member(index_key, &id).await?;
    }
    Ok(values)
}

async fn list_history<F>(
    state: &AppState,
    index_key: &str,
    data_prefix: &str,
    page: i64,
    limit: i64,
    matches: F,
) -> redis::RedisResult<(Vec<Value>, i64)>
where
    F: Fn(&Value) -> bool,
{
    let ids = state.redis.zrevrange_strings(index_key).await?;
    let page_start = (page.saturating_sub(1)).saturating_mul(limit);
    let mut matched_total = 0_i64;
    let mut items = Vec::new();
    let mut stale_ids = Vec::new();

    for id in ids {
        let value = state
            .redis
            .get_json_value(&format!("{data_prefix}{id}"))
            .await?;
        let Some(value) = value else {
            stale_ids.push(id);
            continue;
        };
        if !matches(&value) {
            continue;
        }
        if matched_total >= page_start && (items.len() as i64) < limit {
            items.push(value);
        }
        matched_total += 1;
    }
    for id in stale_ids {
        state.redis.zrem_string_member(index_key, &id).await?;
    }
    Ok((items, matched_total))
}

struct ClearDeliveryFilter {
    rule_id: Option<String>,
    provider_id: Option<String>,
    trigger_id: Option<String>,
    status: Option<String>,
}

async fn clear_delivery_values(
    state: &AppState,
    filter: ClearDeliveryFilter,
) -> redis::RedisResult<usize> {
    let ids = state.redis.zrevrange_strings(DELIVERIES_INDEX_KEY).await?;
    let mut matched_ids = Vec::new();
    let mut stale_ids = Vec::new();
    for id in ids {
        match state.redis.get_json_value(&delivery_key(&id)).await? {
            Some(value) => {
                if matches_optional_string(&value, "rule_id", filter.rule_id.as_deref())
                    && matches_optional_string(&value, "provider_id", filter.provider_id.as_deref())
                    && matches_optional_string(&value, "trigger_id", filter.trigger_id.as_deref())
                    && matches_optional_string(&value, "status", filter.status.as_deref())
                {
                    matched_ids.push(id);
                }
            }
            None => stale_ids.push(id),
        }
    }

    let delete_keys = matched_ids
        .iter()
        .map(|id| delivery_key(id))
        .collect::<Vec<_>>();
    state.redis.delete_keys(&delete_keys).await?;
    for id in stale_ids.iter().chain(matched_ids.iter()) {
        state
            .redis
            .zrem_string_member(DELIVERIES_INDEX_KEY, id)
            .await?;
    }
    for id in &matched_ids {
        state
            .redis
            .zrem_string_member(DELIVERIES_READY_KEY, id)
            .await?;
    }
    Ok(matched_ids.len())
}

#[derive(Debug)]
enum NotifyError {
    BadRequest(String),
    Redis(redis::RedisError),
}

type NotifyResult<T> = Result<T, NotifyError>;

impl From<redis::RedisError> for NotifyError {
    fn from(value: redis::RedisError) -> Self {
        Self::Redis(value)
    }
}

trait OptionBadRequest<T> {
    fn ok_or_bad<S: Into<String>>(self, message: S) -> NotifyResult<T>;
}

impl<T> OptionBadRequest<T> for Option<T> {
    fn ok_or_bad<S: Into<String>>(self, message: S) -> NotifyResult<T> {
        let message = message.into();
        self.ok_or_else(|| NotifyError::BadRequest(message))
    }
}

fn provider_key(id: &str) -> String {
    format!("{PROVIDERS_DATA_PREFIX}{id}")
}

fn rule_key(id: &str) -> String {
    format!("{RULES_DATA_PREFIX}{id}")
}

fn delivery_key(id: &str) -> String {
    format!("{DELIVERIES_DATA_PREFIX}{id}")
}

fn provider_definition(provider_type: &str) -> Option<ProviderDefinition> {
    match provider_type {
        "webhook" => Some(ProviderDefinition {
            provider_type: "webhook",
            label: "Webhook",
            description: "Send a JSON payload to a custom webhook endpoint.",
            connection_schema: vec![
                string_schema("url", "Webhook URL", true, true, None)
                    .placeholder("https://example.com/hooks/fn-knock"),
                select_schema("method", "Method", true, Some("POST"), &["POST", "PUT"]),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
                string_schema("shared_secret", "Shared secret", false, true, None)
                    .placeholder("secret"),
            ],
            target_schema: vec![
                string_schema("endpoint_path", "Endpoint path", false, false, None)
                    .placeholder("/alerts"),
                json_schema("extra_headers_json", "Extra headers", false)
                    .placeholder(r#"{"X-Env":"prod"}"#),
                json_schema("extra_body_json", "Extra body", false)
                    .placeholder(r#"{"service":"gateway"}"#),
            ],
            sensitive_fields: vec!["url", "shared_secret"],
            supports_markdown: true,
            supports_actions: true,
            supports_mentions: true,
            supports_provider_dedupe_key: true,
        }),
        "wxpusher" => Some(ProviderDefinition {
            provider_type: "wxpusher",
            label: "WxPusher",
            description: "Send notifications through WxPusher.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://wxpusher.zjiecode.com"),
                )
                .placeholder("https://wxpusher.zjiecode.com"),
                string_schema("app_token", "AppToken", true, true, None).placeholder("AT_xxx"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
                string_schema("uids", "UIDs", false, false, None).placeholder("UID_xxx,UID_yyy"),
                string_schema("topic_ids", "Topic IDs", false, false, None).placeholder("123,456"),
                string_schema("url", "URL", false, false, None)
                    .placeholder("https://example.com/events/123"),
                select_schema(
                    "verify_pay_type",
                    "Verify pay type",
                    false,
                    Some("0"),
                    &["0", "1", "2"],
                ),
            ],
            target_schema: vec![
                string_schema("uids", "UIDs", false, false, None).placeholder("UID_xxx,UID_yyy"),
                string_schema("topic_ids", "Topic IDs", false, false, None).placeholder("123,456"),
                string_schema("url", "URL", false, false, None)
                    .placeholder("https://example.com/events/123"),
                select_schema(
                    "verify_pay_type",
                    "Verify pay type",
                    false,
                    Some("__inherit__"),
                    &["__inherit__", "0", "1", "2"],
                ),
            ],
            sensitive_fields: vec!["app_token"],
            supports_markdown: true,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "serverchan" => Some(ProviderDefinition {
            provider_type: "serverchan",
            label: "ServerChan",
            description: "Send notifications through ServerChan.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://sctapi.ftqq.com"),
                )
                .placeholder("https://sctapi.ftqq.com"),
                string_schema("sendkey", "SendKey", true, true, None)
                    .placeholder("SCTxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: vec![
                string_schema("channel", "Channel", false, false, None).placeholder("9|66"),
                string_schema("openid", "OpenID / UID", false, false, None)
                    .placeholder("openid1,openid2 or uid1|uid2"),
                string_schema("short", "Short text", false, false, None)
                    .placeholder("Login anomaly, please check"),
                bool_schema("noip", "Hide caller IP", false, Some(false)),
            ],
            sensitive_fields: vec!["sendkey"],
            supports_markdown: true,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "pushplus" => Some(ProviderDefinition {
            provider_type: "pushplus",
            label: "PushPlus",
            description: "Send notifications through PushPlus.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://www.pushplus.plus"),
                )
                .placeholder("https://www.pushplus.plus"),
                string_schema("token", "Token", true, true, None)
                    .placeholder("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: vec![
                string_schema("topic", "Topic", false, false, None).placeholder("alarm-topic"),
                select_schema(
                    "template",
                    "Template",
                    false,
                    Some("markdown"),
                    &["markdown", "html", "txt", "json"],
                ),
                select_schema(
                    "channel",
                    "Channel",
                    false,
                    Some("wechat"),
                    &[
                        "wechat",
                        "webhook",
                        "cp",
                        "mail",
                        "sms",
                        "voice",
                        "extension",
                        "app",
                        "clawbot",
                    ],
                ),
                string_schema("option", "Option", false, false, None)
                    .placeholder("my-channel-code"),
                string_schema("to", "Recipient", false, false, None)
                    .placeholder("friend_token or user1,user2"),
                string_schema("callback_url", "Callback URL", false, false, None)
                    .placeholder("https://example.com/hooks/pushplus"),
                string_schema("pre", "Pre", false, false, None).placeholder("appendMsg"),
            ],
            sensitive_fields: vec!["token"],
            supports_markdown: true,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "wecom" => Some(webhook_like_definition(
            "wecom",
            "WeCom",
            "Send notifications through WeCom robot webhook.",
            &["webhook_url"],
            vec![
                string_schema("mentioned_list", "Mentioned users", false, false, None)
                    .placeholder("zhangsan,@all"),
                string_schema(
                    "mentioned_mobile_list",
                    "Mentioned mobile list",
                    false,
                    false,
                    None,
                )
                .placeholder("13800001111,@all"),
            ],
        )),
        "dingtalk" => Some(webhook_like_definition(
            "dingtalk",
            "DingTalk",
            "Send notifications through DingTalk robot webhook.",
            &["webhook_url", "secret"],
            vec![
                string_schema("at_mobiles", "At mobiles", false, false, None)
                    .placeholder("13800001111,13900002222"),
                string_schema("at_user_ids", "At user IDs", false, false, None)
                    .placeholder("manager7675,user123"),
                bool_schema("is_at_all", "At all", false, Some(false)),
            ],
        )),
        "feishu" => Some(webhook_like_definition(
            "feishu",
            "Feishu",
            "Send notifications through Feishu robot webhook.",
            &["webhook_url", "secret"],
            vec![
                string_schema("mention_user_ids", "Mention user IDs", false, false, None)
                    .placeholder("ou_xxx,all"),
            ],
        )),
        "email" => Some(ProviderDefinition {
            provider_type: "email",
            label: "Email",
            description: "Send notifications through SMTP.",
            connection_schema: vec![
                string_schema("smtp_host", "SMTP host", true, false, None)
                    .placeholder("smtp.example.com"),
                number_schema("smtp_port", "SMTP port", true, Some(465)).bounds(1, 65535),
                select_schema(
                    "smtp_security",
                    "SMTP security",
                    true,
                    Some("ssl_tls"),
                    &["ssl_tls", "starttls", "none"],
                ),
                select_schema(
                    "smtp_auth_mode",
                    "SMTP auth mode",
                    true,
                    Some("auto"),
                    &["auto", "plain", "login", "none"],
                ),
                string_schema("smtp_username", "SMTP username", false, false, None)
                    .placeholder("no-reply@example.com"),
                string_schema("smtp_password", "SMTP password", false, true, None)
                    .placeholder("password"),
                string_schema("from_address", "From address", true, false, None)
                    .placeholder("no-reply@example.com"),
                string_schema("from_name", "From name", false, false, None).placeholder("fn-knock"),
                string_schema("to_addresses", "To addresses", true, false, None)
                    .placeholder("ops@example.com, admin@example.com"),
                string_schema("cc_addresses", "CC addresses", false, false, None)
                    .placeholder("audit@example.com"),
                string_schema("bcc_addresses", "BCC addresses", false, false, None)
                    .placeholder("archive@example.com"),
                string_schema("reply_to", "Reply-To", false, false, None)
                    .placeholder("support@example.com"),
                bool_schema("allow_invalid_tls", "Allow invalid TLS", false, Some(false)),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(10)).bounds(1, 30),
                string_schema("imap_host", "IMAP host", false, false, None)
                    .placeholder("imap.example.com"),
                number_schema("imap_port", "IMAP port", false, Some(993)).bounds(1, 65535),
                select_schema(
                    "imap_security",
                    "IMAP security",
                    false,
                    Some("ssl_tls"),
                    &["ssl_tls", "starttls", "none"],
                ),
                string_schema("imap_username", "IMAP username", false, false, None)
                    .placeholder("no-reply@example.com"),
                string_schema("imap_password", "IMAP password", false, true, None)
                    .placeholder("password"),
                string_schema("imap_mailbox", "IMAP mailbox", false, false, Some("INBOX"))
                    .placeholder("INBOX"),
            ],
            target_schema: vec![
                string_schema("to_addresses", "To addresses", false, false, None)
                    .placeholder("team@example.com"),
                string_schema("cc_addresses", "CC addresses", false, false, None)
                    .placeholder("audit@example.com"),
                string_schema("bcc_addresses", "BCC addresses", false, false, None)
                    .placeholder("archive@example.com"),
                string_schema("reply_to", "Reply-To", false, false, None)
                    .placeholder("support@example.com"),
                string_schema("subject_prefix", "Subject prefix", false, false, None),
            ],
            sensitive_fields: vec!["smtp_password", "imap_password"],
            supports_markdown: false,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "pushdeer" => Some(ProviderDefinition {
            provider_type: "pushdeer",
            label: "PushDeer",
            description: "Send notifications through PushDeer.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://api2.pushdeer.com"),
                )
                .placeholder("https://api2.pushdeer.com"),
                string_schema("pushkey", "PushKey", true, true, None)
                    .placeholder("PDUxxxx,PDUyyyy"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: Vec::new(),
            sensitive_fields: vec!["pushkey"],
            supports_markdown: true,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "magicpush" => Some(ProviderDefinition {
            provider_type: "magicpush",
            label: "MagicPush",
            description: "Send notifications through MagicPush.",
            connection_schema: vec![
                string_schema("server_url", "Server URL", true, false, None)
                    .placeholder("http://192.168.31.98:3000"),
                select_schema(
                    "delivery_mode",
                    "Delivery mode",
                    true,
                    Some("push"),
                    &["push", "inbound"],
                ),
                string_schema("token", "Token", true, false, None).placeholder("your_token"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: Vec::new(),
            sensitive_fields: Vec::new(),
            supports_markdown: false,
            supports_actions: false,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "bark" => Some(ProviderDefinition {
            provider_type: "bark",
            label: "Bark",
            description: "Send notifications through Bark.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://api.day.app"),
                )
                .placeholder("https://api.day.app"),
                string_schema("device_key", "Device Key", true, true, None)
                    .placeholder("ynJ5Ft4atkMkWeo2PAvFhF"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: vec![
                select_schema(
                    "level",
                    "Level",
                    false,
                    Some("active"),
                    &["active", "timeSensitive", "passive", "critical"],
                ),
                string_schema("group", "Group", false, false, None).placeholder("fn-knock"),
                string_schema("sound", "Sound", false, false, None).placeholder("alarm"),
                string_schema("url", "URL", false, false, None)
                    .placeholder("https://example.com/events/123"),
                string_schema("icon", "Icon", false, false, None)
                    .placeholder("https://day.app/assets/images/avatar.jpg"),
                number_schema("badge", "Badge", false, None).bounds(0, 99999),
                bool_schema("call", "Call", false, Some(false)),
            ],
            sensitive_fields: vec!["device_key"],
            supports_markdown: false,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        "telegram" => Some(ProviderDefinition {
            provider_type: "telegram",
            label: "Telegram",
            description: "Send notifications through Telegram bot API.",
            connection_schema: vec![
                string_schema(
                    "server_url",
                    "Server URL",
                    true,
                    false,
                    Some("https://api.telegram.org"),
                )
                .placeholder("https://api.telegram.org"),
                string_schema("bot_token", "Bot Token", true, true, None)
                    .placeholder("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"),
                string_schema("chat_id", "Chat ID", true, false, None)
                    .placeholder("-1001234567890"),
                number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            ],
            target_schema: vec![
                number_schema("message_thread_id", "Topic ID", false, None).min(1),
                bool_schema(
                    "disable_notification",
                    "Disable notification",
                    false,
                    Some(false),
                ),
            ],
            sensitive_fields: vec!["bot_token"],
            supports_markdown: false,
            supports_actions: true,
            supports_mentions: false,
            supports_provider_dedupe_key: false,
        }),
        _ => None,
    }
}

fn webhook_like_definition(
    provider_type: &'static str,
    label: &'static str,
    description: &'static str,
    sensitive_fields: &[&'static str],
    target_schema: Vec<SchemaField>,
) -> ProviderDefinition {
    let mut connection_schema = vec![
        string_schema("webhook_url", "Webhook URL", true, true, None).placeholder(
            match provider_type {
                "wecom" => "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
                "dingtalk" => "https://oapi.dingtalk.com/robot/send?access_token=xxxxxx",
                "feishu" => "https://open.feishu.cn/open-apis/bot/v2/hook/xxxxxxxxx",
                _ => "",
            },
        ),
    ];
    if provider_type != "wecom" {
        connection_schema.push(
            string_schema("secret", "Secret", false, true, None).placeholder(match provider_type {
                "dingtalk" => "SECxxxxxxxx",
                "feishu" => "xxxxxxxxxxxxxxxx",
                _ => "",
            }),
        );
        connection_schema.push(string_schema(
            "keyword_prefix",
            "Keyword prefix",
            false,
            false,
            None,
        ));
    }
    connection_schema
        .push(number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30));

    ProviderDefinition {
        provider_type,
        label,
        description,
        connection_schema,
        target_schema,
        sensitive_fields: sensitive_fields.to_vec(),
        supports_markdown: provider_type != "feishu",
        supports_actions: true,
        supports_mentions: true,
        supports_provider_dedupe_key: false,
    }
}

fn string_schema(
    key: &'static str,
    label: &'static str,
    required: bool,
    sensitive: bool,
    default_value: Option<&'static str>,
) -> SchemaField {
    SchemaField {
        key,
        label,
        field_type: "string",
        required,
        sensitive,
        placeholder: None,
        default_value: default_value.map(|value| Value::String(value.to_string())),
        min: None,
        max: None,
        options: Vec::new(),
    }
}

fn number_schema(
    key: &'static str,
    label: &'static str,
    required: bool,
    default_value: Option<i64>,
) -> SchemaField {
    SchemaField {
        key,
        label,
        field_type: "number",
        required,
        sensitive: false,
        placeholder: None,
        default_value: default_value.map(|value| json!(value)),
        min: None,
        max: None,
        options: Vec::new(),
    }
}

fn bool_schema(
    key: &'static str,
    label: &'static str,
    required: bool,
    default_value: Option<bool>,
) -> SchemaField {
    SchemaField {
        key,
        label,
        field_type: "boolean",
        required,
        sensitive: false,
        placeholder: None,
        default_value: default_value.map(|value| json!(value)),
        min: None,
        max: None,
        options: Vec::new(),
    }
}

fn select_schema(
    key: &'static str,
    label: &'static str,
    required: bool,
    default_value: Option<&'static str>,
    options: &[&'static str],
) -> SchemaField {
    SchemaField {
        key,
        label,
        field_type: "select",
        required,
        sensitive: false,
        placeholder: None,
        default_value: default_value.map(|value| Value::String(value.to_string())),
        min: None,
        max: None,
        options: options.iter().map(|value| (*value, *value)).collect(),
    }
}

fn json_schema(key: &'static str, label: &'static str, required: bool) -> SchemaField {
    SchemaField {
        key,
        label,
        field_type: "json",
        required,
        sensitive: false,
        placeholder: None,
        default_value: None,
        min: None,
        max: None,
        options: Vec::new(),
    }
}

fn provider_definition_view(definition: &ProviderDefinition, translator: &Translator) -> Value {
    let base_key = format!(
        "server.notifications.providers.catalog.{}",
        definition.provider_type
    );
    json!({
        "type": definition.provider_type,
        "label": provider_definition_label(definition, translator),
        "description": translator.t_with_fallback(&format!("{base_key}.description"), definition.description),
        "connection_schema": schema_view(&definition.connection_schema, definition.provider_type, "connection", translator),
        "target_schema": schema_view(&definition.target_schema, definition.provider_type, "target", translator),
        "sensitive_fields": definition.sensitive_fields,
        "capabilities": {
            "supports_text": true,
            "supports_markdown": definition.supports_markdown,
            "supports_rich_blocks": false,
            "supports_actions": definition.supports_actions,
            "supports_mentions": definition.supports_mentions,
            "supports_attachments": false,
            "supports_provider_dedupe_key": definition.supports_provider_dedupe_key,
            "max_body_length": provider_max_body_length(definition.provider_type)
        }
    })
}

fn provider_max_body_length(provider_type: &str) -> Value {
    match provider_type {
        "serverchan" => json!(32768),
        "wecom" => json!(4096),
        "feishu" => json!(20480),
        "telegram" => json!(4096),
        _ => Value::Null,
    }
}

fn provider_definition_label(definition: &ProviderDefinition, translator: &Translator) -> String {
    translator.t_with_fallback(
        &format!(
            "server.notifications.providers.catalog.{}.label",
            definition.provider_type
        ),
        definition.label,
    )
}

fn notification_service_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.notifications.service.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn notification_service_default_text(key: &str, params: &[(&str, String)]) -> String {
    notification_service_text(&Translator::new(crate::i18n::DEFAULT_LOCALE), key, params)
}

fn notification_route_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.notifications.routes.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn notification_provider_error_default(
    provider_type: &str,
    key: &str,
    params: &[(&str, String)],
) -> String {
    notification_provider_error_text(
        &Translator::new(crate::i18n::DEFAULT_LOCALE),
        provider_type,
        key,
        params,
    )
}

fn notification_provider_error_text(
    translator: &Translator,
    provider_type: &str,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.notifications.providers.catalog.{provider_type}.errors.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn notification_provider_field_text(
    translator: &Translator,
    provider_type: &str,
    field_key: &str,
    part: &str,
    fallback: &str,
) -> String {
    translator.t_with_fallback(
        &format!(
            "server.notifications.providers.catalog.{provider_type}.fields.{field_key}.{part}"
        ),
        fallback,
    )
}

fn notification_email_message_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.notifications.providers.catalog.email.message.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn localize_provider_test_result(
    mut result: ProviderTestResult,
    translator: &Translator,
) -> ProviderTestResult {
    result.message = localize_provider_test_message(translator, &result.message);
    result
}

fn localize_provider_test_message(translator: &Translator, message: &str) -> String {
    let normalized = message.trim();
    if normalized.is_empty() {
        return notification_service_text(translator, "testSendFailed", &[]);
    }
    if normalized == "Notification provider test sent successfully" {
        return notification_service_text(translator, "testSendSuccess", &[]);
    }
    if let Some(message) = localize_notification_service_message(translator, normalized) {
        return message;
    }
    if let Some(message) = localize_notification_provider_error(translator, normalized) {
        return message;
    }
    if let Some(message) = localize_wxpusher_invalid_topic_ids(translator, normalized) {
        return message;
    }
    if let Some(message) = localize_notification_request_status(translator, normalized) {
        return message;
    }
    if let Some(message) = localize_bark_partial_failure(translator, normalized) {
        return message;
    }
    normalized.to_string()
}

fn localize_notification_service_message(translator: &Translator, message: &str) -> Option<String> {
    for &key in NOTIFICATION_TEST_SERVICE_KEYS {
        for locale in NOTIFICATION_MESSAGE_LOCALES {
            if notification_service_text(&Translator::new(locale), key, &[]) == message {
                return Some(notification_service_text(translator, key, &[]));
            }
        }
    }
    None
}

fn localize_notification_provider_error(translator: &Translator, message: &str) -> Option<String> {
    for &provider_type in PROVIDER_TYPES {
        for &key in NOTIFICATION_PROVIDER_ERROR_KEYS {
            for locale in NOTIFICATION_MESSAGE_LOCALES {
                if notification_provider_error_text(
                    &Translator::new(locale),
                    provider_type,
                    key,
                    &[],
                ) == message
                {
                    return Some(notification_provider_error_text(
                        translator,
                        provider_type,
                        key,
                        &[],
                    ));
                }
            }
        }
    }
    None
}

fn localize_wxpusher_invalid_topic_ids(translator: &Translator, message: &str) -> Option<String> {
    if let Some(values) = message.strip_prefix("Invalid WxPusher topic id(s): ") {
        return Some(notification_provider_error_text(
            translator,
            "wxpusher",
            "invalidTopicIds",
            &[("values", values.trim().to_string())],
        ));
    }

    let marker = "__FN_KNOCK_VALUES__";
    for locale in NOTIFICATION_MESSAGE_LOCALES {
        let sample = notification_provider_error_text(
            &Translator::new(locale),
            "wxpusher",
            "invalidTopicIds",
            &[("values", marker.to_string())],
        );
        if let Some((prefix, suffix)) = sample.split_once(marker)
            && message.starts_with(prefix)
            && message.ends_with(suffix)
        {
            let values = &message[prefix.len()..message.len().saturating_sub(suffix.len())];
            return Some(notification_provider_error_text(
                translator,
                "wxpusher",
                "invalidTopicIds",
                &[("values", values.trim().to_string())],
            ));
        }
    }

    None
}

fn localize_notification_request_status(translator: &Translator, message: &str) -> Option<String> {
    let (provider, status) = message.split_once(" request returned status ")?;
    let provider = provider.trim();
    let status = status.trim();
    if provider.is_empty() || status.is_empty() {
        return None;
    }
    Some(notification_service_text(
        translator,
        "providerRequestReturnedStatus",
        &[
            ("provider", provider.to_string()),
            ("status", status.to_string()),
        ],
    ))
}

fn localize_bark_partial_failure(translator: &Translator, message: &str) -> Option<String> {
    let counts = message
        .strip_prefix("Bark failed for ")?
        .strip_suffix(" target(s)")?;
    let (failed, total) = counts.split_once('/')?;
    Some(notification_service_text(
        translator,
        "barkPartialFailed",
        &[
            ("failed", failed.trim().to_string()),
            ("total", total.trim().to_string()),
        ],
    ))
}

fn schema_view(
    fields: &[SchemaField],
    provider_type: &str,
    scope: &str,
    translator: &Translator,
) -> Vec<Value> {
    fields
        .iter()
        .map(|field| {
            let mut value = Map::new();
            value.insert("key".to_string(), Value::String(field.key.to_string()));
            value.insert(
                "label".to_string(),
                Value::String(localize_notification_schema_part(
                    translator,
                    provider_type,
                    field.key,
                    scope,
                    "label",
                    field.label,
                )),
            );
            value.insert(
                "type".to_string(),
                Value::String(field.field_type.to_string()),
            );
            if let Some(description) = optional_notification_schema_part(
                translator,
                provider_type,
                field.key,
                scope,
                "description",
            ) {
                value.insert("description".to_string(), Value::String(description));
            }
            let placeholder = optional_notification_schema_part(
                translator,
                provider_type,
                field.key,
                scope,
                "placeholder",
            )
            .or_else(|| field.placeholder.map(str::to_string));
            if let Some(placeholder) = placeholder {
                value.insert("placeholder".to_string(), Value::String(placeholder));
            }
            if field.required {
                value.insert("required".to_string(), Value::Bool(true));
            }
            if field.sensitive {
                value.insert("sensitive".to_string(), Value::Bool(true));
            }
            if let Some(default_value) = &field.default_value {
                value.insert("default_value".to_string(), default_value.clone());
            }
            if let Some(min) = field.min {
                value.insert("min".to_string(), json!(min));
            }
            if let Some(max) = field.max {
                value.insert("max".to_string(), json!(max));
            }
            if !field.options.is_empty() {
                value.insert(
                    "options".to_string(),
                    Value::Array(
                        field
                            .options
                            .iter()
                            .map(|(label, option_value)| {
                                let key = format!(
                                    "server.notifications.providers.catalog.{provider_type}.fields.{}.options.{option_value}",
                                    field.key
                                );
                                json!({
                                    "label": translator.t_with_fallback(&key, label),
                                    "value": option_value
                                })
                            })
                            .collect(),
                    ),
                );
            }
            Value::Object(value)
        })
        .collect()
}

fn localize_notification_schema_part(
    translator: &Translator,
    provider_type: &str,
    field_key: &str,
    scope: &str,
    part: &str,
    fallback: &str,
) -> String {
    optional_notification_schema_part(translator, provider_type, field_key, scope, part)
        .unwrap_or_else(|| fallback.to_string())
}

fn optional_notification_schema_part(
    translator: &Translator,
    provider_type: &str,
    field_key: &str,
    scope: &str,
    part: &str,
) -> Option<String> {
    let base_key =
        format!("server.notifications.providers.catalog.{provider_type}.fields.{field_key}");
    let scoped_part = if scope == "target" {
        match part {
            "label" => Some("targetLabel"),
            "description" => Some("targetDescription"),
            "placeholder" => Some("targetPlaceholder"),
            _ => None,
        }
    } else {
        None
    };
    if let Some(scoped_part) = scoped_part {
        let key = format!("{base_key}.{scoped_part}");
        let translated = translator.t(&key);
        if translated != key {
            return Some(translated);
        }
    }
    let key = format!("{base_key}.{part}");
    let translated = translator.t(&key);
    if translated == key {
        None
    } else {
        Some(translated)
    }
}

fn normalize_schema_config(
    raw: &Map<String, Value>,
    fields: &[SchemaField],
) -> NotifyResult<Map<String, Value>> {
    let mut normalized = normalize_schema_patch(raw, fields)?;
    apply_schema_defaults(&mut normalized, fields);
    Ok(normalized)
}

fn normalize_schema_patch(
    raw: &Map<String, Value>,
    fields: &[SchemaField],
) -> NotifyResult<Map<String, Value>> {
    let mut normalized = Map::new();
    for field in fields {
        let Some(input) = raw.get(field.key) else {
            continue;
        };
        let value = match field.field_type {
            "string" => Value::String(value_to_trimmed_string(input)),
            "number" => json!(value_to_i64(input, 0)),
            "boolean" => Value::Bool(value_to_bool(input)),
            "select" => {
                let selected = value_to_trimmed_string(input);
                if !field.options.is_empty()
                    && !field.options.iter().any(|(_, value)| *value == selected)
                {
                    return Err(NotifyError::BadRequest(notification_service_default_text(
                        "invalidSelectValue",
                        &[("field", field.label.to_string())],
                    )));
                }
                Value::String(selected)
            }
            "json" => normalize_json_field(input, field.label)?,
            _ => input.clone(),
        };
        if field.field_type == "json" && value.is_null() {
            continue;
        }
        normalized.insert(field.key.to_string(), value);
    }
    Ok(normalized)
}

fn apply_schema_defaults(config: &mut Map<String, Value>, fields: &[SchemaField]) {
    for field in fields {
        if config.contains_key(field.key) {
            continue;
        }
        if let Some(default_value) = &field.default_value {
            config.insert(field.key.to_string(), default_value.clone());
        }
    }
}

fn validate_required_fields(
    config: &Map<String, Value>,
    fields: &[SchemaField],
) -> NotifyResult<()> {
    for field in fields {
        if !field.required {
            continue;
        }
        let missing = match config.get(field.key) {
            None | Some(Value::Null) => true,
            Some(Value::String(value)) => value.trim().is_empty(),
            _ => false,
        };
        if missing {
            return Err(NotifyError::BadRequest(notification_service_default_text(
                "fieldRequired",
                &[("field", field.label.to_string())],
            )));
        }
    }
    Ok(())
}

fn normalize_json_field(value: &Value, label: &str) -> NotifyResult<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    if let Some(value) = value.as_str() {
        if value.is_empty() {
            return Ok(Value::Null);
        }
        return serde_json::from_str(value).map_err(|_| {
            NotifyError::BadRequest(notification_service_default_text(
                "invalidJson",
                &[("field", label.to_string())],
            ))
        });
    }
    Ok(value.clone())
}

fn normalize_provider_connection_aliases(provider_type: &str, raw: &mut Map<String, Value>) {
    if provider_type != "wxpusher" {
        return;
    }
    copy_alias(raw, "appToken", "app_token");
    copy_alias(raw, "serverUrl", "server_url");
    copy_alias(raw, "timeoutSeconds", "timeout_seconds");
}

fn normalize_provider_target_aliases(provider_type: &str, raw: &mut Map<String, Value>) {
    if provider_type != "wxpusher" {
        return;
    }
    for alias in ["topicIds", "topic_id", "topicId", "topic", "Topic"] {
        copy_alias(raw, alias, "topic_ids");
    }
    copy_alias(raw, "verifyPayType", "verify_pay_type");
}

fn copy_alias(raw: &mut Map<String, Value>, alias: &str, canonical: &str) {
    if raw.contains_key(canonical) {
        return;
    }
    if let Some(value) = raw.get(alias).cloned() {
        raw.insert(canonical.to_string(), value);
    }
}

fn drop_masked_sensitive_patch_values(
    definition: &ProviderDefinition,
    raw: &mut Map<String, Value>,
) {
    for key in &definition.sensitive_fields {
        let Some(value) = raw.get(*key).and_then(Value::as_str) else {
            continue;
        };
        if value.contains("***") || value == "[configured]" {
            raw.remove(*key);
        }
    }
}

fn mask_provider(provider: &Value) -> Result<Value, String> {
    let provider_type = provider
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| notification_service_default_text("invalidProviderRecord", &[]))?;
    let definition = provider_definition(provider_type)
        .ok_or_else(|| notification_service_default_text("unsupportedProviderType", &[]))?;
    let connection_config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let sensitive = definition
        .sensitive_fields
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let masked = connection_config
        .into_iter()
        .map(|(key, value)| {
            if sensitive.contains(key.as_str()) {
                (key, mask_sensitive_value(&value))
            } else {
                (key, value)
            }
        })
        .collect::<Map<_, _>>();

    Ok(json!({
        "id": provider.get("id").cloned().unwrap_or(Value::Null),
        "name": provider.get("name").cloned().unwrap_or(Value::Null),
        "type": provider.get("type").cloned().unwrap_or(Value::Null),
        "enabled": provider.get("enabled").cloned().unwrap_or(Value::Bool(true)),
        "created_at": provider.get("created_at").cloned().unwrap_or(Value::Null),
        "updated_at": provider.get("updated_at").cloned().unwrap_or(Value::Null),
        "last_test_at": provider.get("last_test_at").cloned().unwrap_or(Value::Null),
        "last_test_status": provider.get("last_test_status").cloned().unwrap_or(Value::Null),
        "last_error": provider.get("last_error").cloned().unwrap_or(Value::Null),
        "connection_config_masked": Value::Object(masked)
    }))
}

fn reveal_provider(provider: &Value) -> Result<Value, String> {
    let mut view = mask_provider(provider)?;
    if let Value::Object(ref mut object) = view {
        object.insert(
            "connection_config".to_string(),
            provider
                .get("connection_config")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new())),
        );
    }
    Ok(view)
}

fn mask_sensitive_value(value: &Value) -> Value {
    if value.is_null() {
        return Value::String(String::new());
    }
    if let Some(value) = value.as_str() {
        if value.is_empty() {
            return Value::String(String::new());
        }
        if value.chars().count() <= 8 {
            return Value::String("********".to_string());
        }
        let prefix = value.chars().take(2).collect::<String>();
        return Value::String(format!("{prefix}******"));
    }
    Value::String("[configured]".to_string())
}

#[derive(Clone)]
struct ProviderTestResult {
    success: bool,
    message: String,
    request_summary: Option<Value>,
    response_summary: Option<Value>,
}

async fn run_provider_test(
    state: &AppState,
    provider: Value,
    translator: &Translator,
) -> Result<ProviderTestResult, String> {
    let provider_type = provider
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match provider_type {
        "webhook" => send_webhook_test(state, &provider, translator).await,
        provider_type if is_http_notification_provider(provider_type) => {
            let message = build_provider_test_message(translator);
            let target = json!({ "target_config": {} });
            Ok(send_http_notification_provider(
                state,
                &provider,
                &target,
                &message,
                provider_timeout_seconds(&provider, 5),
            )
            .await)
        }
        "email" => {
            let message = build_provider_test_message(translator);
            let target = json!({ "target_config": {} });
            Ok(send_email_notification(
                &provider,
                &target,
                &message,
                provider_timeout_seconds(&provider, 10),
                translator,
            )
            .await)
        }
        _ => Ok(ProviderTestResult {
            success: false,
            message: notification_service_text(translator, "unsupportedProviderType", &[]),
            request_summary: None,
            response_summary: None,
        }),
    }
}

fn is_http_notification_provider(provider_type: &str) -> bool {
    matches!(
        provider_type,
        "wxpusher"
            | "serverchan"
            | "pushplus"
            | "wecom"
            | "dingtalk"
            | "feishu"
            | "pushdeer"
            | "magicpush"
            | "bark"
            | "telegram"
    )
}

fn build_provider_test_message(translator: &Translator) -> Value {
    let now = time_utils::now_iso();
    json!({
        "title": notification_service_text(translator, "testMessage.title", &[]),
        "summary": notification_service_text(translator, "testMessage.summary", &[]),
        "body_text": notification_service_text(translator, "testMessage.bodyText", &[]),
        "body_markdown": notification_service_text(translator, "testMessage.bodyMarkdown", &[]),
        "severity": "info",
        "facts": [
            {
                "label": notification_service_text(translator, "testMessage.sendType", &[]),
                "value": notification_service_text(translator, "testMessage.providerTest", &[])
            },
            {
                "label": notification_service_text(translator, "testMessage.sentAt", &[]),
                "value": now
            }
        ],
        "actions": [],
        "mentions": [],
        "dedupe_key": Value::Null,
        "occurred_at": now,
        "event_id": Value::Null,
        "metadata": { "test": true }
    })
}

async fn send_http_notification_provider(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    match provider
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "wxpusher" => send_wxpusher(state, provider, target, message, timeout_seconds).await,
        "serverchan" => send_serverchan(state, provider, target, message, timeout_seconds).await,
        "pushplus" => send_pushplus(state, provider, target, message, timeout_seconds).await,
        "wecom" => send_wecom(state, provider, target, message, timeout_seconds).await,
        "dingtalk" => send_dingtalk(state, provider, target, message, timeout_seconds).await,
        "feishu" => send_feishu(state, provider, target, message, timeout_seconds).await,
        "pushdeer" => send_pushdeer(state, provider, message, timeout_seconds).await,
        "magicpush" => send_magicpush(state, provider, message, timeout_seconds).await,
        "bark" => send_bark(state, provider, target, message, timeout_seconds).await,
        "telegram" => send_telegram(state, provider, target, message, timeout_seconds).await,
        _provider_type => ProviderTestResult {
            success: false,
            message: notification_service_default_text("unsupportedProviderType", &[]),
            request_summary: None,
            response_summary: None,
        },
    }
}

async fn send_wecom(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let webhook_url = config_text(&config, "webhook_url");
    if webhook_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wecom",
            "missingWebhookUrl",
            &[],
        ));
    }

    let mentioned_list = split_values(target_config.get("mentioned_list"));
    let mentioned_mobile_list = split_values(target_config.get("mentioned_mobile_list"));
    let markdown_content = build_wecom_markdown_content(message, &mentioned_list);
    let use_text_payload =
        !mentioned_mobile_list.is_empty() || markdown_content.as_bytes().len() > 4096;
    let body = if use_text_payload {
        json!({
            "msgtype": "text",
            "text": {
                "content": default_string(
                    truncate_utf8_bytes(&build_wecom_text_content(message), 2048),
                    DEFAULT_NOTIFICATION_MESSAGE_TITLE,
                ),
                "mentioned_list": mentioned_list,
                "mentioned_mobile_list": mentioned_mobile_list
            }
        })
    } else {
        json!({
            "msgtype": "markdown",
            "markdown": {
                "content": default_string(
                    truncate_utf8_bytes(&markdown_content, 4096),
                    DEFAULT_NOTIFICATION_MESSAGE_TITLE,
                )
            }
        })
    };
    let request_summary = json!({
        "method": "POST",
        "url": redact_query_value(&webhook_url, "key"),
        "msgtype": body.get("msgtype").cloned().unwrap_or(Value::Null),
        "mentioned_count": split_values(target_config.get("mentioned_list")).len(),
        "mentioned_mobile_count": split_values(target_config.get("mentioned_mobile_list")).len()
    });

    let (status, ok, text, parsed) = post_json(state, &webhook_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "WeCom",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "errcode").unwrap_or(0) == 0,
        |value| json_text(value, "errmsg"),
    )
}

async fn send_dingtalk(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let webhook_url = config_text(&config, "webhook_url");
    if webhook_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "dingtalk",
            "missingWebhookUrl",
            &[],
        ));
    }
    let secret = config_text(&config, "secret");
    let keyword_prefix = config_text(&config, "keyword_prefix");
    let at_mobiles = split_values(target_config.get("at_mobiles"));
    let at_user_ids = split_values(target_config.get("at_user_ids"));
    let is_at_all = target_config
        .get("is_at_all")
        .map(value_to_bool)
        .unwrap_or(false);
    let mention_text = build_dingtalk_mention_text(&at_mobiles, &at_user_ids, is_at_all);
    let title = apply_keyword_prefix(&message_title(message), &keyword_prefix);
    let markdown_text = non_empty_or(
        build_markdown_body(message, &mention_text),
        message_summary(message),
        &title,
    );
    let request_url = if secret.is_empty() {
        webhook_url.clone()
    } else {
        let timestamp = time_utils::now_ms().to_string();
        let sign = hmac_sha256_base64(
            secret.as_bytes(),
            format!("{timestamp}\n{secret}").as_bytes(),
        );
        append_query_params(&webhook_url, &[("timestamp", timestamp), ("sign", sign)])
    };
    let body = json!({
        "msgtype": "markdown",
        "markdown": { "title": title, "text": markdown_text },
        "at": {
            "atMobiles": at_mobiles,
            "atUserIds": at_user_ids,
            "isAtAll": is_at_all
        }
    });
    let request_summary = json!({
        "method": "POST",
        "url": redact_query_value(&redact_query_value(&request_url, "access_token"), "sign"),
        "msgtype": "markdown",
        "signed": !secret.is_empty(),
        "mentioned_mobile_count": body.pointer("/at/atMobiles").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "mentioned_user_count": body.pointer("/at/atUserIds").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "is_at_all": is_at_all,
        "title_preview": truncate_text(body.pointer("/markdown/title").and_then(Value::as_str).unwrap_or(""), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &request_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "DingTalk",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "errcode").unwrap_or(0) == 0,
        |value| json_text(value, "errmsg"),
    )
}

async fn send_feishu(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let webhook_url = config_text(&config, "webhook_url");
    if webhook_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "feishu",
            "missingWebhookUrl",
            &[],
        ));
    }
    let secret = config_text(&config, "secret");
    let keyword_prefix = config_text(&config, "keyword_prefix");
    let mention_user_ids = split_values(target_config.get("mention_user_ids"));
    let title = apply_keyword_prefix(&message_title(message), &keyword_prefix);
    let mut body = json!({
        "msg_type": "post",
        "content": {
            "post": {
                "zh_cn": {
                    "title": title,
                    "content": build_feishu_post_content(message, &mention_user_ids)
                }
            }
        }
    });
    if !secret.is_empty() {
        let timestamp = (time_utils::now_ms() / 1000).to_string();
        let key = format!("{timestamp}\n{secret}");
        let sign = hmac_sha256_base64(key.as_bytes(), b"");
        if let Some(object) = body.as_object_mut() {
            object.insert("timestamp".to_string(), Value::String(timestamp));
            object.insert("sign".to_string(), Value::String(sign));
        }
    }
    let request_summary = json!({
        "method": "POST",
        "url": redact_path_tail(&webhook_url),
        "msg_type": "post",
        "signed": !secret.is_empty(),
        "mentioned_user_count": mention_user_ids.len(),
        "title_preview": truncate_text(body.pointer("/content/post/zh_cn/title").and_then(Value::as_str).unwrap_or(""), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &webhook_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "Feishu",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code").unwrap_or(0) == 0,
        |value| json_text(value, "msg"),
    )
}

async fn send_serverchan(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let sendkey = config_text(&config, "sendkey");
    if sendkey.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "serverchan",
            "missingSendKey",
            &[],
        ));
    }
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://sctapi.ftqq.com",
    );
    let url = format!("{}/{}.send", base_url.trim_end_matches('/'), sendkey);
    let title = truncate_text(&message_title(message), 32);
    let desp = truncate_utf8_bytes(&build_markdown_body(message, ""), 32 * 1024);
    let short = truncate_text(&config_text(&target_config, "short"), 64);
    let channel = config_text(&target_config, "channel");
    let openid = config_text(&target_config, "openid");
    let noip = target_config
        .get("noip")
        .map(value_to_bool)
        .unwrap_or(false);
    let mut form = vec![(
        "title".to_string(),
        default_string(title.clone(), "fn-knock"),
    )];
    push_form_if(&mut form, "desp", desp.clone());
    push_form_if(&mut form, "short", short.clone());
    push_form_if(&mut form, "channel", channel.clone());
    push_form_if(&mut form, "openid", openid.clone());
    if noip {
        form.push(("noip".to_string(), "1".to_string()));
    }
    let request_summary = json!({
        "method": "POST",
        "endpoint": base_url,
        "has_desp": !desp.is_empty(),
        "has_short": !short.is_empty(),
        "channel": empty_to_null(&channel),
        "has_openid": !openid.is_empty(),
        "noip": noip,
        "title_preview": title
    });
    let (status, ok, text, parsed) = post_form(state, &url, &form, timeout_seconds).await;
    provider_result_from_api(
        "ServerChan",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64_any(value, &["code", "errno", "error_code"]).unwrap_or(0) == 0,
        |value| json_text_any(value, &["message", "msg", "error"]),
    )
}

async fn send_pushplus(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let token = config_text(&config, "token");
    if token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "pushplus",
            "missingToken",
            &[],
        ));
    }
    let url = resolve_pushplus_url(&default_string(
        config_text(&config, "server_url"),
        "https://www.pushplus.plus",
    ));
    let template = match config_text(&target_config, "template").as_str() {
        "html" => "html",
        "txt" => "txt",
        "json" => "json",
        _ => "markdown",
    };
    let channel = default_string(config_text(&target_config, "channel"), "wechat");
    let topic = config_text(&target_config, "topic");
    let option = config_text(&target_config, "option");
    let to = config_text(&target_config, "to");
    let callback_url = config_text(&target_config, "callback_url");
    let pre = config_text(&target_config, "pre");
    let title = truncate_text(&message_title(message), 128);
    let content = match template {
        "html" => build_pushplus_html_content(message),
        "txt" => build_pushplus_text_content(message),
        "json" => build_pushplus_json_content(message),
        _ => build_pushplus_markdown_content(message),
    };
    let mut body = json!({
        "token": token,
        "title": title,
        "content": default_string(content, "fn-knock"),
        "template": template,
        "channel": channel
    });
    insert_non_empty(&mut body, "topic", topic.clone());
    insert_non_empty(&mut body, "option", option.clone());
    insert_non_empty(&mut body, "to", to.clone());
    insert_non_empty(&mut body, "callbackUrl", callback_url.clone());
    insert_non_empty(&mut body, "pre", pre.clone());
    let request_summary = json!({
        "method": "POST",
        "endpoint": url,
        "channel": channel,
        "template": template,
        "has_topic": !topic.is_empty(),
        "has_option": !option.is_empty(),
        "has_to": !to.is_empty(),
        "has_callback_url": !callback_url.is_empty(),
        "has_pre": !pre.is_empty(),
        "title_preview": title
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    provider_result_from_api(
        "PushPlus",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code") == Some(200),
        |value| json_text_any(value, &["msg", "message", "error"]),
    )
}

async fn send_pushdeer(
    state: &AppState,
    provider: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let pushkey = config_text(&config, "pushkey");
    if pushkey.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "pushdeer",
            "missingPushKey",
            &[],
        ));
    }
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://api2.pushdeer.com",
    );
    let url = format!("{}/message/push", base_url.trim_end_matches('/'));
    let form = vec![
        ("pushkey".to_string(), pushkey.clone()),
        ("text".to_string(), message_title(message)),
        ("desp".to_string(), build_markdown_body(message, "")),
        ("type".to_string(), "markdown".to_string()),
    ];
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "pushkey_count": split_values(Some(&Value::String(pushkey))).len(),
        "type": "markdown",
        "title_preview": message_title(message)
    });
    let (status, ok, text, parsed) = post_form(state, &url, &form, timeout_seconds).await;
    provider_result_from_api(
        "PushDeer",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code").unwrap_or(0) == 0,
        |value| json_text_any(value, &["error", "message", "msg"]),
    )
}

async fn send_magicpush(
    state: &AppState,
    provider: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let token = config_text(&config, "token");
    if token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "magicpush",
            "missingToken",
            &[],
        ));
    }
    let base_url = config_text(&config, "server_url");
    if base_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "magicpush",
            "missingBaseUrl",
            &[],
        ));
    }
    let delivery_mode = if config_text(&config, "delivery_mode") == "inbound" {
        "inbound"
    } else {
        "push"
    };
    let url = resolve_magicpush_url(&base_url, &token, delivery_mode);
    let title = message_title(message);
    let content = default_string(build_magicpush_content(message), &title);
    let magicpush_facts = magicpush_facts_object(message);
    let payload = if delivery_mode == "inbound" {
        json!({
            "source": "fn-knock",
            "title": title,
            "summary": message.get("summary").cloned().unwrap_or(Value::Null),
            "content": content,
            "body": content,
            "body_text": message.get("body_text").cloned().unwrap_or(Value::Null),
            "body_markdown": message.get("body_markdown").cloned().unwrap_or(Value::Null),
            "type": if message_text(message, "body_markdown").is_empty() { "text" } else { "markdown" },
            "severity": message.get("severity").cloned().unwrap_or(Value::Null),
            "facts": magicpush_facts,
            "facts_list": message.get("facts").cloned().unwrap_or_else(|| json!([])),
            "actions": message.get("actions").cloned().unwrap_or_else(|| json!([])),
            "mentions": message.get("mentions").cloned().unwrap_or_else(|| json!([])),
            "dedupe_key": message.get("dedupe_key").cloned().unwrap_or(Value::Null),
            "occurred_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
            "event_id": message.get("event_id").cloned().unwrap_or(Value::Null),
            "metadata": message.get("metadata").cloned().unwrap_or_else(|| json!({}))
        })
    } else {
        json!({ "title": title, "content": content, "type": "text" })
    };
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "delivery_mode": delivery_mode,
        "type": payload.get("type").cloned().unwrap_or(Value::Null),
        "title_preview": payload.get("title").cloned().unwrap_or(Value::Null),
        "content_preview": truncate_text(payload.get("content").and_then(Value::as_str).unwrap_or(""), 500)
    });
    let mut request = state
        .fallback_client
        .post(&url)
        .header("content-type", "application/json; charset=utf-8");
    if delivery_mode == "inbound" {
        request = request.header("x-fn-knock-provider", "magicpush");
    } else {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let (status, ok, text, parsed) = send_prepared_json(request, &payload, timeout_seconds).await;
    provider_result_from_api(
        "MagicPush",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| {
            value.get("success").and_then(Value::as_bool) != Some(false)
                && json_i64(value, "code").is_none_or(|code| code == 200)
        },
        |value| json_text_any(value, &["message", "msg", "error"]),
    )
}

async fn send_bark(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let device_keys = split_values(config.get("device_key"));
    if device_keys.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "bark",
            "missingDeviceKey",
            &[],
        ));
    }
    let base_url = default_string(config_text(&config, "server_url"), "https://api.day.app");
    let url = format!("{}/push", base_url.trim_end_matches('/'));
    let payload_preview = build_bark_payload(message, target);
    let mut results = Vec::new();
    for device_key in &device_keys {
        let mut payload = payload_preview.clone();
        insert_string(&mut payload, "device_key", device_key.clone());
        let (status, ok, text, parsed) = post_json(state, &url, &payload, timeout_seconds).await;
        let bark_code = parsed.as_ref().and_then(|value| json_i64(value, "code"));
        let success = ok && bark_code.is_none_or(|code| code == 200);
        results.push(json!({
            "success": success,
            "status": status,
            "ok": ok,
            "code": bark_code,
            "message": parsed.as_ref().and_then(|value| json_text(value, "message")),
            "body_preview": truncate_text(&text, 500)
        }));
    }
    let failed_count = results
        .iter()
        .filter(|result| result.get("success").and_then(Value::as_bool) != Some(true))
        .count();
    ProviderTestResult {
        success: failed_count == 0,
        message: if failed_count == 0 {
            notification_service_default_text("testSendSuccess", &[])
        } else {
            format!("Bark failed for {failed_count}/{} target(s)", results.len())
        },
        request_summary: Some(json!({
            "method": "POST",
            "url": url,
            "device_key_count": device_keys.len(),
            "level": payload_preview.get("level").cloned().unwrap_or(Value::Null),
            "group": payload_preview.get("group").cloned().unwrap_or(Value::Null),
            "title_preview": payload_preview.get("title").cloned().unwrap_or(Value::Null)
        })),
        response_summary: Some(json!({
            "success_count": results.len().saturating_sub(failed_count),
            "failed_count": failed_count,
            "results": results
        })),
    }
}

async fn send_telegram(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://api.telegram.org",
    );
    let bot_token = config_text(&config, "bot_token");
    let chat_id = config_text(&config, "chat_id");
    if bot_token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "telegram",
            "missingBotToken",
            &[],
        ));
    }
    if chat_id.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "telegram",
            "missingChatId",
            &[],
        ));
    }
    let url = format!(
        "{}/bot{}/sendMessage",
        base_url.trim_end_matches('/'),
        bot_token
    );
    let reply_markup = build_telegram_reply_markup(message);
    let message_thread_id = optional_positive_i64(target_config.get("message_thread_id"));
    let disable_notification = target_config
        .get("disable_notification")
        .map(value_to_bool)
        .unwrap_or(false);
    let mut body = json!({
        "chat_id": chat_id,
        "text": default_string(build_telegram_text(message), DEFAULT_NOTIFICATION_MESSAGE_TITLE),
        "parse_mode": "HTML",
    });
    if disable_notification {
        insert_value(&mut body, "disable_notification", Value::Bool(true));
    }
    if let Some(message_thread_id) = message_thread_id {
        insert_i64(&mut body, "message_thread_id", message_thread_id);
    }
    if let Some(reply_markup) = reply_markup {
        insert_value(&mut body, "reply_markup", reply_markup);
    }
    let request_summary = json!({
        "method": "POST",
        "url": format!("{}/bot<redacted>/sendMessage", base_url.trim_end_matches('/')),
        "chat_id": chat_id,
        "message_thread_id": message_thread_id,
        "disable_notification": disable_notification,
        "has_inline_keyboard": body.get("reply_markup").is_some(),
        "text_preview": truncate_text(&message_title(message), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    provider_result_from_api(
        "Telegram",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| value.get("ok").and_then(Value::as_bool).unwrap_or(true),
        |value| json_text(value, "description"),
    )
}

async fn send_wxpusher(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let app_token = first_config_text(&config, &["app_token", "appToken"]);
    if app_token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wxpusher",
            "missingAppToken",
            &[],
        ));
    }
    let uids = split_values(effective_config_value(&config, &target_config, &["uids"]).as_ref());
    let topic_ids_value = effective_config_value(
        &config,
        &target_config,
        &[
            "topic_ids",
            "topicIds",
            "topic_id",
            "topicId",
            "topic",
            "Topic",
        ],
    );
    let (topic_ids, invalid_topic_ids) = parse_topic_ids(topic_ids_value.as_ref());
    if !invalid_topic_ids.is_empty() {
        return ProviderTestResult {
            success: false,
            message: notification_provider_error_default(
                "wxpusher",
                "invalidTopicIds",
                &[("values", invalid_topic_ids.join(", "))],
            ),
            request_summary: None,
            response_summary: None,
        };
    }
    if uids.is_empty() && topic_ids.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wxpusher",
            "recipientRequired",
            &[],
        ));
    }
    let link_url = effective_config_value(&config, &target_config, &["url"])
        .as_ref()
        .map(value_to_trimmed_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| primary_action_url(message));
    let verify_pay_type = effective_config_value(
        &config,
        &target_config,
        &["verify_pay_type", "verifyPayType"],
    )
    .as_ref()
    .and_then(|value| {
        let parsed = value_to_i64(value, -1);
        (0..=2).contains(&parsed).then_some(parsed)
    });
    let mut body = json!({
        "appToken": app_token,
        "content": default_string(
            build_wxpusher_html_content(message),
            &format!("<p>{}</p>", escape_html(DEFAULT_NOTIFICATION_MESSAGE_TITLE)),
        ),
        "summary": truncate_utf8_bytes(
            &default_string(message_summary(message), &message_title(message)),
            100,
        ),
        "contentType": 2
    });
    if !topic_ids.is_empty() {
        insert_value(&mut body, "topicIds", json!(topic_ids));
    }
    if !uids.is_empty() {
        insert_value(&mut body, "uids", json!(uids));
    }
    insert_non_empty(&mut body, "url", link_url.clone());
    if let Some(verify_pay_type) = verify_pay_type {
        insert_i64(&mut body, "verifyPayType", verify_pay_type);
    }
    let url = format!(
        "{}/api/send/message",
        default_string(
            first_config_text(&config, &["server_url", "serverUrl"]),
            "https://wxpusher.zjiecode.com"
        )
        .trim_end_matches('/')
    );
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "uid_count": body.get("uids").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "topic_id_count": body.get("topicIds").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "content_type": 2,
        "has_url": !link_url.is_empty()
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    let failed_items = parsed
        .as_ref()
        .and_then(|value| value.get("data"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| json_i64(item, "code") != Some(1000))
                .count()
        })
        .unwrap_or(0);
    provider_result_from_api(
        "WxPusher",
        request_summary,
        status,
        ok,
        text,
        parsed,
        move |value| {
            value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                && json_i64(value, "code").unwrap_or(1000) == 1000
                && failed_items == 0
        },
        |value| json_text_any(value, &["msg", "message", "error"]),
    )
}

async fn send_email_notification(
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
    translator: &Translator,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let smtp_host = config_text(&config, "smtp_host");
    let from_address = config_text(&config, "from_address");
    if smtp_host.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "email",
            "missingSmtpHost",
            &[],
        ));
    }
    if from_address.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "email",
            "invalidFromAddress",
            &[],
        ));
    }
    let smtp_security = default_string(config_text(&config, "smtp_security"), "ssl_tls");
    let smtp_port = config
        .get("smtp_port")
        .map(|value| value_to_i64(value, default_smtp_port(&smtp_security)))
        .unwrap_or_else(|| default_smtp_port(&smtp_security))
        .clamp(1, 65535) as u16;
    let auth_mode = default_string(config_text(&config, "smtp_auth_mode"), "auto");
    let smtp_username = config_text(&config, "smtp_username");
    let smtp_password = config_text(&config, "smtp_password");
    let from_name = config_text(&config, "from_name");
    let subject_prefix = config_text(&target_config, "subject_prefix");
    let subject = [subject_prefix, message_title(message)]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let to_addresses =
        config_text(&target_config, "to_addresses").if_empty(config_text(&config, "to_addresses"));
    let cc_addresses =
        config_text(&target_config, "cc_addresses").if_empty(config_text(&config, "cc_addresses"));
    let bcc_addresses = config_text(&target_config, "bcc_addresses")
        .if_empty(config_text(&config, "bcc_addresses"));
    let reply_to =
        config_text(&target_config, "reply_to").if_empty(config_text(&config, "reply_to"));
    let to = match parse_mailboxes(&to_addresses, "to_addresses", translator) {
        Ok(value) if !value.is_empty() => value,
        Ok(_) => {
            return missing_config_result(&notification_provider_error_default(
                "email",
                "recipientRequired",
                &[],
            ));
        }
        Err(message) => return missing_config_result(&message),
    };
    let cc = match parse_mailboxes(&cc_addresses, "cc_addresses", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let bcc = match parse_mailboxes(&bcc_addresses, "bcc_addresses", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let reply_to = match parse_mailboxes(&reply_to, "reply_to", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let from = match build_from_mailbox(&from_address, &from_name, translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };

    let fallback_subject = notification_email_message_text(translator, "fallbackTitle", &[]);
    let mut builder = Message::builder()
        .from(from)
        .subject(default_string(subject.clone(), &fallback_subject));
    for item in &to {
        builder = builder.to(item.clone());
    }
    for item in &cc {
        builder = builder.cc(item.clone());
    }
    for item in &bcc {
        builder = builder.bcc(item.clone());
    }
    for item in &reply_to {
        builder = builder.reply_to(item.clone());
    }
    let email = match builder.body(build_email_plain_text_body(message, translator)) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTestResult {
                success: false,
                message: error.to_string(),
                request_summary: None,
                response_summary: None,
            };
        }
    };

    let transport_result = build_smtp_transport(
        &smtp_host,
        smtp_port,
        &smtp_security,
        &auth_mode,
        &smtp_username,
        &smtp_password,
    );
    let mailer = match transport_result {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let request_summary = json!({
        "method": "SMTP",
        "host": smtp_host,
        "port": smtp_port,
        "security": smtp_security,
        "auth_mode": auth_mode,
        "to_count": to.len(),
        "cc_count": cc.len(),
        "bcc_count": bcc.len(),
        "subject_preview": truncate_text(&subject, 160)
    });
    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        mailer.send(email),
    )
    .await
    {
        Ok(Ok(response)) => ProviderTestResult {
            success: true,
            message: notification_service_default_text("testSendSuccess", &[]),
            request_summary: Some(request_summary),
            response_summary: Some(json!({
                "ok": true,
                "code": response.code().to_string(),
                "message": response.message().collect::<Vec<_>>().join("\n")
            })),
        },
        Ok(Err(error)) => ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "error": error.to_string() })),
        },
        Err(_) => ProviderTestResult {
            success: false,
            message: notification_provider_error_default("email", "smtpConnectionTimeout", &[]),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "timeout": true })),
        },
    }
}

async fn send_webhook_test(
    state: &AppState,
    provider: &Value,
    translator: &Translator,
) -> Result<ProviderTestResult, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let url = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| notification_provider_error_default("webhook", "missingUrl", &[]))?;
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value == "POST" || value == "PUT")
        .unwrap_or_else(|| "POST".to_string());
    let message = build_provider_test_message(translator);
    let body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message,
        "context": { "mode": "provider_test" },
        "payload": { "extra_body": {} }
    });

    let mut request = match method.as_str() {
        "PUT" => state.fallback_client.put(url),
        _ => state.fallback_client.post(url),
    }
    .header("content-type", "application/json")
    .header("x-fn-knock-provider", "webhook")
    .json(&body);
    let mut header_names = vec![
        "content-type".to_string(),
        "x-fn-knock-provider".to_string(),
    ];
    if let Some(secret) = config
        .get("shared_secret")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("x-fn-knock-signature", secret);
        header_names.push("x-fn-knock-signature".to_string());
    }
    let request_summary = json!({
        "method": method,
        "url": url,
        "header_names": header_names,
        "body_preview": {
            "title": message.get("title").cloned().unwrap_or(Value::Null),
            "severity": "info",
            "event_id": Value::Null
        }
    });

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let response_summary = json!({
                "status": status.as_u16(),
                "ok": status.is_success(),
                "body_preview": truncate_text(&text, 500)
            });
            if status.is_success() {
                Ok(ProviderTestResult {
                    success: true,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                })
            } else {
                Ok(ProviderTestResult {
                    success: false,
                    message: notification_provider_error_text(
                        translator,
                        "webhook",
                        "requestReturned",
                        &[("status", status.as_u16().to_string())],
                    ),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                })
            }
        }
        Err(error) => Ok(ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        }),
    }
}

async fn send_webhook_delivery(
    state: &AppState,
    provider: &Value,
    target: &Value,
    delivery: &Value,
    trigger: &Value,
    rule: &Value,
    timeout_seconds: i64,
    translator: &Translator,
) -> ProviderTestResult {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let target_config = target
        .get("target_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let Some(base_url) = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return ProviderTestResult {
            success: false,
            message: notification_provider_error_default("webhook", "missingUrl", &[]),
            request_summary: None,
            response_summary: None,
        };
    };
    let endpoint_path = target_config
        .get("endpoint_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let url = resolve_webhook_url(base_url, endpoint_path);
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value == "POST" || value == "PUT")
        .unwrap_or_else(|| "POST".to_string());
    let message = delivery
        .get("message_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let extra_headers = target_config
        .get("extra_headers_json")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let extra_body = target_config
        .get("extra_body_json")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message,
        "context": {
            "trigger_id": trigger.get("id").cloned().unwrap_or(Value::Null),
            "delivery_id": delivery.get("id").cloned().unwrap_or(Value::Null),
            "rule_id": rule.get("id").cloned().unwrap_or(Value::Null),
            "target_id": target.get("id").cloned().unwrap_or(Value::Null),
            "event_id": delivery.get("event_id").cloned().unwrap_or(Value::Null)
        },
        "payload": { "extra_body": extra_body }
    });

    let mut request = match method.as_str() {
        "PUT" => state.fallback_client.put(&url),
        _ => state.fallback_client.post(&url),
    }
    .header("content-type", "application/json")
    .header("x-fn-knock-provider", "webhook");
    let mut header_names = vec![
        "content-type".to_string(),
        "x-fn-knock-provider".to_string(),
    ];
    for (key, value) in extra_headers {
        if let Some(value) = value_to_header_string(&value) {
            header_names.push(key.clone());
            request = request.header(key, value);
        }
    }
    if let Some(secret) = config
        .get("shared_secret")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("x-fn-knock-signature", secret);
        header_names.push("x-fn-knock-signature".to_string());
    }
    let request_summary = json!({
        "method": method,
        "url": url,
        "header_names": header_names,
        "body_preview": {
            "title": body.pointer("/message/title").cloned().unwrap_or(Value::Null),
            "severity": body.pointer("/message/severity").cloned().unwrap_or(Value::Null),
            "event_id": body.pointer("/context/event_id").cloned().unwrap_or(Value::Null)
        }
    });

    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        request.json(&body).send(),
    )
    .await
    {
        Ok(Ok(response)) => {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let response_summary = json!({
                "status": status.as_u16(),
                "ok": status.is_success(),
                "body_preview": truncate_text(&text, 500)
            });
            if status.is_success() {
                ProviderTestResult {
                    success: true,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            } else {
                ProviderTestResult {
                    success: false,
                    message: notification_provider_error_text(
                        translator,
                        "webhook",
                        "requestReturned",
                        &[("status", status.as_u16().to_string())],
                    ),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            }
        }
        Ok(Err(error)) => ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        },
        Err(_) => ProviderTestResult {
            success: false,
            message: notification_provider_error_default("webhook", "requestFailed", &[]),
            request_summary: Some(request_summary),
            response_summary: None,
        },
    }
}

async fn post_json(
    state: &AppState,
    url: &str,
    body: &Value,
    timeout_seconds: i64,
) -> (u16, bool, String, Option<Value>) {
    let request = state
        .fallback_client
        .post(url)
        .header("content-type", "application/json; charset=utf-8");
    send_prepared_json(request, body, timeout_seconds).await
}

async fn send_prepared_json(
    request: reqwest::RequestBuilder,
    body: &Value,
    timeout_seconds: i64,
) -> (u16, bool, String, Option<Value>) {
    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        request.json(body).send(),
    )
    .await
    {
        Ok(Ok(response)) => read_provider_response(response).await,
        Ok(Err(error)) => (599, false, error.to_string(), None),
        Err(_) => (
            599,
            false,
            notification_service_default_text("testSendFailed", &[]),
            None,
        ),
    }
}

async fn post_form(
    state: &AppState,
    url: &str,
    form: &[(String, String)],
    timeout_seconds: i64,
) -> (u16, bool, String, Option<Value>) {
    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in form {
            serializer.append_pair(key, value);
        }
        serializer.finish()
    };
    let request = state
        .fallback_client
        .post(url)
        .header(
            "content-type",
            "application/x-www-form-urlencoded; charset=utf-8",
        )
        .body(body);
    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        request.send(),
    )
    .await
    {
        Ok(Ok(response)) => read_provider_response(response).await,
        Ok(Err(error)) => (599, false, error.to_string(), None),
        Err(_) => (
            599,
            false,
            notification_service_default_text("testSendFailed", &[]),
            None,
        ),
    }
}

async fn read_provider_response(response: reqwest::Response) -> (u16, bool, String, Option<Value>) {
    let status = response.status();
    let ok = status.is_success();
    let text = response.text().await.unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&text).ok();
    (status.as_u16(), ok, text, parsed)
}

fn provider_result_from_api<S, M>(
    provider_label: &str,
    request_summary: Value,
    status: u16,
    ok: bool,
    text: String,
    parsed: Option<Value>,
    success_check: S,
    message_getter: M,
) -> ProviderTestResult
where
    S: Fn(&Value) -> bool,
    M: Fn(&Value) -> Option<String>,
{
    let parsed_value = parsed.as_ref().unwrap_or(&Value::Null);
    let success = ok && success_check(parsed_value);
    let api_message = message_getter(parsed_value);
    ProviderTestResult {
        success,
        message: if success {
            notification_service_default_text("testSendSuccess", &[])
        } else {
            api_message.unwrap_or_else(|| {
                if status == 599 && !text.is_empty() {
                    text.clone()
                } else {
                    format!("{provider_label} request returned status {status}")
                }
            })
        },
        request_summary: Some(request_summary),
        response_summary: Some(json!({
            "status": status,
            "ok": ok,
            "body_preview": truncate_text(&text, 500),
            "json": parsed.unwrap_or(Value::Null)
        })),
    }
}

fn missing_config_result(message: &str) -> ProviderTestResult {
    ProviderTestResult {
        success: false,
        message: message.to_string(),
        request_summary: None,
        response_summary: None,
    }
}

fn provider_timeout_seconds(provider: &Value, fallback: i64) -> i64 {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .and_then(|config| config.get("timeout_seconds"))
        .map(|value| value_to_i64(value, fallback))
        .unwrap_or(fallback)
        .clamp(1, 30)
}

fn provider_config(provider: &Value) -> Map<String, Value> {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn target_config(target: &Value) -> Map<String, Value> {
    target
        .get("target_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn config_text(config: &Map<String, Value>, key: &str) -> String {
    config
        .get(key)
        .map(value_to_trimmed_string)
        .unwrap_or_default()
}

fn first_config_text(config: &Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            let value = config_text(config, key);
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_default()
}

fn effective_config_value(
    provider_config: &Map<String, Value>,
    target_config: &Map<String, Value>,
    keys: &[&str],
) -> Option<Value> {
    for key in keys {
        if let Some(value) = target_config.get(*key)
            && !value_is_empty(value)
            && value_to_trimmed_string(value) != "__inherit__"
        {
            return Some(value.clone());
        }
    }
    for key in keys {
        if let Some(value) = provider_config.get(*key)
            && !value_is_empty(value)
        {
            return Some(value.clone());
        }
    }
    None
}

fn value_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.trim().is_empty(),
        _ => false,
    }
}

fn split_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .map(value_to_trimmed_string)
            .filter(|value| !value.is_empty())
            .collect(),
        Some(value) => value_to_trimmed_string(value)
            .split([',', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

fn parse_topic_ids(value: Option<&Value>) -> (Vec<i64>, Vec<String>) {
    let mut ids = Vec::new();
    let mut invalid = Vec::new();
    for item in split_values(value) {
        if !item.chars().all(|ch| ch.is_ascii_digit()) {
            invalid.push(item);
            continue;
        }
        match item.parse::<i64>() {
            Ok(value) if value > 0 => ids.push(value),
            _ => invalid.push(item),
        }
    }
    (ids, invalid)
}

fn optional_positive_i64(value: Option<&Value>) -> Option<i64> {
    value
        .map(|value| value_to_i64(value, 0))
        .filter(|value| *value > 0)
}

fn optional_nonnegative_i64(value: Option<&Value>) -> Option<i64> {
    value
        .map(|value| value_to_i64(value, -1))
        .filter(|value| *value >= 0)
}

fn message_text(message: &Value, key: &str) -> String {
    message
        .get(key)
        .map(value_to_trimmed_string)
        .unwrap_or_default()
}

fn message_title(message: &Value) -> String {
    default_string(
        message_text(message, "title").if_empty(message_text(message, "summary")),
        DEFAULT_NOTIFICATION_MESSAGE_TITLE,
    )
}

fn message_summary(message: &Value) -> String {
    message_text(message, "summary")
}

trait EmptyStringExt {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}

fn default_string(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn non_empty_or(first: String, second: String, third: &str) -> String {
    if !first.trim().is_empty() {
        first
    } else if !second.trim().is_empty() {
        second
    } else {
        third.to_string()
    }
}

fn empty_to_null(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

fn build_text_body(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    let body_text = message_text(message, "body_text");
    if body_text.is_empty() {
        push_if_non_empty(&mut sections, message_text(message, "body_markdown"));
    } else {
        push_if_non_empty(&mut sections, body_text);
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_markdown_body(message: &Value, tail: &str) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    let body_markdown = message_text(message, "body_markdown");
    if body_markdown.is_empty() {
        push_if_non_empty(
            &mut sections,
            normalize_multiline_trimmed(&message_text(message, "body_text"), true),
        );
    } else {
        push_if_non_empty(&mut sections, body_markdown);
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_markdown_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_markdown_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    push_if_non_empty(&mut sections, tail.to_string());
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_pushplus_text_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        push_if_non_empty(&mut sections, normalize_multiline_trimmed(&body_text, true));
    } else {
        push_if_non_empty(&mut sections, message_text(message, "body_markdown"));
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_fullwidth_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    default_string(
        sections
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        &message_title(message),
    )
}

fn build_pushplus_markdown_content(message: &Value) -> String {
    let body = build_markdown_body(message, "");
    if body.trim().is_empty() {
        build_pushplus_text_content(message)
    } else {
        body
    }
}

fn build_pushplus_html_content(message: &Value) -> String {
    let mut sections = Vec::new();
    let summary = message_summary(message);
    if !summary.is_empty() {
        sections.push(format!("<p>{}</p>", escape_html(&summary)));
    }
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let body_html = normalize_multiline_trimmed(&body_text, true)
            .lines()
            .map(escape_html)
            .collect::<Vec<_>>()
            .join("<br />");
        if !body_html.is_empty() {
            sections.push(format!("<p>{body_html}</p>"));
        }
    } else {
        let body_markdown = message_text(message, "body_markdown");
        if !body_markdown.is_empty() {
            sections.push(format!("<pre>{}</pre>", escape_html(&body_markdown)));
        }
    }
    push_html_facts(&mut sections, message);
    push_html_action_list(&mut sections, message);
    default_string(
        sections
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(""),
        &format!("<p>{}</p>", escape_html(&message_title(message))),
    )
}

fn build_wxpusher_html_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(
        &mut sections,
        format!("<h2>{}</h2>", escape_html(&message_title(message))),
    );
    let summary = message_summary(message);
    if !summary.is_empty() {
        sections.push(format!("<p>{}</p>", escape_html(&summary)));
    }
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let paragraphs = normalize_multiline_trimmed(&body_text, true)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| format!("<p>{}</p>", escape_html(line)))
            .collect::<String>();
        push_if_non_empty(&mut sections, paragraphs);
    }
    push_html_facts(&mut sections, message);
    push_html_actions_as_paragraphs(&mut sections, message);
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

fn push_html_facts(sections: &mut Vec<String>, message: &Value) {
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        let items = facts
            .iter()
            .filter_map(|fact| {
                let label = fact.get("label").map(value_to_trimmed_string)?;
                let value = fact.get("value").map(value_to_trimmed_string)?;
                Some(format!(
                    "<li><strong>{}</strong>：{}</li>",
                    escape_html(&label),
                    escape_html(&value)
                ))
            })
            .collect::<String>();
        push_if_non_empty(sections, format!("<ul>{items}</ul>"));
    }
}

fn push_html_action_list(sections: &mut Vec<String>, message: &Value) {
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        let items = actions
            .iter()
            .filter_map(|action| {
                let label = action.get("label").map(value_to_trimmed_string)?;
                let url = action.get("url").map(value_to_trimmed_string)?;
                if label.is_empty() || url.is_empty() {
                    None
                } else {
                    Some(format!(
                        "<li><a href=\"{}\">{}</a></li>",
                        escape_html(&url),
                        escape_html(&label)
                    ))
                }
            })
            .collect::<String>();
        push_if_non_empty(sections, format!("<ul>{items}</ul>"));
    }
}

fn push_html_actions_as_paragraphs(sections: &mut Vec<String>, message: &Value) {
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        let items = actions
            .iter()
            .filter_map(|action| {
                let label = action.get("label").map(value_to_trimmed_string)?;
                let url = action.get("url").map(value_to_trimmed_string)?;
                if label.is_empty() || url.is_empty() {
                    None
                } else {
                    Some(format!(
                        "<p><a href=\"{}\">{}</a></p>",
                        escape_html(&url),
                        escape_html(&label)
                    ))
                }
            })
            .collect::<String>();
        push_if_non_empty(sections, items);
    }
}

fn build_pushplus_json_content(message: &Value) -> String {
    serde_json::to_string_pretty(&json!({
        "summary": message.get("summary").cloned().unwrap_or(Value::Null),
        "body_text": message.get("body_text").cloned().unwrap_or(Value::Null),
        "body_markdown": message.get("body_markdown").cloned().unwrap_or(Value::Null),
        "severity": message.get("severity").cloned().unwrap_or(Value::Null),
        "facts": message.get("facts").cloned().unwrap_or_else(|| json!([])),
        "actions": message.get("actions").cloned().unwrap_or_else(|| json!([])),
        "occurred_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
        "event_id": message.get("event_id").cloned().unwrap_or(Value::Null),
        "metadata": message.get("metadata").cloned().unwrap_or_else(|| json!({})),
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn build_wecom_markdown_content(message: &Value, mentioned_list: &[String]) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(
        &mut sections,
        format!("# {}", sanitize_wecom_text(&message_title(message))),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&message_summary(message)),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&normalize_multiline_trimmed(
            &message_text(message, "body_text"),
            true,
        )),
    );
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        push_if_non_empty(
            &mut sections,
            facts
                .iter()
                .filter_map(|fact| {
                    let label =
                        sanitize_wecom_text(&fact.get("label").map(value_to_trimmed_string)?);
                    let value =
                        sanitize_wecom_text(&fact.get("value").map(value_to_trimmed_string)?);
                    Some(format!("> {label}：{value}"))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array) {
        push_if_non_empty(
            &mut sections,
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .map(|line| format!("> {}", sanitize_wecom_text(&line)))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if !mentioned_list.is_empty() {
        sections.push(
            mentioned_list
                .iter()
                .map(|value| {
                    if value.starts_with('@') {
                        format!("<{value}>")
                    } else {
                        format!("<@{value}>")
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_wecom_text_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, sanitize_wecom_text(&message_title(message)));
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&message_summary(message)),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&normalize_multiline_trimmed(
            &message_text(message, "body_text"),
            true,
        )),
    );
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_fullwidth_plain_line)
                .map(|line| sanitize_wecom_text(&line))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .map(|line| sanitize_wecom_text(&line))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_dingtalk_mention_text(
    at_mobiles: &[String],
    at_user_ids: &[String],
    is_at_all: bool,
) -> String {
    let mut tokens = Vec::new();
    if is_at_all {
        tokens.push("@all".to_string());
    }
    tokens.extend(at_mobiles.iter().map(|value| format!("@{value}")));
    tokens.extend(at_user_ids.iter().map(|value| format!("@{value}")));
    tokens.join(" ")
}

fn build_feishu_post_content(message: &Value, mention_user_ids: &[String]) -> Value {
    let mut paragraphs: Vec<Value> = Vec::new();
    let body_source =
        message_text(message, "body_text").if_empty(message_text(message, "body_markdown"));
    for section in [message_summary(message), body_source] {
        for line in section
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            paragraphs.push(json!([{ "tag": "text", "text": line }]));
        }
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        for fact in facts {
            if let Some(line) = fact_fullwidth_plain_line(fact) {
                paragraphs.push(json!([{ "tag": "text", "text": line }]));
            }
        }
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array) {
        for action in actions {
            let label = action
                .get("label")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            let url = action
                .get("url")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            if !label.is_empty() && !url.is_empty() {
                paragraphs.push(json!([{ "tag": "a", "text": label, "href": url }]));
            }
        }
    }
    if !mention_user_ids.is_empty() {
        paragraphs.push(Value::Array(
            mention_user_ids
                .iter()
                .map(|user_id| {
                    if user_id == "all" {
                        json!({ "tag": "at", "user_id": "all", "user_name": "所有人" })
                    } else {
                        json!({ "tag": "at", "user_id": user_id })
                    }
                })
                .collect(),
        ));
    }
    if paragraphs.is_empty() {
        paragraphs.push(json!([{ "tag": "text", "text": message_title(message) }]));
    }
    Value::Array(paragraphs)
}

fn build_magicpush_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    push_if_non_empty(&mut sections, message_text(message, "body_text"));
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn magicpush_facts_object(message: &Value) -> Value {
    let mut facts = Map::new();
    if let Some(values) = message.get("facts").and_then(Value::as_array) {
        for fact in values {
            let label = fact
                .get("label")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            if label.is_empty() {
                continue;
            }
            let value = fact
                .get("value")
                .map(js_string_like_node)
                .unwrap_or_default();
            facts.insert(label, Value::String(value));
        }
    }
    Value::Object(facts)
}

fn build_bark_payload(message: &Value, target: &Value) -> Value {
    let target_config = target_config(target);
    let summary = message_summary(message);
    let body_text = message_text(message, "body_text");
    let has_standalone_body = !body_text.is_empty() && body_text != summary;
    let mut payload = json!({
        "title": message_title(message),
        "body": if has_standalone_body { body_text.clone() } else { default_string(summary.clone(), &message_title(message)) },
        "level": default_string(config_text(&target_config, "level"), "active")
    });
    if has_standalone_body && !summary.is_empty() {
        insert_string(&mut payload, "subtitle", summary);
    }
    for key in ["group", "sound", "url", "icon"] {
        insert_non_empty(&mut payload, key, config_text(&target_config, key));
    }
    if let Some(action_url) = (!payload.get("url").is_some()).then(|| primary_action_url(message))
        && !action_url.is_empty()
    {
        insert_string(&mut payload, "url", action_url);
    }
    if let Some(badge) = optional_nonnegative_i64(target_config.get("badge")) {
        insert_i64(&mut payload, "badge", badge);
    }
    if target_config
        .get("call")
        .map(value_to_bool)
        .unwrap_or(false)
    {
        insert_string(&mut payload, "call", "1".to_string());
    }
    payload
}

fn build_telegram_text(message: &Value) -> String {
    let mut plain_sections = Vec::new();
    let mut rich_sections = Vec::new();
    let title = message_title(message);
    push_if_non_empty(&mut plain_sections, title.clone());
    push_if_non_empty(
        &mut rich_sections,
        format!("<b>{}</b>", escape_html(&title)),
    );
    let summary = message_summary(message);
    push_if_non_empty(&mut plain_sections, summary.clone());
    push_if_non_empty(&mut rich_sections, escape_html(&summary));
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let normalized = normalize_multiline_trimmed(&body_text, false);
        plain_sections.push(normalized.clone());
        rich_sections.push(
            normalized
                .lines()
                .map(escape_html)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        push_if_non_empty(
            &mut plain_sections,
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        push_if_non_empty(
            &mut rich_sections,
            facts
                .iter()
                .filter_map(|fact| {
                    let label = escape_html(&fact.get("label").map(value_to_trimmed_string)?);
                    let value = escape_html(&fact.get("value").map(value_to_trimmed_string)?);
                    Some(format!("<b>{label}:</b> {value}"))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let rich_text = rich_sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if rich_text.encode_utf16().count() <= 4096 {
        rich_text
    } else {
        escape_html(&truncate_text(
            &plain_sections
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n"),
            4096,
        ))
    }
}

fn build_telegram_reply_markup(message: &Value) -> Option<Value> {
    let buttons = message
        .get("actions")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|action| {
            let label = action.get("label").map(value_to_trimmed_string)?;
            let url = action.get("url").map(value_to_trimmed_string)?;
            if label.is_empty() || url.is_empty() {
                None
            } else {
                Some(json!([{ "text": label, "url": url }]))
            }
        })
        .collect::<Vec<_>>();
    (!buttons.is_empty()).then(|| json!({ "inline_keyboard": buttons }))
}

fn fact_plain_line(fact: &Value) -> Option<String> {
    let label = fact.get("label").map(value_to_trimmed_string)?;
    let value = fact.get("value").map(value_to_trimmed_string)?;
    if label.is_empty() && value.is_empty() {
        None
    } else if label.is_empty() {
        Some(value)
    } else if value.is_empty() {
        Some(label)
    } else {
        Some(format!("{label}: {value}"))
    }
}

fn fact_fullwidth_plain_line(fact: &Value) -> Option<String> {
    let label = fact.get("label").map(value_to_trimmed_string)?;
    let value = fact.get("value").map(value_to_trimmed_string)?;
    if label.is_empty() && value.is_empty() {
        None
    } else if label.is_empty() {
        Some(value)
    } else if value.is_empty() {
        Some(label)
    } else {
        Some(format!("{label}：{value}"))
    }
}

fn fact_markdown_line(fact: &Value) -> Option<String> {
    let label = fact.get("label").map(value_to_trimmed_string)?;
    let value = fact.get("value").map(value_to_trimmed_string)?;
    if label.is_empty() && value.is_empty() {
        None
    } else {
        Some(format!("- **{label}**：{value}"))
    }
}

fn action_plain_line(action: &Value) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("{label}: {url}"))
    }
}

fn action_fullwidth_plain_line(action: &Value) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("{label}：{url}"))
    }
}

fn action_markdown_line(action: &Value) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("- [{label}]({url})"))
    }
}

fn primary_action_url(message: &Value) -> String {
    message
        .get("actions")
        .and_then(Value::as_array)
        .and_then(|actions| {
            actions.iter().find_map(|action| {
                let url = action.get("url").map(value_to_trimmed_string)?;
                (!url.is_empty()).then_some(url)
            })
        })
        .unwrap_or_default()
}

fn push_if_non_empty(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() {
        values.push(value);
    }
}

fn normalize_multiline_trimmed(value: &str, drop_empty_lines: bool) -> String {
    let lines = value.lines().map(str::trim);
    if drop_empty_lines {
        lines
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        lines.collect::<Vec<_>>().join("\n")
    }
}

fn push_form_if(form: &mut Vec<(String, String)>, key: &str, value: String) {
    if !value.trim().is_empty() {
        form.push((key.to_string(), value));
    }
}

fn insert_non_empty(object: &mut Value, key: &str, value: String) {
    if !value.trim().is_empty() {
        insert_string(object, key, value);
    }
}

fn insert_string(object: &mut Value, key: &str, value: String) {
    insert_value(object, key, Value::String(value));
}

fn insert_i64(object: &mut Value, key: &str, value: i64) {
    insert_value(object, key, json!(value));
}

fn insert_value(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn truncate_utf8_bytes(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut bytes = 0;
    let mut output = String::new();
    for ch in value.chars() {
        let len = ch.len_utf8();
        if bytes + len > limit {
            break;
        }
        bytes += len;
        output.push(ch);
    }
    output
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sanitize_wecom_text(value: &str) -> String {
    value.replace('<', "＜").replace('>', "＞")
}

fn apply_keyword_prefix(value: &str, keyword: &str) -> String {
    let keyword = keyword.trim();
    let value = value.trim();
    if keyword.is_empty() || value.contains(keyword) {
        value.to_string()
    } else if value.is_empty() {
        keyword.to_string()
    } else {
        format!("[{keyword}] {value}")
    }
}

fn hmac_sha256_base64(key: &[u8], payload: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    BASE64_STANDARD.encode(mac.finalize().into_bytes())
}

fn append_query_params(url: &str, params: &[(&str, String)]) -> String {
    if let Ok(mut parsed) = url::Url::parse(url) {
        {
            let mut query = parsed.query_pairs_mut();
            for (key, value) in params {
                query.append_pair(key, value);
            }
        }
        return parsed.to_string();
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    let query = serializer.finish();
    format!(
        "{}{}{}",
        url,
        if url.contains('?') { "&" } else { "?" },
        query
    )
}

fn redact_query_value(value: &str, key: &str) -> String {
    if let Ok(mut url) = url::Url::parse(value) {
        if url.query_pairs().any(|(name, _)| name == key) {
            let pairs = url
                .query_pairs()
                .map(|(name, value)| {
                    if name == key {
                        (name.to_string(), "<redacted>".to_string())
                    } else {
                        (name.to_string(), value.to_string())
                    }
                })
                .collect::<Vec<_>>();
            url.set_query(None);
            for (name, value) in pairs {
                url.query_pairs_mut().append_pair(&name, &value);
            }
        }
        return url.to_string();
    }
    value.replace(&format!("{key}="), &format!("{key}=<redacted>"))
}

fn redact_path_tail(value: &str) -> String {
    if let Ok(mut url) = url::Url::parse(value) {
        if let Some(mut segments) = url.path_segments_mut().ok() {
            segments.pop().push("<redacted>");
        }
        return url.to_string();
    }
    value.to_string()
}

fn resolve_pushplus_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with("/send") || lower.ends_with("/batchsend") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/send")
    }
}

fn resolve_magicpush_url(base_url: &str, token: &str, delivery_mode: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let lower = base.to_ascii_lowercase();
    if delivery_mode == "inbound" {
        if path_matches_magicpush_endpoint_with_tail(&lower, "/api/inbound") {
            base.to_string()
        } else if lower.ends_with("/api/inbound") {
            format!("{base}/{}", url_encode_component(token))
        } else {
            format!("{base}/api/inbound/{}", url_encode_component(token))
        }
    } else if lower.ends_with("/api/push")
        || path_matches_magicpush_endpoint_with_tail(&lower, "/api/push")
    {
        base.to_string()
    } else {
        format!("{base}/api/push")
    }
}

fn path_matches_magicpush_endpoint_with_tail(value: &str, endpoint: &str) -> bool {
    let Some((prefix, tail)) = value.rsplit_once('/') else {
        return false;
    };
    prefix.ends_with(endpoint) && !tail.is_empty() && !tail.contains('/')
}

fn url_encode_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(value_as_i64)
}

fn json_i64_any(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| json_i64(value, key))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|value| parse_int_prefix_like_node(value, 10))
    })
}

fn json_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .map(value_to_trimmed_string)
        .filter(|value| !value.is_empty())
}

fn json_text_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| json_text(value, key))
}

fn default_smtp_port(security: &str) -> i64 {
    match security {
        "starttls" => 587,
        "none" => 25,
        _ => 465,
    }
}

fn build_smtp_transport(
    host: &str,
    port: u16,
    security: &str,
    auth_mode: &str,
    username: &str,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let mut builder = match security {
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host).tls(Tls::None),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|error| error.to_string())?,
        _ => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host).map_err(|error| error.to_string())?
        }
    }
    .port(port);
    if auth_mode != "none" && !username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(username.to_string(), password.to_string()));
    }
    Ok(builder.build())
}

fn parse_mailboxes(
    value: &str,
    field_key: &str,
    translator: &Translator,
) -> Result<Vec<Mailbox>, String> {
    let field_label =
        notification_provider_field_text(translator, "email", field_key, "addressLabel", field_key);
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<Mailbox>().map_err(|_| {
                notification_provider_error_text(
                    translator,
                    "email",
                    "invalidEmailAddress",
                    &[("field", field_label.clone()), ("value", value.to_string())],
                )
            })
        })
        .collect()
}

fn build_from_mailbox(
    address: &str,
    name: &str,
    translator: &Translator,
) -> Result<Mailbox, String> {
    if name.trim().is_empty() {
        return address.parse::<Mailbox>().map_err(|_| {
            notification_provider_error_text(translator, "email", "invalidFromAddress", &[])
        });
    }
    let address = address.parse::<Address>().map_err(|_| {
        notification_provider_error_text(translator, "email", "invalidFromAddress", &[])
    })?;
    Ok(Mailbox::new(Some(name.trim().to_string()), address))
}

fn build_email_plain_text_body(message: &Value, translator: &Translator) -> String {
    let mut body = build_text_body(message);
    let mut footer = Vec::new();
    let severity = message_text(message, "severity");
    if !severity.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "severity",
            &[("value", severity)],
        ));
    }
    let event_id = message_text(message, "event_id");
    if !event_id.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "eventId",
            &[("value", event_id)],
        ));
    }
    let occurred_at = message_text(message, "occurred_at");
    if !occurred_at.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "occurredAt",
            &[("value", occurred_at)],
        ));
    }
    if !footer.is_empty() {
        body.push_str("\n\n");
        body.push_str(&footer.join("\n"));
    }
    body
}

fn resolve_webhook_url(base_url: &str, endpoint_path: &str) -> String {
    if endpoint_path.trim().is_empty() {
        return base_url.to_string();
    }
    if let Ok(base) = url::Url::parse(base_url)
        && let Ok(joined) = base.join(endpoint_path)
    {
        return joined.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        endpoint_path.trim_start_matches('/')
    )
}

fn value_to_header_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn apply_provider_test_result(provider: &mut Value, result: &ProviderTestResult) {
    let Some(object) = provider.as_object_mut() else {
        return;
    };
    object.insert(
        "last_test_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    object.insert(
        "last_test_status".to_string(),
        Value::String(if result.success { "success" } else { "failed" }.to_string()),
    );
    object.insert(
        "last_error".to_string(),
        if result.success {
            Value::Null
        } else {
            Value::String(result.message.clone())
        },
    );
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
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

fn object_field(value: &Value, key: &str) -> Map<String, Value> {
    value
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn trimmed_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn value_to_trimmed_string(value: &Value) -> String {
    js_string_like_node(value).trim().to_string()
}

fn value_to_i64(value: &Value, fallback: i64) -> i64 {
    js_number_like_node(value)
        .map(|value| value.floor() as i64)
        .unwrap_or(fallback)
}

fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn js_number_like_node(value: &Value) -> Option<f64> {
    match value {
        Value::Null => Some(0.0),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        Value::Number(value) => value.as_f64().filter(|value| value.is_finite()),
        Value::String(value) => js_number_from_string_like_node(value),
        Value::Array(values) => {
            let text = values
                .iter()
                .map(js_string_like_node)
                .collect::<Vec<_>>()
                .join(",");
            js_number_from_string_like_node(&text)
        }
        Value::Object(_) => None,
    }
}

fn js_number_from_string_like_node(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };

    match radix_value {
        Some(value) => Some(value),
        None => trimmed.parse::<f64>().ok(),
    }
    .filter(|value| value.is_finite())
}

fn parse_int_prefix_like_node(value: &str, radix: u32) -> Option<i64> {
    let trimmed = value.trim_start();
    let mut chars = trimmed.char_indices();
    let mut end = 0;
    let mut saw_digit = false;
    if let Some((_, first)) = chars.clone().next()
        && (first == '+' || first == '-')
    {
        end = first.len_utf8();
        chars.next();
    }
    for (index, ch) in chars {
        if ch.to_digit(radix).is_some() {
            saw_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    trimmed[..end].parse::<i64>().ok()
}

fn js_string_like_node(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(js_string_like_node)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn bool_field(value: &Value, key: &str, fallback: bool) -> bool {
    value.get(key).map(value_to_bool).unwrap_or(fallback)
}

fn number_field(value: &Value, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .get(key)
        .map(|value| value_to_i64(value, fallback))
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn unique_string_array(value: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    if let Some(array) = value.and_then(Value::as_array) {
        for item in array {
            let text = value_to_trimmed_string(item);
            if text.is_empty() || !seen.insert(text.clone()) {
                continue;
            }
            values.push(text);
        }
    }
    values
}

fn parse_positive_int(value: Option<&str>, fallback: i64, max: i64) -> i64 {
    value
        .and_then(|value| parse_i64_prefix_like_node(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
        .min(max)
}

fn parse_i64_prefix_like_node(value: &str) -> Option<i64> {
    let mut chars = value.chars().peekable();
    let negative = match chars.peek() {
        Some('+') => {
            chars.next();
            false
        }
        Some('-') => {
            chars.next();
            true
        }
        _ => false,
    };
    let digits = chars
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let magnitude = digits.parse::<u128>().unwrap_or(u128::MAX);
    if negative {
        if magnitude >= (i64::MAX as u128) + 1 {
            Some(i64::MIN)
        } else {
            Some(-(magnitude as i64))
        }
    } else if magnitude > i64::MAX as u128 {
        Some(i64::MAX)
    } else {
        Some(magnitude as i64)
    }
}

fn matches_optional_string(value: &Value, key: &str, expected: Option<&str>) -> bool {
    let Some(expected) = expected.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    value.get(key).and_then(Value::as_str) == Some(expected)
}

fn parse_json_body(body: &Bytes, translator: &Translator) -> Result<Value, String> {
    if body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(body)
        .map_err(|_| notification_service_text(translator, "invalidJsonBody", &[]))
}

fn iso_score_ms(value: Option<&str>) -> i64 {
    value
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or_else(time_utils::now_ms)
}

fn build_next_sequential_name(base: &str, existing_names: &[String]) -> String {
    let base = if base.trim().is_empty() {
        notification_service_default_text("unnamed", &[])
    } else {
        base.trim().to_string()
    };
    let prefix = format!("{base} ");
    let used = existing_names
        .iter()
        .filter_map(|name| name.trim().strip_prefix(&prefix))
        .filter_map(|suffix| suffix.parse::<usize>().ok())
        .collect::<HashSet<_>>();
    let mut index = 1;
    while used.contains(&index) {
        index += 1;
    }
    format!("{base} {index}")
}

fn build_notification_rule_name(event_type: &str, translator: &Translator) -> String {
    let event = format_notification_event_label(event_type, translator);
    notification_template_text(translator, "ruleName", &[("event", event)])
}

fn build_notification_title(event: &Value, matched_count: i64, translator: &Translator) -> String {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    let base = if event_type == "FN_EVENT_DDNS_UPDATE_COMPLETED" {
        let target = read_payload_value(event, "target_name")
            .if_empty(read_payload_value(event, "domain_summary"))
            .if_empty("DDNS".to_string());
        if read_payload_value(event, "success") == "true" {
            notification_detail_text(
                translator,
                "titles.ddnsUpdateSuccess",
                &[("target", target)],
            )
        } else {
            notification_detail_text(
                translator,
                "titles.ddnsUpdateFailure",
                &[("target", target)],
            )
        }
    } else if event_type == "FN_EVENT_AUTH_SESSION_IP_DRIFT" {
        let credential_name = read_payload_value(event, "credential_name");
        if credential_name.is_empty() {
            format_notification_event_label(event_type, translator)
        } else {
            notification_detail_text(
                translator,
                "titles.credentialIpDrift",
                &[("credential", credential_name)],
            )
        }
    } else if event_type == "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" {
        notification_detail_text(
            translator,
            "titles.appUpdateAvailable",
            &[("version", read_payload_value(event, "latest_version"))],
        )
        .trim()
        .to_string()
    } else {
        format_notification_event_label(event_type, translator)
    };
    if matched_count > 1 {
        format!("{base} x{matched_count}")
    } else {
        base
    }
}

fn format_notification_event_label(event_type: &str, translator: &Translator) -> String {
    let Some(key) = notification_event_label_key(event_type) else {
        return event_type.to_string();
    };
    notification_template_text(translator, key, &[])
}

fn notification_event_label_key(event_type: &str) -> Option<&'static str> {
    Some(match event_type {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => "events.authLoginSuccess",
        "FN_EVENT_AUTH_LOGOUT" => "events.authLogout",
        "FN_EVENT_AUTH_LOGIN_FAILURE" => "events.authLoginFailure",
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => "events.authSessionIpDrift",
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => "events.securityScannerBlocked",
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => "events.ddnsUpdateCompleted",
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => "events.gatewayThrottleBlocked",
        "FN_EVENT_WAF_BLOCKED" => "events.wafBlocked",
        "FN_EVENT_SSH_LOGIN_SUCCESS" => "events.sshLoginSuccess",
        "FN_EVENT_SSH_LOGIN_FAILURE" => "events.sshLoginFailure",
        "FN_EVENT_SSH_IP_BLOCKED" => "events.sshIpBlocked",
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => "events.appUpdateAvailable",
        "FN_EVENT_SYSTEM_CPU_ALERT" => "events.cpuAlert",
        "FN_EVENT_SYSTEM_CPU_RECOVERED" => "events.cpuRecovered",
        "FN_EVENT_SYSTEM_MEMORY_ALERT" => "events.memoryAlert",
        "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => "events.memoryRecovered",
        "FN_EVENT_TUNNEL_FRP_CONNECTED" => "events.frpConnected",
        "FN_EVENT_TUNNEL_FRP_DISCONNECTED" => "events.frpDisconnected",
        "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED" => "events.cloudflaredConnected",
        "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => "events.cloudflaredDisconnected",
        _ => return None,
    })
}

fn format_notification_level_label(level: &str, translator: &Translator) -> String {
    let key = match level {
        "WARN" => "levels.warn",
        "ERROR" => "levels.error",
        "CRITICAL" => "levels.critical",
        _ => "levels.info",
    };
    notification_template_text(translator, key, &[])
}

fn format_notification_source_label(source: &str, translator: &Translator) -> String {
    let key = match source {
        "GO_REAUTH_PROXY" => "sources.goReauthProxy",
        "SYSTEM_MONITOR" => "sources.systemMonitor",
        _ => "sources.serverAdmin",
    };
    notification_template_text(translator, key, &[])
}

fn notification_template_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.notifications.templates.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn notification_detail_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    notification_template_text(translator, &format!("details.{key}"), params)
}

fn notification_fact_label(translator: &Translator, key: &str) -> String {
    notification_detail_text(translator, &format!("facts.{key}"), &[])
}

fn create_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

fn create_stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("\u{0}"));
    let digest = hex::encode(hasher.finalize());
    format!("{prefix}_{}", &digest[..24])
}

fn create_runtime_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}_{}",
        std::process::id(),
        time_utils::now_ms(),
        hex::encode(rand::random::<[u8; 6]>())
    )
}

fn truncate_text(value: &str, max_len: usize) -> String {
    let mut result = value.chars().take(max_len).collect::<String>();
    if value.chars().count() > max_len {
        result.push('…');
    }
    result
}

async fn internal_error(state: &AppState, context: &str, error: redis::RedisError) -> Response {
    let translator = Translator::from_state(state).await;
    tracing::warn!(%error, "{context}");
    response::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        notification_service_text(&translator, "storageUnavailable", &[]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_field<'a>(view: &'a Value, schema: &str, key: &str) -> &'a Value {
        view.get(schema)
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|field| field.get("key").and_then(Value::as_str) == Some(key))
            .unwrap()
    }

    #[test]
    fn masks_sensitive_provider_values_like_node() {
        assert_eq!(mask_sensitive_value(&json!("short")), json!("********"));
        assert_eq!(
            mask_sensitive_value(&json!("abcdefghijkl")),
            json!("ab******")
        );
        assert_eq!(mask_sensitive_value(&json!(true)), json!("[configured]"));
    }

    #[test]
    fn provider_test_result_updates_provider_status_like_node() {
        let mut provider = json!({
            "id": "ntfprov_1",
            "last_test_status": "idle",
            "last_error": "old error"
        });
        apply_provider_test_result(
            &mut provider,
            &ProviderTestResult {
                success: true,
                message: "ok".to_string(),
                request_summary: None,
                response_summary: None,
            },
        );
        assert_eq!(provider.get("last_test_status"), Some(&json!("success")));
        assert_eq!(provider.get("last_error"), Some(&Value::Null));
        assert!(
            provider
                .get("last_test_at")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        );
    }

    #[test]
    fn applies_schema_defaults_and_required_validation() {
        let definition = provider_definition("webhook").unwrap();
        let mut raw = Map::new();
        raw.insert("url".to_string(), json!(" https://example.com/hook "));
        let normalized = normalize_schema_config(&raw, &definition.connection_schema).unwrap();
        assert_eq!(
            normalized.get("url"),
            Some(&json!("https://example.com/hook"))
        );
        assert_eq!(normalized.get("method"), Some(&json!("POST")));
        assert_eq!(normalized.get("timeout_seconds"), Some(&json!(5)));
        validate_required_fields(&normalized, &definition.connection_schema).unwrap();
    }

    #[test]
    fn rejects_invalid_select_values() {
        let definition = provider_definition("webhook").unwrap();
        let mut raw = Map::new();
        raw.insert("method".to_string(), json!("DELETE"));
        assert!(normalize_schema_patch(&raw, &definition.connection_schema).is_err());
    }

    #[test]
    fn schema_boolean_values_follow_node_truthiness() {
        let definition = provider_definition("bark").unwrap();
        let mut raw = Map::new();

        raw.insert("call".to_string(), json!("false"));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("call"),
            Some(&json!(true))
        );

        raw.insert("call".to_string(), json!(""));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("call"),
            Some(&json!(false))
        );

        raw.insert("call".to_string(), json!(0));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("call"),
            Some(&json!(false))
        );

        raw.insert("call".to_string(), json!({}));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("call"),
            Some(&json!(true))
        );
    }

    #[test]
    fn schema_string_values_follow_node_string_coercion() {
        let definition = provider_definition("webhook").unwrap();
        let mut raw = Map::new();
        raw.insert("endpoint_path".to_string(), json!({ "path": "/alerts" }));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("endpoint_path"),
            Some(&json!("[object Object]"))
        );

        raw.insert("endpoint_path".to_string(), json!(["alerts", 1, null]));
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("endpoint_path"),
            Some(&json!("alerts,1,"))
        );
    }

    #[test]
    fn json_schema_whitespace_matches_node_parse_behavior() {
        let definition = provider_definition("webhook").unwrap();
        let mut raw = Map::new();

        raw.insert("extra_headers_json".to_string(), json!(""));
        assert!(
            !normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .contains_key("extra_headers_json")
        );

        raw.insert("extra_headers_json".to_string(), json!("   "));
        assert!(normalize_schema_patch(&raw, &definition.target_schema).is_err());

        raw.insert(
            "extra_headers_json".to_string(),
            json!(" {\"X-Env\":\"prod\"} "),
        );
        assert_eq!(
            normalize_schema_patch(&raw, &definition.target_schema)
                .unwrap()
                .get("extra_headers_json"),
            Some(&json!({ "X-Env": "prod" }))
        );
    }

    #[test]
    fn notification_number_fields_follow_node_number_coercion() {
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            1
        );
        assert_eq!(
            number_field(
                &json!({ "threshold_count": null }),
                "threshold_count",
                9,
                1,
                9999
            ),
            1
        );
        assert_eq!(
            number_field(
                &json!({ "cooldown_seconds": false }),
                "cooldown_seconds",
                60,
                0,
                86400
            ),
            0
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "2.9" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            2
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "2x" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            60
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "0x10" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            16
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "0b10" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            2
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": "0o10" }),
                "window_seconds",
                60,
                1,
                86400
            ),
            8
        );
        assert_eq!(
            number_field(
                &json!({ "window_seconds": ["4.9"] }),
                "window_seconds",
                60,
                1,
                86400
            ),
            4
        );
    }

    #[test]
    fn notification_provider_payload_helpers_match_node_edges() {
        let message = json!({
            "summary": " 概览 ",
            "body_text": " 第一行 \n 第二行 ",
            "facts": [{ "label": "状态", "value": "异常" }],
            "actions": [{ "label": "查看", "url": "https://example.com/a" }],
        });

        assert_eq!(message_title(&json!({})), "fn-knock 通知");
        assert_eq!(build_markdown_body(&json!({}), ""), "");
        assert!(build_pushplus_markdown_content(&message).contains("- **状态**：异常"));

        let pushplus_html = build_pushplus_html_content(&message);
        assert!(!pushplus_html.contains("<h2>"));
        assert!(pushplus_html.contains("<strong>状态</strong>：异常"));

        let wxpusher_html = build_wxpusher_html_content(&message);
        assert!(wxpusher_html.contains("<h2>概览</h2>"));
        assert!(wxpusher_html.contains("<strong>状态</strong>：异常"));

        assert_eq!(magicpush_facts_object(&message), json!({ "状态": "异常" }));
        assert_eq!(
            build_bark_payload(&message, &json!({ "target_config": { "badge": 0 } })).get("badge"),
            Some(&json!(0))
        );
    }

    #[test]
    fn notification_provider_parsers_follow_node_edges() {
        assert_eq!(value_as_i64(&json!("200 OK")), Some(200));
        assert_eq!(value_as_i64(&json!("0x10")), Some(0));
        assert_eq!(value_as_i64(&json!("  -12x")), Some(-12));

        let topic_value = json!("+1,01,abc");
        let (topic_ids, invalid_topic_ids) = parse_topic_ids(Some(&topic_value));
        assert_eq!(topic_ids, vec![1]);
        assert_eq!(invalid_topic_ids, vec!["+1", "abc"]);

        assert_eq!(
            resolve_pushplus_url("https://push.example.com/BatchSend"),
            "https://push.example.com/BatchSend"
        );
        assert_eq!(
            resolve_magicpush_url("https://push.example.com/API/PUSH/token", "other", "push"),
            "https://push.example.com/API/PUSH/token"
        );
        assert_eq!(
            resolve_magicpush_url("https://push.example.com/API/INBOUND", "a b", "inbound"),
            "https://push.example.com/API/INBOUND/a+b"
        );
    }

    #[test]
    fn notification_page_parser_matches_node_parse_int_edges() {
        assert_eq!(parse_positive_int(None, 1, i64::MAX), 1);
        assert_eq!(parse_positive_int(Some(""), 20, 100), 20);
        assert_eq!(parse_positive_int(Some("2x"), 1, 100), 2);
        assert_eq!(parse_positive_int(Some("  +3.9"), 1, 100), 3);
        assert_eq!(parse_positive_int(Some("-1"), 1, 100), 1);
        assert_eq!(parse_positive_int(Some("0x10"), 7, 100), 7);
        assert_eq!(parse_positive_int(Some("999"), 20, 100), 100);
        assert_eq!(
            parse_positive_int(Some("999999999999999999999999"), 20, 100),
            100
        );
    }

    #[test]
    fn builds_sequential_names() {
        let names = vec!["Webhook 1".to_string(), "Webhook 3".to_string()];
        assert_eq!(build_next_sequential_name("Webhook", &names), "Webhook 2");
        assert_eq!(
            build_next_sequential_name("", &["未命名 1".to_string()]),
            "未命名 2"
        );
    }

    #[test]
    fn provider_catalog_view_localizes_schema_text() {
        let definition = provider_definition("email").unwrap();
        let view = provider_definition_view(&definition, &Translator::new("zh-CN"));
        assert_eq!(view.get("label"), Some(&json!("邮件")));
        assert!(
            view.get("description")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("SMTP"))
        );
        let smtp_host = view
            .get("connection_schema")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|field| field.get("key").and_then(Value::as_str) == Some("smtp_host"))
            .unwrap();
        assert_eq!(smtp_host.get("label"), Some(&json!("SMTP 主机")));
        assert!(
            smtp_host
                .get("description")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("邮件发送服务器"))
        );

        let pushplus = provider_definition_view(
            &provider_definition("pushplus").unwrap(),
            &Translator::new("zh-CN"),
        );
        assert_eq!(pushplus.get("label"), Some(&json!("PushPlus 推送")));
        let token = schema_field(&pushplus, "connection_schema", "token");
        assert_eq!(token.get("label"), Some(&json!("令牌")));

        let dingtalk = provider_definition_view(
            &provider_definition("dingtalk").unwrap(),
            &Translator::new("zh-CN"),
        );
        let webhook_url = schema_field(&dingtalk, "connection_schema", "webhook_url");
        assert_eq!(webhook_url.get("label"), Some(&json!("Webhook 地址")));

        let bark = provider_definition_view(
            &provider_definition("bark").unwrap(),
            &Translator::new("zh-CN"),
        );
        let level = schema_field(&bark, "target_schema", "level");
        assert_eq!(level.get("label"), Some(&json!("通知级别")));
        assert!(
            level
                .get("options")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .any(|option| option == &json!({"label": "时效性通知", "value": "timeSensitive"}))
        );

        let telegram = provider_definition_view(
            &provider_definition("telegram").unwrap(),
            &Translator::new("zh-CN"),
        );
        let chat_id = schema_field(&telegram, "connection_schema", "chat_id");
        assert_eq!(chat_id.get("label"), Some(&json!("聊天 ID")));
    }

    #[test]
    fn provider_default_names_use_localized_label() {
        let definition = provider_definition("email").unwrap();
        let zh = Translator::new("zh-CN");
        let base = provider_definition_label(&definition, &zh);

        assert_eq!(base, "邮件");
        assert_eq!(
            build_next_sequential_name(&base, &["邮件 1".to_string()]),
            "邮件 2"
        );
    }

    #[test]
    fn provider_catalog_view_includes_node_schema_metadata() {
        let translator = Translator::new("zh-CN");

        let email = provider_definition_view(&provider_definition("email").unwrap(), &translator);
        let smtp_host = schema_field(&email, "connection_schema", "smtp_host");
        assert_eq!(
            smtp_host.get("placeholder"),
            Some(&json!("smtp.example.com"))
        );
        let smtp_port = schema_field(&email, "connection_schema", "smtp_port");
        assert_eq!(smtp_port.get("min"), Some(&json!(1)));
        assert_eq!(smtp_port.get("max"), Some(&json!(65535)));

        let wxpusher =
            provider_definition_view(&provider_definition("wxpusher").unwrap(), &translator);
        let default_uids = schema_field(&wxpusher, "connection_schema", "uids");
        assert_eq!(default_uids.get("label"), Some(&json!("默认 UID 列表")));
        assert_eq!(
            default_uids.get("placeholder"),
            Some(&json!("UID_xxx,UID_yyy"))
        );
        let target_verify = schema_field(&wxpusher, "target_schema", "verify_pay_type");
        assert_eq!(
            target_verify.get("default_value"),
            Some(&json!("__inherit__"))
        );

        let wecom = provider_definition_view(&provider_definition("wecom").unwrap(), &translator);
        assert_eq!(
            schema_field(&wecom, "connection_schema", "webhook_url").get("placeholder"),
            Some(&json!(
                "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
            ))
        );
        assert!(
            wecom
                .get("connection_schema")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .all(|field| field.get("key").and_then(Value::as_str) != Some("secret"))
        );
    }

    #[test]
    fn provider_catalog_view_matches_node_capabilities() {
        let translator = Translator::new("zh-CN");

        let magicpush =
            provider_definition_view(&provider_definition("magicpush").unwrap(), &translator);
        assert_eq!(
            magicpush.pointer("/capabilities/supports_markdown"),
            Some(&json!(false))
        );
        assert_eq!(
            magicpush.pointer("/capabilities/supports_actions"),
            Some(&json!(false))
        );

        let bark = provider_definition_view(&provider_definition("bark").unwrap(), &translator);
        assert_eq!(
            bark.pointer("/capabilities/supports_markdown"),
            Some(&json!(false))
        );
        assert_eq!(
            bark.pointer("/capabilities/supports_actions"),
            Some(&json!(true))
        );

        let feishu = provider_definition_view(&provider_definition("feishu").unwrap(), &translator);
        assert_eq!(
            feishu.pointer("/capabilities/supports_markdown"),
            Some(&json!(false))
        );
        assert_eq!(
            feishu.pointer("/capabilities/supports_actions"),
            Some(&json!(true))
        );
        assert_eq!(
            feishu.pointer("/capabilities/max_body_length"),
            Some(&json!(20480))
        );

        let wecom = provider_definition_view(&provider_definition("wecom").unwrap(), &translator);
        assert_eq!(
            wecom.pointer("/capabilities/supports_mentions"),
            Some(&json!(true))
        );
        assert_eq!(
            wecom.pointer("/capabilities/max_body_length"),
            Some(&json!(4096))
        );

        let serverchan =
            provider_definition_view(&provider_definition("serverchan").unwrap(), &translator);
        assert_eq!(
            serverchan.pointer("/capabilities/max_body_length"),
            Some(&json!(32768))
        );

        let telegram =
            provider_definition_view(&provider_definition("telegram").unwrap(), &translator);
        assert_eq!(
            telegram.pointer("/capabilities/max_body_length"),
            Some(&json!(4096))
        );

        let webhook =
            provider_definition_view(&provider_definition("webhook").unwrap(), &translator);
        assert_eq!(
            webhook.pointer("/capabilities/max_body_length"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn localizes_provider_test_builtin_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            parse_json_body(&Bytes::from_static(b"{"), &zh)
                .expect_err("invalid json body should fail"),
            "请求体必须是合法 JSON"
        );
        assert_eq!(
            notification_service_text(&zh, "providerTestName", &[("provider", "Webhook".into())]),
            "Webhook 测试"
        );
        assert_eq!(
            localize_provider_test_message(&zh, "Notification provider test sent successfully"),
            "测试发送成功"
        );
        assert_eq!(
            localize_provider_test_message(&zh, "Webhook request returned status 503"),
            "Webhook 请求返回状态 503"
        );
        assert_eq!(
            localize_provider_test_result(
                ProviderTestResult {
                    success: false,
                    message: "Telegram request returned status 429".to_string(),
                    request_summary: None,
                    response_summary: None,
                },
                &zh,
            )
            .message,
            "Telegram 请求返回状态 429"
        );
        assert_eq!(
            localize_provider_test_message(&zh, "Bark failed for 1/2 target(s)"),
            "Bark 1/2 个目标发送失败"
        );
        assert_eq!(
            localize_provider_test_message(&zh, "Invalid WxPusher topic id(s): abc"),
            "Topic ID 格式不正确：abc"
        );

        let en = Translator::new("en");
        assert_eq!(
            localize_provider_test_message(&en, "缺少 Webhook URL"),
            "Missing Webhook URL"
        );
        assert_eq!(
            localize_provider_test_message(&en, "测试发送成功"),
            "Test send succeeded"
        );
        assert_eq!(
            localize_provider_test_message(&en, "Topic ID 格式不正确：abc"),
            "Invalid Topic ID format: abc"
        );
    }

    #[test]
    fn deleted_provider_snapshot_uses_config_locale() {
        let snapshot = deleted_provider_snapshot(
            "provider-1",
            "2026-01-02T03:04:05Z",
            &Translator::new("zh-CN"),
        );
        assert_eq!(snapshot.get("name"), Some(&json!("已删除提供商")));
    }

    #[test]
    fn localizes_rule_names_and_fallback_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            build_notification_rule_name("FN_EVENT_AUTH_LOGIN_SUCCESS", &zh),
            "登录成功 通知"
        );
        let message = build_notification_message(
            &json!({
                "id": "evt_1",
                "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
                "level": "WARN",
                "source": "GO_REAUTH_PROXY",
                "happened_at": "2026-07-06T00:00:00.000Z",
                "dedupe_key": "auth-login"
            }),
            &json!({
                "id": "rule_1",
                "window_seconds": 60
            }),
            2,
            "global",
            &zh,
        );

        assert_eq!(message.get("title"), Some(&json!("敲门 Knock 登录成功 x2")));
        assert!(
            message
                .get("body_text")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("本次通知已在 60 秒窗口内聚合 2 条相似事件"))
        );
        let facts = message.get("facts").and_then(Value::as_array).unwrap();
        assert!(
            facts
                .iter()
                .any(|fact| fact.get("label") == Some(&json!("事件类型")))
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.get("label") == Some(&json!("风险级别")))
        );
        assert!(!serde_json::to_string(&message).unwrap().contains("Matched"));
    }

    #[test]
    fn localizes_email_address_validation_errors() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            parse_mailboxes("bad-address", "to_addresses", &zh)
                .expect_err("invalid mailbox should fail"),
            "收件人 中包含无效邮箱地址: bad-address"
        );
        assert_eq!(
            build_from_mailbox("bad-address", "", &zh).expect_err("invalid from should fail"),
            "发件邮箱格式不正确"
        );
        assert!(
            build_email_plain_text_body(
                &json!({
                    "body_text": "正文",
                    "severity": "info",
                    "event_id": "evt_1",
                    "occurred_at": "2026-07-06T00:00:00.000Z"
                }),
                &zh
            )
            .contains("发生时间: 2026-07-06T00:00:00.000Z")
        );
    }
}
