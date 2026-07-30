use super::*;

pub(super) async fn sync_gateway_visibility_runtime(
    state: &AppState,
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_gateway_visibility(&visibility_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(crate) async fn sync_gateway_visibility_runtime_from_store(
    state: &AppState,
) -> Result<(), String> {
    let runtime = state
        .store
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
        .map_err(|error| error.to_string())?
        .unwrap_or_else(default_gateway_visibility_runtime);
    sync_gateway_visibility_runtime(state, &runtime).await
}

pub(super) async fn sync_gateway_proxy_headers_runtime(
    state: &AppState,
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_forwarded_headers_config(&omit_targets_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) async fn sync_gateway_host_response_runtime(
    state: &AppState,
    config: &Value,
    _runtime: &Value,
) -> Result<(), String> {
    sync_gateway_target_runtime_for_config(state, config, false, false).await
}

async fn sync_gateway_host_response_runtime_locked(
    state: &AppState,
    config: &Value,
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_preserve_host_config(&omit_targets_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )?;
    proxy_config::sync_go_host_rules_for_config_locked(state, config).await
}

pub(crate) async fn sync_gateway_target_runtime_for_config(
    state: &AppState,
    config: &Value,
    save_config: bool,
    host_rules_lock_held: bool,
) -> Result<(), String> {
    if host_rules_lock_held {
        return sync_gateway_target_runtime_for_config_locked(state, config, save_config).await;
    }
    proxy_config::with_host_mappings_runtime_transaction(state, |state| async move {
        let current_config = state
            .store
            .get_config()
            .await
            .map_err(|error| error.to_string())?;
        sync_gateway_target_runtime_for_config_locked(&state, &current_config, save_config).await
    })
    .await
}

async fn sync_gateway_target_runtime_for_config_locked(
    state: &AppState,
    config: &Value,
    save_config: bool,
) -> Result<(), String> {
    // Keep the exact original section values (including absence) as the
    // optimistic-merge precondition. A gateway settings request may update
    // either section after this snapshot was read; in that case storage must
    // retain the newer section instead of replacing it with this stale
    // compiled value.
    let expected_proxy_source = config.get("gateway_proxy_headers").cloned();
    let proxy_source = expected_proxy_source
        .clone()
        .unwrap_or_else(default_disabled_hosts_config);
    let proxy_requested = normalize_disabled_hosts_config(&proxy_source);
    let requested_proxy_compiled = compile_gateway_proxy_headers_state(config, &proxy_requested);

    let mut requested_config = config.clone();
    ensure_object(&mut requested_config).insert(
        "gateway_proxy_headers".to_string(),
        requested_proxy_compiled.config.clone(),
    );

    let expected_host_response_source = config.get("gateway_host_response").cloned();
    let host_response_source = expected_host_response_source
        .clone()
        .unwrap_or_else(default_disabled_hosts_config);
    let host_response_requested = normalize_disabled_hosts_config(&host_response_source);
    let requested_host_response_compiled =
        compile_gateway_host_response_state(&requested_config, &host_response_requested);

    let effective_config = if save_config {
        state
            .store
            .merge_gateway_target_config_sections(
                expected_proxy_source.as_ref(),
                &requested_proxy_compiled.config,
                expected_host_response_source.as_ref(),
                &requested_host_response_compiled.config,
            )
            .await
            .map_err(|error| error.to_string())?
    } else {
        config.clone()
    };

    // The section merge may have rebased onto a newer run_type/submode. Build
    // runtime payloads again from the complete config returned by storage.
    let proxy_source = effective_config
        .get("gateway_proxy_headers")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let proxy_requested = normalize_disabled_hosts_config(&proxy_source);
    let proxy_compiled = compile_gateway_proxy_headers_state(&effective_config, &proxy_requested);
    let host_response_source = effective_config
        .get("gateway_host_response")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let host_response_requested = normalize_disabled_hosts_config(&host_response_source);
    let host_response_compiled =
        compile_gateway_host_response_state(&effective_config, &host_response_requested);

    state
        .store
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, &proxy_compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_proxy_headers_runtime(state, &proxy_compiled.runtime).await?;

    state
        .store
        .set_json_value(
            GATEWAY_HOST_RESPONSE_RUNTIME_KEY,
            &host_response_compiled.runtime,
        )
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_host_response_runtime_locked(
        state,
        &effective_config,
        &host_response_compiled.runtime,
    )
    .await
}

pub(super) fn visibility_sync_payload(runtime: &Value) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "enabled".to_string(),
        Value::Bool(
            runtime
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    payload.insert(
        "cidrs".to_string(),
        runtime
            .get("cidrs")
            .and_then(Value::as_array)
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    if let Some(policy_id) = runtime.get("policy_id").and_then(Value::as_str) {
        payload.insert(
            "policy_id".to_string(),
            Value::String(policy_id.to_string()),
        );
    }
    if let Some(policy) = runtime.get("policy").filter(|value| value.is_object()) {
        payload.insert("policy".to_string(), policy.clone());
    }
    if let Some(updated_at) = runtime.get("updated_at").and_then(Value::as_str) {
        payload.insert(
            "updated_at".to_string(),
            Value::String(updated_at.to_string()),
        );
    }
    Value::Object(payload)
}

pub(super) fn omit_targets_sync_payload(runtime: &Value) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "enabled".to_string(),
        Value::Bool(
            runtime
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    payload.insert(
        "omit_targets".to_string(),
        runtime
            .get("omit_targets")
            .and_then(Value::as_array)
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    if let Some(updated_at) = runtime.get("updated_at").and_then(Value::as_str) {
        payload.insert(
            "updated_at".to_string(),
            Value::String(updated_at.to_string()),
        );
    }
    Value::Object(payload)
}

pub(super) async fn sync_gateway_runtime(state: &AppState, _config: &Value) -> Result<(), String> {
    proxy_config::with_host_mappings_runtime_transaction(state, |state| async move {
        let current_config = state
            .store
            .get_config()
            .await
            .map_err(|error| error.to_string())?;
        sync_gateway_runtime_locked(&state, &current_config).await
    })
    .await
}

async fn sync_gateway_runtime_locked(state: &AppState, config: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_auth_config(&build_gateway_auth_config(config))
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let throttle = normalize_reverse_proxy_throttle(
        config
            .get("reverse_proxy_throttle")
            .unwrap_or(&default_reverse_proxy_throttle()),
    );
    ensure_go_success(
        state
            .go_backend
            .set_reverse_proxy_throttle(&throttle)
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let crawler = normalize_gateway_crawler_blocker(
        config
            .get("gateway_crawler_blocker")
            .unwrap_or(&default_gateway_crawler_blocker()),
    );
    ensure_go_success(
        state
            .go_backend
            .set_crawler_blocker_config(&crawler)
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let portal = normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    );
    let portal_response = state
        .go_backend
        .set_gateway_portal_config(&portal)
        .await
        .map_err(|error| error.to_string())?;
    ensure_gateway_portal_applied(&portal, portal_response)?;
    let unmatched_route = normalize_gateway_unmatched_route(
        config
            .get("gateway_unmatched_route")
            .unwrap_or(&default_gateway_unmatched_route()),
    );
    let unmatched_route_response = state
        .go_backend
        .set_gateway_unmatched_route_config(&unmatched_route)
        .await
        .map_err(|error| error.to_string())?;
    ensure_gateway_unmatched_route_applied(&unmatched_route, unmatched_route_response)
}

pub(super) async fn apply_gateway_portal_host_rules_patches_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    apply_gateway_portal_title_host_rules_patch_if_needed(state, config).await?;
    apply_gateway_portal_icon_host_rules_patch_if_needed(state, config).await?;
    Ok(())
}

pub(super) async fn apply_gateway_portal_title_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<bool, String> {
    if !is_gateway_portal_title_mode(config) {
        return Ok(false);
    }
    apply_gateway_portal_host_rules_patch_if_needed(
        state,
        config,
        GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY,
    )
    .await
}

pub(super) async fn apply_gateway_portal_icon_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<bool, String> {
    if !is_gateway_portal_app_icon_mode(config) {
        return Ok(false);
    }
    apply_gateway_portal_host_rules_patch_if_needed(
        state,
        config,
        GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY,
    )
    .await
}

pub(super) async fn apply_gateway_portal_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
    flag_key: &str,
) -> Result<bool, String> {
    if !is_any_subdomain_routing_mode(config) {
        return Ok(false);
    }
    if state
        .store
        .get_string_value(flag_key)
        .await
        .map_err(|error| error.to_string())?
        .as_deref()
        == Some("1")
    {
        return Ok(false);
    }

    proxy_config::sync_current_go_host_rules(state).await?;

    if let Err(error) = state.store.set_string_value(flag_key, "1").await {
        tracing::warn!(%error, %flag_key, "failed to mark gateway portal host-rules patch applied");
    }
    Ok(true)
}

pub(super) fn ensure_go_success(value: Value) -> Result<(), String> {
    if crate::go_backend::response_success(&value) {
        return Ok(());
    }
    Err(crate::go_backend::response_message(
        &value,
        GO_BACKEND_UNSUCCESSFUL_RESPONSE,
    ))
}

pub(super) fn ensure_gateway_portal_applied(
    requested: &Value,
    response: Value,
) -> Result<(), String> {
    ensure_go_success(response.clone())?;

    let Some(applied) = response.get("data").filter(|value| value.is_object()) else {
        return Err("Go backend did not return the applied gateway portal config".to_string());
    };
    let requested = normalize_gateway_portal(requested);
    let applied = normalize_gateway_portal(applied);
    if requested == applied {
        return Ok(());
    }

    Err(format!(
        "Go backend did not apply gateway portal config (requested {requested}, reported {applied}); upgrade the gateway backend"
    ))
}

pub(super) fn ensure_gateway_unmatched_route_applied(
    requested: &Value,
    response: Value,
) -> Result<(), String> {
    ensure_go_success(response.clone())?;

    let Some(applied) = response.get("data").filter(|value| value.is_object()) else {
        return Err(
            "Go backend did not return the applied gateway unmatched-route config".to_string(),
        );
    };
    let requested = normalize_gateway_unmatched_route(requested);
    let applied = normalize_gateway_unmatched_route(applied);
    if requested == applied {
        return Ok(());
    }

    Err(format!(
        "Go backend did not apply gateway unmatched-route config (requested {requested}, reported {applied}); upgrade the gateway backend"
    ))
}
