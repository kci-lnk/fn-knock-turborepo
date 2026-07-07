use super::*;

pub(in crate::notifications::routes) async fn post_json(
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

pub(in crate::notifications::routes) async fn send_prepared_json(
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

pub(in crate::notifications::routes) async fn post_form(
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

pub(in crate::notifications::routes) async fn read_provider_response(
    response: reqwest::Response,
) -> (u16, bool, String, Option<Value>) {
    let status = response.status();
    let ok = status.is_success();
    let text = response.text().await.unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&text).ok();
    (status.as_u16(), ok, text, parsed)
}

pub(in crate::notifications::routes) fn provider_result_from_api<S, M>(
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
    let retryable =
        !success && provider_api_failure_retryable(provider_label, status, parsed_value);
    let has_response_summary = !(status == 599 && parsed.is_none());
    ProviderTestResult {
        success,
        retryable,
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
        response_summary: has_response_summary.then(|| {
            json!({
                "status": status,
                "ok": ok,
                "body_preview": truncate_text(&text, 500),
                "json": parsed.unwrap_or(Value::Null)
            })
        }),
    }
}

pub(in crate::notifications::routes) fn provider_api_failure_retryable(
    provider_label: &str,
    status: u16,
    parsed: &Value,
) -> bool {
    status >= 500
        || status == 429
        || match provider_label {
            "Feishu" => json_i64(parsed, "code") == Some(11232),
            "PushPlus" => matches!(json_i64(parsed, "code"), Some(500 | 999)),
            "Telegram" => json_i64(parsed, "error_code") == Some(429),
            _ => false,
        }
}
