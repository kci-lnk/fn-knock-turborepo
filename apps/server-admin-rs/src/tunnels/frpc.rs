use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{fs, process::Command, sync::Mutex};
use uuid::Uuid;

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    response,
    state::AppState,
    store::Store,
    system_events, time_utils,
    tunnels::supervisor::{SupervisorFailure, SupervisorPhase, SupervisorSnapshot},
};

const FRPC_PRIMARY_INSTANCE_ID: &str = "primary";
const KEY_PREFIX: &str = "fn_knock:frpc:v2";
const INSTANCE_IDS_KEY: &str = "fn_knock:frpc:v2:instance_ids";
const PRIMARY_INSTANCE_ID_KEY: &str = "fn_knock:frpc:v2:primary_instance_id";
const TUNNEL_RUNTIME_KEY: &str = "fn_knock:tunnel:runtime";
const LOG_TTL_SEC: usize = 24 * 3600;
const PRIMARY_LOG_MAX_LEN: usize = 1000;
const EXTRA_LOG_MAX_LEN: usize = 500;
const EXTRA_INSTANCE_LIMIT: usize = 20;
const FRPC_CONNECTED_PATTERNS: &[&str] = &["login to server success", "start proxy success"];
const FRPC_DISCONNECTED_PATTERNS: &[&str] = &[
    "connect to server error",
    "login to the server failed",
    "session shutdown",
];

#[derive(Default)]
struct FrpcConnectionState {
    connected: bool,
    stop_requested: bool,
}

#[derive(Debug)]
struct FrpcHttpError {
    status: StatusCode,
    message: String,
}

impl std::fmt::Display for FrpcHttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

type FrpcResult<T> = Result<T, FrpcHttpError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrpcInstanceMeta {
    id: String,
    name: String,
    is_primary: bool,
    config_path: String,
    work_dir: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrpcInstanceRuntime {
    desired_running: bool,
    pid: Option<u32>,
    started_at: Option<String>,
    stopped_at: Option<String>,
    last_exit_code: Option<i32>,
    last_message: Option<String>,
    supervisor: SupervisorSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrpcInstanceSummary {
    server_addr: String,
    server_port: String,
    local_port: String,
    remote_port: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrpcInstanceStatus {
    id: String,
    name: String,
    is_primary: bool,
    config_path: String,
    work_dir: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
    desired_running: bool,
    running: bool,
    attached: bool,
    pid: Option<u32>,
    started_at: Option<String>,
    stopped_at: Option<String>,
    last_exit_code: Option<i32>,
    last_message: Option<String>,
    supervisor: SupervisorSnapshot,
    summary: FrpcInstanceSummary,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrpcInstancesOverview {
    initialized: bool,
    platform: String,
    primary_instance_id: String,
    total: usize,
    extra_count: usize,
    running_count: usize,
    defaults: Value,
    items: Vec<FrpcInstanceStatus>,
}

#[derive(Deserialize)]
struct ConfigBody {
    content: String,
}

#[derive(Deserialize)]
struct InstanceBody {
    name: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<String>,
}

#[derive(Deserialize)]
struct PollQuery {
    cursor: Option<String>,
}

mod binary;
mod errors;
mod handlers;
mod i18n;
mod parsing;
mod process;
mod runtime;
mod storage;
mod summary;
mod supervisor;

use binary::*;
use errors::*;
use handlers::*;
use i18n::*;
use parsing::*;
use process::*;
use runtime::*;
use storage::*;
use summary::*;
use supervisor::*;

pub fn frpc_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/frpc/status", get(status))
        .route("/api/admin/frpc/overview", get(overview))
        .route("/api/admin/frpc/web-status", get(web_status))
        .route("/api/admin/frpc/config", get(get_config).post(save_config))
        .route("/api/admin/frpc/start", post(start_primary))
        .route("/api/admin/frpc/stop", post(stop_primary))
        .route("/api/admin/frpc/logs", get(get_logs).delete(clear_logs))
        .route("/api/admin/frpc/poll", get(poll_primary))
        .route(
            "/api/admin/frpc/instances",
            get(get_instances).post(create_instance),
        )
        .route("/api/admin/frpc/instances/draft", post(create_draft))
        .route(
            "/api/admin/frpc/instances/{id}",
            get(get_instance)
                .put(update_instance)
                .delete(delete_instance),
        )
        .route("/api/admin/frpc/instances/{id}/start", post(start_instance))
        .route("/api/admin/frpc/instances/{id}/stop", post(stop_instance))
        .route(
            "/api/admin/frpc/instances/{id}/restart",
            post(restart_instance),
        )
        .route(
            "/api/admin/frpc/instances/{id}/logs",
            get(get_instance_logs).delete(clear_instance_logs),
        )
        .route("/api/admin/frpc/instances/{id}/poll", get(poll_instance))
}

pub fn start_frpc_tasks(state: AppState) {
    tokio::spawn(async move {
        if let Err(error) = restore_on_boot(&state).await {
            tracing::warn!(%error, "failed to restore frpc runtime on boot");
        }
    });
}

#[cfg(test)]
mod tests;
