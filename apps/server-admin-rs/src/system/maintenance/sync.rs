use super::*;

pub(super) async fn sync_runtime_after_import(
    state: &AppState,
    translator: &Translator,
    previous_config: &Value,
) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut synced_steps = Vec::new();
    let config = match state.storage.store.get_config().await {
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
    let restore_waf_early = should_restore_waf_before_other_runtime_steps(&config);
    let waf_label = maintenance_backup_text(translator, "syncSteps.wafRuntime");

    if restore_waf_early {
        record_waf_restore_result(
            waf::restore_waf_runtime_after_import(state, &config).await,
            &waf_label,
            &mut warnings,
            &mut synced_steps,
        );
    }

    let run_mode_label = maintenance_backup_text(translator, "syncSteps.runModeGatewayRoutes");
    match runtime_config::apply_run_type_config_with_host_rules_lock(state, &config, run_type).await
    {
        Ok(()) => synced_steps.push(run_mode_label),
        Err(error) => warnings.push(format!(
            "{}: {}",
            run_mode_label,
            localize_backup_error_message(translator, &error)
        )),
    }

    let trusted_ips_label = maintenance_backup_text(translator, "syncSteps.trustedClientIps");
    match whitelist::sync_reverse_proxy_trusted_ips_required(state).await {
        Ok(()) => synced_steps.push(trusted_ips_label),
        Err(error) => warnings.push(format!("{trusted_ips_label}: {error}")),
    }

    let gateway_logging_label = maintenance_backup_text(translator, "syncSteps.gatewayLogging");
    let gateway_logging = config.get("gateway_logging").cloned().unwrap_or_else(|| {
        json!({
            "enabled": true,
            "record_localhost": false,
            "max_days": 7
        })
    });
    match state
        .gateway
        .client
        .set_gateway_logging_config_status(&gateway_logging)
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

    let gateway_memory_label = maintenance_backup_text(translator, "syncSteps.gatewayMemory");
    let _memory_update_guard = state.gateway.memory_update_lock.lock().await;
    match state.storage.store.get_config().await {
        Ok(current_config) => {
            match gateway_settings::sync_gateway_memory_runtime(state, &current_config).await {
                Ok(_) => synced_steps.push(gateway_memory_label),
                Err(error) => warnings.push(format!("{gateway_memory_label}: {error}")),
            }
        }
        Err(error) => warnings.push(format!("{gateway_memory_label}: {error}")),
    }
    drop(_memory_update_guard);

    if !restore_waf_early {
        record_waf_restore_result(
            waf::restore_waf_runtime_after_import(state, &config).await,
            &waf_label,
            &mut warnings,
            &mut synced_steps,
        );
    }

    let ssl_label = maintenance_backup_text(translator, "syncSteps.sslDeployment");
    match ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await {
        Ok(()) => synced_steps.push(ssl_label),
        Err(error) => warnings.push(format!("{ssl_label}: {error}")),
    }

    let auto_https_label = maintenance_backup_text(translator, "syncSteps.autoHttps");
    let auto_https_config = auto_https::normalize_auto_https_config(config.get("auto_https"));
    let auto_https_runtime = state
        .auto_https
        .apply_config(
            auto_https_config
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await;
    if auto_https_runtime.get("status").and_then(Value::as_str) == Some("error") {
        warnings.push(format!(
            "{}: {}",
            auto_https_label,
            auto_https_runtime
                .get("last_error")
                .and_then(Value::as_str)
                .unwrap_or("runtime apply failed")
        ));
    } else {
        synced_steps.push(auto_https_label);
    }

    let smart_connect_label = maintenance_backup_text(translator, "syncSteps.smartConnect");
    match runtime_config::sync_smart_connect_after_import(state, &config).await {
        Ok(()) => synced_steps.push(smart_connect_label),
        Err(error) => warnings.push(format!("{smart_connect_label}: {error}")),
    }

    let fnos_icon_label = maintenance_backup_text(translator, "syncSteps.fnosPortIconHijack");
    match runtime_config::sync_fnos_port_icon_hijack_after_import(state, &config).await {
        Ok(()) => synced_steps.push(fnos_icon_label),
        Err(error) => warnings.push(format!("{fnos_icon_label}: {error}")),
    }

    let fnos_network_label = maintenance_backup_text(translator, "syncSteps.fnosNetworkTuning");
    match runtime_config::sync_fnos_network_tuning_after_import(
        state,
        previous_config,
        &config,
        translator,
    )
    .await
    {
        Ok(()) => synced_steps.push(fnos_network_label),
        Err(error) => warnings.push(format!("{fnos_network_label}: {error}")),
    }

    let locale_label = maintenance_backup_text(translator, "syncSteps.locale");
    let locale = normalize_locale_config(config.get("locale").unwrap_or(&Value::Null));
    match state.gateway.client.set_locale_config(&locale).await {
        Ok((status, value))
            if status == reqwest::StatusCode::NOT_FOUND
                || (status.is_success()
                    && value.get("success").and_then(Value::as_bool) != Some(false)) =>
        {
            synced_steps.push(locale_label);
        }
        Ok((status, value)) => warnings.push(format!(
            "{}: {}",
            locale_label,
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or(status.as_str())
        )),
        Err(error) => warnings.push(format!("{locale_label}: {error}")),
    }

    state.fnos_connect_waf_notify.notify_one();

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

pub(super) fn should_restore_waf_before_other_runtime_steps(config: &Value) -> bool {
    !config
        .get("waf")
        .and_then(|waf| waf.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn record_waf_restore_result(
    result: anyhow::Result<Value>,
    label: &str,
    warnings: &mut Vec<String>,
    synced_steps: &mut Vec<String>,
) {
    match result {
        Ok(_) => synced_steps.push(label.to_string()),
        Err(error) => warnings.push(format!("{label}: {error}")),
    }
}
