use super::*;

pub(super) async fn compile_gateway_visibility_config(
    state: &AppState,
    input: &Map<String, Value>,
) -> Result<CompiledGatewayVisibility, String> {
    let translator = Translator::from_state(state).await;
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let selections = dedupe_visibility_selection_inputs(input.get("selections"))
        .map_err(|message| crate::cidr::localize_error(&translator, &message))?;
    let custom_cidrs =
        validate_gateway_custom_cidrs(string_list(input.get("custom_cidrs")), &translator)?;
    let mut stored_selections = Vec::new();
    let mut region_policies = Vec::new();

    for selection in selections {
        let lookup = crate::cidr::lookup_region(state, &selection.query())
            .await
            .map_err(|error| crate::cidr::localize_error(&translator, &error.to_string()))?;
        stored_selections.push(lookup.selection);
        region_policies.push(lookup.policy);
    }

    let custom_policy = compile_ip_set(&custom_cidrs)
        .map_err(|error| crate::cidr::localize_error(&translator, &error))?;
    let policy = if enabled {
        Some(crate::cidr::union_ip_sets(
            region_policies
                .iter()
                .chain(std::iter::once(&custom_policy)),
        ))
    } else {
        None
    };
    let policy_id = policy.as_ref().map(|value| value.id.clone());
    let source_cidr_count = policy
        .as_ref()
        .map(|value| value.source_cidr_count)
        .unwrap_or_default();
    let range_count = policy
        .as_ref()
        .map(CompiledIpSet::range_count)
        .unwrap_or_default();
    let runtime_policy = policy
        .as_ref()
        .map(CompiledIpSet::to_compact_transport_value);

    Ok(CompiledGatewayVisibility {
        config: json!({
            "enabled": enabled,
            "selections": stored_selections,
            "custom_cidrs": custom_cidrs,
            "policy_id": policy_id,
            "source_cidr_count": source_cidr_count,
            "range_count": range_count,
        }),
        runtime: json!({
            "enabled": enabled,
            "policy_id": policy.as_ref().map(|value| value.id.clone()),
            "source_cidr_count": source_cidr_count,
            "range_count": range_count,
            "policy": runtime_policy,
            "updated_at": time_utils::now_iso(),
        }),
        policy,
    })
}

pub(crate) async fn compile_host_visibility_config(
    state: &AppState,
    input: &Map<String, Value>,
) -> Result<CompiledHostVisibility, String> {
    let translator = Translator::from_state(state).await;
    let selections = dedupe_visibility_selection_inputs(input.get("selections"))
        .map_err(|message| crate::cidr::localize_error(&translator, &message))?;
    let custom_cidrs =
        validate_gateway_custom_cidrs(string_list(input.get("custom_cidrs")), &translator)?;
    let mut stored_selections = Vec::new();
    let mut region_policies = Vec::new();

    for selection in selections {
        let lookup = crate::cidr::lookup_region(state, &selection.query())
            .await
            .map_err(|error| crate::cidr::localize_error(&translator, &error.to_string()))?;
        stored_selections.push(lookup.selection);
        region_policies.push(lookup.policy);
    }

    let custom_policy = compile_ip_set(&custom_cidrs)
        .map_err(|error| crate::cidr::localize_error(&translator, &error))?;
    let policy = crate::cidr::union_ip_sets(
        region_policies
            .iter()
            .chain(std::iter::once(&custom_policy)),
    );
    if policy.range_count() == 0 {
        return Err(translator.t("server.gatewayVisibility.emptyEnabledConfig"));
    }

    Ok(CompiledHostVisibility {
        config: json!({
            "mode": "custom",
            "selections": stored_selections,
            "custom_cidrs": custom_cidrs,
            "policy_id": policy.id,
            "source_cidr_count": policy.source_cidr_count,
            "range_count": policy.range_count(),
        }),
        policy,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct VisibilitySelectionInput {
    pub(super) province: String,
    pub(super) query_city: Option<String>,
    pub(super) operator: Option<CidrOperator>,
}

impl VisibilitySelectionInput {
    fn query(&self) -> CidrRegionQuery {
        CidrRegionQuery::new(
            self.province.clone(),
            self.query_city.clone(),
            self.operator,
        )
    }
}

pub(super) fn dedupe_visibility_selection_inputs(
    value: Option<&Value>,
) -> Result<Vec<VisibilitySelectionInput>, String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(result);
    };
    for item in items {
        let province = item
            .get("province")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if province.is_empty() {
            continue;
        }
        let query_city = item
            .get("query_city")
            .or_else(|| item.get("queryCity"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let operator = CidrOperator::parse_value(item.get("operator"))?;
        let key = CidrRegionQuery::new(province.clone(), query_city.clone(), operator).key();
        if seen.insert(key) {
            result.push(VisibilitySelectionInput {
                province,
                query_city,
                operator,
            });
        }
    }
    Ok(result)
}

pub(super) fn validate_gateway_custom_cidrs(
    values: Vec<Value>,
    translator: &Translator,
) -> Result<Vec<String>, String> {
    let cidrs = normalize_cidr_lines(values.into_iter().filter_map(|value| {
        value.as_str().map(|value| value.to_string()).or_else(|| {
            if value.is_null() {
                None
            } else {
                Some(value.to_string())
            }
        })
    }));
    let invalid = cidrs
        .iter()
        .filter(|cidr| !is_valid_cidr(cidr))
        .cloned()
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(cidrs)
    } else {
        Err(translator.t_params(
            "server.gatewayVisibility.customCidrInvalid",
            &[("cidrs", invalid.join(", "))],
        ))
    }
}

pub(super) fn normalize_cidr_lines(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let raw = value.trim();
        if raw.is_empty() {
            continue;
        }
        let cidr = raw
            .parse::<IpNet>()
            .map(|network| match network {
                IpNet::V4(network) => {
                    format!("{}/{}", network.network(), network.prefix_len())
                }
                IpNet::V6(network) => {
                    format!("{}/{}", network.network(), network.prefix_len())
                }
            })
            .unwrap_or_else(|_| raw.to_string());
        if seen.insert(cidr.to_ascii_lowercase()) {
            result.push(cidr);
        }
    }
    result
}

pub(super) fn is_valid_cidr(value: &str) -> bool {
    let normalized = value.trim();
    let Some((address, prefix_raw)) = normalized.split_once('/') else {
        return false;
    };
    if address.trim().is_empty()
        || prefix_raw.trim().is_empty()
        || prefix_raw.trim().chars().any(|ch| !ch.is_ascii_digit())
    {
        return false;
    }
    let Ok(prefix) = prefix_raw.trim().parse::<u16>() else {
        return false;
    };
    match address.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => prefix <= 32,
        Ok(IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

pub(super) fn compile_gateway_proxy_headers_state(
    config: &Value,
    requested: &Value,
) -> CompiledGatewayTargetRuntime {
    let next_config = sanitize_disabled_hosts_config(config, requested);
    let host_mappings = config_host_mappings(config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_proxy_header_items(&visible_hosts, &next_config);
    let omit_targets = omitted_targets(&items, "send_proxy_headers");
    let enabled = is_any_subdomain_routing_mode(config);

    CompiledGatewayTargetRuntime {
        config: next_config,
        runtime: json!({
            "enabled": enabled,
            "omit_targets": if enabled { omit_targets } else { Vec::<String>::new() },
            "updated_at": time_utils::now_iso(),
        }),
    }
}

pub(super) fn compile_gateway_host_response_state(
    config: &Value,
    requested: &Value,
) -> CompiledGatewayTargetRuntime {
    let next_config = sanitize_disabled_hosts_config(config, requested);
    let host_mappings = config_host_mappings(config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_host_response_items(&visible_hosts, &next_config);
    let omit_targets = omitted_targets(&items, "preserve_host");
    let enabled = is_any_subdomain_routing_mode(config);

    CompiledGatewayTargetRuntime {
        config: next_config,
        runtime: json!({
            "enabled": enabled,
            "omit_targets": if enabled { omit_targets } else { Vec::<String>::new() },
            "updated_at": time_utils::now_iso(),
        }),
    }
}

pub(super) fn omitted_targets(items: &[Value], enabled_field: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for item in items {
        if item.get(enabled_field).and_then(Value::as_bool) != Some(false) {
            continue;
        }
        let target = item
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !target.is_empty() && seen.insert(target.to_string()) {
            targets.push(target.to_string());
        }
    }
    targets
}

pub(super) fn disabled_hosts_config_from_body(body: &Value) -> Result<Value, String> {
    let Some(object) = body.as_object() else {
        return Err("Gateway payload must be an object".to_string());
    };
    Ok(json!({
        "disabled_hosts": string_list(object.get("disabled_hosts")),
    }))
}

pub(super) fn build_gateway_visibility_summary(config: &Value, runtime: &Value) -> Value {
    json!({
        "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "selection_count": config.get("selections").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "custom_cidr_count": config.get("custom_cidrs").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "cidr_count": runtime.get("source_cidr_count").and_then(Value::as_u64)
            .or_else(|| runtime.get("cidrs").and_then(Value::as_array).map(|items| items.len() as u64))
            .unwrap_or(0),
        "range_count": runtime.get("range_count").and_then(Value::as_u64).unwrap_or(0),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn build_gateway_proxy_headers_summary(items: &[Value], runtime: &Value) -> Value {
    json!({
        "total_count": items.len(),
        "disabled_count": items.iter().filter(|item| item.get("send_proxy_headers").and_then(Value::as_bool) == Some(false)).count(),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn build_gateway_host_response_summary(items: &[Value], runtime: &Value) -> Value {
    json!({
        "total_count": items.len(),
        "disabled_count": items.iter().filter(|item| item.get("preserve_host").and_then(Value::as_bool) == Some(false)).count(),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn build_proxy_headers_availability(config: &Value, translator: &Translator) -> Value {
    if is_any_subdomain_routing_mode(config) {
        return json!({ "available": true, "reason": "" });
    }
    json!({
        "available": false,
        "reason": translator.t_params(
            "server.gatewayProxyHeaders.unavailableReason",
            &[("mode", run_type_label(translator, config, "server.gatewayProxyHeaders.runTypes"))],
        ),
    })
}

pub(super) fn build_host_response_availability(config: &Value, translator: &Translator) -> Value {
    if is_any_subdomain_routing_mode(config) {
        return json!({ "available": true, "reason": "" });
    }
    json!({
        "available": false,
        "reason": translator.t_params(
            "server.gatewayHostResponse.unavailableReason",
            &[("mode", run_type_label(translator, config, "server.gatewayHostResponse.runTypes"))],
        ),
    })
}

pub(super) fn run_type_label(translator: &Translator, config: &Value, prefix: &str) -> String {
    match config.get("run_type").and_then(Value::as_i64).unwrap_or(3) {
        0 => translator.t(&format!("{prefix}.direct")),
        1 => translator.t(&format!("{prefix}.reverseProxy")),
        _ => translator.t(&format!("{prefix}.subdomain")),
    }
}

pub(super) fn config_host_mappings(config: &Value) -> Vec<Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn sanitize_disabled_hosts_config(config: &Value, raw_config: &Value) -> Value {
    let visible_hosts = visible_host_mappings(&config_host_mappings(config))
        .iter()
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_host)
        .filter(|host| !host.is_empty())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let disabled_hosts = raw_config
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| {
                    !host.is_empty() && visible_hosts.contains(host) && seen.insert(host.clone())
                })
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "disabled_hosts": disabled_hosts })
}

pub(super) use crate::proxy_utils::is_any_subdomain_routing_mode;
