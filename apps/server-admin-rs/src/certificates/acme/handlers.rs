use super::*;

pub(super) async fn status(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    if let Err(error) = ensure_acme_data_migrated(&state).await {
        tracing::warn!(%error, "failed to migrate ACME data before status");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            acme_route_text(&t, "loadStatusFailed"),
        );
    }
    let acme_state = current_acme_install_state(&state, &t).await;
    let client_settings = match ensure_client_settings(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME client settings");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadClientSettingsFailed"),
            );
        }
    };
    let acme_cert = match status_certificate(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME status certificate");
            Value::Null
        }
    };
    let mut data = acme_state;
    if let Some(object) = data.as_object_mut() {
        object.insert("acmeCert".to_string(), acme_cert);
        object.insert(
            "certificateAuthority".to_string(),
            client_settings
                .get("certificateAuthority")
                .cloned()
                .unwrap_or_else(|| json!(DEFAULT_ACME_CERTIFICATE_AUTHORITY)),
        );
        object.insert(
            "certificateAuthorityUpdatedAt".to_string(),
            client_settings
                .get("updatedAt")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    response::ok(data).into_response()
}

pub(super) async fn overview(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    if let Err(error) = ensure_acme_data_migrated(&state).await {
        tracing::warn!(%error, "failed to migrate ACME data before overview");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            acme_route_text(&t, "loadOverviewFailed"),
        );
    }
    let client_settings = match ensure_client_settings(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME client settings for overview");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadClientSettingsFailed"),
            );
        }
    };
    let lock = match get_active_acme_runtime_lock(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load active ACME lock");
            json!({ "locked": false })
        }
    };
    let running_job = if lock.get("locked").and_then(Value::as_bool) == Some(true) {
        if let Some(job_id) = lock
            .get("jobId")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            get_acme_job(&state, &job_id).await.ok().flatten()
        } else {
            None
        }
    } else {
        None
    };
    let applications = match build_application_overview(&state, &t).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to build ACME application overview");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationOverviewFailed"),
            );
        }
    };
    response::ok(json!({
        "acmeState": current_acme_install_state(&state, &t).await,
        "clientSettings": client_settings,
        "lock": lock,
        "applications": applications,
        "runningJob": running_job.map(|job| json!({
            "id": job.get("id").cloned().unwrap_or(Value::Null),
            "applicationId": job.get("applicationId").cloned().unwrap_or(Value::Null),
            "status": job.get("status").cloned().unwrap_or(Value::Null),
            "progress": job.get("progress").cloned().unwrap_or_else(|| json!(0)),
        })).unwrap_or(Value::Null),
    }))
    .into_response()
}

pub(super) async fn uninstall_acme(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    if acme_install_is_installing(&state).await {
        return response::error(
            StatusCode::CONFLICT,
            t.t("server.acmeRoutes.installingCannotDelete"),
        );
    }

    let acme_home = acme_home_dir(&state);
    match tokio::fs::remove_dir_all(&acme_home).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            set_acme_install_state(
                &state,
                "error",
                0,
                "deleteFailed",
                &[("detail", error.to_string())],
            )
            .await;
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "uninstallFailed"),
            );
        }
    }
    set_acme_install_state(&state, "uninstalled", 0, "notInstalled", &[]).await;
    response::ok(current_acme_install_state(&state, &t).await).into_response()
}

pub(super) async fn init_acme(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    let client_settings = match ensure_client_settings(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME client settings before init");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadClientSettingsFailed"),
            );
        }
    };
    if !acme_install_is_installing(&state).await && !acme_executable_path(&state).is_file() {
        let install_state = state.clone();
        let certificate_authority = client_settings
            .get("certificateAuthority")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
            .to_string();
        tokio::spawn(async move {
            start_acme_install(install_state, certificate_authority).await;
        });
    }
    response::ok(json!({
        "executablePath": acme_executable_path(&state),
        "certificateAuthority": client_settings
            .get("certificateAuthority")
            .cloned()
            .unwrap_or_else(|| json!(DEFAULT_ACME_CERTIFICATE_AUTHORITY)),
        "state": current_acme_install_state(&state, &t).await,
    }))
    .into_response()
}

pub(super) async fn legacy_scan_acme_check(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    Json(current_acme_install_state(&state, &t).await).into_response()
}

pub(super) async fn legacy_scan_acme_install(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    let current_state = current_acme_install_state(&state, &t).await;
    match current_state.get("status").and_then(Value::as_str) {
        Some("installed") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": t.t("server.acme.alreadyInstalled") })),
            )
                .into_response();
        }
        Some("installing") => {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "error": t.t("server.acme.installInProgress") })),
            )
                .into_response();
        }
        _ => {}
    }
    let client_settings = match ensure_client_settings(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME client settings before legacy install");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": acme_route_text(&t, "loadClientSettingsFailed") })),
            )
                .into_response();
        }
    };
    let install_state = state.clone();
    let certificate_authority = client_settings
        .get("certificateAuthority")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
        .to_string();
    tokio::spawn(async move {
        start_acme_install(install_state, certificate_authority).await;
    });
    Json(json!({
        "message": t.t("server.acme.installSubmitted"),
        "status": "installing"
    }))
    .into_response()
}

pub(super) async fn legacy_scan_acme_issue(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let (mut body, _) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if body.get("credentials").is_none()
        && let Some(env_vars) = body.get("envVars").cloned()
    {
        ensure_object(&mut body).insert("credentials".to_string(), env_vars);
    }
    let method = body.get("method").and_then(Value::as_str).unwrap_or("dns");
    if method != "dns" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": t.t("server.acmeRoutes.dns01Only") })),
        )
            .into_response();
    }
    let normalized = match validate_acme_request(&body, &t) {
        Ok(value) => value,
        Err(message) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response();
        }
    };
    let target =
        match resolve_legacy_application_for_mutation(&state, &normalized.domains, &t).await {
            Ok(value) => value,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response();
            }
        };
    let saved = match save_acme_application_with_effects(
        &state,
        &t,
        SaveAcmeApplicationInput {
            id: target
                .as_ref()
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            name: target
                .as_ref()
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            name_provided: target
                .as_ref()
                .and_then(|value| value.get("name"))
                .is_some(),
            domains: normalized.domains,
            dns_type: normalized.dns_type,
            credentials: normalized.credentials,
            renew_enabled: target
                .as_ref()
                .and_then(|value| value.get("renewEnabled"))
                .and_then(Value::as_bool)
                .or(Some(true)),
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };
    match start_acme_application_job(
        state.clone(),
        saved.application,
        "manual_request",
        t.clone(),
    )
    .await
    {
        Ok((job, _lock)) => Json(json!({
            "message": t.t("server.acme.issueSucceeded"),
            "jobId": job.get("id").cloned().unwrap_or(Value::Null)
        }))
        .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub(super) async fn save_client_settings_route(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let t = Translator::from_state(&state).await;
    if acme_install_is_installing(&state).await {
        return response::error(
            StatusCode::CONFLICT,
            t.t("server.acmeRoutes.installingCannotSwitchCa"),
        );
    }

    let (body, _) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let certificate_authority =
        normalize_certificate_authority(body.get("certificateAuthority").and_then(Value::as_str));
    let previous = match ensure_client_settings(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME client settings before save");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadClientSettingsFailed"),
            );
        }
    };
    let next = match save_client_settings(&state, &certificate_authority).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to save ACME client settings");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "saveClientSettingsFailed"),
            );
        }
    };

    if !acme_executable_path(&state).is_file() {
        let mut data = next;
        data["synced"] = json!(false);
        return response::ok(data).into_response();
    }

    match switch_certificate_authority(&state, &certificate_authority, &t).await {
        Ok(account_email) => {
            let mut data = next;
            data["synced"] = json!(true);
            data["accountEmail"] = json!(account_email);
            data["state"] = current_acme_install_state(&state, &t).await;
            response::ok(data).into_response()
        }
        Err(error) => {
            let previous_ca = previous
                .get("certificateAuthority")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY);
            save_client_settings(&state, previous_ca).await.ok();
            tracing::warn!(%error, "failed to switch ACME certificate authority");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "switchCertificateAuthorityFailed"),
            )
        }
    }
}

pub(super) async fn config(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    match get_acme_settings(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadConfigFailed"),
            )
        }
    }
}

pub(super) async fn save_config(State(state): State<AppState>, req: Request<Body>) -> Response {
    let t = Translator::from_state(&state).await;
    let (body, _) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let normalized = match validate_acme_request(&body, &t) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let target =
        match resolve_legacy_application_for_mutation(&state, &normalized.domains, &t).await {
            Ok(value) => value,
            Err(error) => {
                return response::error(StatusCode::BAD_REQUEST, error.to_string());
            }
        };
    let target_name = target
        .as_ref()
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string);

    match save_acme_application_with_effects(
        &state,
        &t,
        SaveAcmeApplicationInput {
            id: target
                .as_ref()
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            name: target_name.clone(),
            name_provided: target_name.is_some(),
            domains: normalized.domains,
            dns_type: normalized.dns_type,
            credentials: normalized.credentials,
            renew_enabled: target
                .as_ref()
                .and_then(|value| value.get("renewEnabled"))
                .and_then(Value::as_bool)
                .or(Some(true)),
        },
    )
    .await
    {
        Ok(saved) => {
            if let Err(error) = sync_gateway_if_acme_library_removed(
                &state,
                saved.removed_active_library_certificate,
                saved.removed_library_certificate_count,
            )
            .await
            {
                tracing::warn!(%error, "failed to sync gateway after ACME config cleanup");
            }
            response::ok(json!({
                "domains": saved.application.get("domains").cloned().unwrap_or_else(|| json!([])),
                "dnsType": saved.application.get("dnsType").cloned().unwrap_or_else(|| json!("")),
                "credentials": saved.application.get("credentials").cloned().unwrap_or_else(|| json!({})),
                "updatedAt": saved.application.get("updatedAt").cloned().unwrap_or(Value::Null),
            }))
            .into_response()
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error.to_string()),
    }
}

pub(super) async fn create_application(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let (body, _replayable_req) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let submit_now = submit_now_requested(&body);
    let normalized = match validate_acme_request(&body, &t) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    match save_acme_application_with_effects(
        &state,
        &t,
        SaveAcmeApplicationInput {
            id: None,
            name: body.get("name").and_then(Value::as_str).map(str::to_string),
            name_provided: body.get("name").is_some(),
            domains: normalized.domains,
            dns_type: normalized.dns_type,
            credentials: normalized.credentials,
            renew_enabled: body.get("renewEnabled").and_then(Value::as_bool),
        },
    )
    .await
    {
        Ok(saved) => {
            if let Err(error) = sync_gateway_if_acme_library_removed(
                &state,
                saved.removed_active_library_certificate,
                saved.removed_library_certificate_count,
            )
            .await
            {
                tracing::warn!(%error, "failed to sync gateway after ACME application cleanup");
            }
            if submit_now {
                return match start_acme_application_job(
                    state.clone(),
                    saved.application.clone(),
                    "manual_request",
                    t.clone(),
                )
                .await
                {
                    Ok((job, lock)) => response::ok(json!({
                        "application": saved.application,
                        "job": job,
                        "lock": lock,
                    }))
                    .into_response(),
                    Err(error) => response::error(StatusCode::CONFLICT, error.to_string()),
                };
            }
            response::ok(json!({ "application": saved.application })).into_response()
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error.to_string()),
    }
}

pub(super) async fn update_application(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    req: Request<Body>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let (body, _replayable_req) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let submit_now = submit_now_requested(&body);
    let existing = match find_acme_application(&state, &id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.notFound"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME application before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "updateApplicationFailed"),
            );
        }
    };
    let normalized = match validate_acme_request(&body, &t) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let mut reservation = if submit_now {
        let pending = build_pending_acme_application_for_update(&existing, &body, &normalized);
        match reserve_acme_application_job(&state, &pending, "manual_request", &t).await {
            Ok(reservation) => Some(reservation),
            Err(error) => return response::error(StatusCode::CONFLICT, error.to_string()),
        }
    } else {
        None
    };
    match save_acme_application_with_effects(
        &state,
        &t,
        SaveAcmeApplicationInput {
            id: Some(id),
            name: body.get("name").and_then(Value::as_str).map(str::to_string),
            name_provided: body.get("name").is_some(),
            domains: normalized.domains,
            dns_type: normalized.dns_type,
            credentials: normalized.credentials,
            renew_enabled: body
                .get("renewEnabled")
                .and_then(Value::as_bool)
                .or_else(|| existing.get("renewEnabled").and_then(Value::as_bool)),
        },
    )
    .await
    {
        Ok(saved) => {
            if let Err(error) = sync_gateway_if_acme_library_removed(
                &state,
                saved.removed_active_library_certificate,
                saved.removed_library_certificate_count,
            )
            .await
            {
                tracing::warn!(%error, "failed to sync gateway after ACME application update cleanup");
                if let Some((job, lock)) = reservation.take() {
                    let message = error.to_string();
                    fail_reserved_acme_application_job(
                        &state, &existing, &job, &lock, &message, &t,
                    )
                    .await
                    .ok();
                    return response::error(StatusCode::BAD_REQUEST, message);
                }
            }
            if let Some((job, lock)) = reservation.take() {
                return match run_reserved_acme_application_job(
                    state.clone(),
                    saved.application.clone(),
                    "manual_request",
                    job.clone(),
                    lock.clone(),
                    t.clone(),
                )
                .await
                {
                    Ok((job, lock)) => response::ok(json!({
                        "application": saved.application,
                        "job": job,
                        "lock": lock,
                    }))
                    .into_response(),
                    Err(error) => {
                        let message = error.to_string();
                        fail_reserved_acme_application_job(
                            &state,
                            &saved.application,
                            &job,
                            &lock,
                            &message,
                            &t,
                        )
                        .await
                        .ok();
                        response::error(StatusCode::CONFLICT, message)
                    }
                };
            }
            response::ok(json!({ "application": saved.application })).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            if let Some((job, lock)) = reservation.take() {
                fail_reserved_acme_application_job(&state, &existing, &job, &lock, &message, &t)
                    .await
                    .ok();
            }
            response::error(StatusCode::BAD_REQUEST, message)
        }
    }
}

pub(super) async fn delete_application(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match get_active_acme_runtime_lock(&state).await {
        Ok(lock) if lock.get("locked").and_then(Value::as_bool) == Some(true) => {
            return response::error(
                StatusCode::CONFLICT,
                t.t("server.acmeJobRunner.activeTaskRunning"),
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to check active ACME lock before delete");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "deleteApplicationFailed"),
            );
        }
    }

    match delete_acme_application_internal(&state, &id).await {
        Ok(true) => response::ok(json!({ "id": id })).into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            t.t("server.acmeRoutes.applicationNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to delete ACME application");
            response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(&t, "deleteApplicationFailed"),
            )
        }
    }
}

pub(super) async fn delete_application_certificate(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match delete_acme_application_certificate_internal(&state, &id).await {
        Ok(true) => response::success_empty().into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            t.t("server.acmeRoutes.applicationNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to delete ACME application certificate");
            response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(&t, "deleteCertificateFailed"),
            )
        }
    }
}

pub(super) async fn sync_application_library(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let application = match find_acme_application(&state, &id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.notFound"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME application before library sync");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationFailed"),
            );
        }
    };
    if get_usable_issued_certificate_for_application(&state, &application)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            t.t("server.acmeRoutes.noMatchingIssuedCertificate"),
        );
    }
    match save_acme_certificate_to_library_by_application(&state, &application, false, None, &t)
        .await
    {
        Ok(saved) => {
            let certificate_id = saved
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Err(error) = sync_gateway_if_acme_library_touched(&state, &certificate_id).await
            {
                tracing::warn!(%error, "failed to sync gateway after ACME library sync");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    acme_route_text(&t, "syncLibraryFailed"),
                );
            }
            response::ok(json!({ "certificateId": certificate_id, "linked": true })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save ACME certificate to library");
            response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(&t, "syncLibraryFailed"),
            )
        }
    }
}

pub(super) async fn deploy_application_certificate(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let application = match find_acme_application(&state, &id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.notFound"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME application before deploy");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationFailed"),
            );
        }
    };
    if get_usable_issued_certificate_for_application(&state, &application)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            t.t("server.acmeRoutes.noMatchingIssuedCertificate"),
        );
    }
    match save_acme_certificate_to_library_by_application(&state, &application, true, None, &t)
        .await
    {
        Ok(_) => match ssl::sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => response::success_message(t.t("server.acmeRoutes.success")).into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to sync gateway after ACME certificate deploy");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    acme_route_text(&t, "deployCertificateFailed"),
                )
            }
        },
        Err(error) => {
            tracing::warn!(%error, "failed to deploy ACME certificate from application");
            response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(&t, "deployCertificateFailed"),
            )
        }
    }
}

pub(super) async fn request_application_certificate(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let application = match find_acme_application(&state, &id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return response::error(
                StatusCode::NOT_FOUND,
                t.t("server.acmeRoutes.applicationNotFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME application before request");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationFailed"),
            );
        }
    };
    match start_acme_application_job(state.clone(), application, "manual_request", t).await {
        Ok((job, lock)) => response::ok(json!({ "job": job, "lock": lock })).into_response(),
        Err(error) => response::error(StatusCode::CONFLICT, error.to_string()),
    }
}

pub(super) async fn request_certificate(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let (body, _) = match read_replayable_json_body(req, &t).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let method = body.get("method").and_then(Value::as_str).unwrap_or("dns");
    if method != "dns" {
        return response::error(StatusCode::BAD_REQUEST, t.t("server.acmeRoutes.dns01Only"));
    }
    let normalized = match validate_acme_request(&body, &t) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let target =
        match resolve_legacy_application_for_mutation(&state, &normalized.domains, &t).await {
            Ok(value) => value,
            Err(error) => return response::error(StatusCode::BAD_REQUEST, error.to_string()),
        };
    match save_acme_application_with_effects(
        &state,
        &t,
        SaveAcmeApplicationInput {
            id: target
                .as_ref()
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            name: target
                .as_ref()
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            name_provided: target
                .as_ref()
                .and_then(|value| value.get("name"))
                .is_some(),
            domains: normalized.domains,
            dns_type: normalized.dns_type,
            credentials: normalized.credentials,
            renew_enabled: target
                .as_ref()
                .and_then(|value| value.get("renewEnabled"))
                .and_then(Value::as_bool)
                .or(Some(true)),
        },
    )
    .await
    {
        Ok(saved) => {
            match start_acme_application_job(state.clone(), saved.application, "manual_request", t)
                .await
            {
                Ok((job, _lock)) => response::ok(json!({ "jobId": job["id"] })).into_response(),
                Err(error) => response::error(StatusCode::CONFLICT, error.to_string()),
            }
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error.to_string()),
    }
}

pub(super) async fn stop_active_job(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    match stop_active_acme_job(&state, &t).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to stop active ACME job");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "stopJobFailed"),
            )
        }
    }
}

pub(super) async fn dns_providers(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    response::ok(Value::Array(acme_dns_providers(&t))).into_response()
}

pub(super) async fn subdomain_recommendation(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    match state.redis.get_config().await {
        Ok(config) => response::ok(build_subdomain_certificate_recommendation(
            &state, &config, &t,
        ))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load config for ACME subdomain recommendation");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadSubdomainRecommendationFailed"),
            )
        }
    }
}

pub(super) async fn build_application_overview(
    state: &AppState,
    t: &Translator,
) -> redis::RedisResult<Vec<Value>> {
    let applications = read_acme_applications(state).await?;
    let issued_certificates = read_issued_certificates(state).await?;
    let ssl_status = ssl::build_ssl_status(state)
        .await
        .unwrap_or_else(|_| json!({ "certificates": [] }));
    let mut output = Vec::new();

    for application in applications {
        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let issued_certificate = issued_certificates
            .iter()
            .find(|certificate| {
                certificate.get("applicationId").and_then(Value::as_str)
                    == Some(application_id.as_str())
                    && issued_certificate_compatible(&application, certificate)
            })
            .cloned();
        let latest_job = match application.get("latestJobId").and_then(Value::as_str) {
            Some(job_id) => get_acme_job(state, job_id).await?,
            None => None,
        };
        let library_certificate = issued_certificate.as_ref().and_then(|certificate| {
            find_library_certificate(&ssl_status, &application, certificate)
        });

        output.push(json!({
            "id": application.get("id").cloned().unwrap_or(Value::Null),
            "name": application.get("name").cloned().unwrap_or(Value::Null),
            "primaryDomain": application.get("primaryDomain").cloned().unwrap_or(Value::Null),
            "domains": application.get("domains").cloned().unwrap_or_else(|| json!([])),
            "dnsType": application.get("dnsType").cloned().unwrap_or(Value::Null),
            "providerLabel": provider_label(t, application.get("dnsType").and_then(Value::as_str).unwrap_or("")),
            "renewEnabled": application.get("renewEnabled").cloned().unwrap_or_else(|| json!(true)),
            "createdAt": application.get("createdAt").cloned().unwrap_or(Value::Null),
            "updatedAt": application.get("updatedAt").cloned().unwrap_or(Value::Null),
            "latestJob": build_latest_job_summary(&application, latest_job.as_ref()),
            "certificate": match issued_certificate.as_ref() {
                Some(certificate) => json!({
                    "exists": true,
                    "validFrom": certificate.pointer("/certInfo/validFrom").cloned().unwrap_or(Value::Null),
                    "validTo": certificate.pointer("/certInfo/validTo").cloned().unwrap_or(Value::Null),
                    "dnsNames": certificate.pointer("/certInfo/dnsNames").cloned().unwrap_or_else(|| json!([])),
                    "issuer": certificate.pointer("/certInfo/issuer").cloned().unwrap_or(Value::Null),
                }),
                None => json!({ "exists": false }),
            },
            "library": match library_certificate {
                Some(certificate) => json!({
                    "linked": true,
                    "certificateId": certificate.get("id").cloned().unwrap_or(Value::Null),
                    "isActive": certificate.get("is_active").cloned().unwrap_or_else(|| json!(false)),
                }),
                None => json!({ "linked": false }),
            },
        }));
    }

    Ok(output)
}

pub(super) async fn applications(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    match read_acme_applications(&state).await {
        Ok(value) => response::ok(Value::Array(value)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME applications");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationsFailed"),
            )
        }
    }
}

pub(super) async fn application(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match find_acme_application(&state, &id).await {
        Ok(Some(value)) => response::ok(value).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            t.t("server.acmeRoutes.applicationNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME application");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadApplicationFailed"),
            )
        }
    }
}

pub(super) async fn job(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    let t = Translator::from_state(&state).await;
    match get_acme_job(&state, &id).await {
        Ok(Some(value)) => response::ok(value).into_response(),
        Ok(None) => response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.notFound")),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME job");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadJobFailed"),
            )
        }
    }
}

pub(super) async fn job_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match get_acme_logs(&state, &id, DEFAULT_ACME_LOG_LIMIT, "desc").await {
        Ok(value) => response::ok(Value::Array(value)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME job logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadJobLogsFailed"),
            )
        }
    }
}

pub(super) async fn job_poll(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<AcmeLogsQuery>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let job = match get_acme_job(&state, &id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.notFound"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME job");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadJobFailed"),
            );
        }
    };
    let limit = normalize_log_limit(query.limit.as_deref());
    let order = if query.order.as_deref() == Some("asc") {
        "asc"
    } else {
        "desc"
    };
    match get_acme_logs(&state, &id, limit, order).await {
        Ok(logs) => response::ok(json!({
            "job": job,
            "logs": logs,
            "analysis": Value::Null,
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME job poll data");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadJobPollFailed"),
            )
        }
    }
}

pub(super) async fn cert_info(
    State(state): State<AppState>,
    AxumPath(domain): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match get_certificate_for_domain(&state, &domain).await {
        Ok(Some((primary_domain, _cert, _key, info))) => response::ok(json!({
            "domain": primary_domain,
            "info": info,
        }))
        .into_response(),
        Ok(None) => response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.certNotFound")),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME certificate info");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadCertificateInfoFailed"),
            )
        }
    }
}

pub(super) async fn delete_cert(
    State(state): State<AppState>,
    AxumPath(domain): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let normalized_domain = normalize_domain_name(&domain);
    if normalized_domain.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            t.t("server.redis.acme.domainRequired"),
        );
    }
    match find_application_by_primary_domain(&state, &normalized_domain).await {
        Ok(Some(application)) => {
            let Some(id) = application.get("id").and_then(Value::as_str) else {
                return response::error(
                    StatusCode::NOT_FOUND,
                    t.t("server.acmeRoutes.certNotFound"),
                );
            };
            match delete_acme_application_certificate_internal(&state, id).await {
                Ok(true) => response::success_empty().into_response(),
                Ok(false) => response::error(
                    StatusCode::NOT_FOUND,
                    t.t("server.acmeRoutes.applicationNotFound"),
                ),
                Err(error) => {
                    tracing::warn!(%error, "failed to delete ACME application certificate");
                    response::error(
                        StatusCode::BAD_REQUEST,
                        acme_route_text(&t, "deleteCertificateFailed"),
                    )
                }
            }
        }
        Ok(None) => {
            if let Err(error) = delete_acme_cert_pair(&state, &normalized_domain).await {
                tracing::warn!(%error, "failed to delete ACME certificate files");
                return response::error(
                    StatusCode::BAD_REQUEST,
                    acme_route_text(&t, "deleteCertificateFailed"),
                );
            }
            let (removed_count, removed_active) = match ssl::delete_acme_ssl_certificates(
                &state,
                None,
                Some(&normalized_domain),
            )
            .await
            {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, "failed to delete ACME certificate from SSL library");
                    return response::error(
                        StatusCode::BAD_REQUEST,
                        acme_route_text(&t, "deleteCertificateFailed"),
                    );
                }
            };
            if let Err(error) =
                remove_acme_domain_artifacts(&state, &[normalized_domain.clone()]).await
            {
                tracing::warn!(%error, "failed to remove ACME certificate files");
            }
            if let Err(error) =
                sync_gateway_if_acme_library_removed(&state, removed_active, removed_count).await
            {
                tracing::warn!(%error, "failed to sync gateway after ACME cert delete");
            }
            response::success_empty().into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to resolve ACME certificate domain");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "deleteCertificateFailed"),
            )
        }
    }
}

pub(super) async fn cert_download(
    State(state): State<AppState>,
    AxumPath(domain): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    match get_certificate_for_domain(&state, &domain).await {
        Ok(Some((primary_domain, cert, key, _info))) => {
            match zip_acme_cert_pair(&primary_domain, &cert, &key) {
                Ok(bytes) => {
                    ssl::binary_response(bytes, "application/zip", &format!("{primary_domain}.zip"))
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to create ACME certificate zip");
                    response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        acme_route_text(&t, "createCertificateZipFailed"),
                    )
                }
            }
        }
        Ok(None) => response::error(StatusCode::NOT_FOUND, t.t("server.acmeRoutes.certNotFound")),
        Err(error) => {
            tracing::warn!(%error, "failed to load ACME certificate for download");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "loadCertificateFailed"),
            )
        }
    }
}

pub(super) async fn deploy_domain_certificate(
    State(state): State<AppState>,
    AxumPath(domain): AxumPath<String>,
) -> Response {
    let t = Translator::from_state(&state).await;
    let normalized_domain = normalize_domain_name(&domain);
    if normalized_domain.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            t.t("server.redis.acme.domainRequired"),
        );
    }

    match find_application_by_primary_domain(&state, &normalized_domain).await {
        Ok(Some(application)) => {
            if get_usable_issued_certificate_for_application(&state, &application)
                .await
                .ok()
                .flatten()
                .is_none()
            {
                return response::error(
                    StatusCode::BAD_REQUEST,
                    t.t("server.acmeRoutes.noMatchingIssuedCertificate"),
                );
            }
            match save_acme_certificate_to_library_by_application(
                &state,
                &application,
                true,
                None,
                &t,
            )
            .await
            {
                Ok(_) => match ssl::sync_ssl_deployment_to_gateway(&state, None).await {
                    Ok(()) => {
                        response::success_message(t.t("server.acmeRoutes.success")).into_response()
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to sync gateway after ACME certificate deploy");
                        response::error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            acme_route_text(&t, "deployCertificateFailed"),
                        )
                    }
                },
                Err(error) => {
                    tracing::warn!(%error, "failed to deploy ACME application certificate");
                    response::error(
                        StatusCode::BAD_REQUEST,
                        acme_route_text(&t, "deployCertificateFailed"),
                    )
                }
            }
        }
        Ok(None) => {
            let Some((cert, key)) = read_acme_cert_pair(&state, &normalized_domain)
                .await
                .ok()
                .flatten()
            else {
                return response::error(
                    StatusCode::NOT_FOUND,
                    t.t("server.acmeRoutes.certNotFound"),
                );
            };
            if ssl::parse_cert_info(&cert).is_none()
                || !key.contains("-----BEGIN ")
                || !key.contains("PRIVATE KEY-----")
            {
                return response::error(
                    StatusCode::BAD_REQUEST,
                    t.t("server.acmeRoutes.certOrKeyInvalid"),
                );
            }
            match ssl::save_acme_certificate_to_library(
                &state,
                None,
                Some(&normalized_domain),
                &normalized_domain,
                None,
                &cert,
                &key,
                true,
            )
            .await
            {
                Ok(_) => match ssl::sync_ssl_deployment_to_gateway(&state, None).await {
                    Ok(()) => {
                        response::success_message(t.t("server.acmeRoutes.success")).into_response()
                    }
                    Err(error) => {
                        tracing::warn!(%error, "failed to sync gateway after ACME domain certificate deploy");
                        response::error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            acme_route_text(&t, "deployCertificateFailed"),
                        )
                    }
                },
                Err(error) => {
                    tracing::warn!(%error, "failed to deploy ACME domain certificate");
                    response::error(
                        StatusCode::BAD_REQUEST,
                        acme_route_text(&t, "deployCertificateFailed"),
                    )
                }
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to resolve ACME certificate domain before deploy");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                acme_route_text(&t, "deployCertificateFailed"),
            )
        }
    }
}
