use super::*;

const HARMONYOS_MEOW_DEFAULT_SERVER_URL: &str = "https://api.chuckfang.com";

pub(in crate::notifications::routes) fn harmonyosmeow_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "harmonyosmeow",
        label: "HarmonyOSMeoW",
        description: "Send Markdown notifications through HarmonyOSMeoW.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some(HARMONYOS_MEOW_DEFAULT_SERVER_URL),
            )
            .placeholder(HARMONYOS_MEOW_DEFAULT_SERVER_URL),
            string_schema("nickname", "Nickname", true, true, None).placeholder("JohnDoe"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: Vec::new(),
        sensitive_fields: vec!["nickname"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) fn resolve_harmonyosmeow_url(
    base_url: &str,
    nickname: &str,
    title: &str,
) -> Result<String, ()> {
    if !harmonyosmeow_nickname_is_valid(nickname) {
        return Err(());
    }
    let mut url = url::Url::parse(base_url.trim()).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return Err(());
    }
    url.set_query(None);
    url.set_fragment(None);
    {
        let mut segments = url.path_segments_mut().map_err(|_| ())?;
        segments.pop_if_empty();
        segments.push(nickname);
        segments.push(title);
    }
    url.query_pairs_mut().append_pair("msgType", "markdown");
    Ok(url.to_string())
}

pub(in crate::notifications::routes) fn harmonyosmeow_nickname_is_valid(nickname: &str) -> bool {
    !nickname.contains('/')
}

pub(in crate::notifications::routes) fn build_harmonyosmeow_body(message: &Value) -> String {
    default_string(build_markdown_body(message, ""), &message_title(message))
}

pub(in crate::notifications::routes) fn harmonyosmeow_result(
    request_summary: Value,
    status: u16,
    ok: bool,
    text: String,
    parsed: Option<Value>,
) -> ProviderTestResult {
    provider_result_from_api(
        "HarmonyOSMeoW",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "status") == Some(200),
        |value| json_text_any(value, &["message", "msg", "error"]),
    )
}

pub(in crate::notifications::routes) async fn send_harmonyosmeow(
    state: &AppState,
    provider: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let nickname = config_text(&config, "nickname");
    if nickname.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "harmonyosmeow",
            "missingNickname",
            &[],
        ));
    }
    if !harmonyosmeow_nickname_is_valid(&nickname) {
        return missing_config_result(&notification_provider_error_default(
            "harmonyosmeow",
            "invalidNickname",
            &[],
        ));
    }
    let base_url = default_string(
        config_text(&config, "server_url"),
        HARMONYOS_MEOW_DEFAULT_SERVER_URL,
    );
    let title = message_title(message);
    let body = build_harmonyosmeow_body(message);
    let Ok(url) = resolve_harmonyosmeow_url(&base_url, &nickname, &title) else {
        return missing_config_result(&notification_provider_error_default(
            "harmonyosmeow",
            "invalidServerUrl",
            &[],
        ));
    };
    let request_summary = json!({
        "method": "POST",
        "endpoint": base_url,
        "msg_type": "markdown",
        "title_preview": truncate_text(&title, 200),
        "body_preview": truncate_text(&body, 500)
    });
    let (status, ok, text, parsed) = post_text(state, &url, &body, timeout_seconds).await;
    harmonyosmeow_result(request_summary, status, ok, text, parsed)
}
