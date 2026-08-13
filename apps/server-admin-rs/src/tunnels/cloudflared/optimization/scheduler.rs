use super::*;

pub(super) async fn scheduled_tick(state: &AppState) -> Result<(), CloudflareApiError> {
    let guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(state).await;
    if managed.get("mode").and_then(Value::as_str) != Some("managed") {
        return Ok(());
    }
    let Some(api) = api_for_background(state).await? else {
        return Ok(());
    };
    let mut ownership = load_managed_state(state).await;
    let mut runtime = load_runtime(state).await;
    if managed.get("optimizationEnabled").and_then(Value::as_bool) != Some(true) {
        let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
        if !zone_id.is_empty() {
            let local = state
                .storage
                .store
                .get_config()
                .await
                .map_err(local_error_display)?;
            reconcile_optimization_host_membership(
                state,
                &api,
                zone_id,
                &mut ownership,
                &local,
                &managed_instance_id(&managed),
            )
            .await?;
        }
        return Ok(());
    }
    if ownership
        .pointer("/optimization/publishSuppressed")
        .is_none()
    {
        let suppression = legacy_publish_suppression(&ownership, &runtime);
        ensure_nested_object(&mut ownership, &["optimization"])
            .insert("publishSuppressed".to_string(), json!(suppression));
        save_managed_state(state, &ownership).await?;
    }
    reconcile_resources(state, &api, &managed, &mut ownership, false, None).await?;
    let now = time_utils::now_ms();

    let last_health = runtime
        .get("lastHealthAtMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if now.saturating_sub(last_health) >= HEALTH_INTERVAL_MS {
        run_health_check(state, &api, &managed, &mut ownership, &mut runtime).await?;
    }

    let next_scan = runtime
        .get("nextFullScanAtMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let scan_due = next_scan == 0 || now >= next_scan;
    if scan_due && scan_validation_hostname(&ownership).is_none() {
        // Reconciliation intentionally returns while Cloudflare is still
        // validating a Custom Hostname or certificate. This is an expected
        // provisioning state, so keep the scan due and retry on the next
        // scheduler tick instead of recording a failed scheduled scan.
        ensure_object(&mut runtime).insert("lastError".to_string(), Value::Null);
        return save_runtime(state, &runtime).await;
    }
    let confirmation_due = !scan_due
        && runtime
            .pointer("/pendingCandidate/confirmAtMs")
            .and_then(Value::as_i64)
            .is_some_and(|confirm_at| now >= confirm_at);
    let confirmation = if confirmation_due {
        match confirmation_snapshot(&ownership, &runtime) {
            Ok(value) => value,
            Err(error) => {
                ensure_object(&mut runtime).remove("pendingCandidate");
                ensure_object(&mut runtime)
                    .insert("lastError".to_string(), json!(error.to_string()));
                save_runtime(state, &runtime).await?;
                return Ok(());
            }
        }
    } else {
        None
    };
    if !scan_due && confirmation.is_none() {
        ensure_object(&mut runtime).insert("lastError".to_string(), Value::Null);
        return save_runtime(state, &runtime).await;
    }
    ensure_object(&mut runtime).insert("lastError".to_string(), Value::Null);
    save_runtime(state, &runtime).await?;
    drop(guard);

    if scan_due {
        run_scheduled_scan(state).await
    } else if let Some(snapshot) = confirmation {
        run_scheduled_confirmation(state, snapshot).await
    } else {
        Ok(())
    }
}
async fn run_scheduled_scan(state: &AppState) -> Result<(), CloudflareApiError> {
    let _scan_guard = state.tunnel.cloudflared_scan_lock.lock().await;
    let scan = time::timeout(Duration::from_secs(180), run_scan(state, None, None))
        .await
        .map_err(|_| local_error("Optimization scan exceeded the three-minute limit"))??;

    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(state).await;
    if managed.get("mode").and_then(Value::as_str) != Some("managed")
        || !optimization_is_enabled(&managed)
    {
        return Ok(());
    }
    let settings = load_source_settings(state).await?;
    if source_settings_fingerprint(&settings) != scan.source_fingerprint {
        let mut runtime = load_runtime(state).await;
        let object = ensure_object(&mut runtime);
        object.insert("nextFullScanAtMs".to_string(), json!(0));
        object.insert(
            "lastError".to_string(),
            json!("Candidate sources changed during the scheduled scan; discarded stale results"),
        );
        return save_runtime(state, &runtime).await;
    }
    let mut ownership = load_managed_state(state).await;
    let mut runtime = load_runtime(state).await;
    let runtime_object = ensure_object(&mut runtime);
    runtime_object.insert("lastVantage".to_string(), scan.vantage);
    runtime_object.insert(
        "lastSourceWarnings".to_string(),
        json!(scan.source_warnings),
    );
    runtime_object.insert(
        "lastResolverDiagnostics".to_string(),
        json!(scan.resolver_diagnostics),
    );
    runtime_object.insert(
        "lastResolutionPath".to_string(),
        json!(scan.resolution_path),
    );
    apply_automatic_scan_result(&mut ownership, &mut runtime, &scan.candidates);
    let completed_at = time_utils::now_ms();
    let next = completed_at + WEEK_MS + weekly_jitter_ms();
    let runtime_object = ensure_object(&mut runtime);
    runtime_object.insert("lastFullScanAtMs".to_string(), json!(completed_at));
    runtime_object.insert(
        "lastFullScanAt".to_string(),
        json!(time_utils::iso_from_ms(completed_at)),
    );
    runtime_object.insert("nextFullScanAtMs".to_string(), json!(next));
    runtime_object.insert(
        "nextFullScanAt".to_string(),
        json!(time_utils::iso_from_ms(next)),
    );
    runtime_object.insert("lastError".to_string(), Value::Null);
    save_runtime(state, &runtime).await
}

async fn run_health_check(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &mut Value,
) -> Result<(), CloudflareApiError> {
    let now = time_utils::now_ms();
    let selected_ip = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Ipv4Addr>().ok());
    let host = optimized_health_hostname(ownership);
    let success = match (selected_ip, host.as_deref()) {
        (Some(ip), Some(host)) => probe_custom_hostname(host, ip).await.is_ok(),
        _ => true,
    };
    let failures = if success {
        0
    } else {
        runtime
            .get("healthFailures")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1)
    };
    let object = ensure_object(runtime);
    object.insert("lastHealthAtMs".to_string(), json!(now));
    object.insert(
        "lastHealthAt".to_string(),
        json!(time_utils::iso_from_ms(now)),
    );
    object.insert("healthFailures".to_string(), json!(failures));
    if failures >= 3 {
        if try_verified_backup_candidate(
            state,
            api,
            managed,
            ownership,
            runtime,
            selected_ip,
            host.as_deref(),
        )
        .await?
        {
            let object = ensure_object(runtime);
            object.insert("healthFailures".to_string(), json!(0));
            object.insert("lastSwitchReason".to_string(), json!("health-failover"));
        } else {
            fallback_to_wildcard(state, api, managed, ownership).await?;
            let object = ensure_object(runtime);
            object.remove("pendingCandidate");
            object.insert(
                "lastError".to_string(),
                json!(
                    "Preferred edge failed three health checks; wildcard Tunnel fallback activated"
                ),
            );
            object.insert("lastSwitchReason".to_string(), json!("health-fallback"));
        }
    }
    save_runtime(state, runtime).await
}

#[allow(clippy::too_many_arguments)]
async fn try_verified_backup_candidate(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &Value,
    current_ip: Option<Ipv4Addr>,
    hostname: Option<&str>,
) -> Result<bool, CloudflareApiError> {
    let Some(hostname) = hostname else {
        return Ok(false);
    };
    let candidates = serde_json::from_value::<Vec<OptimizationCandidate>>(
        runtime
            .get("lastCandidates")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .unwrap_or_default();
    for candidate in candidates {
        let Ok(ip) = candidate.ip.parse::<Ipv4Addr>() else {
            continue;
        };
        if Some(ip) == current_ip || candidate.verified_at.is_none() {
            continue;
        }
        if probe_latency(ip).await.is_none() || probe_custom_hostname(hostname, ip).await.is_err() {
            continue;
        }
        let previous = ownership.pointer("/optimization/selected").cloned();
        set_selected(ownership, &candidate, "health-failover");
        save_managed_state(state, ownership).await?;
        if let Err(error) = reconcile_resources(state, api, managed, ownership, true, None).await {
            restore_selected(ownership, previous);
            save_managed_state(state, ownership).await?;
            return Err(error);
        }
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn apply_automatic_scan_result(
    ownership: &mut Value,
    runtime: &mut Value,
    candidates: &[OptimizationCandidate],
) {
    ensure_object(runtime).insert("lastCandidates".to_string(), json!(candidates));
    let Some(current_ip) = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
    else {
        ensure_object(runtime).remove("pendingCandidate");
        ensure_object(runtime).insert(
            "lastSwitchReason".to_string(),
            json!("awaiting-initial-manual-selection"),
        );
        return;
    };
    let Some(current) = candidates
        .iter()
        .find(|candidate| candidate.ip == current_ip)
    else {
        ensure_object(runtime).remove("pendingCandidate");
        ensure_object(runtime).insert(
            "lastError".to_string(),
            json!("Current preferred IP could not be measured; automatic switching was skipped"),
        );
        return;
    };
    let Some(best) = candidates
        .iter()
        .find(|candidate| candidate.ip != current_ip)
    else {
        ensure_object(runtime).remove("pendingCandidate");
        return;
    };
    if !score_is_15_percent_better(best.score, current.score) {
        ensure_object(runtime).remove("pendingCandidate");
        return;
    }
    let now = time_utils::now_ms();
    ensure_object(runtime).insert(
        "pendingCandidate".to_string(),
        json!({
            "candidate": best,
            "firstSeenAtMs": now,
            "confirmAtMs": now + CONFIRMATION_DELAY_MS,
        }),
    );
}

fn confirmation_snapshot(
    ownership: &Value,
    runtime: &Value,
) -> Result<Option<ConfirmationSnapshot>, CloudflareApiError> {
    let Some(pending) = runtime
        .pointer("/pendingCandidate/candidate")
        .cloned()
        .and_then(|value| serde_json::from_value::<OptimizationCandidate>(value).ok())
    else {
        return Ok(None);
    };
    let current = ownership
        .pointer("/optimization/selected")
        .cloned()
        .and_then(|value| serde_json::from_value::<OptimizationCandidate>(value).ok())
        .ok_or_else(|| local_error("Current optimization candidate is unavailable"))?;
    pending
        .ip
        .parse::<Ipv4Addr>()
        .map_err(|_| local_error("Pending optimization candidate is invalid"))?;
    current
        .ip
        .parse::<Ipv4Addr>()
        .map_err(|_| local_error("Current optimization candidate is invalid"))?;
    let hostname = optimized_health_hostname(ownership)
        .ok_or_else(|| local_error("No active optimized hostname is available for confirmation"))?;
    Ok(Some(ConfirmationSnapshot {
        pending,
        current,
        hostname,
        selected_at: ownership
            .pointer("/optimization/selected/selectedAt")
            .and_then(Value::as_str)
            .map(str::to_string),
    }))
}

async fn remeasure_candidate(
    mut candidate: OptimizationCandidate,
    hostname: &str,
) -> Option<OptimizationCandidate> {
    let ip = candidate.ip.parse::<Ipv4Addr>().ok()?;
    let metrics = probe_latency_metrics(ip).await?;
    if metrics.loss_ratio > 1.0 / 3.0 {
        return None;
    }
    let mut downloads = Vec::new();
    for _ in 0..2 {
        if let Some(mbps) = probe_download(ip, DOWNLOAD_BYTES).await {
            downloads.push(mbps);
        }
    }
    downloads.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let download_mbps = median(&downloads)?;
    let business = probe_custom_hostname_details(hostname, ip).await.ok()?;
    candidate.median_latency_ms = metrics.median_latency_ms;
    candidate.jitter_ms = metrics.jitter_ms;
    candidate.loss_ratio = metrics.loss_ratio;
    candidate.download_mbps = download_mbps;
    candidate.score = score_candidate(
        metrics.median_latency_ms,
        metrics.jitter_ms,
        metrics.loss_ratio,
        download_mbps,
    );
    candidate.verified_at = Some(time_utils::now_iso());
    candidate.colo = metrics.colo;
    candidate.cf_ray = metrics.cf_ray;
    candidate.business_hostname = Some(hostname.to_string());
    candidate.business_status = Some(business.status);
    candidate.business_colo = business.colo;
    candidate.business_cf_ray = business.cf_ray;
    candidate.business_validated = true;
    Some(candidate)
}

async fn run_scheduled_confirmation(
    state: &AppState,
    snapshot: ConfirmationSnapshot,
) -> Result<(), CloudflareApiError> {
    let (pending, current) = tokio::join!(
        remeasure_candidate(snapshot.pending.clone(), &snapshot.hostname),
        remeasure_candidate(snapshot.current.clone(), &snapshot.hostname),
    );

    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(state).await;
    if managed.get("mode").and_then(Value::as_str) != Some("managed")
        || !optimization_is_enabled(&managed)
    {
        return Ok(());
    }
    let Some(api) = api_for_background(state).await? else {
        return Ok(());
    };
    let mut ownership = load_managed_state(state).await;
    let mut runtime = load_runtime(state).await;
    let pending_is_current = runtime
        .pointer("/pendingCandidate/candidate/ip")
        .and_then(Value::as_str)
        == Some(snapshot.pending.ip.as_str());
    let selected_is_current = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
        == Some(snapshot.current.ip.as_str())
        && ownership
            .pointer("/optimization/selected/selectedAt")
            .and_then(Value::as_str)
            == snapshot.selected_at.as_deref();
    if !pending_is_current || !selected_is_current {
        return Ok(());
    }
    ensure_object(&mut runtime).remove("pendingCandidate");
    let (Some(pending), Some(current)) = (pending, current) else {
        ensure_object(&mut runtime).insert(
            "lastError".to_string(),
            json!("Automatic candidate confirmation failed; current route was left unchanged"),
        );
        return save_runtime(state, &runtime).await;
    };
    if score_is_15_percent_better(pending.score, current.score) {
        let previous = ownership.pointer("/optimization/selected").cloned();
        set_selected(&mut ownership, &pending, "automatic");
        save_managed_state(state, &ownership).await?;
        if let Err(error) =
            reconcile_resources(state, &api, &managed, &mut ownership, true, None).await
        {
            restore_selected(&mut ownership, previous);
            save_managed_state(state, &ownership).await?;
            return Err(error);
        }
        ensure_object(&mut runtime)
            .insert("lastSwitchReason".to_string(), json!("automatic-confirmed"));
    }
    ensure_object(&mut runtime).insert("lastError".to_string(), Value::Null);
    save_runtime(state, &runtime).await
}

fn set_selected(ownership: &mut Value, candidate: &OptimizationCandidate, source: &str) {
    let mut selected = serde_json::to_value(candidate).unwrap_or_else(|_| json!({}));
    ensure_object(&mut selected).insert("selectedAt".to_string(), json!(time_utils::now_iso()));
    ensure_object(&mut selected).insert("source".to_string(), json!(source));
    ensure_nested_object(ownership, &["optimization"]).insert("selected".to_string(), selected);
}

fn restore_selected(ownership: &mut Value, previous: Option<Value>) {
    let optimization = ensure_nested_object(ownership, &["optimization"]);
    match previous {
        Some(value) => {
            optimization.insert("selected".to_string(), value);
        }
        None => {
            optimization.remove("selected");
        }
    }
}
