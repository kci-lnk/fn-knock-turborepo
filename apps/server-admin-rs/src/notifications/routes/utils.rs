use super::*;

pub(super) fn object_field(value: &Value, key: &str) -> Map<String, Value> {
    value
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn trimmed_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn value_to_trimmed_string(value: &Value) -> String {
    js_string_like_node(value).trim().to_string()
}

pub(super) fn value_to_i64(value: &Value, fallback: i64) -> i64 {
    js_number_like_node(value)
        .map(|value| value.floor() as i64)
        .unwrap_or(fallback)
}

pub(super) fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

pub(super) fn js_number_like_node(value: &Value) -> Option<f64> {
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

pub(super) fn js_number_from_string_like_node(value: &str) -> Option<f64> {
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

pub(super) fn parse_int_prefix_like_node(value: &str, radix: u32) -> Option<i64> {
    if radix == 10 {
        return crate::node_compat::parse_i64_prefix_trim_start(value);
    }

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
        if ch.is_digit(radix) {
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

pub(super) fn js_string_like_node(value: &Value) -> String {
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

pub(super) fn bool_field(value: &Value, key: &str, fallback: bool) -> bool {
    value.get(key).map(value_to_bool).unwrap_or(fallback)
}

pub(super) fn number_field(value: &Value, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .get(key)
        .map(|value| value_to_i64(value, fallback))
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn unique_string_array(value: Option<&Value>) -> Vec<String> {
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

pub(super) fn parse_positive_int(value: Option<&str>, fallback: i64, max: i64) -> i64 {
    value
        .and_then(|value| parse_i64_prefix_like_node(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
        .min(max)
}

pub(super) fn parse_i64_prefix_like_node(value: &str) -> Option<i64> {
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
        if magnitude > (i64::MAX as u128) {
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

pub(super) fn matches_optional_string(value: &Value, key: &str, expected: Option<&str>) -> bool {
    let Some(expected) = expected.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    value.get(key).and_then(Value::as_str) == Some(expected)
}

pub(super) fn parse_json_body(body: &Bytes, translator: &Translator) -> Result<Value, String> {
    if body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(body)
        .map_err(|_| notification_service_text(translator, "invalidJsonBody", &[]))
}

pub(super) fn iso_score_ms(value: Option<&str>) -> i64 {
    value
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or_else(time_utils::now_ms)
}

pub(super) fn build_next_sequential_name(base: &str, existing_names: &[String]) -> String {
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

pub(super) fn build_notification_rule_name(event_type: &str, translator: &Translator) -> String {
    let event = format_notification_event_label(event_type, translator);
    notification_template_text(translator, "ruleName", &[("event", event)])
}

pub(super) fn build_notification_title(
    event: &Value,
    matched_count: i64,
    translator: &Translator,
) -> String {
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

pub(super) fn format_notification_event_label(event_type: &str, translator: &Translator) -> String {
    let Some(key) = notification_event_label_key(event_type) else {
        return event_type.to_string();
    };
    notification_template_text(translator, key, &[])
}

pub(super) fn notification_event_label_key(event_type: &str) -> Option<&'static str> {
    Some(match event_type {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => "events.authLoginSuccess",
        "FN_EVENT_AUTH_LOGOUT" => "events.authLogout",
        "FN_EVENT_AUTH_LOGIN_FAILURE" => "events.authLoginFailure",
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => "events.authSessionIpDrift",
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => "events.securityScannerBlocked",
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => "events.ddnsUpdateCompleted",
        "FN_EVENT_WOL_WAKE_COMPLETED" => "events.wolWakeCompleted",
        "FN_EVENT_WOL_SHUTDOWN_COMPLETED" => "events.wolShutdownCompleted",
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => "events.gatewayThrottleBlocked",
        "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED" => "events.gatewayVisibilityBlocked",
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
        "FN_EVENT_RUNTIME_STARTED" => "events.runtimeStarted",
        "FN_EVENT_RUNTIME_STOPPED" => "events.runtimeStopped",
        "FN_EVENT_RUNTIME_RESTARTED" => "events.runtimeRestarted",
        "FN_EVENT_RUNTIME_HEALTH_FAILED" => "events.runtimeHealthFailed",
        "FN_EVENT_RUNTIME_RECOVERED" => "events.runtimeRecovered",
        "FN_EVENT_RUNTIME_ABNORMAL_EXIT" => "events.runtimeAbnormalExit",
        _ => return None,
    })
}

pub(super) fn format_notification_level_label(level: &str, translator: &Translator) -> String {
    let key = match level {
        "WARN" => "levels.warn",
        "ERROR" => "levels.error",
        "CRITICAL" => "levels.critical",
        _ => "levels.info",
    };
    notification_template_text(translator, key, &[])
}

pub(super) fn format_notification_source_label(source: &str, translator: &Translator) -> String {
    let key = match source {
        "GO_REAUTH_PROXY" => "sources.goReauthProxy",
        "SYSTEM_MONITOR" => "sources.systemMonitor",
        "RUNTIME_MONITOR" => "sources.runtimeMonitor",
        _ => "sources.serverAdmin",
    };
    notification_template_text(translator, key, &[])
}

pub(super) fn notification_template_text(
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

pub(super) fn notification_detail_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    notification_template_text(translator, &format!("details.{key}"), params)
}

pub(super) fn notification_fact_label(translator: &Translator, key: &str) -> String {
    notification_detail_text(translator, &format!("facts.{key}"), &[])
}

pub(super) fn create_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

pub(super) fn create_stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(parts.join("\u{0}"));
    let digest = hex::encode(hasher.finalize());
    format!("{prefix}_{}", &digest[..24])
}

pub(super) fn create_runtime_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}_{}",
        std::process::id(),
        time_utils::now_ms(),
        hex::encode(rand::random::<[u8; 6]>())
    )
}

pub(super) fn truncate_text(value: &str, max_len: usize) -> String {
    let mut result = value.chars().take(max_len).collect::<String>();
    if value.chars().count() > max_len {
        result.push('…');
    }
    result
}

pub(super) async fn internal_error(
    state: &AppState,
    context: &str,
    error: crate::storage::StorageError,
) -> Response {
    let translator = Translator::from_state(state).await;
    tracing::warn!(%error, "{context}");
    response::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        notification_service_text(&translator, "storageUnavailable", &[]),
    )
}
