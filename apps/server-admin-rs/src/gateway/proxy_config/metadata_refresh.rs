use super::*;

pub(super) async fn refresh_host_mapping_metadata(mappings: Vec<Value>) -> (Vec<Value>, Value) {
    let mut updated = 0_i64;
    let mut failed = 0_i64;
    let mut skipped = 0_i64;
    let mut next_mappings = Vec::with_capacity(mappings.len());

    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            skipped += 1;
            next_mappings.push(mapping);
            continue;
        };
        let target = object.get("target").and_then(Value::as_str).unwrap_or("");
        if normalize_http_probe_url(target).is_none() {
            skipped += 1;
            next_mappings.push(Value::Object(object));
            continue;
        }
        match fetch_host_mapping_metadata(target, object.get("basic_auth")).await {
            Ok(metadata) => {
                object.insert(
                    "title".to_string(),
                    metadata
                        .get("title")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                );
                object.insert(
                    "favicon".to_string(),
                    metadata
                        .get("favicon")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                );
                updated += 1;
            }
            Err(error) => {
                tracing::debug!(%error, target, "failed to refresh host mapping metadata");
                failed += 1;
            }
        }
        next_mappings.push(Value::Object(object));
    }

    (
        next_mappings,
        json!({
            "updated": updated,
            "failed": failed,
            "skipped": skipped,
        }),
    )
}

pub(super) fn schedule_host_mappings_metadata_refresh(
    state: AppState,
    mappings: Vec<Value>,
    previous_mappings: Vec<Value>,
) {
    tokio::spawn(async move {
        let (items, summary) =
            enrich_host_mapping_metadata_for_save(mappings, previous_mappings).await;
        tracing::debug!(
            updated = summary.updated,
            failed = summary.failed,
            skipped = summary.skipped,
            "host mappings metadata background refresh finished"
        );
        if summary.updated == 0 {
            return;
        }

        let current_config = match state.redis.get_config().await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "failed to load config before merging host mappings metadata refresh"
                );
                return;
            }
        };
        let current_mappings = current_config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let (next_mappings, changed) =
            merge_metadata_into_current_mappings(current_mappings, items);
        if !changed {
            return;
        }

        let mut next_config = current_config.clone();
        ensure_object(&mut next_config).insert(
            "host_mappings".to_string(),
            Value::Array(next_mappings.clone()),
        );
        if let Err(error) = state.redis.save_config(&next_config).await {
            tracing::warn!(
                %error,
                "failed to save host mappings after metadata background refresh"
            );
            return;
        }
        if let Err(message) =
            sync_gateway_portal_host_rules_if_title_mode(&state, &next_config, &next_mappings).await
        {
            tracing::warn!(
                %message,
                "failed to sync refreshed host mapping metadata to gateway"
            );
        }
    });
}

pub(super) async fn enrich_host_mapping_metadata_for_save(
    mappings: Vec<Value>,
    previous_mappings: Vec<Value>,
) -> (
    Vec<HostMappingMetadataRefreshItem>,
    HostMappingMetadataRefreshSummary,
) {
    let previous_by_host = previous_mappings
        .into_iter()
        .map(|mapping| (host_mapping_key(&mapping), mapping))
        .collect::<HashMap<_, _>>();
    let mut summary = HostMappingMetadataRefreshSummary::default();
    let mut items = Vec::new();

    for mapping in mappings {
        let Some(object) = mapping.as_object() else {
            summary.skipped += 1;
            continue;
        };
        let (refresh_title, refresh_favicon) =
            resolve_metadata_refresh_decision(&mapping, &previous_by_host);
        if !refresh_title && !refresh_favicon {
            summary.skipped += 1;
            continue;
        }

        let target = object.get("target").and_then(Value::as_str).unwrap_or("");
        match fetch_host_mapping_metadata(target, object.get("basic_auth")).await {
            Ok(metadata) => {
                let mut refreshed = object.clone();
                if refresh_title {
                    refreshed.insert(
                        "title".to_string(),
                        Value::String(metadata_string(&metadata, "title")),
                    );
                }
                if refresh_favicon {
                    refreshed.insert(
                        "favicon".to_string(),
                        Value::String(metadata_string(&metadata, "favicon")),
                    );
                }
                items.push(HostMappingMetadataRefreshItem {
                    mapping: Value::Object(refreshed),
                    refresh_title,
                    refresh_favicon,
                });
                summary.updated += 1;
            }
            Err(error) => {
                tracing::debug!(%error, target, "failed to refresh host mapping metadata on save");
                summary.failed += 1;
            }
        }
    }

    (items, summary)
}

pub(super) fn resolve_metadata_refresh_decision(
    mapping: &Value,
    previous_by_host: &HashMap<String, Value>,
) -> (bool, bool) {
    let target = mapping_target(mapping);
    if target.is_empty() || normalize_http_probe_url(&target).is_none() {
        return (false, false);
    }

    let previous = previous_by_host.get(&host_mapping_key(mapping));
    let target_changed = previous
        .map(|previous| mapping_target(previous) != target)
        .unwrap_or(true);
    let basic_auth_changed = host_mapping_has_usable_basic_auth(mapping)
        && previous
            .map(|previous| !host_mapping_basic_auth_matches(previous, mapping))
            .unwrap_or(true);
    let refresh_title =
        target_changed || basic_auth_changed || metadata_string(mapping, "title").is_empty();
    let refresh_favicon =
        target_changed || basic_auth_changed || metadata_string(mapping, "favicon").is_empty();

    (refresh_title, refresh_favicon)
}

pub(super) fn merge_metadata_into_current_mappings(
    current_mappings: Vec<Value>,
    refreshed_items: Vec<HostMappingMetadataRefreshItem>,
) -> (Vec<Value>, bool) {
    let refreshed_by_host = refreshed_items
        .into_iter()
        .map(|item| (host_mapping_key(&item.mapping), item))
        .collect::<HashMap<_, _>>();
    let mut changed = false;
    let next_mappings = current_mappings
        .into_iter()
        .map(|mapping| {
            let Some(refreshed) = refreshed_by_host.get(&host_mapping_key(&mapping)) else {
                return mapping;
            };
            if mapping_target(&mapping) != mapping_target(&refreshed.mapping)
                || !host_mapping_basic_auth_matches(&mapping, &refreshed.mapping)
            {
                return mapping;
            }

            let Some(object) = mapping.as_object() else {
                return mapping;
            };
            let mut next = object.clone();
            let current_title = metadata_string(&mapping, "title");
            let current_favicon = metadata_string(&mapping, "favicon");
            let next_title = if refreshed.refresh_title {
                metadata_string(&refreshed.mapping, "title")
            } else {
                current_title.clone()
            };
            let next_favicon = if refreshed.refresh_favicon {
                metadata_string(&refreshed.mapping, "favicon")
            } else {
                current_favicon.clone()
            };

            if next_title == current_title && next_favicon == current_favicon {
                return mapping;
            }

            next.insert("title".to_string(), Value::String(next_title));
            next.insert("favicon".to_string(), Value::String(next_favicon));
            changed = true;
            Value::Object(next)
        })
        .collect();
    (next_mappings, changed)
}

pub(super) async fn sync_gateway_portal_host_rules_if_title_mode(
    state: &AppState,
    config: &Value,
    mappings: &[Value],
) -> Result<bool, String> {
    if !is_gateway_portal_title_mode(config) || !is_any_subdomain_routing_mode(config) {
        return Ok(false);
    }
    sync_go_host_rules(state, &build_host_rules_payload(mappings)).await?;
    Ok(true)
}

pub(super) fn is_gateway_portal_title_mode(config: &Value) -> bool {
    config
        .pointer("/gateway_portal/display_style")
        .and_then(Value::as_str)
        != Some("domain")
}

pub(super) fn host_mapping_key(value: &Value) -> String {
    normalize_host_value(value.get("host").and_then(Value::as_str).unwrap_or(""))
}

pub(super) fn mapping_target(value: &Value) -> String {
    value
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(super) fn metadata_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

pub(super) fn host_mapping_basic_auth_matches(left: &Value, right: &Value) -> bool {
    normalize_host_basic_auth(left.get("basic_auth"))
        == normalize_host_basic_auth(right.get("basic_auth"))
}

pub(super) fn host_mapping_has_usable_basic_auth(value: &Value) -> bool {
    normalize_host_basic_auth(value.get("basic_auth"))
        .get("enabled")
        .and_then(Value::as_bool)
        == Some(true)
}
