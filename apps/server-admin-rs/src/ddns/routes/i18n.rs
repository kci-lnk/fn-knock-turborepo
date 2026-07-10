use super::*;

pub(super) fn ddns_error_response(translator: &Translator, error: anyhow::Error) -> Response {
    if let Some(domain_error) = error.downcast_ref::<DDNSDomainConfigError>() {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_ddns_domain_config_error(translator, domain_error),
        );
    }
    let message = error.to_string();
    let status = if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else if message.contains("Unknown")
        || message.contains("Duplicate")
        || message.contains("Primary")
        || message.contains("interval")
    {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        tracing::warn!(%message, "DDNS route failed");
    }
    response::error(status, localize_ddns_error(translator, &message))
}

pub(super) fn localize_ddns_error(translator: &Translator, message: &str) -> String {
    if message == "DDNS target not found" {
        return ddns_text(translator, "targetNotFound", &[]);
    }
    if message == "Failed to initialize primary DDNS target" {
        return ddns_text(translator, "primaryInitFailed", &[]);
    }
    if message == "Primary DDNS target cannot be deleted" {
        return ddns_text(translator, "primaryDeleteForbidden", &[]);
    }
    if message == "Primary DDNS target cannot be disabled" {
        return ddns_text(translator, "primaryDisableForbidden", &[]);
    }
    if message == "Duplicate DDNS target" {
        return ddns_text(translator, "duplicateTarget", &[]);
    }
    if let Some(provider) = message.strip_prefix("Unknown DDNS provider: ") {
        return ddns_text(
            translator,
            "unknownProvider",
            &[("provider", provider.to_string())],
        );
    }
    message.to_string()
}

pub(super) fn parse_ddns_log_limit(value: Option<&str>) -> usize {
    let raw = value.filter(|value| !value.is_empty()).unwrap_or("200");
    let parsed = parse_node_parse_int(raw).unwrap_or(200);
    parsed.clamp(1, 1000) as usize
}

pub(super) use crate::node_compat::parse_i64_prefix_trim_start as parse_node_parse_int;

pub(super) fn parse_log_entries(lines: Vec<String>) -> Vec<Value> {
    lines
        .into_iter()
        .map(|line| {
            serde_json::from_str::<Value>(&line)
                .unwrap_or_else(|_| json!({ "time": "", "level": "info", "message": line }))
        })
        .collect()
}

pub(super) fn provider_catalog(translator: &Translator) -> Value {
    localize_ddns_provider_catalog(
        Value::Array(vec![
            alidns_catalog_entry(),
            baiducloud_catalog_entry(),
            cloudflare_catalog_entry(),
            dnspod_catalog_entry(),
            duckdns_catalog_entry(),
            dynu_catalog_entry(),
            dynv6_catalog_entry(),
            edgeone_cname_catalog_entry(),
            edgeone_catalog_entry(),
            esa_catalog_entry(),
            godaddy_catalog_entry(),
            huaweicloud_catalog_entry(),
            noip_catalog_entry(),
            porkbun_catalog_entry(),
            tencentcloud_catalog_entry(),
        ]),
        translator,
    )
}

pub(super) fn localize_ddns_provider_catalog(mut catalog: Value, translator: &Translator) -> Value {
    if let Some(providers) = catalog.as_array_mut() {
        for provider in providers {
            apply_ddns_domain_targets_capability(provider);
            localize_ddns_provider(provider, translator);
        }
    }
    catalog
}

pub(super) fn localize_ddns_provider(provider: &mut Value, translator: &Translator) {
    let provider_name = provider
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let provider_key = ddns_provider_i18n_key(&provider_name);
    if let Some(object) = provider.as_object_mut() {
        if let Some(label) = object.get("label").and_then(Value::as_str) {
            object.insert(
                "label".to_string(),
                Value::String(ddns_catalog_text(
                    translator,
                    &format!("providers.{provider_key}.label"),
                    label,
                    &[],
                )),
            );
        }
        if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut) {
            for field in fields {
                localize_ddns_field(field, provider_key, translator);
            }
        }
    }
}

pub(super) fn localize_ddns_field(field: &mut Value, provider_key: &str, translator: &Translator) {
    let field_key = field
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let ttl_seconds = field
        .get("placeholder")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(600)
        .to_string();
    let params = [("seconds", ttl_seconds)];

    let label = field
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    let placeholder = field
        .get("placeholder")
        .and_then(Value::as_str)
        .map(str::to_string);
    let description = field
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            ddns_optional_catalog_text(
                translator,
                provider_key,
                &field_key,
                "description",
                "",
                &params,
            )
        });

    if let Some(object) = field.as_object_mut() {
        if let Some(label) = label {
            object.insert(
                "label".to_string(),
                Value::String(
                    ddns_optional_catalog_text(
                        translator,
                        provider_key,
                        &field_key,
                        "label",
                        &label,
                        &params,
                    )
                    .unwrap_or(label),
                ),
            );
        }
        if let Some(placeholder) = placeholder {
            object.insert(
                "placeholder".to_string(),
                Value::String(
                    ddns_optional_catalog_text(
                        translator,
                        provider_key,
                        &field_key,
                        "placeholder",
                        &placeholder,
                        &params,
                    )
                    .unwrap_or(placeholder),
                ),
            );
        }
        if let Some(description) = description.filter(|value| !value.is_empty()) {
            object.insert("description".to_string(), Value::String(description));
        }
        if let Some(options) = object.get_mut("options").and_then(Value::as_array_mut) {
            for option in options {
                localize_ddns_option(option, provider_key, &field_key, translator);
            }
        }
    }
}

pub(super) fn localize_ddns_option(
    option: &mut Value,
    provider_key: &str,
    field_key: &str,
    translator: &Translator,
) {
    let Some(value) = option.get("value").and_then(Value::as_str) else {
        return;
    };
    let option_key = ddns_option_i18n_key(provider_key, field_key, value);
    let Some(label) = option
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return;
    };
    let field_i18n_key = ddns_field_i18n_key(provider_key, field_key);
    let translated = ddns_catalog_text(
        translator,
        &format!("providers.{provider_key}.fields.{field_i18n_key}.options.{option_key}"),
        &label,
        &[],
    );
    if let Some(object) = option.as_object_mut() {
        object.insert("label".to_string(), Value::String(translated));
    }
}

pub(super) fn ddns_optional_catalog_text(
    translator: &Translator,
    provider_key: &str,
    field_key: &str,
    part: &str,
    fallback: &str,
    params: &[(&str, String)],
) -> Option<String> {
    let field_i18n_key = ddns_field_i18n_key(provider_key, field_key);
    let provider_value = ddns_catalog_text(
        translator,
        &format!("providers.{provider_key}.fields.{field_i18n_key}.{part}"),
        fallback,
        params,
    );
    if provider_value != fallback {
        return Some(provider_value);
    }
    if part == "placeholder"
        && field_key == "record_line"
        && matches!(provider_key, "dnspod" | "tencentcloud")
    {
        let default_line = ddns_catalog_text(
            translator,
            &format!("providers.{provider_key}.defaultLine"),
            fallback,
            params,
        );
        if default_line != fallback {
            return Some(default_line);
        }
    }
    if part == "label" && provider_key == "cloudflare" && field_key == "domain" {
        let short_label = ddns_catalog_text(
            translator,
            "providers.common.fields.domain.shortLabel",
            fallback,
            params,
        );
        if short_label != fallback {
            return Some(short_label);
        }
    }
    if part == "description"
        && field_key == "domain"
        && matches!(provider_key, "alidns" | "tencentcloud" | "esa")
    {
        let host_description = ddns_catalog_text(
            translator,
            "providers.common.fields.domain.hostDescription",
            fallback,
            params,
        );
        if host_description != fallback {
            return Some(host_description);
        }
    }
    let common_value = ddns_catalog_text(
        translator,
        &format!("providers.common.fields.{field_key}.{part}"),
        fallback,
        params,
    );
    if common_value != fallback {
        Some(common_value)
    } else if fallback.is_empty() {
        None
    } else {
        Some(fallback.to_string())
    }
}

pub(super) fn ddns_catalog_text(
    translator: &Translator,
    key: &str,
    fallback: &str,
    params: &[(&str, String)],
) -> String {
    let translated = ddns_text(translator, key, params);
    if translated == format!("server.ddns.{key}") {
        fallback.to_string()
    } else {
        translated
    }
}

pub(super) fn ddns_text(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    let full_key = format!("server.ddns.{key}");

    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

pub(super) fn ddns_provider_i18n_key(provider_name: &str) -> &str {
    match provider_name {
        "baiducloud" => "baidu",
        "huaweicloud" => "huawei",
        value => value,
    }
}

pub(super) fn ddns_field_i18n_key<'a>(provider_key: &str, field_key: &'a str) -> &'a str {
    match (provider_key, field_key) {
        ("edgeone" | "edgeone_cname", DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD) => "overseas_access",
        _ => field_key,
    }
}

pub(super) fn ddns_option_i18n_key(provider_key: &str, field_key: &str, value: &str) -> String {
    match (field_key, value) {
        ("proxied", "false") => "dnsOnly".to_string(),
        ("proxied", "true") if provider_key == "esa" => "enabled".to_string(),
        ("proxied", "true") => "orangeCloud".to_string(),
        ("biz_name", "image_video") => "imageVideo".to_string(),
        ("edgeone_overseas_access", "block_overseas") => "blockOverseas".to_string(),
        _ => value.to_string(),
    }
}

pub(super) fn provider(name: &str, label: &str, fields: Vec<Value>) -> Value {
    json!({ "name": name, "label": label, "fields": fields })
}

pub(super) fn field(
    key: &str,
    label: &str,
    field_type: &str,
    placeholder: &str,
    required: bool,
) -> Value {
    json!({
        "key": key,
        "label": label,
        "type": field_type,
        "placeholder": placeholder,
        "required": required
    })
}

pub(super) fn select_field(
    key: &str,
    label: &str,
    required: bool,
    options: Vec<(&str, &str)>,
) -> Value {
    json!({
        "key": key,
        "label": label,
        "type": "select",
        "required": required,
        "options": options
            .into_iter()
            .map(|(label, value)| json!({ "label": label, "value": value }))
            .collect::<Vec<_>>()
    })
}
