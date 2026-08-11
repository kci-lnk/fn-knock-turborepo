use super::*;

pub(crate) fn openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(start_scan))
        .routes(routes!(get_scan))
        .routes(routes!(cancel_scan))
        .routes(routes!(apply_optimization))
        .routes(routes!(fallback_optimization))
        .routes(routes!(update_source_settings))
        .routes(routes!(update_domain_mode))
}

#[utoipa::path(put, path = "/api/admin/cloudflared/optimization/settings", tag = "cloudflared", operation_id = "put_api_admin_cloudflared_optimization_settings", responses((status = 200, description = "Updated optimization source settings")))]
async fn update_source_settings(
    State(state): State<AppState>,
    Json(body): Json<OptimizationSourceSettings>,
) -> Response {
    let settings = match normalize_source_settings(body) {
        Ok(value) => value,
        Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
    };
    if let Err(error) = state
        .storage
        .store
        .set_json_value(OPTIMIZATION_SETTINGS_KEY, &json!(settings.clone()))
        .await
    {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save optimization candidate sources: {error}"),
        );
    }
    let mut runtime = load_runtime(&state).await;
    let runtime_object = ensure_object(&mut runtime);
    runtime_object.remove("pendingCandidate");
    runtime_object.insert("nextFullScanAtMs".to_string(), json!(0));
    runtime_object.insert(
        "lastSwitchReason".to_string(),
        json!("candidate-sources-updated"),
    );
    if let Err(error) = save_runtime(&state, &runtime).await {
        return api_error_response(error);
    }
    state.tunnel.cloudflared_schedule_notify.notify_one();
    response::ok(public_source_settings(&settings)).into_response()
}

#[utoipa::path(put, path = "/api/admin/cloudflared/optimization/domains/{hostname}", tag = "cloudflared", operation_id = "put_api_admin_cloudflared_optimization_domains_hostname", params(("hostname" = String, Path, description = "Configured hostname")), responses((status = 200, description = "Updated optimization domain mode")))]
async fn update_domain_mode(
    State(state): State<AppState>,
    Path(hostname): Path<String>,
    Json(body): Json<UpdateOptimizationDomainRequest>,
) -> Response {
    let hostname = match normalize_candidate_hostname(&hostname) {
        Ok(value) => value,
        Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
    };
    if !matches!(body.mode.as_str(), "optimize" | "external") {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Optimization domain mode must be optimize or external",
        );
    }

    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let local = match state.storage.store.get_config().await {
        Ok(value) => value,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load configured hostnames: {error}"),
            );
        }
    };
    let configured = configured_hosts(&local).into_iter().collect::<HashSet<_>>();
    if !configured.contains(&hostname) {
        return response::error(
            StatusCode::NOT_FOUND,
            "The optimization hostname is no longer configured",
        );
    }

    let mut settings = match load_domain_settings(&state).await {
        Ok(value) => value,
        Err(error) => return api_error_response(error),
    };
    if body.mode == "external" {
        if !settings.external_hostnames.contains(&hostname) {
            settings.external_hostnames.push(hostname.clone());
        }
        settings.external_hostnames.sort();
        settings.external_hostnames.dedup();
    } else {
        settings
            .external_hostnames
            .retain(|value| value != &hostname);
    }

    if let Err(error) = state
        .storage
        .store
        .set_json_value(OPTIMIZATION_DOMAIN_SETTINGS_KEY, &json!(settings.clone()))
        .await
    {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save optimization domain mode: {error}"),
        );
    }

    let mut cleanup_pending = false;
    if body.mode == "external" {
        let managed = load_managed_config(&state).await;
        let mut ownership = load_managed_state(&state).await;
        let tracked_host = ownership
            .pointer(&format!(
                "/optimization/customHostnames/{}",
                json_pointer_escape(&hostname)
            ))
            .cloned();
        if let Some(host) = tracked_host {
            if !host_has_tracked_remote_resources(&host) {
                if let Err(error) =
                    forget_optimization_host_state(&state, &mut ownership, &hostname).await
                {
                    cleanup_pending = true;
                    tracing::warn!(%error, %hostname, "optimization hostname was marked external; local cleanup will be retried");
                }
            } else {
                let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
                let cleanup = match api_for_background(&state).await {
                    Ok(Some(api)) if !zone_id.is_empty() => {
                        relinquish_optimization_host(
                            &state,
                            &api,
                            zone_id,
                            &mut ownership,
                            &hostname,
                            &managed_instance_id(&managed),
                        )
                        .await
                    }
                    Ok(_) => Err(local_error(
                        "Cloudflare API Token and Zone are required to clean up this hostname",
                    )),
                    Err(error) => Err(error),
                };
                if let Err(error) = cleanup {
                    cleanup_pending = true;
                    tracing::warn!(%error, %hostname, "optimization hostname was marked external; remote cleanup will be retried");
                }
            }
        }
    }
    state.tunnel.cloudflared_schedule_notify.notify_one();
    response::ok(json!({
        "hostname": hostname,
        "mode": body.mode,
        "cleanupPending": cleanup_pending,
    }))
    .into_response()
}

pub(super) fn public_source_settings(settings: &OptimizationSourceSettings) -> Value {
    let enabled = settings
        .builtin_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    json!({
        "officialRanges": settings.official_ranges,
        "builtins": BUILTIN_CANDIDATE_SOURCES.iter().map(|source| json!({
            "id": source.id,
            "hostname": source.hostname,
            "category": source.category,
            "enabled": enabled.contains(source.id),
        })).collect::<Vec<_>>(),
        "customHostnames": settings.custom_hostnames,
        "maxCustomHostnames": MAX_CUSTOM_SOURCE_HOSTNAMES,
        "resolutionPolicy": "cloudflare-google-doh-intersection",
        "publishPolicy": "extract-ip-only",
    })
}
#[utoipa::path(post, path = "/api/admin/cloudflared/optimization/scans", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_optimization_scans", responses((status = 200, description = "Started optimization scan")))]
async fn start_scan(State(state): State<AppState>) -> Response {
    let managed = load_managed_config(&state).await;
    if !optimization_is_enabled(&managed) {
        return response::error(
            StatusCode::CONFLICT,
            "Enable optimization by previewing and applying a Cloudflare reconcile plan before starting a speed test",
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    let job = json!({
        "id": id,
        "status": "queued",
        "phase": "queued",
        "progress": 0,
        "createdAt": time_utils::now_iso(),
        "startedAt": Value::Null,
        "completedAt": Value::Null,
        "completedAtMs": Value::Null,
        "cancelRequested": false,
        "candidates": [],
        "recommendedIp": Value::Null,
        "vantage": Value::Null,
        "sourceWarnings": [],
        "sourceFingerprint": Value::Null,
        "errorCode": Value::Null,
        "error": Value::Null,
    });
    {
        let mut jobs = state.tunnel.cloudflared_scan_jobs.write().await;
        if let Some(existing) = jobs.values().find(|job| scan_job_active(job)) {
            return response::error(
                StatusCode::CONFLICT,
                format!(
                    "Optimization scan {} is already running",
                    existing.get("id").and_then(Value::as_str).unwrap_or("")
                ),
            );
        }
        if jobs.len() >= 20 {
            let oldest = jobs
                .iter()
                .filter(|(_, value)| !scan_job_active(value))
                .min_by_key(|(_, value)| {
                    value
                        .get("createdAt")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                })
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                jobs.remove(&oldest);
            }
        }
        jobs.insert(id.clone(), job.clone());
    }
    let scan_state = state.clone();
    let scan_id = id.clone();
    state.spawn_background("cloudflare-optimization-scan", async move {
        let _scan_guard = scan_state.tunnel.cloudflared_scan_lock.lock().await;
        update_job(
            &scan_state,
            &scan_id,
            json!({
                "status": "running",
                "phase": "latency",
                "progress": 1,
                "startedAt": time_utils::now_iso(),
            }),
        )
        .await;
        let result = time::timeout(
            Duration::from_secs(180),
            run_scan(&scan_state, Some(&scan_id)),
        )
        .await;
        match result {
            Ok(Ok(_result)) if is_job_cancelled(&scan_state, &scan_id).await => {
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "cancelled",
                        "phase": "cancelled",
                        "completedAt": time_utils::now_iso(),
                    }),
                )
                .await;
            }
            Ok(Ok(result)) => {
                let completed_at_ms = time_utils::now_ms();
                let recommended = result
                    .candidates
                    .first()
                    .map(|candidate| candidate.ip.clone());
                let mut runtime = load_runtime(&scan_state).await;
                let runtime_object = ensure_object(&mut runtime);
                runtime_object.insert("lastVantage".to_string(), result.vantage.clone());
                runtime_object.insert(
                    "lastSourceWarnings".to_string(),
                    json!(result.source_warnings.clone()),
                );
                runtime_object.insert(
                    "lastCandidates".to_string(),
                    json!(result.candidates.clone()),
                );
                let _ = save_runtime(&scan_state, &runtime).await;
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "completed",
                        "phase": "completed",
                        "progress": 100,
                        "completedAt": time_utils::now_iso(),
                        "completedAtMs": completed_at_ms,
                        "candidates": result.candidates,
                        "recommendedIp": recommended,
                        "vantage": result.vantage,
                        "sourceWarnings": result.source_warnings,
                        "sourceFingerprint": result.source_fingerprint,
                    }),
                )
                .await;
            }
            Ok(Err(error)) => {
                let error_code = optimization_scan_error_code(&error);
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "failed",
                        "phase": "failed",
                        "completedAt": time_utils::now_iso(),
                        "errorCode": error_code,
                        "error": error.to_string(),
                    }),
                )
                .await;
            }
            Err(_) => {
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "failed",
                        "phase": "failed",
                        "completedAt": time_utils::now_iso(),
                        "error": "Optimization scan exceeded the three-minute limit",
                    }),
                )
                .await;
            }
        }
    });
    response::ok(job).into_response()
}

#[utoipa::path(get, path = "/api/admin/cloudflared/optimization/scans/{id}", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_optimization_scans_id", params(("id" = String, Path, description = "Scan identifier")), responses((status = 200, description = "Optimization scan")))]
async fn get_scan(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state
        .tunnel
        .cloudflared_scan_jobs
        .read()
        .await
        .get(&id)
        .cloned()
    {
        Some(job) => response::ok(job).into_response(),
        None => response::error(StatusCode::NOT_FOUND, "Optimization scan was not found"),
    }
}

#[utoipa::path(delete, path = "/api/admin/cloudflared/optimization/scans/{id}", tag = "cloudflared", operation_id = "delete_api_admin_cloudflared_optimization_scans_id", params(("id" = String, Path, description = "Scan identifier")), responses((status = 200, description = "Cancelled optimization scan")))]
async fn cancel_scan(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut jobs = state.tunnel.cloudflared_scan_jobs.write().await;
    let Some(job) = jobs.get_mut(&id) else {
        return response::error(StatusCode::NOT_FOUND, "Optimization scan was not found");
    };
    ensure_object(job).insert("cancelRequested".to_string(), json!(true));
    response::success_empty().into_response()
}

#[utoipa::path(post, path = "/api/admin/cloudflared/optimization/apply", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_optimization_apply", responses((status = 200, description = "Applied optimization candidate")))]
async fn apply_optimization(
    State(state): State<AppState>,
    Json(body): Json<ApplyOptimizationRequest>,
) -> Response {
    let job = match state
        .tunnel
        .cloudflared_scan_jobs
        .read()
        .await
        .get(body.scan_id.trim())
        .cloned()
    {
        Some(job) if job.get("status").and_then(Value::as_str) == Some("completed") => job,
        Some(_) => {
            return response::error(StatusCode::CONFLICT, "Optimization scan is not complete");
        }
        None => return response::error(StatusCode::NOT_FOUND, "Optimization scan was not found"),
    };
    let candidates = serde_json::from_value::<Vec<OptimizationCandidate>>(
        job.get("candidates").cloned().unwrap_or_else(|| json!([])),
    )
    .unwrap_or_default();
    let completed_at_ms = job
        .get("completedAtMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if !scan_is_fresh(completed_at_ms, time_utils::now_ms()) {
        return response::error(
            StatusCode::CONFLICT,
            "Optimization scan has expired; run a new speed test before applying a candidate",
        );
    }
    let settings = match load_source_settings(&state).await {
        Ok(value) => value,
        Err(error) => return api_error_response(error),
    };
    let current_fingerprint = source_settings_fingerprint(&settings);
    if job.get("sourceFingerprint").and_then(Value::as_str) != Some(current_fingerprint.as_str()) {
        return response::error(
            StatusCode::CONFLICT,
            "Optimization candidate sources changed after this scan; run a new speed test",
        );
    }
    let requested = body
        .candidate_ip
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| job.get("recommendedIp").and_then(Value::as_str));
    let Some(candidate) = requested.and_then(|ip| candidates.iter().find(|item| item.ip == ip))
    else {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Select a candidate returned by the completed scan",
        );
    };
    if !candidate.business_validated {
        return response::error(
            StatusCode::CONFLICT,
            "The selected candidate has not passed business hostname TLS and SNI validation",
        );
    }
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let mut managed = load_managed_config(&state).await;
    if !optimization_is_enabled(&managed) {
        return response::error(
            StatusCode::CONFLICT,
            "Enable optimization by previewing and applying a Cloudflare reconcile plan before publishing a candidate",
        );
    }
    let api = match api_for_background(&state).await {
        Ok(Some(api)) => api,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                "Cloudflare API Token is not configured",
            );
        }
        Err(error) => return api_error_response(error),
    };
    let mut ownership = load_managed_state(&state).await;
    let previous_selected = ownership.pointer("/optimization/selected").cloned();
    let previous_fallback = ownership.pointer("/optimization/fallbackActive").cloned();
    let previous_publish_suppressed = ownership
        .pointer("/optimization/publishSuppressed")
        .cloned();
    let mut selected = serde_json::to_value(candidate).unwrap_or_else(|_| json!({}));
    ensure_object(&mut selected).insert("selectedAt".to_string(), json!(time_utils::now_iso()));
    ensure_object(&mut selected).insert("source".to_string(), json!("manual"));
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("selected".to_string(), selected.clone());
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("fallbackActive".to_string(), json!(false));
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("publishSuppressed".to_string(), json!(false));
    if let Err(error) = save_managed_state(&state, &ownership).await {
        return api_error_response(error);
    }
    if let Err(error) =
        reconcile_resources(&state, &api, &managed, &mut ownership, true, None).await
    {
        let optimization = ensure_nested_object(&mut ownership, &["optimization"]);
        match previous_selected {
            Some(value) => {
                optimization.insert("selected".to_string(), value);
            }
            None => {
                optimization.remove("selected");
            }
        }
        match previous_fallback {
            Some(value) => {
                optimization.insert("fallbackActive".to_string(), value);
            }
            None => {
                optimization.remove("fallbackActive");
            }
        }
        match previous_publish_suppressed {
            Some(value) => {
                optimization.insert("publishSuppressed".to_string(), value);
            }
            None => {
                optimization.remove("publishSuppressed");
            }
        }
        let _ = save_managed_state(&state, &ownership).await;
        return api_error_response(error);
    }
    ensure_object(&mut managed).insert(
        "lastOptimizationApplyAt".to_string(),
        json!(time_utils::now_iso()),
    );
    if let Err(error) = save_managed_config(&state, &managed).await {
        return api_error_response(error);
    }
    let mut runtime = load_runtime(&state).await;
    let runtime_object = ensure_object(&mut runtime);
    runtime_object.remove("pendingCandidate");
    runtime_object.insert("lastCandidates".to_string(), json!(candidates));
    runtime_object.insert("lastSwitchReason".to_string(), json!("manual-speed-test"));
    if let Err(error) = save_runtime(&state, &runtime).await {
        return api_error_response(error);
    }
    response::ok(json!({ "selected": selected, "state": ownership.get("optimization") }))
        .into_response()
}

#[utoipa::path(post, path = "/api/admin/cloudflared/optimization/fallback", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_optimization_fallback", responses((status = 200, description = "Restored fallback origin")))]
async fn fallback_optimization(State(state): State<AppState>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(&state).await;
    let api = match api_for_background(&state).await {
        Ok(Some(api)) => api,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                "Cloudflare API Token is not configured",
            );
        }
        Err(error) => return api_error_response(error),
    };
    let mut ownership = load_managed_state(&state).await;
    match fallback_to_wildcard(&state, &api, &managed, &mut ownership).await {
        Ok(()) => {
            let mut runtime = load_runtime(&state).await;
            let object = ensure_object(&mut runtime);
            object.remove("pendingCandidate");
            object.insert("lastSwitchReason".to_string(), json!("manual-fallback"));
            if let Err(error) = save_runtime(&state, &runtime).await {
                return api_error_response(error);
            }
            response::ok(json!({ "fallbackActive": true })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}
