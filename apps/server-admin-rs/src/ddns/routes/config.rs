use super::*;

pub(super) fn normalize_config(
    provider: &str,
    config: HashMap<String, String>,
) -> HashMap<String, String> {
    normalize_config_map(Some(provider), &config)
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
    } else {
        remove_empty(&mut config, DDNS_INTERFACE_IPV4_INDEX_FIELD);
        remove_empty(&mut config, DDNS_INTERFACE_IPV6_INDEX_FIELD);
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

pub(super) fn normalize_domain(value: &str) -> String {
    value.trim().trim_end_matches('.').to_string()
}

pub(super) fn is_edgeone_provider(provider: &str) -> bool {
    matches!(provider, "edgeone" | "edgeone_cname")
}
