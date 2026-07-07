use super::*;

pub(super) async fn get_discover_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_config().await {
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

    let mut config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read scan discover targets config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    ensure_object(&mut config).insert(
        "scan_discovery".to_string(),
        json!({
            "custom_cidrs": custom_cidrs,
            "selected_cidrs": selected_cidrs
        }),
    );
    if let Err(error) = state.redis.save_config(&config).await {
        tracing::warn!(%error, "failed to save scan discover targets config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            translator.t("server.scanDiscovery.saveTargetsFailed"),
        );
    }
    response::ok(build_discover_targets_payload(
        &state,
        &headers,
        &config,
        &translator,
    ))
    .into_response()
}

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
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before scan discover job");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    let full_range_cidrs =
        resolve_full_range_discover_cidrs(&state, &headers, &config, &translator);
    let self_scan_hosts = resolve_discover_self_hosts(&state, &headers);
    let exclude_ports = collect_excluded_ports(&state);
    let job = create_discover_job(
        scan_cidrs,
        full_range_cidrs,
        self_scan_hosts,
        exclude_ports,
        translator,
    );
    let data = serialize_discover_job(&job, None);
    response::ok(data).into_response()
}

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

pub(super) async fn probe_host_mappings(
    State(state): State<AppState>,
    Json(body): Json<HostMappingProbeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.redis.get_config().await {
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
