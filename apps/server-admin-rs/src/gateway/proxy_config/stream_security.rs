use super::*;
use ipnet::IpNet;
use std::net::IpAddr;

use crate::cidr::{self, CidrOperator, CidrRegionQuery, CompiledIpSet};

const MAX_STREAM_BYPASS_GROUPS: usize = 16;
const MAX_STREAM_BYPASS_CONDITIONS: usize = 16;
const MAX_STREAM_BYPASS_VALUES: usize = 256;
const MAX_STREAM_BYPASS_TOTAL_VALUES: usize = 4_096;
const MAX_STREAM_BYPASS_RESOLVED_CIDRS: usize = 100_000;

fn mapping_key(value: &Value) -> Option<(String, u16)> {
    let protocol = value.get("protocol")?.as_str()?.trim().to_ascii_lowercase();
    let port = value.get("listen_port")?.as_u64()?.try_into().ok()?;
    Some((protocol, port))
}

pub(super) fn prepare_stream_mapping_update(previous: &[Value], next: &mut [Value]) {
    const SECURITY_FIELDS: [&str; 5] = [
        "disabled",
        "validation_mode",
        "service_profile",
        "probe_status",
        "bypass_policy",
    ];
    let previous_by_key = previous
        .iter()
        .filter_map(|mapping| mapping_key(mapping).map(|key| (key, mapping)))
        .collect::<HashMap<_, _>>();
    for mapping in next {
        let Some(key) = mapping_key(mapping) else {
            continue;
        };
        let Some(object) = mapping.as_object_mut() else {
            continue;
        };
        let Some(previous) = previous_by_key.get(&key) else {
            for field in SECURITY_FIELDS {
                object.remove(field);
            }
            object.insert("disabled".into(), Value::Bool(true));
            object.insert("probe_status".into(), Value::String("stale".into()));
            object.insert("validation_mode".into(), Value::String("off".into()));
            continue;
        };
        let target_unchanged = previous.get("target") == object.get("target");
        if target_unchanged {
            for field in SECURITY_FIELDS {
                if let Some(value) = previous.get(field) {
                    object.insert(field.into(), value.clone());
                } else {
                    object.remove(field);
                }
            }
        } else {
            for field in SECURITY_FIELDS {
                object.remove(field);
            }
            object.insert("disabled".into(), Value::Bool(true));
            object.insert("probe_status".into(), Value::String("stale".into()));
            object.insert("validation_mode".into(), Value::String("off".into()));
            if let Some(mut policy) = previous.get("bypass_policy").cloned() {
                if let Some(policy) = policy.as_object_mut() {
                    policy.insert("enabled".into(), Value::Bool(false));
                }
                object.insert("bypass_policy".into(), policy);
            }
        }
        if object.get("use_auth").and_then(Value::as_bool) == Some(false)
            && let Some(policy) = object
                .get_mut("bypass_policy")
                .and_then(Value::as_object_mut)
        {
            policy.insert("enabled".into(), Value::Bool(false));
        }
    }
}

pub(super) fn prune_stream_access_policies(config: &mut Value) {
    let referenced = config
        .get("stream_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|mapping| {
            mapping
                .get("bypass_policy")
                .and_then(|policy| policy.get("groups"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .flat_map(|group| {
            group
                .get("conditions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|condition| condition.get("policy_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    let Some(policies) = config
        .get_mut("stream_access_policies")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut retained = HashSet::new();
    policies.retain(|policy| {
        policy
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|id| referenced.contains(id) && retained.insert(id.to_string()))
    });
}

fn find_mapping_mut<'a>(config: &'a mut Value, protocol: &str, port: u16) -> Option<&'a mut Value> {
    config
        .get_mut("stream_mappings")?
        .as_array_mut()?
        .iter_mut()
        .find(|mapping| {
            mapping_key(mapping).is_some_and(|(candidate_protocol, candidate_port)| {
                candidate_protocol == protocol && candidate_port == port
            })
        })
}

fn find_mapping<'a>(config: &'a Value, protocol: &str, port: u16) -> Option<&'a Value> {
    config
        .get("stream_mappings")?
        .as_array()?
        .iter()
        .find(|mapping| {
            mapping_key(mapping).is_some_and(|(candidate_protocol, candidate_port)| {
                candidate_protocol == protocol && candidate_port == port
            })
        })
}

fn clear_stream_service_profile_fields(object: &mut serde_json::Map<String, Value>) {
    object.remove("service_profile");
    object.insert("probe_status".into(), Value::String("stale".into()));
    object.insert("validation_mode".into(), Value::String("off".into()));
    object.insert("disabled".into(), Value::Bool(true));
}

fn service_profile_precondition_matches(
    mapping: &Value,
    expected_target: &str,
    expected_service_id: &str,
) -> bool {
    mapping.get("target").and_then(Value::as_str) == Some(expected_target)
        && mapping
            .pointer("/service_profile/service_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            == expected_service_id
}

fn bypass_mapping_precondition_matches(
    mapping: &Value,
    expected_target: &str,
    expected_use_auth: bool,
) -> bool {
    mapping.get("target").and_then(Value::as_str) == Some(expected_target)
        && mapping
            .get("use_auth")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            == expected_use_auth
}

async fn persist_stream_security_change(
    state: &AppState,
    previous: &Value,
    updated: &Value,
) -> Result<(), String> {
    state
        .storage
        .store
        .save_config(updated)
        .await
        .map_err(|error| error.to_string())?;
    if let Err(error) = sync_stream_mappings_runtime(state, updated).await {
        rollback_stream_mappings(state, previous).await;
        return Err(error);
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/admin/config/stream_service_catalog",
    tag = "configuration",
    operation_id = "get_api_admin_config_stream_service_catalog",
    responses((status = 200, description = "Stream service catalog"))
)]
pub(super) async fn get_stream_service_catalog(State(state): State<AppState>) -> Response {
    match state.gateway.client.get_stream_service_catalog().await {
        Ok(catalog) => response::ok(catalog).into_response(),
        Err(error) => response::error(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/stream_mappings/{protocol}/{port}/probe",
    tag = "configuration",
    operation_id = "post_api_admin_config_stream_mapping_probe",
    params(
        ("protocol" = String, Path, description = "tcp or udp"),
        ("port" = u16, Path, description = "Listening port")
    ),
    responses((status = 200, description = "Probe result"))
)]
pub(super) async fn probe_stream_mapping(
    State(state): State<AppState>,
    Path((protocol, port)): Path<(String, u16)>,
) -> Response {
    let protocol = protocol.trim().to_ascii_lowercase();
    if protocol != "tcp" && protocol != "udp" {
        return response::error(StatusCode::BAD_REQUEST, "protocol must be tcp or udp");
    }
    let before_probe = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(mapping) = find_mapping(&before_probe, &protocol, port) else {
        return response::error(StatusCode::NOT_FOUND, "stream mapping not found");
    };
    let target = mapping
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let probe = match state
        .gateway
        .client
        .probe_stream_target(&protocol, &target)
        .await
    {
        Ok(result) => result,
        Err(error) => return response::error(StatusCode::BAD_GATEWAY, error.to_string()),
    };

    let _guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let previous = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(current) = find_mapping(&previous, &protocol, port) else {
        return response::error(StatusCode::CONFLICT, "stream mapping changed during probe");
    };
    if current.get("target").and_then(Value::as_str) != Some(target.as_str()) {
        return response::error(
            StatusCode::CONFLICT,
            "stream mapping target changed during probe",
        );
    }

    let mut updated = previous.clone();
    let Some(object) =
        find_mapping_mut(&mut updated, &protocol, port).and_then(Value::as_object_mut)
    else {
        return response::error(StatusCode::CONFLICT, "stream mapping changed during probe");
    };
    let status = probe
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let strong = probe
        .get("profile")
        .and_then(|value| value.get("service_confidence"))
        .and_then(Value::as_str)
        == Some("strong");
    let strict_capable = probe
        .get("profile")
        .and_then(|value| value.get("strict_capable"))
        .and_then(Value::as_bool)
        == Some(true);
    object.insert("probe_status".into(), Value::String(status.to_string()));
    object.remove("service_profile");
    if let Some(profile) = probe.get("profile").filter(|value| {
        value
            .get("service_id")
            .and_then(Value::as_str)
            .is_some_and(|service_id| !service_id.is_empty())
    }) {
        object.insert("service_profile".into(), profile.clone());
    }
    let verified = status == "verified" && strong && strict_capable;
    object.insert("disabled".into(), Value::Bool(!verified));
    object.insert(
        "validation_mode".into(),
        Value::String(if verified { "strict" } else { "off" }.into()),
    );
    if let Err(error) = persist_stream_security_change(&state, &previous, &updated).await {
        return response::error(StatusCode::BAD_GATEWAY, error);
    }
    response::ok(probe).into_response()
}

#[utoipa::path(
    put,
    path = "/api/admin/config/stream_mappings/{protocol}/{port}/service_profile",
    tag = "configuration",
    operation_id = "put_api_admin_config_stream_mapping_service_profile",
    params(("protocol" = String, Path), ("port" = u16, Path)),
    request_body = Value,
    responses((status = 200, description = "Manually confirmed service profile"))
)]
pub(super) async fn confirm_stream_service_profile(
    State(state): State<AppState>,
    Path((protocol, port)): Path<(String, u16)>,
    Json(body): Json<Value>,
) -> Response {
    let protocol = protocol.trim().to_ascii_lowercase();
    let service_id = body
        .get("service_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let Some(expected_target) = body
        .get("expected_target")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return response::error(StatusCode::BAD_REQUEST, "expected_target is required");
    };
    let Some(expected_service_id) = body
        .get("expected_service_id")
        .and_then(Value::as_str)
        .map(str::trim)
    else {
        return response::error(StatusCode::BAD_REQUEST, "expected_service_id is required");
    };
    if service_id.is_empty() {
        let _guard = state.gateway.protocol_mapping_update_lock.lock().await;
        let previous = match state.storage.store.get_config().await {
            Ok(config) => config,
            Err(error) => {
                return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
            }
        };
        let Some(current) = find_mapping(&previous, &protocol, port) else {
            return response::error(StatusCode::NOT_FOUND, "stream mapping not found");
        };
        if !service_profile_precondition_matches(current, expected_target, expected_service_id) {
            return response::error(
                StatusCode::CONFLICT,
                "stream mapping or service profile changed; reload before clearing",
            );
        }
        if current
            .pointer("/service_profile/source")
            .and_then(Value::as_str)
            != Some("manual")
        {
            return response::error(
                StatusCode::BAD_REQUEST,
                "only a manually specified service profile can be cleared",
            );
        }
        let mut updated = previous.clone();
        let Some(object) =
            find_mapping_mut(&mut updated, &protocol, port).and_then(Value::as_object_mut)
        else {
            return response::error(StatusCode::CONFLICT, "stream mapping changed");
        };
        clear_stream_service_profile_fields(object);
        if let Err(error) = persist_stream_security_change(&state, &previous, &updated).await {
            return response::error(StatusCode::BAD_GATEWAY, error);
        }
        return response::ok(json!({ "cleared": true })).into_response();
    }
    let catalog = match state.gateway.client.get_stream_service_catalog().await {
        Ok(catalog) => catalog,
        Err(error) => return response::error(StatusCode::BAD_GATEWAY, error.to_string()),
    };
    let descriptor = catalog
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("service_id").and_then(Value::as_str) == Some(service_id.as_str()));
    let Some(descriptor) = descriptor else {
        return response::error(StatusCode::BAD_REQUEST, "unknown stream service");
    };
    let strict_capable = descriptor
        .get("strict_capable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let transport_supported = descriptor
        .get("transports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|value| value.as_str() == Some(protocol.as_str()));
    if !strict_capable || !transport_supported {
        return response::error(
            StatusCode::BAD_REQUEST,
            "selected service cannot be strictly validated on this transport",
        );
    }
    let _guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let previous = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(mapping) = find_mapping(&previous, &protocol, port) else {
        return response::error(StatusCode::NOT_FOUND, "stream mapping not found");
    };
    if !service_profile_precondition_matches(mapping, expected_target, expected_service_id) {
        return response::error(
            StatusCode::CONFLICT,
            "stream mapping or service profile changed; reload before saving",
        );
    }
    let target = mapping
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let fingerprint =
        crate::crypto_utils::sha256_hex_bytes(format!("{protocol}\0{target}").as_bytes());
    let profile = json!({
        "service_id": service_id,
        "service_family": descriptor.get("service_family").cloned().unwrap_or(Value::Null),
        "device_role": "",
        "service_confidence": "strong",
        "role_confidence": "unknown",
        "source": "manual",
        "observed_at": crate::time_utils::now_iso(),
        "classifier_version": catalog.get("classifier_version").cloned().unwrap_or(Value::Null),
        "target_fingerprint": fingerprint,
        "evidence_codes": ["administrator_confirmed"],
        "strict_capable": true,
        "metadata": {},
    });
    let mut updated = previous.clone();
    let Some(object) =
        find_mapping_mut(&mut updated, &protocol, port).and_then(Value::as_object_mut)
    else {
        return response::error(StatusCode::CONFLICT, "stream mapping changed");
    };
    object.insert("service_profile".into(), profile.clone());
    object.insert("probe_status".into(), Value::String("manual".into()));
    object.insert("validation_mode".into(), Value::String("strict".into()));
    object.insert("disabled".into(), Value::Bool(false));
    if let Err(error) = persist_stream_security_change(&state, &previous, &updated).await {
        return response::error(StatusCode::BAD_GATEWAY, error);
    }
    response::ok(profile).into_response()
}

#[utoipa::path(
    get,
    path = "/api/admin/config/stream_mappings/{protocol}/{port}/bypass_policy",
    tag = "configuration",
    operation_id = "get_api_admin_config_stream_mapping_bypass_policy",
    params(("protocol" = String, Path), ("port" = u16, Path)),
    responses((status = 200, description = "Stream bypass policy"))
)]
pub(super) async fn get_stream_bypass_policy(
    State(state): State<AppState>,
    Path((protocol, port)): Path<(String, u16)>,
) -> Response {
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(mapping) = find_mapping(&config, &protocol.to_ascii_lowercase(), port) else {
        return response::error(StatusCode::NOT_FOUND, "stream mapping not found");
    };
    response::ok(mapping.get("bypass_policy").cloned().unwrap_or_else(|| {
        json!({
            "enabled": false, "policy_version": "", "groups": [], "broad_rule_confirmed": false,
        })
    }))
    .into_response()
}

fn source_ip_cidrs(condition: &Map<String, Value>, operator: &str) -> Result<Vec<String>, String> {
    let values = condition
        .get("values")
        .or_else(|| condition.get("cidrs"))
        .and_then(Value::as_array)
        .ok_or_else(|| "source_ip condition requires values".to_string())?;
    if values.is_empty() {
        return Err("source_ip condition requires at least one value".to_string());
    }
    values
        .iter()
        .map(|value| {
            let value = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "source_ip values must be non-empty strings".to_string())?;
            if operator == "equals" || operator == "not_equals" {
                if value.contains('/') {
                    return Err(format!(
                        "source_ip operator {operator} requires IP addresses, not CIDRs"
                    ));
                }
                let address: IpAddr = value
                    .parse()
                    .map_err(|_| format!("invalid IP address {value}"))?;
                return Ok(format!(
                    "{address}/{}",
                    if address.is_ipv4() { 32 } else { 128 }
                ));
            }
            if !value.contains('/') {
                return Err(format!(
                    "source_ip operator {operator} requires CIDR ranges"
                ));
            }
            let network: IpNet = value
                .parse()
                .map_err(|_| format!("invalid CIDR range {value}"))?;
            Ok(network.to_string())
        })
        .collect()
}

fn disabled_stream_bypass_policy(mapping: &Value) -> Value {
    let groups = mapping
        .pointer("/bypass_policy/groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "enabled": false,
        "policy_version": uuid::Uuid::new_v4().to_string(),
        "groups": groups,
        "broad_rule_confirmed": false,
    })
}

fn is_broad_stream_policy(policy: &CompiledIpSet) -> bool {
    policy.to_cidrs().iter().any(|cidr| {
        cidr.parse::<IpNet>().is_ok_and(|network| match network {
            IpNet::V4(network) => network.prefix_len() <= 1,
            IpNet::V6(network) => network.prefix_len() <= 1,
        })
    })
}

fn persisted_stream_values(condition: &Map<String, Value>, target: &str) -> Vec<Value> {
    if target != "source_ip" {
        return Vec::new();
    }
    condition
        .get("values")
        .or_else(|| condition.get("cidrs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Value::String(value.to_string()))
        .collect()
}

fn persisted_stream_selections(condition: &Map<String, Value>, target: &str) -> Vec<Value> {
    if target != "source_region" {
        return Vec::new();
    }
    condition
        .get("selections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|selection| {
            json!({
                "province": selection.get("province").and_then(Value::as_str).map(str::trim).unwrap_or_default(),
                "city": selection.get("city").and_then(Value::as_str).map(str::trim),
                "query_city": selection.get("query_city").and_then(Value::as_str).map(str::trim),
                "operator": selection.get("operator").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

async fn compile_stream_bypass_policy(
    state: &AppState,
    requested: &Value,
    use_auth: bool,
) -> Result<(Value, Vec<CompiledIpSet>), String> {
    let requested = requested
        .as_object()
        .ok_or_else(|| "stream bypass policy must be an object".to_string())?;
    let requested_enabled = requested
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let broad_confirmed = requested
        .get("broad_rule_confirmed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let groups = requested
        .get("groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if groups.len() > MAX_STREAM_BYPASS_GROUPS {
        return Err(format!(
            "stream bypass policy supports at most {MAX_STREAM_BYPASS_GROUPS} groups"
        ));
    }
    let mut compiled_groups = Vec::with_capacity(groups.len());
    let mut policies = Vec::new();
    let mut seen_groups = HashSet::new();
    let mut total_values = 0usize;
    let mut resolved_cidrs = 0usize;
    for (group_index, group) in groups.iter().enumerate() {
        let group = group
            .as_object()
            .ok_or_else(|| "stream bypass group must be an object".to_string())?;
        let group_id = group
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("group-{}", group_index + 1));
        if !seen_groups.insert(group_id.clone()) {
            return Err(format!("duplicate stream bypass group id {group_id}"));
        }
        let conditions = group
            .get("conditions")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("stream bypass group {group_id} requires conditions"))?;
        if conditions.is_empty() {
            return Err(format!(
                "stream bypass group {group_id} requires conditions"
            ));
        }
        if conditions.len() > MAX_STREAM_BYPASS_CONDITIONS {
            return Err(format!(
                "stream bypass group {group_id} supports at most {MAX_STREAM_BYPASS_CONDITIONS} conditions"
            ));
        }
        let mut positive = false;
        let mut compiled_conditions = Vec::with_capacity(conditions.len());
        let mut seen_conditions = HashSet::new();
        for (condition_index, condition) in conditions.iter().enumerate() {
            let condition = condition
                .as_object()
                .ok_or_else(|| "stream bypass condition must be an object".to_string())?;
            let id = condition
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("condition-{}", condition_index + 1));
            if !seen_conditions.insert(id.clone()) {
                return Err(format!(
                    "duplicate stream bypass condition id {id} in group {group_id}"
                ));
            }
            let target = condition
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let operator = condition
                .get("operator")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let value_count = match target.as_str() {
                "source_ip" => condition
                    .get("values")
                    .or_else(|| condition.get("cidrs"))
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                "source_region" => condition
                    .get("selections")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                _ => 0,
            };
            if value_count > MAX_STREAM_BYPASS_VALUES {
                return Err(format!(
                    "stream bypass condition {id} supports at most {MAX_STREAM_BYPASS_VALUES} values"
                ));
            }
            total_values = total_values.saturating_add(value_count);
            if total_values > MAX_STREAM_BYPASS_TOTAL_VALUES {
                return Err(format!(
                    "stream bypass policy supports at most {MAX_STREAM_BYPASS_TOTAL_VALUES} total values"
                ));
            }
            let policy = match target.as_str() {
                "source_ip" => {
                    if !["equals", "not_equals", "in_cidr", "not_in_cidr"]
                        .contains(&operator.as_str())
                    {
                        return Err(format!("unsupported source_ip operator {operator}"));
                    }
                    let cidrs = source_ip_cidrs(condition, &operator)?;
                    crate::cidr::compile_ip_set(&cidrs)?
                }
                "source_region" => {
                    if operator != "in" && operator != "not_in" {
                        return Err(format!("unsupported source_region operator {operator}"));
                    }
                    let selections = condition
                        .get("selections")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "source_region condition requires selections".to_string())?;
                    if selections.is_empty() {
                        return Err(
                            "source_region condition requires at least one selection".to_string()
                        );
                    }
                    let mut selection_policies = Vec::new();
                    for selection in selections {
                        let selection = selection
                            .as_object()
                            .ok_or_else(|| "region selection must be an object".to_string())?;
                        let province = selection
                            .get("province")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| "region province is required".to_string())?;
                        let city = selection
                            .get("query_city")
                            .or_else(|| selection.get("city"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty());
                        let carrier = CidrOperator::parse_value(selection.get("operator"))?;
                        let lookup = cidr::lookup_region(
                            state,
                            &CidrRegionQuery::new(province, city, carrier),
                        )
                        .await
                        .map_err(|error| error.to_string())?;
                        if lookup.is_empty() {
                            return Err(format!("region {province} resolved to no CIDRs"));
                        }
                        selection_policies.push(lookup.policy);
                    }
                    crate::cidr::union_ip_sets(selection_policies.iter())
                }
                _ => return Err("stream bypass only supports source_ip and source_region".into()),
            };
            if ["equals", "in", "in_cidr"].contains(&operator.as_str()) {
                positive = true;
            }
            resolved_cidrs = resolved_cidrs.saturating_add(policy.source_cidr_count);
            if resolved_cidrs > MAX_STREAM_BYPASS_RESOLVED_CIDRS {
                return Err(format!(
                    "resolved stream bypass CIDRs exceed {MAX_STREAM_BYPASS_RESOLVED_CIDRS}"
                ));
            }
            if requested_enabled && is_broad_stream_policy(&policy) && !broad_confirmed {
                return Err("broad IP rules require broad_rule_confirmed".into());
            }
            let persisted_values = persisted_stream_values(condition, &target);
            let persisted_selections = persisted_stream_selections(condition, &target);
            let policy_id = policy.id.clone();
            policies.push(policy);
            compiled_conditions.push(json!({
                "id": id,
                "target": target,
                "operator": operator,
                "policy_id": policy_id,
                "values": persisted_values,
                "selections": persisted_selections,
            }));
        }
        if requested_enabled && !positive && !broad_confirmed {
            return Err(format!(
                "negative-only group {group_id} requires broad_rule_confirmed"
            ));
        }
        compiled_groups.push(json!({"id": group_id, "conditions": compiled_conditions}));
    }
    if requested_enabled && groups.is_empty() {
        return Err("enabled stream bypass policy requires at least one group".into());
    }
    Ok((
        json!({
            "enabled": requested_enabled && use_auth,
            "policy_version": uuid::Uuid::new_v4().to_string(),
            "groups": compiled_groups,
            "broad_rule_confirmed": broad_confirmed,
        }),
        policies,
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/config/stream_mappings/{protocol}/{port}/bypass_policy",
    tag = "configuration",
    operation_id = "put_api_admin_config_stream_mapping_bypass_policy",
    params(("protocol" = String, Path), ("port" = u16, Path)),
    request_body = Value,
    responses((status = 200, description = "Compiled stream bypass policy"))
)]
pub(super) async fn update_stream_bypass_policy(
    State(state): State<AppState>,
    Path((protocol, port)): Path<(String, u16)>,
    Json(body): Json<Value>,
) -> Response {
    let protocol = protocol.trim().to_ascii_lowercase();
    let expected_version = body
        .get("policy_version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let before_compile = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(mapping) = find_mapping(&before_compile, &protocol, port) else {
        return response::error(StatusCode::NOT_FOUND, "stream mapping not found");
    };
    let Some(expected_target) = body
        .get("expected_target")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return response::error(StatusCode::BAD_REQUEST, "expected_target is required");
    };
    let Some(expected_use_auth) = body.get("expected_use_auth").and_then(Value::as_bool) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            "expected_use_auth must be a boolean",
        );
    };
    if !bypass_mapping_precondition_matches(mapping, expected_target, expected_use_auth) {
        return response::error(
            StatusCode::CONFLICT,
            "stream mapping changed; reload before saving bypass policy",
        );
    }
    let target = mapping
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let use_auth = mapping
        .get("use_auth")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let requested_enabled = match body.get("enabled").and_then(Value::as_bool) {
        Some(enabled) => enabled,
        None => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "stream bypass policy enabled must be a boolean",
            );
        }
    };
    if requested_enabled && !use_auth {
        return response::error(
            StatusCode::BAD_REQUEST,
            "enable authentication before enabling stream login bypass",
        );
    }
    let current_version = mapping
        .get("bypass_policy")
        .and_then(|policy| policy.get("policy_version"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if current_version != expected_version {
        return response::error(
            StatusCode::CONFLICT,
            "stream bypass policy changed; reload before saving",
        );
    }
    // Disabling is an emergency operation: it must not depend on region CIDR
    // lookups or on unfinished edits in the now-hidden form. Preserve the last
    // successfully compiled draft and rotate only its version and active flag.
    let (policy, policies) = if requested_enabled {
        match compile_stream_bypass_policy(&state, &body, use_auth).await {
            Ok(result) => result,
            Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
        }
    } else {
        (disabled_stream_bypass_policy(mapping), Vec::new())
    };
    let _guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let previous = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(current) = find_mapping(&previous, &protocol, port) else {
        return response::error(StatusCode::CONFLICT, "stream mapping changed");
    };
    let latest_version = current
        .get("bypass_policy")
        .and_then(|policy| policy.get("policy_version"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if current.get("target").and_then(Value::as_str) != Some(target.as_str())
        || current
            .get("use_auth")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            != use_auth
        || latest_version != expected_version
    {
        return response::error(
            StatusCode::CONFLICT,
            "stream mapping or bypass policy changed; reload before saving",
        );
    }
    let mut updated = previous.clone();
    let Some(mapping) =
        find_mapping_mut(&mut updated, &protocol, port).and_then(Value::as_object_mut)
    else {
        return response::error(StatusCode::CONFLICT, "stream mapping changed");
    };
    mapping.insert("bypass_policy".into(), policy.clone());
    let access_policies = ensure_object(&mut updated)
        .entry("stream_access_policies")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(access_policies) = access_policies.as_array_mut() else {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stream access policy storage is invalid",
        );
    };
    for policy in policies {
        let encoded = policy.to_transport_value();
        if !access_policies
            .iter()
            .any(|item| item.get("id") == encoded.get("id"))
        {
            access_policies.push(encoded);
        }
    }
    prune_stream_access_policies(&mut updated);
    if let Err(error) = persist_stream_security_change(&state, &previous, &updated).await {
        return response::error(StatusCode::BAD_GATEWAY, error);
    }
    response::ok(policy).into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        bypass_mapping_precondition_matches, clear_stream_service_profile_fields,
        disabled_stream_bypass_policy, is_broad_stream_policy, persisted_stream_selections,
        persisted_stream_values, prepare_stream_mapping_update, prune_stream_access_policies,
        service_profile_precondition_matches, source_ip_cidrs,
    };
    use serde_json::json;

    #[test]
    fn new_stream_mapping_is_stale_and_runtime_disabled() {
        let mut next = vec![json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "127.0.0.1:5900", "use_auth": true,
            "disabled": false, "probe_status": "verified", "validation_mode": "strict",
            "service_profile": {"service_id": "rfb"},
            "bypass_policy": {"enabled": true, "policy_version": "injected"},
        })];
        prepare_stream_mapping_update(&[], &mut next);
        assert_eq!(next[0]["probe_status"], "stale");
        assert_eq!(next[0]["disabled"], true);
        assert_eq!(next[0]["validation_mode"], "off");
        assert!(next[0].get("service_profile").is_none());
        assert!(next[0].get("bypass_policy").is_none());
    }

    #[test]
    fn ordinary_update_preserves_compiled_security_state() {
        let previous = vec![json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "camera:5900", "use_auth": true,
            "disabled": false, "probe_status": "verified", "validation_mode": "strict",
            "service_profile": {"service_id": "rfb"},
            "bypass_policy": {"enabled": true, "policy_version": "v1", "groups": []},
        })];
        let mut next = vec![json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "camera:5900", "use_auth": true, "comment": "changed",
            "bypass_policy": {"enabled": true, "policy_version": "injected"},
        })];
        prepare_stream_mapping_update(&previous, &mut next);
        assert_eq!(next[0]["service_profile"]["service_id"], "rfb");
        assert_eq!(next[0]["bypass_policy"]["policy_version"], "v1");
        assert_eq!(next[0]["probe_status"], "verified");
    }

    #[test]
    fn ordinary_update_cannot_add_security_state_to_legacy_mapping() {
        let previous = vec![json!({
            "protocol": "tcp", "listen_port": 22,
            "target": "server:22", "use_auth": true,
        })];
        let mut next = vec![json!({
            "protocol": "tcp", "listen_port": 22,
            "target": "server:22", "use_auth": true,
            "service_profile": {"service_id": "ssh"},
            "bypass_policy": {"enabled": true, "policy_version": "injected"},
        })];
        prepare_stream_mapping_update(&previous, &mut next);
        assert!(next[0].get("service_profile").is_none());
        assert!(next[0].get("bypass_policy").is_none());
    }

    #[test]
    fn target_change_invalidates_profile_and_pauses_bypass() {
        let previous = vec![json!({
            "protocol": "udp", "listen_port": 5060,
            "target": "old:5060", "use_auth": true,
            "disabled": false, "probe_status": "verified", "validation_mode": "strict",
            "service_profile": {"service_id": "sip"},
            "bypass_policy": {"enabled": true, "policy_version": "v1", "groups": []},
        })];
        let mut next = vec![json!({
            "protocol": "udp", "listen_port": 5060,
            "target": "new:5060", "use_auth": true,
        })];
        prepare_stream_mapping_update(&previous, &mut next);
        assert_eq!(next[0]["probe_status"], "stale");
        assert_eq!(next[0]["disabled"], true);
        assert_eq!(next[0]["bypass_policy"]["enabled"], false);
        assert!(next[0].get("service_profile").is_none());
    }

    #[test]
    fn disabling_auth_pauses_bypass_without_dropping_its_draft() {
        let previous = vec![json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "camera:5900", "use_auth": true,
            "bypass_policy": {
                "enabled": true, "policy_version": "v1",
                "groups": [{"id": "trusted", "conditions": []}],
            },
        })];
        let mut next = vec![json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "camera:5900", "use_auth": false,
        })];
        prepare_stream_mapping_update(&previous, &mut next);
        assert_eq!(next[0]["bypass_policy"]["enabled"], false);
        assert_eq!(next[0]["bypass_policy"]["policy_version"], "v1");
        assert_eq!(next[0]["bypass_policy"]["groups"][0]["id"], "trusted");
    }

    #[test]
    fn clearing_manual_service_profile_disables_mapping_but_preserves_login_policy() {
        let mut mapping = json!({
            "protocol": "tcp", "listen_port": 5900,
            "target": "camera:5900", "use_auth": true,
            "disabled": false, "probe_status": "manual", "validation_mode": "strict",
            "service_profile": {"service_id": "rfb", "source": "manual"},
            "bypass_policy": {"enabled": true, "policy_version": "v1", "groups": []},
        });
        clear_stream_service_profile_fields(mapping.as_object_mut().unwrap());
        assert!(mapping.get("service_profile").is_none());
        assert_eq!(mapping["probe_status"], "stale");
        assert_eq!(mapping["validation_mode"], "off");
        assert_eq!(mapping["disabled"], true);
        assert_eq!(mapping["use_auth"], true);
        assert_eq!(mapping["bypass_policy"]["policy_version"], "v1");
    }

    #[test]
    fn service_profile_mutations_reject_stale_mapping_snapshots() {
        let mapping = json!({
            "target": "camera:5900",
            "service_profile": {"service_id": "rfb", "source": "manual"},
        });
        assert!(service_profile_precondition_matches(
            &mapping,
            "camera:5900",
            "rfb"
        ));
        assert!(!service_profile_precondition_matches(
            &mapping,
            "new-camera:5900",
            "rfb"
        ));
        assert!(!service_profile_precondition_matches(
            &mapping,
            "camera:5900",
            "ssh"
        ));
    }

    #[test]
    fn bypass_policy_mutations_reject_stale_mapping_snapshots() {
        let mapping = json!({"target": "camera:5900", "use_auth": true});
        assert!(bypass_mapping_precondition_matches(
            &mapping,
            "camera:5900",
            true
        ));
        assert!(!bypass_mapping_precondition_matches(
            &mapping,
            "new-camera:5900",
            true
        ));
        assert!(!bypass_mapping_precondition_matches(
            &mapping,
            "camera:5900",
            false
        ));
    }

    #[test]
    fn pruning_keeps_only_unique_referenced_stream_ipsets() {
        let mut config = json!({
            "stream_mappings": [{
                "bypass_policy": {"groups": [{"conditions": [
                    {"policy_id": "keep"}, {"policy_id": "keep"}
                ]}]}
            }],
            "stream_access_policies": [
                {"id": "keep"}, {"id": "drop"}, {"id": "keep"}, {"bad": true}
            ],
        });
        prune_stream_access_policies(&mut config);
        assert_eq!(config["stream_access_policies"], json!([{"id": "keep"}]));
    }

    #[test]
    fn half_or_more_of_an_address_family_requires_broad_confirmation() {
        let half = crate::cidr::compile_ip_set(["0.0.0.0/1"]).unwrap();
        let narrow = crate::cidr::compile_ip_set(["10.0.0.0/8"]).unwrap();
        assert!(is_broad_stream_policy(&half));
        assert!(!is_broad_stream_policy(&narrow));
    }

    #[test]
    fn source_ip_operators_reject_mismatched_address_kinds() {
        let exact_with_cidr = json!({"values": ["192.0.2.0/24"]});
        assert!(
            source_ip_cidrs(exact_with_cidr.as_object().unwrap(), "equals")
                .unwrap_err()
                .contains("requires IP addresses")
        );
        let range_with_address = json!({"values": ["192.0.2.10"]});
        assert!(
            source_ip_cidrs(range_with_address.as_object().unwrap(), "in_cidr")
                .unwrap_err()
                .contains("requires CIDR ranges")
        );
    }

    #[test]
    fn disabling_bypass_preserves_only_the_last_saved_draft() {
        let mapping = json!({
            "bypass_policy": {
                "enabled": true,
                "policy_version": "old-version",
                "broad_rule_confirmed": true,
                "groups": [{"id": "saved", "conditions": [{"id": "condition"}]}],
            }
        });
        let disabled = disabled_stream_bypass_policy(&mapping);
        assert_eq!(disabled["enabled"], false);
        assert_eq!(disabled["broad_rule_confirmed"], false);
        assert_eq!(disabled["groups"][0]["id"], "saved");
        assert_ne!(disabled["policy_version"], "old-version");
    }

    #[test]
    fn persisted_policy_draft_drops_uncontrolled_condition_fields() {
        let ip = json!({
            "values": [" 192.0.2.10 ", ""],
            "selections": [{"province": "ignored"}],
            "injected": {"secret": true},
        });
        assert_eq!(
            persisted_stream_values(ip.as_object().unwrap(), "source_ip"),
            vec![json!("192.0.2.10")]
        );
        assert!(persisted_stream_selections(ip.as_object().unwrap(), "source_ip").is_empty());

        let region = json!({
            "values": ["ignored"],
            "selections": [{
                "province": " 广东 ", "city": " 深圳 ", "query_city": " 深圳 ",
                "operator": "电信", "cidrs": ["0.0.0.0/0"], "injected": true,
            }],
        });
        assert!(persisted_stream_values(region.as_object().unwrap(), "source_region").is_empty());
        assert_eq!(
            persisted_stream_selections(region.as_object().unwrap(), "source_region"),
            vec![json!({
                "province": "广东", "city": "深圳", "query_city": "深圳", "operator": "电信"
            })]
        );
    }
}
