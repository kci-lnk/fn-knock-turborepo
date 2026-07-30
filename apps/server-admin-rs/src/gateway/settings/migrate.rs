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
    let migration_source = restore_cached_policy_references(state, &previous).await?;
    let (candidate, runtime) =
        compile_visibility_policy_migration(&migration_source, &previous_runtime)?;

    let requires_visibility_sync = candidate
        .get("visibility_policies")
        .and_then(Value::as_object)
        .is_some_and(|policies| !policies.is_empty())
        || candidate
            .pointer("/gateway_visibility/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    if requires_visibility_sync {
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

async fn restore_cached_policy_references(
    state: &AppState,
    previous: &Value,
) -> Result<Value, String> {
    let mut source = previous.clone();
    let root = ensure_object(&mut source);
    let mappings = root
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut referenced = proxy_config::referenced_host_ipset_policy_ids(&mappings);
    if let Some(id) = root
        .get("gateway_visibility")
        .and_then(|value| value.get("policy_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        referenced.insert(id.to_string());
    }
    let mut policies = root
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut recovered = 0usize;
    for id in referenced {
        let valid = policies
            .get(&id)
            .is_some_and(|value| CompiledIpSet::from_config_value(&id, value).is_ok());
        if valid {
            continue;
        }
        if let Some(policy) = crate::cidr::cached_compiled_policy_by_id(state, &id)
            .await
            .map_err(|error| error.to_string())?
        {
            policies.insert(policy.id.clone(), policy.to_config_value());
            recovered += 1;
        }
    }
    if recovered > 0 {
        tracing::info!(
            recovered,
            "recovered dangling compiled IP set references from local CIDR caches"
        );
    }
    root.insert("visibility_policies".to_string(), Value::Object(policies));
    Ok(source)
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
            let advanced_auth_enabled = mapping
                .pointer("/advanced_auth/enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
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
                    match policies
                        .get(&id)
                        .ok_or_else(|| {
                            format!(
                                "Host mapping {host} advanced auth condition {condition_id} policy {id} is missing"
                            )
                        })
                        .and_then(|encoded| {
                            CompiledIpSet::from_config_value(&id, encoded).map_err(|error| {
                                format!(
                                    "Host mapping {host} advanced auth condition {condition_id} policy is invalid: {error}"
                                )
                            })
                        }) {
                        Ok(policy) => Some(policy.into_current_format()),
                        Err(error) if !advanced_auth_enabled => {
                            tracing::warn!(
                                %host,
                                %condition_id,
                                %error,
                                "preserving disabled advanced-auth draft without an unusable IP set reference"
                            );
                            None
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    let cidrs = condition
                        .get("cidrs")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>();
                    if cidrs.is_empty() {
                        if advanced_auth_enabled {
                            return Err(format!(
                                "Host mapping {host} advanced auth condition {condition_id} has no compiled policy or legacy CIDRs"
                            ));
                        }
                        None
                    } else {
                        Some(compile_ip_set(cidrs).map_err(|error| {
                            format!(
                                "Host mapping {host} advanced auth condition {condition_id}: {error}"
                            )
                        })?)
                    }
                };
                let object = condition
                    .as_object_mut()
                    .ok_or_else(|| "advanced auth condition must be an object".to_string())?;
                object.remove("cidrs");
                if let Some(policy) = policy {
                    policies.insert(policy.id.clone(), policy.to_config_value());
                    object.insert("policy_id".to_string(), Value::String(policy.id.clone()));
                    object.remove("policy_recovery_required");
                    object
                        .entry("source_cidr_count".to_string())
                        .or_insert_with(|| json!(policy.source_cidr_count));
                    object
                        .entry("range_count".to_string())
                        .or_insert_with(|| json!(policy.range_count()));
                } else {
                    object.remove("policy_id");
                    object.remove("source_cidr_count");
                    object.remove("range_count");
                    object.insert("policy_recovery_required".to_string(), Value::Bool(true));
                }
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

    async fn migration_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.runtime_target = "linux".to_string();
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
        settings.internal_rpc_token = "migration-recovery-test-token".to_string();
        settings.request_timeout = std::time::Duration::from_millis(100);
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

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

    #[test]
    fn disabled_advanced_auth_draft_does_not_block_boot_when_policy_is_unrecoverable() {
        let (migrated, _) = compile_visibility_policy_migration(
            &json!({
                "host_mappings": [{
                    "host": "app.example.com",
                    "advanced_auth": {
                        "enabled": false,
                        "groups": [{
                            "id": "group-1",
                            "conditions": [{
                                "id": "condition-1",
                                "target": "source_region",
                                "operator": "in",
                                "policy_id": "ipset-v2:missing",
                                "selections": [{
                                    "province": "甘肃",
                                    "city": "定西",
                                    "operator": "移动"
                                }]
                            }]
                        }]
                    }
                }],
                "visibility_policies": {}
            }),
            &json!({"enabled": false}),
        )
        .unwrap();

        let condition =
            &migrated["host_mappings"][0]["advanced_auth"]["groups"][0]["conditions"][0];
        assert!(condition.get("policy_id").is_none());
        assert_eq!(condition["policy_recovery_required"], json!(true));
        assert_eq!(condition["selections"][0]["province"], json!("甘肃"));
    }

    #[test]
    fn enabled_advanced_auth_still_fails_closed_when_policy_is_missing() {
        let error = compile_visibility_policy_migration(
            &json!({
                "host_mappings": [{
                    "host": "app.example.com",
                    "advanced_auth": {
                        "enabled": true,
                        "groups": [{
                            "id": "group-1",
                            "conditions": [{
                                "id": "condition-1",
                                "target": "source_region",
                                "operator": "in",
                                "policy_id": "ipset-v2:missing"
                            }]
                        }]
                    }
                }],
                "visibility_policies": {}
            }),
            &json!({"enabled": false}),
        )
        .unwrap_err();

        assert!(error.contains("policy ipset-v2:missing is missing"));
    }

    #[tokio::test]
    async fn disabled_advanced_auth_policy_is_restored_from_compact_cache_offline() {
        let (_directory, state) = migration_test_state().await;
        let policy = compile_ip_set(["203.0.113.0/25", "203.0.113.128/25"]).unwrap();
        state
            .store
            .set_json_value(
                "fn_knock:cidr:test-source:cidrs:device-regression",
                &json!({
                    "fnknock_ipset_cache_version": 1,
                    "compiled_policy": policy.to_transport_value(),
                }),
            )
            .await
            .unwrap();
        let previous = json!({
            "host_mappings": [{
                "host": "app.example.com",
                "advanced_auth": {
                    "enabled": false,
                    "groups": [{
                        "id": "group-1",
                        "conditions": [{
                            "id": "condition-1",
                            "target": "source_region",
                            "operator": "in",
                            "policy_id": policy.id,
                            "selections": [{
                                "province": "甘肃",
                                "city": "定西",
                                "operator": "移动"
                            }]
                        }]
                    }]
                }
            }],
            "visibility_policies": {}
        });

        let source = restore_cached_policy_references(&state, &previous)
            .await
            .unwrap();
        let (migrated, _) =
            compile_visibility_policy_migration(&source, &json!({"enabled": false})).unwrap();
        let condition =
            &migrated["host_mappings"][0]["advanced_auth"]["groups"][0]["conditions"][0];
        let recovered_id = condition["policy_id"].as_str().unwrap();

        assert_eq!(recovered_id, policy.id);
        assert!(condition.get("policy_recovery_required").is_none());
        assert!(
            CompiledIpSet::from_config_value(
                recovered_id,
                &migrated["visibility_policies"][recovered_id],
            )
            .is_ok()
        );
    }
}
