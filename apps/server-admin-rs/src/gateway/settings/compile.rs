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
    let selections = dedupe_visibility_selection_inputs(input.get("selections"));
    let custom_cidrs =
        validate_gateway_custom_cidrs(string_list(input.get("custom_cidrs")), &translator)?;
    let mut stored_selections = Vec::new();
    let mut resolved_cidrs = Vec::new();

    for selection in selections {
        let lookup = scanner::lookup_cidr_region(
            state,
            &selection.province,
            selection.query_city.as_deref(),
        )
        .await?;
        stored_selections.push(lookup.selection);
        resolved_cidrs.extend(lookup.cidrs);
    }

    let merged_cidrs = normalize_cidr_lines(resolved_cidrs.into_iter().chain(custom_cidrs.clone()));
    let runtime_cidrs = if enabled { merged_cidrs } else { Vec::new() };

    Ok(CompiledGatewayVisibility {
        config: json!({
            "enabled": enabled,
            "selections": stored_selections,
            "custom_cidrs": custom_cidrs,
        }),
        runtime: json!({
            "enabled": enabled,
            "cidrs": runtime_cidrs,
            "updated_at": time_utils::now_iso(),
        }),
    })
}

pub(crate) async fn compile_host_visibility_config(
    state: &AppState,
    input: &Map<String, Value>,
) -> Result<Value, String> {
    let translator = Translator::from_state(state).await;
    let selections = dedupe_visibility_selection_inputs(input.get("selections"));
    let custom_cidrs =
        validate_gateway_custom_cidrs(string_list(input.get("custom_cidrs")), &translator)?;
    let mut stored_selections = Vec::new();
    let mut resolved_cidrs = Vec::new();

    for selection in selections {
        let lookup = scanner::lookup_cidr_region(
            state,
            &selection.province,
            selection.query_city.as_deref(),
        )
        .await?;
        stored_selections.push(lookup.selection);
        resolved_cidrs.extend(lookup.cidrs);
    }

    let cidrs = normalize_cidr_lines(resolved_cidrs.into_iter().chain(custom_cidrs.clone()));
    if cidrs.is_empty() {
        return Err(translator.t("server.gatewayVisibility.emptyEnabledConfig"));
    }

    Ok(json!({
        "mode": "custom",
        "selections": stored_selections,
        "custom_cidrs": custom_cidrs,
        "cidrs": cidrs,
    }))
}

#[derive(Debug, PartialEq, Eq)]
struct VisibilitySelectionInput {
    province: String,
    query_city: Option<String>,
}

fn dedupe_visibility_selection_inputs(value: Option<&Value>) -> Vec<VisibilitySelectionInput> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let Some(items) = value.and_then(Value::as_array) else {
        return result;
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
        let key = format!("{}::{}", province, query_city.as_deref().unwrap_or(""));
        if seen.insert(key) {
            result.push(VisibilitySelectionInput {
                province,
                query_city,
            });
        }
    }
    result
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
        "cidr_count": runtime.get("cidrs").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
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
