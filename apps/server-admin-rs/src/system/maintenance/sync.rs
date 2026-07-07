use super::*;

pub(super) async fn sync_runtime_after_import(
    state: &AppState,
    translator: &Translator,
) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut synced_steps = Vec::new();
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            warnings.push(format!(
                "{}: {error}",
                maintenance_backup_text(translator, "syncSteps.runModeGatewayRoutes")
            ));
            return (warnings, synced_steps);
        }
    };
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);

    let run_mode_label = maintenance_backup_text(translator, "syncSteps.runModeGatewayRoutes");
    match runtime_config::apply_run_type_config(state, &config, run_type).await {
        Ok(()) => synced_steps.push(run_mode_label),
        Err(error) => warnings.push(format!(
            "{}: {}",
            run_mode_label,
            localize_backup_error_message(translator, &error)
        )),
    }

    if run_type == 0 {
        let whitelist_label = maintenance_backup_text(translator, "syncSteps.directModeWhitelist");
        match sync_direct_mode_whitelist_after_import(state).await {
            Ok(()) => synced_steps.push(whitelist_label),
            Err(error) => warnings.push(format!("{whitelist_label}: {error}")),
        }
    }

    let gateway_logging_label = maintenance_backup_text(translator, "syncSteps.gatewayLogging");
    let gateway_logging = config.get("gateway_logging").cloned().unwrap_or_else(|| {
        json!({
            "enabled": true,
            "max_days": 7
        })
    });
    match state
        .go_backend
        .request_json_with_status(
            axum::http::Method::POST,
            "/api/logging",
            Some(&gateway_logging),
        )
        .await
    {
        Ok((status, value))
            if status.is_success()
                && value.get("success").and_then(Value::as_bool) != Some(false) =>
        {
            synced_steps.push(gateway_logging_label);
        }
        Ok((status, value)) => warnings.push(format!(
            "{}: {}",
            gateway_logging_label,
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| status.to_string())
        )),
        Err(error) => warnings.push(format!("{gateway_logging_label}: {error}")),
    }

    let ssl_label = maintenance_backup_text(translator, "syncSteps.sslDeployment");
    match ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await {
        Ok(()) => synced_steps.push(ssl_label),
        Err(error) => warnings.push(format!("{ssl_label}: {error}")),
    }

    let cleanup_label = maintenance_backup_text(translator, "syncSteps.legacyAuthLogCleanup");
    match crate::cleanup_legacy_auth_log_storage(state).await {
        Ok(()) => synced_steps.push(cleanup_label),
        Err(error) => warnings.push(format!("{cleanup_label}: {error}")),
    }

    let monitor_label = maintenance_backup_text(translator, "syncSteps.systemResourceMonitorReset");
    system_monitor::reset_states(state).await;
    synced_steps.push(monitor_label);

    (warnings, synced_steps)
}

pub(super) async fn sync_direct_mode_whitelist_after_import(
    state: &AppState,
) -> anyhow::Result<()> {
    let records = state.redis.list_whitelist_active_concrete_targets().await?;
    for record in records {
        let value = state.go_backend.allow_ip(&record.target).await?;
        if value.get("success").and_then(Value::as_bool) == Some(false) {
            anyhow::bail!(
                "{}",
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("allow ip failed")
            );
        }
    }
    Ok(())
}
