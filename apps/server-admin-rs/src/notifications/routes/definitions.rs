use super::*;

#[derive(Debug)]
pub(super) enum NotifyError {
    BadRequest(String),
    Storage(crate::storage::StorageError),
}

pub(super) type NotifyResult<T> = Result<T, NotifyError>;

impl From<crate::storage::StorageError> for NotifyError {
    fn from(value: crate::storage::StorageError) -> Self {
        Self::Storage(value)
    }
}

pub(super) trait OptionBadRequest<T> {
    fn ok_or_bad<S: Into<String>>(self, message: S) -> NotifyResult<T>;
}

impl<T> OptionBadRequest<T> for Option<T> {
    fn ok_or_bad<S: Into<String>>(self, message: S) -> NotifyResult<T> {
        let message = message.into();
        self.ok_or_else(|| NotifyError::BadRequest(message))
    }
}

pub(super) fn provider_key(id: &str) -> String {
    format!("{PROVIDERS_DATA_PREFIX}{id}")
}

pub(super) fn rule_key(id: &str) -> String {
    format!("{RULES_DATA_PREFIX}{id}")
}

pub(super) fn delivery_key(id: &str) -> String {
    format!("{DELIVERIES_DATA_PREFIX}{id}")
}

pub(super) fn provider_definition(provider_type: &str) -> Option<ProviderDefinition> {
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

pub(super) fn webhook_like_definition(
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

pub(super) fn string_schema(
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

pub(super) fn number_schema(
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

pub(super) fn bool_schema(
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

pub(super) fn select_schema(
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

pub(super) fn json_schema(key: &'static str, label: &'static str, required: bool) -> SchemaField {
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

pub(super) fn provider_definition_view(
    definition: &ProviderDefinition,
    translator: &Translator,
) -> Value {
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

pub(super) fn provider_max_body_length(provider_type: &str) -> Value {
    match provider_type {
        "serverchan" => json!(32768),
        "wecom" => json!(4096),
        "feishu" => json!(20480),
        "telegram" => json!(4096),
        _ => Value::Null,
    }
}

pub(super) fn provider_definition_label(
    definition: &ProviderDefinition,
    translator: &Translator,
) -> String {
    translator.t_with_fallback(
        &format!(
            "server.notifications.providers.catalog.{}.label",
            definition.provider_type
        ),
        definition.label,
    )
}

pub(super) fn notification_service_text(
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

pub(super) fn notification_service_default_text(key: &str, params: &[(&str, String)]) -> String {
    notification_service_text(&Translator::new(crate::i18n::DEFAULT_LOCALE), key, params)
}

pub(super) fn notification_route_text(
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

pub(super) fn notification_provider_error_default(
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

pub(super) fn notification_provider_error_text(
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

pub(super) fn notification_provider_field_text(
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

pub(super) fn notification_email_message_text(
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

pub(super) fn localize_provider_test_result(
    mut result: ProviderTestResult,
    translator: &Translator,
) -> ProviderTestResult {
    result.message = localize_provider_test_message(translator, &result.message);
    result
}

pub(super) fn localize_provider_test_message(translator: &Translator, message: &str) -> String {
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

pub(super) fn localize_notification_service_message(
    translator: &Translator,
    message: &str,
) -> Option<String> {
    for &key in NOTIFICATION_TEST_SERVICE_KEYS {
        for locale in NOTIFICATION_MESSAGE_LOCALES {
            if notification_service_text(&Translator::new(locale), key, &[]) == message {
                return Some(notification_service_text(translator, key, &[]));
            }
        }
    }
    None
}

pub(super) fn localize_notification_provider_error(
    translator: &Translator,
    message: &str,
) -> Option<String> {
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

pub(super) fn localize_wxpusher_invalid_topic_ids(
    translator: &Translator,
    message: &str,
) -> Option<String> {
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

pub(super) fn localize_notification_request_status(
    translator: &Translator,
    message: &str,
) -> Option<String> {
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

pub(super) fn localize_bark_partial_failure(
    translator: &Translator,
    message: &str,
) -> Option<String> {
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

pub(super) fn schema_view(
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

pub(super) fn localize_notification_schema_part(
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

pub(super) fn optional_notification_schema_part(
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

pub(super) fn normalize_schema_config(
    raw: &Map<String, Value>,
    fields: &[SchemaField],
) -> NotifyResult<Map<String, Value>> {
    let mut normalized = normalize_schema_patch(raw, fields)?;
    apply_schema_defaults(&mut normalized, fields);
    Ok(normalized)
}

pub(super) fn normalize_schema_patch(
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

pub(super) fn apply_schema_defaults(config: &mut Map<String, Value>, fields: &[SchemaField]) {
    for field in fields {
        if config.contains_key(field.key) {
            continue;
        }
        if let Some(default_value) = &field.default_value {
            config.insert(field.key.to_string(), default_value.clone());
        }
    }
}

pub(super) fn validate_required_fields(
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

pub(super) fn normalize_json_field(value: &Value, label: &str) -> NotifyResult<Value> {
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

pub(super) fn normalize_provider_connection_aliases(
    provider_type: &str,
    raw: &mut Map<String, Value>,
) {
    if provider_type != "wxpusher" {
        return;
    }
    copy_alias(raw, "appToken", "app_token");
    copy_alias(raw, "serverUrl", "server_url");
    copy_alias(raw, "timeoutSeconds", "timeout_seconds");
}

pub(super) fn normalize_provider_target_aliases(provider_type: &str, raw: &mut Map<String, Value>) {
    if provider_type != "wxpusher" {
        return;
    }
    for alias in ["topicIds", "topic_id", "topicId", "topic", "Topic"] {
        copy_alias(raw, alias, "topic_ids");
    }
    copy_alias(raw, "verifyPayType", "verify_pay_type");
}

pub(super) fn copy_alias(raw: &mut Map<String, Value>, alias: &str, canonical: &str) {
    if raw.contains_key(canonical) {
        return;
    }
    if let Some(value) = raw.get(alias).cloned() {
        raw.insert(canonical.to_string(), value);
    }
}

pub(super) fn drop_masked_sensitive_patch_values(
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

pub(super) fn mask_provider(provider: &Value) -> Result<Value, String> {
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

pub(super) fn reveal_provider(provider: &Value) -> Result<Value, String> {
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

pub(super) fn mask_sensitive_value(value: &Value) -> Value {
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
