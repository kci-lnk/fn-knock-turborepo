use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalTmuxInstallStateData {
    status: String,
    progress: i64,
    message: String,
    executable_path: String,
    #[schema(required = true)]
    detection_source: Option<String>,
    version: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalRuntimeStatusData {
    enabled: bool,
    tmux_available: bool,
    tmux_executable_path: String,
    #[schema(required = true)]
    tmux_detection_source: Option<String>,
    tmux_version: String,
    tmux_install_state: TerminalTmuxInstallStateData,
    http_polling_available: bool,
    running_as_root: bool,
    blocked_reason: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalSessionData {
    id: String,
    title: String,
    status: String,
    created_at: String,
    updated_at: String,
    last_attached_at: String,
    last_detached_at: String,
    last_client_ip: String,
    shell: String,
    cwd: String,
    cols: i64,
    rows: i64,
    resume_backend: String,
    backend_session_name: String,
    pane_tty_path: String,
    input_pipe_path: String,
    output_log_path: String,
    expires_at: String,
    last_frame_revision: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalAttachmentData {
    id: String,
    session_id: String,
    transport: String,
    created_at: String,
    updated_at: String,
    expires_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalOutputChunkData {
    cursor: i64,
    data_base64: String,
    reset: bool,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalPollResultData {
    changed: bool,
    #[schema(required = true)]
    chunk: Option<TerminalOutputChunkData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalCreateSessionBodyData {
    title: Option<String>,
    shell: Option<String>,
    cwd: Option<String>,
    cols: Option<f64>,
    rows: Option<f64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalRenameSessionBodyData {
    title: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct TerminalInputBodyData {
    data_base64: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TerminalResizeBodyData {
    cols: f64,
    rows: f64,
}
