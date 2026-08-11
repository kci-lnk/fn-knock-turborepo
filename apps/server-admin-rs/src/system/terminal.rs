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
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    i18n::Translator,
    response, runtime_profile,
    state::AppState,
    store::Store,
    terminal_paths::{is_terminal_runtime_cwd, normalize_terminal_default_cwd},
    time_utils::{iso_after_seconds, now_iso, now_ms, parse_iso_ms},
};

mod handlers;
mod normalize_utils;
mod sessions;
mod store;
mod tmux_runtime;

use handlers::*;
use normalize_utils::*;
use sessions::*;
use store::*;
use tmux_runtime::*;

#[cfg(test)]
mod tests;

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

#[derive(Deserialize, utoipa::ToSchema)]
struct RenameSessionBody {
    title: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct InputBody {
    #[serde(rename = "dataBase64")]
    data_base64: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
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
    let terminal_runtime_routes: Router<AppState> = terminal_runtime_routes().into();
    Router::new().merge(terminal_runtime_routes)
}

pub(crate) fn terminal_runtime_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(install_tmux))
        .routes(routes!(list_sessions))
        .routes(routes!(create_session))
        .routes(routes!(get_session))
        .routes(routes!(rename_session))
        .routes(routes!(delete_session))
        .routes(routes!(create_attachment))
        .routes(routes!(poll_attachment))
        .routes(routes!(send_input))
        .routes(routes!(resize_attachment))
        .routes(routes!(delete_attachment))
}

pub fn start_terminal_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("terminal-session-cleanup", async move {
        let mut ticker = interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                result = cleanup_expired_sessions(&task_state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "failed to cleanup expired terminal sessions");
                    }
                }
            }
        }
    });
}
