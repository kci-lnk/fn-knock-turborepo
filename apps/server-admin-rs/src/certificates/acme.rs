use std::{
    collections::BTreeSet,
    env,
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::Ordering,
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
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    time::{self as tokio_time, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use utoipa_axum::router::OpenApiRouter;
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
static WINDOWS_ACME_ACTIVE_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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

pub fn start_acme_tasks(state: AppState) {
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
