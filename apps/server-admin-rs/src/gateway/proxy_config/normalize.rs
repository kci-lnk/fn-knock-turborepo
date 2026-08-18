use super::*;
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use get_if_addrs::{IfAddr, get_if_addrs};

pub(super) fn ensure_go_success(value: Value) -> Result<(), String> {
    crate::go_backend::ensure_response_success(
        &value,
        "Go backend returned an unsuccessful response",
    )
}

pub(super) async fn rollback_proxy_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.storage.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback proxy mappings config");
        return;
    }
    let previous_rules = previous_config
        .get("proxy_mappings")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    if let Err(error) = sync_go_rules(state, &previous_rules).await {
        tracing::warn!(%error, "failed to rollback proxy mappings runtime");
    }
}

pub(super) async fn rollback_host_mappings(
    state: &AppState,
    previous_config: &Value,
    expected_current_mappings: &[Value],
) {
    let attempted_mappings = expected_current_mappings.to_vec();
    rollback_host_mappings_with_runtime_sync(
        state,
        previous_config,
        expected_current_mappings,
        move |state, config, mappings| async move {
            let mut attempted_config = config;
            ensure_object(&mut attempted_config).insert(
                "host_mappings".to_string(),
                Value::Array(attempted_mappings),
            );
            sync_host_mappings_runtime(&state, &attempted_config, &mappings).await
        },
    )
    .await;
}

pub(super) async fn rollback_host_mappings_with_runtime_sync<Sync, SyncFuture>(
    state: &AppState,
    previous_config: &Value,
    expected_current_mappings: &[Value],
    sync_runtime: Sync,
) where
    Sync: FnOnce(AppState, Value, Vec<Value>) -> SyncFuture,
    SyncFuture: std::future::Future<Output = Result<(), String>>,
{
    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let previous_visibility_policies = previous_config
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let restored_config = match state
        .storage
        .store
        .compare_and_set_host_mappings_with_visibility_policies(
            expected_current_mappings,
            &previous_mappings,
            &previous_visibility_policies,
        )
        .await
    {
        Ok(Some(config)) => config,
        Ok(None) => {
            tracing::warn!(
                "host mappings changed before rollback; preserving the newer configuration"
            );
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "failed to rollback host mappings config");
            return;
        }
    };
    if let Err(error) = sync_runtime(state.clone(), restored_config, previous_mappings).await {
        tracing::warn!(%error, "failed to rollback host mappings runtime");
    }
}

pub(super) async fn rollback_stream_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.storage.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings config");
        return;
    }
    if let Err(error) = sync_stream_mappings_runtime(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings runtime");
    }
}

pub(super) async fn rollback_subdomain_mode(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.storage.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback subdomain mode config");
        return;
    }
    if let Err(error) = sync_current_go_auth_config(state).await {
        tracing::warn!(%error, "failed to rollback subdomain mode runtime");
    }
}

pub(super) fn normalize_proxy_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, &'static str> {
    let mut normalized = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Proxy mapping must be an object");
        };
        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err("Proxy mapping target must be a supported HTTP/WebSocket URL");
        }
        object.insert("target".to_string(), Value::String(target));
        normalized.push(Value::Object(object));
    }
    Ok(normalized)
}

pub(super) fn normalize_host_mappings_for_route(
    mappings: Vec<Value>,
    previous_config: &Value,
) -> Result<Vec<Value>, String> {
    let groups = normalize_host_mapping_groups(host_mapping_groups_from_config(previous_config))?;
    normalize_host_mappings_for_catalog(mappings, previous_config, &groups)
}

pub(super) fn normalize_host_mappings_for_catalog(
    mappings: Vec<Value>,
    previous_config: &Value,
    groups: &[Value],
) -> Result<Vec<Value>, String> {
    let previous_by_host = previous_host_mappings_by_host(previous_config);
    let previous_by_target = previous_host_mappings_by_target(previous_config);
    let submitted_hosts = mappings
        .iter()
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_host_value)
        .filter(|host| !host.is_empty())
        .collect::<HashSet<_>>();
    let mut explicit_previous_by_host = HashMap::<String, String>::new();
    let mut explicitly_claimed_previous_hosts = HashSet::<String>::new();
    for mapping in &mappings {
        let Some(object) = mapping.as_object() else {
            continue;
        };
        let host = normalize_host_value(object.get("host").and_then(Value::as_str).unwrap_or(""));
        let Some(raw_previous_host) = object.get("previous_host") else {
            continue;
        };
        let Some(raw_previous_host) = raw_previous_host.as_str() else {
            return Err(format!("Host mapping {host} previous host is invalid"));
        };
        let previous_host = normalize_host_value(raw_previous_host);
        if previous_host.is_empty() {
            return Err(format!("Host mapping {host} previous host is invalid"));
        }
        if previous_host == host {
            continue;
        }
        if previous_by_host.contains_key(&host) {
            return Err(format!(
                "Host mapping {host} already exists and cannot be renamed from {previous_host}"
            ));
        }
        if submitted_hosts.contains(&previous_host) {
            return Err(format!(
                "Previous host mapping {previous_host} is still present"
            ));
        }
        if !previous_by_host.contains_key(&previous_host) {
            return Err(format!(
                "Previous host mapping {previous_host} does not exist"
            ));
        }
        if !explicitly_claimed_previous_hosts.insert(previous_host.clone()) {
            return Err(format!(
                "Previous host mapping {previous_host} is claimed more than once"
            ));
        }
        if explicit_previous_by_host
            .insert(host.clone(), previous_host)
            .is_some()
        {
            return Err(format!("Host mapping {host} is submitted more than once"));
        }
    }
    let mut implicit_new_target_counts = HashMap::<String, usize>::new();
    for mapping in &mappings {
        let Some(object) = mapping.as_object() else {
            continue;
        };
        let host = normalize_host_value(object.get("host").and_then(Value::as_str).unwrap_or(""));
        if previous_by_host.contains_key(&host) || explicit_previous_by_host.contains_key(&host) {
            continue;
        }
        let target = object
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if !target.is_empty() {
            *implicit_new_target_counts
                .entry(target.to_string())
                .or_default() += 1;
        }
    }
    let valid_group_ids = groups
        .iter()
        .filter_map(|group| group.get("id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut has_default_mapping = false;
    let mut auth_mapping_count = 0;
    let mut seen_hosts = HashSet::with_capacity(mappings.len());

    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Host mapping must be an object".to_string());
        };
        object.remove("previous_host");
        let raw_host = object.get("host").and_then(Value::as_str).unwrap_or("");
        if raw_host.contains('*') {
            return Err(format!(
                "Host mapping {} cannot contain wildcard",
                raw_host.trim()
            ));
        }
        let host = normalize_host_value(raw_host);
        if host.is_empty() {
            return Err("Host mapping host is required".to_string());
        }
        if !seen_hosts.insert(host.clone()) {
            return Err(format!("Duplicate host mapping {host}"));
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err(format!(
                "Host mapping {host} target must be a supported HTTP/WebSocket URL"
            ));
        }

        let service_role = if is_auth_service_target(&target) {
            "auth"
        } else {
            "app"
        };
        if service_role == "auth" {
            auth_mapping_count += 1;
            if auth_mapping_count > 1 {
                return Err("Only one auth service host mapping is allowed".to_string());
            }
            if object
                .get("use_auth")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                || object
                    .get("access_mode")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == "strict_whitelist")
            {
                return Err(format!("Auth host mapping {host} must be public"));
            }
            if host_basic_auth_enabled(object.get("basic_auth")) {
                return Err(format!("Auth host mapping {host} cannot enable Basic Auth"));
            }
        } else if host_basic_auth_invalid(object.get("basic_auth")) {
            return Err(format!(
                "Host mapping {host} Basic Auth settings are invalid"
            ));
        }
        // New clients explicitly identify a renamed mapping with previous_host.
        // Legacy clients fall back to an unchanged target only when exactly one
        // implicit new mapping and one disappeared old mapping share that target.
        // This preserves configuration for aliases that share a target without
        // copying a protected draft onto an unrelated new mapping.
        let previous = previous_by_host
            .get(&host)
            .or_else(|| {
                explicit_previous_by_host
                    .get(&host)
                    .and_then(|previous_host| previous_by_host.get(previous_host))
            })
            .or_else(|| {
                implicit_new_target_counts
                    .get(&target)
                    .is_some_and(|count| *count == 1)
                    .then(|| previous_by_target.get(&target))
                    .flatten()
                    .and_then(|candidates| {
                        let mut disappeared = candidates.iter().filter(|mapping| {
                            mapping
                                .get("host")
                                .and_then(Value::as_str)
                                .map(normalize_host_value)
                                .is_some_and(|previous_host| {
                                    !submitted_hosts.contains(&previous_host)
                                        && !explicitly_claimed_previous_hosts
                                            .contains(&previous_host)
                                })
                        });
                        let candidate = disappeared.next()?;
                        disappeared.next().is_none().then_some(candidate)
                    })
            });
        // Stable server-owned identity for external panel synchronization.
        // It survives host renames and ignores any client attempt to replace
        // the identity of an existing mapping.
        let sync_id = previous
            .and_then(|value| value.get("sync_id"))
            .and_then(Value::as_str)
            .filter(|value| uuid::Uuid::parse_str(value).is_ok())
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let target_path_mode = if service_role == "auth" {
            "entry".to_string()
        } else if object.contains_key("target_path_mode") {
            parse_explicit_target_path_mode(object.get("target_path_mode")).ok_or_else(|| {
                format!("Host mapping {host} target path mode must be entry or prefix")
            })?
        } else {
            normalize_target_path_mode(previous.and_then(|value| value.get("target_path_mode")))
        };
        let requested_group_id = object
            .contains_key("group_id")
            .then(|| object.get("group_id"))
            .flatten();
        let group_id = normalize_host_mapping_group_id(
            requested_group_id,
            previous.and_then(|value| value.get("group_id")),
            &valid_group_ids,
            service_role == "auth",
        )
        .map_err(|message| format!("Host mapping {host} {message}"))?;
        let disabled = service_role != "auth"
            && object
                .get("disabled")
                .or_else(|| previous.and_then(|value| value.get("disabled")))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let availability_source = if object.contains_key("availability") {
            object.get("availability")
        } else {
            previous.and_then(|value| value.get("availability"))
        };
        let availability = if service_role == "auth" {
            Value::Null
        } else {
            normalize_host_mapping_availability_for_route(&host, availability_source)?
        };
        let protocol_mode = if object.contains_key("protocol_mode") {
            parse_explicit_protocol_mode(object.get("protocol_mode")).ok_or_else(|| {
                format!("Host mapping {host} HTTPS protocol mode must be auto, http1 or http2")
            })?
        } else {
            normalize_protocol_mode(previous.and_then(|value| value.get("protocol_mode")))
        };
        let visibility = normalize_host_mapping_visibility(
            object.get("visibility"),
            previous.and_then(|value| value.get("visibility")),
            service_role == "auth",
        )
        .map_err(|message| format!("Host mapping {host} {message}"))?;

        let locations = if service_role == "auth" {
            Vec::new()
        } else {
            normalize_host_mapping_locations_for_route(&host, object.get("locations"))?
        };

        let can_reuse_previous_metadata = previous
            .and_then(|value| value.get("target"))
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim() == target);
        let normalized_basic_auth = if service_role == "auth" {
            disabled_host_basic_auth()
        } else {
            normalize_host_basic_auth(
                object
                    .get("basic_auth")
                    .or_else(|| previous.and_then(|value| value.get("basic_auth"))),
            )
        };
        let is_default = service_role != "auth"
            && object
                .get("is_default")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && !has_default_mapping;
        if is_default {
            has_default_mapping = true;
        }

        object.insert("host".to_string(), Value::String(host.clone()));
        object.insert("sync_id".to_string(), Value::String(sync_id));
        object.insert("target".to_string(), Value::String(target));
        object.insert(
            "target_path_mode".to_string(),
            Value::String(target_path_mode),
        );
        object.insert(
            "waf_enabled".to_string(),
            Value::Bool(
                service_role == "auth"
                    || object
                        .get("waf_enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
            ),
        );
        object.insert(
            "use_auth".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("use_auth")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
            ),
        );
        object.insert(
            "access_mode".to_string(),
            Value::String(if service_role == "auth" {
                "login_first".to_string()
            } else {
                normalize_access_mode(object.get("access_mode"))
            }),
        );
        object.insert(
            "suppress_toolbar".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("suppress_toolbar")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
            ),
        );
        object.insert(
            "preserve_host".to_string(),
            Value::Bool(
                object
                    .get("preserve_host")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
        );
        object.insert("is_default".to_string(), Value::Bool(is_default));
        object.insert("group_id".to_string(), group_id);
        object.insert("disabled".to_string(), Value::Bool(disabled));
        object.insert("availability".to_string(), availability);
        object.insert("visibility".to_string(), visibility);
        object.insert("protocol_mode".to_string(), Value::String(protocol_mode));
        object.insert("basic_auth".to_string(), normalized_basic_auth);
        object.insert("locations".to_string(), Value::Array(locations));
        let mut advanced_auth = previous
            .and_then(|value| value.get("advanced_auth"))
            .cloned()
            .unwrap_or_else(|| {
                json!({
                    "enabled": false,
                    "idle_ttl_seconds": 86_400,
                    "max_lifetime_seconds": 2_592_000,
                    "policy_version": uuid::Uuid::new_v4().to_string(),
                    "groups": [],
                })
            });
        if service_role == "auth" {
            advanced_auth = json!({
                "enabled": false,
                "idle_ttl_seconds": 86_400,
                "max_lifetime_seconds": 2_592_000,
                "policy_version": uuid::Uuid::new_v4().to_string(),
                "groups": [],
            });
        } else {
            let should_disable_advanced_auth = !object
                .get("use_auth")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                && advanced_auth
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            if should_disable_advanced_auth && let Some(config) = advanced_auth.as_object_mut() {
                config.insert("enabled".to_string(), Value::Bool(false));
                config.insert(
                    "policy_version".to_string(),
                    Value::String(uuid::Uuid::new_v4().to_string()),
                );
            }
        }
        object.insert("advanced_auth".to_string(), advanced_auth);
        object.insert(
            "service_role".to_string(),
            Value::String(service_role.to_string()),
        );
        object.insert(
            "title".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title"),
                previous,
                "title",
                can_reuse_previous_metadata,
            )),
        );
        object.insert(
            "title_override".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title_override"),
                previous,
                "title_override",
                true,
            )),
        );
        object.insert(
            "favicon".to_string(),
            Value::String(normalize_metadata_string(
                object.get("favicon"),
                previous,
                "favicon",
                can_reuse_previous_metadata,
            )),
        );
        let favicon_override = if service_role == "auth" {
            String::new()
        } else {
            let requested = object
                .get("favicon_override")
                .or_else(|| previous.and_then(|value| value.get("favicon_override")));
            normalize_favicon_override(requested)
                .map_err(|message| format!("Host mapping {host} {message}"))?
        };
        object.insert(
            "favicon_override".to_string(),
            Value::String(favicon_override),
        );
        normalized.push(Value::Object(object));
    }

    Ok(normalized)
}

pub(super) fn normalize_favicon_override(value: Option<&Value>) -> Result<String, String> {
    let Some(value) = value else {
        return Ok(String::new());
    };
    let Some(value) = value.as_str() else {
        return Err("custom icon must be an image data URL".to_string());
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }

    let Some((header, encoded)) = value.split_once(',') else {
        return Err("custom icon must be an image data URL".to_string());
    };
    let media_type = header
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "custom icon must be a base64 image data URL".to_string())?;
    if !matches!(
        media_type.as_str(),
        "image/png" | "image/jpeg" | "image/webp" | "image/x-icon" | "image/vnd.microsoft.icon"
    ) {
        return Err("custom icon format is not supported".to_string());
    }
    let max_encoded_len = MAX_FAVICON_BYTES.div_ceil(3) * 4;
    if encoded.len() > max_encoded_len {
        return Err("custom icon exceeds the 128 KiB limit".to_string());
    }

    let bytes = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| "custom icon contains invalid base64 data".to_string())?;
    if bytes.is_empty() {
        return Err("custom icon image is empty".to_string());
    }
    if bytes.len() > MAX_FAVICON_BYTES {
        return Err("custom icon exceeds the 128 KiB limit".to_string());
    }

    let signature_matches = match media_type.as_str() {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "image/x-icon" | "image/vnd.microsoft.icon" => {
            bytes.starts_with(&[0, 0, 1, 0]) || bytes.starts_with(&[0, 0, 2, 0])
        }
        _ => false,
    };
    if !signature_matches {
        return Err("custom icon content does not match its image format".to_string());
    }

    Ok(value.to_string())
}

#[derive(Debug)]
pub(super) struct CompiledHostMappings {
    pub(super) mappings: Vec<Value>,
    pub(super) visibility_policies: Map<String, Value>,
}

pub(super) async fn compile_host_mapping_visibilities(
    state: &AppState,
    mappings: Vec<Value>,
    previous_config: &Value,
) -> Result<CompiledHostMappings, String> {
    let previous_by_host = previous_host_mappings_by_host(previous_config);
    let mut visibility_policies = previous_config
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut compiled = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Host mapping must be an object".to_string());
        };
        let host = object
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let is_custom = object
            .get("visibility")
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str)
            == Some("custom");
        let matches_previous = previous_by_host
            .get(&host)
            .and_then(|value| value.get("visibility"))
            == object.get("visibility");
        if is_custom {
            let visibility = object
                .get("visibility")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("Host mapping {host} visibility must be an object"))?;
            let existing_policy_id = visibility
                .get("policy_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let existing_policy_valid =
                existing_policy_id.is_some_and(|id| visibility_policies.contains_key(id));
            let next = if matches_previous && !existing_policy_valid {
                let legacy_cidrs = visibility
                    .get("cidrs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>();
                if legacy_cidrs.is_empty() {
                    gateway_settings::compile_host_visibility_config(state, visibility)
                        .await
                        .map_err(|message| format!("Host mapping {host} visibility: {message}"))?
                } else {
                    let policy = crate::cidr::compile_ip_set(legacy_cidrs)
                        .map_err(|message| format!("Host mapping {host} visibility: {message}"))?;
                    gateway_settings::CompiledHostVisibility {
                        config: json!({
                            "mode": "custom",
                            "selections": visibility.get("selections").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                            "custom_cidrs": visibility.get("custom_cidrs").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                            "policy_id": policy.id,
                            "source_cidr_count": policy.source_cidr_count,
                            "range_count": policy.range_count(),
                        }),
                        policy,
                    }
                }
            } else if !matches_previous {
                gateway_settings::compile_host_visibility_config(state, visibility)
                    .await
                    .map_err(|message| format!("Host mapping {host} visibility: {message}"))?
            } else {
                let mut compact = visibility.clone();
                compact.remove("cidrs");
                object.insert("visibility".to_string(), Value::Object(compact));
                compiled.push(Value::Object(object));
                continue;
            };
            visibility_policies.insert(next.policy.id.clone(), next.policy.to_config_value());
            object.insert("visibility".to_string(), next.config);
        } else if let Some(visibility) = object.get_mut("visibility").and_then(Value::as_object_mut)
        {
            visibility.remove("cidrs");
            visibility.remove("policy_id");
            visibility.remove("source_cidr_count");
            visibility.remove("range_count");
        }
        compiled.push(Value::Object(object));
    }
    let mut referenced = referenced_host_ipset_policy_ids(&compiled)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if let Some(global_policy_id) = previous_config
        .pointer("/gateway_visibility/policy_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        referenced.insert(global_policy_id.to_string());
    }
    visibility_policies.retain(|id, _| referenced.contains(id));
    Ok(CompiledHostMappings {
        mappings: compiled,
        visibility_policies,
    })
}

pub(super) fn normalize_host_mapping_visibility(
    requested: Option<&Value>,
    previous: Option<&Value>,
    is_auth: bool,
) -> Result<Value, String> {
    if is_auth {
        return Ok(json!({
            "mode": "inherit",
            "selections": [],
            "custom_cidrs": [],
            "cidrs": [],
        }));
    }

    let source = match requested {
        Some(value) => Some(
            value
                .as_object()
                .ok_or_else(|| "visibility must be an object".to_string())?,
        ),
        None => previous.and_then(Value::as_object),
    };

    if requested.is_some() {
        if source
            .and_then(|value| value.get("selections"))
            .is_some_and(|value| !value.is_array())
        {
            return Err("visibility selections must be an array".to_string());
        }
        if source
            .and_then(|value| value.get("custom_cidrs"))
            .is_some_and(|value| !value.is_array())
        {
            return Err("visibility custom_cidrs must be an array".to_string());
        }
    }

    let raw_mode = source.and_then(|value| value.get("mode"));
    let mode = match raw_mode {
        None => "inherit",
        Some(Value::String(value))
            if value == "inherit" || value == "custom" || value == "disabled" =>
        {
            value.as_str()
        }
        Some(_) if requested.is_none() => "inherit",
        Some(_) => {
            return Err("visibility mode must be inherit, custom or disabled".to_string());
        }
    };
    let requested_selections = source
        .and_then(|value| value.get("selections"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let previous_selections = previous
        .and_then(|value| value.get("selections"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let selections =
        preserve_visibility_selection_metadata(requested_selections, previous_selections)?;
    let custom_cidrs =
        normalized_visibility_strings(source.and_then(|value| value.get("custom_cidrs")));
    let cidr_source = if requested.is_some() {
        previous.and_then(|value| value.get("cidrs"))
    } else {
        source.and_then(|value| value.get("cidrs"))
    };
    let cidrs = normalized_visibility_strings(cidr_source);
    let mut normalized = json!({
        "mode": mode,
        "selections": selections,
        "custom_cidrs": custom_cidrs,
        "cidrs": cidrs,
    });
    if mode == "custom" {
        for key in ["policy_id", "source_cidr_count", "range_count"] {
            if let Some(value) = previous.and_then(|value| value.get(key)).cloned() {
                ensure_object(&mut normalized).insert(key.to_string(), value);
            }
        }
    }
    Ok(normalized)
}

fn preserve_visibility_selection_metadata(
    requested: Vec<Value>,
    previous: &[Value],
) -> Result<Vec<Value>, String> {
    let mut previous_by_key = HashMap::new();
    for selection in previous {
        if let Ok(Some(key)) = visibility_selection_key(selection) {
            previous_by_key.insert(key, selection.clone());
        }
    }

    requested
        .into_iter()
        .map(|selection| {
            let Some(key) = visibility_selection_key(&selection)? else {
                return Ok(selection);
            };
            Ok(previous_by_key.get(&key).cloned().unwrap_or(selection))
        })
        .collect()
}

fn visibility_selection_key(selection: &Value) -> Result<Option<String>, String> {
    let Some(object) = selection.as_object() else {
        return Ok(None);
    };
    let province = object
        .get("province")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if province.is_empty() {
        return Ok(None);
    }
    let query_city = object
        .get("query_city")
        .or_else(|| object.get("queryCity"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let operator = crate::cidr::CidrOperator::parse_value(object.get("operator"))?;
    Ok(Some(
        crate::cidr::CidrRegionQuery::new(province, query_city, operator).key(),
    ))
}

fn normalized_visibility_strings(value: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .filter(|value| seen.insert(value.to_ascii_lowercase()))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_host_mapping_availability_for_route(
    host: &str,
    value: Option<&Value>,
) -> Result<Value, String> {
    match crate::daily_availability::normalize_daily_availability(value) {
        Ok(availability) => Ok(availability),
        Err(crate::daily_availability::DailyAvailabilityError::ObjectRequired) => Err(format!(
            "Host mapping {host} availability must be an object"
        )),
        Err(crate::daily_availability::DailyAvailabilityError::InvalidStart) => Err(format!(
            "Host mapping {host} availability start_time must use HH:mm"
        )),
        Err(crate::daily_availability::DailyAvailabilityError::InvalidEnd) => Err(format!(
            "Host mapping {host} availability end_time must use HH:mm"
        )),
        Err(crate::daily_availability::DailyAvailabilityError::SameTime) => Err(format!(
            "Host mapping {host} availability start_time and end_time must be different"
        )),
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(super) enum HostAvailabilityWindowError {
    InvalidStart,
    InvalidEnd,
    Same,
}

#[cfg(test)]
pub(super) fn validate_host_availability_window(
    start_time: &str,
    end_time: &str,
) -> Result<(), HostAvailabilityWindowError> {
    match crate::daily_availability::validate_daily_availability_window(start_time, end_time) {
        Ok(()) => Ok(()),
        Err(crate::daily_availability::DailyAvailabilityError::InvalidStart) => {
            Err(HostAvailabilityWindowError::InvalidStart)
        }
        Err(crate::daily_availability::DailyAvailabilityError::InvalidEnd) => {
            Err(HostAvailabilityWindowError::InvalidEnd)
        }
        Err(crate::daily_availability::DailyAvailabilityError::SameTime) => {
            Err(HostAvailabilityWindowError::Same)
        }
        Err(crate::daily_availability::DailyAvailabilityError::ObjectRequired) => {
            Err(HostAvailabilityWindowError::InvalidStart)
        }
    }
}

pub(super) fn normalize_stream_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, String> {
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut seen = HashSet::new();
    for mapping in mappings {
        let Some(object) = mapping.as_object() else {
            return Err("Stream mapping must be an object".to_string());
        };
        let protocol = if object
            .get("protocol")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "udp")
        {
            "udp"
        } else {
            "tcp"
        };
        let Some(listen_port) = object.get("listen_port").and_then(json_integer) else {
            return Err("Stream mapping listen_port must be an integer".to_string());
        };
        if listen_port <= 0 || listen_port > 65535 {
            return Err(format!(
                "Stream mapping listen_port {listen_port} is out of range"
            ));
        }
        let key = format!("{protocol}:{listen_port}");
        if !seen.insert(key) {
            return Err(format!(
                "Duplicate stream mapping for {} port {listen_port}",
                protocol.to_ascii_uppercase()
            ));
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_valid_host_port(&target) {
            return Err(format!("Stream mapping target must be host:port: {target}"));
        }

        normalized.push(json!({
            "protocol": protocol,
            "listen_port": listen_port,
            "target": target,
            "use_auth": object.get("use_auth").and_then(Value::as_bool).unwrap_or(true),
            "comment": object
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim(),
        }));
    }
    Ok(normalized)
}

fn stream_target_host_port(target: &str) -> Option<(&str, u16)> {
    if let Some(rest) = target.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        return Some((host, port.parse().ok()?));
    }
    let (host, port) = target.rsplit_once(':')?;
    Some((host, port.parse().ok()?))
}

fn normalize_local_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ipv6) => ipv6
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ipv6)),
        ipv4 => ipv4,
    }
}

fn local_stream_target_addresses() -> HashSet<IpAddr> {
    let mut addresses = HashSet::from([
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    ]);
    if let Ok(interfaces) = get_if_addrs() {
        for interface in interfaces {
            let ip = match interface.addr {
                IfAddr::V4(address) => IpAddr::V4(address.ip),
                IfAddr::V6(address) => IpAddr::V6(address.ip),
            };
            addresses.insert(normalize_local_ip(ip));
        }
    }
    addresses
}

fn is_local_stream_target(host: &str, local_addresses: &HashSet<IpAddr>) -> bool {
    let normalized_host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if normalized_host == "localhost" {
        return true;
    }
    let address_host = normalized_host
        .split_once('%')
        .map(|(address, _)| address)
        .unwrap_or(&normalized_host);
    let Ok(ip) = address_host.parse::<IpAddr>() else {
        return false;
    };
    let normalized_ip = normalize_local_ip(ip);
    normalized_ip.is_loopback()
        || normalized_ip.is_unspecified()
        || local_addresses.contains(&normalized_ip)
}

fn stream_mapping_local_loop_error(
    mapping: &Value,
    local_addresses: &HashSet<IpAddr>,
) -> Option<String> {
    let listen_port = mapping.get("listen_port").and_then(json_integer)?;
    let target = mapping.get("target").and_then(Value::as_str)?.trim();
    let (target_host, target_port) = stream_target_host_port(target)?;
    if i64::from(target_port) != listen_port
        || !is_local_stream_target(target_host, local_addresses)
    {
        return None;
    }
    let protocol = mapping
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("tcp")
        .to_ascii_uppercase();
    Some(format!(
        "Stream mapping {protocol} listen_port {listen_port} cannot target the same local port {target}"
    ))
}

pub(super) fn validate_stream_mapping_local_loops_with_addresses(
    mappings: &[Value],
    local_addresses: &HashSet<IpAddr>,
) -> Result<(), String> {
    for mapping in mappings {
        if let Some(error) = stream_mapping_local_loop_error(mapping, local_addresses) {
            return Err(error);
        }
    }
    Ok(())
}

pub(super) fn validate_stream_mapping_runtime_safety_inner(config: &Value) -> Result<(), String> {
    let mappings = config
        .get("stream_mappings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    validate_stream_mapping_local_loops_with_addresses(mappings, &local_stream_target_addresses())
}

pub(super) fn validate_stream_mapping_update_safety(
    previous_mappings: &[Value],
    next_mappings: &[Value],
    allow_unchanged_legacy_loops: bool,
) -> Result<(), String> {
    let local_addresses = local_stream_target_addresses();
    for mapping in next_mappings {
        let Some(error) = stream_mapping_local_loop_error(mapping, &local_addresses) else {
            continue;
        };
        let is_unchanged_legacy_mapping = allow_unchanged_legacy_loops
            && previous_mappings.iter().any(|previous| {
                stream_mapping_forward_identity(previous)
                    == stream_mapping_forward_identity(mapping)
            });
        if !is_unchanged_legacy_mapping {
            return Err(error);
        }
    }
    Ok(())
}

pub(super) fn stream_mapping_update_only_removes_entries(
    previous_mappings: &[Value],
    next_mappings: &[Value],
) -> bool {
    next_mappings.len() < previous_mappings.len()
        && next_mappings.iter().all(|next| {
            let next_identity = stream_mapping_forward_identity(next);
            next_identity.is_some()
                && previous_mappings
                    .iter()
                    .any(|previous| stream_mapping_forward_identity(previous) == next_identity)
        })
}

fn stream_mapping_forward_identity(mapping: &Value) -> Option<(&str, i64, &str)> {
    let protocol = if mapping.get("protocol").and_then(Value::as_str) == Some("udp") {
        "udp"
    } else {
        "tcp"
    };
    Some((
        protocol,
        mapping.get("listen_port").and_then(json_integer)?,
        mapping.get("target").and_then(Value::as_str)?.trim(),
    ))
}

pub(super) fn validate_host_mappings_section(config: &Value) -> Result<(), String> {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    normalize_host_mappings_for_route(mappings, config).map(|_| ())
}

pub(super) fn validate_passkey_rp_config(config: &Value) -> Result<(), String> {
    let Some(subdomain_mode) = config.get("subdomain_mode").and_then(Value::as_object) else {
        return Ok(());
    };
    let mode = subdomain_mode
        .get("passkey_rp_mode")
        .and_then(Value::as_str)
        .unwrap_or("auth_host");
    if mode != "parent_domain" {
        return Ok(());
    }

    let rp_id = normalize_host_value(
        subdomain_mode
            .get("passkey_rp_id")
            .and_then(Value::as_str)
            .or_else(|| subdomain_mode.get("root_domain").and_then(Value::as_str))
            .unwrap_or(""),
    );
    if rp_id.is_empty() {
        return Err("Passkey parent-domain RP ID is required".to_string());
    }

    let auth_host = get_auth_host_mapping(config)
        .and_then(|mapping| {
            mapping
                .get("host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .or_else(|| {
            subdomain_mode
                .get("auth_host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .unwrap_or_default();

    if !auth_host.is_empty() && auth_host != rp_id && !auth_host.ends_with(&format!(".{rp_id}")) {
        return Err(format!(
            "Passkey auth host {auth_host} must match or belong to RP ID {rp_id}"
        ));
    }
    Ok(())
}

pub(super) fn normalize_host_mapping_locations_for_route(
    host: &str,
    value: Option<&Value>,
) -> Result<Vec<Value>, String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut normalized = Vec::with_capacity(items.len());
    let mut seen = HashSet::new();

    for item in items {
        let object = item.as_object().cloned().unwrap_or_else(Map::new);
        let raw_path = object
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if raw_path.is_empty() {
            return Err(format!("Host mapping {host} location path is required"));
        }
        if !raw_path.starts_with('/') {
            return Err(format!(
                "Host mapping {host} location path {raw_path} must start with /"
            ));
        }
        let path = clean_host_location_path(raw_path);
        if path == "/" {
            return Err(format!("Host mapping {host} location path / is reserved"));
        }
        if path.starts_with("/__") || path == "/s" || path == "/s/" {
            return Err(format!(
                "Host mapping {host} location path {path} is reserved"
            ));
        }
        let match_mode = if object
            .get("match")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "exact")
        {
            "exact"
        } else {
            "prefix"
        };
        let duplicate_key = format!("{match_mode}\0{path}");
        if !seen.insert(duplicate_key) {
            return Err(format!("Host mapping {host} has duplicate location {path}"));
        }

        let action = if object
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "response")
        {
            "response"
        } else {
            "proxy"
        };
        let target = if action == "proxy" {
            let target = object
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if target.is_empty() {
                return Err(format!(
                    "Host mapping {host} location {path} target is required"
                ));
            }
            if !is_supported_proxy_target_url(&target) {
                return Err(format!(
                    "Host mapping {host} location {path} target must be a supported HTTP/WebSocket URL"
                ));
            }
            target
        } else {
            String::new()
        };

        if action == "response" {
            validate_location_response(host, &path, object.get("response"))?;
        }

        normalized.push(json!({
            "path": path,
            "match": match_mode,
            "action": action,
            "target": target,
            "strip_path": action == "proxy" && object.get("strip_path").and_then(Value::as_bool).unwrap_or(true),
            "rewrite_html": action == "proxy" && object.get("rewrite_html").and_then(Value::as_bool).unwrap_or(true),
            "response": if action == "response" {
                normalize_location_response(object.get("response"))
            } else {
                normalize_location_response(None)
            },
        }));
    }

    Ok(normalized)
}

pub(super) fn validate_location_response(
    host: &str,
    path: &str,
    value: Option<&Value>,
) -> Result<(), String> {
    let object = value.and_then(Value::as_object);
    let status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    if !(100..=599).contains(&status) {
        return Err(format!(
            "Host mapping {host} location {path} response status is invalid"
        ));
    }

    let headers = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object);
    if let Some(headers) = headers {
        for raw_name in headers.keys() {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {raw_name} is invalid"
                ));
            }
            if forbidden_response_header(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {name} is forbidden"
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn normalize_location_response(value: Option<&Value>) -> Value {
    let object = value.and_then(Value::as_object);
    let raw_status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    let status = if (100..=599).contains(&raw_status) {
        raw_status
    } else {
        200
    };
    let content_type = object
        .and_then(|map| map.get("content_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE);

    let mut headers = Map::new();
    if let Some(header_map) = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object)
    {
        for (raw_name, raw_value) in header_map {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) || forbidden_response_header(name) {
                continue;
            }
            headers.insert(
                name.to_string(),
                Value::String(
                    raw_value
                        .as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| raw_value.to_string()),
                ),
            );
        }
    }

    json!({
        "status": status,
        "content_type": content_type,
        "headers": headers,
        "body": object
            .and_then(|map| map.get("body"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    })
}
