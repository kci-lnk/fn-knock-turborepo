use super::*;

#[utoipa::path(get, path = "/api/admin/scan/discover-targets", tag = "scan", operation_id = "get_api_admin_scan_discover_targets", responses((status = 200, description = "Scan targets")))]
pub(super) async fn get_discover_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_config().await {
        Ok(config) => response::ok(build_discover_targets_payload(
            &state,
            &headers,
            &config,
            &translator,
        ))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read scan discover targets config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadTargetsFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/scan/discover-targets", tag = "scan", operation_id = "post_api_admin_scan_discover_targets", responses((status = 200, description = "Saved scan targets")))]
pub(super) async fn save_discover_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DiscoverTargetsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let custom_cidrs = normalize_allowed_scan_cidrs(body.custom_cidrs);
    let selected_cidrs = normalize_allowed_scan_cidrs(body.selected_cidrs);
    if let Err(message) = validate_scan_cidrs(&selected_cidrs) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_scan_discovery_error(&translator, &message),
        );
    }

    let config = match state
        .storage
        .store
        .merge_config_object_fields(
            "scan_discovery",
            [
                ("custom_cidrs".to_string(), json!(custom_cidrs)),
                ("selected_cidrs".to_string(), json!(selected_cidrs)),
            ]
            .into_iter()
            .collect(),
        )
        .await
    {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to save scan discover targets config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.saveTargetsFailed"),
            );
        }
    };
    response::ok(build_discover_targets_payload(
        &state,
        &headers,
        &config,
        &translator,
    ))
    .into_response()
}

#[utoipa::path(get, path = "/api/admin/scan/discover-settings", tag = "scan", operation_id = "get_api_admin_scan_discover_settings", responses((status = 200, description = "Scan discovery settings")))]
pub(super) async fn get_discover_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_config().await {
        Ok(config) => response::ok(build_discover_settings_payload(&config)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read scan discover settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadSettingsFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/scan/discover-settings", tag = "scan", operation_id = "post_api_admin_scan_discover_settings", responses((status = 200, description = "Saved scan discovery settings")))]
pub(super) async fn save_discover_settings(
    State(state): State<AppState>,
    Json(body): Json<DiscoverSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(mode) = ScanIntensityMode::parse(&body.intensity_mode) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            translator.t("server.scanDiscovery.invalidIntensityMode"),
        );
    };
    let Some(level) = ScanIntensityLevel::parse(&body.intensity_level) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            translator.t("server.scanDiscovery.invalidIntensityLevel"),
        );
    };
    let config = match state
        .storage
        .store
        .merge_config_object_fields(
            "scan_discovery",
            [
                ("intensity_mode".to_string(), json!(mode.as_str())),
                ("intensity_level".to_string(), json!(level.as_str())),
            ]
            .into_iter()
            .collect(),
        )
        .await
    {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to save scan discover settings");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.saveSettingsFailed"),
            );
        }
    };
    response::ok(build_discover_settings_payload(&config)).into_response()
}

#[utoipa::path(post, path = "/api/admin/scan/discover/jobs", tag = "scan", operation_id = "post_api_admin_scan_discover_jobs", responses((status = 200, description = "Started scan discovery job")))]
pub(super) async fn start_discover_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DiscoverJobBody>,
) -> Response {
    let translator = crate::i18n::Translator::from_state(&state).await;
    let scan_cidrs = match validate_scan_cidrs(&body.target_cidrs) {
        Ok(cidrs) if !cidrs.is_empty() => cidrs,
        Ok(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                translator.t("server.scanDiscovery.selectAtLeastOneCidr"),
            );
        }
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_scan_discovery_error(&translator, &message),
            );
        }
    };
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before scan discover job");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    let self_scan_hosts = resolve_discover_self_hosts(&state, &headers);
    let exclude_ports = collect_excluded_ports(&state);
    let runtime_settings = resolve_scan_runtime_settings(&config);
    let job = create_discover_job(
        &state,
        scan_cidrs,
        self_scan_hosts,
        exclude_ports,
        runtime_settings,
        translator,
    );
    let data = serialize_discover_job(&job, None);
    response::ok(data).into_response()
}

#[utoipa::path(get, path = "/api/admin/scan/discover/jobs/{job_id}", tag = "scan", operation_id = "get_api_admin_scan_discover_jobs_by_job_id", params(("job_id" = String, Path, description = "Scan job identifier")), responses((status = 200, description = "Scan discovery job")))]
pub(super) async fn get_discover_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Query(query): Query<DiscoverJobQuery>,
) -> Response {
    cleanup_discover_jobs();
    let Some(job) = get_discover_job_handle(&job_id) else {
        let translator = crate::i18n::Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.scanDiscovery.scanJobNotFound"),
        );
    };
    response::ok(serialize_discover_job(&job, query.cursor.as_deref())).into_response()
}

#[utoipa::path(delete, path = "/api/admin/scan/discover/jobs/{job_id}", tag = "scan", operation_id = "delete_api_admin_scan_discover_jobs_by_job_id", params(("job_id" = String, Path, description = "Scan job identifier")), responses((status = 200, description = "Cancelled scan discovery job")))]
pub(super) async fn cancel_discover_job_route(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Response {
    cleanup_discover_jobs();
    let Some(job) = get_discover_job_handle(&job_id) else {
        let translator = crate::i18n::Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.scanDiscovery.scanJobNotFound"),
        );
    };
    cancel_discover_job(&job);
    response::ok(serialize_discover_job(&job, None)).into_response()
}

#[utoipa::path(post, path = "/api/admin/scan/host-mappings/probe", tag = "scan", operation_id = "post_api_admin_scan_host_mappings_probe", responses((status = 200, description = "Host mapping probe results")))]
pub(super) async fn probe_host_mappings(
    State(state): State<AppState>,
    Json(body): Json<HostMappingProbeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read host mappings for probe");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    let results = probe_configured_host_mappings(
        config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        body.hosts.unwrap_or_default(),
    )
    .await;
    response::ok(json!({ "results": results })).into_response()
}
