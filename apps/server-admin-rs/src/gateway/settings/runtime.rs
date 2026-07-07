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
        .redis
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
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_preserve_host_config(&omit_targets_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )?;
    if is_any_subdomain_routing_mode(config) {
        let host_mappings = config_host_mappings(config);
        ensure_go_success(
            state
                .go_backend
                .set_host_rules(&build_host_rules_payload(&host_mappings))
                .await
                .map_err(|error| error.to_string())?,
        )?;
    }
    Ok(())
}

pub(crate) async fn sync_gateway_target_runtime_for_config(
    state: &AppState,
    config: &Value,
    save_config: bool,
) -> Result<(), String> {
    let proxy_source = config
        .get("gateway_proxy_headers")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let proxy_requested = normalize_disabled_hosts_config(&proxy_source);
    let proxy_compiled = compile_gateway_proxy_headers_state(config, &proxy_requested);

    let mut effective_config = config.clone();
    if save_config {
        ensure_object(&mut effective_config).insert(
            "gateway_proxy_headers".to_string(),
            proxy_compiled.config.clone(),
        );
    }

    let host_response_source = effective_config
        .get("gateway_host_response")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let host_response_requested = normalize_disabled_hosts_config(&host_response_source);
    let host_response_compiled =
        compile_gateway_host_response_state(&effective_config, &host_response_requested);

    if save_config {
        ensure_object(&mut effective_config).insert(
            "gateway_host_response".to_string(),
            host_response_compiled.config.clone(),
        );
        state
            .redis
            .save_config(&effective_config)
            .await
            .map_err(|error| error.to_string())?;
    }

    state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, &proxy_compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_proxy_headers_runtime(state, &proxy_compiled.runtime).await?;

    state
        .redis
        .set_json_value(
            GATEWAY_HOST_RESPONSE_RUNTIME_KEY,
            &host_response_compiled.runtime,
        )
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_host_response_runtime(state, &effective_config, &host_response_compiled.runtime)
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

pub(super) async fn sync_gateway_runtime(state: &AppState, config: &Value) -> Result<(), String> {
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
    ensure_go_success(
        state
            .go_backend
            .set_gateway_portal_config(&portal)
            .await
            .map_err(|error| error.to_string())?,
    )
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
        .redis
        .get_string_value(flag_key)
        .await
        .map_err(|error| error.to_string())?
        .as_deref()
        == Some("1")
    {
        return Ok(false);
    }

    let host_mappings = config_host_mappings(config);
    ensure_go_success(
        state
            .go_backend
            .set_host_rules(&build_host_rules_payload(&host_mappings))
            .await
            .map_err(|error| error.to_string())?,
    )?;

    if let Err(error) = state.redis.set_string_value(flag_key, "1").await {
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
