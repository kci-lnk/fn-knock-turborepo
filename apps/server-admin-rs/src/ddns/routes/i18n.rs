use super::*;

pub(super) fn ddns_error_response(translator: &Translator, error: anyhow::Error) -> Response {
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
        json!([
            provider(
                "alidns",
                "阿里云 DNS",
                vec![
                    field("access_key_id", "AccessKey ID", "text", "LTAI...", true),
                    field(
                        "access_key_secret",
                        "AccessKey Secret",
                        "password",
                        "AccessKey Secret",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("line", "Line", "text", "default", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "baiducloud",
                "百度智能云",
                vec![
                    field("access_key_id", "Access Key", "text", "Access Key", true),
                    field(
                        "secret_access_key",
                        "Secret Key",
                        "password",
                        "Secret Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "300", false),
                ]
            ),
            provider(
                "cloudflare",
                "Cloudflare",
                vec![
                    field(
                        "api_token",
                        "API Token",
                        "password",
                        "Cloudflare API Token",
                        true
                    ),
                    field("zone_id", "Zone ID", "text", "Zone ID", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    select_field(
                        "proxied",
                        "Proxied",
                        false,
                        vec![("DNS only", "false"), ("Orange cloud", "true")]
                    ),
                ]
            ),
            provider(
                "dnspod",
                "DNSPod",
                vec![
                    field("token_id", "Token ID", "text", "DNSPod Token ID", true),
                    field(
                        "token_key",
                        "Token Key",
                        "password",
                        "DNSPod Token Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("record_line", "Record Line", "text", "默认", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "duckdns",
                "DuckDNS",
                vec![
                    field("domains", "Domains", "text", "home,lab", true),
                    field("token", "Token", "password", "DuckDNS Token", true),
                ]
            ),
            provider(
                "dynu",
                "Dynu",
                vec![
                    field("api_key", "API Key", "password", "Dynu API Key", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "120", false),
                    field("group", "Group", "text", "default", false),
                ]
            ),
            provider(
                "dynv6",
                "dynv6",
                vec![
                    field("token", "HTTP Token", "password", "dynv6 HTTP Token", true),
                    field("zone", "Zone", "text", "myhost.dynv6.net", true),
                    field(
                        "ipv6prefix",
                        "IPv6 Prefix",
                        "text",
                        "2001:db8:1234::/64",
                        false
                    ),
                ]
            ),
            edgeone_cname_provider(),
            edgeone_provider(),
            provider(
                "esa",
                "阿里云 ESA",
                vec![
                    field("access_key_id", "AccessKey ID", "text", "LTAI...", true),
                    field(
                        "access_key_secret",
                        "AccessKey Secret",
                        "password",
                        "AccessKey Secret",
                        true
                    ),
                    field("site_name", "Site Name", "text", "example.com", true),
                    field("site_id", "Site ID", "text", "123456", false),
                    field("domain", "Domain", "text", "home.example.com", true),
                    select_field(
                        "proxied",
                        "Proxied",
                        false,
                        vec![("DNS only", "false"), ("Enabled", "true")]
                    ),
                    select_field(
                        "biz_name",
                        "Business",
                        false,
                        vec![
                            ("Web", "web"),
                            ("API", "api"),
                            ("Image/Video", "image_video")
                        ]
                    ),
                    field("ttl", "TTL", "text", "30", false),
                ]
            ),
            provider(
                "godaddy",
                "GoDaddy",
                vec![
                    field("api_key", "API Key", "text", "GoDaddy API Key", true),
                    field(
                        "api_secret",
                        "API Secret",
                        "password",
                        "GoDaddy API Secret",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "huaweicloud",
                "华为云 DNS",
                vec![
                    field("access_key_id", "Access Key", "text", "Access Key", true),
                    field(
                        "secret_access_key",
                        "Secret Key",
                        "password",
                        "Secret Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "300", false),
                ]
            ),
            provider(
                "noip",
                "NO-IP",
                vec![
                    field("hostname", "Hostname", "text", "home.ddns.net", true),
                    field("username", "Username", "text", "DDNS Key Username", true),
                    field(
                        "password",
                        "Password",
                        "password",
                        "DDNS Key Password",
                        true
                    ),
                ]
            ),
            provider(
                "porkbun",
                "Porkbun",
                vec![
                    field("api_key", "API Key", "text", "Porkbun API Key", true),
                    field(
                        "secret_api_key",
                        "Secret API Key",
                        "password",
                        "Porkbun Secret API Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "tencentcloud",
                "腾讯云 DNSPod",
                vec![
                    field("secret_id", "SecretId", "text", "AKID...", true),
                    field("secret_key", "SecretKey", "password", "SecretKey", true),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("record_line", "Record Line", "text", "默认", false),
                    field("record_line_id", "Record Line ID", "text", "0", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
        ]),
        translator,
    )
}

pub(super) fn localize_ddns_provider_catalog(mut catalog: Value, translator: &Translator) -> Value {
    if let Some(providers) = catalog.as_array_mut() {
        for provider in providers {
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
                localize_ddns_field(field, &provider_key, translator);
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
    let translated = if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    };
    translated
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

pub(super) fn edgeone_cname_provider() -> Value {
    let mut value = provider(
        "edgeone_cname",
        "EdgeOne CNAME",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            select_field(
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "Overseas access control",
                false,
                vec![("Off", "off"), ("Block overseas IPs", "block_overseas")],
            ),
            field(
                "endpoint",
                "API Endpoint",
                "text",
                "https://teo.tencentcloudapi.com",
                false,
            ),
            field("region", "Region", "text", "", false),
        ],
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "capabilities".to_string(),
            json!({ "addressMode": "single_address" }),
        );
    }
    value
}

pub(super) fn edgeone_provider() -> Value {
    provider(
        "edgeone",
        "Tencent EdgeOne",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("location", "Location", "text", "", false),
            field("ttl", "TTL", "text", "300", false),
            select_field(
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "Overseas access control",
                false,
                vec![("Off", "off"), ("Block overseas IPs", "block_overseas")],
            ),
            field(
                "endpoint",
                "API Endpoint",
                "text",
                "https://teo.tencentcloudapi.com",
                false,
            ),
            field("region", "Region", "text", "", false),
        ],
    )
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
