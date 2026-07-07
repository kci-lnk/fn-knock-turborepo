use std::{
    collections::BTreeSet,
    env,
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ::time::{OffsetDateTime, format_description::well_known::Rfc3339};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path as AxumPath, Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    time::{self as tokio_time, MissedTickBehavior},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{i18n::Translator, response, ssl, state::AppState, time_utils};

const ACME_APPLICATIONS_KEY: &str = "fn_knock:acme:applications";
const ACME_ISSUED_CERTIFICATES_KEY: &str = "fn_knock:acme:issued-certificates";
const ACME_LEGACY_SETTINGS_KEY: &str = "fn_knock:acme:settings";
const ACME_CLIENT_SETTINGS_KEY: &str = "fn_knock:acme:client-settings";
const ACME_MIGRATION_VERSION_KEY: &str = "fn_knock:acme:migration:v1";
const ACME_RUNTIME_LOCK_KEY: &str = "fn_knock:acme:runtime-lock";
const ACME_CERT_PREFIX: &str = "fn_knock:acme:cert:";
const ACME_JOB_PREFIX: &str = "fn_knock:acme:job:";
const ACME_LOGS_PREFIX: &str = "fn_knock:acme:logs:";
const DEFAULT_ACME_CERTIFICATE_AUTHORITY: &str = "zerossl";
const DEFAULT_ACME_LOG_LIMIT: usize = 500;
const MAX_ACME_LOG_LIMIT: usize = 1000;
const MAX_ACME_BODY_BYTES: usize = 1024 * 1024;
const ACME_JOB_TTL_SECONDS: usize = 86_400;
const ACME_RUNTIME_LOCK_MIN_TTL_SECONDS: usize = 300;
const ACME_RUNTIME_LOCK_MAX_TTL_SECONDS: usize = 6 * 60 * 60;

#[derive(Deserialize)]
struct AcmeLogsQuery {
    limit: Option<String>,
    order: Option<String>,
}

#[derive(Debug)]
struct NormalizedAcmeRequest {
    domains: Vec<String>,
    dns_type: String,
    credentials: Value,
}

struct SaveAcmeApplicationInput {
    id: Option<String>,
    name: Option<String>,
    name_provided: bool,
    domains: Vec<String>,
    dns_type: String,
    credentials: Value,
    renew_enabled: Option<bool>,
}

struct AcmeApplicationSaveOutcome {
    application: Value,
    removed_library_certificate_count: usize,
    removed_active_library_certificate: bool,
}

fn acme_route_text(t: &Translator, key: &str) -> String {
    t.t(&format!("server.acmeRoutes.{key}"))
}

pub fn acme_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/acme", delete(uninstall_acme))
        .route("/api/admin/acme/", delete(uninstall_acme))
        .route("/api/admin/acme/status", get(status))
        .route("/api/admin/acme/overview", get(overview))
        .route("/api/admin/acme/dns-providers", get(dns_providers))
        .route("/api/admin/acme/check", get(legacy_scan_acme_check))
        .route(
            "/api/admin/acme/install",
            axum::routing::post(legacy_scan_acme_install),
        )
        .route(
            "/api/admin/acme/issue",
            axum::routing::post(legacy_scan_acme_issue),
        )
        .route("/api/admin/scan/check", get(legacy_scan_acme_check))
        .route(
            "/api/admin/scan/install",
            axum::routing::post(legacy_scan_acme_install),
        )
        .route(
            "/api/admin/scan/issue",
            axum::routing::post(legacy_scan_acme_issue),
        )
        .route(
            "/api/admin/acme/subdomain-recommendation",
            get(subdomain_recommendation),
        )
        .route("/api/admin/acme/init", axum::routing::post(init_acme))
        .route(
            "/api/admin/acme/client-settings",
            axum::routing::post(save_client_settings_route),
        )
        .route("/api/admin/acme/config", get(config).post(save_config))
        .route(
            "/api/admin/acme/applications",
            get(applications).post(create_application),
        )
        .route(
            "/api/admin/acme/applications/{id}",
            get(application)
                .patch(update_application)
                .delete(delete_application),
        )
        .route(
            "/api/admin/acme/applications/{id}/certificate",
            delete(delete_application_certificate),
        )
        .route(
            "/api/admin/acme/applications/{id}/library/sync",
            axum::routing::post(sync_application_library),
        )
        .route(
            "/api/admin/acme/applications/{id}/deploy",
            axum::routing::post(deploy_application_certificate),
        )
        .route(
            "/api/admin/acme/applications/{id}/request",
            axum::routing::post(request_application_certificate),
        )
        .route(
            "/api/admin/acme/request",
            axum::routing::post(request_certificate),
        )
        .route(
            "/api/admin/acme/jobs/active/stop",
            axum::routing::post(stop_active_job),
        )
        .route("/api/admin/acme/jobs/{id}/poll", get(job_poll))
        .route("/api/admin/acme/jobs/{id}", get(job))
        .route("/api/admin/acme/jobs/{id}/logs", get(job_logs))
        .route(
            "/api/admin/acme/certs/{domain}",
            get(cert_info).delete(delete_cert),
        )
        .route(
            "/api/admin/acme/certs/{domain}/download",
            get(cert_download),
        )
        .route(
            "/api/admin/acme/certs/{domain}/deploy",
            axum::routing::post(deploy_domain_certificate),
        )
}

pub fn start_acme_tasks(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio_time::interval(acme_renew_interval());
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = run_acme_auto_renew_once(state.clone()).await {
                tracing::warn!(%error, "ACME auto-renew task failed");
            }
        }
    });
}

async fn run_acme_auto_renew_once(state: AppState) -> anyhow::Result<()> {
    let acquired = state
        .redis
        .set_lock_if_not_exists("acme-renew", acme_renew_lock_ttl_seconds())
        .await?;
    if !acquired {
        return Ok(());
    }

    let t = Translator::from_state(&state).await;
    let install_state = current_acme_install_state(&state, &t).await;
    if install_state.get("status").and_then(Value::as_str) != Some("installed") {
        return Ok(());
    }
    let active_lock = get_active_acme_runtime_lock(&state).await?;
    if active_lock.get("locked").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }

    let threshold_seconds = acme_renew_days() * 24 * 60 * 60;
    let mut renewable = Vec::new();
    for application in read_acme_applications(&state).await? {
        if application.get("renewEnabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let Some(certificate) =
            get_usable_issued_certificate_for_application(&state, &application).await?
        else {
            continue;
        };
        let Some(valid_to) = certificate
            .pointer("/certInfo/validTo")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_unix_timestamp)
        else {
            continue;
        };
        if valid_to - time_utils::now_ms() / 1000 > threshold_seconds {
            continue;
        }
        renewable.push((valid_to, application));
    }
    renewable.sort_by_key(|(valid_to, _)| *valid_to);

    for (_, application) in renewable {
        match start_acme_application_job(state.clone(), application, "auto_renew", t.clone()).await
        {
            Ok((job, _lock)) => {
                if wait_for_acme_job_completion(&state, &job).await? == Some("stopped".to_string())
                {
                    return Ok(());
                }
            }
            Err(error) => {
                if error.to_string() == t.t("server.acmeJobRunner.activeTaskRunning") {
                    return Ok(());
                }
                tracing::warn!(%error, "failed to start ACME auto-renew job");
            }
        }
    }

    if let Err(error) = reconcile_acme_ssl_deployment(&state).await {
        tracing::warn!(%error, "failed to reconcile ACME SSL deployment after auto-renew");
    }
    Ok(())
}

async fn reconcile_acme_ssl_deployment(state: &AppState) -> anyhow::Result<()> {
    let applications = read_acme_applications(state).await?;
    let t = Translator::from_state(state).await;
    let mut config = state.redis.get_config().await?;
    let mut deployment_changed = false;

    for application in applications {
        if application.get("renewEnabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }

        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if application_id.is_empty() {
            continue;
        }

        let result: anyhow::Result<bool> = async {
            let Some(issued_certificate) =
                get_usable_issued_certificate_for_application(state, &application).await?
            else {
                return Ok(false);
            };
            let linked_certificate =
                ssl::get_acme_ssl_certificate_by_source_ref(state, &application_id).await?;
            let library_matches_issued = linked_certificate.as_ref().is_some_and(|certificate| {
                same_pem(
                    certificate.get("cert").and_then(Value::as_str),
                    issued_certificate.get("cert").and_then(Value::as_str),
                ) && same_pem(
                    certificate.get("key").and_then(Value::as_str),
                    issued_certificate.get("key").and_then(Value::as_str),
                )
            });
            if library_matches_issued {
                return Ok(false);
            }

            let linked_id = linked_certificate
                .as_ref()
                .and_then(|certificate| certificate.get("id").and_then(Value::as_str))
                .map(str::to_string);
            let should_activate = linked_id.as_deref().is_some_and(|id| {
                config
                    .pointer("/ssl/active_cert_id")
                    .and_then(Value::as_str)
                    == Some(id)
            });
            let label = linked_certificate
                .as_ref()
                .and_then(|certificate| certificate.get("label").and_then(Value::as_str))
                .map(str::to_string)
                .or_else(|| {
                    application
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| {
                    application
                        .get("primaryDomain")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });

            save_acme_certificate_to_library_by_application(
                state,
                &application,
                should_activate,
                label.as_deref(),
                &t,
            )
            .await?;
            config = state.redis.get_config().await?;
            Ok(should_activate
                || config
                    .pointer("/ssl/deployment_mode")
                    .and_then(Value::as_str)
                    == Some("multi_sni"))
        }
        .await;

        match result {
            Ok(changed) => deployment_changed |= changed,
            Err(error) => {
                let domain = application
                    .get("primaryDomain")
                    .and_then(Value::as_str)
                    .unwrap_or(&application_id);
                tracing::warn!(%error, %domain, "ACME certificate library reconcile failed");
            }
        }
    }

    let certificates = config
        .pointer("/ssl/certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_cert_id = config
        .pointer("/ssl/active_cert_id")
        .and_then(Value::as_str);
    let active_certificate = certificates
        .iter()
        .find(|certificate| certificate.get("id").and_then(Value::as_str) == active_cert_id);
    let has_acme_certificate = certificates
        .iter()
        .any(|certificate| certificate.get("source").and_then(Value::as_str) == Some("acme"));
    let deployment_mode = config
        .pointer("/ssl/deployment_mode")
        .and_then(Value::as_str);
    let should_sync = deployment_changed
        || (has_acme_certificate
            && (deployment_mode == Some("multi_sni")
                || active_certificate
                    .and_then(|certificate| certificate.get("source").and_then(Value::as_str))
                    == Some("acme")));
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}

fn same_pem(left: Option<&str>, right: Option<&str>) -> bool {
    left.unwrap_or("").trim() == right.unwrap_or("").trim()
}

async fn wait_for_acme_job_completion(
    state: &AppState,
    job: &Value,
) -> anyhow::Result<Option<String>> {
    let Some(job_id) = job.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    for _ in 0..acme_renew_wait_iterations() {
        if let Some(latest) = get_acme_job(state, job_id).await?
            && let Some(status) = latest.get("status").and_then(Value::as_str)
            && matches!(status, "succeeded" | "failed" | "stopped")
        {
            return Ok(Some(status.to_string()));
        }
        tokio_time::sleep(std::time::Duration::from_secs(5)).await;
    }
    Ok(None)
}

fn acme_renew_interval() -> std::time::Duration {
    std::time::Duration::from_secs(
        env::var("ACME_RENEW_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(6 * 60 * 60)
            .clamp(60, 7 * 24 * 60 * 60),
    )
}

fn acme_renew_days() -> i64 {
    env::var("ACME_RENEW_DAYS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 90)
}

fn acme_renew_lock_ttl_seconds() -> usize {
    env::var("ACME_RENEW_LOCK_TTL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3600)
        .clamp(60, 6 * 60 * 60)
}

fn acme_renew_wait_iterations() -> usize {
    env::var("ACME_RENEW_WAIT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2 * 60 * 60)
        .clamp(60, 24 * 60 * 60)
        / 5
}

fn parse_rfc3339_unix_timestamp(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value.trim(), &Rfc3339)
        .ok()
        .map(|value| value.unix_timestamp())
}

async fn status(State(state): State<AppState>) -> Response {
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

async fn overview(State(state): State<AppState>) -> Response {
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

async fn uninstall_acme(State(state): State<AppState>) -> Response {
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

async fn init_acme(State(state): State<AppState>) -> Response {
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

async fn legacy_scan_acme_check(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    Json(current_acme_install_state(&state, &t).await).into_response()
}

async fn legacy_scan_acme_install(State(state): State<AppState>) -> Response {
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

async fn legacy_scan_acme_issue(State(state): State<AppState>, req: Request<Body>) -> Response {
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

async fn save_client_settings_route(State(state): State<AppState>, req: Request<Body>) -> Response {
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

async fn config(State(state): State<AppState>) -> Response {
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

async fn save_config(State(state): State<AppState>, req: Request<Body>) -> Response {
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

async fn create_application(State(state): State<AppState>, req: Request<Body>) -> Response {
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

async fn update_application(
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

async fn delete_application(
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

async fn delete_application_certificate(
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

async fn sync_application_library(
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

async fn deploy_application_certificate(
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

async fn request_application_certificate(
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

async fn request_certificate(State(state): State<AppState>, req: Request<Body>) -> Response {
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

async fn stop_active_job(State(state): State<AppState>) -> Response {
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

async fn dns_providers(State(state): State<AppState>) -> Response {
    let t = Translator::from_state(&state).await;
    response::ok(Value::Array(acme_dns_providers(&t))).into_response()
}

async fn subdomain_recommendation(State(state): State<AppState>) -> Response {
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

async fn build_application_overview(
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

async fn applications(State(state): State<AppState>) -> Response {
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

async fn application(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
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

async fn job(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
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

async fn job_logs(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
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

async fn job_poll(
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

async fn cert_info(State(state): State<AppState>, AxumPath(domain): AxumPath<String>) -> Response {
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

async fn delete_cert(
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

async fn cert_download(
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

async fn deploy_domain_certificate(
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

async fn ensure_acme_data_migrated(state: &AppState) -> redis::RedisResult<()> {
    let existing = read_acme_applications_raw(state).await?;
    if !existing.is_empty() {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    }

    let Some(legacy) = read_legacy_settings(state).await? else {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    };
    let domains = legacy
        .get("domains")
        .and_then(Value::as_array)
        .map(|value| normalize_domain_list(value.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    }

    let now = time_utils::now_iso();
    let primary_domain = domains[0].clone();
    let updated_at = legacy
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| now.clone());
    let application = json!({
        "id": build_application_id(Some(&primary_domain)),
        "domains": domains,
        "primaryDomain": primary_domain,
        "dnsType": legacy.get("dnsType").and_then(Value::as_str).map(str::trim).unwrap_or(""),
        "credentials": normalize_string_record(legacy.get("credentials")),
        "renewEnabled": true,
        "createdAt": updated_at,
        "updatedAt": updated_at,
        "latestJobStatus": "idle",
    });

    let mut issued_certificates = Vec::new();
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some((cert, key)) = read_acme_cert_pair(state, primary_domain).await?
        && let Some(cert_info) = ssl::parse_cert_info(&cert)
    {
        issued_certificates.push(json!({
            "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
            "primaryDomain": primary_domain,
            "cert": cert,
            "key": key,
            "certInfo": cert_info,
            "createdAt": now,
            "updatedAt": now,
        }));
    }

    state
        .redis
        .set_json_value(ACME_APPLICATIONS_KEY, &Value::Array(vec![application]))
        .await?;
    state
        .redis
        .set_json_value(
            ACME_ISSUED_CERTIFICATES_KEY,
            &Value::Array(issued_certificates),
        )
        .await?;
    state
        .redis
        .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
        .await
}

async fn read_acme_applications(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    ensure_acme_data_migrated(state).await?;
    let mut applications = read_acme_applications_raw(state).await?;
    applications.sort_by(|left, right| {
        right
            .get("updatedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(left.get("updatedAt").and_then(Value::as_str).unwrap_or(""))
    });
    Ok(applications)
}

async fn read_acme_applications_raw(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    Ok(state
        .redis
        .get_json_value(ACME_APPLICATIONS_KEY)
        .await?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_acme_application)
        .collect())
}

async fn write_acme_applications(
    state: &AppState,
    applications: &[Value],
) -> redis::RedisResult<()> {
    state
        .redis
        .set_json_value(ACME_APPLICATIONS_KEY, &Value::Array(applications.to_vec()))
        .await
}

async fn save_acme_application_with_effects(
    state: &AppState,
    t: &Translator,
    input: SaveAcmeApplicationInput,
) -> anyhow::Result<AcmeApplicationSaveOutcome> {
    ensure_acme_data_migrated(state).await?;
    let applications = read_acme_applications_raw(state).await?;
    let normalized_domains = normalize_domain_strings(input.domains);
    let primary_domain = normalized_domains.first().cloned().unwrap_or_default();
    let dns_type = input.dns_type.trim().to_string();
    if normalized_domains.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.domainsRequired"));
    }
    if dns_type.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.dnsProviderRequired"));
    }

    let existing = input.id.as_ref().and_then(|id| {
        applications
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .cloned()
    });
    let duplicated = applications.iter().any(|item| {
        item.get("primaryDomain").and_then(Value::as_str) == Some(primary_domain.as_str())
            && item.get("id").and_then(Value::as_str)
                != existing
                    .as_ref()
                    .and_then(|value| value.get("id"))
                    .and_then(Value::as_str)
    });
    if duplicated {
        anyhow::bail!(t.t_params(
            "server.redis.acme.primaryDomainDuplicated",
            &[("primaryDomain", primary_domain.clone())]
        ));
    }

    let now = time_utils::now_iso();
    let existing_id = existing
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str);
    let id = existing_id
        .map(str::to_string)
        .or(input.id.filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| build_application_id(None));
    let created_at = existing
        .as_ref()
        .and_then(|value| value.get("createdAt"))
        .and_then(Value::as_str)
        .unwrap_or(&now)
        .to_string();
    let mut application = Map::new();
    application.insert("id".to_string(), json!(id));
    if input.name_provided {
        if let Some(name) = input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            application.insert("name".to_string(), json!(name));
        }
    } else if let Some(name) = existing
        .as_ref()
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        application.insert("name".to_string(), json!(name));
    }
    application.insert("domains".to_string(), json!(normalized_domains));
    application.insert("primaryDomain".to_string(), json!(primary_domain));
    application.insert("dnsType".to_string(), json!(dns_type));
    application.insert("credentials".to_string(), input.credentials);
    application.insert(
        "renewEnabled".to_string(),
        json!(
            input
                .renew_enabled
                .or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|value| value.get("renewEnabled"))
                        .and_then(Value::as_bool)
                })
                .unwrap_or(true)
        ),
    );
    application.insert("createdAt".to_string(), json!(created_at));
    application.insert("updatedAt".to_string(), json!(now));
    if let Some(existing) = existing.as_ref() {
        insert_optional_string(&mut application, "latestJobId", existing.get("latestJobId"));
        insert_optional_value(
            &mut application,
            "latestJobStatus",
            normalize_latest_job_status(existing.get("latestJobStatus")),
        );
        insert_optional_value(
            &mut application,
            "latestJobTrigger",
            normalize_job_trigger(existing.get("latestJobTrigger")),
        );
        insert_optional_string(&mut application, "latestJobAt", existing.get("latestJobAt"));
        insert_optional_string(&mut application, "lastError", existing.get("lastError"));
    } else {
        application.insert("latestJobStatus".to_string(), json!("idle"));
    }
    let application = Value::Object(application);
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let next_applications =
        std::iter::once(application.clone())
            .chain(applications.into_iter().filter(|item| {
                item.get("id").and_then(Value::as_str) != Some(application_id.as_str())
            }))
            .collect::<Vec<_>>();
    write_acme_applications(state, &next_applications).await?;

    let domain_changed = existing.as_ref().is_some_and(|previous| {
        previous.get("primaryDomain").and_then(Value::as_str)
            != application.get("primaryDomain").and_then(Value::as_str)
            || normalized_domain_signature(
                &previous
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            ) != normalized_domain_signature(
                &application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            )
    });
    if !domain_changed {
        return Ok(AcmeApplicationSaveOutcome {
            application,
            removed_library_certificate_count: 0,
            removed_active_library_certificate: false,
        });
    }

    let deleted_issued_certificate = delete_acme_issued_certificate(state, &application_id).await?;
    let previous_primary_domain = existing
        .as_ref()
        .and_then(|value| value.get("primaryDomain"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) = cleanup_acme_application_artifacts(
        state,
        &application_id,
        previous_primary_domain,
        deleted_issued_certificate.as_ref(),
    )
    .await?;
    Ok(AcmeApplicationSaveOutcome {
        application,
        removed_library_certificate_count: removed_count,
        removed_active_library_certificate: removed_active,
    })
}

async fn resolve_legacy_application_for_mutation(
    state: &AppState,
    domains: &[String],
    t: &Translator,
) -> anyhow::Result<Option<Value>> {
    let applications = read_acme_applications(state).await?;
    let primary_domain = domains.first().map(String::as_str).unwrap_or("");
    if let Some(application) = applications.iter().find(|application| {
        application.get("primaryDomain").and_then(Value::as_str) == Some(primary_domain)
    }) {
        return Ok(Some(application.clone()));
    }
    if applications.len() == 1 {
        return Ok(applications.first().cloned());
    }
    if applications.len() > 1 {
        anyhow::bail!(t.t("server.redis.acme.multipleApplicationsUseNewApi"));
    }
    Ok(None)
}

async fn delete_acme_application_internal(state: &AppState, id: &str) -> anyhow::Result<bool> {
    ensure_acme_data_migrated(state).await?;
    let applications = read_acme_applications_raw(state).await?;
    let Some(existing) = applications
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
    else {
        return Ok(false);
    };
    let next_applications = applications
        .into_iter()
        .filter(|item| item.get("id").and_then(Value::as_str) != Some(id))
        .collect::<Vec<_>>();
    write_acme_applications(state, &next_applications).await?;
    if next_applications.is_empty() {
        state.redis.delete_key(ACME_LEGACY_SETTINGS_KEY).await?;
    }
    let deleted_issued_certificate = delete_acme_issued_certificate(state, id).await?;
    let primary_domain = existing
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) = cleanup_acme_application_artifacts(
        state,
        id,
        primary_domain,
        deleted_issued_certificate.as_ref(),
    )
    .await?;
    sync_gateway_if_acme_library_removed(state, removed_active, removed_count).await?;
    Ok(true)
}

async fn delete_acme_application_certificate_internal(
    state: &AppState,
    id: &str,
) -> anyhow::Result<bool> {
    let Some(application) = find_acme_application(state, id).await? else {
        return Ok(false);
    };
    let issued_certificate = delete_acme_issued_certificate(state, id).await?;
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) =
        cleanup_acme_application_artifacts(state, id, primary_domain, issued_certificate.as_ref())
            .await?;
    sync_gateway_if_acme_library_removed(state, removed_active, removed_count).await?;
    Ok(true)
}

async fn cleanup_acme_application_artifacts(
    state: &AppState,
    application_id: &str,
    primary_domain: &str,
    deleted_issued_certificate: Option<&Value>,
) -> anyhow::Result<(usize, bool)> {
    let (removed_by_ref, active_by_ref) =
        ssl::delete_acme_ssl_certificates(state, Some(application_id), None).await?;
    let (removed_by_domain, active_by_domain) =
        ssl::delete_acme_ssl_certificates(state, None, Some(primary_domain)).await?;
    let removed_domains = uniq_strings(
        [primary_domain].into_iter().chain(
            deleted_issued_certificate
                .and_then(|value| value.get("primaryDomain"))
                .and_then(Value::as_str),
        ),
    );
    remove_acme_domain_artifacts(state, &removed_domains).await?;
    Ok((
        removed_by_ref + removed_by_domain,
        active_by_ref || active_by_domain,
    ))
}

async fn delete_acme_issued_certificate(
    state: &AppState,
    application_id: &str,
) -> redis::RedisResult<Option<Value>> {
    let issued_certificates = read_issued_certificates(state).await?;
    let mut deleted = None;
    let next = issued_certificates
        .into_iter()
        .filter(|item| {
            let should_delete =
                item.get("applicationId").and_then(Value::as_str) == Some(application_id);
            if should_delete {
                deleted = Some(item.clone());
            }
            !should_delete
        })
        .collect::<Vec<_>>();
    state
        .redis
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(next))
        .await?;
    Ok(deleted)
}

async fn delete_acme_cert_pair(state: &AppState, domain: &str) -> redis::RedisResult<()> {
    state
        .redis
        .delete_key(&format!("{ACME_CERT_PREFIX}{domain}"))
        .await
}

async fn remove_acme_domain_artifacts(state: &AppState, domains: &[String]) -> anyhow::Result<()> {
    for domain in domains {
        delete_acme_cert_pair(state, domain).await?;
        let dir = state.settings.data_dir.join("ssl").join(domain);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn start_acme_application_job(
    state: AppState,
    application: Value,
    trigger: &str,
    t: Translator,
) -> anyhow::Result<(Value, Value)> {
    let (job, lock) = reserve_acme_application_job(&state, &application, trigger, &t).await?;
    match run_reserved_acme_application_job(
        state.clone(),
        application.clone(),
        trigger,
        job.clone(),
        lock.clone(),
        t.clone(),
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(error) => {
            let message = error.to_string();
            fail_reserved_acme_application_job(&state, &application, &job, &lock, &message, &t)
                .await
                .ok();
            Err(error)
        }
    }
}

async fn ensure_acme_installed_for_request(state: &AppState, t: &Translator) -> anyhow::Result<()> {
    if acme_executable_path(state).is_file() {
        return Ok(());
    }
    if acme_install_is_installing(state).await {
        anyhow::bail!(acme_route_text(t, "installingRetryLater"));
    }
    anyhow::bail!(acme_route_text(t, "installFirst"));
}

async fn reserve_acme_application_job(
    state: &AppState,
    application: &Value,
    trigger: &str,
    t: &Translator,
) -> anyhow::Result<(Value, Value)> {
    ensure_acme_installed_for_request(state, t).await?;
    let active_lock = get_active_acme_runtime_lock(&state).await?;
    if active_lock.get("locked").and_then(Value::as_bool) == Some(true) {
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }

    let job = build_queued_acme_job(&application, trigger, &t)?;
    let lock = build_acme_runtime_lock(&application, &job, trigger);
    let leased_lock = with_runtime_lock_lease(lock);
    let acquired = state
        .redis
        .set_json_value_nx_ex(
            ACME_RUNTIME_LOCK_KEY,
            &leased_lock,
            acme_runtime_lock_ttl_seconds(),
        )
        .await?;
    if !acquired {
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }

    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Err(error) = async {
        create_acme_job(&state, &job, &t).await?;
        clear_acme_logs(&state, &job_id).await?;
        update_acme_application_job_state(&state, &application, &job).await
    }
    .await
    {
        release_acme_runtime_lock(&state, &leased_lock).await.ok();
        return Err(error);
    }

    Ok((job, leased_lock))
}

async fn run_reserved_acme_application_job(
    state: AppState,
    application: Value,
    trigger: &str,
    job: Value,
    lock: Value,
    t: Translator,
) -> anyhow::Result<(Value, Value)> {
    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if job_id.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.jobDataInvalid"));
    }
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let provider = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .or_else(|| {
            application
                .get("dnsType")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    update_acme_job(
        &state,
        &job_id,
        json!({
            "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
            "domains": domains,
            "provider": provider,
            "trigger": normalize_trigger_string(trigger),
        }),
    )
    .await?;

    let run_state = state.clone();
    let run_application = application.clone();
    let run_lock = lock.clone();
    let run_t = t.clone();
    let run_job_id = job_id.clone();
    tokio::spawn(async move {
        if let Err(error) =
            execute_acme_application_job(run_state, run_application, run_job_id, run_lock, run_t)
                .await
        {
            tracing::warn!(%error, "ACME job runner failed");
        }
    });

    Ok((job, lock))
}

async fn fail_reserved_acme_application_job(
    state: &AppState,
    application: &Value,
    job: &Value,
    lock: &Value,
    message: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let job_id = job.get("id").and_then(Value::as_str).unwrap_or("");
    if !job_id.is_empty() {
        append_acme_log(
            state,
            job_id,
            &t.t_params(
                "server.acmeJobRunner.flowFailed",
                &[("message", message.to_string())],
            ),
        )
        .await
        .ok();
        let finished_at = time_utils::now_iso();
        if let Some(updated) = update_acme_job(
            state,
            job_id,
            json!({
                "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
                "status": "failed",
                "progress": 100,
                "finishedAt": finished_at,
                "message": message,
            }),
        )
        .await?
        {
            update_acme_application_job_state(state, application, &updated).await?;
        }
    }
    release_acme_runtime_lock(state, lock).await.ok();
    Ok(())
}

fn build_queued_acme_job(
    application: &Value,
    trigger: &str,
    t: &Translator,
) -> anyhow::Result<Value> {
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        anyhow::bail!(t.t("server.acmeRoutes.domainsInvalid"));
    }
    let dns_type = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .or_else(|| {
            application
                .get("dnsType")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    Ok(json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
        "domains": domains,
        "method": "dns",
        "provider": dns_type,
        "trigger": normalize_trigger_string(trigger),
        "createdAt": time_utils::now_iso(),
        "status": "queued",
        "progress": 0,
        "message": if trigger == "auto_renew" { "queued for renew" } else { "queued" },
    }))
}

fn build_acme_runtime_lock(application: &Value, job: &Value, trigger: &str) -> Value {
    json!({
        "locked": true,
        "lockId": uuid::Uuid::new_v4().to_string(),
        "jobId": job.get("id").and_then(Value::as_str).unwrap_or(""),
        "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
        "reason": normalize_trigger_string(trigger),
        "startedAt": job.get("createdAt").and_then(Value::as_str).unwrap_or(""),
    })
}

fn with_runtime_lock_lease(mut lock: Value) -> Value {
    let ttl = acme_runtime_lock_ttl_seconds() as i64;
    lock["heartbeatAt"] = json!(time_utils::now_iso());
    lock["expiresAt"] = json!(time_utils::iso_after_seconds(ttl));
    lock
}

fn normalize_trigger_string(value: &str) -> &'static str {
    match value {
        "auto_renew" => "auto_renew",
        _ => "manual_request",
    }
}

fn acme_runtime_lock_ttl_seconds() -> usize {
    std::env::var("ACME_RUNTIME_LOCK_TTL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(900)
        .clamp(
            ACME_RUNTIME_LOCK_MIN_TTL_SECONDS,
            ACME_RUNTIME_LOCK_MAX_TTL_SECONDS,
        )
}

async fn create_acme_job(state: &AppState, job: &Value, t: &Translator) -> anyhow::Result<()> {
    let job = normalize_acme_job(job.clone())
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let id = job.get("id").and_then(Value::as_str).unwrap_or("");
    state
        .redis
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(())
}

async fn update_acme_job(
    state: &AppState,
    id: &str,
    patch: Value,
) -> anyhow::Result<Option<Value>> {
    let Some(mut job) = get_acme_job(state, id).await? else {
        return Ok(None);
    };
    if let (Some(job_obj), Some(patch_obj)) = (job.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            job_obj.insert(key.clone(), value.clone());
        }
    }
    let Some(job) = normalize_acme_job(job) else {
        return Ok(None);
    };
    state
        .redis
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(Some(job))
}

async fn append_acme_log(state: &AppState, job_id: &str, line: &str) -> redis::RedisResult<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    state
        .redis
        .append_log_buffer(
            &format!("{ACME_LOGS_PREFIX}{job_id}"),
            &[line.to_string()],
            ACME_JOB_TTL_SECONDS,
            MAX_ACME_LOG_LIMIT,
        )
        .await
}

async fn clear_acme_logs(state: &AppState, job_id: &str) -> redis::RedisResult<()> {
    state
        .redis
        .clear_log_buffer(&format!("{ACME_LOGS_PREFIX}{job_id}"))
        .await
}

async fn update_acme_application_job_state(
    state: &AppState,
    application: &Value,
    job: &Value,
) -> anyhow::Result<()> {
    let Some(application_id) = application.get("id").and_then(Value::as_str) else {
        return Ok(());
    };
    let mut applications = read_acme_applications_raw(state).await?;
    let Some(index) = applications
        .iter()
        .position(|item| item.get("id").and_then(Value::as_str) == Some(application_id))
    else {
        return Ok(());
    };
    if let Some(object) = applications[index].as_object_mut() {
        object.insert(
            "latestJobId".to_string(),
            job.get("id").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "latestJobStatus".to_string(),
            job.get("status").cloned().unwrap_or_else(|| json!("idle")),
        );
        object.insert(
            "latestJobTrigger".to_string(),
            job.get("trigger")
                .cloned()
                .unwrap_or_else(|| json!("manual_request")),
        );
        object.insert(
            "latestJobAt".to_string(),
            job.get("finishedAt")
                .or_else(|| job.get("startedAt"))
                .or_else(|| job.get("createdAt"))
                .cloned()
                .unwrap_or_else(|| json!(time_utils::now_iso())),
        );
        if job.get("status").and_then(Value::as_str) == Some("failed") {
            if let Some(message) = job.get("message").and_then(Value::as_str) {
                object.insert("lastError".to_string(), json!(message));
            }
        } else {
            object.remove("lastError");
        }
    }
    write_acme_applications(state, &applications).await?;
    Ok(())
}

async fn release_acme_runtime_lock(state: &AppState, lock: &Value) -> redis::RedisResult<bool> {
    let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) else {
        return Ok(false);
    };
    state
        .redis
        .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
        .await
}

async fn execute_acme_application_job(
    state: AppState,
    application: Value,
    job_id: String,
    lock: Value,
    t: Translator,
) -> anyhow::Result<()> {
    let heartbeat_stop = Arc::new(AtomicBool::new(false));
    let heartbeat_task =
        start_acme_lock_heartbeat(state.clone(), lock.clone(), heartbeat_stop.clone());
    let started_at = time_utils::now_iso();
    let running_message = t.t("server.acmeJobRunner.lockMessages.manualRequest");
    if let Some(job) = update_acme_job(
        &state,
        &job_id,
        json!({
            "status": "running",
            "progress": 5,
            "startedAt": started_at,
            "message": running_message,
        }),
    )
    .await?
    {
        update_acme_application_job_state(&state, &application, &job).await?;
    }

    let result = async {
        let client_settings = ensure_client_settings(&state).await?;
        let certificate_authority = client_settings
            .get("certificateAuthority")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
            .to_string();
        issue_acme_certificate(&state, &application, &job_id, &certificate_authority, &t).await?;
        if let Some(job) = update_acme_job(
            &state,
            &job_id,
            json!({
                "progress": 80,
                "message": "saving",
            }),
        )
        .await?
        {
            update_acme_application_job_state(&state, &application, &job).await?;
        }
        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
        let latest_application = find_acme_application(&state, application_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButApplicationChanged"))
            })?;
        if application.get("primaryDomain").and_then(Value::as_str)
            != latest_application
                .get("primaryDomain")
                .and_then(Value::as_str)
            || normalized_domain_signature(
                &application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            ) != normalized_domain_signature(
                &latest_application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            )
        {
            append_acme_log(
                &state,
                &job_id,
                &t.t("server.acmeJobRunner.applicationChangedSkipped"),
            )
            .await
            .ok();
            anyhow::bail!(t.t("server.acmeJobRunner.issuedButApplicationChanged"));
        }
        save_acme_issued_cert_from_fs(&state, &latest_application, &job_id, &t).await?;
        if let Some(primary_domain) = latest_application
            .get("primaryDomain")
            .and_then(Value::as_str)
        {
            match clear_acme_domain_working_state(&state, primary_domain).await {
                Ok(()) => {
                    append_acme_log(
                        &state,
                        &job_id,
                        &t.t("server.acmeJobRunner.clearedDomainWorkingState"),
                    )
                    .await
                    .ok();
                }
                Err(error) => {
                    append_acme_log(
                        &state,
                        &job_id,
                        &t.t_params(
                            "server.acmeJobRunner.clearDomainWorkingStateFailed",
                            &[("message", error.to_string())],
                        ),
                    )
                    .await
                    .ok();
                }
            }
        }
        sync_acme_library_after_issue(&state, &latest_application, &job_id, &t).await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    match result {
        Ok(()) => {
            if let Some(job) = update_acme_job(
                &state,
                &job_id,
                json!({
                    "status": "succeeded",
                    "progress": 100,
                    "finishedAt": time_utils::now_iso(),
                    "message": "succeeded",
                }),
            )
            .await?
            {
                update_acme_application_job_state(&state, &application, &job).await?;
            }
        }
        Err(error) => {
            let message = error.to_string();
            append_acme_log(
                &state,
                &job_id,
                &t.t_params(
                    "server.acmeJobRunner.flowFailed",
                    &[("message", message.clone())],
                ),
            )
            .await
            .ok();
            if let Some(job) = update_acme_job(
                &state,
                &job_id,
                json!({
                    "status": "failed",
                    "progress": 100,
                    "finishedAt": time_utils::now_iso(),
                    "message": message,
                }),
            )
            .await?
            {
                update_acme_application_job_state(&state, &application, &job).await?;
            }
        }
    }

    heartbeat_stop.store(true, Ordering::Relaxed);
    heartbeat_task.await.ok();
    release_acme_runtime_lock(&state, &lock).await.ok();
    Ok(())
}

fn start_acme_lock_heartbeat(
    state: AppState,
    lock: Value,
    stop: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_seconds = (acme_runtime_lock_ttl_seconds() / 3).clamp(30, 60);
        let Some(lock_id) = lock
            .get("lockId")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return;
        };
        while !stop.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_secs(interval_seconds as u64)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let next = with_runtime_lock_lease(lock.clone());
            match state
                .redis
                .set_json_lock_if_owned_ex(
                    ACME_RUNTIME_LOCK_KEY,
                    &lock_id,
                    &next,
                    acme_runtime_lock_ttl_seconds(),
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(%error, "failed to refresh ACME runtime lock");
                }
            }
        }
    })
}

async fn issue_acme_certificate(
    state: &AppState,
    application: &Value,
    job_id: &str,
    certificate_authority: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let executable = acme_executable_path(state);
    if !executable.is_file() {
        anyhow::bail!(t.t("server.acmeService.installFirst"));
    }
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.domainsRequired"));
    }
    let dns_type = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeRoutes.dnsTypeRequired")))?;
    let acme_home = acme_home_dir(state);
    apply_acme_dns_provider_patches(state, &dns_type, job_id, t).await?;
    register_acme_account(state, None, Some(certificate_authority), t).await?;
    let mut args = vec![
        "--issue".to_string(),
        "--home".to_string(),
        acme_home.to_string_lossy().to_string(),
        "--config-home".to_string(),
        acme_home.to_string_lossy().to_string(),
        "--server".to_string(),
        certificate_authority.to_string(),
        "--force".to_string(),
        "--dns".to_string(),
        dns_type.clone(),
        "--debug".to_string(),
    ];
    for domain in domains {
        args.push("-d".to_string());
        args.push(domain);
    }
    append_acme_log(
        state,
        job_id,
        &format!("$ {} {}", executable.display(), args.join(" ")),
    )
    .await
    .ok();

    let mut command = Command::new(executable);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let env_vars = normalize_acme_env_vars(&dns_type, application.get("credentials"));
    for (key, value) in env_vars {
        if let Some(value) = value.as_str() {
            command.env(key, value);
        }
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_state = state.clone();
    let out_job = job_id.to_string();
    let err_state = state.clone();
    let err_job = job_id.to_string();
    let stdout_task = tokio::spawn(async move {
        if let Some(stream) = stdout {
            append_acme_stream_lines(out_state, out_job, stream).await;
        }
    });
    let stderr_task = tokio::spawn(async move {
        if let Some(stream) = stderr {
            append_acme_stream_lines(err_state, err_job, stream).await;
        }
    });
    let status = child.wait().await?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    if status.success() {
        return Ok(());
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.issueFailed",
        &[
            ("code", status.code().unwrap_or(-1).to_string()),
            ("brief", String::new())
        ],
    ))
}

async fn append_acme_stream_lines<R>(state: AppState, job_id: String, stream: R)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        append_acme_log(&state, &job_id, &line).await.ok();
    }
}

fn acme_home_dir(state: &AppState) -> PathBuf {
    state.settings.data_dir.join(".acme.sh")
}

async fn save_acme_issued_cert_from_fs(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<Value> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    install_acme_cert_to_data_dir(state, primary_domain, job_id).await?;
    let (cert, key) = read_acme_cert_pair_from_fs(state, primary_domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButCertReadFailed")))?;
    let cert_info = ssl::parse_cert_info(&cert)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButCertReadFailed")))?;
    let issued = save_acme_issued_certificate(
        state,
        application_id,
        primary_domain,
        &cert,
        &key,
        cert_info,
    )
    .await?;
    Ok(issued)
}

async fn install_acme_cert_to_data_dir(
    state: &AppState,
    primary_domain: &str,
    job_id: &str,
) -> anyhow::Result<()> {
    let normalized = normalize_domain_name(primary_domain);
    let domain_dir = state.settings.data_dir.join("ssl").join(&normalized);
    tokio::fs::create_dir_all(&domain_dir).await?;
    let installed_key_path = domain_dir.join(format!("{normalized}.key"));
    let installed_fullchain_path = domain_dir.join("fullchain.cer");
    let executable = acme_executable_path(state);
    if !executable.is_file() {
        return Ok(());
    }
    let candidates = [
        (acme_home_dir(state).join(format!("{normalized}_ecc")), true),
        (acme_home_dir(state).join(&normalized), false),
        (
            legacy_acme_home_dir().join(format!("{normalized}_ecc")),
            true,
        ),
        (legacy_acme_home_dir().join(&normalized), false),
    ];
    let mut variants = candidates
        .iter()
        .filter(|(path, _)| path.exists())
        .map(|(_, use_ecc)| *use_ecc)
        .collect::<Vec<_>>();
    if variants.is_empty() {
        variants = vec![true, false];
    }
    variants.sort();
    variants.dedup();

    let mut last_error = None;
    for use_ecc in variants {
        let mut args = vec![
            "--home".to_string(),
            acme_home_dir(state).to_string_lossy().to_string(),
            "--config-home".to_string(),
            acme_home_dir(state).to_string_lossy().to_string(),
            "--install-cert".to_string(),
            "-d".to_string(),
            normalized.clone(),
            "--key-file".to_string(),
            installed_key_path.to_string_lossy().to_string(),
            "--fullchain-file".to_string(),
            installed_fullchain_path.to_string_lossy().to_string(),
        ];
        if use_ecc {
            args.push("--ecc".to_string());
        }
        let result = run_acme_command(state, args, None).await?;
        if result.exit_code == 0 {
            return Ok(());
        }
        let message = format!(
            "[acme][install-cert] {} install failed (exit {}): {}",
            if use_ecc { "ECC" } else { "RSA" },
            result.exit_code,
            command_output_brief(&result.stdout, &result.stderr).trim_start_matches(": ")
        );
        append_acme_log(state, job_id, &message).await.ok();
        last_error = Some(message);
    }
    if read_acme_cert_pair_from_fs(state, &normalized)
        .await?
        .is_some()
    {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        last_error.unwrap_or_else(|| "failed to install ACME certificate files".to_string())
    )
}

async fn read_acme_cert_pair_from_fs(
    state: &AppState,
    domain: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let normalized = normalize_domain_name(domain);
    let candidates = [
        state.settings.data_dir.join("ssl").join(&normalized),
        acme_home_dir(state).join(format!("{normalized}_ecc")),
        acme_home_dir(state).join(&normalized),
    ];
    for dir in candidates {
        let key_path = dir.join(format!("{normalized}.key"));
        let cert_paths = [
            dir.join("fullchain.cer"),
            dir.join(format!("{normalized}.cer")),
        ];
        let Ok(key) = tokio::fs::read_to_string(&key_path).await else {
            continue;
        };
        for cert_path in cert_paths {
            if let Ok(cert) = tokio::fs::read_to_string(&cert_path).await
                && !cert.trim().is_empty()
                && !key.trim().is_empty()
            {
                return Ok(Some((cert, key)));
            }
        }
    }
    Ok(None)
}

async fn save_acme_issued_certificate(
    state: &AppState,
    application_id: &str,
    primary_domain: &str,
    cert: &str,
    key: &str,
    cert_info: Value,
) -> anyhow::Result<Value> {
    let mut issued = read_issued_certificates(state).await?;
    let existing = issued
        .iter()
        .find(|item| item.get("applicationId").and_then(Value::as_str) == Some(application_id))
        .cloned();
    let now = time_utils::now_iso();
    let mut next = Map::new();
    next.insert("applicationId".to_string(), json!(application_id));
    next.insert(
        "primaryDomain".to_string(),
        json!(normalize_domain_name(primary_domain)),
    );
    next.insert("cert".to_string(), json!(cert.trim()));
    next.insert("key".to_string(), json!(key.trim()));
    next.insert("certInfo".to_string(), cert_info);
    next.insert(
        "createdAt".to_string(),
        existing
            .as_ref()
            .and_then(|value| value.get("createdAt"))
            .cloned()
            .unwrap_or_else(|| json!(now.clone())),
    );
    next.insert("updatedAt".to_string(), json!(now));
    if let Some(value) = existing
        .as_ref()
        .and_then(|value| value.get("libraryCertificateId"))
        .and_then(Value::as_str)
    {
        next.insert("libraryCertificateId".to_string(), json!(value));
    }
    if let Some(value) = existing
        .as_ref()
        .and_then(|value| value.get("libraryLinkedAt"))
        .and_then(Value::as_str)
    {
        next.insert("libraryLinkedAt".to_string(), json!(value));
    }
    let next = Value::Object(next);
    issued.retain(|item| item.get("applicationId").and_then(Value::as_str) != Some(application_id));
    issued.insert(0, next.clone());
    state
        .redis
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(issued))
        .await?;
    state
        .redis
        .set_json_value(
            &format!(
                "{ACME_CERT_PREFIX}{}",
                normalize_domain_name(primary_domain)
            ),
            &json!({ "cert": cert.trim(), "key": key.trim() }),
        )
        .await?;
    Ok(next)
}

async fn sync_acme_library_after_issue(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let linked = ssl::get_acme_ssl_certificate_by_source_ref(state, application_id).await?;
    if let Some(linked_certificate) = linked {
        let linked_id = linked_certificate
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let label = linked_certificate
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string);
        let active_id = ssl::active_ssl_certificate_id(state).await?;
        let should_activate = active_id.as_deref() == Some(linked_id.as_str());
        save_acme_certificate_to_library_by_application(
            state,
            application,
            should_activate,
            label.as_deref(),
            t,
        )
        .await?;
        let config = state.redis.get_config().await?;
        let should_sync = should_activate
            || config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni");
        if should_sync {
            ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
        }
        let message = if should_sync {
            t.t("server.acmeJobRunner.linkedLibrarySyncedGateway")
        } else {
            t.t("server.acmeJobRunner.linkedLibraryUpdated")
        };
        append_acme_log(state, job_id, &message).await.ok();
        return Ok(());
    }

    let label = application
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| application.get("primaryDomain").and_then(Value::as_str));
    match save_acme_certificate_to_library_by_application(state, application, false, label, t).await
    {
        Ok(_) => {
            let config = state.redis.get_config().await?;
            if config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni")
            {
                ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
                append_acme_log(
                    state,
                    job_id,
                    &t.t("server.acmeJobRunner.addedToLibraryAndSyncedGateway"),
                )
                .await
                .ok();
            } else {
                append_acme_log(state, job_id, &t.t("server.acmeJobRunner.addedToLibrary"))
                    .await
                    .ok();
            }
            Ok(())
        }
        Err(error) => {
            let message = t.t_params(
                "server.acmeJobRunner.addToLibraryFailed",
                &[("message", error.to_string())],
            );
            append_acme_log(state, job_id, &message).await.ok();
            anyhow::bail!(message)
        }
    }
}

async fn clear_acme_domain_working_state(
    state: &AppState,
    primary_domain: &str,
) -> anyhow::Result<()> {
    let normalized = normalize_domain_name(primary_domain);
    if normalized.is_empty() {
        return Ok(());
    }
    for dir in [
        acme_home_dir(state).join(&normalized),
        acme_home_dir(state).join(format!("{normalized}_ecc")),
    ] {
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn stop_active_acme_job(state: &AppState, t: &Translator) -> anyhow::Result<Value> {
    let lock = get_active_acme_runtime_lock(state).await?;
    let message = t.t("server.acmeJobRunner.manualStop");
    let mut job = Value::Null;
    if lock.get("locked").and_then(Value::as_bool) == Some(true)
        && let Some(job_id) = lock.get("jobId").and_then(Value::as_str)
        && let Some(updated) = update_acme_job(
            state,
            job_id,
            json!({
                "status": "stopped",
                "progress": 100,
                "finishedAt": time_utils::now_iso(),
                "message": message,
            }),
        )
        .await?
    {
        append_acme_log(state, job_id, &message).await.ok();
        if let Some(application_id) = updated.get("applicationId").and_then(Value::as_str)
            && let Some(application) = find_acme_application(state, application_id).await?
        {
            update_acme_application_job_state(state, &application, &updated).await?;
        }
        job = updated;
    }
    let process_result = stop_all_acme_processes(t).await;
    if let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) {
        state
            .redis
            .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
            .await
            .ok();
    }
    Ok(json!({
        "stopped": !job.is_null(),
        "job": job,
        "lock": lock,
        "processResult": process_result,
    }))
}

async fn stop_all_acme_processes(t: &Translator) -> Value {
    let matched_pids = find_acme_process_ids().await.unwrap_or_default();
    let mut errors = Vec::new();
    for pid in &matched_pids {
        #[cfg(unix)]
        unsafe {
            if libc::kill(*pid, libc::SIGTERM) != 0 {
                errors.push(t.t_params(
                    "server.acmeService.sendSignalFailed",
                    &[
                        ("signal", "SIGTERM".to_string()),
                        ("target", pid.to_string()),
                        ("detail", std::io::Error::last_os_error().to_string()),
                    ],
                ));
            }
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let remaining_pids = find_acme_process_ids().await.unwrap_or_default();
    json!({
        "matchedPids": matched_pids,
        "remainingPids": remaining_pids,
        "errors": errors,
    })
}

async fn find_acme_process_ids() -> anyhow::Result<Vec<i32>> {
    let output = Command::new("ps")
        .args(["-eo", "pid=,command="])
        .output()
        .await?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let current_pid = std::process::id() as i32;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ids = BTreeSet::new();
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        let Some((pid_part, command)) = trimmed.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid_part.trim().parse::<i32>() else {
            continue;
        };
        if pid <= 0 || pid == current_pid || !command.contains("acme.sh") {
            continue;
        }
        ids.insert(pid);
    }
    Ok(ids.into_iter().collect())
}

async fn read_issued_certificates(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    ensure_acme_data_migrated(state).await?;
    Ok(state
        .redis
        .get_json_value(ACME_ISSUED_CERTIFICATES_KEY)
        .await?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_issued_certificate)
        .collect())
}

async fn find_acme_application(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    Ok(read_acme_applications(state)
        .await?
        .into_iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id)))
}

async fn find_application_by_primary_domain(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<Value>> {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Ok(None);
    }
    Ok(read_acme_applications(state)
        .await?
        .into_iter()
        .find(|item| {
            item.get("primaryDomain")
                .and_then(Value::as_str)
                .is_some_and(|value| value == domain)
        }))
}

async fn get_acme_settings(state: &AppState) -> redis::RedisResult<Value> {
    let applications = read_acme_applications(state).await?;
    if let Some(application) = applications.first() {
        return Ok(json!({
            "domains": application.get("domains").cloned().unwrap_or_else(|| json!([])),
            "dnsType": application.get("dnsType").cloned().unwrap_or_else(|| json!("")),
            "credentials": application.get("credentials").cloned().unwrap_or_else(|| json!({})),
            "updatedAt": application.get("updatedAt").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(read_legacy_settings(state).await?.unwrap_or(Value::Null))
}

async fn read_legacy_settings(state: &AppState) -> redis::RedisResult<Option<Value>> {
    let Some(value) = state.redis.get_json_value(ACME_LEGACY_SETTINGS_KEY).await? else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let domains = object
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let dns_type = object
        .get("dnsType")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    if domains.is_empty() || dns_type.is_empty() {
        return Ok(None);
    }
    Ok(Some(json!({
        "domains": domains,
        "dnsType": dns_type,
        "credentials": normalize_string_record(object.get("credentials")),
        "updatedAt": object
            .get("updatedAt")
            .and_then(Value::as_str)
            .and_then(normalize_timestamp)
            .unwrap_or_else(time_utils::now_iso),
    })))
}

async fn ensure_client_settings(state: &AppState) -> redis::RedisResult<Value> {
    if let Some(settings) = state
        .redis
        .get_json_value(ACME_CLIENT_SETTINGS_KEY)
        .await?
        .and_then(normalize_client_settings)
    {
        return Ok(settings);
    }
    let settings = json!({
        "certificateAuthority": default_certificate_authority(state),
        "updatedAt": time_utils::now_iso(),
    });
    state
        .redis
        .set_json_value(ACME_CLIENT_SETTINGS_KEY, &settings)
        .await?;
    Ok(settings)
}

async fn status_certificate(state: &AppState) -> redis::RedisResult<Value> {
    let applications = read_acme_applications(state).await?;
    let issued = read_issued_certificates(state).await?;
    for application in applications {
        let Some(application_id) = application.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(certificate) = issued.iter().find(|item| {
            item.get("applicationId").and_then(Value::as_str) == Some(application_id)
                && issued_certificate_compatible(&application, item)
        }) else {
            continue;
        };
        return Ok(json!({
            "primaryDomain": certificate.get("primaryDomain").cloned().unwrap_or(Value::Null),
            "info": certificate.get("certInfo").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(Value::Null)
}

async fn get_certificate_for_domain(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<(String, String, String, Value)>> {
    if let Some(application) = find_application_by_primary_domain(state, domain).await? {
        let Some(application_id) = application.get("id").and_then(Value::as_str) else {
            return Ok(None);
        };
        if let Some(certificate) = read_issued_certificates(state)
            .await?
            .into_iter()
            .find(|item| {
                item.get("applicationId").and_then(Value::as_str) == Some(application_id)
                    && issued_certificate_compatible(&application, item)
            })
        {
            return Ok(Some((
                certificate
                    .get("primaryDomain")
                    .and_then(Value::as_str)
                    .unwrap_or(domain)
                    .to_string(),
                certificate
                    .get("cert")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                certificate
                    .get("key")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                certificate.get("certInfo").cloned().unwrap_or(Value::Null),
            )));
        }
        return Ok(None);
    }

    let domain = domain.trim().to_ascii_lowercase();
    let Some((cert, key)) = read_acme_cert_pair(state, &domain).await? else {
        return Ok(None);
    };
    let Some(info) = ssl::parse_cert_info(&cert) else {
        return Ok(None);
    };
    Ok(Some((domain, cert, key, info)))
}

async fn get_usable_issued_certificate_for_application(
    state: &AppState,
    application: &Value,
) -> redis::RedisResult<Option<Value>> {
    let Some(application_id) = application.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    Ok(read_issued_certificates(state)
        .await?
        .into_iter()
        .find(|certificate| {
            certificate.get("applicationId").and_then(Value::as_str) == Some(application_id)
                && issued_certificate_compatible(application, certificate)
        }))
}

async fn save_acme_certificate_to_library_by_application(
    state: &AppState,
    application: &Value,
    activate: bool,
    override_label: Option<&str>,
    t: &Translator,
) -> anyhow::Result<Value> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let issued = get_usable_issued_certificate_for_application(state, application)
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.noMatchingIssuedCertificate")))?;
    let primary_domain = issued
        .get("primaryDomain")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let cert = issued
        .get("cert")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.ssl.certNotFound")))?;
    let key = issued
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.ssl.certNotFound")))?;
    let existing_id = issued
        .get("libraryCertificateId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let fallback_label = application
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(primary_domain);
    let saved = ssl::save_acme_certificate_to_library(
        state,
        existing_id,
        override_label.or(Some(fallback_label)),
        primary_domain,
        Some(application_id),
        cert,
        key,
        activate,
    )
    .await?;
    if let Some(certificate_id) = saved.get("id").and_then(Value::as_str) {
        link_issued_certificate_to_library(state, application_id, certificate_id).await?;
    }
    Ok(saved)
}

async fn link_issued_certificate_to_library(
    state: &AppState,
    application_id: &str,
    library_certificate_id: &str,
) -> redis::RedisResult<Option<Value>> {
    let mut issued = read_issued_certificates(state).await?;
    let Some(index) = issued
        .iter()
        .position(|item| item.get("applicationId").and_then(Value::as_str) == Some(application_id))
    else {
        return Ok(None);
    };
    if let Some(object) = issued[index].as_object_mut() {
        object.insert(
            "libraryCertificateId".to_string(),
            json!(library_certificate_id),
        );
        object.insert("libraryLinkedAt".to_string(), json!(time_utils::now_iso()));
    }
    let linked = issued[index].clone();
    state
        .redis
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(issued))
        .await?;
    Ok(Some(linked))
}

async fn sync_gateway_if_acme_library_touched(
    state: &AppState,
    certificate_id: &str,
) -> anyhow::Result<()> {
    let config = state.redis.get_config().await?;
    let should_sync = config
        .pointer("/ssl/active_cert_id")
        .and_then(Value::as_str)
        == Some(certificate_id)
        || config
            .pointer("/ssl/deployment_mode")
            .and_then(Value::as_str)
            == Some("multi_sni");
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}

async fn read_acme_cert_pair(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<(String, String)>> {
    let key = format!("{ACME_CERT_PREFIX}{domain}");
    let Some(value) = state.redis.get_json_value(&key).await? else {
        return Ok(None);
    };
    let cert = value
        .get("cert")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(cert.zip(key))
}

async fn get_acme_job(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    Ok(state
        .redis
        .get_json_value(&format!("{ACME_JOB_PREFIX}{id}"))
        .await?
        .and_then(normalize_acme_job))
}

async fn get_acme_logs(
    state: &AppState,
    id: &str,
    limit: usize,
    order: &str,
) -> redis::RedisResult<Vec<Value>> {
    let mut logs = state
        .redis
        .list_log_buffer(
            &format!("{ACME_LOGS_PREFIX}{id}"),
            limit,
            MAX_ACME_LOG_LIMIT,
        )
        .await?
        .into_iter()
        .map(Value::String)
        .collect::<Vec<_>>();
    if order == "desc" {
        logs.reverse();
    }
    Ok(logs)
}

async fn get_active_acme_runtime_lock(state: &AppState) -> redis::RedisResult<Value> {
    let Some(raw_lock) = state.redis.get_json_value(ACME_RUNTIME_LOCK_KEY).await? else {
        return Ok(json!({ "locked": false }));
    };
    let lock = normalize_runtime_lock(&raw_lock);
    if lock.get("locked").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({ "locked": false }));
    }
    let Some(job_id) = lock.get("jobId").and_then(Value::as_str) else {
        return Ok(json!({ "locked": false }));
    };
    let Some(job) = get_acme_job(state, job_id).await? else {
        return Ok(json!({ "locked": false }));
    };
    if matches!(
        job.get("status").and_then(Value::as_str),
        Some("succeeded" | "failed" | "stopped")
    ) {
        return Ok(json!({ "locked": false }));
    }
    Ok(lock)
}

fn normalize_runtime_lock(value: &Value) -> Value {
    let Some(raw) = value.as_object() else {
        return json!({ "locked": false });
    };
    if raw.get("locked").and_then(Value::as_bool) != Some(true) {
        return json!({ "locked": false });
    }
    let mut object = Map::new();
    object.insert("locked".to_string(), json!(true));
    insert_optional_string(&mut object, "lockId", raw.get("lockId"));
    insert_optional_string(&mut object, "jobId", raw.get("jobId"));
    insert_optional_string(&mut object, "applicationId", raw.get("applicationId"));
    insert_optional_value(
        &mut object,
        "reason",
        normalize_job_trigger(raw.get("reason")),
    );
    insert_optional_string(&mut object, "startedAt", raw.get("startedAt"));
    insert_optional_string(&mut object, "heartbeatAt", raw.get("heartbeatAt"));
    insert_optional_string(&mut object, "expiresAt", raw.get("expiresAt"));
    Value::Object(object)
}

fn find_library_certificate(
    ssl_status: &Value,
    application: &Value,
    issued_certificate: &Value,
) -> Option<Value> {
    let application_id = application.get("id").and_then(Value::as_str).unwrap_or("");
    let linked_id = issued_certificate
        .get("libraryCertificateId")
        .and_then(Value::as_str);
    ssl_status
        .get("certificates")
        .and_then(Value::as_array)?
        .iter()
        .find(|certificate| {
            certificate.get("source").and_then(Value::as_str) == Some("acme")
                && (certificate
                    .get("source_ref_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == application_id)
                    || linked_id.is_some_and(|linked_id| {
                        certificate.get("id").and_then(Value::as_str) == Some(linked_id)
                    }))
        })
        .cloned()
}

fn build_latest_job_summary(application: &Value, latest_job: Option<&Value>) -> Value {
    if let Some(job) = latest_job {
        let mut object = Map::new();
        object.insert(
            "id".to_string(),
            job.get("id").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "status".to_string(),
            job.get("status").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "trigger".to_string(),
            job.get("trigger")
                .cloned()
                .unwrap_or_else(|| json!("manual_request")),
        );
        object.insert(
            "createdAt".to_string(),
            job.get("startedAt")
                .or_else(|| job.get("createdAt"))
                .or_else(|| application.get("updatedAt"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        insert_optional_string(&mut object, "message", job.get("message"));
        return Value::Object(object);
    }

    let Some(latest_job_id) = application.get("latestJobId").and_then(Value::as_str) else {
        return Value::Null;
    };
    let mut object = Map::new();
    object.insert("id".to_string(), json!(latest_job_id));
    object.insert(
        "status".to_string(),
        application
            .get("latestJobStatus")
            .cloned()
            .unwrap_or_else(|| json!("idle")),
    );
    object.insert(
        "trigger".to_string(),
        application
            .get("latestJobTrigger")
            .cloned()
            .unwrap_or_else(|| json!("manual_request")),
    );
    object.insert(
        "createdAt".to_string(),
        application
            .get("latestJobAt")
            .or_else(|| application.get("updatedAt"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    insert_optional_string(&mut object, "message", application.get("lastError"));
    Value::Object(object)
}

fn provider_label(t: &Translator, dns_type: &str) -> String {
    let normalized =
        normalize_acme_dns_type(dns_type).unwrap_or_else(|| dns_type.trim().to_string());
    if normalized.is_empty() {
        return "DNS".to_string();
    }
    acme_dns_providers(t)
        .into_iter()
        .find(|provider| {
            provider.get("dnsType").and_then(Value::as_str) == Some(normalized.as_str())
        })
        .and_then(|provider| {
            provider
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or(normalized)
}

fn build_subdomain_certificate_recommendation(
    state: &AppState,
    config: &Value,
    t: &Translator,
) -> Value {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let auth_host = auth_host_mapping(state, config)
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
                .map(normalize_domain_name)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let all_hosts = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|mappings| {
            uniq_strings(
                mappings
                    .iter()
                    .filter_map(|mapping| mapping.get("host").and_then(Value::as_str)),
            )
        })
        .unwrap_or_default();

    let mut mode = "manual";
    let mut summary = t.t("server.subdomainMode.recommendationMissingBase");
    let mut warnings = Vec::<String>::new();
    let mut recommended_domains = Vec::<String>::new();

    if !root_domain.is_empty() {
        mode = "wildcard_parent";
        recommended_domains = uniq_strings([root_domain.as_str(), &format!("*.{root_domain}")]);
        summary = t.t_params(
            "server.subdomainMode.recommendationWildcardSummary",
            &[("rootDomain", root_domain.clone())],
        );
        if !auth_host.is_empty()
            && !is_requirement_covered_by_certificate_domains(&auth_host, &recommended_domains)
        {
            recommended_domains = uniq_strings(
                recommended_domains
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(auth_host.as_str())),
            );
            warnings.push(t.t_params(
                "server.subdomainMode.authOutOfRootWarning",
                &[
                    ("authHost", auth_host.clone()),
                    ("rootDomain", root_domain.clone()),
                ],
            ));
        }
    } else if !auth_host.is_empty() {
        mode = "single_host";
        recommended_domains = vec![auth_host.clone()];
        summary = t.t_params(
            "server.subdomainMode.recommendationSingleHostSummary",
            &[("authHost", auth_host.clone())],
        );
        warnings.push(t.t("server.subdomainMode.wildcardSuggestion"));
    } else {
        warnings.push(t.t("server.subdomainMode.configureRootOrAuth"));
    }

    if auth_host.is_empty() {
        warnings.push(t.t("server.subdomainMode.authMissingWarning"));
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
        warnings.push(t.t_params(
            "server.subdomainMode.uncoveredHostMappingsWarning",
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

fn auth_host_mapping(state: &AppState, config: &Value) -> Option<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| is_auth_service_mapping(state, mapping))
        .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
}

fn is_auth_service_mapping(state: &AppState, mapping: &Value) -> bool {
    if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
        return true;
    }
    let target = mapping.get("target").and_then(Value::as_str).unwrap_or("");
    parse_target_port(target) == Some(state.settings.auth_port)
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

fn uniq_strings<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
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

fn acme_dns_providers(t: &Translator) -> Vec<Value> {
    let common_label = t.t("server.acmeDnsProviders.groups.common");
    let domestic_label = t.t("server.acmeDnsProviders.groups.domestic");
    let international_label = t.t("server.acmeDnsProviders.groups.international");
    let self_hosted_label = t.t("server.acmeDnsProviders.groups.selfHostedAdvanced");
    let common = common_label.as_str();
    let domestic = domestic_label.as_str();
    let international = international_label.as_str();
    let self_hosted = self_hosted_label.as_str();
    let default_credential_label = t.t("server.acmeDnsProviders.credentialSchemes.default");
    let mut providers = vec![
        json!({
            "dnsType": "dns_cf",
            "label": "Cloudflare",
            "group": common,
            "credentialSchemes": [
                scheme("global-key", "Global API Key", &["CF_Key", "CF_Email"], &[]),
                scheme("api-token", "API Token", &["CF_Token", "CF_Zone_ID", "CF_Account_ID"], &["CF_Zone_ID", "CF_Account_ID"]),
            ],
        }),
        simple_provider(
            "dns_ali",
            &t.t("server.acmeDnsProviders.labels.aliyun"),
            common,
            &["Ali_Key", "Ali_Secret"],
            &[],
        ),
        simple_provider("dns_dp", "DNSPod", common, &["DP_Id", "DP_Key"], &[]),
        simple_provider(
            "dns_tencent",
            &t.t("server.acmeDnsProviders.labels.tencentCloudDnspod"),
            common,
            &["Tencent_SecretId", "Tencent_SecretKey"],
            &[],
        ),
        simple_provider("dns_duckdns", "DuckDNS", common, &["DuckDNS_Token"], &[]),
        simple_provider("dns_gd", "GoDaddy", common, &["GD_Key", "GD_Secret"], &[]),
        simple_provider("dns_dgon", "DigitalOcean", common, &["DO_API_KEY"], &[]),
        simple_provider(
            "dns_netlify",
            "Netlify",
            common,
            &["NETLIFY_ACCESS_TOKEN"],
            &[],
        ),
        simple_provider("dns_vercel", "Vercel", common, &["VERCEL_TOKEN"], &[]),
        simple_provider(
            "dns_aws",
            "AWS Route53",
            common,
            &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
            &[],
        ),
        simple_provider(
            "dns_gcloud",
            "Google Cloud DNS (gcloud)",
            common,
            &["CLOUDSDK_ACTIVE_CONFIG_NAME"],
            &["CLOUDSDK_ACTIVE_CONFIG_NAME"],
        ),
        json!({
            "dnsType": "dns_azure",
            "label": "Azure DNS",
            "group": common,
            "credentialSchemes": [
                scheme("service-principal", "Service Principal", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"], &[]),
                scheme("bearer-token", "Bearer Token", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_BEARERTOKEN"], &[]),
                scheme("managed-identity", "Managed Identity", &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_MANAGEDIDENTITY"], &[]),
            ],
        }),
        simple_provider(
            "dns_porkbun",
            "Porkbun",
            common,
            &["PORKBUN_API_KEY", "PORKBUN_SECRET_API_KEY"],
            &[],
        ),
        json!({
            "dnsType": "dns_dynv6",
            "label": "dynv6",
            "group": common,
            "credentialSchemes": [
                scheme("rest-token", "REST API Token", &["DYNV6_TOKEN"], &[]),
                scheme("ssh-key", "SSH Key", &["KEY"], &[]),
            ],
        }),
        simple_provider(
            "dns_huaweicloud",
            &t.t("server.acmeDnsProviders.labels.huaweiCloudDns"),
            domestic,
            &[
                "HUAWEICLOUD_Username",
                "HUAWEICLOUD_Password",
                "HUAWEICLOUD_DomainName",
            ],
            &[],
        ),
        simple_provider(
            "dns_jd",
            &t.t("server.acmeDnsProviders.labels.jdCloudDns"),
            domestic,
            &["JD_ACCESS_KEY_ID", "JD_ACCESS_KEY_SECRET", "JD_REGION"],
            &[],
        ),
        simple_provider("dns_la", "DNS.LA", domestic, &["LA_Id", "LA_Sk"], &[]),
        simple_provider(
            "dns_west_cn",
            &t.t("server.acmeDnsProviders.labels.westCn"),
            domestic,
            &["WEST_Username", "WEST_Key"],
            &[],
        ),
        simple_provider(
            "dns_linode_v4",
            "Linode",
            international,
            &["LINODE_V4_API_KEY"],
            &[],
        ),
        simple_provider("dns_vultr", "Vultr", international, &["VULTR_API_KEY"], &[]),
        simple_provider(
            "dns_ovh",
            "OVH",
            international,
            &["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"],
            &["OVH_END_POINT"],
        ),
        simple_provider(
            "dns_hetzner",
            "Hetzner",
            international,
            &["HETZNER_Token"],
            &[],
        ),
        simple_provider(
            "dns_namecheap",
            "Namecheap",
            international,
            &[
                "NAMECHEAP_API_KEY",
                "NAMECHEAP_USERNAME",
                "NAMECHEAP_SOURCEIP",
            ],
            &[],
        ),
        simple_provider(
            "dns_namecom",
            "Name.com",
            international,
            &["Namecom_Username", "Namecom_Token"],
            &[],
        ),
        simple_provider(
            "dns_namesilo",
            "NameSilo",
            international,
            &["Namesilo_Key"],
            &[],
        ),
        simple_provider(
            "dns_dreamhost",
            "DreamHost",
            international,
            &["DH_API_KEY"],
            &[],
        ),
        simple_provider(
            "dns_freedns",
            "FreeDNS",
            international,
            &["FREEDNS_User", "FREEDNS_Password"],
            &[],
        ),
        simple_provider(
            "dns_dyn",
            "Dyn Managed DNS",
            international,
            &["DYN_Customer", "DYN_Username", "DYN_Password"],
            &[],
        ),
        simple_provider(
            "dns_dynu",
            "Dynu",
            international,
            &["Dynu_ClientId", "Dynu_Secret"],
            &[],
        ),
        simple_provider(
            "dns_bunny",
            "Bunny DNS",
            international,
            &["BUNNY_API_KEY"],
            &[],
        ),
        simple_provider("dns_desec", "deSEC", international, &["DEDYN_TOKEN"], &[]),
        simple_provider(
            "dns_freemyip",
            "FreeMyIP",
            international,
            &["FREEMYIP_Token"],
            &[],
        ),
        simple_provider(
            "dns_ipv64",
            "IPv64.net",
            international,
            &["IPv64_Token"],
            &[],
        ),
        simple_provider(
            "dns_scaleway",
            "Scaleway",
            international,
            &["SCALEWAY_API_TOKEN"],
            &[],
        ),
        simple_provider(
            "dns_easydns",
            "easyDNS",
            international,
            &["EASYDNS_Token", "EASYDNS_Key"],
            &[],
        ),
        simple_provider(
            "dns_zoneedit",
            "ZoneEdit",
            international,
            &["ZONEEDIT_ID", "ZONEEDIT_Token"],
            &[],
        ),
        simple_provider("dns_zonomi", "Zonomi", international, &["ZM_Key"], &[]),
        simple_provider(
            "dns_dnsexit",
            "DNSExit",
            international,
            &["DNSEXIT_API_KEY", "DNSEXIT_AUTH_USER", "DNSEXIT_AUTH_PASS"],
            &[],
        ),
        json!({
            "dnsType": "dns_yandex360",
            "label": "Yandex 360",
            "group": international,
            "credentialSchemes": [
                scheme("oauth-client", "OAuth Client", &["YANDEX360_CLIENT_ID", "YANDEX360_CLIENT_SECRET", "YANDEX360_ORG_ID"], &["YANDEX360_ORG_ID"]),
                scheme("access-token", "Access Token", &["YANDEX360_ACCESS_TOKEN", "YANDEX360_ORG_ID"], &["YANDEX360_ORG_ID"]),
            ],
        }),
        simple_provider(
            "dns_mydnsjp",
            "MyDNS.JP",
            international,
            &["MYDNSJP_MasterID", "MYDNSJP_Password"],
            &[],
        ),
        simple_provider(
            "dns_gandi_livedns",
            "Gandi LiveDNS",
            international,
            &["GANDI_LIVEDNS_KEY"],
            &[],
        ),
        simple_provider("dns_nsone", "NS1", international, &["NS1_Key"], &[]),
        simple_provider(
            "dns_dnsimple",
            "DNSimple",
            international,
            &["DNSimple_OAUTH_TOKEN"],
            &[],
        ),
        json!({
            "dnsType": "dns_cloudns",
            "label": "ClouDNS",
            "group": international,
            "credentialSchemes": [
                scheme("auth-id", "Auth ID", &["CLOUDNS_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"], &[]),
                scheme("sub-auth-id", "Sub Auth ID", &["CLOUDNS_SUB_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"], &[]),
            ],
        }),
        simple_provider(
            "dns_he",
            "Hurricane Electric",
            international,
            &["HE_Username", "HE_Password"],
            &[],
        ),
        simple_provider(
            "dns_transip",
            "TransIP",
            international,
            &["TRANSIP_Username", "TRANSIP_Key_File"],
            &[],
        ),
        simple_provider(
            "dns_doapi",
            "Domain-Offensive",
            international,
            &["DO_LETOKEN"],
            &[],
        ),
        simple_provider(
            "dns_acmedns",
            "acme-dns",
            self_hosted,
            &[
                "ACMEDNS_USERNAME",
                "ACMEDNS_PASSWORD",
                "ACMEDNS_SUBDOMAIN",
                "ACMEDNS_BASE_URL",
            ],
            &["ACMEDNS_BASE_URL"],
        ),
        simple_provider(
            "dns_nsupdate",
            "nsupdate",
            self_hosted,
            &[
                "NSUPDATE_SERVER",
                "NSUPDATE_SERVER_PORT",
                "NSUPDATE_KEY",
                "NSUPDATE_ZONE",
            ],
            &["NSUPDATE_SERVER_PORT", "NSUPDATE_KEY", "NSUPDATE_ZONE"],
        ),
        simple_provider(
            "dns_pdns",
            "PowerDNS",
            self_hosted,
            &["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"],
            &["PDNS_Ttl"],
        ),
        simple_provider(
            "dns_technitium",
            "Technitium DNS",
            self_hosted,
            &[
                "Technitium_Server",
                "Technitium_Token",
                "Technitium_Expiry_Ttl",
            ],
            &["Technitium_Expiry_Ttl"],
        ),
        simple_provider(
            "dns_pleskxml",
            "Plesk XML API",
            self_hosted,
            &["pleskxml_uri", "pleskxml_user", "pleskxml_pass"],
            &[],
        ),
        simple_provider(
            "dns_cpanel",
            "cPanel",
            self_hosted,
            &["cPanel_Username", "cPanel_Apitoken", "cPanel_Hostname"],
            &[],
        ),
        simple_provider(
            "dns_da",
            "DirectAdmin",
            self_hosted,
            &["DA_Api", "DA_Api_Insecure"],
            &[],
        ),
        simple_provider(
            "dns_ispconfig",
            "ISPConfig",
            self_hosted,
            &[
                "ISPC_User",
                "ISPC_Password",
                "ISPC_Api",
                "ISPC_Api_Insecure",
            ],
            &[],
        ),
        simple_provider(
            "dns_opnsense",
            "OPNsense",
            self_hosted,
            &[
                "OPNs_Host",
                "OPNs_Port",
                "OPNs_Key",
                "OPNs_Token",
                "OPNs_Api_Insecure",
            ],
            &["OPNs_Port", "OPNs_Api_Insecure"],
        ),
    ];
    localize_default_credential_labels(&mut providers, &default_credential_label);
    providers
}

fn localize_default_credential_labels(providers: &mut [Value], label: &str) {
    for provider in providers {
        let Some(schemes) = provider
            .get_mut("credentialSchemes")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for scheme in schemes {
            if scheme.get("id").and_then(Value::as_str) == Some("default") {
                scheme["label"] = json!(label);
            }
        }
    }
}

fn simple_provider(
    dns_type: &str,
    label: &str,
    group: &str,
    fields: &[&str],
    optional_fields: &[&str],
) -> Value {
    json!({
        "dnsType": dns_type,
        "label": label,
        "group": group,
        "credentialSchemes": [scheme("default", "Default credentials", fields, optional_fields)],
    })
}

fn scheme(id: &str, label: &str, fields: &[&str], optional_fields: &[&str]) -> Value {
    let optional = optional_fields.iter().copied().collect::<BTreeSet<_>>();
    json!({
        "id": id,
        "label": label,
        "fields": fields.iter().map(|key| {
            json!({
                "key": key,
                "required": !optional.contains(key),
            })
        }).collect::<Vec<_>>(),
    })
}

async fn current_acme_install_state(state: &AppState, t: &Translator) -> Value {
    if let Some(raw) = state.acme_install_state.read().await.clone()
        && raw.get("status").and_then(Value::as_str) == Some("installing")
    {
        return localize_acme_install_state(raw, t);
    }

    if let Err(error) = migrate_legacy_acme_install_if_needed(state).await {
        set_acme_install_state(
            state,
            "error",
            0,
            "checkInstallFailed",
            &[("detail", error.to_string())],
        )
        .await;
        if let Some(raw) = state.acme_install_state.read().await.clone() {
            return localize_acme_install_state(raw, t);
        }
    }

    let executable_path = acme_executable_path(state);
    if executable_path.is_file() {
        json!({
            "status": "installed",
            "progress": 100,
            "message": t.t("server.acmeService.ready"),
            "messageKey": "ready",
            "executablePath": executable_path,
        })
    } else if let Some(raw) = state.acme_install_state.read().await.clone()
        && raw.get("status").and_then(Value::as_str) == Some("error")
    {
        localize_acme_install_state(raw, t)
    } else {
        acme_install_state_value(state, "uninstalled", 0, "notInstalled", &[], t)
    }
}

fn acme_executable_path(state: &AppState) -> PathBuf {
    state.settings.data_dir.join(".acme.sh").join("acme.sh")
}

async fn acme_install_is_installing(state: &AppState) -> bool {
    state
        .acme_install_state
        .read()
        .await
        .as_ref()
        .and_then(|value| value.get("status").and_then(Value::as_str))
        == Some("installing")
}

async fn set_acme_install_state(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
) {
    let value = acme_install_state_value(
        state,
        status,
        progress,
        message_key,
        params,
        &Translator::new(DEFAULT_ACME_LOCALE),
    );
    *state.acme_install_state.write().await = Some(value);
}

fn acme_install_state_value(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
    t: &Translator,
) -> Value {
    let full_key = format!("server.acmeService.{message_key}");
    let mut params_object = Map::new();
    for (key, value) in params {
        params_object.insert((*key).to_string(), json!(value));
    }
    json!({
        "status": status,
        "progress": progress.clamp(0, 100),
        "message": t.t_params(&full_key, params),
        "messageKey": message_key,
        "messageParams": params_object,
        "executablePath": acme_executable_path(state),
    })
}

fn localize_acme_install_state(mut raw: Value, t: &Translator) -> Value {
    let Some(message_key) = raw
        .get("messageKey")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return raw;
    };
    let owned_params = raw
        .get("messageParams")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .as_str()
                        .map(|value| (key.to_string(), value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let params = owned_params
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect::<Vec<_>>();
    raw["message"] = json!(t.t_params(&format!("server.acmeService.{message_key}"), &params));
    raw
}

const DEFAULT_ACME_LOCALE: &str = "zh-CN";

fn default_certificate_authority(state: &AppState) -> &'static str {
    let account_conf = state
        .settings
        .data_dir
        .join(".acme.sh")
        .join("account.conf");
    let content = std::fs::read_to_string(account_conf).unwrap_or_default();
    for line in content.lines() {
        let Some(raw) = line.strip_prefix("DEFAULT_ACME_SERVER=") else {
            continue;
        };
        let lower = raw.trim_matches(['"', '\'']).to_ascii_lowercase();
        if lower.contains("letsencrypt") {
            return "letsencrypt";
        }
        if lower.contains("zerossl") {
            return "zerossl";
        }
    }
    DEFAULT_ACME_CERTIFICATE_AUTHORITY
}

fn normalize_certificate_authority(value: Option<&str>) -> String {
    if value == Some("letsencrypt") {
        "letsencrypt".to_string()
    } else {
        DEFAULT_ACME_CERTIFICATE_AUTHORITY.to_string()
    }
}

async fn save_client_settings(
    state: &AppState,
    certificate_authority: &str,
) -> redis::RedisResult<Value> {
    let settings = json!({
        "certificateAuthority": normalize_certificate_authority(Some(certificate_authority)),
        "updatedAt": time_utils::now_iso(),
    });
    state
        .redis
        .set_json_value(ACME_CLIENT_SETTINGS_KEY, &settings)
        .await?;
    Ok(settings)
}

async fn migrate_legacy_acme_install_if_needed(state: &AppState) -> anyhow::Result<()> {
    let acme_home = acme_home_dir(state);
    let legacy_home = legacy_acme_home_dir();
    if acme_home == legacy_home || !legacy_home.join("acme.sh").is_file() {
        return Ok(());
    }
    let acme_home_clone = acme_home.clone();
    tokio::task::spawn_blocking(move || {
        if acme_home_clone.exists() {
            std::fs::remove_dir_all(&acme_home_clone)?;
        }
        std::fs::create_dir_all(&acme_home_clone)?;
        copy_dir_recursive(&legacy_home, &acme_home_clone)?;
        chmod_executable(&acme_home_clone.join("acme.sh"));
        Ok::<(), anyhow::Error>(())
    })
    .await?
}

async fn start_acme_install(state: AppState, certificate_authority: String) {
    if acme_install_is_installing(&state).await || acme_executable_path(&state).is_file() {
        return;
    }
    set_acme_install_state(&state, "installing", 10, "initializingBundled", &[]).await;

    let install_result = async {
        let install_state = state.clone();
        let executable_path =
            tokio::task::spawn_blocking(move || install_from_bundled_zip_blocking(&install_state))
                .await??;

        set_acme_install_state(&state, "installing", 90, "registeringAccount", &[]).await;
        let account_email = register_acme_account(
            &state,
            None,
            Some(&certificate_authority),
            &Translator::new(DEFAULT_ACME_LOCALE),
        )
        .await?;
        set_acme_install_state(&state, "installing", 95, "savingDefaultCa", &[]).await;
        set_default_certificate_authority(
            &state,
            &certificate_authority,
            &Translator::new(DEFAULT_ACME_LOCALE),
        )
        .await?;
        Ok::<(PathBuf, String), anyhow::Error>((executable_path, account_email))
    }
    .await;

    match install_result {
        Ok((_executable_path, account_email)) => {
            set_acme_install_state(
                &state,
                "installed",
                100,
                "installSuccess",
                &[("email", account_email)],
            )
            .await;
        }
        Err(error) => {
            set_acme_install_state(
                &state,
                "error",
                0,
                "installFailed",
                &[("detail", error.to_string())],
            )
            .await;
        }
    }
}

fn install_from_bundled_zip_blocking(state: &AppState) -> anyhow::Result<PathBuf> {
    let bundle_zip_path = resolve_bundled_acme_zip_path().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.bundledZipMissing")
        )
    })?;
    let acme_home = acme_home_dir(state);
    let executable_path = acme_executable_path(state);
    let tmp_dir = acme_home
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".acme-extract-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));

    set_acme_install_state_blocking(state, "installing", 35, "extractingBundled", &[]);
    std::fs::create_dir_all(&tmp_dir)?;
    let result = (|| {
        extract_zip_file(&bundle_zip_path, &tmp_dir)?;
        let extracted_root = locate_extracted_root(&tmp_dir).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.extractedAcmeMissing")
            )
        })?;
        set_acme_install_state_blocking(state, "installing", 70, "writingDataDir", &[]);
        if acme_home.exists() {
            std::fs::remove_dir_all(&acme_home)?;
        }
        std::fs::create_dir_all(&acme_home)?;
        copy_dir_recursive(&extracted_root, &acme_home)?;
        if !executable_path.is_file() {
            anyhow::bail!(
                "{}",
                Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.writtenAcmeMissing")
            );
        }
        chmod_executable(&executable_path);
        Ok(executable_path.clone())
    })();
    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

fn set_acme_install_state_blocking(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
) {
    if let Ok(mut guard) = state.acme_install_state.try_write() {
        *guard = Some(acme_install_state_value(
            state,
            status,
            progress,
            message_key,
            params,
            &Translator::new(DEFAULT_ACME_LOCALE),
        ));
    }
}

fn resolve_bundled_acme_zip_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var("ACME_BUNDLE_ZIP") {
        if !value.trim().is_empty() {
            candidates.push(PathBuf::from(value.trim()));
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(meta_dir) = exe.parent()
    {
        candidates.extend([
            meta_dir.join("resources/acmesh.zip"),
            meta_dir.join("../resources/acmesh.zip"),
            meta_dir.join("../../resources/acmesh.zip"),
            meta_dir.join("../../../resources/acmesh.zip"),
        ]);
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.extend([
            cwd.join("resources/acmesh.zip"),
            cwd.join("apps/server-admin/resources/acmesh.zip"),
            cwd.join("server/server-admin/resources/acmesh.zip"),
        ]);
    }

    let mut seen = BTreeSet::new();
    candidates.into_iter().find(|path| {
        let normalized = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string();
        seen.insert(normalized) && path.is_file()
    })
}

fn extract_zip_file(zip_path: &Path, output_dir: &Path) -> anyhow::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let output_path = output_dir.join(enclosed);
        if entry.is_dir() {
            std::fs::create_dir_all(&output_path)?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

fn locate_extracted_root(tmp_dir: &Path) -> Option<PathBuf> {
    for candidate in [
        tmp_dir.join("acmesh"),
        tmp_dir.join(".acme.sh"),
        tmp_dir.to_path_buf(),
    ] {
        if candidate.join("acme.sh").is_file() {
            return Some(candidate);
        }
    }
    let entries = std::fs::read_dir(tmp_dir).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.file_name().and_then(|value| value.to_str()) == Some("__MACOSX") {
            continue;
        }
        if candidate.is_dir() && candidate.join("acme.sh").is_file() {
            return Some(candidate);
        }
    }
    None
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn legacy_acme_home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".acme.sh")
}

#[cfg(unix)]
fn chmod_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn chmod_executable(_path: &Path) {}

struct AcmeCommandResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

async fn switch_certificate_authority(
    state: &AppState,
    certificate_authority: &str,
    t: &Translator,
) -> anyhow::Result<String> {
    if !acme_executable_path(state).is_file() {
        anyhow::bail!(t.t("server.acmeService.installFirst"));
    }
    let account_email = register_acme_account(state, None, Some(certificate_authority), t).await?;
    set_default_certificate_authority(state, certificate_authority, t).await?;
    Ok(account_email)
}

async fn register_acme_account(
    state: &AppState,
    email: Option<&str>,
    certificate_authority: Option<&str>,
    t: &Translator,
) -> anyhow::Result<String> {
    let account_email = resolve_account_email(state, email).await;
    let mut args = vec![
        "--register-account".to_string(),
        "-m".to_string(),
        account_email.clone(),
    ];
    args.extend(shared_acme_args(state, certificate_authority));
    args.push("--debug".to_string());
    let result = run_acme_command(state, args, None).await?;
    if result.exit_code == 0 {
        return Ok(account_email);
    }
    let merged = format!("{}\n{}", result.stdout, result.stderr).to_ascii_lowercase();
    if (merged.contains("already") && merged.contains("account"))
        || (merged.contains("already") && merged.contains("registered"))
    {
        return Ok(account_email);
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.registerAccountFailed",
        &[
            ("code", result.exit_code.to_string()),
            (
                "brief",
                command_output_brief(&result.stdout, &result.stderr)
            ),
        ],
    ))
}

async fn set_default_certificate_authority(
    state: &AppState,
    certificate_authority: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let mut args = vec![
        "--set-default-ca".to_string(),
        "--server".to_string(),
        normalize_certificate_authority(Some(certificate_authority)),
    ];
    args.extend(shared_acme_args(state, None));
    args.push("--debug".to_string());
    let result = run_acme_command(state, args, None).await?;
    if result.exit_code == 0 {
        return Ok(());
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.setDefaultCaFailed",
        &[
            ("code", result.exit_code.to_string()),
            (
                "brief",
                command_output_brief(&result.stdout, &result.stderr)
            ),
        ],
    ))
}

async fn run_acme_command(
    state: &AppState,
    args: Vec<String>,
    extra_env: Option<&Map<String, Value>>,
) -> anyhow::Result<AcmeCommandResult> {
    let mut command = Command::new(acme_executable_path(state));
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(extra_env) = extra_env {
        for (key, value) in extra_env {
            if let Some(value) = value.as_str() {
                command.env(key, value);
            }
        }
    }
    let output = command.output().await?;
    Ok(AcmeCommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn shared_acme_args(state: &AppState, certificate_authority: Option<&str>) -> Vec<String> {
    let acme_home = acme_home_dir(state).to_string_lossy().to_string();
    let mut args = vec![
        "--home".to_string(),
        acme_home.clone(),
        "--config-home".to_string(),
        acme_home,
    ];
    if let Some(certificate_authority) = certificate_authority {
        args.push("--server".to_string());
        args.push(normalize_certificate_authority(Some(certificate_authority)));
    }
    args
}

fn command_output_brief(stdout: &str, stderr: &str) -> String {
    let brief = format!("{stdout}\n{stderr}")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    if brief.is_empty() {
        String::new()
    } else {
        format!(": {brief}")
    }
}

async fn resolve_account_email(state: &AppState, email: Option<&str>) -> String {
    if let Some(value) = email.map(str::trim).filter(|value| is_valid_email(value)) {
        return value.to_string();
    }
    if let Ok(value) = env::var("ACME_ACCOUNT_EMAIL")
        && is_valid_email(value.trim())
    {
        return value.trim().to_string();
    }
    if let Some(value) = get_existing_account_email(state).await {
        return value;
    }
    format!(
        "acme-{}-{}@fnknock.com",
        time_utils::now_ms(),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

async fn get_existing_account_email(state: &AppState) -> Option<String> {
    let candidates = [
        acme_home_dir(state).join("account.conf"),
        acme_home_dir(state).join("ca/acme.zerossl.com/v2/DV90/account.conf"),
        acme_home_dir(state).join("ca/acme-v02.api.letsencrypt.org/directory/account.conf"),
    ];
    for path in candidates {
        let Ok(content) = tokio::fs::read_to_string(path).await else {
            continue;
        };
        for line in content.lines() {
            let Some(raw) = line.strip_prefix("ACCOUNT_EMAIL=") else {
                continue;
            };
            let value = raw.trim().trim_matches(['"', '\'']);
            if is_valid_email(value) {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn is_valid_email(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

async fn apply_acme_dns_provider_patches(
    state: &AppState,
    dns_type: &str,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    if dns_type != "dns_duckdns" {
        return Ok(());
    }
    const DEFAULT_API: &str = "https://www.duckdns.org/update";
    const PROXY_API: &str = "https://duckdns.fnknock.cn/update";
    let script_path = acme_home_dir(state).join("dnsapi/dns_duckdns.sh");
    let content = tokio::fs::read_to_string(&script_path).await.map_err(|_| {
        anyhow::anyhow!(t.t_params(
            "server.acmePatches.duckdns.scriptMissing",
            &[("path", script_path.to_string_lossy().to_string())],
        ))
    })?;
    if content.contains(PROXY_API) || !content.contains(DEFAULT_API) {
        return Ok(());
    }
    let updated = content.replace(DEFAULT_API, PROXY_API);
    if updated != content {
        tokio::fs::write(&script_path, updated).await?;
        append_acme_log(
            state,
            job_id,
            &t.t_params(
                "server.acmePatches.duckdns.proxyApplied",
                &[
                    ("from", DEFAULT_API.to_string()),
                    ("to", PROXY_API.to_string()),
                ],
            ),
        )
        .await
        .ok();
    }
    Ok(())
}

async fn read_replayable_json_body(
    req: Request<Body>,
    t: &Translator,
) -> Result<(Value, Request<Body>), Response> {
    let (parts, body) = req.into_parts();
    let bytes = match to_bytes(body, MAX_ACME_BODY_BYTES).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to read ACME request body");
            return Err(response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(t, "invalidRequestBody"),
            ));
        }
    };
    let replayable = Request::from_parts(parts, Body::from(bytes.clone()));
    let value = parse_json_bytes(&bytes, t)?;
    Ok((value, replayable))
}

fn parse_json_bytes(bytes: &Bytes, t: &Translator) -> Result<Value, Response> {
    if bytes.is_empty() {
        return Err(response::error(
            StatusCode::BAD_REQUEST,
            acme_route_text(t, "invalidRequestBody"),
        ));
    }
    serde_json::from_slice(bytes).map_err(|_| {
        response::error(
            StatusCode::BAD_REQUEST,
            acme_route_text(t, "invalidRequestBody"),
        )
    })
}

fn submit_now_requested(value: &Value) -> bool {
    value.get("submitNow").and_then(Value::as_bool) == Some(true)
}

fn build_pending_acme_application_for_update(
    existing: &Value,
    body: &Value,
    normalized: &NormalizedAcmeRequest,
) -> Value {
    let mut application = existing.as_object().cloned().unwrap_or_default();
    if body.get("name").is_some() {
        if let Some(name) = body
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            application.insert("name".to_string(), json!(name));
        } else {
            application.remove("name");
        }
    }
    application.insert("domains".to_string(), json!(normalized.domains.clone()));
    application.insert(
        "primaryDomain".to_string(),
        json!(
            normalized
                .domains
                .first()
                .cloned()
                .or_else(|| {
                    existing
                        .get("primaryDomain")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default()
        ),
    );
    application.insert("dnsType".to_string(), json!(normalized.dns_type.clone()));
    application.insert("credentials".to_string(), normalized.credentials.clone());
    if let Some(renew_enabled) = body.get("renewEnabled").and_then(Value::as_bool) {
        application.insert("renewEnabled".to_string(), json!(renew_enabled));
    }
    Value::Object(application)
}

fn validate_acme_request(input: &Value, t: &Translator) -> Result<NormalizedAcmeRequest, String> {
    let domains = input
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_valid_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        return Err(t.t("server.acmeRoutes.domainsInvalid"));
    }

    let dns_type = input
        .get("dnsType")
        .or_else(|| input.get("provider"))
        .and_then(value_to_trimmed_string)
        .and_then(|value| normalize_acme_dns_type(&value))
        .ok_or_else(|| t.t("server.acmeRoutes.dnsTypeRequired"))?;
    let provider = acme_dns_providers(t)
        .into_iter()
        .find(|provider| provider.get("dnsType").and_then(Value::as_str) == Some(dns_type.as_str()))
        .ok_or_else(|| t.t("server.acmeRoutes.unsupportedDnsProvider"))?;
    let credentials =
        filter_acme_credentials_for_provider(&provider, &dns_type, input.get("credentials"));
    if !credential_scheme_satisfied(&provider, &credentials) {
        return Err(t.t_params(
            "server.acmeRoutes.missingDnsCredentials",
            &[("requirements", format_credential_requirements(&provider, t))],
        ));
    }

    Ok(NormalizedAcmeRequest {
        domains,
        dns_type,
        credentials: Value::Object(credentials),
    })
}

fn normalize_valid_domain_list<'a>(values: impl Iterator<Item = &'a Value>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let Some(domain) = value_to_trimmed_string(value).map(|value| value.to_ascii_lowercase())
        else {
            continue;
        };
        if !is_valid_acme_domain(&domain) || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

fn is_valid_acme_domain(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 253
        || value.contains("..")
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains('/')
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    let host = value.strip_prefix("*.").unwrap_or(value);
    if host.starts_with("*.") || host.contains('*') {
        return false;
    }
    let labels = host.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

fn value_to_trimmed_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("value was normalized to object")
}

fn normalize_acme_dns_type(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let aliased = match lower.as_str() {
        "aliyun" => "dns_ali",
        "cloudflare" => "dns_cf",
        "dnspod" => "dns_dp",
        "tencentcloud" => "dns_tencent",
        "duckdns" => "dns_duckdns",
        "google" | "gcloud" | "dns_google" => "dns_gcloud",
        "huaweicloud" | "huawei" => "dns_huaweicloud",
        "netlify" => "dns_netlify",
        _ => "",
    };
    if !aliased.is_empty() {
        return Some(aliased.to_string());
    }
    if lower.starts_with("dns_")
        && lower
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Some(lower);
    }
    None
}

fn normalize_acme_env_vars(dns_type: &str, credentials: Option<&Value>) -> Map<String, Value> {
    let mut record = normalize_string_record(credentials)
        .as_object()
        .cloned()
        .unwrap_or_default();
    if dns_type == "dns_netlify"
        && !record.contains_key("NETLIFY_ACCESS_TOKEN")
        && let Some(value) = record.get("NETLIFY_TOKEN").cloned()
    {
        record.insert("NETLIFY_ACCESS_TOKEN".to_string(), value);
    }
    record
}

fn filter_acme_credentials_for_provider(
    provider: &Value,
    dns_type: &str,
    credentials: Option<&Value>,
) -> Map<String, Value> {
    let normalized = normalize_string_record(credentials);
    let mut record = normalized.as_object().cloned().unwrap_or_default();
    if dns_type == "dns_netlify"
        && !record.contains_key("NETLIFY_ACCESS_TOKEN")
        && let Some(value) = record.get("NETLIFY_TOKEN").cloned()
    {
        record.insert("NETLIFY_ACCESS_TOKEN".to_string(), value);
    }
    let allowed_keys = provider_credential_keys(provider);
    record
        .into_iter()
        .filter(|(key, _)| allowed_keys.contains(key))
        .collect()
}

fn provider_credential_keys(provider: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(schemes) = provider.get("credentialSchemes").and_then(Value::as_array) {
        for scheme in schemes {
            if let Some(fields) = scheme.get("fields").and_then(Value::as_array) {
                for field in fields {
                    if let Some(key) = field.get("key").and_then(Value::as_str) {
                        keys.insert(key.to_string());
                    }
                }
            }
        }
    }
    keys
}

fn credential_scheme_satisfied(provider: &Value, credentials: &Map<String, Value>) -> bool {
    provider
        .get("credentialSchemes")
        .and_then(Value::as_array)
        .is_some_and(|schemes| {
            schemes.iter().any(|scheme| {
                scheme
                    .get("fields")
                    .and_then(Value::as_array)
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .filter(|field| {
                                field.get("required").and_then(Value::as_bool) != Some(false)
                            })
                            .all(|field| {
                                field
                                    .get("key")
                                    .and_then(Value::as_str)
                                    .and_then(|key| credentials.get(key))
                                    .and_then(Value::as_str)
                                    .is_some_and(|value| !value.trim().is_empty())
                            })
                    })
            })
        })
}

fn format_credential_requirements(provider: &Value, t: &Translator) -> String {
    let schemes = provider
        .get("credentialSchemes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if schemes.len() == 1 {
        return required_credential_keys(&schemes[0]).join(", ");
    }
    schemes
        .iter()
        .map(|scheme| {
            let required = required_credential_keys(scheme).join(", ");
            let optional = optional_credential_keys(scheme);
            let suffix = if optional.is_empty() {
                String::new()
            } else {
                t.t_params(
                    "server.acmeDnsProviders.requirements.optionalSuffix",
                    &[("keys", optional.join(", "))],
                )
            };
            format!(
                "{}: {required}{suffix}",
                scheme
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("Credentials")
            )
        })
        .collect::<Vec<_>>()
        .join(&t.t("server.acmeDnsProviders.requirements.orSeparator"))
}

fn required_credential_keys(scheme: &Value) -> Vec<String> {
    scheme
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) != Some(false))
        .filter_map(|field| field.get("key").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn optional_credential_keys(scheme: &Value) -> Vec<String> {
    scheme
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) == Some(false))
        .filter_map(|field| field.get("key").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn normalize_domain_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let domain = normalize_domain_name(&value);
        if domain.is_empty() || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

async fn sync_gateway_if_acme_library_removed(
    state: &AppState,
    removed_active: bool,
    removed_count: usize,
) -> anyhow::Result<()> {
    if !removed_active && removed_count == 0 {
        return Ok(());
    }
    let config = state.redis.get_config().await?;
    let should_sync = removed_active
        || (removed_count > 0
            && config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni"));
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}

fn normalize_acme_application(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let id = non_empty_string(raw.get("id"))?;
    let domains = raw
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let primary_domain = raw
        .get("primaryDomain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .or_else(|| domains.first().cloned())?;
    let dns_type = non_empty_string(raw.get("dnsType"))?;
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let updated_at = raw
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| created_at.clone());
    let mut object = Map::new();
    object.insert("id".to_string(), json!(id));
    insert_optional_string(&mut object, "name", raw.get("name"));
    object.insert("domains".to_string(), json!(domains));
    object.insert("primaryDomain".to_string(), json!(primary_domain));
    object.insert("dnsType".to_string(), json!(dns_type));
    object.insert(
        "credentials".to_string(),
        normalize_string_record(raw.get("credentials")),
    );
    object.insert(
        "renewEnabled".to_string(),
        json!(raw.get("renewEnabled").and_then(Value::as_bool) != Some(false)),
    );
    object.insert("createdAt".to_string(), json!(created_at));
    object.insert("updatedAt".to_string(), json!(updated_at));
    insert_optional_string(&mut object, "latestJobId", raw.get("latestJobId"));
    insert_optional_value(
        &mut object,
        "latestJobStatus",
        normalize_latest_job_status(raw.get("latestJobStatus")),
    );
    insert_optional_value(
        &mut object,
        "latestJobTrigger",
        normalize_job_trigger(raw.get("latestJobTrigger")),
    );
    insert_optional_string(&mut object, "latestJobAt", raw.get("latestJobAt"));
    insert_optional_string(&mut object, "lastError", raw.get("lastError"));
    Some(Value::Object(object))
}

fn normalize_issued_certificate(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let application_id = non_empty_string(raw.get("applicationId"))?;
    let primary_domain = raw
        .get("primaryDomain")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())?;
    let cert = non_empty_string(raw.get("cert"))?;
    let key = non_empty_string(raw.get("key"))?;
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let updated_at = raw
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| created_at.clone());
    let cert_info = normalize_cert_info(raw.get("certInfo"))?;
    let mut object = Map::new();
    object.insert("applicationId".to_string(), json!(application_id));
    object.insert("primaryDomain".to_string(), json!(primary_domain));
    object.insert("cert".to_string(), json!(cert));
    object.insert("key".to_string(), json!(key));
    object.insert("certInfo".to_string(), cert_info);
    object.insert("createdAt".to_string(), json!(created_at));
    object.insert("updatedAt".to_string(), json!(updated_at));
    insert_optional_string(
        &mut object,
        "libraryCertificateId",
        raw.get("libraryCertificateId"),
    );
    insert_optional_string(&mut object, "libraryLinkedAt", raw.get("libraryLinkedAt"));
    Some(Value::Object(object))
}

fn normalize_acme_job(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let id = non_empty_string(raw.get("id"))?;
    let domains = raw
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let created_at = raw
        .get("createdAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)?;
    let status = normalize_job_status(raw.get("status"))?;
    if domains.is_empty() {
        return None;
    }
    let method = match raw.get("method").and_then(Value::as_str) {
        Some("http") => "http",
        Some("https") => "https",
        _ => "dns",
    };
    let mut object = Map::new();
    object.insert("id".to_string(), json!(id));
    insert_optional_string(&mut object, "applicationId", raw.get("applicationId"));
    object.insert("domains".to_string(), json!(domains));
    object.insert("method".to_string(), json!(method));
    insert_optional_string(&mut object, "provider", raw.get("provider"));
    insert_optional_value(
        &mut object,
        "trigger",
        normalize_job_trigger(raw.get("trigger")),
    );
    object.insert("createdAt".to_string(), json!(created_at));
    insert_optional_string(&mut object, "startedAt", raw.get("startedAt"));
    insert_optional_string(&mut object, "finishedAt", raw.get("finishedAt"));
    object.insert("status".to_string(), json!(status));
    object.insert(
        "progress".to_string(),
        json!(
            raw.get("progress")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .clamp(0, 100)
        ),
    );
    insert_optional_string(&mut object, "message", raw.get("message"));
    Some(Value::Object(object))
}

fn normalize_client_settings(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let ca = match raw.get("certificateAuthority").and_then(Value::as_str) {
        Some("letsencrypt") => "letsencrypt",
        _ => DEFAULT_ACME_CERTIFICATE_AUTHORITY,
    };
    Some(json!({
        "certificateAuthority": ca,
        "updatedAt": raw
            .get("updatedAt")
            .and_then(Value::as_str)
            .and_then(normalize_timestamp)
            .unwrap_or_else(time_utils::now_iso),
    }))
}

fn normalize_cert_info(value: Option<&Value>) -> Option<Value> {
    let raw = value?.as_object()?;
    let issuer = non_empty_string(raw.get("issuer"))?;
    let subject = non_empty_string(raw.get("subject"))?;
    let valid_from = non_empty_string(raw.get("validFrom"))?;
    let valid_to = non_empty_string(raw.get("validTo"))?;
    let serial_number = non_empty_string(raw.get("serialNumber"))?;
    let dns_names = raw
        .get("dnsNames")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    Some(json!({
        "issuer": issuer,
        "subject": subject,
        "validFrom": valid_from,
        "validTo": valid_to,
        "dnsNames": dns_names,
        "serialNumber": serial_number,
    }))
}

fn issued_certificate_compatible(application: &Value, certificate: &Value) -> bool {
    if certificate.get("primaryDomain").and_then(Value::as_str)
        != application.get("primaryDomain").and_then(Value::as_str)
    {
        return false;
    }
    let app_domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let cert_domains = certificate
        .pointer("/certInfo/dnsNames")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    normalized_domain_signature(&app_domains) == normalized_domain_signature(&cert_domains)
}

fn normalize_domain_list<'a>(values: impl Iterator<Item = &'a Value>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let Some(domain) = value
            .as_str()
            .map(|value| value.trim().to_ascii_lowercase())
        else {
            continue;
        };
        if domain.is_empty() || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

fn normalized_domain_signature(domains: &[String]) -> String {
    let mut normalized = domains
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized.join("\n")
}

fn normalize_string_record(value: Option<&Value>) -> Value {
    let mut output = Map::new();
    let Some(object) = value.and_then(Value::as_object) else {
        return Value::Object(output);
    };
    for (key, value) in object {
        let key = key.trim();
        let value = value.as_str().unwrap_or("").trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        output.insert(key.to_string(), Value::String(value.to_string()));
    }
    Value::Object(output)
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalize_timestamp(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    time_utils::parse_iso_ms(trimmed).map(|_| trimmed.to_string())
}

fn insert_optional_string(object: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(value) = non_empty_string(value) {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn insert_optional_value(object: &mut Map<String, Value>, key: &str, value: Value) {
    if !value.is_null() {
        object.insert(key.to_string(), value);
    }
}

fn normalize_job_status(value: Option<&Value>) -> Option<&'static str> {
    match value.and_then(Value::as_str) {
        Some("queued") => Some("queued"),
        Some("running") => Some("running"),
        Some("succeeded") => Some("succeeded"),
        Some("failed") => Some("failed"),
        Some("stopped") => Some("stopped"),
        _ => None,
    }
}

fn normalize_latest_job_status(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some("idle") => json!("idle"),
        Some("queued") => json!("queued"),
        Some("running") => json!("running"),
        Some("succeeded") => json!("succeeded"),
        Some("failed") => json!("failed"),
        Some("stopped") => json!("stopped"),
        _ => Value::Null,
    }
}

fn normalize_job_trigger(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some("manual_request") => json!("manual_request"),
        Some("auto_renew") => json!("auto_renew"),
        _ => Value::Null,
    }
}

fn normalize_log_limit(value: Option<&str>) -> usize {
    let parsed = value
        .map(parse_js_number_like_query)
        .unwrap_or(Some(DEFAULT_ACME_LOG_LIMIT as f64))
        .unwrap_or(DEFAULT_ACME_LOG_LIMIT as f64);
    if parsed.is_nan() {
        return DEFAULT_ACME_LOG_LIMIT;
    }
    let clamped = parsed.max(1.0).min(MAX_ACME_LOG_LIMIT as f64);
    if !clamped.is_finite() {
        return if clamped.is_sign_positive() {
            MAX_ACME_LOG_LIMIT
        } else {
            1
        };
    }
    clamped.floor() as usize
}

fn parse_js_number_like_query(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }
    trimmed.parse::<f64>().ok()
}

fn build_application_id(seed: Option<&str>) -> String {
    let normalized_seed = seed.unwrap_or("").trim().to_ascii_lowercase();
    if !normalized_seed.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(normalized_seed.as_bytes());
        let digest = hex::encode(hasher.finalize());
        return format!("acme_app_{}", &digest[..16]);
    }
    format!("acme_app_{}", uuid::Uuid::new_v4().simple())
}

fn zip_acme_cert_pair(domain: &str, cert: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::<u8>::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file(format!("{domain}.cert.pem"), options)?;
    zip.write_all(cert.as_bytes())?;
    zip.start_file(format!("{domain}.key.pem"), options)?;
    zip.write_all(key.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_acme_application_like_node() {
        let value = normalize_acme_application(json!({
            "id": " app ",
            "domains": ["Example.com", "example.com", "www.example.com"],
            "dnsType": " dns_cf ",
            "credentials": { " CF_Key ": " secret ", "empty": "" },
            "createdAt": "2026-07-05T01:02:03Z",
            "latestJobStatus": "bad"
        }))
        .expect("application");
        assert_eq!(value["id"], json!("app"));
        assert_eq!(value["primaryDomain"], json!("example.com"));
        assert_eq!(value["domains"], json!(["example.com", "www.example.com"]));
        assert_eq!(value["credentials"], json!({ "CF_Key": "secret" }));
        assert_eq!(value["renewEnabled"], json!(true));
        assert_eq!(value["latestJobStatus"], Value::Null);
    }

    #[test]
    fn detects_issued_certificate_compatibility_by_domain_set() {
        let application = json!({
            "primaryDomain": "example.com",
            "domains": ["example.com", "www.example.com"],
        });
        let certificate = json!({
            "primaryDomain": "example.com",
            "certInfo": { "dnsNames": ["www.example.com", "example.com"] },
        });
        assert!(issued_certificate_compatible(&application, &certificate));
    }

    #[test]
    fn builds_stable_legacy_application_id() {
        assert_eq!(
            build_application_id(Some("Example.com")),
            build_application_id(Some("example.com"))
        );
        assert!(build_application_id(Some("example.com")).starts_with("acme_app_"));
    }

    #[test]
    fn normalizes_log_limit_bounds() {
        assert_eq!(normalize_log_limit(None), DEFAULT_ACME_LOG_LIMIT);
        assert_eq!(normalize_log_limit(Some("")), 1);
        assert_eq!(normalize_log_limit(Some("   ")), 1);
        assert_eq!(normalize_log_limit(Some("0")), 1);
        assert_eq!(normalize_log_limit(Some("-5")), 1);
        assert_eq!(normalize_log_limit(Some("2000")), MAX_ACME_LOG_LIMIT);
        assert_eq!(normalize_log_limit(Some("10")), 10);
        assert_eq!(normalize_log_limit(Some("3.9")), 3);
        assert_eq!(normalize_log_limit(Some("10x")), DEFAULT_ACME_LOG_LIMIT);
    }

    #[test]
    fn localizes_queued_job_domain_validation() {
        let t = Translator::new("zh-CN");
        let error = build_queued_acme_job(&json!({ "domains": [] }), "manual_request", &t)
            .expect_err("empty domains should be rejected");
        assert_eq!(error.to_string(), "域名列表不能为空或格式无效");

        let job = build_queued_acme_job(
            &json!({
                "id": "app-1",
                "domains": ["Example.com"],
                "dnsType": "dns_cf"
            }),
            "auto_renew",
            &t,
        )
        .expect("valid job");
        assert_eq!(job["status"], json!("queued"));
        assert_eq!(job["message"], json!("queued for renew"));
    }

    #[test]
    fn builds_pending_application_for_submit_now_update_like_node() {
        let existing = json!({
            "id": "app-1",
            "name": "Old name",
            "domains": ["old.example.com"],
            "primaryDomain": "old.example.com",
            "dnsType": "dns_cf",
            "credentials": { "CF_Token": "old" },
            "renewEnabled": true,
            "latestJobId": "job-1"
        });
        let normalized = NormalizedAcmeRequest {
            domains: vec!["example.com".to_string(), "*.example.com".to_string()],
            dns_type: "dns_ali".to_string(),
            credentials: json!({ "Ali_Key": "key", "Ali_Secret": "secret" }),
        };
        let pending = build_pending_acme_application_for_update(
            &existing,
            &json!({
                "name": "  ",
                "renewEnabled": false
            }),
            &normalized,
        );

        assert_eq!(pending["id"], json!("app-1"));
        assert!(pending.get("name").is_none());
        assert_eq!(pending["domains"], json!(["example.com", "*.example.com"]));
        assert_eq!(pending["primaryDomain"], json!("example.com"));
        assert_eq!(pending["dnsType"], json!("dns_ali"));
        assert_eq!(
            pending["credentials"],
            json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
        );
        assert_eq!(pending["renewEnabled"], json!(false));
        assert_eq!(pending["latestJobId"], json!("job-1"));
    }

    #[test]
    fn provider_catalog_contains_node_dns_types() {
        let t = Translator::new("en");
        let providers = acme_dns_providers(&t);
        let dns_types = providers
            .iter()
            .filter_map(|item| item.get("dnsType").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        assert!(dns_types.contains("dns_cf"));
        assert!(dns_types.contains("dns_azure"));
        assert!(dns_types.contains("dns_opnsense"));
        assert_eq!(
            providers
                .iter()
                .find(|item| item.get("dnsType").and_then(Value::as_str) == Some("dns_cf"))
                .and_then(|item| item.get("credentialSchemes"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn validates_acme_request_with_alias_and_filters_credentials() {
        let t = Translator::new("en");
        let normalized = validate_acme_request(
            &json!({
                "domains": ["Example.com", "bad host", "*.example.com", "example.com"],
                "dnsType": "aliyun",
                "credentials": {
                    "Ali_Key": " key ",
                    "Ali_Secret": " secret ",
                    "Ignored": "value"
                }
            }),
            &t,
        )
        .expect("valid request");

        assert_eq!(normalized.domains, vec!["example.com", "*.example.com"]);
        assert_eq!(normalized.dns_type, "dns_ali");
        assert_eq!(
            normalized.credentials,
            json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
        );
    }

    #[test]
    fn validates_netlify_credential_alias() {
        let t = Translator::new("en");
        let normalized = validate_acme_request(
            &json!({
                "domains": ["example.com"],
                "provider": "netlify",
                "credentials": {
                    "NETLIFY_TOKEN": "token"
                }
            }),
            &t,
        )
        .expect("valid request");

        assert_eq!(normalized.dns_type, "dns_netlify");
        assert_eq!(
            normalized.credentials,
            json!({ "NETLIFY_ACCESS_TOKEN": "token" })
        );
    }

    #[test]
    fn rejects_missing_acme_credentials() {
        let t = Translator::new("en");
        let error = validate_acme_request(
            &json!({
                "domains": ["example.com"],
                "dnsType": "dns_ali",
                "credentials": {
                    "Ali_Key": "key"
                }
            }),
            &t,
        )
        .expect_err("credentials should be incomplete");

        assert!(error.contains("DNS API credentials are missing"));
        assert!(error.contains("Ali_Secret"));
    }

    #[test]
    fn localizes_acme_route_errors() {
        let t = Translator::new("zh-CN");
        assert_eq!(acme_route_text(&t, "invalidRequestBody"), "请求体不正确");
        assert_eq!(acme_route_text(&t, "loadJobFailed"), "读取 ACME 任务失败");
        assert_eq!(
            acme_route_text(&t, "createCertificateZipFailed"),
            "创建 ACME 证书压缩包失败"
        );
        assert_eq!(
            acme_route_text(&t, "updateApplicationFailed"),
            "更新 ACME 申请项失败"
        );
        assert_eq!(
            acme_route_text(&t, "saveClientSettingsFailed"),
            "保存 ACME 客户端设置失败"
        );
        assert_eq!(
            acme_route_text(&t, "syncLibraryFailed"),
            "同步 ACME 证书到证书库失败"
        );
        assert_eq!(
            acme_route_text(&t, "deployCertificateFailed"),
            "部署 ACME 证书失败"
        );
        assert_eq!(acme_route_text(&t, "stopJobFailed"), "停止 ACME 任务失败");
    }

    #[test]
    fn detects_submit_now_requests_for_fallback() {
        assert!(submit_now_requested(&json!({ "submitNow": true })));
        assert!(!submit_now_requested(&json!({ "submitNow": false })));
        assert!(!submit_now_requested(&json!({})));
    }

    #[test]
    fn validates_acme_domain_like_node() {
        assert!(is_valid_acme_domain("example.com"));
        assert!(is_valid_acme_domain("*.example.com"));
        assert!(!is_valid_acme_domain("example"));
        assert!(!is_valid_acme_domain("deep.*.example.com"));
        assert!(!is_valid_acme_domain("bad host.example.com"));
    }

    #[test]
    fn wildcard_domains_cover_single_label_subdomains_only() {
        let domains = vec!["example.com".to_string(), "*.example.com".to_string()];
        assert!(is_requirement_covered_by_certificate_domains(
            "app.example.com",
            &domains
        ));
        assert!(is_requirement_covered_by_certificate_domains(
            "example.com",
            &domains
        ));
        assert!(!is_requirement_covered_by_certificate_domains(
            "deep.app.example.com",
            &domains
        ));
    }
}
