use std::{
    collections::BTreeSet,
    env,
    io::{Cursor, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use ::time::{OffsetDateTime, format_description::well_known::Rfc3339};
use anyhow::anyhow;
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path as AxumPath, Query, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use x509_parser::{extensions::GeneralName, pem::parse_x509_pem};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    response,
    state::AppState,
    time_utils,
};

const MAX_SHARED_FILES: usize = 500;
const MAX_SHARED_FILE_SIZE: u64 = 512 * 1024;
const MAX_SHARED_SCAN_DEPTH: usize = 3;
const SSL_CERT_SHARE_NAME: &str = "fn-knock";
const CA_HOSTS_KEY: &str = "fn_knock:ca:hosts";
const CA_CERT_FILENAME: &str = "rootCA.pem";
const CA_KEY_FILENAME: &str = "rootCA.key.pem";

fn ssl_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sslRoutes.{key}"))
}

fn ssl_redis_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.redis.ssl.{key}"), params)
}

fn fnos_data_share_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.fnosDataShare.{key}"))
}

fn ssl_gateway_error(translator: &Translator, detail: &str) -> String {
    let base = translator.t("server.sslGateway.syncFailed");
    let detail = detail.trim();
    if detail.is_empty() {
        base
    } else {
        format!("{base}: {detail}")
    }
}

fn validate_ssl_cert_for_response(
    cert: &str,
    key: &str,
    translator: &Translator,
) -> Result<(), String> {
    if cert.trim().is_empty() || key.trim().is_empty() {
        return Err(translator.t("server.redis.ssl.certContentRequired"));
    }
    validate_ssl_cert_pair(cert, key)
        .map_err(|error| ssl_validation_error_message(translator, &error))
}

#[derive(Debug)]
enum SslValidationError {
    CertFormatInvalid(String),
    KeyFormatInvalid(String),
    CertKeyMismatch,
    CertKeyCheckFailed(String),
}

fn ssl_validation_error_message(translator: &Translator, error: &SslValidationError) -> String {
    match error {
        SslValidationError::CertFormatInvalid(message) => ssl_redis_text_params(
            translator,
            "certFormatInvalid",
            &[("message", message.clone())],
        ),
        SslValidationError::KeyFormatInvalid(message) => ssl_redis_text_params(
            translator,
            "keyFormatInvalid",
            &[("message", message.clone())],
        ),
        SslValidationError::CertKeyMismatch => translator.t("server.redis.ssl.certKeyMismatch"),
        SslValidationError::CertKeyCheckFailed(message) => ssl_redis_text_params(
            translator,
            "certKeyCheckFailed",
            &[("message", message.clone())],
        ),
    }
}

fn ssl_validation_error_plain(error: SslValidationError) -> String {
    match error {
        SslValidationError::CertFormatInvalid(_) => "Certificate format is invalid".to_string(),
        SslValidationError::KeyFormatInvalid(_) => "Private key format is invalid".to_string(),
        SslValidationError::CertKeyMismatch => {
            "Certificate and private key do not match".to_string()
        }
        SslValidationError::CertKeyCheckFailed(message) => {
            if message.trim().is_empty() {
                "Certificate and private key check failed".to_string()
            } else {
                format!("Certificate and private key check failed: {message}")
            }
        }
    }
}

fn localize_ssl_error(translator: &Translator, error: &dyn std::fmt::Display) -> String {
    let message = error.to_string();
    if message.starts_with("Certificate and private key check failed") {
        return ssl_route_text(translator, "certOrKeyInvalid");
    }
    match message.as_str() {
        "Root CA not initialized" => ssl_route_text(translator, "rootCaNotInitialized"),
        "No hosts configured" => ssl_route_text(translator, "emptyDomains"),
        "Certificate format is invalid" | "Private key format is invalid" => {
            ssl_route_text(translator, "certOrKeyInvalid")
        }
        "Certificate and private key do not match" => {
            ssl_route_text(translator, "certOrKeyInvalid")
        }
        _ => message,
    }
}

fn ssl_error_or_route_text(
    translator: &Translator,
    fallback_key: &str,
    error: &dyn std::fmt::Display,
) -> String {
    let raw = error.to_string();
    let localized = localize_ssl_error(translator, error);
    if localized == raw {
        ssl_route_text(translator, fallback_key)
    } else {
        localized
    }
}

fn shared_file_error_status_and_message(
    translator: &Translator,
    error: &anyhow::Error,
) -> (StatusCode, String) {
    if error.downcast_ref::<SharedFileNotFound>().is_some() {
        return (
            StatusCode::NOT_FOUND,
            ssl_route_text(translator, "readSharedFileFailed"),
        );
    }
    if error.downcast_ref::<SharedFileForbidden>().is_some() {
        return (
            StatusCode::FORBIDDEN,
            ssl_route_text(translator, "readSharedFileFailed"),
        );
    }
    let message = error.to_string();
    match message.as_str() {
        "Shared directory is not configured" => (
            StatusCode::NOT_FOUND,
            fnos_data_share_text(translator, "shareMissing"),
        ),
        "Invalid shared file path" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "invalidPath"),
        ),
        "Shared path must be a file" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "fileOnly"),
        ),
        "Shared file is too large" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "fileTooLarge"),
        ),
        _ => (StatusCode::BAD_REQUEST, message),
    }
}

fn shared_file_error_response(translator: &Translator, error: anyhow::Error) -> Response {
    let (status, message) = shared_file_error_status_and_message(translator, &error);
    response::error(status, message)
}

#[derive(Deserialize)]
struct SharedContentQuery {
    path: String,
}

#[derive(Deserialize)]
struct SaveCertificateBody {
    id: Option<String>,
    label: Option<String>,
    source: Option<String>,
    primary_domain: Option<String>,
    source_ref_id: Option<String>,
    cert: String,
    key: String,
    activate: Option<bool>,
}

#[derive(Deserialize)]
struct LegacySaveBody {
    ssl: LegacySslPayload,
}

#[derive(Deserialize)]
struct LegacySslPayload {
    cert: String,
    key: String,
}

#[derive(Deserialize)]
struct ActivateBody {
    id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CertificateCoverageInput {
    id: String,
    certificate_domains: Vec<String>,
}

#[derive(Deserialize)]
struct DeploymentModeBody {
    deployment_mode: String,
}

#[derive(Deserialize)]
struct AddCaHostBody {
    value: String,
}

pub fn ssl_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/ssl/status", get(status))
        .route("/api/admin/ssl/shared-files", get(shared_files))
        .route(
            "/api/admin/ssl/shared-files/content",
            get(shared_file_content),
        )
        .route("/api/admin/ssl/cert.pem", get(active_cert_pem))
        .route("/api/admin/ssl/cert.zip", get(active_cert_zip))
        .route("/api/admin/ssl/ca/status", get(ca_status))
        .route("/api/admin/ssl/ca/init", post(ca_init))
        .route("/api/admin/ssl/ca", delete(ca_clear))
        .route("/api/admin/ssl/ca/cert.pem", get(ca_cert_pem))
        .route("/api/admin/ssl/ca/server-cert.zip", get(ca_server_cert_zip))
        .route(
            "/api/admin/ssl/ca/hosts",
            get(ca_hosts).post(add_ca_host).delete(delete_ca_host),
        )
        .route("/api/admin/ssl/ca/issue", post(ca_issue))
        .route(
            "/api/admin/ssl/certificates",
            post(save_certificate).delete(clear_library),
        )
        .route(
            "/api/admin/ssl/certificates/{id}",
            delete(delete_certificate),
        )
        .route("/api/admin/ssl/activate", post(activate_certificate))
        .route("/api/admin/ssl/deployment-mode", post(set_deployment_mode))
        .route("/api/admin/ssl", post(save_legacy_ssl).delete(clear_ssl))
        .route("/api/admin/ssl/", post(save_legacy_ssl).delete(clear_ssl))
}

async fn status(State(state): State<AppState>) -> Response {
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

async fn shared_files() -> Response {
    response::ok(list_ssl_shared_files()).into_response()
}

async fn shared_file_content(
    State(state): State<AppState>,
    Query(query): Query<SharedContentQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match read_ssl_shared_file(&query.path) {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => shared_file_error_response(&translator, error),
    }
}

async fn active_cert_pem(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_config().await {
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

async fn active_cert_zip(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_config().await {
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

async fn ca_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let paths = ca_paths(&state);
    if !paths.cert.exists() || !paths.key.exists() {
        return response::ok(json!({ "initialized": false })).into_response();
    }
    match std::fs::read_to_string(&paths.cert) {
        Ok(cert) => response::ok(json!({
            "initialized": true,
            "info": parse_cert_info(&cert)
        }))
        .into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "statusReadFailed", &error),
        ),
    }
}

async fn ca_init(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match init_root_ca(&state) {
        Ok(info) => response::ok(info).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caInitFailed", &error),
        ),
    }
}

async fn ca_clear(State(state): State<AppState>) -> Response {
    let paths = ca_paths(&state);
    let _ = std::fs::remove_file(paths.cert);
    let _ = std::fs::remove_file(paths.key);
    response::success_empty().into_response()
}

async fn ca_cert_pem(State(state): State<AppState>) -> Response {
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

async fn ca_server_cert_zip(State(state): State<AppState>) -> Response {
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

async fn ca_hosts(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_ca_hosts(&state).await {
        Ok(hosts) => response::ok(hosts).into_response(),
        Err(error) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "caHostLoadFailed", &error),
        ),
    }
}

async fn add_ca_host(State(state): State<AppState>, Json(body): Json<AddCaHostBody>) -> Response {
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

async fn delete_ca_host(State(state): State<AppState>, body: Bytes) -> Response {
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

async fn ca_issue(State(state): State<AppState>) -> Response {
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
    let body = SaveCertificateBody {
        id: None,
        label: hosts.first().cloned(),
        source: Some("ca".to_string()),
        primary_domain: hosts.first().cloned(),
        source_ref_id: None,
        cert,
        key,
        activate: Some(true),
    };
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

async fn save_certificate(
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
                let config = match state.redis.get_config().await {
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

async fn save_legacy_ssl(
    State(state): State<AppState>,
    Json(body): Json<LegacySaveBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(message) = validate_ssl_cert_for_response(&body.ssl.cert, &body.ssl.key, &translator)
    {
        return response::error(StatusCode::BAD_REQUEST, message);
    }
    let body = SaveCertificateBody {
        id: None,
        label: Some(ssl_route_text(&translator, "manualCertificateLabel")),
        source: Some("manual".to_string()),
        primary_domain: None,
        source_ref_id: None,
        cert: body.ssl.cert,
        key: body.ssl.key,
        activate: Some(true),
    };
    match save_ssl_certificate(&state, body, true).await {
        Ok(_) => match sync_ssl_deployment_to_gateway(&state, None).await {
            Ok(()) => response::success_empty().into_response(),
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

async fn activate_certificate(
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

async fn set_deployment_mode(
    State(state): State<AppState>,
    Json(body): Json<DeploymentModeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.redis.get_config().await {
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
    if let Err(error) = state.redis.save_config(&config).await {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ssl_error_or_route_text(&translator, "deploymentModeSaveFailed", &error),
        );
    }
    if let Err(error) = sync_ssl_deployment_to_gateway(&state, Some(&config)).await {
        let _ = state.redis.save_config(&previous).await;
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

async fn delete_certificate(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match delete_ssl_certificate(&state, &id).await {
        Ok((true, removed_active)) => {
            let config = match state.redis.get_config().await {
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

async fn clear_library(State(state): State<AppState>) -> Response {
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

async fn clear_ssl(State(state): State<AppState>) -> Response {
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

pub(crate) async fn build_ssl_status(state: &AppState) -> anyhow::Result<Value> {
    let translator = Translator::from_state(state).await;
    build_ssl_status_with_translator(state, &translator).await
}

async fn build_ssl_status_with_translator(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let local_status = local_ssl_status(&ssl);
    let gateway = gateway_ssl_status(state, translator).await;
    let gateway_status = gateway.as_ref().ok().and_then(|value| value.clone());
    let gateway_mode = gateway_status
        .as_ref()
        .and_then(|value| value.get("deployment_mode").and_then(Value::as_str));
    let effective_mode = if gateway_mode == Some("multi_sni") {
        "multi_sni".to_string()
    } else {
        local_status
            .get("deploymentMode")
            .and_then(Value::as_str)
            .unwrap_or("single_active")
            .to_string()
    };
    let enabled = gateway_status
        .as_ref()
        .and_then(|value| value.get("enabled").and_then(Value::as_bool))
        .unwrap_or_else(|| {
            local_status
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });

    let gateway_payload = if let Some(status) = gateway_status {
        json!({
            "enabled": status.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            "deployment_mode": if status.get("deployment_mode").and_then(Value::as_str) == Some("multi_sni") { "multi_sni" } else { "single_active" },
            "certificates": status.get("certificates").cloned().unwrap_or_else(|| json!([])),
            "sync_error": null
        })
    } else {
        json!({
            "enabled": false,
            "deployment_mode": "single_active",
            "certificates": [],
            "sync_error": gateway.err().unwrap_or_else(|| ssl_route_text(translator, "gatewayStatusReadFailed"))
        })
    };

    let mut status = local_status;
    let mut certificates = status
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for certificate in &mut certificates {
        let certificate_domains = certificate_dns_names(certificate);
        certificate["coverage"] = build_subdomain_certificate_coverage(
            state.settings.auth_port,
            &config,
            &certificate_domains,
            translator,
        );
    }
    let active_certificate_domains = status
        .get("certInfo")
        .map(certificate_info_dns_names)
        .unwrap_or_default();
    let active_certificate_id = status
        .get("activeCertId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let inventory_certificates = certificates
        .iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            Some(CertificateCoverageInput {
                id,
                certificate_domains: certificate_dns_names(certificate),
            })
        })
        .collect::<Vec<_>>();

    status["enabled"] = Value::Bool(enabled);
    status["configuredDeploymentMode"] = status
        .get("deploymentMode")
        .cloned()
        .unwrap_or_else(|| json!("single_active"));
    status["deploymentMode"] = json!(effective_mode);
    status["certificates"] = Value::Array(certificates);
    status["subdomain_coverage"] = build_subdomain_certificate_coverage(
        state.settings.auth_port,
        &config,
        &active_certificate_domains,
        translator,
    );
    status["library_coverage"] = build_subdomain_certificate_inventory_coverage(
        state.settings.auth_port,
        &config,
        &inventory_certificates,
        active_certificate_id.as_deref(),
        &effective_mode,
        translator,
    );
    status["gateway_status"] = gateway_payload;
    Ok(status)
}

async fn gateway_ssl_status(
    state: &AppState,
    translator: &Translator,
) -> Result<Option<Value>, String> {
    match state
        .go_backend
        .request_json_with_status::<Value>(Method::GET, "/api/ssl", None)
        .await
    {
        Ok((status, value)) if status.is_success() => {
            if value.get("success").and_then(Value::as_bool) == Some(false) {
                return Err(value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| ssl_route_text(translator, "gatewayStatusReadFailed")));
            }
            Ok(value.get("data").cloned().or(Some(value)))
        }
        Ok((status, value)) => Err(value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}: {status}",
                    ssl_route_text(translator, "gatewayStatusReadFailed")
                )
            })),
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway SSL status");
            Err(ssl_route_text(translator, "gatewayStatusReadFailed"))
        }
    }
}

fn local_ssl_status(ssl: &Value) -> Value {
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            Some(json!({
                "id": id,
                "label": certificate.get("label").and_then(Value::as_str).unwrap_or(""),
                "source": normalize_certificate_source(certificate.get("source").and_then(Value::as_str)),
                "primary_domain": optional_string(certificate.get("primary_domain")),
                "source_ref_id": optional_string(certificate.get("source_ref_id")),
                "created_at": certificate.get("created_at").and_then(Value::as_str).unwrap_or(""),
                "updated_at": certificate.get("updated_at").and_then(Value::as_str).unwrap_or(""),
                "certInfo": certificate.get("cert").and_then(Value::as_str).and_then(parse_cert_info),
                "is_active": id == active_id,
                "coverage": Value::Null
            }))
        })
        .collect::<Vec<_>>();
    let active = certificates
        .iter()
        .find(|item| item.get("is_active").and_then(Value::as_bool) == Some(true));
    json!({
        "enabled": active.is_some(),
        "activeCertId": active.and_then(|item| item.get("id").and_then(Value::as_str)),
        "deploymentMode": normalize_deployment_mode(ssl.get("deployment_mode").and_then(Value::as_str)),
        "certInfo": active.and_then(|item| item.get("certInfo").cloned()),
        "certificates": certificates
    })
}

async fn save_ssl_certificate(
    state: &AppState,
    input: SaveCertificateBody,
    activate: bool,
) -> anyhow::Result<Value> {
    validate_ssl_cert(&input.cert, &input.key)?;
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let mut certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cert = input.cert.trim().to_string();
    let key = input.key.trim().to_string();
    let id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            certificates
                .iter()
                .find(|item| {
                    item.get("cert").and_then(Value::as_str) == Some(cert.as_str())
                        && item.get("key").and_then(Value::as_str) == Some(key.as_str())
                })
                .and_then(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_else(|| build_ssl_certificate_id(&cert, &key));
    let existing = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .cloned();
    let now = time_utils::now_iso();
    let source = normalize_certificate_source(input.source.as_deref());
    let primary_domain = input
        .primary_domain
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let label = input
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|item| item.get("label").and_then(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_else(|| default_certificate_label(source, primary_domain.as_deref()));
    let created_at = existing
        .as_ref()
        .and_then(|item| item.get("created_at").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&now)
        .to_string();
    let next = json!({
        "id": id,
        "label": label,
        "source": source,
        "primary_domain": primary_domain,
        "source_ref_id": input.source_ref_id.as_deref().map(str::trim).filter(|value| !value.is_empty()),
        "cert": cert,
        "key": key,
        "created_at": created_at,
        "updated_at": now
    });
    certificates.retain(|item| {
        item.get("id").and_then(Value::as_str) != next.get("id").and_then(Value::as_str)
    });
    certificates.insert(0, next.clone());
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(certificates);
    if activate {
        let id = next.get("id").and_then(Value::as_str).unwrap_or("");
        next_ssl = mirror_active_ssl_certificate(&next_ssl, Some(id));
    }
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok(next)
}

pub(crate) async fn save_acme_certificate_to_library(
    state: &AppState,
    id: Option<&str>,
    label: Option<&str>,
    primary_domain: &str,
    source_ref_id: Option<&str>,
    cert: &str,
    key: &str,
    activate: bool,
) -> anyhow::Result<Value> {
    let normalized_domain = primary_domain.trim().to_ascii_lowercase();
    let normalized_ref = source_ref_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut resolved_id = id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if resolved_id.is_none() {
        resolved_id = find_acme_ssl_certificate(state, normalized_ref, Some(&normalized_domain))
            .await?
            .and_then(|certificate| {
                certificate
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
    }

    save_ssl_certificate(
        state,
        SaveCertificateBody {
            id: resolved_id,
            label: label
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            source: Some("acme".to_string()),
            primary_domain: Some(normalized_domain),
            source_ref_id: normalized_ref.map(str::to_string),
            cert: cert.to_string(),
            key: key.to_string(),
            activate: Some(activate),
        },
        activate,
    )
    .await
}

pub(crate) async fn get_acme_ssl_certificate_by_source_ref(
    state: &AppState,
    source_ref_id: &str,
) -> anyhow::Result<Option<Value>> {
    find_acme_ssl_certificate(state, Some(source_ref_id), None).await
}

pub(crate) async fn active_ssl_certificate_id(state: &AppState) -> anyhow::Result<Option<String>> {
    let config = state.redis.get_config().await?;
    Ok(normalize_ssl_config(config.get("ssl"))
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

pub(crate) async fn auto_select_certificate_for_subdomain(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Option<Value>> {
    let config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_certificate_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let deployment_mode =
        normalize_deployment_mode(ssl.get("deployment_mode").and_then(Value::as_str));
    let inventory_certificates = certificates
        .iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            Some(CertificateCoverageInput {
                id,
                certificate_domains: certificate
                    .get("cert")
                    .and_then(Value::as_str)
                    .and_then(parse_cert_info)
                    .map(|info| certificate_info_dns_names(&info))
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    let coverage = build_subdomain_certificate_inventory_coverage(
        state.settings.auth_port,
        &config,
        &inventory_certificates,
        active_certificate_id.as_deref(),
        deployment_mode,
        translator,
    );
    if coverage.get("can_auto_activate").and_then(Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let Some(suggested_certificate_id) = coverage
        .get("suggested_certificate_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(None);
    };

    let Some(candidate) =
        set_active_ssl_certificate(state, Some(suggested_certificate_id.as_str())).await?
    else {
        return Ok(None);
    };

    match sync_ssl_deployment_to_gateway(state, None).await {
        Ok(()) => Ok(Some(json!({
            "applied": true,
            "certificate_id": candidate.get("id").and_then(Value::as_str).unwrap_or(""),
            "label": candidate.get("label").and_then(Value::as_str).unwrap_or(""),
            "message": translator.t("server.admin.subdomainMode.sslAutoSelected")
        }))),
        Err(error) => {
            let _ = set_active_ssl_certificate(state, active_certificate_id.as_deref()).await;
            let _ = sync_ssl_deployment_to_gateway(state, None).await;
            let detail = error.to_string();
            let message = if detail.trim().is_empty() {
                translator.t("server.admin.subdomainMode.sslAutoSelectionSyncFailed")
            } else {
                detail
            };
            Ok(Some(json!({
                "applied": false,
                "certificate_id": candidate.get("id").and_then(Value::as_str).unwrap_or(""),
                "label": candidate.get("label").and_then(Value::as_str).unwrap_or(""),
                "message": message
            })))
        }
    }
}

async fn find_acme_ssl_certificate(
    state: &AppState,
    source_ref_id: Option<&str>,
    primary_domain: Option<&str>,
) -> anyhow::Result<Option<Value>> {
    let config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let normalized_ref = source_ref_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_domain = primary_domain
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    Ok(ssl
        .get("certificates")
        .and_then(Value::as_array)
        .and_then(|certificates| {
            certificates
                .iter()
                .find(|certificate| {
                    certificate.get("source").and_then(Value::as_str) == Some("acme")
                        && (normalized_ref.is_some_and(|id| {
                            certificate.get("source_ref_id").and_then(Value::as_str) == Some(id)
                        }) || normalized_domain.as_deref().is_some_and(|domain| {
                            certificate
                                .get("primary_domain")
                                .and_then(Value::as_str)
                                .is_some_and(|value| value.trim().eq_ignore_ascii_case(domain))
                        }))
                })
                .cloned()
        }))
}

async fn activate_ssl_certificate(state: &AppState, id: &str) -> anyhow::Result<bool> {
    Ok(set_active_ssl_certificate(state, Some(id)).await?.is_some())
}

async fn set_active_ssl_certificate(
    state: &AppState,
    id: Option<&str>,
) -> anyhow::Result<Option<Value>> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let normalized_id = id.map(str::trim).filter(|value| !value.is_empty());
    let candidate = normalized_id.and_then(|id| {
        ssl.get("certificates")
            .and_then(Value::as_array)?
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
            .cloned()
    });
    if normalized_id.is_some() && candidate.is_none() {
        return Ok(None);
    }
    config["ssl"] = mirror_active_ssl_certificate(&ssl, normalized_id);
    state.redis.save_config(&config).await?;
    Ok(candidate)
}

async fn delete_ssl_certificate(state: &AppState, id: &str) -> anyhow::Result<(bool, bool)> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let removed = certificates
        .iter()
        .any(|item| item.get("id").and_then(Value::as_str) == Some(id));
    if !removed {
        return Ok((false, false));
    }
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(
        certificates
            .into_iter()
            .filter(|item| item.get("id").and_then(Value::as_str) != Some(id))
            .collect(),
    );
    let removed_active = active_id == id;
    next_ssl = mirror_active_ssl_certificate(
        &next_ssl,
        if removed_active {
            None
        } else {
            Some(&active_id)
        },
    );
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok((true, removed_active))
}

pub(crate) async fn delete_acme_ssl_certificates(
    state: &AppState,
    application_id: Option<&str>,
    primary_domain: Option<&str>,
) -> anyhow::Result<(usize, bool)> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let normalized_application_id = application_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_primary_domain = primary_domain
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for certificate in certificates {
        let is_acme = certificate.get("source").and_then(Value::as_str) == Some("acme");
        let matches_ref = normalized_application_id
            .is_some_and(|id| certificate.get("source_ref_id").and_then(Value::as_str) == Some(id));
        let matches_domain = normalized_primary_domain.as_deref().is_some_and(|domain| {
            certificate
                .get("primary_domain")
                .and_then(Value::as_str)
                .is_some_and(|value| value.trim().eq_ignore_ascii_case(domain))
        });
        if is_acme && (matches_ref || matches_domain) {
            removed.push(certificate);
        } else {
            kept.push(certificate);
        }
    }

    if removed.is_empty() {
        return Ok((0, false));
    }

    let removed_active = removed
        .iter()
        .any(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()));
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(kept);
    next_ssl = mirror_active_ssl_certificate(
        &next_ssl,
        if removed_active {
            None
        } else {
            Some(&active_id)
        },
    );
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok((removed.len(), removed_active))
}

async fn clear_ssl_certificate_library(state: &AppState) -> anyhow::Result<()> {
    let mut config = state.redis.get_config().await?;
    let mut ssl = normalize_ssl_config(config.get("ssl"));
    ssl["certificates"] = json!([]);
    config["ssl"] = mirror_active_ssl_certificate(&ssl, None);
    state.redis.save_config(&config).await?;
    Ok(())
}

async fn clear_active_ssl(state: &AppState) -> anyhow::Result<()> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    config["ssl"] = mirror_active_ssl_certificate(&ssl, None);
    state.redis.save_config(&config).await?;
    Ok(())
}

struct CaPaths {
    dir: PathBuf,
    cert: PathBuf,
    key: PathBuf,
}

fn ca_paths(state: &AppState) -> CaPaths {
    let dir = state.settings.data_dir.join("ssl");
    CaPaths {
        cert: dir.join(CA_CERT_FILENAME),
        key: dir.join(CA_KEY_FILENAME),
        dir,
    }
}

fn init_root_ca(state: &AppState) -> anyhow::Result<Value> {
    let paths = ca_paths(state);
    std::fs::create_dir_all(&paths.dir)?;
    let subject = "/CN=KCI-LNK Root Certificate Authority/O=KCI-LNK Corporation/OU=Information Security Department/C=TW/ST=Taiwan/L=Taipei";
    run_openssl(vec![
        "req".to_string(),
        "-x509".to_string(),
        "-newkey".to_string(),
        "rsa:2048".to_string(),
        "-sha256".to_string(),
        "-days".to_string(),
        (20 * 365).to_string(),
        "-nodes".to_string(),
        "-keyout".to_string(),
        paths.key.to_string_lossy().to_string(),
        "-out".to_string(),
        paths.cert.to_string_lossy().to_string(),
        "-subj".to_string(),
        subject.to_string(),
        "-addext".to_string(),
        "basicConstraints=critical,CA:TRUE,pathlen:0".to_string(),
        "-addext".to_string(),
        "keyUsage=critical,keyCertSign,cRLSign,digitalSignature".to_string(),
    ])?;
    chmod_private(&paths.cert);
    chmod_private(&paths.key);
    let cert = std::fs::read_to_string(&paths.cert)?;
    Ok(parse_cert_info(&cert).unwrap_or_else(|| json!({})))
}

fn issue_ca_server_cert(state: &AppState, hosts: &[String]) -> anyhow::Result<(String, String)> {
    let paths = ca_paths(state);
    if !paths.cert.exists() || !paths.key.exists() {
        anyhow::bail!("Root CA not initialized");
    }
    let clean_hosts = hosts
        .iter()
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty())
        .collect::<Vec<_>>();
    if clean_hosts.is_empty() {
        anyhow::bail!("No hosts configured");
    }
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-ca-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let result = (|| {
        let key_path = temp_dir.join("server-key.pem");
        let csr_path = temp_dir.join("server.csr");
        let cert_path = temp_dir.join("server-cert.pem");
        let config_path = temp_dir.join("openssl.cnf");
        std::fs::write(&config_path, openssl_server_cert_config(&clean_hosts))?;
        run_openssl(vec![
            "genrsa".to_string(),
            "-out".to_string(),
            key_path.to_string_lossy().to_string(),
            "2048".to_string(),
        ])?;
        run_openssl(vec![
            "req".to_string(),
            "-new".to_string(),
            "-key".to_string(),
            key_path.to_string_lossy().to_string(),
            "-out".to_string(),
            csr_path.to_string_lossy().to_string(),
            "-config".to_string(),
            config_path.to_string_lossy().to_string(),
        ])?;
        run_openssl(vec![
            "x509".to_string(),
            "-req".to_string(),
            "-in".to_string(),
            csr_path.to_string_lossy().to_string(),
            "-CA".to_string(),
            paths.cert.to_string_lossy().to_string(),
            "-CAkey".to_string(),
            paths.key.to_string_lossy().to_string(),
            "-CAcreateserial".to_string(),
            "-out".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-days".to_string(),
            (20 * 365).to_string(),
            "-sha256".to_string(),
            "-extensions".to_string(),
            "v3_req".to_string(),
            "-extfile".to_string(),
            config_path.to_string_lossy().to_string(),
        ])?;
        let cert = std::fs::read_to_string(cert_path)?;
        let key = std::fs::read_to_string(key_path)?;
        validate_ssl_cert(&cert, &key)?;
        Ok((cert, key))
    })();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

fn openssl_server_cert_config(hosts: &[String]) -> String {
    let common_name = hosts
        .first()
        .map(|host| openssl_dn_value(host))
        .unwrap_or_else(|| "KCI-LNK Root Certificate".to_string());
    let mut dns_index = 1;
    let mut ip_index = 1;
    let mut alt_names = Vec::new();
    for host in hosts {
        if host.parse::<IpAddr>().is_ok() {
            alt_names.push(format!("IP.{ip_index} = {host}"));
            ip_index += 1;
        } else {
            alt_names.push(format!("DNS.{dns_index} = {host}"));
            dns_index += 1;
        }
    }
    format!(
        "[req]\nprompt = no\ndistinguished_name = req_distinguished_name\nreq_extensions = v3_req\n\n[req_distinguished_name]\nCN = {common_name}\n\n[v3_req]\nbasicConstraints = CA:FALSE\nkeyUsage = digitalSignature, keyEncipherment\nextendedKeyUsage = serverAuth\nsubjectAltName = @alt_names\n\n[alt_names]\n{}\n",
        alt_names.join("\n")
    )
}

fn openssl_dn_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "")
        .replace('\r', "")
}

fn run_openssl(args: Vec<String>) -> anyhow::Result<()> {
    run_openssl_capture(args).map(|_| ())
}

fn run_openssl_capture(args: Vec<String>) -> anyhow::Result<String> {
    let output = Command::new("openssl")
        .args(&args)
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    )
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .rev()
    .take(8)
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
    .collect::<Vec<_>>()
    .join(" | ");
    Err(anyhow!(
        "{}",
        if detail.is_empty() {
            "openssl command failed".to_string()
        } else {
            detail
        }
    ))
}

fn validate_ssl_cert_pair(cert: &str, key: &str) -> Result<(), SslValidationError> {
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-ssl-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;
    let result = (|| {
        let cert_path = temp_dir.join("cert.pem");
        let key_path = temp_dir.join("key.pem");
        std::fs::write(&cert_path, cert)
            .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;
        std::fs::write(&key_path, key)
            .map_err(|error| SslValidationError::CertKeyCheckFailed(error.to_string()))?;

        let cert_public_key = run_openssl_capture(vec![
            "x509".to_string(),
            "-in".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-noout".to_string(),
            "-pubkey".to_string(),
        ])
        .map_err(|error| SslValidationError::CertFormatInvalid(error.to_string()))?;
        let key_public_key = run_openssl_capture(vec![
            "pkey".to_string(),
            "-in".to_string(),
            key_path.to_string_lossy().to_string(),
            "-pubout".to_string(),
        ])
        .map_err(|error| SslValidationError::KeyFormatInvalid(error.to_string()))?;

        if normalize_public_key_pem(&cert_public_key) != normalize_public_key_pem(&key_public_key) {
            return Err(SslValidationError::CertKeyMismatch);
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

fn normalize_public_key_pem(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn get_ca_hosts(state: &AppState) -> redis::RedisResult<Vec<String>> {
    Ok(state
        .redis
        .get_json_value(CA_HOSTS_KEY)
        .await?
        .and_then(|value| {
            value.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default())
}

async fn save_ca_hosts(state: &AppState, hosts: &[String]) -> redis::RedisResult<()> {
    state
        .redis
        .set_json_value(CA_HOSTS_KEY, &json!(hosts))
        .await
}

async fn add_ca_host_inner(state: &AppState, host: &str) -> redis::RedisResult<Vec<String>> {
    let mut hosts = get_ca_hosts(state).await?;
    let host = host.trim();
    if !host.is_empty() && !hosts.iter().any(|item| item == host) {
        hosts.push(host.to_string());
        save_ca_hosts(state, &hosts).await?;
    }
    Ok(hosts)
}

async fn remove_ca_host_inner(state: &AppState, host: &str) -> redis::RedisResult<Vec<String>> {
    let mut hosts = get_ca_hosts(state).await?;
    let before = hosts.len();
    hosts.retain(|item| item != host.trim());
    if hosts.len() != before {
        save_ca_hosts(state, &hosts).await?;
    }
    Ok(hosts)
}

#[cfg(unix)]
fn chmod_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn chmod_private(_path: &Path) {}

pub(crate) async fn sync_ssl_deployment_to_gateway(
    state: &AppState,
    config: Option<&Value>,
) -> anyhow::Result<()> {
    let owned_config;
    let config = match config {
        Some(config) => config,
        None => {
            owned_config = state.redis.get_config().await?;
            &owned_config
        }
    };
    let deployment = build_gateway_ssl_deployment(config.get("ssl"));
    let certificates = deployment
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (status, value) = if certificates.is_empty() {
        state
            .go_backend
            .request_json_with_status::<Value>(Method::DELETE, "/api/ssl", None)
            .await?
    } else {
        state
            .go_backend
            .request_json_with_status(Method::POST, "/api/ssl", Some(&deployment))
            .await?
    };
    if !status.is_success() || value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(anyhow!(
            "{}",
            value.get("message").and_then(Value::as_str).unwrap_or("")
        ));
    }
    Ok(())
}

fn build_gateway_ssl_deployment(ssl: Option<&Value>) -> Value {
    let ssl = normalize_ssl_config(ssl);
    let deployment_mode =
        normalize_deployment_mode(ssl.get("deployment_mode").and_then(Value::as_str));
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let active = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()))
        .cloned();
    if deployment_mode != "multi_sni" {
        return json!({
            "deployment_mode": "single_active",
            "certificates": active.as_ref().map(|certificate| gateway_certificate_payload(certificate, true)).into_iter().collect::<Vec<_>>()
        });
    }
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(active) = active.clone() {
        if let Some(id) = active.get("id").and_then(Value::as_str) {
            seen.insert(id.to_string());
        }
        ordered.push(active.clone());
    }
    for certificate in certificates {
        let id = certificate.get("id").and_then(Value::as_str).unwrap_or("");
        if !id.is_empty() && seen.insert(id.to_string()) {
            ordered.push(certificate);
        }
    }
    json!({
        "deployment_mode": "multi_sni",
        "certificates": ordered.iter().enumerate().map(|(index, certificate)| {
            let is_default = if active.is_some() {
                certificate.get("id").and_then(Value::as_str) == Some(active_id.as_str())
            } else {
                index == 0
            };
            gateway_certificate_payload(certificate, is_default)
        }).collect::<Vec<_>>()
    })
}

fn gateway_certificate_payload(certificate: &Value, is_default: bool) -> Value {
    json!({
        "id": certificate.get("id").and_then(Value::as_str).unwrap_or(""),
        "label": certificate.get("label").and_then(Value::as_str).unwrap_or(""),
        "cert": certificate.get("cert").and_then(Value::as_str).unwrap_or(""),
        "key": certificate.get("key").and_then(Value::as_str).unwrap_or(""),
        "is_default": is_default
    })
}

fn normalize_ssl_config(value: Option<&Value>) -> Value {
    let raw = value.cloned().unwrap_or_else(|| json!({}));
    let mut certificates = raw
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_managed_certificate)
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    certificates.retain(|item| {
        let id = item.get("id").and_then(Value::as_str).unwrap_or("");
        !id.is_empty() && seen.insert(id.to_string())
    });
    let legacy_cert = raw.get("cert").and_then(Value::as_str).unwrap_or("").trim();
    let legacy_key = raw.get("key").and_then(Value::as_str).unwrap_or("").trim();
    let mut legacy_match_id = String::new();
    if !legacy_cert.is_empty() && !legacy_key.is_empty() {
        legacy_match_id = certificates
            .iter()
            .find(|item| {
                item.get("cert").and_then(Value::as_str) == Some(legacy_cert)
                    && item.get("key").and_then(Value::as_str) == Some(legacy_key)
            })
            .and_then(|item| item.get("id").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        if legacy_match_id.is_empty() {
            if let Some(migrated) = normalize_managed_certificate(json!({
                "id": build_ssl_certificate_id(legacy_cert, legacy_key),
                "label": default_certificate_label("current", None),
                "source": "manual",
                "cert": legacy_cert,
                "key": legacy_key
            })) {
                legacy_match_id = migrated
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                certificates.insert(0, migrated);
            }
        }
    }
    let active_id = raw
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| {
            certificates
                .iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(*id))
        })
        .unwrap_or(&legacy_match_id)
        .to_string();
    let active = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()));
    json!({
        "cert": active.and_then(|item| item.get("cert").and_then(Value::as_str)).unwrap_or(""),
        "key": active.and_then(|item| item.get("key").and_then(Value::as_str)).unwrap_or(""),
        "active_cert_id": active.and_then(|item| item.get("id").and_then(Value::as_str)).unwrap_or(""),
        "deployment_mode": normalize_deployment_mode(raw.get("deployment_mode").and_then(Value::as_str)),
        "certificates": certificates
    })
}

fn normalize_managed_certificate(value: Value) -> Option<Value> {
    let cert = value
        .get("cert")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    let key = value.get("key").and_then(Value::as_str)?.trim().to_string();
    if cert.is_empty() || key.is_empty() {
        return None;
    }
    let source = normalize_certificate_source(value.get("source").and_then(Value::as_str));
    let primary_domain = value
        .get("primary_domain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| build_ssl_certificate_id(&cert, &key));
    let label = value
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_certificate_label(source, primary_domain.as_deref()));
    Some(json!({
        "id": id,
        "label": label,
        "source": source,
        "primary_domain": primary_domain,
        "source_ref_id": optional_string(value.get("source_ref_id")),
        "cert": cert,
        "key": key,
        "created_at": normalize_timestamp(value.get("created_at")).unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        "updated_at": normalize_timestamp(value.get("updated_at")).unwrap_or_else(|| normalize_timestamp(value.get("created_at")).unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()))
    }))
}

fn mirror_active_ssl_certificate(ssl: &Value, active_id: Option<&str>) -> Value {
    let normalized = normalize_ssl_config(Some(ssl));
    let active = active_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|id| {
            normalized
                .get("certificates")
                .and_then(Value::as_array)?
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        });
    let active_cert = active
        .and_then(|item| item.get("cert").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let active_key = active
        .and_then(|item| item.get("key").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let active_id = active
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let mut next = normalized;
    next["cert"] = json!(active_cert);
    next["key"] = json!(active_key);
    next["active_cert_id"] = json!(active_id);
    next
}

fn validate_ssl_cert(cert: &str, key: &str) -> anyhow::Result<()> {
    validate_ssl_cert_pair(cert, key).map_err(|error| anyhow!(ssl_validation_error_plain(error)))
}

pub(crate) fn parse_cert_info(cert_pem: &str) -> Option<Value> {
    let (_, pem) = parse_x509_pem(cert_pem.as_bytes()).ok()?;
    let cert = pem.parse_x509().ok()?;
    let mut dns_names = Vec::new();
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in &san.value.general_names {
            match name {
                GeneralName::DNSName(value) => dns_names.push(value.to_string()),
                GeneralName::IPAddress(bytes) => {
                    if bytes.len() == 4 {
                        dns_names.push(format!(
                            "{}.{}.{}.{}",
                            bytes[0], bytes[1], bytes[2], bytes[3]
                        ));
                    } else if bytes.len() == 16 {
                        let mut segments = [0_u16; 8];
                        for (index, chunk) in bytes.chunks(2).enumerate().take(8) {
                            segments[index] = u16::from_be_bytes([chunk[0], chunk[1]]);
                        }
                        dns_names.push(
                            std::net::Ipv6Addr::new(
                                segments[0],
                                segments[1],
                                segments[2],
                                segments[3],
                                segments[4],
                                segments[5],
                                segments[6],
                                segments[7],
                            )
                            .to_string(),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(cn) = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(str::to_string)
    {
        if !dns_names
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(&cn))
        {
            dns_names.push(cn);
        }
    }
    Some(json!({
        "issuer": cert.issuer().to_string(),
        "subject": cert.subject().to_string(),
        "validFrom": cert.validity().not_before.to_string(),
        "validTo": cert.validity().not_after.to_string(),
        "dnsNames": dns_names,
        "serialNumber": cert.raw_serial_as_string()
    }))
}

fn build_subdomain_certificate_recommendation(
    auth_port: u16,
    config: &Value,
    t: &Translator,
) -> Value {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let auth_host = auth_host_mapping(auth_port, config)
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
                .map(normalize_domain_name)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let all_hosts = host_mapping_hosts(config);

    let mut mode = "manual";
    let mut summary = subdomain_text(t, "recommendationMissingBase");
    let mut warnings = Vec::<String>::new();
    let mut recommended_domains = Vec::<String>::new();

    if !root_domain.is_empty() {
        mode = "wildcard_parent";
        let wildcard_domain = format!("*.{root_domain}");
        recommended_domains = uniq_domain_strings([root_domain.as_str(), wildcard_domain.as_str()]);
        summary = subdomain_text_params(
            t,
            "recommendationWildcardSummary",
            &[("rootDomain", root_domain.clone())],
        );
        if !auth_host.is_empty()
            && !is_requirement_covered_by_certificate_domains(&auth_host, &recommended_domains)
        {
            recommended_domains = uniq_domain_strings(
                recommended_domains
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(auth_host.as_str())),
            );
            warnings.push(subdomain_text_params(
                t,
                "authOutOfRootWarning",
                &[
                    ("authHost", auth_host.clone()),
                    ("rootDomain", root_domain.clone()),
                ],
            ));
        }
    } else if !auth_host.is_empty() {
        mode = "single_host";
        recommended_domains = vec![auth_host.clone()];
        summary = subdomain_text_params(
            t,
            "recommendationSingleHostSummary",
            &[("authHost", auth_host.clone())],
        );
        warnings.push(subdomain_text(t, "wildcardSuggestion"));
    } else {
        warnings.push(subdomain_text(t, "configureRootOrAuth"));
    }

    if auth_host.is_empty() {
        warnings.push(subdomain_text(t, "authMissingWarning"));
    }

    let covered_hosts = all_hosts
        .iter()
        .filter(|host| is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_hosts = all_hosts
        .iter()
        .filter(|host| !is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();

    if !uncovered_hosts.is_empty() && !recommended_domains.is_empty() {
        warnings.push(subdomain_text_params(
            t,
            "uncoveredHostMappingsWarning",
            &[("count", uncovered_hosts.len().to_string())],
        ));
    }

    json!({
        "mode": mode,
        "root_domain": if root_domain.is_empty() { Value::Null } else { json!(root_domain) },
        "auth_host": if auth_host.is_empty() { Value::Null } else { json!(auth_host) },
        "recommended_domains": recommended_domains,
        "covered_hosts": covered_hosts,
        "uncovered_hosts": uncovered_hosts,
        "warnings": warnings,
        "can_autofill": !recommended_domains.is_empty(),
        "summary": summary,
    })
}

fn build_subdomain_certificate_coverage(
    auth_port: u16,
    config: &Value,
    certificate_domains: &[String],
    t: &Translator,
) -> Value {
    let recommendation = build_subdomain_certificate_recommendation(auth_port, config, t);
    let current_certificate_domains =
        uniq_domain_strings(certificate_domains.iter().map(String::as_str));
    let all_hosts = host_mapping_hosts(config);
    let recommended_domains = recommendation_domains(&recommendation);
    let auth_host = recommendation
        .get("auth_host")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let concrete_requirements = uniq_domain_strings(
        std::iter::once(auth_host.as_str()).chain(all_hosts.iter().map(String::as_str)),
    );
    let effective_requirements = if concrete_requirements.is_empty() {
        recommended_domains.clone()
    } else {
        concrete_requirements.clone()
    };
    let covered_recommended_domains = recommended_domains
        .iter()
        .filter(|domain| {
            is_requirement_covered_by_certificate_domains(domain, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_recommended_domains = recommended_domains
        .iter()
        .filter(|domain| {
            !is_requirement_covered_by_certificate_domains(domain, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let covered_hosts = all_hosts
        .iter()
        .filter(|host| {
            is_requirement_covered_by_certificate_domains(host, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_hosts = all_hosts
        .iter()
        .filter(|host| {
            !is_requirement_covered_by_certificate_domains(host, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let covers_auth_host = if auth_host.is_empty() {
        false
    } else {
        is_requirement_covered_by_certificate_domains(&auth_host, &current_certificate_domains)
    };
    let covered_requirements = effective_requirements
        .iter()
        .filter(|requirement| {
            is_requirement_covered_by_certificate_domains(requirement, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_requirements = effective_requirements
        .iter()
        .filter(|requirement| {
            !is_requirement_covered_by_certificate_domains(
                requirement,
                &current_certificate_domains,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let has_concrete_requirements = !concrete_requirements.is_empty();

    let mut status = "missing";
    let mut summary = subdomain_text(t, "coverageNoSsl");
    let mut warnings = recommendation
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if current_certificate_domains.is_empty() {
        if recommendation.get("can_autofill").and_then(Value::as_bool) != Some(true) {
            summary = recommendation
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
    } else if uncovered_requirements.is_empty() {
        status = "ready";
        summary = if has_concrete_requirements {
            subdomain_text(t, "coverageReadyConcrete")
        } else {
            subdomain_text(t, "coverageReadyRecommended")
        };
    } else if !covered_requirements.is_empty() {
        status = "partial";
        summary = if has_concrete_requirements {
            subdomain_text(t, "coveragePartialConcrete")
        } else {
            subdomain_text(t, "coveragePartialRecommended")
        };
    } else {
        summary = if has_concrete_requirements {
            subdomain_text(t, "coverageMismatchConcrete")
        } else {
            subdomain_text(t, "coverageMismatchRecommended")
        };
    }

    if !current_certificate_domains.is_empty()
        && has_concrete_requirements
        && !uncovered_requirements.is_empty()
    {
        warnings.push(subdomain_text_params(
            t,
            "coverageMissingRequiredWarning",
            &[("count", uncovered_requirements.len().to_string())],
        ));
    } else if !current_certificate_domains.is_empty()
        && !has_concrete_requirements
        && !uncovered_recommended_domains.is_empty()
    {
        warnings.push(subdomain_text_params(
            t,
            "coverageMissingRecommendedWarning",
            &[("count", uncovered_recommended_domains.len().to_string())],
        ));
    }

    if !current_certificate_domains.is_empty() && !auth_host.is_empty() && !covers_auth_host {
        warnings.push(subdomain_text_params(
            t,
            "coverageAuthHostMissingWarning",
            &[("authHost", auth_host.clone())],
        ));
    }

    json!({
        "status": status,
        "auth_host": if auth_host.is_empty() { Value::Null } else { json!(auth_host) },
        "certificate_domains": current_certificate_domains,
        "recommended_domains": recommended_domains,
        "covered_recommended_domains": covered_recommended_domains,
        "uncovered_recommended_domains": uncovered_recommended_domains,
        "covered_hosts": covered_hosts,
        "uncovered_hosts": uncovered_hosts,
        "covers_auth_host": covers_auth_host,
        "warnings": warnings,
        "summary": summary,
    })
}

fn build_subdomain_certificate_inventory_coverage(
    auth_port: u16,
    config: &Value,
    certificates: &[CertificateCoverageInput],
    active_certificate_id: Option<&str>,
    deployment_mode: &str,
    t: &Translator,
) -> Value {
    let recommendation = build_subdomain_certificate_recommendation(auth_port, config, t);
    let all_hosts = host_mapping_hosts(config);
    let recommended_domains = recommendation_domains(&recommendation);
    let auth_host = recommendation
        .get("auth_host")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let concrete_requirements = uniq_domain_strings(
        std::iter::once(auth_host.as_str()).chain(all_hosts.iter().map(String::as_str)),
    );
    let requirements = if concrete_requirements.is_empty() {
        recommended_domains
    } else {
        concrete_requirements
    };

    let analyses = certificates
        .iter()
        .map(|certificate| {
            let normalized_domains =
                uniq_domain_strings(certificate.certificate_domains.iter().map(String::as_str));
            let coverage =
                build_subdomain_certificate_coverage(auth_port, config, &normalized_domains, t);
            let covered_requirements = requirements
                .iter()
                .filter(|requirement| {
                    is_requirement_covered_by_certificate_domains(requirement, &normalized_domains)
                })
                .cloned()
                .collect::<Vec<_>>();
            CertificateCoverageAnalysis {
                id: certificate.id.clone(),
                coverage,
                covered_requirements,
            }
        })
        .collect::<Vec<_>>();

    let fully_covering = analyses
        .iter()
        .filter(|item| item.coverage.get("status").and_then(Value::as_str) == Some("ready"))
        .cloned()
        .collect::<Vec<_>>();
    let partially_covering = analyses
        .iter()
        .filter(|item| {
            item.coverage.get("status").and_then(Value::as_str) != Some("ready")
                && !item.covered_requirements.is_empty()
        })
        .cloned()
        .collect::<Vec<_>>();
    let active_analysis = active_certificate_id
        .and_then(|id| analyses.iter().find(|item| item.id == id))
        .cloned();

    let mut uncovered_requirements = requirements.iter().cloned().collect::<BTreeSet<_>>();
    let mut combined_covering_certificate_ids = Vec::<String>::new();
    let mut remaining = analyses.clone();

    while !uncovered_requirements.is_empty() && !remaining.is_empty() {
        let mut best_index = None;
        let mut best_gain = 0_usize;
        for (index, item) in remaining.iter().enumerate() {
            let gain = item
                .covered_requirements
                .iter()
                .filter(|requirement| uncovered_requirements.contains(*requirement))
                .count();
            if gain > best_gain {
                best_gain = gain;
                best_index = Some(index);
            }
        }
        let Some(best_index) = best_index else {
            break;
        };
        if best_gain == 0 {
            break;
        }
        let selected = remaining.remove(best_index);
        combined_covering_certificate_ids.push(selected.id.clone());
        for requirement in selected.covered_requirements {
            uncovered_requirements.remove(&requirement);
        }
    }

    let combined_ready = !requirements.is_empty() && uncovered_requirements.is_empty();
    let active_ready = active_analysis
        .as_ref()
        .and_then(|item| item.coverage.get("status").and_then(Value::as_str))
        == Some("ready");
    let deployment_mode = normalize_deployment_mode(Some(deployment_mode));

    let mut status = "missing";
    let summary;
    let mut warnings = Vec::<String>::new();

    if active_ready {
        status = "ready";
        summary = subdomain_text(t, "inventoryActiveReady");
    } else if fully_covering.len() == 1 {
        status = "ready";
        summary = subdomain_text(t, "inventoryOneReady");
    } else if fully_covering.len() > 1 {
        status = "ready";
        summary = subdomain_text_params(
            t,
            "inventoryMultipleReady",
            &[("count", fully_covering.len().to_string())],
        );
    } else if combined_ready && deployment_mode == "multi_sni" {
        status = "ready";
        summary = if combined_covering_certificate_ids.len() > 1 {
            subdomain_text(t, "inventoryCombinedReady")
        } else {
            subdomain_text(t, "inventoryCandidateReady")
        };
    } else if combined_ready {
        status = "partial";
        summary = subdomain_text(t, "inventoryCombinedNeedsMultiSni");
    } else if !partially_covering.is_empty() {
        status = "partial";
        summary = subdomain_text(t, "inventoryPartialCandidates");
    } else if recommendation.get("can_autofill").and_then(Value::as_bool) == Some(true) {
        summary = subdomain_text(t, "inventoryNoCertificateCoversRecommendation");
    } else {
        summary = recommendation
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }

    if combined_ready
        && combined_covering_certificate_ids.len() > 1
        && deployment_mode != "multi_sni"
    {
        warnings.push(subdomain_text(t, "inventoryMultiCertRequiresSniWarning"));
    }
    if active_analysis.is_some() && !active_ready && fully_covering.len() == 1 {
        warnings.push(subdomain_text(t, "inventorySwitchRecommendedWarning"));
    }
    if active_analysis.is_none()
        && fully_covering.is_empty()
        && combined_covering_certificate_ids.len() > 1
    {
        warnings.push(subdomain_text(t, "inventoryBetterForSniWarning"));
    }

    let suggested_certificate_id = if active_ready || fully_covering.len() != 1 {
        Value::Null
    } else {
        json!(fully_covering[0].id)
    };

    json!({
        "status": status,
        "deployment_mode": deployment_mode,
        "active_certificate_id": active_analysis.as_ref().map(|item| item.id.as_str()),
        "fully_covering_certificate_ids": fully_covering.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "partially_covering_certificate_ids": partially_covering.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "combined_covering_certificate_ids": combined_covering_certificate_ids,
        "suggested_certificate_id": suggested_certificate_id,
        "can_auto_activate": !active_ready && fully_covering.len() == 1,
        "warnings": warnings,
        "summary": summary,
    })
}

#[derive(Clone, Debug)]
struct CertificateCoverageAnalysis {
    id: String,
    coverage: Value,
    covered_requirements: Vec<String>,
}

fn certificate_dns_names(certificate: &Value) -> Vec<String> {
    certificate
        .get("certInfo")
        .map(certificate_info_dns_names)
        .unwrap_or_default()
}

fn certificate_info_dns_names(info: &Value) -> Vec<String> {
    info.get("dnsNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn recommendation_domains(recommendation: &Value) -> Vec<String> {
    recommendation
        .get("recommended_domains")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn host_mapping_hosts(config: &Value) -> Vec<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|mappings| {
            uniq_domain_strings(
                mappings
                    .iter()
                    .filter_map(|mapping| mapping.get("host").and_then(Value::as_str)),
            )
        })
        .unwrap_or_default()
}

fn auth_host_mapping(auth_port: u16, config: &Value) -> Option<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| is_auth_service_mapping(auth_port, mapping))
        .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
}

fn is_auth_service_mapping(auth_port: u16, mapping: &Value) -> bool {
    if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
        return true;
    }
    let target = mapping.get("target").and_then(Value::as_str).unwrap_or("");
    parse_target_port(target) == Some(auth_port)
}

fn parse_target_port(target: &str) -> Option<u16> {
    let parsed = url::Url::parse(target.trim()).ok()?;
    if let Some(port) = parsed.port() {
        return Some(port);
    }
    match parsed.scheme() {
        "https" | "wss" => Some(443),
        "http" | "ws" => Some(80),
        _ => None,
    }
}

fn uniq_domain_strings<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let normalized = normalize_domain_name(value);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        output.push(normalized);
    }
    output
}

fn normalize_domain_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn is_wildcard_domain(value: &str) -> bool {
    normalize_domain_name(value).starts_with("*.")
}

fn strip_wildcard_prefix(value: &str) -> String {
    let normalized = normalize_domain_name(value);
    normalized
        .strip_prefix("*.")
        .unwrap_or(normalized.as_str())
        .to_string()
}

fn does_pattern_cover_concrete_host(concrete_host: &str, pattern: &str) -> bool {
    let normalized_host = normalize_domain_name(concrete_host);
    let normalized_pattern = normalize_domain_name(pattern);
    if normalized_host.is_empty()
        || normalized_pattern.is_empty()
        || is_wildcard_domain(&normalized_host)
    {
        return false;
    }
    if !is_wildcard_domain(&normalized_pattern) {
        return normalized_host == normalized_pattern;
    }
    let suffix = strip_wildcard_prefix(&normalized_pattern);
    if suffix.is_empty() || !normalized_host.ends_with(&format!(".{suffix}")) {
        return false;
    }
    let label = &normalized_host[..normalized_host.len() - suffix.len() - 1];
    !label.is_empty() && !label.contains('.')
}

fn is_requirement_covered_by_certificate_domains(
    requirement: &str,
    certificate_domains: &[String],
) -> bool {
    let requirement = normalize_domain_name(requirement);
    if requirement.is_empty() {
        return false;
    }
    if is_wildcard_domain(&requirement) {
        return certificate_domains
            .iter()
            .any(|domain| normalize_domain_name(domain) == requirement);
    }
    certificate_domains
        .iter()
        .any(|domain| does_pattern_cover_concrete_host(&requirement, domain))
}

fn subdomain_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.subdomainMode.{key}"))
}

fn subdomain_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.subdomainMode.{key}"), params)
}

fn build_ssl_certificate_id(cert: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert.as_bytes());
    hasher.update(b"\n");
    hasher.update(key.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("ssl_{}", &digest[..16])
}

fn normalize_certificate_source(value: Option<&str>) -> &'static str {
    match value {
        Some("acme") => "acme",
        Some("ca") => "ca",
        _ => "manual",
    }
}

fn normalize_deployment_mode(value: Option<&str>) -> &'static str {
    if value == Some("multi_sni") {
        "multi_sni"
    } else {
        "single_active"
    }
}

fn should_sync_ssl_deployment_after_save(activate: bool, deployment_mode: &str) -> bool {
    activate || normalize_deployment_mode(Some(deployment_mode)) == "multi_sni"
}

fn default_certificate_label(source: &str, primary_domain: Option<&str>) -> String {
    if let Some(primary_domain) = primary_domain {
        return primary_domain.to_string();
    }
    let translator = Translator::new(DEFAULT_LOCALE);
    match source {
        "acme" => translator.t("server.redis.certificateLabels.acme"),
        "ca" => translator.t("server.redis.certificateLabels.ca"),
        "current" => translator.t("server.redis.certificateLabels.current"),
        _ => translator.t("server.redis.certificateLabels.manual"),
    }
}

fn optional_string(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value))
        .unwrap_or(Value::Null)
}

fn normalize_timestamp(value: Option<&Value>) -> Option<String> {
    let raw = value.and_then(Value::as_str)?.trim();
    (!raw.is_empty()).then_some(raw.to_string())
}

fn list_ssl_shared_files() -> Value {
    let Some(directory) = configured_share_directory() else {
        return json!({ "shareName": SSL_CERT_SHARE_NAME, "available": false, "files": [] });
    };
    if !directory.is_dir() {
        return json!({ "shareName": SSL_CERT_SHARE_NAME, "available": false, "files": [] });
    }
    let mut files = Vec::new();
    walk_shared_files(&directory, &directory, &mut files, 0);
    files.sort_by(|left, right| {
        let time = right
            .get("modifiedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(left.get("modifiedAt").and_then(Value::as_str).unwrap_or(""));
        if time == std::cmp::Ordering::Equal {
            left.get("relativePath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(
                    right
                        .get("relativePath")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                )
        } else {
            time
        }
    });
    json!({ "shareName": SSL_CERT_SHARE_NAME, "available": true, "files": files })
}

fn read_ssl_shared_file(relative_path: &str) -> anyhow::Result<Value> {
    let directory = configured_share_directory()
        .ok_or_else(|| anyhow!("Shared directory is not configured"))?;
    let file_path = resolve_share_path(&directory, relative_path)?;
    let metadata = std::fs::metadata(&file_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow!(SharedFileNotFound)
        } else if error.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow!(SharedFileForbidden)
        } else {
            anyhow!(error)
        }
    })?;
    if !metadata.is_file() {
        return Err(anyhow!("Shared path must be a file"));
    }
    if metadata.len() > MAX_SHARED_FILE_SIZE {
        return Err(anyhow!("Shared file is too large"));
    }
    let content = std::fs::read_to_string(&file_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow!(SharedFileForbidden)
        } else {
            anyhow!(error)
        }
    })?;
    Ok(json!({
        "file": shared_file_entry(&directory, &file_path, &metadata),
        "content": content.trim_start_matches('\u{feff}')
    }))
}

#[derive(Debug)]
struct SharedFileNotFound;

impl std::fmt::Display for SharedFileNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared file not found")
    }
}

impl std::error::Error for SharedFileNotFound {}

#[derive(Debug)]
struct SharedFileForbidden;

impl std::fmt::Display for SharedFileForbidden {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared file cannot be read")
    }
}

impl std::error::Error for SharedFileForbidden {}

fn configured_share_directory() -> Option<PathBuf> {
    if let Ok(value) =
        env::var("FN_KNOCK_ROOT_SHARE_DIR").or_else(|_| env::var("FN_KNOCK_CERT_SHARE_DIR"))
    {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let paths = env::var("TRIM_DATA_SHARE_PATHS").ok()?;
    paths
        .split(':')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .min_by_key(|value| value.len())
        .map(PathBuf::from)
}

fn walk_shared_files(root: &Path, current: &Path, bucket: &mut Vec<Value>, depth: usize) {
    if bucket.len() >= MAX_SHARED_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        if bucket.len() >= MAX_SHARED_FILES {
            return;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let path = entry.path();
        if metadata.is_dir() {
            if depth < MAX_SHARED_SCAN_DEPTH {
                walk_shared_files(root, &path, bucket, depth + 1);
            }
            continue;
        }
        if metadata.is_file() {
            bucket.push(shared_file_entry(root, &path, &metadata));
        }
    }
}

fn shared_file_entry(root: &Path, path: &Path, metadata: &std::fs::Metadata) -> Value {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    json!({
        "name": path.file_name().and_then(|value| value.to_str()).unwrap_or(""),
        "relativePath": relative,
        "extension": path.extension().and_then(|value| value.to_str()).map(|value| format!(".{}", value.to_ascii_lowercase())).unwrap_or_default(),
        "size": metadata.len(),
        "modifiedAt": metadata.modified().ok().map(system_time_iso).unwrap_or_else(time_utils::now_iso)
    })
}

fn resolve_share_path(root: &Path, relative_path: &str) -> anyhow::Result<PathBuf> {
    let sanitized = relative_path.replace('\\', "/").trim().to_string();
    if sanitized.is_empty() || sanitized.starts_with('/') {
        return Err(anyhow!("Invalid shared file path"));
    }
    let resolved = root.join(&sanitized);
    let normalized_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let normalized_parent = resolved
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .unwrap_or_else(|| root.to_path_buf());
    if !normalized_parent.starts_with(&normalized_root) {
        return Err(anyhow!("Invalid shared file path"));
    }
    Ok(resolved)
}

fn system_time_iso(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs() as i64;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(time_utils::now_iso)
}

pub(crate) fn zip_cert_pair(cert: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::<u8>::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("server-cert.pem", options)?;
    zip.write_all(cert.as_bytes())?;
    zip.start_file("server-key.pem", options)?;
    zip.write_all(key.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

fn pem_response(content: &str, filename: &str, content_type: &'static str) -> Response {
    binary_response(content.as_bytes().to_vec(), content_type, filename)
}

pub(crate) fn binary_response(
    bytes: Vec<u8>,
    content_type: &'static str,
    filename: &str,
) -> Response {
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIC3jCCAcagAwIBAgIJAMBvhSD/r2eYMA0GCSqGSIb3DQEBCwUAMBcxFTATBgNV
BAMMDGV4YW1wbGUudGVzdDAeFw0yNjA3MDQyMDUxMzdaFw0zNjA3MDEyMDUxMzda
MBcxFTATBgNVBAMMDGV4YW1wbGUudGVzdDCCASIwDQYJKoZIhvcNAQEBBQADggEP
ADCCAQoCggEBAK81U4+GWNLRNKq+y6/pslTHyvQZgX8bZQ68dijipzwyGLG5hnF0
ea+qXNicPvFb9MXGBL4i1GWnnp1x7T4d9WtFYi2Q+w2ZFoTJfmVtuwJhvmhFIBgI
nPyK3Sa/DDb886h5B/drD54wjpcFbEm7xbxHxzyRF3q1WZl69NevYDHiVhTa6n3s
x0XezCyuJ0GEgsqiJ5N61c3TLfwd1AJFV8WZnVUuUU4DzhSMadOrwSChd8s4jQ9A
+QZNrWYLBRxTAuJ2RYsPEgQ6sWw7k4//xJ4jhlzGi6AfS/FjvOGv+xCQlPhedSSM
/9qjo7m7oDhVXkbUJeIE7ZCWbGTW2B85fXECAwEAAaMtMCswKQYDVR0RBCIwIIIM
ZXhhbXBsZS50ZXN0ghBhbHQuZXhhbXBsZS50ZXN0MA0GCSqGSIb3DQEBCwUAA4IB
AQCTD4yYqhrVVL4pYaY1uyVqXV3/Ba6cFuXIExoe9XOljJu2M6I8D6KjWVtC9rVu
n+SwZed1BIdEKqv1sbdw45mMhJi1lYZe5QLFoRI+mB3/AjCx493ia8KSx7mrqO0y
Kc9jOEHzjkutbjTxoAhUdb9Pfwz6W9RIqZ2IpXxgIpDrQuRBp6yyw5/gpNQfPAt7
iQHXpmfpjC4kBqCEakPKpPURcBB4HY/tGg7tbqVLK6Q/Ujj/WAONeZuxB/mAtkiW
b6DS1sxh2TNX1zXA5idWls2foZDzzcC1XRB9iF+q7JCDdIYstLBgN23ZxJbDH3yS
uvwBvERVoHMCF4qFay/Qy8sf
-----END CERTIFICATE-----"#;

    #[test]
    fn ssl_certificate_ids_match_node_shape() {
        let id = build_ssl_certificate_id("cert", "key");
        assert!(id.starts_with("ssl_"));
        assert_eq!(id.len(), 20);
    }

    #[test]
    fn normalizes_legacy_ssl_into_library() {
        let ssl = normalize_ssl_config(Some(&json!({
            "cert": "CERT",
            "key": "KEY",
            "deployment_mode": "multi_sni"
        })));
        assert_eq!(ssl["deployment_mode"], json!("multi_sni"));
        assert_eq!(ssl["certificates"].as_array().unwrap().len(), 1);
        assert_eq!(ssl["active_cert_id"], ssl["certificates"][0]["id"]);
        assert_eq!(ssl["certificates"][0]["label"], json!("当前证书"));
    }

    #[test]
    fn ssl_save_sync_condition_matches_node() {
        assert!(should_sync_ssl_deployment_after_save(true, "single_active"));
        assert!(should_sync_ssl_deployment_after_save(false, "multi_sni"));
        assert!(!should_sync_ssl_deployment_after_save(
            false,
            "single_active"
        ));
        assert!(!should_sync_ssl_deployment_after_save(false, "bad"));
    }

    #[test]
    fn localizes_ssl_route_errors_and_default_labels() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            ssl_route_text(&zh, "rootCaNotInitialized"),
            "本地 CA 尚未初始化"
        );
        assert_eq!(
            localize_ssl_error(&zh, &anyhow!("Root CA not initialized")),
            "本地 CA 尚未初始化"
        );
        assert_eq!(
            localize_ssl_error(&zh, &anyhow!("No hosts configured")),
            "域名列表为空，请先添加域名或 IP"
        );
        assert_eq!(ssl_route_text(&zh, "certReadFailed"), "读取 SSL 证书失败");
        assert_eq!(
            ssl_route_text(&zh, "certZipCreateFailed"),
            "创建 SSL 证书压缩包失败"
        );
        assert_eq!(ssl_route_text(&zh, "caInitFailed"), "初始化本地 CA 失败");
        assert_eq!(
            ssl_route_text(&zh, "caHostLoadFailed"),
            "读取本地 CA Host 列表失败"
        );
        assert_eq!(
            ssl_error_or_route_text(&zh, "caInitFailed", &anyhow!("openssl command failed")),
            "初始化本地 CA 失败"
        );
        assert_eq!(
            ssl_error_or_route_text(
                &zh,
                "certSaveFailed",
                &anyhow!("Certificate format is invalid")
            ),
            "证书或私钥无效"
        );
        assert_eq!(
            validate_ssl_cert_for_response("", "", &zh).unwrap_err(),
            "证书内容不能为空"
        );
        assert_eq!(
            shared_file_error_status_and_message(&zh, &anyhow!("Invalid shared file path")),
            (StatusCode::BAD_REQUEST, "非法的共享文件路径".to_string())
        );
        assert_eq!(
            shared_file_error_status_and_message(
                &zh,
                &anyhow!("Shared directory is not configured")
            ),
            (
                StatusCode::NOT_FOUND,
                "未找到飞牛共享目录，请确认应用资源已正确配置".to_string()
            )
        );
        assert_eq!(
            shared_file_error_status_and_message(&zh, &anyhow!("Shared path must be a file")),
            (
                StatusCode::BAD_REQUEST,
                "只能读取共享目录中的文件".to_string()
            )
        );
        assert_eq!(
            shared_file_error_status_and_message(&zh, &anyhow!("Shared file is too large")),
            (
                StatusCode::BAD_REQUEST,
                "文件过大，请仅放入证书或私钥文本文件".to_string()
            )
        );
        assert_eq!(
            shared_file_error_status_and_message(&zh, &anyhow!(SharedFileForbidden)),
            (StatusCode::FORBIDDEN, "读取共享目录文件失败".to_string())
        );
        assert_eq!(default_certificate_label("manual", None), "手动上传证书");
        assert_eq!(default_certificate_label("ca", None), "自签发证书");
        assert_eq!(
            default_certificate_label("acme", Some("example.com")),
            "example.com"
        );
    }

    #[test]
    fn validates_ssl_certificate_private_key_match_like_node() {
        let Some((cert, key)) = generate_test_cert_pair("match.example.test") else {
            return;
        };
        let Some((_other_cert, other_key)) = generate_test_cert_pair("other.example.test") else {
            return;
        };
        let zh = Translator::new("zh-CN");

        assert!(validate_ssl_cert_for_response(&cert, &key, &zh).is_ok());
        assert_eq!(
            validate_ssl_cert_for_response(&cert, &other_key, &zh).unwrap_err(),
            "证书与私钥不匹配"
        );
    }

    fn generate_test_cert_pair(common_name: &str) -> Option<(String, String)> {
        if !Command::new("openssl")
            .arg("version")
            .stdin(Stdio::null())
            .output()
            .ok()?
            .status
            .success()
        {
            return None;
        }
        let temp_dir = std::env::temp_dir().join(format!("fn-knock-ssl-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).ok()?;
        let key_path = temp_dir.join("key.pem");
        let cert_path = temp_dir.join("cert.pem");
        let result = (|| {
            run_openssl(vec![
                "req".to_string(),
                "-x509".to_string(),
                "-newkey".to_string(),
                "rsa:2048".to_string(),
                "-sha256".to_string(),
                "-days".to_string(),
                "1".to_string(),
                "-nodes".to_string(),
                "-keyout".to_string(),
                key_path.to_string_lossy().to_string(),
                "-out".to_string(),
                cert_path.to_string_lossy().to_string(),
                "-subj".to_string(),
                format!("/CN={common_name}"),
            ])
            .ok()?;
            Some((
                std::fs::read_to_string(&cert_path).ok()?,
                std::fs::read_to_string(&key_path).ok()?,
            ))
        })();
        let _ = std::fs::remove_dir_all(temp_dir);
        result
    }

    #[test]
    fn builds_gateway_deployment_for_multi_sni_with_active_first() {
        let deployment = build_gateway_ssl_deployment(Some(&json!({
            "active_cert_id": "b",
            "deployment_mode": "multi_sni",
            "certificates": [
                {"id":"a","label":"A","cert":"CERTA","key":"KEYA"},
                {"id":"b","label":"B","cert":"CERTB","key":"KEYB"}
            ]
        })));
        assert_eq!(deployment["deployment_mode"], json!("multi_sni"));
        assert_eq!(deployment["certificates"][0]["id"], json!("b"));
        assert_eq!(deployment["certificates"][0]["is_default"], json!(true));
    }

    #[test]
    fn parses_certificate_info_when_pem_is_valid() {
        let info = parse_cert_info(SAMPLE_CERT).expect("certificate should parse");
        assert_eq!(info["dnsNames"][0], json!("example.test"));
        assert_eq!(info["dnsNames"][1], json!("alt.example.test"));
    }

    #[test]
    fn builds_subdomain_certificate_coverage_like_node() {
        let zh = Translator::new("zh-CN");
        let config = json!({
            "subdomain_mode": {
                "root_domain": "example.com"
            },
            "host_mappings": [
                {
                    "host": "auth.example.com",
                    "target": "http://127.0.0.1:7997",
                    "service_role": "auth"
                },
                {
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080"
                }
            ]
        });
        let certificate_domains = vec!["example.com".to_string(), "*.example.com".to_string()];

        let coverage =
            build_subdomain_certificate_coverage(7997, &config, &certificate_domains, &zh);

        assert_eq!(coverage["status"], json!("ready"));
        assert_eq!(coverage["covers_auth_host"], json!(true));
        assert_eq!(
            coverage["covered_hosts"],
            json!(["auth.example.com", "app.example.com"])
        );
        assert_eq!(coverage["uncovered_hosts"], json!([]));
    }

    #[test]
    fn subdomain_inventory_suggests_single_fully_covering_certificate() {
        let zh = Translator::new("zh-CN");
        let config = json!({
            "subdomain_mode": {
                "root_domain": "example.com"
            },
            "host_mappings": [
                {
                    "host": "auth.example.com",
                    "target": "http://127.0.0.1:7997",
                    "service_role": "auth"
                },
                {
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080"
                }
            ]
        });
        let certificates = vec![
            CertificateCoverageInput {
                id: "old".to_string(),
                certificate_domains: vec!["auth.example.com".to_string()],
            },
            CertificateCoverageInput {
                id: "recommended".to_string(),
                certificate_domains: vec!["example.com".to_string(), "*.example.com".to_string()],
            },
        ];

        let coverage = build_subdomain_certificate_inventory_coverage(
            7997,
            &config,
            &certificates,
            Some("old"),
            "single_active",
            &zh,
        );

        assert_eq!(coverage["status"], json!("ready"));
        assert_eq!(coverage["can_auto_activate"], json!(true));
        assert_eq!(coverage["suggested_certificate_id"], json!("recommended"));
        assert_eq!(
            coverage["fully_covering_certificate_ids"],
            json!(["recommended"])
        );
        assert_eq!(
            coverage["partially_covering_certificate_ids"],
            json!(["old"])
        );
        assert!(
            coverage["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|warning| {
                    warning.as_str()
                        == Some("当前活动证书与子域模式不完全匹配，建议切换到推荐证书。")
                })
        );
    }

    #[test]
    fn builds_ca_server_cert_config_with_dns_and_ip_sans() {
        let config = openssl_server_cert_config(&[
            "example.test".to_string(),
            "192.168.1.10".to_string(),
            "alt.example.test".to_string(),
        ]);
        assert!(config.contains("CN = example.test"));
        assert!(config.contains("DNS.1 = example.test"));
        assert!(config.contains("IP.1 = 192.168.1.10"));
        assert!(config.contains("DNS.2 = alt.example.test"));
    }

    #[test]
    fn cleans_openssl_dn_value_newlines() {
        assert_eq!(openssl_dn_value("example\n.test\r"), "example.test");
    }
}
