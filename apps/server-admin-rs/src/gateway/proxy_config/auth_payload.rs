use super::*;

#[cfg(test)]
pub(crate) fn build_host_rules_payload(mappings: &[Value]) -> Value {
    build_host_rules_payload_with_groups(mappings, &[])
}

pub(crate) fn build_host_rules_payload_for_config(config: &Value) -> Value {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let items = if !host_mapping_grouped_view_from_config(config) {
        build_host_rules_payload_with_groups(&mappings, &[])
    } else {
        let groups = normalize_host_mapping_groups(host_mapping_groups_from_config(config))
            .unwrap_or_default();
        let ordered = ordered_host_mappings_for_groups(&mappings, &groups);
        build_host_rules_payload_with_groups(&ordered, &groups)
    };
    let referenced = referenced_host_ipset_policy_ids(items.as_array().into_iter().flatten());
    let visibility_policies = config
        .get("visibility_policies")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|policies| policies.iter())
        .filter(|(id, _)| referenced.contains(id.as_str()))
        .map(|(id, policy)| {
            let mut policy = policy.as_object().cloned().unwrap_or_default();
            policy.insert("id".to_string(), Value::String(id.clone()));
            Value::Object(policy)
        })
        .collect::<Vec<_>>();
    json!({
        "items": items,
        "visibility_policies": visibility_policies,
    })
}

fn build_host_rules_payload_with_groups(mappings: &[Value], groups: &[Value]) -> Value {
    let group_names = host_mapping_group_names(groups);
    Value::Array(
        mappings
            .iter()
            .filter_map(Value::as_object)
            .map(|object| {
                let group_id = object
                    .get("group_id")
                    .and_then(Value::as_str)
                    .filter(|id| group_names.contains_key(*id))
                    .unwrap_or("");
                let group_name = group_names.get(group_id).map(String::as_str).unwrap_or("");
                let title = resolve_host_rule_title(object);
                let favicon = object
                    .get("favicon_override")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        object
                            .get("favicon")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .map(|value| Value::String(value.to_string()))
                    .unwrap_or(Value::Null);
                json!({
                    "host": object.get("host").cloned().unwrap_or(Value::String(String::new())),
                    "target": object.get("target").cloned().unwrap_or(Value::String(String::new())),
                    "use_auth": object.get("use_auth").cloned().unwrap_or(Value::Bool(true)),
                    "access_mode": object.get("access_mode").cloned().unwrap_or(Value::String("login_first".to_string())),
                    "suppress_toolbar": object.get("suppress_toolbar").cloned().unwrap_or(Value::Bool(false)),
                    "preserve_host": object.get("preserve_host").cloned().unwrap_or(Value::Bool(true)),
                    "is_default": object.get("is_default").cloned().unwrap_or(Value::Bool(false)),
                    "disabled": object.get("disabled").cloned().unwrap_or(Value::Bool(false)),
                    "availability": object.get("availability").cloned().unwrap_or(Value::Null),
                    "visibility": object.get("visibility").map(|visibility| json!({
                        "mode": visibility.get("mode").and_then(Value::as_str).unwrap_or("inherit"),
                        "policy_id": visibility.get("policy_id").cloned().unwrap_or(Value::Null),
                    })).unwrap_or_else(|| json!({ "mode": "inherit" })),
                    "advanced_auth": object.get("advanced_auth").cloned().unwrap_or(Value::Null),
                    "protocol_mode": normalize_protocol_mode(object.get("protocol_mode")),
                    "group_id": group_id,
                    "group_name": group_name,
                    "title": title,
                    "favicon": favicon,
                    "basic_auth": object.get("basic_auth").cloned().unwrap_or_else(disabled_host_basic_auth),
                    "locations": object.get("locations").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                })
            })
            .collect(),
    )
}

pub(super) fn resolve_host_rule_title(object: &Map<String, Value>) -> String {
    let override_title = object
        .get("title_override")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !override_title.is_empty() {
        return override_title.to_string();
    }
    object
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn build_gateway_auth_config(config: &Value) -> Value {
    let subdomain_mode = config
        .get("subdomain_mode")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let is_subdomain_mode_active = is_any_subdomain_routing_mode(config);
    let is_reverse_subdomain_mode = is_reverse_proxy_subdomain_mode(config);
    let default_auth_port = resolve_auth_service_port();
    let auth_mapping = get_auth_host_mapping(config);
    let explicit_public_auth_base_url = if is_subdomain_mode_active && !is_reverse_subdomain_mode {
        apply_public_port_to_base_url(
            subdomain_mode
                .get("public_auth_base_url")
                .and_then(Value::as_str)
                .unwrap_or(""),
            config,
        )
    } else {
        String::new()
    };
    let auth_target = auth_mapping
        .and_then(|mapping| mapping.get("target").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            subdomain_mode
                .get("auth_target")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("");
    let auth_port = parse_target_port(auth_target).unwrap_or(default_auth_port);
    let public_auth_base_url = if is_subdomain_mode_active {
        if explicit_public_auth_base_url.is_empty() {
            resolve_public_auth_base_url(config)
        } else {
            explicit_public_auth_base_url
        }
    } else {
        String::new()
    };
    let edge_client_ip_enabled = config.get("run_type").and_then(Value::as_i64) == Some(3)
        && subdomain_mode
            .get("edge_client_ip_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let tencent_edgeone_enabled = edge_client_ip_enabled
        && subdomain_mode
            .get("tencent_edgeone_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let aliyun_esa_enabled = edge_client_ip_enabled
        && !tencent_edgeone_enabled
        && subdomain_mode
            .get("aliyun_esa_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let public_http_port = if is_subdomain_mode_active {
        resolve_auth_public_port_for_scheme(config, "http", &public_auth_base_url, false)
            .unwrap_or(0)
    } else {
        0
    };
    let public_https_port = if is_subdomain_mode_active {
        resolve_auth_public_port_for_scheme(config, "https", &public_auth_base_url, true)
            .unwrap_or(0)
    } else {
        0
    };
    let auth_host = if is_subdomain_mode_active {
        auth_mapping
            .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                subdomain_mode
                    .get("auth_host")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    json!({
        "auth_port": auth_port,
        "auth_url": "/api/auth/verify",
        "login_url": "/login",
        "logout_url": "/api/auth/logout",
        "preflight_url": "/api/auth/preflight",
        "auth_cache_ttl_seconds": subdomain_mode
            .get("auth_cache_ttl_seconds")
            .and_then(json_number_floor)
            .unwrap_or(1),
        "auth_cache_unauthorized_ttl_seconds": subdomain_mode
            .get("auth_cache_unauthorized_ttl_seconds")
            .and_then(json_number_floor)
            .unwrap_or(1),
        "edge_client_ip_enabled": edge_client_ip_enabled && (aliyun_esa_enabled || tencent_edgeone_enabled),
        "aliyun_esa_enabled": aliyun_esa_enabled,
        "tencent_edgeone_enabled": tencent_edgeone_enabled,
        "public_auth_base_url": public_auth_base_url,
        "public_http_port": public_http_port,
        "public_https_port": public_https_port,
        "auth_host": auth_host,
        "trust_forwarded_proto": is_cloudflared_reverse_proxy_subdomain_mode(config),
    })
}
