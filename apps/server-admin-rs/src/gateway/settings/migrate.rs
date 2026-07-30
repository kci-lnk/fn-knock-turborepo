use super::*;

pub(crate) async fn migrate_visibility_policies_on_boot(state: &AppState) -> Result<(), String> {
    proxy_config::with_host_mappings_runtime_transaction(state, |state| async move {
        migrate_visibility_policies_locked(&state).await
    })
    .await
}

/// Migrates and synchronizes visibility while the caller owns the Host
/// Mapping transaction lock and lease. Backup import uses this variant so the
/// restored data cannot become visible between replacement and migration.
pub(crate) async fn migrate_visibility_policies_locked(state: &AppState) -> Result<(), String> {
    let previous = state
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let previous_runtime = state
        .store
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
        .map_err(|error| error.to_string())?
        .unwrap_or_else(default_gateway_visibility_runtime);
    let (candidate, runtime) = compile_visibility_policy_migration(&previous, &previous_runtime)?;

    let requires_go_validation = candidate
        .get("visibility_policies")
        .and_then(Value::as_object)
        .is_some_and(|policies| !policies.is_empty())
        || candidate
            .pointer("/gateway_visibility/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    if requires_go_validation {
        // Both gRPC setters decode and validate the packed ranges and digest.
        // Only persist after the matching Go process has accepted the exact
        // candidate policy table.
        proxy_config::sync_go_host_rules_for_config_locked(state, &candidate).await?;
        sync_gateway_visibility_runtime(state, &runtime).await?;
    }

    if candidate != previous {
        state
            .store
            .compare_and_set_config_migration(&previous, &candidate)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "configuration changed while migrating visibility policies".to_string()
            })?;
    }
    state
        .store
        .set_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn compile_visibility_policy_migration(
    previous: &Value,
    previous_runtime: &Value,
) -> Result<(Value, Value), String> {
    let mut candidate = previous.clone();
    let root = ensure_object(&mut candidate);
    let mut policies = root
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if let Some(mappings) = root.get_mut("host_mappings").and_then(Value::as_array_mut) {
        for mapping in mappings.iter_mut() {
            let host = mapping
                .get("host")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_string();
            let Some(visibility) = mapping.get_mut("visibility").and_then(Value::as_object_mut)
            else {
                continue;
            };
            if visibility.get("mode").and_then(Value::as_str) != Some("custom") {
                visibility.remove("cidrs");
                visibility.remove("policy_id");
                visibility.remove("source_cidr_count");
                visibility.remove("range_count");
                continue;
            }

            let existing_id = visibility
                .get("policy_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            let policy = if let Some(id) = existing_id {
                let encoded = policies.get(&id).ok_or_else(|| {
                    format!("Host mapping {host} visibility policy {id} is missing")
                })?;
                CompiledIpSet::from_config_value(&id, encoded)
                    .map_err(|error| {
                        format!("Host mapping {host} visibility policy is invalid: {error}")
                    })?
                    .into_current_format()
            } else {
                let cidrs = visibility
                    .get("cidrs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>();
                if cidrs.is_empty() {
                    return Err(format!(
                        "Host mapping {host} custom visibility has no compiled policy or legacy CIDRs"
                    ));
                }
                compile_ip_set(cidrs)
                    .map_err(|error| format!("Host mapping {host} visibility: {error}"))?
            };
            policies.insert(policy.id.clone(), policy.to_config_value());
            visibility.remove("cidrs");
            visibility.insert("policy_id".to_string(), Value::String(policy.id.clone()));
            visibility
                .entry("source_cidr_count".to_string())
                .or_insert_with(|| json!(policy.source_cidr_count));
            visibility
                .entry("range_count".to_string())
                .or_insert_with(|| json!(policy.range_count()));
        }

        for mapping in mappings.iter_mut() {
            let host = mapping
                .get("host")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_string();
            for condition in mapping
                .pointer_mut("/advanced_auth/groups")
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
                .filter_map(|group| group.get_mut("conditions").and_then(Value::as_array_mut))
                .flatten()
            {
                let target = condition
                    .get("target")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if target != "source_ip" && target != "source_region" {
                    if let Some(object) = condition.as_object_mut() {
                        object.remove("cidrs");
                    }
                    continue;
                }
                let condition_id = condition
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_string();
                let existing_id = condition
                    .get("policy_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string);
                let policy = if let Some(id) = existing_id {
                    let encoded = policies.get(&id).ok_or_else(|| {
                        format!(
                            "Host mapping {host} advanced auth condition {condition_id} policy {id} is missing"
                        )
                    })?;
                    CompiledIpSet::from_config_value(&id, encoded)
                        .map_err(|error| {
                            format!(
                                "Host mapping {host} advanced auth condition {condition_id} policy is invalid: {error}"
                            )
                        })?
                        .into_current_format()
                } else {
                    let cidrs = condition
                        .get("cidrs")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>();
                    if cidrs.is_empty() {
                        return Err(format!(
                            "Host mapping {host} advanced auth condition {condition_id} has no compiled policy or legacy CIDRs"
                        ));
                    }
                    compile_ip_set(cidrs).map_err(|error| {
                        format!(
                            "Host mapping {host} advanced auth condition {condition_id}: {error}"
                        )
                    })?
                };
                policies.insert(policy.id.clone(), policy.to_config_value());
                let object = condition
                    .as_object_mut()
                    .ok_or_else(|| "advanced auth condition must be an object".to_string())?;
                object.remove("cidrs");
                object.insert("policy_id".to_string(), Value::String(policy.id.clone()));
                object
                    .entry("source_cidr_count".to_string())
                    .or_insert_with(|| json!(policy.source_cidr_count));
                object
                    .entry("range_count".to_string())
                    .or_insert_with(|| json!(policy.range_count()));
            }
        }
    }

    let gateway_visibility = root
        .entry("gateway_visibility".to_string())
        .or_insert_with(default_gateway_visibility);
    let gateway = gateway_visibility
        .as_object_mut()
        .ok_or_else(|| "gateway visibility config must be an object".to_string())?;
    let enabled = gateway
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let global_policy = if enabled {
        if let Some(id) = gateway
            .get("policy_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let encoded = policies
                .get(id)
                .ok_or_else(|| format!("Gateway visibility policy {id} is missing"))?;
            Some(
                CompiledIpSet::from_config_value(id, encoded)
                    .map_err(|error| format!("Gateway visibility policy is invalid: {error}"))?
                    .into_current_format(),
            )
        } else {
            let cidrs = previous_runtime
                .get("cidrs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            Some(compile_ip_set(cidrs).map_err(|error| format!("Gateway visibility: {error}"))?)
        }
    } else {
        None
    };

    gateway.remove("cidrs");
    let runtime = if let Some(policy) = global_policy {
        policies.insert(policy.id.clone(), policy.to_config_value());
        gateway.insert("policy_id".to_string(), Value::String(policy.id.clone()));
        gateway
            .entry("source_cidr_count".to_string())
            .or_insert_with(|| json!(policy.source_cidr_count));
        gateway
            .entry("range_count".to_string())
            .or_insert_with(|| json!(policy.range_count()));
        json!({
            "enabled": true,
            "policy_id": policy.id,
            "source_cidr_count": gateway.get("source_cidr_count").cloned().unwrap_or_else(|| json!(0)),
            "range_count": gateway.get("range_count").cloned().unwrap_or_else(|| json!(policy.range_count())),
            "policy": policy_transport_value(&policy),
            "updated_at": previous_runtime.get("updated_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso())),
        })
    } else {
        gateway.remove("policy_id");
        gateway.remove("source_cidr_count");
        gateway.remove("range_count");
        json!({
            "enabled": false,
            "policy_id": null,
            "source_cidr_count": 0,
            "range_count": 0,
            "policy": null,
            "updated_at": previous_runtime.get("updated_at").cloned().unwrap_or(Value::Null),
        })
    };
    let global_policy_id = gateway
        .get("policy_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let mut referenced = proxy_config::referenced_host_ipset_policy_ids(
        root.get("host_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten(),
    )
    .into_iter()
    .collect::<BTreeSet<_>>();
    if let Some(id) = global_policy_id {
        referenced.insert(id);
    }
    policies.retain(|id, _| referenced.contains(id));
    root.insert("visibility_policies".to_string(), Value::Object(policies));
    Ok((candidate, runtime))
}

fn policy_transport_value(policy: &CompiledIpSet) -> Value {
    let mut value = policy
        .to_config_value()
        .as_object()
        .cloned()
        .unwrap_or_default();
    value.insert("id".to_string(), Value::String(policy.id.clone()));
    Value::Object(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_shared_legacy_host_and_global_cidrs_once() {
        let previous = json!({
            "host_mappings": [
                {"host": "a.example.com", "visibility": {"mode": "custom", "cidrs": ["203.0.113.0/25", "203.0.113.128/25"]}},
                {"host": "b.example.com", "visibility": {"mode": "custom", "cidrs": ["203.0.113.0/24"]}}
            ],
            "gateway_visibility": {"enabled": true}
        });
        let runtime = json!({
            "enabled": true,
            "cidrs": ["203.0.113.0/24"],
            "updated_at": "2026-01-01T00:00:00Z"
        });
        let (migrated, runtime) = compile_visibility_policy_migration(&previous, &runtime).unwrap();
        let policies = migrated["visibility_policies"].as_object().unwrap();
        assert_eq!(policies.len(), 1);
        let first_id = migrated["host_mappings"][0]["visibility"]["policy_id"]
            .as_str()
            .unwrap();
        assert_eq!(
            migrated["host_mappings"][1]["visibility"]["policy_id"],
            json!(first_id)
        );
        assert_eq!(migrated["gateway_visibility"]["policy_id"], json!(first_id));
        assert!(
            migrated["host_mappings"][0]["visibility"]
                .get("cidrs")
                .is_none()
        );
        assert_eq!(runtime["policy_id"], json!(first_id));
    }

    #[test]
    fn refuses_custom_visibility_without_policy_or_legacy_cidrs() {
        let error = compile_visibility_policy_migration(
            &json!({
                "host_mappings": [{"host": "broken.example.com", "visibility": {"mode": "custom"}}]
            }),
            &json!({"enabled": false}),
        )
        .unwrap_err();
        assert!(error.contains("no compiled policy or legacy CIDRs"));
    }
}
