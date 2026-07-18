use super::*;
use crate::cidr::{self, CidrOperator, CidrRegionQuery};
use ipnet::IpNet;
use regex::Regex;

const DEFAULT_IDLE_TTL_SECONDS: i64 = 24 * 60 * 60;
const DEFAULT_MAX_LIFETIME_SECONDS: i64 = 30 * 24 * 60 * 60;
const MIN_TTL_SECONDS: i64 = 5 * 60;
const MAX_IDLE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const MAX_LIFETIME_SECONDS: i64 = 365 * 24 * 60 * 60;
const MAX_GROUPS: usize = 16;
const MAX_CONDITIONS: usize = 16;
const MAX_VALUES_PER_CONDITION: usize = 256;
const MAX_TOTAL_VALUES: usize = 4_096;
const MAX_TOTAL_REGEXES: usize = 256;
const MAX_REGEX_BYTES: usize = 512;
const MAX_RESOLVED_CIDRS: usize = 100_000;
const MAX_CONFIG_BYTES: usize = 8 * 1024 * 1024;

fn default_advanced_auth() -> Value {
    json!({
        "enabled": false,
        "idle_ttl_seconds": DEFAULT_IDLE_TTL_SECONDS,
        "max_lifetime_seconds": DEFAULT_MAX_LIFETIME_SECONDS,
        "policy_version": uuid::Uuid::new_v4().to_string(),
        "groups": [],
    })
}

pub(super) async fn get_advanced_auth(
    State(state): State<AppState>,
    Path(host): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let host = normalize_host_value(&host);
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load advanced authentication configuration");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(mapping) = ordinary_host_mapping(&mappings, &host) else {
        return response::error(StatusCode::NOT_FOUND, "Subdomain does not exist");
    };
    response::ok(json!({
        "host": host,
        "revision": host_mappings_revision(&mappings),
        "advanced_auth": mapping.get("advanced_auth").cloned().unwrap_or_else(default_advanced_auth),
    }))
    .into_response()
}

pub(super) async fn update_advanced_auth(
    State(state): State<AppState>,
    Path(host): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let host = normalize_host_value(&host);
    let revision = body
        .get("revision")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let acknowledge_broad_rules = body
        .get("acknowledge_broad_rules")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let requested = body.get("advanced_auth").unwrap_or(&body);
    if serde_json::to_vec(requested).is_ok_and(|encoded| encoded.len() > MAX_CONFIG_BYTES) {
        return response::error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Advanced authentication configuration exceeds 8 MiB",
        );
    }

    let _update_guard = state.host_mappings_update_lock.lock().await;
    let transaction_lease = match acquire_host_mappings_transaction_lease(&state).await {
        Ok(Some(lease)) => lease,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to acquire advanced authentication transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    };
    let previous_config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load advanced authentication configuration");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if revision
        .as_deref()
        .is_some_and(|revision| revision != host_mappings_revision(&previous_mappings))
    {
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }
    let Some(index) = ordinary_host_mapping_index(&previous_mappings, &host) else {
        return response::error(StatusCode::NOT_FOUND, "Subdomain does not exist");
    };
    if !previous_mappings[index]
        .get("use_auth")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Enable authentication for this subdomain before advanced authentication",
        );
    }
    let requested_enabled = requested
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let previously_enabled = previous_mappings[index]
        .pointer("/advanced_auth/enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // Emergency deactivation must not depend on the CIDR data service. Keep
    // the last successfully compiled draft intact and only rotate the policy
    // version plus locally validated TTLs. A later edit while already disabled
    // still follows the normal compile path.
    let advanced_auth = if previously_enabled && !requested_enabled {
        match disabled_advanced_auth(&previous_mappings[index], requested) {
            Ok(config) => config,
            Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
        }
    } else {
        match compile_advanced_auth(&state, requested, acknowledge_broad_rules).await {
            Ok(config) => config,
            Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
        }
    };
    if advanced_auth
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        match state.go_backend.verify_bundle_compatibility().await {
            Ok(info)
                if info
                    .pointer("/data/capabilities")
                    .and_then(Value::as_array)
                    .is_some_and(|capabilities| {
                        capabilities
                            .iter()
                            .any(|value| value.as_str() == Some("subdomain_rule_grant_v1"))
                    }) => {}
            Ok(_) => {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    "Go gateway does not support subdomain_rule_grant_v1; upgrade the gateway before enabling advanced authentication",
                );
            }
            Err(error) => {
                tracing::warn!(%error, "advanced authentication gateway capability check failed");
                return response::error(StatusCode::BAD_GATEWAY, error.to_string());
            }
        }
    }
    // The request-size guard above only covers user input.  Region expansion
    // adds immutable CIDR arrays and selection metadata, so enforce the same
    // cap on the compiled representation before it can enter persistent
    // config or the gateway payload.
    if serde_json::to_vec(&advanced_auth)
        .map(|encoded| encoded.len() > MAX_CONFIG_BYTES)
        .unwrap_or(true)
    {
        return response::error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Compiled advanced authentication configuration exceeds 8 MiB",
        );
    }
    let mut next_mappings = previous_mappings.clone();
    let Some(mapping) = next_mappings[index].as_object_mut() else {
        return response::error(StatusCode::BAD_REQUEST, "Invalid host mapping");
    };
    mapping.insert("advanced_auth".to_string(), advanced_auth.clone());

    match transaction_lease.ensure_valid().await {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to refresh advanced authentication transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    }
    match state
        .store
        .compare_and_set_host_mappings(&previous_mappings, &next_mappings)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save advanced authentication configuration");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    }
    if let Err(error) = transaction_lease.ensure_owned().await {
        tracing::warn!(%error, "advanced authentication transaction lease was lost");
        let _ = state
            .store
            .compare_and_set_host_mappings(&next_mappings, &previous_mappings)
            .await;
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }
    if let Err(message) = sync_host_mappings_runtime(&state, &previous_config, &next_mappings).await
    {
        rollback_host_mappings(&state, &previous_config, &next_mappings).await;
        tracing::warn!(%message, "failed to sync advanced authentication configuration");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.hostMappings.syncHostRulesFailed",
            ),
        );
    }
    if let Err(error) = transaction_lease.release().await {
        tracing::warn!(%error, "failed to release advanced authentication transaction lease");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "hostMappings.updateFailed"),
        );
    }
    response::ok(json!({
        "host": host,
        "revision": host_mappings_revision(&next_mappings),
        "advanced_auth": advanced_auth,
    }))
    .into_response()
}

fn ordinary_host_mapping<'a>(mappings: &'a [Value], host: &str) -> Option<&'a Value> {
    ordinary_host_mapping_index(mappings, host).and_then(|index| mappings.get(index))
}

fn ordinary_host_mapping_index(mappings: &[Value], host: &str) -> Option<usize> {
    mappings.iter().position(|mapping| {
        normalize_host_value(mapping.get("host").and_then(Value::as_str).unwrap_or("")) == host
            && mapping
                .get("target")
                .and_then(Value::as_str)
                .and_then(|target| Url::parse(target.trim()).ok())
                .is_some_and(|target| matches!(target.scheme(), "http" | "https"))
            && mapping
                .get("service_role")
                .and_then(Value::as_str)
                .map(|role| role != "auth")
                .unwrap_or_else(|| {
                    !mapping
                        .get("target")
                        .and_then(Value::as_str)
                        .is_some_and(is_auth_service_target)
                })
    })
}

async fn compile_advanced_auth(
    state: &AppState,
    requested: &Value,
    acknowledge_broad_rules: bool,
) -> Result<Value, String> {
    let requested = requested
        .as_object()
        .ok_or_else(|| "Advanced authentication configuration must be an object".to_string())?;
    let enabled = requested
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let (idle_ttl_seconds, max_lifetime_seconds) = validated_ttls(requested)?;
    let groups = requested
        .get("groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if groups.len() > MAX_GROUPS {
        return Err(format!("At most {MAX_GROUPS} OR groups are allowed"));
    }
    if enabled && groups.is_empty() {
        return Err("At least one rule group is required when enabled".to_string());
    }

    let needs_region_source = groups.iter().any(|group| {
        group
            .get("conditions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|condition| {
                condition.get("target").and_then(Value::as_str) == Some("source_region")
            })
    });
    let (source, source_fingerprint) = if needs_region_source {
        let (source, base_url) = cidr::configured_cidr_source(state).await?;
        (source, cidr::source_fingerprint(&base_url))
    } else {
        (String::new(), String::new())
    };
    let resolved_at = crate::time_utils::now_iso();
    let mut compiled_groups = Vec::with_capacity(groups.len());
    let mut resolved_cidr_count = 0usize;
    let mut total_value_count = 0usize;
    let mut total_regex_count = 0usize;
    let mut seen_groups = HashSet::new();
    for (group_index, group) in groups.iter().enumerate() {
        let group = group
            .as_object()
            .ok_or_else(|| format!("Rule group {} must be an object", group_index + 1))?;
        let group_id = required_id(group.get("id"), "rule group")?;
        if !seen_groups.insert(group_id.clone()) {
            return Err(format!("Duplicate rule group id {group_id}"));
        }
        let conditions = group
            .get("conditions")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("Rule group {group_id} conditions must be an array"))?;
        if conditions.is_empty() {
            return Err(format!("Rule group {group_id} cannot be empty"));
        }
        if conditions.len() > MAX_CONDITIONS {
            return Err(format!(
                "Rule group {group_id} supports at most {MAX_CONDITIONS} conditions"
            ));
        }
        let mut compiled_conditions = Vec::with_capacity(conditions.len());
        let mut seen_conditions = HashSet::new();
        for condition in conditions {
            let condition = condition
                .as_object()
                .ok_or_else(|| format!("Rule group {group_id} contains an invalid condition"))?;
            let condition_id = required_id(condition.get("id"), "condition")?;
            if !seen_conditions.insert(condition_id.clone()) {
                return Err(format!(
                    "Duplicate condition id {condition_id} in group {group_id}"
                ));
            }
            let condition_values = condition_match_value_count(condition);
            if condition_values > MAX_VALUES_PER_CONDITION {
                return Err(format!(
                    "Condition {condition_id} supports at most {MAX_VALUES_PER_CONDITION} match values"
                ));
            }
            total_value_count = total_value_count.saturating_add(condition_values);
            if total_value_count > MAX_TOTAL_VALUES {
                return Err(format!(
                    "Advanced authentication supports at most {MAX_TOTAL_VALUES} match values per subdomain"
                ));
            }
            if matches!(
                condition.get("operator").and_then(Value::as_str),
                Some("regex" | "not_regex")
            ) {
                total_regex_count = total_regex_count.saturating_add(condition_values);
                if total_regex_count > MAX_TOTAL_REGEXES {
                    return Err(format!(
                        "Advanced authentication supports at most {MAX_TOTAL_REGEXES} regular expressions per subdomain"
                    ));
                }
            }
            let compiled = compile_condition(
                state,
                condition,
                &condition_id,
                &source,
                &source_fingerprint,
                &resolved_at,
            )
            .await?;
            resolved_cidr_count += compiled
                .get("cidrs")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            if resolved_cidr_count > MAX_RESOLVED_CIDRS {
                return Err(format!(
                    "Resolved CIDRs exceed the per-subdomain limit of {MAX_RESOLVED_CIDRS}"
                ));
            }
            compiled_conditions.push(compiled);
        }
        // Broad-rule confirmation is an activation guard.  Disabling a policy
        // must remain possible even when its retained draft contains a broad
        // group, otherwise an administrator could be locked out of the page
        // merely by losing the one-time acknowledgement flag.
        if enabled && is_broad_group(&compiled_conditions) && !acknowledge_broad_rules {
            return Err(format!(
                "Rule group {group_id} is broad and requires explicit risk confirmation"
            ));
        }
        compiled_groups.push(json!({
            "id": group_id,
            "conditions": compiled_conditions,
        }));
    }
    Ok(json!({
        "enabled": enabled,
        "idle_ttl_seconds": idle_ttl_seconds,
        "max_lifetime_seconds": max_lifetime_seconds,
        "policy_version": uuid::Uuid::new_v4().to_string(),
        "groups": compiled_groups,
        "compiled_at": resolved_at,
        "cidr_source": source,
        "cidr_source_fingerprint": source_fingerprint,
    }))
}

fn validated_ttls(requested: &Map<String, Value>) -> Result<(i64, i64), String> {
    let idle_ttl_seconds = requested
        .get("idle_ttl_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(DEFAULT_IDLE_TTL_SECONDS);
    let max_lifetime_seconds = requested
        .get("max_lifetime_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(DEFAULT_MAX_LIFETIME_SECONDS);
    if !(MIN_TTL_SECONDS..=MAX_IDLE_TTL_SECONDS).contains(&idle_ttl_seconds) {
        return Err("Idle validity must be between 5 minutes and 30 days".to_string());
    }
    if !(MIN_TTL_SECONDS..=MAX_LIFETIME_SECONDS).contains(&max_lifetime_seconds) {
        return Err("Maximum lifetime must be between 5 minutes and 365 days".to_string());
    }
    if max_lifetime_seconds < idle_ttl_seconds {
        return Err("Maximum lifetime cannot be shorter than idle validity".to_string());
    }
    Ok((idle_ttl_seconds, max_lifetime_seconds))
}

fn disabled_advanced_auth(mapping: &Value, requested: &Value) -> Result<Value, String> {
    let requested = requested
        .as_object()
        .ok_or_else(|| "Advanced authentication configuration must be an object".to_string())?;
    let (idle_ttl_seconds, max_lifetime_seconds) = validated_ttls(requested)?;
    let mut retained = mapping
        .get("advanced_auth")
        .cloned()
        .unwrap_or_else(default_advanced_auth);
    let object = retained
        .as_object_mut()
        .ok_or_else(|| "Stored advanced authentication configuration is invalid".to_string())?;
    object.insert("enabled".to_string(), Value::Bool(false));
    object.insert(
        "idle_ttl_seconds".to_string(),
        Value::Number(idle_ttl_seconds.into()),
    );
    object.insert(
        "max_lifetime_seconds".to_string(),
        Value::Number(max_lifetime_seconds.into()),
    );
    object.insert(
        "policy_version".to_string(),
        Value::String(uuid::Uuid::new_v4().to_string()),
    );
    Ok(retained)
}

async fn compile_condition(
    state: &AppState,
    condition: &Map<String, Value>,
    id: &str,
    cidr_source: &str,
    cidr_source_fingerprint: &str,
    resolved_at: &str,
) -> Result<Value, String> {
    let target = text_field(condition, "target").to_ascii_lowercase();
    let operator = text_field(condition, "operator").to_ascii_lowercase();
    let name = text_field(condition, "name");
    let values = string_values(condition.get("values"))?;
    match target.as_str() {
        "source_ip" => {
            if !["equals", "not_equals", "in_cidr", "not_in_cidr"].contains(&operator.as_str()) {
                return Err(format!("Unsupported source IP operator {operator}"));
            }
            if values.is_empty() {
                return Err("Source IP condition requires an IP or CIDR".to_string());
            }
            let cidrs =
                compile_source_networks(&values, operator == "equals" || operator == "not_equals")?;
            Ok(
                json!({"id": id, "target": target, "operator": operator, "name": "", "values": [], "cidrs": cidrs}),
            )
        }
        "source_region" => {
            if operator != "in" && operator != "not_in" {
                return Err(format!("Unsupported source region operator {operator}"));
            }
            let selections = condition
                .get("selections")
                .and_then(Value::as_array)
                .ok_or_else(|| "Source region condition requires region selections".to_string())?;
            if selections.is_empty() {
                return Err("Source region condition requires at least one region".to_string());
            }
            if selections.len() > MAX_VALUES_PER_CONDITION {
                return Err(format!(
                    "Source region condition supports at most {MAX_VALUES_PER_CONDITION} selections"
                ));
            }
            let mut cidrs = Vec::new();
            let mut compiled_selections = Vec::with_capacity(selections.len());
            for selection in selections {
                let selection = selection
                    .as_object()
                    .ok_or_else(|| "Region selection must be an object".to_string())?;
                let province = selection
                    .get("province")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "Region province cannot be empty".to_string())?;
                let query_city = selection
                    .get("query_city")
                    .or_else(|| selection.get("queryCity"))
                    .or_else(|| selection.get("city"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let operator_filter = CidrOperator::parse_value(selection.get("operator"))?;
                let query = CidrRegionQuery::new(province, query_city, operator_filter);
                let lookup = cidr::lookup_region(state, &query)
                    .await
                    .map_err(|error| error.to_string())?;
                if lookup.cidrs.is_empty() {
                    return Err(format!(
                        "Region selection {} resolved to no CIDRs",
                        query.key()
                    ));
                }
                cidrs.extend(lookup.cidrs);
                compiled_selections.push(
                    serde_json::to_value(lookup.selection).map_err(|error| error.to_string())?,
                );
            }
            cidrs.sort();
            cidrs.dedup();
            Ok(json!({
                "id": id, "target": target, "operator": operator, "name": "", "values": [],
                "selections": compiled_selections, "cidrs": cidrs, "resolved_at": resolved_at,
                "cidr_source": cidr_source, "cidr_source_fingerprint": cidr_source_fingerprint,
            }))
        }
        "url_path" => {
            validate_text_operator(&operator, true, &values)?;
            Ok(
                json!({"id": id, "target": target, "operator": operator, "name": "", "values": values, "cidrs": []}),
            )
        }
        "request_header" => {
            if !valid_header_name(&name) {
                return Err("Header name is invalid or protected".to_string());
            }
            validate_text_operator(&operator, false, &values)?;
            // Go's http.CanonicalHeaderKey is applied while compiling the
            // immutable gateway snapshot.  Canonicalize here as well so the
            // control-plane payload and the gateway echo compare equal during
            // the CAS/runtime transaction (header matching remains
            // case-insensitive at request time).
            let name = canonical_header_name(&name);
            Ok(
                json!({"id": id, "target": target, "operator": operator, "name": name, "values": values, "cidrs": []}),
            )
        }
        "query_parameter" => {
            if name.is_empty() {
                return Err("Query parameter name cannot be empty".to_string());
            }
            validate_text_operator(&operator, false, &values)?;
            Ok(
                json!({"id": id, "target": target, "operator": operator, "name": name, "values": values, "cidrs": []}),
            )
        }
        "http_method" => {
            if operator != "in" && operator != "not_in" {
                return Err(format!("Unsupported HTTP Method operator {operator}"));
            }
            let mut seen = HashSet::new();
            let methods = values
                .into_iter()
                .map(|value| value.trim().to_ascii_uppercase())
                .filter(|value| seen.insert(value.clone()))
                .collect::<Vec<_>>();
            if methods.is_empty() || methods.iter().any(String::is_empty) {
                return Err("HTTP Method condition requires at least one method".to_string());
            }
            Ok(
                json!({"id": id, "target": target, "operator": operator, "name": "", "values": methods, "cidrs": []}),
            )
        }
        _ => Err(format!(
            "Unsupported advanced authentication target {target}"
        )),
    }
}

fn text_field(object: &Map<String, Value>, field: &str) -> String {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn required_id(value: Option<&Value>, label: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("{label} id is required"))
}

fn string_values(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| "Condition values must be an array".to_string())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| "Condition values must be strings".to_string())
        })
        .collect()
}

fn condition_match_value_count(condition: &Map<String, Value>) -> usize {
    let field = if condition
        .get("target")
        .and_then(Value::as_str)
        .is_some_and(|target| target.trim().eq_ignore_ascii_case("source_region"))
    {
        "selections"
    } else {
        "values"
    };
    condition
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn canonical_network(value: &str, require_address: bool) -> Result<String, String> {
    if require_address {
        let address = value
            .trim()
            .parse::<std::net::IpAddr>()
            .map_err(|_| "must be a valid IPv4 or IPv6 address".to_string())?;
        return Ok(match address {
            std::net::IpAddr::V4(address) => format!("{address}/32"),
            std::net::IpAddr::V6(address) => address
                .to_ipv4()
                .map(|address| format!("{address}/32"))
                .unwrap_or_else(|| format!("{address}/128")),
        });
    }
    value
        .trim()
        .parse::<IpNet>()
        .map(|network| network.trunc().to_string())
        .map_err(|_| "must be a valid IPv4 or IPv6 CIDR".to_string())
}

fn compile_source_networks(
    values: &[String],
    require_address: bool,
) -> Result<Vec<String>, String> {
    let mut compiled = Vec::with_capacity(values.len());
    let mut seen = HashSet::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let network = canonical_network(value, require_address)
            .map_err(|message| format!("Source IP entry {} {message}", index + 1))?;
        if seen.insert(network.clone()) {
            compiled.push(network);
        }
    }
    Ok(compiled)
}

fn validate_text_operator(operator: &str, path: bool, values: &[String]) -> Result<(), String> {
    let allowed = [
        "equals",
        "not_equals",
        "contains",
        "not_contains",
        "starts_with",
        "not_starts_with",
        "ends_with",
        "not_ends_with",
        "regex",
        "not_regex",
    ];
    let supported = allowed.contains(&operator)
        || (path && matches!(operator, "prefix" | "not_prefix"))
        || (!path && matches!(operator, "exists" | "not_exists"));
    if !supported {
        return Err(format!("Unsupported text operator {operator}"));
    }
    if !matches!(operator, "exists" | "not_exists") && values.is_empty() {
        return Err(format!("Operator {operator} requires a match value"));
    }
    if !matches!(operator, "exists" | "not_exists") && values.iter().any(String::is_empty) {
        return Err(format!(
            "Operator {operator} does not allow empty match values"
        ));
    }
    if matches!(operator, "regex" | "not_regex") {
        for expression in values {
            if expression.len() > MAX_REGEX_BYTES {
                return Err(format!(
                    "Regular expression exceeds {MAX_REGEX_BYTES} bytes"
                ));
            }
            Regex::new(&format!("^(?:{expression})$"))
                .map_err(|_| "Invalid RE2-compatible regular expression".to_string())?;
        }
    }
    Ok(())
}

fn valid_header_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.starts_with("x-reauth-") || is_protected_header(&lower) {
        return false;
    }
    lower
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

/// Headers whose values are either credentials, hop-by-hop state, or a
/// client-address assertion supplied by a trusted proxy.  They must never be
/// user-matchable: a request can otherwise manufacture a value that defeats
/// the trust boundary used for source-IP/region rules.
fn is_protected_header(lower: &str) -> bool {
    lower.starts_with("x-forwarded-")
        || lower == "forwarded"
        || lower == "x-real-ip"
        || lower == "ali-real-client-ip"
        || lower == "eo-connecting-ip"
        || lower == "cf-connecting-ip"
        || lower == "true-client-ip"
        || lower == "fastly-client-ip"
        || lower == "client-ip"
        || lower.ends_with("-client-ip")
        || matches!(
            lower,
            "host"
                | "cookie"
                | "authorization"
                | "proxy-authorization"
                | "connection"
                | "keep-alive"
                | "proxy-authenticate"
                | "proxy-connection"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
        )
}

fn canonical_header_name(name: &str) -> String {
    let mut canonical = String::with_capacity(name.len());
    let mut capitalize = true;
    for byte in name.trim().bytes() {
        if byte == b'-' {
            capitalize = true;
            canonical.push('-');
        } else if capitalize {
            canonical.push(byte.to_ascii_uppercase() as char);
            capitalize = false;
        } else {
            canonical.push(byte.to_ascii_lowercase() as char);
        }
    }
    canonical
}

fn is_broad_group(conditions: &[Value]) -> bool {
    let all_negative = conditions.iter().all(|condition| {
        condition
            .get("operator")
            .and_then(Value::as_str)
            .is_some_and(|operator| operator.starts_with("not_"))
    });
    let method_only = conditions.len() == 1
        && conditions[0].get("target").and_then(Value::as_str) == Some("http_method");
    let root_prefix = conditions.iter().any(|condition| {
        condition.get("target").and_then(Value::as_str) == Some("url_path")
            && matches!(
                condition.get("operator").and_then(Value::as_str),
                Some("prefix" | "starts_with")
            )
            && condition
                .get("values")
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("/")))
    });
    let all_networks = conditions.iter().any(|condition| {
        condition
            .get("cidrs")
            .and_then(Value::as_array)
            .is_some_and(|cidrs| {
                cidrs
                    .iter()
                    .any(|cidr| matches!(cidr.as_str(), Some("0.0.0.0/0" | "::/0")))
            })
    });
    all_negative || method_only || root_prefix || all_networks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_disable_retains_compiled_draft_and_rotates_version() {
        let mapping = json!({
            "advanced_auth": {
                "enabled": true,
                "idle_ttl_seconds": 86400,
                "max_lifetime_seconds": 2592000,
                "policy_version": "old-version",
                "groups": [{
                    "id": "region-group",
                    "conditions": [{
                        "id": "region",
                        "target": "source_region",
                        "operator": "in",
                        "cidrs": ["192.0.2.0/24"],
                        "selections": [{"province": "test"}]
                    }]
                }],
                "cidr_source_fingerprint": "fingerprint"
            }
        });
        let disabled = disabled_advanced_auth(
            &mapping,
            &json!({
                "enabled": false,
                "idle_ttl_seconds": 3600,
                "max_lifetime_seconds": 7200
            }),
        )
        .expect("disable policy");
        assert_eq!(disabled["enabled"], false);
        assert_eq!(disabled["idle_ttl_seconds"], 3600);
        assert_eq!(disabled["max_lifetime_seconds"], 7200);
        assert_ne!(disabled["policy_version"], "old-version");
        assert_eq!(
            disabled.pointer("/groups/0/conditions/0/cidrs/0"),
            Some(&json!("192.0.2.0/24"))
        );
        assert_eq!(disabled["cidr_source_fingerprint"], "fingerprint");
    }

    #[test]
    fn ttl_validation_rejects_a_hard_limit_shorter_than_idle() {
        let requested = json!({
            "idle_ttl_seconds": 7200,
            "max_lifetime_seconds": 3600
        });
        assert!(validated_ttls(requested.as_object().unwrap()).is_err());
    }

    #[test]
    fn source_region_selections_count_toward_the_policy_value_limit() {
        let condition = json!({
            "target": " SOURCE_REGION ",
            "values": [],
            "selections": [
                {"province": "Beijing"},
                {"province": "Shanghai"}
            ]
        });
        assert_eq!(
            condition_match_value_count(condition.as_object().unwrap()),
            2
        );
    }

    #[test]
    fn text_operators_reject_empty_values_even_when_another_value_is_present() {
        let values = vec!["/admin".to_string(), String::new()];
        assert!(validate_text_operator("prefix", true, &values).is_err());
    }

    #[test]
    fn source_ip_compilation_supports_multiple_ipv4_and_ipv6_addresses() {
        let compiled = compile_source_networks(
            &[
                "192.0.2.10".to_string(),
                "2001:0db8::10".to_string(),
                "2001:db8::10".to_string(),
            ],
            true,
        )
        .expect("compile exact source addresses");
        assert_eq!(compiled, vec!["192.0.2.10/32", "2001:db8::10/128"]);
    }

    #[test]
    fn source_cidr_compilation_supports_multiple_ipv4_and_ipv6_networks() {
        let compiled = compile_source_networks(
            &[
                "192.0.2.99/24".to_string(),
                "2001:db8:abcd::1234/48".to_string(),
            ],
            false,
        )
        .expect("compile source CIDRs");
        assert_eq!(compiled, vec!["192.0.2.0/24", "2001:db8:abcd::/48"]);
    }

    #[test]
    fn source_network_errors_identify_the_line_without_echoing_its_value() {
        let secret_invalid_value = "not-an-ip-secret";
        let error = compile_source_networks(
            &["192.0.2.10".to_string(), secret_invalid_value.to_string()],
            true,
        )
        .expect_err("reject invalid second address");
        assert!(error.contains("entry 2"));
        assert!(!error.contains(secret_invalid_value));

        assert!(compile_source_networks(&["192.0.2.10".to_string()], false).is_err());
        assert!(compile_source_networks(&["2001:db8::/32".to_string()], true).is_err());
    }
}
