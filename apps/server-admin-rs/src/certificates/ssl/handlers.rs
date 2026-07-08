use super::*;

pub(super) async fn status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_ssl_status_with_translator(&state, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to build SSL status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_route_text(&translator, "statusReadFailed"),
            )
        }
    }
}

pub(super) async fn shared_files() -> Response {
    response::ok(list_ssl_shared_files()).into_response()
}

pub(super) async fn shared_file_content(
    State(state): State<AppState>,
    Query(query): Query<SharedContentQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match read_ssl_shared_file(&query.path) {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => shared_file_error_response(&translator, error),
    }
}

pub(super) async fn active_cert_pem(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_config().await {
        Ok(config) => {
            let ssl = normalize_ssl_config(config.get("ssl"));
            let cert = ssl.get("cert").and_then(Value::as_str).unwrap_or("");
            if cert.trim().is_empty() {
                return response::error(
                    StatusCode::NOT_FOUND,
                    ssl_route_text(&translator, "certNotInstalled"),
                );
            }
            pem_response(
                cert,
                "server-cert.pem",
                "application/x-pem-file; charset=utf-8",
            )
        }
        Err(error) => {
            tracing::warn!(%error, "failed to read SSL cert");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_route_text(&translator, "certReadFailed"),
            )
        }
    }
}

pub(super) async fn active_cert_zip(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_config().await {
        Ok(config) => {
            let ssl = normalize_ssl_config(config.get("ssl"));
            let cert = ssl.get("cert").and_then(Value::as_str).unwrap_or("");
            let key = ssl.get("key").and_then(Value::as_str).unwrap_or("");
            if cert.trim().is_empty() || key.trim().is_empty() {
                return response::error(
                    StatusCode::NOT_FOUND,
                    ssl_route_text(&translator, "certNotInstalled"),
                );
            }
            match zip_cert_pair(cert, key) {
                Ok(bytes) => binary_response(bytes, "application/zip", "server-cert.zip"),
                Err(error) => {
                    tracing::warn!(%error, "failed to zip SSL cert");
                    response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ssl_route_text(&translator, "certZipCreateFailed"),
                    )
                }
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to read SSL cert");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_route_text(&translator, "certReadFailed"),
            )
        }
    }
}

pub(super) async fn ca_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let paths = ca_paths(&state);
    if !paths.cert.exists() || !paths.key.exists() {
        return response::ok(json!({ "initialized": false })).into_response();
    }
    match std::fs::read_to_string(&paths.cert) {
        Ok(cert) => match build_ca_status_payload(&cert) {
            Some(payload) => response::ok(payload).into_response(),
            None => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_route_text(&translator, "statusReadFailed"),
            ),
        },
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "statusReadFailed", &error),
        ),
    }
}

pub(super) fn build_ca_status_payload(cert: &str) -> Option<Value> {
    Some(json!({
        "initialized": true,
        "info": parse_cert_info(cert)?,
    }))
}

pub(super) async fn ca_init(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match init_root_ca(&state) {
        Ok(info) => response::ok(info).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caInitFailed", &error),
        ),
    }
}

pub(super) async fn ca_clear(State(state): State<AppState>) -> Response {
    let paths = ca_paths(&state);
    let _ = std::fs::remove_file(paths.cert);
    let _ = std::fs::remove_file(paths.key);
    response::success_empty().into_response()
}

pub(super) async fn ca_cert_pem(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let paths = ca_paths(&state);
    match std::fs::read_to_string(&paths.cert) {
        Ok(content) => pem_response(
            &content,
            "KCI-LNK-Root-CA.pem",
            "application/x-pem-file; charset=utf-8",
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => response::error(
            StatusCode::NOT_FOUND,
            ssl_route_text(&translator, "rootCaNotInitialized"),
        ),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certReadFailed", &error),
        ),
    }
}

pub(super) async fn ca_server_cert_zip(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let hosts = match get_ca_hosts(&state).await {
        Ok(hosts) => hosts,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "caHostLoadFailed", &error),
            );
        }
    };
    if hosts.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            ssl_route_text(&translator, "emptyDomains"),
        );
    }
    match issue_ca_server_cert(&state, &hosts) {
        Ok((cert, key)) => match zip_cert_pair(&cert, &key) {
            Ok(bytes) => binary_response(bytes, "application/zip", "server-cert.zip"),
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "certZipCreateFailed", &error),
            ),
        },
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certZipCreateFailed", &error),
        ),
    }
}

pub(super) async fn ca_hosts(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_ca_hosts(&state).await {
        Ok(hosts) => response::ok(hosts).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caHostLoadFailed", &error),
        ),
    }
}

pub(super) async fn add_ca_host(
    State(state): State<AppState>,
    Json(body): Json<AddCaHostBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let value = body.value.trim();
    if value.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            ssl_route_text(&translator, "hostRequired"),
        );
    }
    match add_ca_host_inner(&state, value).await {
        Ok(hosts) => response::ok(hosts).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caHostSaveFailed", &error),
        ),
    }
}

pub(super) async fn delete_ca_host(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!({}))
    };
    if parsed.get("all").and_then(Value::as_bool) == Some(true) {
        return match save_ca_hosts(&state, &[]).await {
            Ok(()) => response::success_empty().into_response(),
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "caHostSaveFailed", &error),
            ),
        };
    }
    let Some(value) = parsed.get("value").and_then(Value::as_str) else {
        return response::success_empty().into_response();
    };
    match remove_ca_host_inner(&state, value).await {
        Ok(hosts) => response::ok(hosts).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caHostSaveFailed", &error),
        ),
    }
}

pub(super) async fn ca_issue(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let hosts = match get_ca_hosts(&state).await {
        Ok(hosts) => hosts,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "caHostLoadFailed", &error),
            );
        }
    };
    if hosts.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            ssl_route_text(&translator, "emptyDomains"),
        );
    }
    let (cert, key) = match issue_ca_server_cert(&state, &hosts) {
        Ok(pair) => pair,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "certSaveFailed", &error),
            );
        }
    };
    let body = build_ca_issue_certificate_body(&hosts, cert, key);
    match save_ssl_certificate(&state, body, true).await {
        Ok(_) => match sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => {
                response::success_message(ssl_route_text(&translator, "success")).into_response()
            }
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_gateway_error(&translator, &error.to_string()),
            ),
        },
        Err(error) => response::error(
            StatusCode::BAD_REQUEST,
            ssl_error_or_route_text(&translator, "certSaveFailed", &error),
        ),
    }
}

pub(super) fn build_ca_issue_certificate_body(
    hosts: &[String],
    cert: String,
    key: String,
) -> SaveCertificateBody {
    SaveCertificateBody {
        id: None,
        label: hosts.first().cloned(),
        source: Some("ca".to_string()),
        primary_domain: None,
        source_ref_id: None,
        cert,
        key,
        activate: Some(true),
    }
}

pub(super) async fn save_certificate(
    State(state): State<AppState>,
    Json(body): Json<SaveCertificateBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let activate = body.activate != Some(false);
    if let Err(message) = validate_ssl_cert_for_response(&body.cert, &body.key, &translator) {
        return response::error(StatusCode::BAD_REQUEST, message);
    }
    match save_ssl_certificate(&state, body, activate).await {
        Ok(saved) => {
            let mut config_for_sync = None;
            let deployment_mode = if activate {
                "single_active"
            } else {
                let config = match state.store.get_config().await {
                    Ok(config) => config,
                    Err(error) => {
                        return response::error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ssl_error_or_route_text(&translator, "certSaveFailed", &error),
                        );
                    }
                };
                let deployment_mode = normalize_deployment_mode(
                    config
                        .pointer("/ssl/deployment_mode")
                        .and_then(Value::as_str),
                );
                config_for_sync = Some(config);
                deployment_mode
            };
            if should_sync_ssl_deployment_after_save(activate, deployment_mode) {
                if let Err(error) =
                    sync_ssl_deployment_to_gateway(&state, config_for_sync.as_ref()).await
                {
                    tracing::warn!(%error, "failed to sync SSL deployment after save");
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ssl_gateway_error(&translator, &error.to_string()),
                    );
                }
            }
            response::ok(json!({ "id": saved.get("id").and_then(Value::as_str).unwrap_or("") }))
                .into_response()
        }
        Err(error) => response::error(
            StatusCode::BAD_REQUEST,
            ssl_error_or_route_text(&translator, "certSaveFailed", &error),
        ),
    }
}

pub(super) async fn activate_certificate(
    State(state): State<AppState>,
    Json(body): Json<ActivateBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match activate_ssl_certificate(&state, &body.id).await {
        Ok(true) => match sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => response::success_empty().into_response(),
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_gateway_error(&translator, &error.to_string()),
            ),
        },
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            ssl_route_text(&translator, "certNotFound"),
        ),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certActivateFailed", &error),
        ),
    }
}

pub(super) async fn set_deployment_mode(
    State(state): State<AppState>,
    Json(body): Json<DeploymentModeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_error_or_route_text(&translator, "deploymentModeSaveFailed", &error),
            );
        }
    };
    let previous = config.clone();
    let mut ssl = normalize_ssl_config(config.get("ssl"));
    ssl["deployment_mode"] = json!(normalize_deployment_mode(Some(&body.deployment_mode)));
    if ssl.get("deployment_mode").and_then(Value::as_str) == Some("multi_sni")
        && ssl
            .get("active_cert_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
    {
        if let Some(first) = ssl
            .get("certificates")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .cloned()
        {
            let active_id = first
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            ssl = mirror_active_ssl_certificate(&ssl, Some(&active_id));
        }
    }
    config["ssl"] = ssl;
    if let Err(error) = state.store.save_config(&config).await {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "deploymentModeSaveFailed", &error),
        );
    }
    if let Err(error) = sync_ssl_deployment_to_gateway(&state, Some(&config)).await {
        let _ = state.store.save_config(&previous).await;
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_gateway_error(&translator, &error.to_string()),
        );
    }
    match build_ssl_status_with_translator(&state, &translator).await {
        Ok(status) => response::ok(status).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "statusReadFailed", &error),
        ),
    }
}

pub(super) async fn delete_certificate(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match delete_ssl_certificate(&state, &id).await {
        Ok((true, removed_active)) => {
            let config = match state.store.get_config().await {
                Ok(config) => config,
                Err(error) => {
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ssl_error_or_route_text(&translator, "certDeleteFailed", &error),
                    );
                }
            };
            let deployment_mode = config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                .unwrap_or("single_active");
            if removed_active || deployment_mode == "multi_sni" {
                if let Err(error) = sync_ssl_deployment_to_gateway(&state, Some(&config)).await {
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ssl_gateway_error(&translator, &error.to_string()),
                    );
                }
            }
            response::success_empty().into_response()
        }
        Ok((false, _)) => response::error(
            StatusCode::NOT_FOUND,
            ssl_route_text(&translator, "certNotFound"),
        ),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certDeleteFailed", &error),
        ),
    }
}

pub(super) async fn clear_library(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match clear_ssl_certificate_library(&state).await {
        Ok(()) => match sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => response::success_empty().into_response(),
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_gateway_error(&translator, &error.to_string()),
            ),
        },
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certClearFailed", &error),
        ),
    }
}

pub(super) async fn clear_ssl(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match clear_active_ssl(&state).await {
        Ok(()) => match sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => response::success_empty().into_response(),
            Err(error) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssl_gateway_error(&translator, &error.to_string()),
            ),
        },
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "certClearFailed", &error),
        ),
    }
}
