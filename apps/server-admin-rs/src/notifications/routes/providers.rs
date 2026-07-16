use super::*;
mod bark;
mod config;
mod content;
mod dingtalk;
mod email;
mod feishu;
mod harmonyosmeow;
mod http;
mod magicpush;
mod misc;
mod pushdeer;
mod pushplus;
mod serverchan;
mod telegram;
mod webhook;
mod wecom;
mod wxpusher;

pub(super) use bark::*;
pub(super) use config::*;
pub(super) use content::*;
pub(super) use dingtalk::*;
pub(super) use email::*;
pub(super) use feishu::*;
pub(super) use harmonyosmeow::*;
pub(super) use http::*;
pub(super) use magicpush::*;
pub(super) use misc::*;
pub(super) use pushdeer::*;
pub(super) use pushplus::*;
pub(super) use serverchan::*;
pub(super) use telegram::*;
pub(super) use webhook::*;
pub(super) use wecom::*;
pub(super) use wxpusher::*;

#[derive(Clone)]
pub(super) struct ProviderTestResult {
    pub(super) success: bool,
    pub(super) retryable: bool,
    pub(super) message: String,
    pub(super) request_summary: Option<Value>,
    pub(super) response_summary: Option<Value>,
}

pub(super) async fn run_provider_test(
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
            retryable: false,
            message: notification_service_text(translator, "unsupportedProviderType", &[]),
            request_summary: None,
            response_summary: None,
        }),
    }
}

pub(super) fn is_http_notification_provider(provider_type: &str) -> bool {
    matches!(
        provider_type,
        "wxpusher"
            | "serverchan"
            | "pushplus"
            | "wecom"
            | "dingtalk"
            | "feishu"
            | "pushdeer"
            | "harmonyosmeow"
            | "magicpush"
            | "bark"
            | "telegram"
    )
}

pub(super) fn build_provider_test_message(translator: &Translator) -> Value {
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

pub(super) async fn send_http_notification_provider(
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
        "harmonyosmeow" => send_harmonyosmeow(state, provider, message, timeout_seconds).await,
        "magicpush" => send_magicpush(state, provider, message, timeout_seconds).await,
        "bark" => send_bark(state, provider, target, message, timeout_seconds).await,
        "telegram" => send_telegram(state, provider, target, message, timeout_seconds).await,
        _provider_type => ProviderTestResult {
            success: false,
            retryable: false,
            message: notification_service_default_text("unsupportedProviderType", &[]),
            request_summary: None,
            response_summary: None,
        },
    }
}

pub(super) fn empty_to_null(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}
