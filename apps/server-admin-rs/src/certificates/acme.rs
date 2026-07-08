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

mod analysis;
mod certificates;
mod handlers;
mod install;
mod jobs;
mod lookup;
mod normalization;
mod providers;
mod runtime;
mod storage;
mod tasks;
mod validation;

use analysis::*;
use certificates::*;
use handlers::*;
use install::*;
use jobs::*;
use lookup::*;
use normalization::*;
use providers::*;
use runtime::*;
use storage::*;
use tasks::*;
use validation::*;

#[cfg(test)]
mod tests;

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
        .route("/api/admin/acme/status", get(status))
        .route("/api/admin/acme/overview", get(overview))
        .route("/api/admin/acme/dns-providers", get(dns_providers))
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
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(error) = run_acme_auto_renew_once(state.clone()).await {
                tracing::warn!(%error, "ACME auto-renew task failed");
            }
        }
    });
}
