use super::*;

pub(in crate::notifications::routes) fn serverchan_definition() -> ProviderDefinition {
    ProviderDefinition {
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
    }
}

pub(in crate::notifications::routes) async fn send_serverchan(
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
