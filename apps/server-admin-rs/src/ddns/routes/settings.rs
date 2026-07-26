use super::*;

pub(super) fn parse_settings(raw: Option<&str>) -> Value {
    let parsed = raw.and_then(|value| serde_json::from_str::<Value>(value).ok());
    let is_record = parsed.as_ref().is_some_and(Value::is_object);
    let default_sources = default_public_check_sources();
    let public_sources = parsed
        .as_ref()
        .and_then(|value| value.get("publicCheckSources"))
        .map(normalize_public_check_sources)
        .unwrap_or_else(default_public_check_sources);
    json!({
        "updateIntervalMinutes": if is_record {
            parsed
                .as_ref()
                .and_then(|value| value.get("updateIntervalMinutes"))
                .and_then(normalize_update_interval_minutes)
                .unwrap_or(10)
        } else {
            default_update_interval_minutes()
        },
        "publicCheckSources": public_sources,
        "defaultPublicCheckSources": default_sources,
        "httpTransport": normalize_http_transport(parsed.as_ref().and_then(|value| value.get("httpTransport"))),
        "publicDnsProvider": normalize_public_dns_provider(
            parsed
                .as_ref()
                .and_then(|value| value.get("publicDnsProvider"))
                .and_then(Value::as_str),
        )
    })
}

pub(super) fn normalize_public_check_sources(value: &Value) -> Value {
    let fallback = default_public_check_sources();
    json!({
        "ipv4": normalize_public_check_source_list(
            value.get("ipv4"),
            fallback.get("ipv4").and_then(Value::as_array).cloned().unwrap_or_default(),
        ),
        "ipv6": normalize_public_check_source_list(
            value.get("ipv6"),
            fallback.get("ipv6").and_then(Value::as_array).cloned().unwrap_or_default(),
        )
    })
}

pub(super) fn normalize_public_check_sources_strict(
    value: &Value,
    fallback: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    if !value.is_object() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", "IPv4/IPv6".to_string()),
                ("source", public_check_source_value_string(value)),
            ],
        ));
    }
    Ok(json!({
        "ipv4": normalize_public_check_source_list_strict(
            value.get("ipv4"),
            "ipv4",
            fallback
                .get("ipv4")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            translator,
        )?,
        "ipv6": normalize_public_check_source_list_strict(
            value.get("ipv6"),
            "ipv6",
            fallback
                .get("ipv6")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            translator,
        )?
    }))
}

pub(super) fn normalize_public_check_source_list(
    value: Option<&Value>,
    fallback: Vec<Value>,
) -> Vec<Value> {
    let Some(items) = value.and_then(Value::as_array) else {
        return fallback;
    };
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for item in items.iter().filter_map(Value::as_str) {
        let source = item.trim();
        if source.is_empty() {
            continue;
        }
        let candidate = if has_explicit_scheme(source) {
            source.to_string()
        } else {
            format!("https://{source}")
        };
        if !candidate.starts_with("http://") && !candidate.starts_with("https://") {
            continue;
        }
        if seen.insert(candidate.clone()) {
            normalized.push(Value::String(candidate));
        }
    }
    if normalized.is_empty() {
        fallback
    } else {
        normalized
    }
}

pub(super) fn normalize_public_check_source_list_strict(
    value: Option<&Value>,
    family: &str,
    fallback: Vec<Value>,
    translator: &Translator,
) -> Result<Vec<Value>, String> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let Some(items) = value.as_array() else {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", public_check_family_label(family).to_string()),
                ("source", public_check_source_value_string(value)),
            ],
        ));
    };
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for item in items {
        let source = normalize_public_check_source_strict(item, family, translator)?;
        if seen.insert(source.clone()) {
            normalized.push(Value::String(source));
        }
    }
    Ok(normalized)
}

pub(super) fn normalize_public_check_source_strict(
    value: &Value,
    family: &str,
    translator: &Translator,
) -> Result<String, String> {
    let source = public_check_source_value_string(value);
    let family_label = public_check_family_label(family);
    if source.is_empty() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceEmpty",
            &[("family", family_label.to_string())],
        ));
    }

    let candidate = build_public_check_candidate_url(&source, family_label, translator)?;
    let parsed = Url::parse(&candidate).map_err(|_| {
        ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", family_label.to_string()),
                ("source", source.clone()),
            ],
        )
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(ddns_text(
            translator,
            "publicCheckSourceUnsupportedProtocol",
            &[("family", family_label.to_string()), ("source", source)],
        ));
    }
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[("family", family_label.to_string()), ("source", source)],
        ));
    }
    Ok(candidate)
}

pub(super) fn build_public_check_candidate_url(
    source: &str,
    family_label: &str,
    translator: &Translator,
) -> Result<String, String> {
    let Some(scheme) = explicit_url_scheme(source) else {
        return Ok(format!("https://{source}"));
    };
    if scheme != "http" && scheme != "https" {
        return Err(ddns_text(
            translator,
            "publicCheckSourceUnsupportedProtocol",
            &[
                ("family", family_label.to_string()),
                ("source", source.to_string()),
            ],
        ));
    }
    let lower = source.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", family_label.to_string()),
                ("source", source.to_string()),
            ],
        ));
    }
    Ok(source.to_string())
}

pub(super) fn explicit_url_scheme(source: &str) -> Option<String> {
    let (scheme, _) = source.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-')) {
        Some(scheme.to_ascii_lowercase())
    } else {
        None
    }
}

pub(super) fn public_check_family_label(family: &str) -> &'static str {
    if family == "ipv4" { "IPv4" } else { "IPv6" }
}

pub(super) fn public_check_source_value_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        _ => value.to_string(),
    }
}

pub(super) fn default_public_check_sources() -> Value {
    json!({
        "ipv4": DEFAULT_PUBLIC_CHECK_IPV4,
        "ipv6": DEFAULT_PUBLIC_CHECK_IPV6
    })
}

pub(super) fn normalize_update_interval_minutes(value: &Value) -> Option<i64> {
    let parsed = match value {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => js_number_from_string_like_node(value)?,
        _ => return None,
    };
    if !parsed.is_finite() || parsed.fract() != 0.0 {
        return None;
    }
    let parsed = parsed as i64;
    (5..=1440).contains(&parsed).then_some(parsed)
}

fn js_number_from_string_like_node(value: &str) -> Option<f64> {
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
    if let Some(value) = radix_value {
        return Some(value);
    }

    trimmed.parse::<f64>().ok()
}

pub(super) fn default_update_interval_minutes() -> i64 {
    parse_legacy_ddns_cron_interval_minutes(env::var("DDNS_CRON").ok().as_deref()).unwrap_or(10)
}

pub(super) fn parse_legacy_ddns_cron_interval_minutes(pattern: Option<&str>) -> Option<i64> {
    let parts = pattern?
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 5 && parts.len() != 6 {
        return None;
    }
    if parts.len() == 6 && parts[0] != "0" {
        return None;
    }
    let minute_part = if parts.len() == 6 { parts[1] } else { parts[0] };
    let other_parts = if parts.len() == 6 {
        &parts[2..]
    } else {
        &parts[1..]
    };
    if !other_parts.iter().all(|part| *part == "*") {
        return None;
    }
    let interval = minute_part.strip_prefix("*/")?;
    if interval.is_empty() || !interval.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let minutes = interval.parse::<i64>().ok()?;
    (5..=1440).contains(&minutes).then_some(minutes)
}

pub(super) fn normalize_http_transport(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("node" | "fetch") => "node",
        Some("curl") => "curl",
        _ => "node",
    }
}

pub(super) fn merge_http_transport_update(input: Option<&str>, current: &Value) -> &'static str {
    match input {
        Some("node" | "fetch") => "node",
        Some("curl") => "curl",
        None => normalize_http_transport(current.get("httpTransport")),
        Some(_) => "node",
    }
}

pub(super) const DEFAULT_PUBLIC_DNS_PROVIDER: &str = "alidns";

pub(super) fn normalize_public_dns_provider(value: Option<&str>) -> &'static str {
    match value {
        Some("none") => "none",
        Some("tencent") => "tencent",
        Some("cloudflare") => "cloudflare",
        Some("google") => "google",
        Some("alidns") | None => DEFAULT_PUBLIC_DNS_PROVIDER,
        Some(_) => DEFAULT_PUBLIC_DNS_PROVIDER,
    }
}

pub(super) fn merge_public_dns_provider_update(
    input: Option<&str>,
    current: &Value,
) -> &'static str {
    match input {
        Some(value) => normalize_public_dns_provider(Some(value)),
        None => {
            normalize_public_dns_provider(current.get("publicDnsProvider").and_then(Value::as_str))
        }
    }
}

pub(super) fn normalize_update_scope(value: Option<&str>) -> &'static str {
    match value {
        Some("ipv6_only") => "ipv6_only",
        Some("ipv4_only") => "ipv4_only",
        _ => "dual_stack",
    }
}

pub(super) fn normalize_ip_source(value: Option<&str>) -> &'static str {
    match value {
        Some("interface") => "interface",
        Some("static") => "static",
        Some("domain") => "domain",
        _ => "public",
    }
}

pub(super) fn normalize_network_interface(value: Option<&str>) -> String {
    value.unwrap_or("").trim().to_string()
}

pub(super) fn parse_last_ip(data: &HashMap<String, String>) -> Value {
    json!({
        "ipv4": non_empty_string(data.get("ipv4")),
        "ipv6": non_empty_string(data.get("ipv6")),
        "updated_at": non_empty_string(data.get("updated_at"))
    })
}

pub(super) fn parse_last_check(data: &HashMap<String, String>) -> Value {
    json!({
        "checked_at": non_empty_string(data.get("checked_at")),
        "outcome": normalize_last_check_outcome(data.get("outcome").map(String::as_str)),
        "message": non_empty_string(data.get("message"))
    })
}

pub(super) fn empty_last_ip() -> Value {
    json!({ "ipv4": null, "ipv6": null, "updated_at": null })
}

pub(super) fn empty_last_check() -> Value {
    json!({ "checked_at": null, "outcome": null, "message": null })
}

pub(super) fn normalize_last_check_outcome(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("updated") => Some("updated"),
        Some("noop") => Some("noop"),
        Some("skipped") => Some("skipped"),
        Some("error") => Some("error"),
        _ => None,
    }
}

pub(super) fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn domain_summary(
    provider: Option<&str>,
    config: &HashMap<String, String>,
    translator: &Translator,
) -> String {
    if let Some(value) = domain_summary_candidate(config) {
        return value;
    }
    if provider.and_then(normalize_provider_name).is_some() {
        String::new()
    } else {
        ddns_text(translator, "noProviderSelected", &[])
    }
}

pub(super) fn domain_summary_candidate(config: &HashMap<String, String>) -> Option<String> {
    for key in [
        "domain",
        "hostname",
        "domains",
        "zone",
        "root_domain",
        "site_name",
        "site_id",
    ] {
        if let Some(value) = config
            .get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Some(value);
        }
    }
    None
}

pub(super) fn provider_label(provider: Option<&str>, translator: &Translator) -> String {
    let Some(provider) = provider.and_then(normalize_provider_name) else {
        return ddns_text(translator, "notConfigured", &[]);
    };
    let fallback = provider_label_fallback(&provider);
    ddns_catalog_text(
        translator,
        &format!("providers.{}.label", ddns_provider_i18n_key(&provider)),
        &fallback,
        &[],
    )
}

pub(super) fn provider_label_fallback(provider: &str) -> String {
    match provider {
        "alidns" => "阿里云 DNS".to_string(),
        "baiducloud" => "百度智能云".to_string(),
        "cloudflare" => "Cloudflare".to_string(),
        "dnspod" => "DNSPod".to_string(),
        "duckdns" => "DuckDNS".to_string(),
        "dynu" => "Dynu".to_string(),
        "dynv6" => "dynv6".to_string(),
        "edgeone_cname" => "EdgeOne CNAME".to_string(),
        "edgeone" => "Tencent EdgeOne".to_string(),
        "esa" => "阿里云 ESA".to_string(),
        "godaddy" => "GoDaddy".to_string(),
        "huaweicloud" => "华为云 DNS".to_string(),
        "noip" => "NO-IP".to_string(),
        "porkbun" => "Porkbun".to_string(),
        "tencentcloud" => "腾讯云 DNSPod".to_string(),
        _ => provider.to_string(),
    }
}

pub(super) fn normalize_provider_name(value: &str) -> Option<String> {
    let normalized = value.trim();
    if provider_names().contains(normalized) {
        Some(normalized.to_string())
    } else {
        None
    }
}

pub(super) fn provider_names() -> BTreeSet<&'static str> {
    [
        "alidns",
        "baiducloud",
        "cloudflare",
        "dnspod",
        "duckdns",
        "dynu",
        "dynv6",
        "edgeone_cname",
        "edgeone",
        "esa",
        "godaddy",
        "huaweicloud",
        "noip",
        "porkbun",
        "tencentcloud",
    ]
    .into_iter()
    .collect()
}

pub(super) fn has_explicit_scheme(value: &str) -> bool {
    value.find(':').is_some_and(|index| {
        value[..index]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
    })
}

pub(super) fn target_meta_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:meta")
}

pub(super) fn target_config_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:config")
}

pub(super) fn target_last_ip_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:last_ip")
}

pub(super) fn target_selection_anchor_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:selection_anchor")
}

pub(super) fn target_interface_recovery_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:interface_recovery")
}

pub(super) fn target_last_check_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:last_check")
}
