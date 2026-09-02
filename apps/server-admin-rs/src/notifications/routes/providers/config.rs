use super::*;

pub(in crate::notifications::routes) fn missing_config_result(message: &str) -> ProviderTestResult {
    ProviderTestResult {
        success: false,
        retryable: false,
        message: message.to_string(),
        request_summary: None,
        response_summary: None,
    }
}

pub(in crate::notifications::routes) fn provider_timeout_seconds(
    provider: &Value,
    fallback: i64,
) -> i64 {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .and_then(|config| config.get("timeout_seconds"))
        .map(|value| value_to_i64(value, fallback))
        .unwrap_or(fallback)
        .clamp(1, 30)
}

pub(in crate::notifications::routes) fn provider_config(provider: &Value) -> Map<String, Value> {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn validate_provider_connection_config(
    definition: &ProviderDefinition,
    config: &Map<String, Value>,
    translator: &Translator,
) -> NotifyResult<()> {
    validate_provider_connection_patch(definition, config, translator)?;
    if definition.provider_type == "harmonyosmeow"
        && !harmonyosmeow_nickname_is_valid(&config_text(config, "nickname"))
    {
        return Err(NotifyError::BadRequest(notification_provider_error_text(
            translator,
            "harmonyosmeow",
            "invalidNickname",
            &[],
        )));
    }
    Ok(())
}

pub(in crate::notifications::routes) fn validate_provider_connection_patch(
    definition: &ProviderDefinition,
    config: &Map<String, Value>,
    translator: &Translator,
) -> NotifyResult<()> {
    if definition.provider_type == "webhook"
        && let Some(headers) = config.get("custom_headers")
    {
        parse_webhook_custom_headers(headers)
            .map_err(|error| NotifyError::BadRequest(error.text(translator)))?;
    }
    Ok(())
}

pub(in crate::notifications::routes) fn target_config(target: &Value) -> Map<String, Value> {
    target
        .get("target_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn config_text(
    config: &Map<String, Value>,
    key: &str,
) -> String {
    config
        .get(key)
        .map(value_to_trimmed_string)
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn first_config_text(
    config: &Map<String, Value>,
    keys: &[&str],
) -> String {
    keys.iter()
        .find_map(|key| {
            let value = config_text(config, key);
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn effective_config_value(
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

pub(in crate::notifications::routes) fn value_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(value) => value.trim().is_empty(),
        _ => false,
    }
}

pub(in crate::notifications::routes) fn split_values(value: Option<&Value>) -> Vec<String> {
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

pub(in crate::notifications::routes) fn parse_topic_ids(
    value: Option<&Value>,
) -> (Vec<i64>, Vec<String>) {
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

pub(in crate::notifications::routes) fn optional_positive_i64(
    value: Option<&Value>,
) -> Option<i64> {
    value
        .map(|value| value_to_i64(value, 0))
        .filter(|value| *value > 0)
}

pub(in crate::notifications::routes) fn optional_nonnegative_i64(
    value: Option<&Value>,
) -> Option<i64> {
    value
        .map(|value| value_to_i64(value, -1))
        .filter(|value| *value >= 0)
}

pub(in crate::notifications::routes) fn message_text(message: &Value, key: &str) -> String {
    message
        .get(key)
        .map(value_to_trimmed_string)
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn message_title(message: &Value) -> String {
    default_string(
        message_text(message, "title").if_empty(message_text(message, "summary")),
        DEFAULT_NOTIFICATION_MESSAGE_TITLE,
    )
}

pub(in crate::notifications::routes) fn message_summary(message: &Value) -> String {
    message_text(message, "summary")
}

pub(in crate::notifications::routes) use crate::text_utils::{EmptyStringExt, default_string};

pub(in crate::notifications::routes) fn non_empty_or(
    first: String,
    second: String,
    third: &str,
) -> String {
    if !first.trim().is_empty() {
        first
    } else if !second.trim().is_empty() {
        second
    } else {
        third.to_string()
    }
}
