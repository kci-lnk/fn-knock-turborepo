use super::*;

pub(super) fn normalize_config(
    provider: &str,
    config: HashMap<String, String>,
) -> HashMap<String, String> {
    normalize_config_map(Some(provider), &config)
}

pub(super) fn normalize_and_validate_config(
    provider: &str,
    config: HashMap<String, String>,
) -> anyhow::Result<HashMap<String, String>> {
    validate_interface_selector_config(&config)?;
    let mut normalized = normalize_config(provider, config);
    normalize_and_validate_ddns_domain_config(provider, &mut normalized)?;
    Ok(normalized)
}

pub(super) fn normalize_config_map(
    provider: Option<&str>,
    config: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut data = config.clone();
    data.insert(
        "update_scope".to_string(),
        normalize_update_scope(data.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str)).to_string(),
    );
    data.insert(
        DDNS_IP_SOURCE_FIELD.to_string(),
        normalize_ip_source(data.get(DDNS_IP_SOURCE_FIELD).map(String::as_str)).to_string(),
    );
    data.insert(
        DDNS_NETWORK_INTERFACE_FIELD.to_string(),
        normalize_network_interface(data.get(DDNS_NETWORK_INTERFACE_FIELD).map(String::as_str)),
    );
    data.insert(
        DDNS_INTERFACE_IPV4_INDEX_FIELD.to_string(),
        normalize_interface_index(
            data.get(DDNS_INTERFACE_IPV4_INDEX_FIELD)
                .map(String::as_str),
        ),
    );
    data.insert(
        DDNS_INTERFACE_IPV6_INDEX_FIELD.to_string(),
        normalize_interface_index(
            data.get(DDNS_INTERFACE_IPV6_INDEX_FIELD)
                .map(String::as_str),
        ),
    );
    data.insert(
        DDNS_INTERFACE_IPV4_SELECTOR_FIELD.to_string(),
        normalize_interface_selector_string(
            data.get(DDNS_INTERFACE_IPV4_SELECTOR_FIELD)
                .map(String::as_str),
            "ipv4",
        ),
    );
    data.insert(
        DDNS_INTERFACE_IPV6_SELECTOR_FIELD.to_string(),
        normalize_interface_selector_string(
            data.get(DDNS_INTERFACE_IPV6_SELECTOR_FIELD)
                .map(String::as_str),
            "ipv6",
        ),
    );
    data.insert(
        DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD.to_string(),
        normalize_config_boolean(
            data.get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
                .map(String::as_str),
        )
        .to_string(),
    );
    data.insert(
        DDNS_STATIC_IPV4_FIELD.to_string(),
        normalize_static_ip(data.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str), 4),
    );
    data.insert(
        DDNS_STATIC_IPV6_FIELD.to_string(),
        normalize_static_ip(data.get(DDNS_STATIC_IPV6_FIELD).map(String::as_str), 6),
    );
    data.insert(
        DDNS_SOURCE_DOMAIN_FIELD.to_string(),
        normalize_domain(
            data.get(DDNS_SOURCE_DOMAIN_FIELD)
                .map(String::as_str)
                .unwrap_or(""),
        ),
    );
    if is_edgeone_provider(provider.unwrap_or("")) {
        let mode = if data
            .get(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD)
            .map(String::as_str)
            == Some("block_overseas")
        {
            "block_overseas"
        } else {
            "off"
        };
        data.insert(
            DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD.to_string(),
            mode.to_string(),
        );
    }
    data
}

pub(super) fn prepare_config_for_storage(
    provider: Option<&str>,
    mut config: HashMap<String, String>,
) -> HashMap<String, String> {
    let ip_source = normalize_ip_source(config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    if ip_source == "public" {
        config.remove(DDNS_IP_SOURCE_FIELD);
    }
    if ip_source != "interface" {
        config.remove(DDNS_INTERFACE_IPV4_INDEX_FIELD);
        config.remove(DDNS_INTERFACE_IPV6_INDEX_FIELD);
        config.remove(DDNS_INTERFACE_IPV4_SELECTOR_FIELD);
        config.remove(DDNS_INTERFACE_IPV6_SELECTOR_FIELD);
        config.remove(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD);
    } else {
        if config
            .get(DDNS_INTERFACE_IPV4_SELECTOR_FIELD)
            .is_some_and(|value| !value.trim().is_empty())
        {
            config.remove(DDNS_INTERFACE_IPV4_INDEX_FIELD);
        }
        if config
            .get(DDNS_INTERFACE_IPV6_SELECTOR_FIELD)
            .is_some_and(|value| !value.trim().is_empty())
        {
            config.remove(DDNS_INTERFACE_IPV6_INDEX_FIELD);
        }
        remove_empty(&mut config, DDNS_INTERFACE_IPV4_INDEX_FIELD);
        remove_empty(&mut config, DDNS_INTERFACE_IPV6_INDEX_FIELD);
        remove_empty(&mut config, DDNS_INTERFACE_IPV4_SELECTOR_FIELD);
        remove_empty(&mut config, DDNS_INTERFACE_IPV6_SELECTOR_FIELD);
        if !config_flag_enabled(
            config
                .get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
                .map(String::as_str),
        ) {
            config.remove(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD);
        }
    }
    if ip_source != "static" {
        config.remove(DDNS_STATIC_IPV4_FIELD);
        config.remove(DDNS_STATIC_IPV6_FIELD);
    } else {
        remove_empty(&mut config, DDNS_STATIC_IPV4_FIELD);
        remove_empty(&mut config, DDNS_STATIC_IPV6_FIELD);
    }
    if ip_source != "domain" {
        config.remove(DDNS_SOURCE_DOMAIN_FIELD);
    } else {
        remove_empty(&mut config, DDNS_SOURCE_DOMAIN_FIELD);
    }
    if !is_edgeone_provider(provider.unwrap_or(""))
        || config
            .get(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD)
            .map(String::as_str)
            == Some("off")
    {
        config.remove(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
    }
    config
}

pub(super) fn validate_interface_selector_config(
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    parse_interface_selector(
        config
            .get(DDNS_INTERFACE_IPV4_SELECTOR_FIELD)
            .map(String::as_str),
        "ipv4",
    )?;
    parse_interface_selector(
        config
            .get(DDNS_INTERFACE_IPV6_SELECTOR_FIELD)
            .map(String::as_str),
        "ipv6",
    )?;
    Ok(())
}

pub(super) fn remove_empty(config: &mut HashMap<String, String>, key: &str) {
    if config.get(key).is_none_or(|value| value.trim().is_empty()) {
        config.remove(key);
    }
}

pub(super) fn duplicate_key(provider: &str, config: &HashMap<String, String>) -> String {
    let provider = provider.trim();
    let domain = domain_summary_candidate(config)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if provider.is_empty() || domain.is_empty() {
        String::new()
    } else {
        format!("{provider}::{domain}")
    }
}

pub(super) fn comparable_config_key(
    provider: Option<&str>,
    config: &HashMap<String, String>,
) -> String {
    let prepared = prepare_config_for_storage(provider, normalize_config_map(provider, config));
    let mut entries = prepared.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    serde_json::to_string(&entries).unwrap_or_default()
}

pub(super) fn normalize_interface_index(value: Option<&str>) -> String {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return String::new();
    }
    value
        .parse::<u32>()
        .ok()
        .map(|value| value.to_string())
        .unwrap_or_default()
}

pub(super) fn normalize_static_ip(value: Option<&str>, _family: u8) -> String {
    value.unwrap_or("").trim().to_string()
}

pub(super) fn config_flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

pub(super) fn normalize_config_boolean(value: Option<&str>) -> &'static str {
    if config_flag_enabled(value) {
        "true"
    } else {
        "false"
    }
}

pub(super) fn normalize_domain(value: &str) -> String {
    value.trim().trim_end_matches('.').to_string()
}

pub(super) fn is_edgeone_provider(provider: &str) -> bool {
    matches!(provider, "edgeone" | "edgeone_cname")
}
