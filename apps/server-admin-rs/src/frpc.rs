use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::LazyLock,
    time::Duration,
};

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::Mutex,
    time::sleep,
};
use uuid::Uuid;

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    redis_store::RedisStore,
    response,
    state::AppState,
    system_events, time_utils,
};

const FRPC_PRIMARY_INSTANCE_ID: &str = "primary";
const FRPC_VERSION: &str = "0.67.0";
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

static ATTACHED_PIDS: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CONNECTION_STATES: LazyLock<Mutex<HashMap<String, FrpcConnectionState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

async fn status(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let overview = build_overview(&state).await?;
            let primary = overview
                .items
                .iter()
                .find(|item| item.id == overview.primary_instance_id);
            Ok(json!({
                "initialized": overview.initialized,
                "platform": overview.platform,
                "running": primary.map(|item| item.running).unwrap_or(false),
                "pid": primary.and_then(|item| item.pid),
                "config_path": primary.map(|item| item.config_path.clone()).unwrap_or_default(),
                "defaults": overview.defaults,
                "total": overview.total,
                "running_count": overview.running_count,
            }))
        }
        .await,
    )
    .await
}

async fn overview(State(state): State<AppState>, Query(query): Query<LimitQuery>) -> Response {
    frpc_response(
        &state,
        async {
            let logs = list_logs_inner(
                &state,
                FRPC_PRIMARY_INSTANCE_ID,
                parse_limit(query.limit.as_deref()),
            )
            .await?;
            Ok(json!({ "tcp": [], "logs": logs }))
        }
        .await,
    )
    .await
}

async fn web_status() -> Response {
    response::ok(json!({ "tcp": [] })).into_response()
}

async fn get_config(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let content = read_config(&state, FRPC_PRIMARY_INSTANCE_ID).await?;
            Ok(json!({ "content": content }))
        }
        .await,
    )
    .await
}

async fn save_config(State(state): State<AppState>, Json(body): Json<ConfigBody>) -> Response {
    frpc_response_empty(
        &state,
        save_config_inner(&state, FRPC_PRIMARY_INSTANCE_ID, body.content).await,
    )
    .await
}

async fn start_primary(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let pid = start_instance_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

async fn stop_primary(State(state): State<AppState>) -> Response {
    frpc_response_empty(
        &state,
        stop_instance_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await,
    )
    .await
}

async fn get_logs(State(state): State<AppState>, Query(query): Query<LimitQuery>) -> Response {
    frpc_response(
        &state,
        async {
            Ok(json!(
                list_logs_inner(
                    &state,
                    FRPC_PRIMARY_INSTANCE_ID,
                    parse_limit(query.limit.as_deref()),
                )
                .await?
            ))
        }
        .await,
    )
    .await
}

async fn clear_logs(State(state): State<AppState>) -> Response {
    frpc_response_empty(
        &state,
        clear_logs_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await,
    )
    .await
}

async fn poll_primary(State(state): State<AppState>, Query(query): Query<PollQuery>) -> Response {
    frpc_response(
        &state,
        async {
            let mut data =
                poll_inner(&state, FRPC_PRIMARY_INSTANCE_ID, query.cursor.as_deref()).await?;
            let overview = build_overview(&state).await?;
            if let Some(status) = data.get_mut("status").and_then(Value::as_object_mut) {
                status.insert("tcp".to_string(), json!([]));
                status.insert("instances".to_string(), serde_json::to_value(overview)?);
            }
            Ok(data)
        }
        .await,
    )
    .await
}

async fn get_instances(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async { Ok(serde_json::to_value(build_overview(&state).await?)?) }.await,
    )
    .await
}

async fn create_draft(State(state): State<AppState>) -> Response {
    let _ = state;
    response::ok(json!({ "content": default_frpc_template() })).into_response()
}

async fn create_instance(
    State(state): State<AppState>,
    Json(body): Json<InstanceBody>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(serde_json::to_value(
                create_instance_inner(&state, body).await?,
            )?)
        }
        .await,
    )
    .await
}

async fn get_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            let meta = get_meta_or_error(&state, &id).await?;
            let item = build_status(&state, &meta).await?;
            let content = read_config_for_meta(&meta).await?;
            let logs =
                list_logs_inner(&state, &meta.id, parse_limit(query.limit.as_deref())).await?;
            Ok(json!({ "item": item, "content": content, "logs": logs }))
        }
        .await,
    )
    .await
}

async fn update_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<InstanceBody>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(serde_json::to_value(
                update_instance_inner(&state, &id, body).await?,
            )?)
        }
        .await,
    )
    .await
}

async fn delete_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response_empty(&state, delete_instance_inner(&state, &id).await).await
}

async fn start_instance(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    frpc_response(
        &state,
        async {
            let pid = start_instance_inner(&state, &id).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

async fn stop_instance(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    frpc_response_empty(&state, stop_instance_inner(&state, &id).await).await
}

async fn restart_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response(
        &state,
        async {
            stop_instance_inner(&state, &id).await?;
            let pid = start_instance_inner(&state, &id).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

async fn get_instance_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(json!(
                list_logs_inner(&state, &id, parse_limit(query.limit.as_deref())).await?
            ))
        }
        .await,
    )
    .await
}

async fn clear_instance_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response_empty(&state, clear_logs_inner(&state, &id).await).await
}

async fn poll_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<PollQuery>,
) -> Response {
    frpc_response(
        &state,
        poll_inner(&state, &id, query.cursor.as_deref()).await,
    )
    .await
}

async fn frpc_response(state: &AppState, result: FrpcResult<Value>) -> Response {
    let translator = Translator::from_state(state).await;
    match result {
        Ok(value) => response::ok(localize_frpc_response_value(value, &translator)).into_response(),
        Err(error) => response::error(
            error.status,
            localize_frpc_error(&translator, &error.message),
        ),
    }
}

async fn frpc_response_empty(state: &AppState, result: FrpcResult<()>) -> Response {
    let translator = Translator::from_state(state).await;
    match result {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => response::error(
            error.status,
            localize_frpc_error(&translator, &error.message),
        ),
    }
}

fn frpc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.frpc.{key}"))
}

fn frpc_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.frpc.{key}"), params)
}

fn default_frpc_text(key: &str) -> String {
    frpc_text(&Translator::new(DEFAULT_LOCALE), key)
}

fn default_frpc_primary_name() -> String {
    default_frpc_text("primaryName")
}

fn default_frpc_instance_name() -> String {
    default_frpc_text("instanceName")
}

fn localize_frpc_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    if let Some(id) = message.strip_prefix("FRPC instance not found: ") {
        return frpc_text_params(translator, "instanceNotFound", &[("id", id.to_string())]);
    }
    if let Some(limit) = message
        .strip_prefix("FRPC instance limit exceeded (")
        .and_then(|value| value.strip_suffix(')'))
    {
        return frpc_text_params(
            translator,
            "instanceLimitExceeded",
            &[("limit", limit.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("Failed to verify frpc config: ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }
    if let Some(code) = message.strip_prefix("frpc config verify failed with code ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithCode",
            &[("code", code.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("frpc config verify failed: ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("Failed to start frpc: ") {
        return frpc_text_params(
            translator,
            "startFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }

    match message {
        "Primary FRPC instance cannot be deleted" => frpc_text(translator, "primaryDeleteDenied"),
        "FRP is not initialized" => frpc_text(translator, "notInitialized"),
        "Failed to read frpc pid" => frpc_text(translator, "pidReadFailed"),
        _ => message.to_string(),
    }
}

fn localize_frpc_response_value(mut value: Value, translator: &Translator) -> Value {
    localize_frpc_value_in_place(&mut value, translator);
    value
}

fn localize_frpc_value_in_place(value: &mut Value, translator: &Translator) {
    match value {
        Value::Object(object) => {
            for key in ["lastMessage", "last_message"] {
                if let Some(message) = object.get(key).and_then(Value::as_str) {
                    object.insert(
                        key.to_string(),
                        Value::String(localize_frpc_runtime_message(translator, message)),
                    );
                }
            }
            for child in object.values_mut() {
                localize_frpc_value_in_place(child, translator);
            }
        }
        Value::Array(items) => {
            for item in items {
                localize_frpc_value_in_place(item, translator);
            }
        }
        _ => {}
    }
}

fn localize_frpc_runtime_message(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    if let Some(pid) = message.strip_prefix("frpc started pid=") {
        return frpc_text_params(translator, "startedWithPid", &[("pid", pid.to_string())]);
    }
    if let Some(pid) = message.strip_prefix("frpc stopped pid=") {
        return frpc_text_params(translator, "stoppedWithPid", &[("pid", pid.to_string())]);
    }
    if let Some(code) = message.strip_prefix("frpc exited with code ") {
        return frpc_text_params(
            translator,
            "processExitedWithCode",
            &[("code", code.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("frpc process error: ") {
        return frpc_text_params(
            translator,
            "processCrashed",
            &[("message", detail.to_string())],
        );
    }
    match message {
        "frpc pid is no longer running" => frpc_text(translator, "pidInvalidForInstance"),
        "frpc already stopped" => frpc_text(translator, "alreadyStopped"),
        _ => message.to_string(),
    }
}

fn frpc_error(status: StatusCode, message: impl Into<String>) -> FrpcHttpError {
    FrpcHttpError {
        status,
        message: message.into(),
    }
}

fn frpc_internal(error: impl std::fmt::Display) -> FrpcHttpError {
    frpc_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn frpc_validation(message: impl Into<String>) -> FrpcHttpError {
    frpc_error(StatusCode::BAD_REQUEST, message)
}

fn frpc_not_found(id: &str) -> FrpcHttpError {
    frpc_error(
        StatusCode::NOT_FOUND,
        format!("FRPC instance not found: {id}"),
    )
}

impl From<anyhow::Error> for FrpcHttpError {
    fn from(value: anyhow::Error) -> Self {
        frpc_internal(value)
    }
}

impl From<std::io::Error> for FrpcHttpError {
    fn from(value: std::io::Error) -> Self {
        frpc_internal(value)
    }
}

impl From<redis::RedisError> for FrpcHttpError {
    fn from(value: redis::RedisError) -> Self {
        frpc_internal(value)
    }
}

impl From<serde_json::Error> for FrpcHttpError {
    fn from(value: serde_json::Error) -> Self {
        frpc_internal(value)
    }
}

fn parse_limit(value: Option<&str>) -> usize {
    let parsed = value.and_then(parse_node_parse_int).unwrap_or(200);
    parsed.clamp(1, 1000) as usize
}

fn parse_node_parse_int(value: &str) -> Option<i64> {
    let trimmed = value.trim_start();
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let parsed = digits.parse::<i64>().ok()?;
    Some(if negative { -parsed } else { parsed })
}

fn frpc_dir(state: &AppState) -> PathBuf {
    state.settings.data_dir.join("frp")
}

fn frpc_instances_dir(state: &AppState) -> PathBuf {
    frpc_dir(state).join("instances")
}

fn primary_config_path(state: &AppState) -> PathBuf {
    frpc_dir(state).join("frpc.toml")
}

fn extra_instance_paths(state: &AppState, id: &str) -> (PathBuf, PathBuf, PathBuf) {
    let work_dir = frpc_instances_dir(state).join(id);
    let config_path = work_dir.join("frpc.toml");
    let pid_path = work_dir.join("frpc.pid");
    (work_dir, config_path, pid_path)
}

async fn ensure_layout(state: &AppState) -> anyhow::Result<()> {
    fs::create_dir_all(frpc_dir(state)).await?;
    fs::create_dir_all(frpc_instances_dir(state)).await?;
    Ok(())
}

async fn ensure_primary_instance(state: &AppState) -> anyhow::Result<()> {
    ensure_layout(state).await?;
    let mut ids = read_instance_ids(&state.redis).await?;
    if !ids.iter().any(|id| id == FRPC_PRIMARY_INSTANCE_ID) {
        ids.insert(0, FRPC_PRIMARY_INSTANCE_ID.to_string());
        write_instance_ids(&state.redis, &ids).await?;
    }
    if read_meta(&state.redis, state, FRPC_PRIMARY_INSTANCE_ID)
        .await?
        .is_none()
    {
        write_meta(&state.redis, &primary_meta(state)).await?;
    }
    let config_path = primary_config_path(state);
    if !config_path.exists() {
        fs::write(config_path, default_frpc_template()).await?;
    }
    Ok(())
}

fn default_frpc_template() -> String {
    let local_port = std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string());
    [
        "serverAddr = \"\"".to_string(),
        "serverPort = 7000".to_string(),
        "".to_string(),
        "[auth]".to_string(),
        "method = \"token\"".to_string(),
        "token = \"\"".to_string(),
        "".to_string(),
        "[[proxies]]".to_string(),
        "name = \"reproxy\"".to_string(),
        "type = \"tcp\"".to_string(),
        "localIP = \"127.0.0.1\"".to_string(),
        format!("localPort = {local_port}"),
        "remotePort = 7999".to_string(),
        "transport.proxyProtocolVersion = \"v2\"".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

fn primary_meta(state: &AppState) -> FrpcInstanceMeta {
    let now = time_utils::now_iso();
    FrpcInstanceMeta {
        id: FRPC_PRIMARY_INSTANCE_ID.to_string(),
        name: default_frpc_primary_name(),
        is_primary: true,
        config_path: primary_config_path(state).to_string_lossy().to_string(),
        work_dir: frpc_dir(state).to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: 0,
    }
}

fn fallback_meta(state: &AppState, id: &str) -> FrpcInstanceMeta {
    if id == FRPC_PRIMARY_INSTANCE_ID {
        return primary_meta(state);
    }
    let now = time_utils::now_iso();
    let (work_dir, config_path, _) = extra_instance_paths(state, id);
    FrpcInstanceMeta {
        id: id.to_string(),
        name: default_frpc_instance_name(),
        is_primary: false,
        config_path: config_path.to_string_lossy().to_string(),
        work_dir: work_dir.to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: 1000,
    }
}

fn default_runtime() -> FrpcInstanceRuntime {
    FrpcInstanceRuntime {
        desired_running: false,
        pid: None,
        started_at: None,
        stopped_at: None,
        last_exit_code: None,
        last_message: None,
    }
}

fn instance_key(id: &str, part: &str) -> String {
    format!("{KEY_PREFIX}:instance:{id}:{part}")
}

fn log_key(id: &str) -> String {
    format!("{KEY_PREFIX}:instance:{id}:logs")
}

async fn read_instance_ids(redis: &RedisStore) -> anyhow::Result<Vec<String>> {
    let raw = redis.get_string_value(INSTANCE_IDS_KEY).await?;
    let parsed = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let mut seen = Vec::new();
    for value in parsed {
        let Some(id) = value.as_str().and_then(sanitize_instance_id) else {
            continue;
        };
        if !seen.iter().any(|existing| existing == &id) {
            seen.push(id);
        }
    }
    Ok(seen)
}

async fn write_instance_ids(redis: &RedisStore, ids: &[String]) -> anyhow::Result<()> {
    let mut unique = Vec::new();
    for id in ids {
        if !unique.iter().any(|existing| existing == id) {
            unique.push(id.clone());
        }
    }
    redis
        .set_string_value(INSTANCE_IDS_KEY, &serde_json::to_string(&unique)?)
        .await?;
    redis
        .set_string_value(PRIMARY_INSTANCE_ID_KEY, FRPC_PRIMARY_INSTANCE_ID)
        .await?;
    Ok(())
}

async fn read_meta(
    redis: &RedisStore,
    state: &AppState,
    id: &str,
) -> anyhow::Result<Option<FrpcInstanceMeta>> {
    let Some(value) = redis.get_json_value(&instance_key(id, "meta")).await? else {
        return Ok(None);
    };
    let fallback = fallback_meta(state, id);
    Ok(Some(normalize_meta(value, fallback)))
}

async fn write_meta(redis: &RedisStore, meta: &FrpcInstanceMeta) -> anyhow::Result<()> {
    redis
        .set_json_value(
            &instance_key(&meta.id, "meta"),
            &serde_json::to_value(meta)?,
        )
        .await?;
    Ok(())
}

async fn read_runtime(redis: &RedisStore, id: &str) -> anyhow::Result<FrpcInstanceRuntime> {
    let raw = redis.get_json_value(&instance_key(id, "runtime")).await?;
    Ok(raw.map(normalize_runtime).unwrap_or_else(default_runtime))
}

async fn write_runtime(
    redis: &RedisStore,
    id: &str,
    runtime: &FrpcInstanceRuntime,
) -> anyhow::Result<()> {
    redis
        .set_json_value(
            &instance_key(id, "runtime"),
            &serde_json::to_value(runtime)?,
        )
        .await?;
    Ok(())
}

fn normalize_meta(value: Value, fallback: FrpcInstanceMeta) -> FrpcInstanceMeta {
    FrpcInstanceMeta {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.id),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.name),
        is_primary: value
            .get("isPrimary")
            .and_then(Value::as_bool)
            .unwrap_or(fallback.is_primary),
        config_path: value
            .get("configPath")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.config_path),
        work_dir: value
            .get("workDir")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.work_dir),
        created_at: value
            .get("createdAt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.created_at),
        updated_at: value
            .get("updatedAt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.updated_at),
        sort_order: value
            .get("sortOrder")
            .and_then(Value::as_i64)
            .unwrap_or(fallback.sort_order),
    }
}

fn normalize_runtime(value: Value) -> FrpcInstanceRuntime {
    FrpcInstanceRuntime {
        desired_running: value
            .get("desiredRunning")
            .or_else(|| value.get("desired_running"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        pid: value
            .get("pid")
            .and_then(Value::as_u64)
            .filter(|pid| *pid > 0)
            .and_then(|pid| u32::try_from(pid).ok()),
        started_at: value
            .get("startedAt")
            .or_else(|| value.get("started_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
        stopped_at: value
            .get("stoppedAt")
            .or_else(|| value.get("stopped_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
        last_exit_code: value
            .get("lastExitCode")
            .or_else(|| value.get("last_exit_code"))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        last_message: value
            .get("lastMessage")
            .or_else(|| value.get("last_message"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

async fn all_metas(state: &AppState) -> FrpcResult<Vec<FrpcInstanceMeta>> {
    ensure_primary_instance(state).await?;
    let ids = read_instance_ids(&state.redis).await?;
    let mut metas = Vec::new();
    for id in ids {
        if let Some(meta) = read_meta(&state.redis, state, &id).await? {
            metas.push(meta);
        }
    }
    metas.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.created_at.cmp(&right.created_at))
    });
    Ok(metas)
}

async fn get_meta_or_error(state: &AppState, id: &str) -> FrpcResult<FrpcInstanceMeta> {
    let Some(safe_id) = sanitize_instance_id(id) else {
        return Err(frpc_not_found(id));
    };
    ensure_primary_instance(state).await?;
    read_meta(&state.redis, state, &safe_id)
        .await?
        .ok_or_else(|| frpc_not_found(id))
}

fn sanitize_instance_id(id: &str) -> Option<String> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 80
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }
    Some(trimmed.to_string())
}

async fn read_config(state: &AppState, id: &str) -> FrpcResult<String> {
    let meta = get_meta_or_error(state, id).await?;
    Ok(read_config_for_meta(&meta).await?)
}

async fn read_config_for_meta(meta: &FrpcInstanceMeta) -> anyhow::Result<String> {
    fs::create_dir_all(&meta.work_dir).await?;
    let config_path = PathBuf::from(&meta.config_path);
    if fs::metadata(&config_path).await.is_err() {
        let content = default_frpc_template_for_port();
        fs::write(&config_path, &content).await?;
        return Ok(content);
    }
    Ok(fs::read_to_string(config_path).await?)
}

fn default_frpc_template_for_port() -> String {
    let local_port = std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string());
    [
        "serverAddr = \"\"".to_string(),
        "serverPort = 7000".to_string(),
        "".to_string(),
        "[auth]".to_string(),
        "method = \"token\"".to_string(),
        "token = \"\"".to_string(),
        "".to_string(),
        "[[proxies]]".to_string(),
        "name = \"reproxy\"".to_string(),
        "type = \"tcp\"".to_string(),
        "localIP = \"127.0.0.1\"".to_string(),
        format!("localPort = {local_port}"),
        "remotePort = 7999".to_string(),
        "transport.proxyProtocolVersion = \"v2\"".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

async fn write_config_for_meta(meta: &FrpcInstanceMeta, content: &str) -> anyhow::Result<()> {
    fs::create_dir_all(&meta.work_dir).await?;
    fs::write(&meta.config_path, content).await?;
    Ok(())
}

async fn save_config_inner(state: &AppState, id: &str, content: String) -> FrpcResult<()> {
    let mut meta = get_meta_or_error(state, id).await?;
    verify_frpc_config(state, &meta, &content).await?;
    write_config_for_meta(&meta, &content).await?;
    meta.updated_at = time_utils::now_iso();
    write_meta(&state.redis, &meta).await?;
    Ok(())
}

async fn update_instance_inner(
    state: &AppState,
    id: &str,
    body: InstanceBody,
) -> FrpcResult<FrpcInstanceStatus> {
    let mut meta = get_meta_or_error(state, id).await?;
    if let Some(name) = body.name {
        let name = name.trim();
        meta.name = if name.is_empty() {
            if meta.is_primary {
                default_frpc_primary_name()
            } else {
                default_frpc_instance_name()
            }
        } else {
            name.to_string()
        };
    }
    if let Some(content) = body.content {
        verify_frpc_config(state, &meta, &content).await?;
        write_config_for_meta(&meta, &content).await?;
    }
    meta.updated_at = time_utils::now_iso();
    write_meta(&state.redis, &meta).await?;
    build_status(state, &meta).await
}

async fn create_instance_inner(
    state: &AppState,
    body: InstanceBody,
) -> FrpcResult<FrpcInstanceStatus> {
    ensure_primary_instance(state).await?;
    let metas = all_metas(state).await?;
    if metas.iter().filter(|meta| !meta.is_primary).count() >= EXTRA_INSTANCE_LIMIT {
        return Err(frpc_validation(format!(
            "FRPC instance limit exceeded ({EXTRA_INSTANCE_LIMIT})"
        )));
    }
    let id = Uuid::new_v4().to_string();
    let (work_dir, config_path, _) = extra_instance_paths(state, &id);
    let now = time_utils::now_iso();
    let meta = FrpcInstanceMeta {
        id: id.clone(),
        name: body
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(default_frpc_instance_name),
        is_primary: false,
        config_path: config_path.to_string_lossy().to_string(),
        work_dir: work_dir.to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: metas.iter().map(|meta| meta.sort_order).max().unwrap_or(0) + 1,
    };
    let content = body.content.unwrap_or_else(default_frpc_template_for_port);
    let result = async {
        verify_frpc_config(state, &meta, &content).await?;
        fs::create_dir_all(&meta.work_dir).await?;
        write_config_for_meta(&meta, &content).await?;
        write_meta(&state.redis, &meta).await?;
        write_runtime(&state.redis, &meta.id, &default_runtime()).await?;
        let mut ids = metas.iter().map(|meta| meta.id.clone()).collect::<Vec<_>>();
        ids.push(meta.id.clone());
        write_instance_ids(&state.redis, &ids).await?;
        append_logs(state, &meta, &["frpc instance created".to_string()]).await?;
        build_status(state, &meta).await
    }
    .await;
    if result.is_err() {
        cleanup_created_instance(state, &meta, &metas).await;
    }
    result
}

async fn delete_instance_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    if meta.is_primary {
        return Err(frpc_validation("Primary FRPC instance cannot be deleted"));
    }
    let status = build_status(state, &meta).await?;
    if status.running {
        stop_instance_inner(state, &meta.id).await?;
    }
    state
        .redis
        .delete_keys(&[
            instance_key(&meta.id, "meta"),
            instance_key(&meta.id, "runtime"),
            log_key(&meta.id),
            format!("{}:seq", log_key(&meta.id)),
        ])
        .await?;
    let ids = read_instance_ids(&state.redis).await?;
    write_instance_ids(
        &state.redis,
        &ids.into_iter()
            .filter(|item| item != &meta.id)
            .collect::<Vec<_>>(),
    )
    .await?;
    let _ = fs::remove_dir_all(&meta.work_dir).await;
    ATTACHED_PIDS.lock().await.remove(&meta.id);
    Ok(())
}

async fn cleanup_created_instance(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    previous_metas: &[FrpcInstanceMeta],
) {
    let _ = state
        .redis
        .delete_keys(&[
            instance_key(&meta.id, "meta"),
            instance_key(&meta.id, "runtime"),
            log_key(&meta.id),
            format!("{}:seq", log_key(&meta.id)),
        ])
        .await;
    let ids = previous_metas
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let _ = write_instance_ids(&state.redis, &ids).await;
    let _ = fs::remove_dir_all(&meta.work_dir).await;
    ATTACHED_PIDS.lock().await.remove(&meta.id);
}

async fn build_overview(state: &AppState) -> FrpcResult<FrpcInstancesOverview> {
    let metas = all_metas(state).await?;
    let mut items = Vec::new();
    for meta in metas {
        items.push(build_status(state, &meta).await?);
    }
    Ok(FrpcInstancesOverview {
        initialized: frp_executable(state).is_some(),
        platform: detect_frp_platform().to_string(),
        primary_instance_id: FRPC_PRIMARY_INSTANCE_ID.to_string(),
        total: items.len(),
        extra_count: items.iter().filter(|item| !item.is_primary).count(),
        running_count: items.iter().filter(|item| item.running).count(),
        defaults: json!({ "local_port": std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string()) }),
        items,
    })
}

async fn build_status(state: &AppState, meta: &FrpcInstanceMeta) -> FrpcResult<FrpcInstanceStatus> {
    let runtime = reconcile_runtime(state, meta).await?;
    let content = read_config_for_meta(meta).await?;
    Ok(FrpcInstanceStatus {
        id: meta.id.clone(),
        name: meta.name.clone(),
        is_primary: meta.is_primary,
        config_path: meta.config_path.clone(),
        work_dir: meta.work_dir.clone(),
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        sort_order: meta.sort_order,
        desired_running: runtime.0.desired_running,
        running: runtime.1,
        attached: runtime.2,
        pid: runtime.0.pid,
        started_at: runtime.0.started_at,
        stopped_at: runtime.0.stopped_at,
        last_exit_code: runtime.0.last_exit_code,
        last_message: runtime.0.last_message,
        summary: build_summary(&content),
    })
}

async fn reconcile_runtime(
    state: &AppState,
    meta: &FrpcInstanceMeta,
) -> FrpcResult<(FrpcInstanceRuntime, bool, bool)> {
    let runtime = read_runtime(&state.redis, &meta.id).await?;
    let original_runtime = runtime.clone();
    let pid = read_candidate_pid(meta, &runtime).await;
    let attached = if let Some(pid) = pid {
        ATTACHED_PIDS.lock().await.get(&meta.id).copied() == Some(pid)
    } else {
        false
    };
    if let Some(pid) = pid {
        let next = merge_detected_frpc_runtime(runtime, pid);
        if should_persist_detected_runtime(&original_runtime, &next) {
            write_runtime(&state.redis, &meta.id, &next).await?;
        }
        write_pid_file(&pid_path_for_meta(meta), pid).await;
        return Ok((next, true, attached));
    }
    let had_pid = runtime.pid.is_some() || read_pid_file(&pid_path_for_meta(meta)).await.is_some();
    remove_pid_file(&pid_path_for_meta(meta)).await;
    if runtime.pid.is_some() || had_pid {
        let mut next = runtime;
        next.pid = None;
        if next.stopped_at.is_none() {
            next.stopped_at = Some(time_utils::now_iso());
        }
        if next.last_message.is_none() {
            next.last_message = Some("frpc pid is no longer running".to_string());
        }
        write_runtime(&state.redis, &meta.id, &next).await?;
        return Ok((next, false, false));
    }
    Ok((runtime, false, false))
}

fn merge_detected_frpc_runtime(mut runtime: FrpcInstanceRuntime, pid: u32) -> FrpcInstanceRuntime {
    let preserve_message = runtime.pid == Some(pid)
        && runtime.stopped_at.is_none()
        && runtime.last_exit_code.is_none()
        && runtime.last_message.is_some();
    runtime.pid = Some(pid);
    if runtime.started_at.is_none() {
        runtime.started_at = Some(time_utils::now_iso());
    }
    runtime.stopped_at = None;
    runtime.last_exit_code = None;
    if !preserve_message {
        runtime.last_message = Some(format!("frpc process detected pid={pid}"));
    }
    runtime
}

fn should_persist_detected_runtime(
    left: &FrpcInstanceRuntime,
    right: &FrpcInstanceRuntime,
) -> bool {
    left.pid != right.pid
        || left.started_at != right.started_at
        || left.stopped_at != right.stopped_at
        || left.last_exit_code != right.last_exit_code
        || left.last_message != right.last_message
}

async fn read_candidate_pid(meta: &FrpcInstanceMeta, runtime: &FrpcInstanceRuntime) -> Option<u32> {
    if let Some(pid) = ATTACHED_PIDS.lock().await.get(&meta.id).copied() {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    if let Some(pid) = runtime.pid {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    if let Some(pid) = read_pid_file(&pid_path_for_meta(meta)).await {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    find_frpc_pid_by_config_path(&meta.config_path).await
}

async fn verify_frpc_config(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    content: &str,
) -> FrpcResult<()> {
    let Some(bin) = frp_executable(state) else {
        return Err(frpc_validation("FRP is not initialized"));
    };
    fs::create_dir_all(&meta.work_dir).await?;
    let temp = PathBuf::from(&meta.work_dir).join(format!("frpc.verify.{}.toml", Uuid::new_v4()));
    fs::write(&temp, content).await?;
    let output = Command::new(&bin)
        .arg("verify")
        .arg("-c")
        .arg(&temp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;
    let _ = fs::remove_file(&temp).await;
    let output = output
        .map_err(|error| frpc_validation(format!("Failed to verify frpc config: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = normalize_verify_output(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    ));
    if detail.is_empty() {
        Err(frpc_validation(format!(
            "frpc config verify failed with code {}",
            output.status.code().unwrap_or(-1)
        )))
    } else {
        Err(frpc_validation(format!(
            "frpc config verify failed: {detail}"
        )))
    }
}

fn normalize_verify_output(value: &str) -> String {
    let normalized = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.len() <= 4000 {
        normalized
    } else {
        format!("{}...", &normalized[..4000])
    }
}

async fn start_instance_inner(state: &AppState, id: &str) -> FrpcResult<u32> {
    let meta = get_meta_or_error(state, id).await?;
    let Some(bin) = frp_executable(state) else {
        return Err(frpc_validation("FRP is not initialized"));
    };
    let content = read_config_for_meta(&meta).await?;
    verify_frpc_config(state, &meta, &content).await?;
    let current = build_status(state, &meta).await?;
    if current.running {
        if let Some(pid) = current.pid {
            let mut runtime = read_runtime(&state.redis, &meta.id).await?;
            runtime.desired_running = true;
            runtime.pid = Some(pid);
            write_runtime(&state.redis, &meta.id, &runtime).await?;
            return Ok(pid);
        }
    }
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        connection.stop_requested = false;
    }
    let mut child = Command::new(bin)
        .arg("-c")
        .arg(&meta.config_path)
        .current_dir(&meta.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| frpc_internal(format!("Failed to start frpc: {error}")))?;
    let pid = child
        .id()
        .ok_or_else(|| frpc_internal("Failed to read frpc pid"))?;
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(state.clone(), meta.clone(), stdout, false);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(state.clone(), meta.clone(), stderr, true);
    }
    ATTACHED_PIDS.lock().await.insert(meta.id.clone(), pid);
    write_pid_file(&pid_path_for_meta(&meta), pid).await;
    write_runtime(
        &state.redis,
        &meta.id,
        &FrpcInstanceRuntime {
            desired_running: true,
            pid: Some(pid),
            started_at: Some(time_utils::now_iso()),
            stopped_at: None,
            last_exit_code: None,
            last_message: Some(format!("frpc started pid={pid}")),
        },
    )
    .await?;
    append_logs(state, &meta, &[format!("frpc started pid={pid}")]).await?;
    mark_tunnel_running(state).await;
    spawn_exit_watcher(state.clone(), meta.clone(), child);
    Ok(pid)
}

fn spawn_log_reader<R>(state: AppState, meta: FrpcInstanceMeta, reader: R, stderr: bool)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = if stderr {
                format!("[ERR] {line}")
            } else {
                line
            };
            if let Err(error) = append_logs(&state, &meta, &[line]).await {
                tracing::warn!(instance_id = %meta.id, %error, "failed to append frpc process log");
            }
        }
    });
}

fn spawn_exit_watcher(state: AppState, meta: FrpcInstanceMeta, mut child: tokio::process::Child) {
    tokio::spawn(async move {
        let pid = child.id();
        let status = child.wait().await;
        let code = status
            .as_ref()
            .ok()
            .and_then(|status| status.code())
            .unwrap_or(-1);
        let was_attached = ATTACHED_PIDS.lock().await.remove(&meta.id).is_some();
        let expected_stop = {
            let states = CONNECTION_STATES.lock().await;
            states
                .get(&meta.id)
                .map(|state| state.stop_requested)
                .unwrap_or(false)
                || !was_attached
        };
        remove_pid_file(&pid_path_for_meta(&meta)).await;
        let message = match status {
            Ok(_) => format!("frpc exited with code {code}"),
            Err(error) => format!("frpc process error: {error}"),
        };
        let mut runtime = read_runtime(&state.redis, &meta.id)
            .await
            .unwrap_or_else(|_| default_runtime());
        runtime.pid = None;
        runtime.stopped_at = Some(time_utils::now_iso());
        runtime.last_exit_code = Some(code);
        runtime.last_message = Some(message.clone());
        let _ = write_runtime(&state.redis, &meta.id, &runtime).await;
        let _ = append_logs(&state, &meta, &[message]).await;
        if !expected_stop {
            let exit_message = runtime.last_message.as_deref();
            emit_frpc_connectivity(&state, &meta, false, exit_message, pid).await;
        }
        if let Some(connection) = CONNECTION_STATES.lock().await.get_mut(&meta.id) {
            connection.stop_requested = false;
        }
        let _ = update_aggregate_tunnel_state(&state).await;
    });
}

async fn stop_instance_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    let status = build_status(state, &meta).await?;
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        connection.stop_requested = true;
        connection.connected = false;
    }
    if let Some(pid) = status.pid {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            terminate_pid(pid).await?;
        }
    }
    ATTACHED_PIDS.lock().await.remove(&meta.id);
    remove_pid_file(&pid_path_for_meta(&meta)).await;
    let mut runtime = read_runtime(&state.redis, &meta.id).await?;
    runtime.desired_running = false;
    runtime.pid = None;
    runtime.stopped_at = Some(time_utils::now_iso());
    runtime.last_message = Some(
        status
            .pid
            .map(|pid| format!("frpc stopped pid={pid}"))
            .unwrap_or_else(|| "frpc already stopped".to_string()),
    );
    write_runtime(&state.redis, &meta.id, &runtime).await?;
    if let Some(pid) = status.pid {
        append_logs(state, &meta, &[format!("frpc stopped pid={pid}")]).await?;
    }
    if let Some(connection) = CONNECTION_STATES.lock().await.get_mut(&meta.id) {
        connection.stop_requested = false;
    }
    update_aggregate_tunnel_state(state).await?;
    Ok(())
}

async fn list_logs_inner(state: &AppState, id: &str, limit: usize) -> FrpcResult<Vec<String>> {
    let meta = get_meta_or_error(state, id).await?;
    Ok(state
        .redis
        .list_log_buffer(&log_key(&meta.id), limit, log_max_len(&meta.id))
        .await?)
}

async fn clear_logs_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    state.redis.clear_log_buffer(&log_key(&meta.id)).await?;
    Ok(())
}

async fn poll_inner(state: &AppState, id: &str, cursor: Option<&str>) -> FrpcResult<Value> {
    let meta = get_meta_or_error(state, id).await?;
    let logs = state
        .redis
        .poll_log_buffer(&log_key(&meta.id), cursor)
        .await?;
    let status = build_status(state, &meta).await?;
    Ok(json!({
        "cursor": logs.get("cursor").cloned().unwrap_or(json!(0)),
        "reset": logs.get("reset").cloned().unwrap_or(json!(false)),
        "logs": logs.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter_map(|item| item.as_str().map(str::to_string)).collect::<Vec<_>>(),
        "status": status
    }))
}

async fn append_logs(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    lines: &[String],
) -> anyhow::Result<()> {
    let normalized = lines
        .iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    state
        .redis
        .append_log_buffer(
            &log_key(&meta.id),
            &normalized,
            LOG_TTL_SEC,
            log_max_len(&meta.id),
        )
        .await?;
    handle_frpc_runtime_signals(state, meta, &normalized).await;
    Ok(())
}

async fn handle_frpc_runtime_signals(state: &AppState, meta: &FrpcInstanceMeta, lines: &[String]) {
    for line in lines {
        let Some(message) = normalize_frpc_tunnel_event_message(line) else {
            continue;
        };
        let normalized = message.to_ascii_lowercase();
        let pid = ATTACHED_PIDS.lock().await.get(&meta.id).copied();
        if FRPC_CONNECTED_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
        {
            emit_frpc_connectivity(state, meta, true, Some(&message), pid).await;
            continue;
        }
        if FRPC_DISCONNECTED_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
        {
            emit_frpc_connectivity(state, meta, false, Some(&message), pid).await;
        }
    }
}

async fn emit_frpc_connectivity(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    connected: bool,
    message: Option<&str>,
    pid: Option<u32>,
) {
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        if connected {
            if connection.connected {
                return;
            }
            connection.connected = true;
        } else {
            if !connection.connected {
                return;
            }
            connection.connected = false;
            if connection.stop_requested {
                return;
            }
        }
    }

    let event_message = message.map(|value| format!("{}: {value}", meta.name));
    if let Err(error) = system_events::publish_tunnel_connectivity_event(
        state,
        "frp",
        connected,
        pid,
        event_message.as_deref(),
        Some(&meta.id),
        Some(&meta.name),
        Some(meta.is_primary),
    )
    .await
    {
        tracing::warn!(instance_id = %meta.id, %error, "failed to publish frpc connectivity event");
    }
}

fn normalize_frpc_tunnel_event_message(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let line = if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("[ERR]"))
    {
        trimmed[5..].trim_start()
    } else {
        line
    };
    normalize_tunnel_event_message(line)
}

fn normalize_tunnel_event_message(line: &str) -> Option<String> {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= 240 {
        return Some(normalized);
    }
    let truncated = normalized.chars().take(240).collect::<String>();
    Some(format!("{}...", truncated.trim()))
}

fn log_max_len(id: &str) -> usize {
    if id == FRPC_PRIMARY_INSTANCE_ID {
        PRIMARY_LOG_MAX_LEN
    } else {
        EXTRA_LOG_MAX_LEN
    }
}

async fn restore_on_boot(state: &AppState) -> FrpcResult<()> {
    let had_runtime = has_any_runtime_data(state).await?;
    ensure_primary_instance(state).await?;
    if !had_runtime && should_resume_tunnel(state).await {
        let mut runtime = read_runtime(&state.redis, FRPC_PRIMARY_INSTANCE_ID).await?;
        runtime.desired_running = true;
        write_runtime(&state.redis, FRPC_PRIMARY_INSTANCE_ID, &runtime).await?;
    }
    let metas = all_metas(state).await?;
    for meta in metas {
        let status = build_status(state, &meta).await?;
        if !status.desired_running || status.running {
            continue;
        }
        append_logs(state, &meta, &["resume on boot".to_string()]).await?;
        if let Err(error) = start_instance_inner(state, &meta.id).await {
            append_logs(state, &meta, &[format!("resume error: {}", error.message)]).await?;
        }
    }
    update_aggregate_tunnel_state(state).await?;
    Ok(())
}

async fn has_any_runtime_data(state: &AppState) -> FrpcResult<bool> {
    for id in read_instance_ids(&state.redis).await? {
        if state
            .redis
            .get_json_value(&instance_key(&id, "runtime"))
            .await?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn update_aggregate_tunnel_state(state: &AppState) -> anyhow::Result<()> {
    let overview = build_overview(state)
        .await
        .map_err(|error| anyhow!(error.message))?;
    if overview.running_count > 0 {
        mark_tunnel_running(state).await;
    } else {
        mark_tunnel_stopped(state).await;
    }
    Ok(())
}

async fn should_resume_tunnel(state: &AppState) -> bool {
    load_tunnel_state(state)
        .await
        .get("frp_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

async fn mark_tunnel_running(state: &AppState) {
    let mut object = load_tunnel_state(state).await;
    object.insert("frp_enabled".to_string(), Value::Bool(true));
    object.insert("last_tunnel".to_string(), Value::String("frp".to_string()));
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let _ = state
        .redis
        .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
        .await;
}

async fn mark_tunnel_stopped(state: &AppState) {
    let mut object = load_tunnel_state(state).await;
    if object
        .get("frp_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        object.insert("frp_enabled".to_string(), Value::Bool(false));
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        let _ = state
            .redis
            .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
            .await;
    }
}

async fn load_tunnel_state(state: &AppState) -> serde_json::Map<String, Value> {
    let raw = state
        .redis
        .get_json_value(TUNNEL_RUNTIME_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({}));
    let mut object = if !raw.get("frp_enabled").is_some() && raw.get("tunnel").is_some() {
        let tunnel = raw.get("tunnel").and_then(Value::as_str).unwrap_or("frp");
        let enabled = raw.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        json!({
            "frp_enabled": tunnel == "frp" && enabled,
            "cloudflared_enabled": tunnel == "cloudflared" && enabled,
            "last_tunnel": if tunnel == "cloudflared" { "cloudflared" } else { "frp" },
            "updated_at": raw.get("updated_at").and_then(Value::as_str).unwrap_or("1970-01-01T00:00:00Z")
        })
    } else {
        raw
    };
    let object = object.as_object_mut().cloned().unwrap_or_default();
    let mut normalized = serde_json::Map::new();
    normalized.insert(
        "frp_enabled".to_string(),
        Value::Bool(
            object
                .get("frp_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    normalized.insert(
        "cloudflared_enabled".to_string(),
        Value::Bool(
            object
                .get("cloudflared_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    normalized.insert(
        "last_tunnel".to_string(),
        Value::String(
            if object.get("last_tunnel").and_then(Value::as_str) == Some("cloudflared") {
                "cloudflared"
            } else {
                "frp"
            }
            .to_string(),
        ),
    );
    normalized.insert(
        "updated_at".to_string(),
        Value::String(
            object
                .get("updated_at")
                .and_then(Value::as_str)
                .unwrap_or("1970-01-01T00:00:00Z")
                .to_string(),
        ),
    );
    normalized
}

fn build_summary(content: &str) -> FrpcInstanceSummary {
    let proxy = first_proxy_block(content);
    FrpcInstanceSummary {
        server_addr: extract_toml_value(content, "serverAddr")
            .or_else(|| extract_toml_value(content, "server_addr"))
            .unwrap_or_default(),
        server_port: extract_toml_value(content, "serverPort")
            .or_else(|| extract_toml_value(content, "server_port"))
            .unwrap_or_else(|| "7000".to_string()),
        local_port: extract_toml_value(&proxy, "localPort")
            .or_else(|| extract_toml_value(&proxy, "local_port"))
            .unwrap_or_default(),
        remote_port: extract_toml_value(&proxy, "remotePort")
            .or_else(|| extract_toml_value(&proxy, "remote_port"))
            .unwrap_or_default(),
    }
}

fn first_proxy_block(content: &str) -> String {
    let mut in_block = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if line.trim() == "[[proxies]]" {
            in_block = true;
            continue;
        }
        if in_block && line.trim_start().starts_with("[[") {
            break;
        }
        if in_block {
            lines.push(line);
        }
    }
    lines.join("\n")
}

fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        if left.trim() != key {
            continue;
        }
        let value = right.trim();
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            return Some(value[1..value.len().saturating_sub(1)].to_string());
        }
        if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(value.to_string());
        }
        return None;
    }
    None
}

fn frp_executable(state: &AppState) -> Option<PathBuf> {
    let path = frp_binary_path(&state.settings.data_dir, detect_frp_platform(), "frpc")?;
    path.exists().then_some(path)
}

fn detect_frp_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

fn frp_archive_name(platform: &str) -> Option<String> {
    match platform {
        "linux-amd64" => Some(format!("frp_{FRPC_VERSION}_linux_amd64")),
        "linux-arm64" => Some(format!("frp_{FRPC_VERSION}_linux_arm64")),
        "linux-arm" => Some(format!("frp_{FRPC_VERSION}_linux_arm")),
        "darwin-arm64" => Some(format!("frp_{FRPC_VERSION}_darwin_arm64")),
        _ => None,
    }
}

fn frp_binary_path(data_dir: &Path, platform: &str, binary: &str) -> Option<PathBuf> {
    frp_archive_name(platform).map(|archive| data_dir.join("frp").join(archive).join(binary))
}

fn pid_path_for_meta(meta: &FrpcInstanceMeta) -> PathBuf {
    if meta.is_primary {
        PathBuf::from(&meta.work_dir).join("frpc.pid")
    } else {
        PathBuf::from(&meta.work_dir).join("frpc.pid")
    }
}

async fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .await
        .ok()
        .and_then(|content| content.trim().parse::<u32>().ok())
        .filter(|pid| *pid > 0)
}

async fn write_pid_file(path: &Path, pid: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(path, format!("{pid}\n")).await;
}

async fn remove_pid_file(path: &Path) {
    let _ = fs::remove_file(path).await;
}

async fn terminate_pid(pid: u32) -> FrpcResult<()> {
    if pid == std::process::id() || !is_process_alive(pid) {
        return Ok(());
    }
    send_signal(pid, libc::SIGTERM);
    for _ in 0..20 {
        if !is_process_alive(pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    send_signal(pid, libc::SIGKILL);
    for _ in 0..10 {
        if !is_process_alive(pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    if is_process_alive(pid) {
        return Err(frpc_internal(format!(
            "frpc process is still running: {pid}"
        )));
    }
    Ok(())
}

fn send_signal(pid: u32, signal: libc::c_int) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, signal);
    }
}

fn is_process_alive(pid: u32) -> bool {
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, 0) == 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

async fn is_owned_frpc_pid(pid: u32, config_path: &str) -> bool {
    if !is_process_alive(pid) {
        return false;
    }
    let args = read_process_args(pid).await;
    args.as_deref()
        .is_some_and(|args| is_frpc_process_args_for_config(args, config_path))
}

async fn find_frpc_pid_by_config_path(config_path: &str) -> Option<u32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let pid = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok());
        let Some(pid) = pid else {
            continue;
        };
        if pid == std::process::id() || !is_process_alive(pid) {
            continue;
        }
        let args = read_proc_cmdline_args(pid).await;
        if args
            .as_deref()
            .is_some_and(|args| is_frpc_process_args_for_config(args, config_path))
        {
            return Some(pid);
        }
    }
    None
}

async fn read_process_args(pid: u32) -> Option<Vec<String>> {
    read_proc_cmdline_args(pid)
        .await
        .or_else(|| read_ps_command_args(pid))
}

async fn read_proc_cmdline_args(pid: u32) -> Option<Vec<String>> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    let args = bytes
        .split(|byte| *byte == 0)
        .filter_map(|part| {
            let value = String::from_utf8_lossy(part).trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .collect::<Vec<_>>();
    (!args.is_empty()).then_some(args)
}

fn read_ps_command_args(pid: u32) -> Option<Vec<String>> {
    let output = std::process::Command::new("ps")
        .args(["-ww", "-p", &pid.to_string(), "-o", "args="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let command = String::from_utf8_lossy(&output.stdout);
    let args = split_command_line(command.trim());
    (!args.is_empty()).then_some(args)
}

fn split_command_line(command: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut in_arg = false;
    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            in_arg = true;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            in_arg = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            in_arg = true;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            in_arg = true;
            continue;
        }
        if ch.is_whitespace() {
            if in_arg {
                args.push(std::mem::take(&mut current));
                in_arg = false;
            }
            continue;
        }
        current.push(ch);
        in_arg = true;
    }
    if escaped {
        current.push('\\');
    }
    if in_arg {
        args.push(current);
    }
    args
}

fn is_frpc_process_args_for_config(args: &[String], config_path: &str) -> bool {
    let Some(first) = args.first() else {
        return false;
    };
    let executable = Path::new(first)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if executable != "frpc" && executable != "frpc.exe" {
        return false;
    }
    for (index, arg) in args.iter().enumerate().skip(1) {
        if matches!(arg.as_str(), "-c" | "--config" | "--config-file") {
            return args
                .get(index + 1)
                .is_some_and(|candidate| same_path(candidate, config_path));
        }
        if let Some(candidate) = arg.strip_prefix("--config=") {
            return same_path(candidate, config_path);
        }
        if let Some(candidate) = arg.strip_prefix("--config-file=") {
            return same_path(candidate, config_path);
        }
    }
    false
}

fn same_path(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(value: &str) -> String {
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    absolute.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frpc_summary_from_camel_case_toml() {
        let summary = build_summary(
            r#"
serverAddr = "frp.example.com"
serverPort = 7001

[[proxies]]
localPort = 7999
remotePort = 443
"#,
        );
        assert_eq!(summary.server_addr, "frp.example.com");
        assert_eq!(summary.server_port, "7001");
        assert_eq!(summary.local_port, "7999");
        assert_eq!(summary.remote_port, "443");
    }

    #[test]
    fn parses_frpc_summary_like_node_toml_regex() {
        let summary = build_summary(
            r#"
serverAddr = frp.example.com
serverPort = 7001 # comment

[[proxies]]
localPort = "7999"
remotePort = 443 # comment
"#,
        );
        assert_eq!(summary.server_addr, "");
        assert_eq!(summary.server_port, "7000");
        assert_eq!(summary.local_port, "7999");
        assert_eq!(summary.remote_port, "");
    }

    #[test]
    fn detected_frpc_runtime_clears_stale_exit_state() {
        let runtime = FrpcInstanceRuntime {
            desired_running: true,
            pid: Some(42),
            started_at: Some("2026-01-01T00:00:00Z".to_string()),
            stopped_at: Some("2026-01-01T00:01:00Z".to_string()),
            last_exit_code: Some(1),
            last_message: Some("frpc exited with code 1".to_string()),
        };

        let next = merge_detected_frpc_runtime(runtime.clone(), 42);
        assert_eq!(next.pid, Some(42));
        assert_eq!(next.started_at, runtime.started_at);
        assert_eq!(next.stopped_at, None);
        assert_eq!(next.last_exit_code, None);
        assert_eq!(
            next.last_message.as_deref(),
            Some("frpc process detected pid=42")
        );
        assert!(should_persist_detected_runtime(&runtime, &next));
    }

    #[test]
    fn matches_frpc_process_config_args() {
        assert!(is_frpc_process_args_for_config(
            &[
                "/opt/frp/frpc".to_string(),
                "-c".to_string(),
                "/tmp/frpc.toml".to_string()
            ],
            "/tmp/frpc.toml"
        ));
        assert!(is_frpc_process_args_for_config(
            &["frpc".to_string(), "--config=/tmp/frpc.toml".to_string()],
            "/tmp/frpc.toml"
        ));
        assert!(!is_frpc_process_args_for_config(
            &[
                "frps".to_string(),
                "-c".to_string(),
                "/tmp/frpc.toml".to_string()
            ],
            "/tmp/frpc.toml"
        ));
    }

    #[test]
    fn sanitizes_instance_ids_like_node() {
        assert_eq!(sanitize_instance_id("abc-123").as_deref(), Some("abc-123"));
        assert!(sanitize_instance_id("../bad").is_none());
        assert!(sanitize_instance_id("").is_none());
    }

    #[test]
    fn default_instance_names_match_node_default_locale() {
        assert_eq!(default_frpc_primary_name(), "主 FRP");
        assert_eq!(default_frpc_instance_name(), "FRP 实例");
    }

    #[test]
    fn log_limit_parser_matches_node_parse_int_prefixes() {
        assert_eq!(parse_limit(None), 200);
        assert_eq!(parse_limit(Some("")), 200);
        assert_eq!(parse_limit(Some("10x")), 10);
        assert_eq!(parse_limit(Some("0x10")), 1);
        assert_eq!(parse_limit(Some("-5")), 1);
        assert_eq!(parse_limit(Some("5000")), 1000);
        assert_eq!(parse_limit(Some("abc")), 200);
    }

    #[test]
    fn localizes_frpc_errors_and_runtime_messages() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_frpc_error(&translator, "FRPC instance not found: abc"),
            "FRP 实例不存在：abc"
        );
        assert_eq!(
            localize_frpc_error(&translator, "FRPC instance limit exceeded (20)"),
            "额外 FRP 实例最多支持 20 个"
        );
        assert_eq!(
            localize_frpc_error(&translator, "Primary FRPC instance cannot be deleted"),
            "主 FRP 实例不允许删除"
        );
        assert_eq!(
            localize_frpc_error(&translator, "frpc config verify failed with code 2"),
            "frpc verify 校验失败，退出码 2"
        );
        assert_eq!(
            localize_frpc_error(&translator, "Failed to read frpc pid"),
            "读取 frpc PID 失败"
        );

        let localized = localize_frpc_response_value(
            json!({
                "item": { "lastMessage": "frpc started pid=1234" },
                "status": { "lastMessage": "frpc exited with code 1" },
                "legacy": { "last_message": "frpc already stopped" }
            }),
            &translator,
        );
        assert_eq!(localized["item"]["lastMessage"], "frpc 已启动 pid=1234");
        assert_eq!(
            localized["status"]["lastMessage"],
            "frpc 进程已退出（退出码 1）"
        );
        assert_eq!(localized["legacy"]["last_message"], "frpc 已停止");
    }
}
