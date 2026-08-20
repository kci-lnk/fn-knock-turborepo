use std::{
    collections::BTreeSet,
    env,
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Stdio,
};

use ::time::{
    Date, Month, OffsetDateTime, PrimitiveDateTime, Time, format_description::well_known::Rfc3339,
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path as AxumPath, Query, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
    process::Command,
    time::{self as tokio_time, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use utoipa_axum::router::OpenApiRouter;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    i18n::Translator,
    response, ssl,
    state::{AcmeJobControl, AppState},
    time_utils,
};

mod analysis;
mod certificates;
mod handlers;
mod install;
mod jobs;
mod lookup;
mod normalization;
mod providers;
mod resource;
mod runtime;
mod storage;
mod tasks;
mod validation;

use analysis::*;
use certificates::*;
#[cfg(test)]
use handlers::build_init_acme_payload;
use install::*;
use jobs::*;
use lookup::*;
use normalization::*;
use providers::*;
use resource::*;
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
}

fn acme_route_text(t: &Translator, key: &str) -> String {
    t.t(&format!("server.acmeRoutes.{key}"))
}

pub fn acme_routes() -> Router<AppState> {
    acme_openapi_routes().into()
}

pub(crate) fn acme_openapi_routes() -> OpenApiRouter<AppState> {
    handlers::openapi_routes().merge(resource::openapi_routes())
}

pub async fn start_acme_tasks(state: AppState) {
    let t = Translator::from_state(&state).await;
    if let Err(error) = recover_orphaned_acme_runtime_job(&state, &t).await {
        tracing::warn!(%error, "failed to recover interrupted ACME runtime state");
    }
    let task_state = state.clone();
    state.spawn_background("acme-auto-renew", async move {
        let mut ticker = acme_renew_ticker(acme_renew_interval());
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                result = run_acme_auto_renew_once(task_state.clone()) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "ACME auto-renew task failed");
                    }
                }
            }
        }
    });
}

fn acme_renew_ticker(interval: std::time::Duration) -> tokio_time::Interval {
    let mut ticker = tokio_time::interval(interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    ticker
}

#[derive(Clone, Debug)]
pub(crate) struct ExternalCertificateTakeoverSnapshot {
    applications: Vec<Value>,
    issued_certificates: Vec<Value>,
    pub(crate) disabled_renewal_count: usize,
}

pub(crate) async fn apply_external_certificate_takeover(
    state: &AppState,
    replaced_certificate_ids: &BTreeSet<String>,
    known_application_ids: &BTreeSet<String>,
) -> anyhow::Result<Option<ExternalCertificateTakeoverSnapshot>> {
    if replaced_certificate_ids.is_empty() && known_application_ids.is_empty() {
        return Ok(None);
    }
    let previous_applications = read_acme_applications_raw(state).await?;
    let previous_issued_certificates = read_issued_certificates(state).await?;
    let mut application_ids = known_application_ids.clone();
    for certificate in &previous_issued_certificates {
        let linked_library_id = certificate
            .get("libraryCertificateId")
            .and_then(Value::as_str);
        if !linked_library_id.is_some_and(|id| replaced_certificate_ids.contains(id)) {
            continue;
        }
        if let Some(application_id) = certificate
            .get("applicationId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            application_ids.insert(application_id.to_string());
        }
    }
    if application_ids.is_empty() {
        return Ok(None);
    }
    let mut applications = previous_applications.clone();
    let mut disabled_renewal_count = 0;
    for application in &mut applications {
        let Some(id) = application.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !application_ids.contains(id) {
            continue;
        }
        if application.get("renewEnabled").and_then(Value::as_bool) != Some(false) {
            disabled_renewal_count += 1;
        }
        application["renewEnabled"] = json!(false);
        application["updatedAt"] = json!(now_node_iso());
    }
    let mut issued_certificates = previous_issued_certificates.clone();
    for certificate in &mut issued_certificates {
        let Some(application_id) = certificate.get("applicationId").and_then(Value::as_str) else {
            continue;
        };
        if !application_ids.contains(application_id) {
            continue;
        }
        if let Some(object) = certificate.as_object_mut() {
            object.remove("libraryCertificateId");
            object.remove("libraryLinkedAt");
        }
    }
    let applications_value = Value::Array(applications);
    let issued_certificates_value = Value::Array(issued_certificates);
    state
        .storage
        .store
        .set_json_values_atomically(&[
            (ACME_APPLICATIONS_KEY, &applications_value),
            (ACME_ISSUED_CERTIFICATES_KEY, &issued_certificates_value),
        ])
        .await?;
    Ok(Some(ExternalCertificateTakeoverSnapshot {
        applications: previous_applications,
        issued_certificates: previous_issued_certificates,
        disabled_renewal_count,
    }))
}

pub(crate) async fn restore_external_certificate_takeover(
    state: &AppState,
    snapshot: &ExternalCertificateTakeoverSnapshot,
) -> anyhow::Result<()> {
    let applications = Value::Array(snapshot.applications.clone());
    let issued_certificates = Value::Array(snapshot.issued_certificates.clone());
    state
        .storage
        .store
        .set_json_values_atomically(&[
            (ACME_APPLICATIONS_KEY, &applications),
            (ACME_ISSUED_CERTIFICATES_KEY, &issued_certificates),
        ])
        .await?;
    Ok(())
}
