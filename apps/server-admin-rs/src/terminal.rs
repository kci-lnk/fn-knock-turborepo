use std::{
    collections::{BTreeSet, HashMap},
    env, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, anyhow};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
    process::Command,
    sync::Mutex,
    task,
    time::{Instant, MissedTickBehavior, interval, sleep},
};
use uuid::Uuid;

use crate::{
    i18n::Translator,
    redis_store::RedisStore,
    response, runtime_profile,
    state::AppState,
    terminal_paths::{is_terminal_runtime_cwd, normalize_terminal_default_cwd},
    time_utils::{iso_after_seconds, now_iso, now_ms, parse_iso_ms},
};

const SESSION_INDEX_KEY: &str = "fn_knock:terminal:session:index";
const SESSION_DATA_PREFIX: &str = "fn_knock:terminal:session:data:";
const SESSION_ATTACHMENTS_PREFIX: &str = "fn_knock:terminal:session:attachments:";
const ATTACHMENT_DATA_PREFIX: &str = "fn_knock:terminal:attachment:data:";
const TERMINAL_STREAM_DIR_NAME: &str = "terminal-streams";
const TERMINAL_STREAM_CHUNK_MAX_BYTES: i64 = 256 * 1024;
const TERMINAL_SNAPSHOT_SCROLLBACK_ROWS: i64 = 200;
const DEFAULT_POLL_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_POLL_INTERVAL_MS: u64 = 300;
const DEFAULT_ATTACHMENT_TTL_SECONDS: i64 = 120;
const INPUT_SESSION_TOUCH_THROTTLE_MS: i64 = 5_000;
const TMUX_TARGET_PANE_SUFFIX: &str = ":0.0";
const TMUX_ABSOLUTE_FALLBACK_PATH: &str = "/usr/bin/tmux";
const DEBIAN_APT_GET_PATH: &str = "/usr/bin/apt-get";
const LEGACY_DEFAULT_SESSION_TITLE_PREFIX: &str = "Terminal Session ";
const TERMINAL_MESSAGE_LOCALES: [&str; 5] = ["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"];
const TERMINAL_SIMPLE_ERROR_KEYS: &[&str] = &[
    "tmuxNotDetectedInstallFirst",
    "refreshingApt",
    "aptUpdateFailed",
    "installingTmux",
    "aptInstallTmuxFailed",
    "verifyingTmuxInstall",
    "tmuxMissingAfterInstall",
    "tmuxInstallFailed",
    "webTerminalDisabled",
    "tmuxInstallingWait",
    "tmuxMissingCannotCreate",
    "rootRunRequiresDangerToggle",
    "noShellDetected",
    "paneMetadataReadFailed",
    "paneTtyParseFailed",
    "inputPipeCreateFailed",
    "ioRelayCreateFailed",
    "tmuxSessionCreateFailed",
    "sessionTitleRequired",
    "sessionMissingOrExpired",
    "tmuxMissingCannotAttach",
    "inputPipeNotReady",
    "inputWriteInterrupted",
    "attachmentExpired",
    "inputSendFailed",
    "resizeFailed",
    "sessionNotFound",
];
const TERMINAL_PARAMETERIZED_ERROR_KEYS: &[(&str, &str)] = &[
    ("tmuxReadyWithVersion", "version"),
    ("tmuxInstallCompleteWithVersion", "version"),
    ("cwdUnavailable", "path"),
    ("tmuxStatusError", "message"),
    ("requestedShellUnavailable", "shell"),
    ("sessionLimitReached", "count"),
];

static TMUX_CACHE: LazyLock<Mutex<Option<TmuxExecutableInfo>>> = LazyLock::new(|| Mutex::new(None));
static TMUX_INSTALL_STATE: LazyLock<Mutex<TerminalTmuxInstallState>> =
    LazyLock::new(|| Mutex::new(default_tmux_install_state()));
static TMUX_INSTALL_RUNNING: AtomicBool = AtomicBool::new(false);
static SESSION_TOUCH_DEADLINES: LazyLock<Mutex<HashMap<String, i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug)]
struct ExecResult {
    code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Debug)]
struct TmuxExecutableInfo {
    path: String,
    detection_source: String,
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TerminalFeatureConfig {
    enabled: bool,
    default_cwd: String,
    max_sessions: i64,
    idle_timeout_seconds: i64,
    resume_backend: String,
    allow_mobile_toolbar: bool,
    dangerously_run_as_current_user: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalTmuxInstallState {
    status: String,
    progress: i64,
    message: String,
    executable_path: String,
    detection_source: Option<String>,
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalRuntimeStatus {
    enabled: bool,
    tmux_available: bool,
    tmux_executable_path: String,
    tmux_detection_source: Option<String>,
    tmux_version: String,
    tmux_install_state: TerminalTmuxInstallState,
    http_polling_available: bool,
    running_as_root: bool,
    blocked_reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TerminalSessionRecord {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    last_attached_at: String,
    #[serde(default)]
    last_detached_at: String,
    #[serde(default)]
    last_client_ip: String,
    #[serde(default)]
    shell: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    cols: i64,
    #[serde(default)]
    rows: i64,
    #[serde(default)]
    resume_backend: String,
    #[serde(default)]
    backend_session_name: String,
    #[serde(default)]
    pane_tty_path: String,
    #[serde(default)]
    input_pipe_path: String,
    #[serde(default)]
    output_log_path: String,
    #[serde(default)]
    expires_at: String,
    #[serde(default)]
    last_frame_revision: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TerminalAttachmentRecord {
    #[serde(default)]
    id: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    transport: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    expires_at: String,
}

#[derive(Clone, Debug, Serialize)]
struct TerminalOutputChunk {
    cursor: i64,
    data_base64: String,
    reset: bool,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
struct TerminalPollResult {
    changed: bool,
    chunk: Option<TerminalOutputChunk>,
}

#[derive(Deserialize)]
struct CreateSessionBody {
    title: Option<String>,
    shell: Option<String>,
    cwd: Option<String>,
    cols: Option<f64>,
    rows: Option<f64>,
}

#[derive(Deserialize)]
struct RenameSessionBody {
    title: String,
}

#[derive(Deserialize)]
struct InputBody {
    #[serde(rename = "dataBase64")]
    data_base64: String,
}

#[derive(Deserialize)]
struct ResizeBody {
    cols: f64,
    rows: f64,
}

#[derive(Deserialize)]
struct PollQuery {
    cursor: Option<String>,
    timeout_ms: Option<f64>,
}

pub fn terminal_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/terminal/status", get(status))
        .route("/api/admin/terminal/tmux/install", post(install_tmux))
        .route(
            "/api/admin/terminal/sessions",
            get(list_sessions).post(create_session),
        )
        .route(
            "/api/admin/terminal/sessions/{id}",
            get(get_session)
                .patch(rename_session)
                .delete(delete_session),
        )
        .route(
            "/api/admin/terminal/sessions/{id}/attachments",
            post(create_attachment),
        )
        .route(
            "/api/admin/terminal/attachments/{id}/poll",
            get(poll_attachment),
        )
        .route(
            "/api/admin/terminal/attachments/{id}/input",
            post(send_input),
        )
        .route(
            "/api/admin/terminal/attachments/{id}/resize",
            post(resize_attachment),
        )
        .route(
            "/api/admin/terminal/attachments/{id}",
            delete(delete_attachment),
        )
}

pub fn start_terminal_tasks(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = cleanup_expired_sessions(&state).await {
                tracing::warn!(%error, "failed to cleanup expired terminal sessions");
            }
        }
    });
}

async fn status(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match runtime_status(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn install_tmux(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match start_tmux_install().await {
        Ok(mut data) => {
            let translator = Translator::from_state(&state).await;
            localize_tmux_install_state(&mut data, &translator);
            response::ok(data).into_response()
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn list_sessions(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_list_sessions(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn get_session(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_get_session(&state, &id).await {
        Ok(Some(data)) => response::ok(data).into_response(),
        Ok(None) => {
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::NOT_FOUND,
                terminal_text(&translator, "sessionNotFound", &[]),
            )
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    let client_ip = detect_client_ip(&headers);
    match terminal_create_session(&state, body, &client_ip).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn rename_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<RenameSessionBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_rename_session(&state, &id, &body.title).await {
        Ok(Some(data)) => response::ok(data).into_response(),
        Ok(None) => {
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::NOT_FOUND,
                terminal_text(&translator, "sessionNotFound", &[]),
            )
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn delete_session(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_kill_session(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn create_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    let client_ip = detect_client_ip(&headers);
    match terminal_create_attachment(&state, &id, &client_ip).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn poll_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<PollQuery>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_wait_for_output(
        &state,
        &id,
        parse_output_cursor_like_node(query.cursor.as_deref()),
        query.timeout_ms,
    )
    .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn send_input(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<InputBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_send_input(&state, &id, &body.data_base64).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn resize_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<ResizeBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_resize_attachment(&state, &id, body.cols, body.rows).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn delete_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_detach_attachment(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

async fn terminal_unavailable_response(state: &AppState) -> Option<Response> {
    if runtime_profile::terminal_available(state) {
        return None;
    }
    let translator = Translator::from_state(state).await;
    let profile = runtime_profile::get_runtime_profile(state);
    Some(response::error(
        StatusCode::FORBIDDEN,
        runtime_profile::capability_unavailable_message(
            "terminal_available",
            &profile,
            &translator,
        ),
    ))
}

async fn terminal_error(state: &AppState, error: anyhow::Error) -> Response {
    let translator = Translator::from_state(state).await;
    response::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        localize_terminal_error(&translator, &error.to_string()),
    )
}

fn terminal_text(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    let full_key = format!("server.terminal.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

fn terminal_default_text(key: &str, params: &[(&str, String)]) -> String {
    terminal_text(&Translator::new(crate::i18n::DEFAULT_LOCALE), key, params)
}

fn localize_terminal_error(translator: &Translator, raw_message: &str) -> String {
    let raw_message = raw_message.trim();
    if raw_message.is_empty() {
        return terminal_text(translator, "operationFailed", &[]);
    }
    if let Some(message) = localize_known_terminal_message(translator, raw_message) {
        return message;
    }
    terminal_text(
        translator,
        "operationFailedWithMessage",
        &[("message", raw_message.to_string())],
    )
}

fn localize_known_terminal_message(translator: &Translator, raw_message: &str) -> Option<String> {
    localize_terminal_parameterized_message(translator, raw_message)
        .or_else(|| localize_terminal_simple_message(translator, raw_message))
}

fn localize_terminal_simple_message(translator: &Translator, raw_message: &str) -> Option<String> {
    for &key in TERMINAL_SIMPLE_ERROR_KEYS {
        for locale in TERMINAL_MESSAGE_LOCALES {
            let source = terminal_text(&Translator::new(locale), key, &[]);
            if source == raw_message {
                return Some(terminal_text(translator, key, &[]));
            }
            if let Some(detail) = raw_message.strip_prefix(&format!("{source}:")) {
                return Some(format!(
                    "{}:{}",
                    terminal_text(translator, key, &[]),
                    detail
                ));
            }
        }
    }
    None
}

fn localize_terminal_parameterized_message(
    translator: &Translator,
    raw_message: &str,
) -> Option<String> {
    const MARKER: &str = "__fn_knock_terminal_param__";
    for &(key, param) in TERMINAL_PARAMETERIZED_ERROR_KEYS {
        for locale in TERMINAL_MESSAGE_LOCALES {
            let template = terminal_text(&Translator::new(locale), key, &[(param, MARKER.into())]);
            if let Some(value) = extract_single_template_value(&template, raw_message, MARKER) {
                return Some(terminal_text(translator, key, &[(param, value)]));
            }
        }
    }
    None
}

fn extract_single_template_value(
    template: &str,
    raw_message: &str,
    marker: &str,
) -> Option<String> {
    let (prefix, suffix) = template.split_once(marker)?;
    if !raw_message.starts_with(prefix) || !raw_message.ends_with(suffix) {
        return None;
    }
    let value_end = raw_message.len().checked_sub(suffix.len())?;
    if value_end < prefix.len() {
        return None;
    }
    Some(raw_message[prefix.len()..value_end].to_string())
}

async fn runtime_status(state: &AppState) -> anyhow::Result<TerminalRuntimeStatus> {
    let translator = Translator::from_state(state).await;
    let config = terminal_feature_config(state).await?;
    let mut install_state = get_tmux_install_state().await;
    localize_tmux_install_state(&mut install_state, &translator);
    let tmux_available = install_state.status == "installed";
    let running_as_root = is_running_as_root();
    let blocked_reason = if !config.enabled {
        translator.t("server.terminal.webTerminalDisabled")
    } else if install_state.status == "installing" {
        translator.t("server.terminal.tmuxInstallingWait")
    } else if !tmux_available {
        if install_state.status == "error" {
            translator.t_params(
                "server.terminal.tmuxStatusError",
                &[("message", install_state.message.clone())],
            )
        } else {
            translator.t("server.terminal.tmuxMissingCannotCreate")
        }
    } else if running_as_root && !config.dangerously_run_as_current_user {
        translator.t("server.terminal.rootRunRequiresDangerToggle")
    } else {
        String::new()
    };

    Ok(TerminalRuntimeStatus {
        enabled: config.enabled,
        tmux_available,
        tmux_executable_path: install_state.executable_path.clone(),
        tmux_detection_source: install_state.detection_source.clone(),
        tmux_version: install_state.version.clone(),
        tmux_install_state: install_state,
        http_polling_available: true,
        running_as_root,
        blocked_reason,
    })
}

fn localize_tmux_install_state(state: &mut TerminalTmuxInstallState, translator: &Translator) {
    state.message = match state.status.as_str() {
        "installed" => translator.t_params(
            "server.terminal.tmuxReadyWithVersion",
            &[("version", state.version.clone())],
        ),
        "installing" if state.progress < 30 => translator.t("server.terminal.refreshingApt"),
        "installing" if state.progress < 90 => translator.t("server.terminal.installingTmux"),
        "installing" => translator.t("server.terminal.verifyingTmuxInstall"),
        "uninstalled" => translator.t("server.terminal.tmuxNotDetectedInstallFirst"),
        "error" => localize_known_terminal_message(translator, &state.message)
            .unwrap_or_else(|| state.message.clone()),
        _ => state.message.clone(),
    };
}

async fn terminal_feature_config(state: &AppState) -> anyhow::Result<TerminalFeatureConfig> {
    let config = state.redis.get_config().await?;
    Ok(normalize_terminal_feature(config.get("terminal_feature")))
}

fn normalize_terminal_feature(value: Option<&Value>) -> TerminalFeatureConfig {
    TerminalFeatureConfig {
        enabled: bool_field(value, "enabled", false),
        default_cwd: normalize_terminal_default_cwd(
            value
                .and_then(|value| value.get("default_cwd"))
                .and_then(Value::as_str),
        ),
        max_sessions: int_field(value, "max_sessions", 3, 1, 12),
        idle_timeout_seconds: int_field(
            value,
            "idle_timeout_seconds",
            24 * 60 * 60,
            60,
            7 * 24 * 60 * 60,
        ),
        resume_backend: "tmux".to_string(),
        allow_mobile_toolbar: bool_field(value, "allow_mobile_toolbar", true),
        dangerously_run_as_current_user: bool_field(value, "dangerously_run_as_current_user", true),
    }
}

fn bool_field(value: Option<&Value>, key: &str, fallback: bool) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

fn int_field(value: Option<&Value>, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .and_then(|value| value.get(key))
        .and_then(parse_int_field_value)
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn parse_int_field_value(value: &Value) -> Option<i64> {
    parse_i64_prefix(js_string_for_parse_int(value).trim_start())
}

fn js_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(js_array_item_string_for_parse_int)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_array_item_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Array(_) => js_string_for_parse_int(value),
        Value::Object(_) => "[object Object]".to_string(),
        _ => js_string_for_parse_int(value),
    }
}

async fn start_tmux_install() -> anyhow::Result<TerminalTmuxInstallState> {
    let current = get_tmux_install_state().await;
    if current.status == "installed" || current.status == "installing" {
        return Ok(current);
    }

    if TMUX_INSTALL_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        {
            let mut state = TMUX_INSTALL_STATE.lock().await;
            *state = TerminalTmuxInstallState {
                status: "installing".to_string(),
                progress: 15,
                message: terminal_default_text("refreshingApt", &[]),
                executable_path: String::new(),
                detection_source: None,
                version: String::new(),
            };
        }
        tokio::spawn(async {
            install_tmux_in_background().await;
            TMUX_INSTALL_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    Ok(TMUX_INSTALL_STATE.lock().await.clone())
}

async fn install_tmux_in_background() {
    if let Err(error) = do_install_tmux().await {
        reset_tmux_probe_cache().await;
        let mut state = TMUX_INSTALL_STATE.lock().await;
        *state = TerminalTmuxInstallState {
            status: "error".to_string(),
            progress: 0,
            message: error.to_string(),
            executable_path: String::new(),
            detection_source: None,
            version: String::new(),
        };
    }
}

async fn do_install_tmux() -> anyhow::Result<()> {
    set_install_state(
        "installing",
        15,
        &terminal_default_text("refreshingApt", &[]),
        None,
    )
    .await;
    ensure_process_succeeded(
        DEBIAN_APT_GET_PATH,
        &["update"],
        &terminal_default_text("aptUpdateFailed", &[]),
    )
    .await?;

    set_install_state(
        "installing",
        60,
        &terminal_default_text("installingTmux", &[]),
        None,
    )
    .await;
    ensure_process_succeeded(
        DEBIAN_APT_GET_PATH,
        &["install", "-y", "tmux"],
        &terminal_default_text("aptInstallTmuxFailed", &[]),
    )
    .await?;

    set_install_state(
        "installing",
        90,
        &terminal_default_text("verifyingTmuxInstall", &[]),
        None,
    )
    .await;
    reset_tmux_probe_cache().await;
    let Some(tmux) = detect_tmux_executable().await else {
        return Err(anyhow!(terminal_default_text(
            "tmuxMissingAfterInstall",
            &[]
        )));
    };
    let ready_message = terminal_default_text(
        "tmuxInstallCompleteWithVersion",
        &[("version", tmux.version.clone())],
    );
    set_install_state("installed", 100, &ready_message, Some(tmux)).await;
    Ok(())
}

async fn set_install_state(
    status: &str,
    progress: i64,
    message: &str,
    tmux: Option<TmuxExecutableInfo>,
) {
    let mut state = TMUX_INSTALL_STATE.lock().await;
    *state = TerminalTmuxInstallState {
        status: status.to_string(),
        progress,
        message: message.to_string(),
        executable_path: tmux
            .as_ref()
            .map(|value| value.path.clone())
            .unwrap_or_default(),
        detection_source: tmux.as_ref().map(|value| value.detection_source.clone()),
        version: tmux.map(|value| value.version).unwrap_or_default(),
    };
}

async fn get_tmux_install_state() -> TerminalTmuxInstallState {
    if TMUX_INSTALL_RUNNING.load(Ordering::SeqCst) {
        return TMUX_INSTALL_STATE.lock().await.clone();
    }

    if let Some(tmux) = detect_tmux_executable().await {
        let ready_message =
            terminal_default_text("tmuxReadyWithVersion", &[("version", tmux.version.clone())]);
        let state = TerminalTmuxInstallState {
            status: "installed".to_string(),
            progress: 100,
            message: ready_message,
            executable_path: tmux.path,
            detection_source: Some(tmux.detection_source),
            version: tmux.version,
        };
        *TMUX_INSTALL_STATE.lock().await = state.clone();
        return state;
    }

    let current = TMUX_INSTALL_STATE.lock().await.clone();
    if current.status == "error" {
        current
    } else {
        default_tmux_install_state()
    }
}

fn default_tmux_install_state() -> TerminalTmuxInstallState {
    TerminalTmuxInstallState {
        status: "uninstalled".to_string(),
        progress: 0,
        message: terminal_default_text("tmuxNotDetectedInstallFirst", &[]),
        executable_path: String::new(),
        detection_source: None,
        version: String::new(),
    }
}

async fn detect_tmux_executable() -> Option<TmuxExecutableInfo> {
    if let Some(cached) = TMUX_CACHE.lock().await.clone() {
        return Some(cached);
    }

    let candidates = [
        ("tmux", "env-path"),
        (TMUX_ABSOLUTE_FALLBACK_PATH, "absolute-path"),
    ];
    for (path, detection_source) in candidates {
        let Ok(result) = run_process(path, &["-V"], None, true).await else {
            continue;
        };
        if result.code == 0 {
            let info = TmuxExecutableInfo {
                path: path.to_string(),
                detection_source: detection_source.to_string(),
                version: if result.stdout.trim().is_empty() {
                    "tmux".to_string()
                } else {
                    result.stdout
                },
            };
            *TMUX_CACHE.lock().await = Some(info.clone());
            return Some(info);
        }
    }
    None
}

async fn reset_tmux_probe_cache() {
    *TMUX_CACHE.lock().await = None;
}

async fn run_tmux(args: &[&str]) -> anyhow::Result<ExecResult> {
    let tmux = detect_tmux_executable().await;
    run_process(
        tmux.as_ref()
            .map(|value| value.path.as_str())
            .unwrap_or("tmux"),
        args,
        None,
        true,
    )
    .await
}

async fn run_tmux_raw(args: &[&str]) -> anyhow::Result<ExecResult> {
    let tmux = detect_tmux_executable().await;
    run_process(
        tmux.as_ref()
            .map(|value| value.path.as_str())
            .unwrap_or("tmux"),
        args,
        None,
        false,
    )
    .await
}

async fn run_process(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
    trim_output: bool,
) -> anyhow::Result<ExecResult> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd
        .output()
        .await
        .with_context(|| format!("run process {command}"))?;
    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if trim_output {
        stdout = stdout.trim_end().to_string();
        stderr = stderr.trim_end().to_string();
    }
    Ok(ExecResult {
        code: output.status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

async fn ensure_process_succeeded(
    command: &str,
    args: &[&str],
    failure_message: &str,
) -> anyhow::Result<ExecResult> {
    let result = run_process(command, args, None, false).await?;
    if result.code == 0 {
        return Ok(ExecResult {
            code: result.code,
            stdout: result.stdout.trim_end().to_string(),
            stderr: result.stderr.trim_end().to_string(),
        });
    }
    let detail = summarize_process_output(&result);
    if detail.is_empty() {
        Err(anyhow!(failure_message.to_string()))
    } else {
        Err(anyhow!("{failure_message}: {detail}"))
    }
}

fn summarize_process_output(result: &ExecResult) -> String {
    let detail = format!("{}\n{}", result.stderr, result.stdout);
    let lines = detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    lines
        .iter()
        .rev()
        .take(8)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(500)
        .collect()
}

async fn terminal_list_sessions(state: &AppState) -> anyhow::Result<Vec<TerminalSessionRecord>> {
    cleanup_expired_sessions(state).await?;
    store_list_sessions(&state.redis).await
}

async fn terminal_get_session(
    state: &AppState,
    id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.redis, id).await? else {
        return Ok(None);
    };
    if !tmux_session_exists(&session.backend_session_name).await {
        cleanup_session_artifacts(&session).await;
        store_delete_session(&state.redis, id).await?;
        return Ok(None);
    }
    Ok(Some(session))
}

async fn terminal_create_session(
    state: &AppState,
    input: CreateSessionBody,
    client_ip: &str,
) -> anyhow::Result<TerminalSessionRecord> {
    cleanup_expired_sessions(state).await?;
    assert_create_allowed(state).await?;

    let config = terminal_feature_config(state).await?;
    let translator = Translator::from_state(state).await;
    let existing = store_list_sessions(&state.redis).await?;
    if existing.len() as i64 >= config.max_sessions {
        return Err(anyhow!(terminal_default_text(
            "sessionLimitReached",
            &[("count", config.max_sessions.to_string())],
        )));
    }

    let shell = resolve_shell(input.shell.as_deref()).await?;
    let cwd = resolve_cwd(&config, input.cwd.as_deref()).await?;
    let cols = normalize_terminal_dimension(input.cols, 120, 40, 400);
    let rows = normalize_terminal_dimension(input.rows, 32, 12, 200);
    let id = Uuid::new_v4().to_string();
    let session_name = build_session_name(&id);
    let title = sanitize_title(input.title.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| build_default_session_title(&existing, &translator));
    let command = build_session_shell_command(&shell);

    let create_result = run_tmux(&[
        "new-session",
        "-d",
        "-s",
        &session_name,
        "-x",
        &cols.to_string(),
        "-y",
        &rows.to_string(),
        "-c",
        path_to_str(&cwd)?,
        &command,
    ])
    .await?;
    if create_result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &create_result.stderr,
                &terminal_default_text("tmuxSessionCreateFailed", &[])
            )
        ));
    }

    let stream_dir = stream_directory(state);
    let session = normalize_session(TerminalSessionRecord {
        id: id.clone(),
        title,
        status: "detached".to_string(),
        created_at: now_iso(),
        updated_at: now_iso(),
        last_client_ip: client_ip.to_string(),
        shell,
        cwd: cwd.to_string_lossy().to_string(),
        cols,
        rows,
        resume_backend: "tmux".to_string(),
        backend_session_name: session_name.clone(),
        input_pipe_path: build_input_pipe_path(&stream_dir, &id)
            .to_string_lossy()
            .to_string(),
        output_log_path: build_output_log_path(&stream_dir, &id)
            .to_string_lossy()
            .to_string(),
        expires_at: iso_after_seconds(config.idle_timeout_seconds),
        ..Default::default()
    });

    match configure_session_runtime(state, session.clone()).await {
        Ok(session) => Ok(session),
        Err(error) => {
            let _ = run_tmux(&["kill-session", "-t", &session_name]).await;
            cleanup_session_artifacts(&session).await;
            Err(error)
        }
    }
}

async fn terminal_rename_session(
    state: &AppState,
    id: &str,
    title: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.redis, id).await? else {
        return Ok(None);
    };
    let title = sanitize_title(Some(title)).unwrap_or_default();
    if title.is_empty() {
        return Err(anyhow!(terminal_default_text("sessionTitleRequired", &[])));
    }
    store_save_session(
        &state.redis,
        normalize_session(TerminalSessionRecord {
            title,
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
    .map(Some)
}

async fn terminal_kill_session(state: &AppState, id: &str) -> anyhow::Result<()> {
    let Some(session) = store_get_session(&state.redis, id).await? else {
        return Ok(());
    };
    let _ = run_tmux(&["kill-session", "-t", &session.backend_session_name]).await;
    cleanup_session_artifacts(&session).await;
    store_delete_session(&state.redis, id).await?;
    Ok(())
}

async fn terminal_create_attachment(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
) -> anyhow::Result<TerminalAttachmentRecord> {
    let Some(session) = terminal_get_session(state, session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let status = runtime_status(state).await?;
    if !status.enabled {
        return Err(anyhow!(terminal_default_text("webTerminalDisabled", &[])));
    }
    if !status.tmux_available {
        return Err(anyhow!(terminal_default_text(
            "tmuxMissingCannotAttach",
            &[]
        )));
    }

    let runtime_session = ensure_session_runtime(state, session).await?;
    let now = now_iso();
    let config = terminal_feature_config(state).await?;
    store_save_session(
        &state.redis,
        normalize_session(TerminalSessionRecord {
            status: "attached".to_string(),
            updated_at: now.clone(),
            last_attached_at: now.clone(),
            last_client_ip: client_ip.to_string(),
            expires_at: iso_after_seconds(config.idle_timeout_seconds),
            ..runtime_session.clone()
        }),
    )
    .await?;

    store_save_attachment(
        &state.redis,
        normalize_attachment(TerminalAttachmentRecord {
            id: Uuid::new_v4().to_string(),
            session_id: runtime_session.id,
            transport: "http-polling".to_string(),
            created_at: now.clone(),
            updated_at: now,
            expires_at: iso_after_seconds(DEFAULT_ATTACHMENT_TTL_SECONDS),
        }),
        DEFAULT_ATTACHMENT_TTL_SECONDS,
    )
    .await
}

async fn terminal_detach_attachment(state: &AppState, attachment_id: &str) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(&state.redis, attachment_id).await? else {
        return Ok(());
    };
    store_delete_attachment(&state.redis, attachment_id).await?;
    let remaining =
        store_list_attachment_ids_for_session(&state.redis, &attachment.session_id).await?;
    if remaining.is_empty() {
        mark_session_detached(state, &attachment.session_id).await?;
    }
    Ok(())
}

async fn terminal_send_input(
    state: &AppState,
    attachment_id: &str,
    data_base64: &str,
) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(&state.redis, attachment_id).await? else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = store_get_session(&state.redis, &attachment.session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let data = general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .unwrap_or_default();
    if data.is_empty() {
        return Ok(());
    }

    let runtime_session = if session.input_pipe_path.trim().is_empty() {
        ensure_session_runtime(state, session).await?
    } else {
        session
    };

    if let Err(error) = write_input_pipe(&runtime_session.input_pipe_path, data.clone()).await {
        let Some(confirmed) = terminal_get_session(state, &runtime_session.id).await? else {
            return Err(anyhow!(terminal_default_text(
                "sessionMissingOrExpired",
                &[]
            )));
        };
        let refreshed = configure_session_runtime(state, confirmed).await?;
        write_input_pipe(&refreshed.input_pipe_path, data)
            .await
            .map_err(|retry_error| {
                anyhow!(
                    "{}: {retry_error}",
                    terminal_default_text("inputSendFailed", &[])
                )
            })?;
        tracing::warn!(session_id = %refreshed.id, %error, "terminal input pipe recovered after runtime refresh");
        touch_session_activity(state, refreshed, false).await?;
    } else {
        touch_session_activity(state, runtime_session, false).await?;
    }
    Ok(())
}

async fn terminal_resize_attachment(
    state: &AppState,
    attachment_id: &str,
    cols: f64,
    rows: f64,
) -> anyhow::Result<TerminalSessionRecord> {
    let Some(attachment) =
        store_refresh_attachment(&state.redis, attachment_id, DEFAULT_ATTACHMENT_TTL_SECONDS)
            .await?
    else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = terminal_get_session(state, &attachment.session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let next_cols = normalize_terminal_dimension(Some(cols), session.cols, 40, 400);
    let next_rows = normalize_terminal_dimension(Some(rows), session.rows, 12, 200);
    let resize_result = run_tmux(&[
        "resize-window",
        "-t",
        &session.backend_session_name,
        "-x",
        &next_cols.to_string(),
        "-y",
        &next_rows.to_string(),
    ])
    .await?;
    if resize_result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &resize_result.stderr,
                &terminal_default_text("resizeFailed", &[])
            )
        ));
    }
    refresh_session_expiry(
        state,
        normalize_session(TerminalSessionRecord {
            cols: next_cols,
            rows: next_rows,
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
}

async fn terminal_wait_for_output(
    state: &AppState,
    attachment_id: &str,
    cursor: i64,
    timeout_ms: Option<f64>,
) -> anyhow::Result<TerminalPollResult> {
    let Some(attachment) =
        store_refresh_attachment(&state.redis, attachment_id, DEFAULT_ATTACHMENT_TTL_SECONDS)
            .await?
    else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = terminal_get_session(state, &attachment.session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let runtime_session = ensure_session_runtime(state, session).await?;
    let requested_cursor = cursor.max(0);
    let timeout = normalize_terminal_poll_timeout_ms(timeout_ms);
    let deadline = Instant::now() + Duration::from_millis(timeout);

    while Instant::now() < deadline {
        if let Some(chunk) = read_output_chunk(&runtime_session, requested_cursor).await? {
            return Ok(TerminalPollResult {
                changed: true,
                chunk: Some(chunk),
            });
        }
        sleep(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS)).await;
    }

    Ok(TerminalPollResult {
        changed: false,
        chunk: None,
    })
}

async fn cleanup_expired_sessions(state: &AppState) -> anyhow::Result<()> {
    let sessions = store_list_sessions(&state.redis).await?;
    let now = now_ms();
    for session in sessions {
        if parse_iso_ms(&session.expires_at).is_some_and(|expires_at| expires_at <= now) {
            if let Err(error) = terminal_kill_session(state, &session.id).await {
                tracing::warn!(session_id = %session.id, %error, "failed to cleanup expired terminal session");
            }
            continue;
        }
        if !tmux_session_exists(&session.backend_session_name).await {
            cleanup_session_artifacts(&session).await;
            store_delete_session(&state.redis, &session.id).await?;
            continue;
        }
        if session.status == "attached" {
            let attachments =
                store_list_attachment_ids_for_session(&state.redis, &session.id).await?;
            if attachments.is_empty() {
                mark_session_detached(state, &session.id).await?;
            }
        }
    }
    Ok(())
}

async fn assert_create_allowed(state: &AppState) -> anyhow::Result<()> {
    let status = runtime_status(state).await?;
    if status.blocked_reason.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(status.blocked_reason))
    }
}

async fn tmux_session_exists(session_name: &str) -> bool {
    if session_name.trim().is_empty() {
        return false;
    }
    run_tmux(&["has-session", "-t", session_name])
        .await
        .is_ok_and(|result| result.code == 0)
}

async fn read_pane_runtime_metadata(
    session: &TerminalSessionRecord,
) -> anyhow::Result<(String, i64, i64)> {
    let result = run_tmux(&[
        "display-message",
        "-p",
        "-t",
        &pane_target(session),
        "#{pane_tty}\t#{pane_width}\t#{pane_height}",
    ])
    .await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("paneMetadataReadFailed", &[])
            )
        ));
    }
    let mut parts = result.stdout.split('\t');
    let pane_tty_path = parts.next().unwrap_or("").trim().to_string();
    if pane_tty_path.is_empty() {
        return Err(anyhow!(terminal_default_text("paneTtyParseFailed", &[])));
    }
    let cols = parse_tmux_number(parts.next().unwrap_or(""), session.cols);
    let rows = parse_tmux_number(parts.next().unwrap_or(""), session.rows);
    Ok((pane_tty_path, cols, rows))
}

async fn is_relay_pipe_active(session: &TerminalSessionRecord) -> bool {
    let Ok(result) = run_tmux(&[
        "display-message",
        "-p",
        "-t",
        &pane_target(session),
        "#{?pane_pipe,1,0}",
    ])
    .await
    else {
        return false;
    };
    result.code == 0 && result.stdout.trim() == "1"
}

async fn ensure_output_log_path(
    state: &AppState,
    session: &TerminalSessionRecord,
) -> anyhow::Result<PathBuf> {
    ensure_stream_directory(state).await?;
    let path = if session.output_log_path.trim().is_empty() {
        build_output_log_path(&stream_directory(state), &session.id)
    } else {
        PathBuf::from(session.output_log_path.trim())
    };
    let _file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .with_context(|| format!("open terminal output log {}", path.display()))?;
    Ok(path)
}

async fn ensure_input_pipe_path(
    state: &AppState,
    session: &TerminalSessionRecord,
) -> anyhow::Result<PathBuf> {
    ensure_stream_directory(state).await?;
    let path = if session.input_pipe_path.trim().is_empty() {
        build_input_pipe_path(&stream_directory(state), &session.id)
    } else {
        PathBuf::from(session.input_pipe_path.trim())
    };
    if let Ok(metadata) = fs::metadata(&path).await {
        if is_fifo(&metadata) {
            return Ok(path);
        }
        let _ = fs::remove_file(&path).await;
    }
    let result = run_process("mkfifo", &[path_to_str(&path)?], None, true).await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("inputPipeCreateFailed", &[])
            )
        ));
    }
    Ok(path)
}

async fn configure_relay_pipe(
    session: &TerminalSessionRecord,
    output_log_path: &Path,
    input_pipe_path: &Path,
) -> anyhow::Result<()> {
    let relay = build_relay_command(output_log_path, input_pipe_path)?;
    let result = run_tmux(&["pipe-pane", "-I", "-O", "-t", &pane_target(session), &relay]).await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("ioRelayCreateFailed", &[])
            )
        ));
    }
    Ok(())
}

async fn configure_session_runtime(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let output_log_path = ensure_output_log_path(state, &session).await?;
    let input_pipe_path = ensure_input_pipe_path(state, &session).await?;
    let (pane_tty_path, cols, rows) = read_pane_runtime_metadata(&session).await?;
    configure_relay_pipe(&session, &output_log_path, &input_pipe_path).await?;
    store_save_session(
        &state.redis,
        normalize_session(TerminalSessionRecord {
            cols,
            rows,
            pane_tty_path,
            input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
            output_log_path: output_log_path.to_string_lossy().to_string(),
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
}

async fn ensure_session_runtime(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let stream_dir = stream_directory(state);
    let output_log_path = if session.output_log_path.trim().is_empty() {
        build_output_log_path(&stream_dir, &session.id)
    } else {
        PathBuf::from(session.output_log_path.trim())
    };
    let input_pipe_path = if session.input_pipe_path.trim().is_empty() {
        build_input_pipe_path(&stream_dir, &session.id)
    } else {
        PathBuf::from(session.input_pipe_path.trim())
    };
    let output_exists = fs::metadata(&output_log_path)
        .await
        .is_ok_and(|metadata| metadata.is_file());
    let input_exists = fs::metadata(&input_pipe_path)
        .await
        .is_ok_and(|metadata| is_fifo(&metadata));
    let relay_active = !session.pane_tty_path.trim().is_empty()
        && output_exists
        && input_exists
        && is_relay_pipe_active(&session).await;

    if relay_active {
        if !session.output_log_path.trim().is_empty() && !session.input_pipe_path.trim().is_empty()
        {
            return Ok(session);
        }
        return store_save_session(
            &state.redis,
            normalize_session(TerminalSessionRecord {
                input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
                output_log_path: output_log_path.to_string_lossy().to_string(),
                updated_at: now_iso(),
                ..session
            }),
        )
        .await;
    }

    configure_session_runtime(
        state,
        normalize_session(TerminalSessionRecord {
            input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
            output_log_path: output_log_path.to_string_lossy().to_string(),
            ..session
        }),
    )
    .await
}

async fn refresh_session_expiry(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let config = terminal_feature_config(state).await?;
    store_save_session(
        &state.redis,
        normalize_session(TerminalSessionRecord {
            updated_at: now_iso(),
            expires_at: iso_after_seconds(config.idle_timeout_seconds),
            ..session
        }),
    )
    .await
}

async fn touch_session_activity(
    state: &AppState,
    session: TerminalSessionRecord,
    force: bool,
) -> anyhow::Result<TerminalSessionRecord> {
    let now = now_ms();
    let next_allowed = {
        let deadlines = SESSION_TOUCH_DEADLINES.lock().await;
        deadlines.get(&session.id).copied().unwrap_or(0)
    };
    let normalized = normalize_session(TerminalSessionRecord {
        updated_at: now_iso(),
        ..session
    });
    if !force && now < next_allowed {
        return Ok(normalized);
    }
    let saved = refresh_session_expiry(state, normalized).await?;
    SESSION_TOUCH_DEADLINES
        .lock()
        .await
        .insert(saved.id.clone(), now + INPUT_SESSION_TOUCH_THROTTLE_MS);
    Ok(saved)
}

async fn mark_session_detached(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.redis, session_id).await? else {
        return Ok(None);
    };
    let saved = refresh_session_expiry(
        state,
        normalize_session(TerminalSessionRecord {
            status: "detached".to_string(),
            updated_at: now_iso(),
            last_detached_at: now_iso(),
            ..session
        }),
    )
    .await?;
    Ok(Some(saved))
}

async fn cleanup_session_artifacts(session: &TerminalSessionRecord) {
    SESSION_TOUCH_DEADLINES.lock().await.remove(&session.id);
    if !session.input_pipe_path.trim().is_empty() {
        let _ = fs::remove_file(session.input_pipe_path.trim()).await;
    }
    if !session.output_log_path.trim().is_empty() {
        let _ = fs::remove_file(session.output_log_path.trim()).await;
    }
}

async fn write_input_pipe(path: &str, data: Vec<u8>) -> io::Result<()> {
    let path = PathBuf::from(path.trim());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last_error: Option<io::Error> = None;
    while Instant::now() < deadline {
        let path_for_write = path.clone();
        let data_for_write = data.clone();
        match task::spawn_blocking(move || {
            write_input_pipe_blocking(path_for_write, data_for_write)
        })
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
        {
            Ok(()) => return Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                last_error = Some(error);
                sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| io::Error::other(terminal_default_text("inputPipeNotReady", &[]))))
}

#[cfg(unix)]
fn write_input_pipe_blocking(path: PathBuf, data: Vec<u8>) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    file.write_all(&data)
}

#[cfg(not(unix))]
fn write_input_pipe_blocking(path: PathBuf, data: Vec<u8>) -> io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.write_all(&data)
}

async fn read_output_chunk(
    session: &TerminalSessionRecord,
    requested_cursor: i64,
) -> anyhow::Result<Option<TerminalOutputChunk>> {
    let output_log_path = session.output_log_path.trim();
    let updated_at = now_iso();
    if output_log_path.is_empty() {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    }

    let Ok(metadata) = fs::metadata(output_log_path).await else {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    };
    if !metadata.is_file() {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    }
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    if requested_cursor <= 0 || requested_cursor > size {
        return capture_pane_snapshot_chunk(session, size, updated_at)
            .await
            .map(Some);
    }
    if requested_cursor >= size {
        return Ok(None);
    }

    let bytes_to_read = (size - requested_cursor).min(TERMINAL_STREAM_CHUNK_MAX_BYTES);
    let mut file = fs::File::open(output_log_path).await?;
    file.seek(std::io::SeekFrom::Start(requested_cursor as u64))
        .await?;
    let mut buffer = vec![0_u8; bytes_to_read as usize];
    let bytes_read = file.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(None);
    }
    buffer.truncate(bytes_read);
    Ok(Some(TerminalOutputChunk {
        cursor: requested_cursor + bytes_read as i64,
        data_base64: general_purpose::STANDARD.encode(&buffer),
        reset: false,
        updated_at,
    }))
}

async fn capture_pane_snapshot_chunk(
    session: &TerminalSessionRecord,
    cursor: i64,
    updated_at: String,
) -> anyhow::Result<TerminalOutputChunk> {
    let rows = session
        .rows
        .max((session.rows * 2).min(TERMINAL_SNAPSHOT_SCROLLBACK_ROWS));
    let result = run_tmux_raw(&[
        "capture-pane",
        "-p",
        "-e",
        "-t",
        &pane_target(session),
        "-S",
        &format!("-{rows}"),
    ])
    .await
    .ok();
    let snapshot = result
        .filter(|result| result.code == 0)
        .map(|result| normalize_pane_snapshot_output(&result.stdout))
        .unwrap_or_default();
    Ok(TerminalOutputChunk {
        cursor,
        data_base64: general_purpose::STANDARD.encode(snapshot.as_bytes()),
        reset: true,
        updated_at,
    })
}

fn normalize_pane_snapshot_output(output: &str) -> String {
    let trimmed = output.trim_end_matches([' ', '\t', '\r', '\n']);
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut normalized = String::with_capacity(trimmed.len());
        let mut chars = trimmed.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' && chars.peek().is_some_and(|next| *next == '\n') {
                chars.next();
                normalized.push_str("\r\n");
            } else if ch == '\n' {
                normalized.push_str("\r\n");
            } else {
                normalized.push(ch);
            }
        }
        normalized
    }
}

async fn store_list_sessions(redis: &RedisStore) -> anyhow::Result<Vec<TerminalSessionRecord>> {
    let ids = redis.zrevrange_strings(SESSION_INDEX_KEY).await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let keys = ids
        .iter()
        .map(|id| session_data_key(id))
        .collect::<Vec<_>>();
    let raws = redis.mget_string_values(&keys).await?;
    let mut sessions = Vec::new();
    let mut stale_ids = Vec::new();
    for (index, raw) in raws.into_iter().enumerate() {
        let Some(id) = ids.get(index) else {
            continue;
        };
        let Some(raw) = raw else {
            stale_ids.push(id.clone());
            continue;
        };
        match serde_json::from_str::<TerminalSessionRecord>(&raw) {
            Ok(session) => sessions.push(normalize_session(session)),
            Err(error) => {
                tracing::warn!(session_id = %id, %error, "failed to parse terminal session record");
                stale_ids.push(id.clone());
            }
        }
    }
    for id in stale_ids {
        redis
            .delete_string_and_zrem(&session_data_key(&id), SESSION_INDEX_KEY, &id)
            .await?;
    }
    Ok(sessions)
}

async fn store_get_session(
    redis: &RedisStore,
    id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(raw) = redis.get_string_value(&session_data_key(id)).await? else {
        return Ok(None);
    };
    match serde_json::from_str::<TerminalSessionRecord>(&raw) {
        Ok(session) => Ok(Some(normalize_session(session))),
        Err(error) => {
            tracing::warn!(session_id = %id, %error, "failed to parse terminal session record");
            redis
                .delete_string_and_zrem(&session_data_key(id), SESSION_INDEX_KEY, id)
                .await?;
            Ok(None)
        }
    }
}

async fn store_save_session(
    redis: &RedisStore,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let normalized = normalize_session(session);
    let value = serde_json::to_string(&normalized)?;
    let score = parse_iso_ms(&normalized.updated_at).unwrap_or_else(now_ms);
    redis
        .set_string_and_zadd(
            &session_data_key(&normalized.id),
            &value,
            SESSION_INDEX_KEY,
            &normalized.id,
            score,
        )
        .await?;
    Ok(normalized)
}

async fn store_delete_session(redis: &RedisStore, id: &str) -> anyhow::Result<()> {
    let attachment_ids = redis.smembers_strings(&session_attachments_key(id)).await?;
    let mut keys = vec![session_data_key(id), session_attachments_key(id)];
    keys.extend(
        attachment_ids
            .iter()
            .map(|attachment_id| attachment_data_key(attachment_id)),
    );
    redis.delete_keys(&keys).await?;
    redis.zrem_string_member(SESSION_INDEX_KEY, id).await?;
    Ok(())
}

async fn store_list_attachment_ids_for_session(
    redis: &RedisStore,
    session_id: &str,
) -> anyhow::Result<Vec<String>> {
    let ids = redis
        .smembers_strings(&session_attachments_key(session_id))
        .await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let keys = ids
        .iter()
        .map(|id| attachment_data_key(id))
        .collect::<Vec<_>>();
    let raws = redis.mget_string_values(&keys).await?;
    let mut live_ids = Vec::new();
    let mut stale_ids = Vec::new();
    for (index, raw) in raws.into_iter().enumerate() {
        let Some(id) = ids.get(index) else {
            continue;
        };
        let Some(raw) = raw else {
            stale_ids.push(id.clone());
            continue;
        };
        match serde_json::from_str::<TerminalAttachmentRecord>(&raw) {
            Ok(attachment) => {
                let normalized = normalize_attachment(attachment);
                if normalized.id.is_empty() {
                    stale_ids.push(id.clone());
                } else {
                    live_ids.push(id.clone());
                }
            }
            Err(error) => {
                tracing::warn!(attachment_id = %id, %error, "failed to parse terminal attachment record");
                stale_ids.push(id.clone());
            }
        }
    }
    for id in stale_ids {
        redis
            .delete_string_and_srem(
                &attachment_data_key(&id),
                &session_attachments_key(session_id),
                &id,
            )
            .await?;
    }
    Ok(live_ids)
}

async fn store_get_attachment(
    redis: &RedisStore,
    id: &str,
) -> anyhow::Result<Option<TerminalAttachmentRecord>> {
    let Some(raw) = redis.get_string_value(&attachment_data_key(id)).await? else {
        return Ok(None);
    };
    match serde_json::from_str::<TerminalAttachmentRecord>(&raw) {
        Ok(attachment) => Ok(Some(normalize_attachment(attachment))),
        Err(error) => {
            tracing::warn!(attachment_id = %id, %error, "failed to parse terminal attachment record");
            redis.delete_key(&attachment_data_key(id)).await?;
            Ok(None)
        }
    }
}

async fn store_save_attachment(
    redis: &RedisStore,
    attachment: TerminalAttachmentRecord,
    ttl_seconds: i64,
) -> anyhow::Result<TerminalAttachmentRecord> {
    let normalized = normalize_attachment(attachment);
    let value = serde_json::to_string(&normalized)?;
    let ttl = ttl_seconds.max(30) as usize;
    redis
        .save_expiring_string_and_sadd(
            &attachment_data_key(&normalized.id),
            &value,
            ttl,
            &session_attachments_key(&normalized.session_id),
            &normalized.id,
        )
        .await?;
    Ok(normalized)
}

async fn store_refresh_attachment(
    redis: &RedisStore,
    id: &str,
    ttl_seconds: i64,
) -> anyhow::Result<Option<TerminalAttachmentRecord>> {
    let Some(attachment) = store_get_attachment(redis, id).await? else {
        return Ok(None);
    };
    let next = normalize_attachment(TerminalAttachmentRecord {
        updated_at: now_iso(),
        expires_at: iso_after_seconds(ttl_seconds.max(30)),
        ..attachment
    });
    store_save_attachment(redis, next, ttl_seconds)
        .await
        .map(Some)
}

async fn store_delete_attachment(redis: &RedisStore, id: &str) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(redis, id).await? else {
        redis.delete_key(&attachment_data_key(id)).await?;
        return Ok(());
    };
    redis
        .delete_string_and_srem(
            &attachment_data_key(id),
            &session_attachments_key(&attachment.session_id),
            id,
        )
        .await?;
    Ok(())
}

fn normalize_session(mut session: TerminalSessionRecord) -> TerminalSessionRecord {
    let now = now_iso();
    session.id = clean_string(&session.id, "");
    session.cwd = clean_string(&session.cwd, "~");
    let default_title = terminal_default_text("defaultTitle", &[]);
    let title_fallback = path_basename(&session.cwd)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_title.as_str());
    session.title = clean_string(&session.title, title_fallback);
    if !matches!(
        session.status.as_str(),
        "attached" | "detached" | "stopped" | "error"
    ) {
        session.status = "created".to_string();
    }
    session.created_at = normalize_iso(&session.created_at).unwrap_or_else(|| now.clone());
    session.updated_at = normalize_iso(&session.updated_at).unwrap_or_else(|| now.clone());
    session.last_attached_at = normalize_iso(&session.last_attached_at).unwrap_or_default();
    session.last_detached_at = normalize_iso(&session.last_detached_at).unwrap_or_default();
    session.last_client_ip = clean_string(&session.last_client_ip, "");
    session.shell = clean_string(
        &session.shell,
        &env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
    );
    session.cols = if session.cols <= 0 { 120 } else { session.cols }.clamp(20, 400);
    session.rows = if session.rows <= 0 { 32 } else { session.rows }.clamp(8, 200);
    session.resume_backend = "tmux".to_string();
    session.backend_session_name = clean_string(&session.backend_session_name, "");
    session.pane_tty_path = clean_string(&session.pane_tty_path, "");
    session.input_pipe_path = clean_string(&session.input_pipe_path, "");
    session.output_log_path = clean_string(&session.output_log_path, "");
    session.expires_at = normalize_iso(&session.expires_at).unwrap_or_default();
    session.last_frame_revision = clean_string(&session.last_frame_revision, "");
    session
}

fn normalize_attachment(mut attachment: TerminalAttachmentRecord) -> TerminalAttachmentRecord {
    let now = now_iso();
    attachment.id = clean_string(&attachment.id, "");
    attachment.session_id = clean_string(&attachment.session_id, "");
    attachment.transport = "http-polling".to_string();
    attachment.created_at = normalize_iso(&attachment.created_at).unwrap_or_else(|| now.clone());
    attachment.updated_at = normalize_iso(&attachment.updated_at).unwrap_or_else(|| now.clone());
    attachment.expires_at = normalize_iso(&attachment.expires_at).unwrap_or(now);
    attachment
}

fn normalize_terminal_dimension(value: Option<f64>, fallback: i64, min: i64, max: i64) -> i64 {
    let selected = match value {
        Some(value) if value.is_finite() && value != 0.0 => value,
        _ => fallback as f64,
    };
    let floored = selected.floor();
    let parsed = if floored <= i64::MIN as f64 {
        i64::MIN
    } else if floored >= i64::MAX as f64 {
        i64::MAX
    } else {
        floored as i64
    };
    parsed.clamp(min, max)
}

fn normalize_terminal_poll_timeout_ms(value: Option<f64>) -> u64 {
    let selected = match value {
        Some(value) if value.is_finite() && value != 0.0 => value,
        _ => DEFAULT_POLL_TIMEOUT_MS as f64,
    };
    selected.clamp(1_000.0, 20_000.0).floor() as u64
}

fn parse_output_cursor_like_node(value: Option<&str>) -> i64 {
    let Some(parsed) = parse_i64_prefix(value.unwrap_or("").trim_start()) else {
        return 0;
    };
    parsed.max(0)
}

fn parse_i64_prefix(value: &str) -> Option<i64> {
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }

    let mut end = 0;
    let mut has_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }

    value[..end].parse::<i64>().ok()
}

fn clean_string(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_iso(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    parse_iso_ms(trimmed).map(|_| trimmed.to_string())
}

fn session_data_key(id: &str) -> String {
    format!("{SESSION_DATA_PREFIX}{id}")
}

fn session_attachments_key(session_id: &str) -> String {
    format!("{SESSION_ATTACHMENTS_PREFIX}{session_id}")
}

fn attachment_data_key(id: &str) -> String {
    format!("{ATTACHMENT_DATA_PREFIX}{id}")
}

fn stream_directory(state: &AppState) -> PathBuf {
    state.settings.data_dir.join(TERMINAL_STREAM_DIR_NAME)
}

async fn ensure_stream_directory(state: &AppState) -> anyhow::Result<()> {
    fs::create_dir_all(stream_directory(state)).await?;
    Ok(())
}

fn build_session_name(id: &str) -> String {
    let compact = id.replace('-', "");
    format!("fnk_{}", compact.chars().take(16).collect::<String>())
}

fn build_output_log_path(stream_directory: &Path, id: &str) -> PathBuf {
    stream_directory.join(format!("{id}.log"))
}

fn build_input_pipe_path(stream_directory: &Path, id: &str) -> PathBuf {
    stream_directory.join(format!("{id}.in"))
}

fn pane_target(session: &TerminalSessionRecord) -> String {
    format!(
        "{}{}",
        session.backend_session_name, TMUX_TARGET_PANE_SUFFIX
    )
}

fn sanitize_title(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn build_default_session_title(
    existing_sessions: &[TerminalSessionRecord],
    translator: &Translator,
) -> String {
    let prefix = terminal_text(translator, "defaultSessionTitlePrefix", &[]);
    let mut used = BTreeSet::new();
    for session in existing_sessions {
        let title = session.title.trim();
        let suffix = title
            .strip_prefix(&prefix)
            .or_else(|| title.strip_prefix(LEGACY_DEFAULT_SESSION_TITLE_PREFIX));
        let Some(suffix) = suffix else { continue };
        if let Ok(index) = suffix.parse::<i64>() {
            if index > 0 {
                used.insert(index);
            }
        }
    }
    let mut next = 1;
    while used.contains(&next) {
        next += 1;
    }
    format!("{prefix}{next}")
}

async fn resolve_shell(shell: Option<&str>) -> anyhow::Result<String> {
    let requested = shell.map(str::trim).filter(|value| !value.is_empty());
    if let Some(requested) = requested {
        if can_start_shell(requested).await {
            return Ok(requested.to_string());
        }
        return Err(anyhow!(terminal_default_text(
            "requestedShellUnavailable",
            &[("shell", requested.to_string())],
        )));
    }

    for candidate in auto_shell_candidates() {
        if can_start_shell(&candidate).await {
            return Ok(candidate);
        }
    }
    Err(anyhow!(terminal_default_text("noShellDetected", &[])))
}

fn auto_shell_candidates() -> Vec<String> {
    auto_shell_candidates_from_env(&env::var("SHELL").unwrap_or_default())
}

fn auto_shell_candidates_from_env(env_shell: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if is_zsh_shell(env_shell) {
        candidates.push(env_shell.to_string());
    }
    candidates.extend(["zsh", "/bin/zsh", "/usr/bin/zsh"].map(String::from));
    candidates.push(env_shell.to_string());
    candidates.extend(
        [
            "bash",
            "/bin/bash",
            "/usr/bin/bash",
            "sh",
            "/bin/sh",
            "/usr/bin/sh",
        ]
        .map(String::from),
    );
    dedupe_strings(candidates)
}

async fn can_start_shell(command: &str) -> bool {
    run_process(command, &["-c", "exit 0"], None, true)
        .await
        .is_ok_and(|result| result.code == 0)
}

fn build_session_shell_command(shell: &str) -> String {
    if is_zsh_shell(shell) {
        format!("exec {} -il", shell_quote(shell))
    } else {
        format!("exec {}", shell_quote(shell))
    }
}

fn is_zsh_shell(shell: &str) -> bool {
    Path::new(shell)
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("zsh"))
}

async fn resolve_cwd(config: &TerminalFeatureConfig, cwd: Option<&str>) -> anyhow::Result<PathBuf> {
    let configured = normalize_terminal_default_cwd(Some(&config.default_cwd));
    let requested = cwd.map(str::trim).filter(|value| !value.is_empty());
    let next = requested
        .map(|value| normalize_terminal_default_cwd(Some(value)))
        .unwrap_or(configured);
    let next = next.trim();
    let resolved = if next.is_empty() || next == "~" {
        home_dir()
    } else if let Some(rest) = next.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(next)
    };
    let metadata = fs::metadata(&resolved)
        .await
        .with_context(|| format!("working directory is unavailable: {}", resolved.display()))?;
    if metadata.is_dir() {
        Ok(resolved)
    } else {
        Err(anyhow!(
            "working directory is unavailable: {}",
            resolved.display()
        ))
    }
}

fn home_dir() -> PathBuf {
    let env_home = env::var("HOME").ok();
    let platform_home = platform_home_dir();
    let current_dir = env::current_dir().ok();
    resolve_home_dir(env_home.as_deref(), platform_home, current_dir.as_deref())
}

fn resolve_home_dir(
    env_home: Option<&str>,
    platform_home: Option<PathBuf>,
    current_dir: Option<&Path>,
) -> PathBuf {
    if let Some(home) = env_home.map(str::trim).filter(|value| !value.is_empty()) {
        let env_home = PathBuf::from(home);
        if (current_dir.is_some_and(|cwd| cwd == env_home) || is_terminal_runtime_cwd(home))
            && let Some(platform_home) = platform_home.as_ref()
            && !platform_home.as_os_str().is_empty()
            && platform_home != &env_home
        {
            return platform_home.clone();
        }
        return env_home;
    }
    platform_home
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(unix)]
fn platform_home_dir() -> Option<PathBuf> {
    unsafe {
        let passwd = libc::getpwuid(libc::geteuid());
        if passwd.is_null() || (*passwd).pw_dir.is_null() {
            return None;
        }
        let value = std::ffi::CStr::from_ptr((*passwd).pw_dir)
            .to_string_lossy()
            .trim()
            .to_string();
        (!value.is_empty()).then(|| PathBuf::from(value))
    }
}

#[cfg(not(unix))]
fn platform_home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE").ok().map(PathBuf::from).or_else(|| {
        let drive = env::var("HOMEDRIVE").ok()?;
        let path = env::var("HOMEPATH").ok()?;
        Some(PathBuf::from(format!("{drive}{path}")))
    })
}

fn path_basename(path: &str) -> Option<&str> {
    Path::new(path).file_name().and_then(|value| value.to_str())
}

fn path_to_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))
}

fn parse_tmux_number(value: &str, fallback: i64) -> i64 {
    value.trim().parse::<i64>().unwrap_or(fallback)
}

fn fallback_message<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            result.push(trimmed.to_string());
        }
    }
    result
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn build_relay_command(output_log_path: &Path, input_pipe_path: &Path) -> anyhow::Result<String> {
    let log = shell_quote(path_to_str(output_log_path)?);
    let input = shell_quote(path_to_str(input_pipe_path)?);
    Ok(format!(
        "sh -c 'log=$1; input=$2; exec 3<> \"$input\"; cat <&3 & input_pid=$!; cat >> \"$log\"; kill \"$input_pid\" 2>/dev/null || true' fnk-relay {log} {input}"
    ))
}

fn detect_client_ip(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    {
        if let Some(first) = forwarded
            .split(',')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return first.to_string();
        }
    }
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(unix)]
fn is_fifo(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;
    metadata.file_type().is_fifo()
}

#[cfg(not(unix))]
fn is_fifo(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() == 0 }
}

#[cfg(not(unix))]
fn is_running_as_root() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_session_name_matches_node_prefix_and_length() {
        assert_eq!(
            build_session_name("12345678-90ab-cdef-1111-222233334444"),
            "fnk_1234567890abcdef"
        );
    }

    #[test]
    fn default_session_title_skips_used_indexes() {
        let translator = Translator::new("zh-CN");
        let sessions = vec![
            TerminalSessionRecord {
                title: "会话-1".to_string(),
                ..Default::default()
            },
            TerminalSessionRecord {
                title: "Terminal Session 3".to_string(),
                ..Default::default()
            },
        ];
        assert_eq!(
            build_default_session_title(&sessions, &translator),
            "会话-2"
        );
    }

    #[test]
    fn normalize_session_default_title_and_shape_match_node() {
        let session = normalize_session(TerminalSessionRecord {
            cwd: "/".to_string(),
            ..Default::default()
        });
        assert_eq!(session.title, terminal_default_text("defaultTitle", &[]));

        let value = serde_json::to_value(&session).expect("serialize terminal session");
        assert_eq!(
            value.get("last_frame_revision").and_then(Value::as_str),
            Some("")
        );
    }

    #[test]
    fn pane_snapshot_output_normalizes_crlf_like_node_regex() {
        assert_eq!(normalize_pane_snapshot_output("a\nb\n"), "a\r\nb");
        assert_eq!(normalize_pane_snapshot_output("a\r\nb\r\n"), "a\r\nb");
        assert_eq!(normalize_pane_snapshot_output("a\rb"), "a\rb");
        assert_eq!(normalize_pane_snapshot_output("  \r\n\t"), "");
    }

    #[test]
    fn normalizes_terminal_feature_like_node() {
        let value = serde_json::json!({
            "enabled": true,
            "default_cwd": "",
            "max_sessions": 99,
            "idle_timeout_seconds": 1,
            "allow_mobile_toolbar": false,
            "dangerously_run_as_current_user": false
        });
        assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 12);
        assert_eq!(
            normalize_terminal_feature(Some(&value)).idle_timeout_seconds,
            60
        );
        assert_eq!(normalize_terminal_feature(Some(&value)).default_cwd, "~");
        assert!(!normalize_terminal_feature(Some(&value)).allow_mobile_toolbar);

        let value = serde_json::json!({
            "max_sessions": "2x",
            "idle_timeout_seconds": "90.8"
        });
        assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 2);
        assert_eq!(
            normalize_terminal_feature(Some(&value)).idle_timeout_seconds,
            90
        );

        let value = serde_json::json!({
            "max_sessions": ["4.9"]
        });
        assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 4);
    }

    #[test]
    fn normalizes_terminal_runtime_default_cwd_to_home_marker() {
        let value = serde_json::json!({
            "default_cwd": "/usr/local/etc/fn-knock/"
        });
        assert_eq!(normalize_terminal_feature(Some(&value)).default_cwd, "~");
    }

    #[test]
    fn auto_shell_candidates_prefer_zsh_like_node() {
        assert_eq!(
            auto_shell_candidates_from_env("/bin/bash"),
            vec![
                "zsh",
                "/bin/zsh",
                "/usr/bin/zsh",
                "/bin/bash",
                "bash",
                "/usr/bin/bash",
                "sh",
                "/bin/sh",
                "/usr/bin/sh",
            ]
        );
        assert_eq!(
            auto_shell_candidates_from_env("/opt/homebrew/bin/zsh"),
            vec![
                "/opt/homebrew/bin/zsh",
                "zsh",
                "/bin/zsh",
                "/usr/bin/zsh",
                "bash",
                "/bin/bash",
                "/usr/bin/bash",
                "sh",
                "/bin/sh",
                "/usr/bin/sh",
            ]
        );
        assert_eq!(
            auto_shell_candidates_from_env("/bin/zsh"),
            vec![
                "/bin/zsh",
                "zsh",
                "/usr/bin/zsh",
                "bash",
                "/bin/bash",
                "/usr/bin/bash",
                "sh",
                "/bin/sh",
                "/usr/bin/sh",
            ]
        );
    }

    #[test]
    fn zsh_session_command_uses_login_interactive_shell_like_node() {
        assert_eq!(
            build_session_shell_command("/bin/zsh"),
            "exec '/bin/zsh' -il"
        );
        assert_eq!(build_session_shell_command("/bin/bash"), "exec '/bin/bash'");
    }

    #[test]
    fn terminal_dimensions_match_node_number_rules() {
        assert_eq!(normalize_terminal_dimension(None, 120, 40, 400), 120);
        assert_eq!(normalize_terminal_dimension(Some(0.0), 120, 40, 400), 120);
        assert_eq!(normalize_terminal_dimension(Some(80.9), 120, 40, 400), 80);
        assert_eq!(normalize_terminal_dimension(Some(-1.2), 120, 40, 400), 40);
        assert_eq!(normalize_terminal_dimension(Some(999.0), 120, 40, 400), 400);
    }

    #[test]
    fn terminal_poll_timeout_matches_node_default_and_clamp_rules() {
        assert_eq!(
            normalize_terminal_poll_timeout_ms(None),
            DEFAULT_POLL_TIMEOUT_MS
        );
        assert_eq!(
            normalize_terminal_poll_timeout_ms(Some(0.0)),
            DEFAULT_POLL_TIMEOUT_MS
        );
        assert_eq!(normalize_terminal_poll_timeout_ms(Some(500.0)), 1_000);
        assert_eq!(normalize_terminal_poll_timeout_ms(Some(1500.8)), 1_500);
        assert_eq!(normalize_terminal_poll_timeout_ms(Some(30_000.0)), 20_000);
    }

    #[test]
    fn output_cursor_parser_matches_node_parse_int_edges() {
        assert_eq!(parse_output_cursor_like_node(None), 0);
        assert_eq!(parse_output_cursor_like_node(Some("")), 0);
        assert_eq!(parse_output_cursor_like_node(Some("   ")), 0);
        assert_eq!(parse_output_cursor_like_node(Some("2x")), 2);
        assert_eq!(parse_output_cursor_like_node(Some("  +3.9")), 3);
        assert_eq!(parse_output_cursor_like_node(Some("-1")), 0);
    }

    #[test]
    fn home_dir_resolution_matches_node_homedir_fallback() {
        assert_eq!(
            resolve_home_dir(
                Some(" /home/fn "),
                Some(PathBuf::from("/root")),
                Some(Path::new("/srv/fn-knock")),
            ),
            PathBuf::from("/home/fn")
        );
        assert_eq!(
            resolve_home_dir(None, Some(PathBuf::from("/root")), None),
            PathBuf::from("/root")
        );
        assert_eq!(
            resolve_home_dir(Some(""), Some(PathBuf::from("/root")), None),
            PathBuf::from("/root")
        );
        assert_eq!(resolve_home_dir(None, None, None), PathBuf::from("/"));
    }

    #[test]
    fn home_dir_prefers_account_home_when_env_home_is_runtime_directory() {
        assert_eq!(
            resolve_home_dir(
                Some("/usr/local/etc/fn-knock"),
                Some(PathBuf::from("/root")),
                Some(Path::new("/usr/local/etc/fn-knock")),
            ),
            PathBuf::from("/root")
        );
    }

    #[test]
    fn localizes_terminal_error_from_default_locale() {
        let translator = Translator::new("en");
        let raw = terminal_default_text("sessionLimitReached", &[("count", "3".to_string())]);
        assert_eq!(
            localize_terminal_error(&translator, &raw),
            "Terminal session limit reached (3)"
        );

        let raw = format!(
            "{}: broken pipe",
            terminal_default_text("inputSendFailed", &[])
        );
        assert_eq!(
            localize_terminal_error(&translator, &raw),
            "Failed to send terminal input: broken pipe"
        );
    }

    #[test]
    fn tmux_install_state_defaults_use_server_locale_messages() {
        let state = default_tmux_install_state();
        assert_eq!(state.message, "未检测到 tmux，请先安装 tmux 环境");

        let ready = terminal_default_text(
            "tmuxInstallCompleteWithVersion",
            &[("version", "tmux 3.4".to_string())],
        );
        assert_eq!(ready, "tmux 安装完成：tmux 3.4");
    }

    #[test]
    fn tmux_error_state_message_is_not_double_wrapped() {
        let translator = Translator::new("en");
        let mut state = TerminalTmuxInstallState {
            status: "error".to_string(),
            progress: 0,
            message: format!("{}: broken", terminal_default_text("aptUpdateFailed", &[])),
            executable_path: String::new(),
            detection_source: None,
            version: String::new(),
        };
        localize_tmux_install_state(&mut state, &translator);
        assert_eq!(state.message, "apt-get update failed: broken");

        let blocked_reason = translator.t_params(
            "server.terminal.tmuxStatusError",
            &[("message", state.message.clone())],
        );
        assert_eq!(
            blocked_reason,
            "tmux status error: apt-get update failed: broken"
        );
    }

    #[test]
    fn localizes_terminal_error_from_english_and_wraps_unknown() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_terminal_error(&translator, "Failed to resize terminal: tmux failed"),
            "终端尺寸调整失败: tmux failed"
        );
        assert_eq!(
            localize_terminal_error(&translator, "run process tmux: No such file or directory"),
            "终端操作失败：run process tmux: No such file or directory"
        );
    }

    #[test]
    fn relay_command_uses_shell_paths_without_node_runtime() {
        let command = build_relay_command(Path::new("/tmp/a b.log"), Path::new("/tmp/a.in"))
            .expect("relay command");
        assert!(command.starts_with("sh -c "));
        assert!(!command.contains("node"));
        assert!(command.contains("cat >>"));
    }
}
