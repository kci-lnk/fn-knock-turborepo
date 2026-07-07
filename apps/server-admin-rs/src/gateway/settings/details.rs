use super::*;

pub(super) async fn build_gateway_settings_response(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    build_gateway_settings_response_from_config(state, config).await
}

pub(super) async fn build_gateway_settings_response_from_config(
    state: &AppState,
    config: Value,
) -> anyhow::Result<Value> {
    let visibility_runtime = state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_visibility_runtime);
    let proxy_headers_runtime = state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_proxy_headers_runtime);
    let host_response_runtime = state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_host_response_runtime);

    let subdomain = config
        .get("subdomain_mode")
        .cloned()
        .unwrap_or_else(default_subdomain_mode);
    let reverse_proxy_throttle = normalize_reverse_proxy_throttle(
        config
            .get("reverse_proxy_throttle")
            .unwrap_or(&default_reverse_proxy_throttle()),
    );
    let visibility_config = normalize_gateway_visibility(
        config
            .get("gateway_visibility")
            .unwrap_or(&default_gateway_visibility()),
    );
    let proxy_headers_config = normalize_disabled_hosts_config(
        config
            .get("gateway_proxy_headers")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let host_response_config = normalize_disabled_hosts_config(
        config
            .get("gateway_host_response")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let proxy_headers_config = sanitize_disabled_hosts_config(&config, &proxy_headers_config);
    let host_response_config = sanitize_disabled_hosts_config(&config, &host_response_config);
    let crawler_blocker = normalize_gateway_crawler_blocker(
        config
            .get("gateway_crawler_blocker")
            .unwrap_or(&default_gateway_crawler_blocker()),
    );
    let portal = normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    );
    let host_mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible_hosts = visible_host_mappings(&host_mappings);
    let proxy_header_items =
        build_gateway_proxy_header_items(&visible_hosts, &proxy_headers_config);
    let host_response_items =
        build_gateway_host_response_items(&visible_hosts, &host_response_config);

    Ok(json!({
        "auth_cache_ttl_seconds": subdomain
            .get("auth_cache_ttl_seconds")
            .and_then(number_floor)
            .unwrap_or(1),
        "auth_cache_unauthorized_ttl_seconds": subdomain
            .get("auth_cache_unauthorized_ttl_seconds")
            .and_then(number_floor)
            .unwrap_or(1),
        "reverse_proxy_throttle": reverse_proxy_throttle,
        "visibility": build_gateway_visibility_summary(&visibility_config, &visibility_runtime),
        "proxy_headers": build_gateway_proxy_headers_summary(&proxy_header_items, &proxy_headers_runtime),
        "host_response": build_gateway_host_response_summary(&host_response_items, &host_response_runtime),
        "crawler_blocker": crawler_blocker,
        "portal": portal,
    }))
}

pub(super) async fn get_gateway_visibility_details(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let visibility_config = normalize_gateway_visibility(
        config
            .get("gateway_visibility")
            .unwrap_or(&default_gateway_visibility()),
    );
    let runtime = state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_visibility_runtime);
    Ok(json!({
        "config": visibility_config,
        "summary": build_gateway_visibility_summary(&visibility_config, &runtime),
    }))
}

pub(super) async fn update_gateway_visibility_inner(
    state: &AppState,
    body: &Value,
) -> Result<Value, String> {
    let Some(object) = body.as_object() else {
        return Err("Gateway visibility payload must be an object".to_string());
    };
    let compiled = compile_gateway_visibility_config(state, object).await?;
    let mut next_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    ensure_object(&mut next_config)
        .insert("gateway_visibility".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_visibility_runtime(state, &compiled.runtime).await?;

    Ok(json!({
        "config": compiled.config,
        "summary": build_gateway_visibility_summary(&compiled.config, &compiled.runtime),
    }))
}

pub(super) async fn get_gateway_proxy_headers_details(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let proxy_config = normalize_disabled_hosts_config(
        config
            .get("gateway_proxy_headers")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let sanitized_config = sanitize_disabled_hosts_config(&config, &proxy_config);
    let host_mappings = config_host_mappings(&config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_proxy_header_items(&visible_hosts, &sanitized_config);
    let runtime = state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_proxy_headers_runtime);

    Ok(json!({
        "config": sanitized_config,
        "availability": build_proxy_headers_availability(&config, translator),
        "items": items,
        "summary": build_gateway_proxy_headers_summary(&items, &runtime),
    }))
}

pub(super) async fn update_gateway_proxy_headers_inner(
    state: &AppState,
    previous_config: &Value,
    body: &Value,
) -> Result<Value, String> {
    let requested = disabled_hosts_config_from_body(body)?;
    let compiled = compile_gateway_proxy_headers_state(previous_config, &requested);
    let mut next_config = previous_config.clone();
    ensure_object(&mut next_config)
        .insert("gateway_proxy_headers".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_proxy_headers_runtime(state, &compiled.runtime).await?;
    let translator = Translator::from_state(state).await;
    get_gateway_proxy_headers_details(state, &translator)
        .await
        .map_err(|error| error.to_string())
}

pub(super) async fn get_gateway_host_response_details(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let host_response_config = normalize_disabled_hosts_config(
        config
            .get("gateway_host_response")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let sanitized_config = sanitize_disabled_hosts_config(&config, &host_response_config);
    let host_mappings = config_host_mappings(&config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_host_response_items(&visible_hosts, &sanitized_config);
    let runtime = state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_host_response_runtime);

    Ok(json!({
        "config": sanitized_config,
        "availability": build_host_response_availability(&config, translator),
        "items": items,
        "summary": build_gateway_host_response_summary(&items, &runtime),
    }))
}

pub(super) async fn update_gateway_host_response_inner(
    state: &AppState,
    previous_config: &Value,
    body: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let requested = disabled_hosts_config_from_body(body)?;
    let compiled = compile_gateway_host_response_state(previous_config, &requested);
    let mut next_config = previous_config.clone();
    ensure_object(&mut next_config)
        .insert("gateway_host_response".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_host_response_runtime(state, previous_config, &compiled.runtime).await?;
    get_gateway_host_response_details(state, translator)
        .await
        .map_err(|error| error.to_string())
}
