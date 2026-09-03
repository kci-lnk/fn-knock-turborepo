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

#[derive(Clone)]
pub(in crate::notifications::routes) struct SchemaField {
    pub(in crate::notifications::routes) key: &'static str,
    pub(in crate::notifications::routes) label: &'static str,
    pub(in crate::notifications::routes) field_type: &'static str,
    pub(in crate::notifications::routes) required: bool,
    pub(in crate::notifications::routes) sensitive: bool,
    pub(in crate::notifications::routes) placeholder: Option<&'static str>,
    pub(in crate::notifications::routes) default_value: Option<Value>,
    pub(in crate::notifications::routes) min: Option<i64>,
    pub(in crate::notifications::routes) max: Option<i64>,
    pub(in crate::notifications::routes) options: Vec<(&'static str, &'static str)>,
    pub(in crate::notifications::routes) constraints: Option<Value>,
}

impl SchemaField {
    pub(in crate::notifications::routes) fn placeholder(mut self, value: &'static str) -> Self {
        if !value.is_empty() {
            self.placeholder = Some(value);
        }
        self
    }

    pub(in crate::notifications::routes) fn bounds(mut self, min: i64, max: i64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    pub(in crate::notifications::routes) fn min(mut self, min: i64) -> Self {
        self.min = Some(min);
        self
    }
}

#[derive(Clone)]
pub(in crate::notifications::routes) struct ProviderDefinition {
    pub(in crate::notifications::routes) provider_type: &'static str,
    pub(in crate::notifications::routes) label: &'static str,
    pub(in crate::notifications::routes) description: &'static str,
    pub(in crate::notifications::routes) connection_schema: Vec<SchemaField>,
    pub(in crate::notifications::routes) target_schema: Vec<SchemaField>,
    pub(in crate::notifications::routes) sensitive_fields: Vec<&'static str>,
    pub(in crate::notifications::routes) supports_markdown: bool,
    pub(in crate::notifications::routes) supports_actions: bool,
    pub(in crate::notifications::routes) supports_mentions: bool,
    pub(in crate::notifications::routes) supports_provider_dedupe_key: bool,
}

pub(super) fn provider_definition(provider_type: &str) -> Option<ProviderDefinition> {
    match provider_type {
        "webhook" => Some(webhook_definition()),
        "wxpusher" => Some(wxpusher_definition()),
        "serverchan" => Some(serverchan_definition()),
        "pushplus" => Some(pushplus_definition()),
        "wecom" => Some(wecom_definition()),
        "dingtalk" => Some(dingtalk_definition()),
        "feishu" => Some(feishu_definition()),
        "email" => Some(email_definition()),
        "pushdeer" => Some(pushdeer_definition()),
        "harmonyosmeow" => Some(harmonyosmeow_definition()),
        "magicpush" => Some(magicpush_definition()),
        "bark" => Some(bark_definition()),
        "telegram" => Some(telegram_definition()),
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
        constraints: None,
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
        constraints: None,
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
        constraints: None,
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
        constraints: None,
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
            if let Some(constraints) = &field.constraints {
                value.insert("constraints".to_string(), constraints.clone());
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
            "headers" => normalize_webhook_custom_headers(input)?,
            "webhook_body" => normalize_webhook_body_config(
                input,
                if field.key == "body_override" {
                    WebhookBodyScope::Target
                } else {
                    WebhookBodyScope::Provider
                },
            )?,
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
