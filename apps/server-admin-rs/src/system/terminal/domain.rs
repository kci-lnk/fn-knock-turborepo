use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    Password,
    PrivateKey,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedHostKey {
    pub algorithm: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub trusted_host_key: Option<TrustedHostKey>,
    pub revision: u64,
    pub last_verified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTarget {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub trusted_host_key: Option<TrustedHostKey>,
    pub credential_configured: bool,
    pub passphrase_configured: bool,
    pub revision: u64,
    pub last_verified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SecretAction {
    Keep,
    Replace,
    Clear,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMutation {
    pub action: SecretAction,
    #[schema(write_only)]
    pub secret: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseMutation {
    pub action: SecretAction,
    #[schema(write_only)]
    pub secret: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TargetCreateInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub trusted_host_key: Option<TrustedHostKey>,
    pub credential: CredentialMutation,
    pub passphrase: PassphraseMutation,
    /// One-time proof returned by a successful connection test for this exact
    /// draft and credential set.
    pub verification_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TargetUpdateInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub trusted_host_key: Option<TrustedHostKey>,
    pub revision: u64,
    pub credential: CredentialMutation,
    pub passphrase: PassphraseMutation,
    pub verification_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TargetDraft {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub trusted_host_key: Option<TrustedHostKey>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProbeHostKeyInput {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyProbeResult {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTestConnectionInput {
    pub target_id: Option<String>,
    pub draft: Option<TargetDraft>,
    pub credential: CredentialMutation,
    pub passphrase: PassphraseMutation,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub verification_token: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionBackend {
    Ssh,
    Local,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalTerminalBlockedReason {
    UnsupportedPlatform,
    ShellUnavailable,
}

#[derive(Clone, Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTerminalStatus {
    pub target_id: String,
    pub supported: bool,
    pub enabled: bool,
    pub ready: bool,
    pub execution_identity: String,
    pub privileged: bool,
    pub shell: Option<String>,
    pub working_directory: Option<String>,
    pub blocked_reason: Option<LocalTerminalBlockedReason>,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocalTerminalSettingsInput {
    pub enabled: bool,
    pub revision: u64,
    pub acknowledge_risk: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionPhase {
    Creating,
    OpeningPty,
    StartingShell,
    Resolving,
    Connecting,
    VerifyingHostKey,
    Authenticating,
    OpeningChannel,
    RequestingPty,
    Running,
    Closing,
    Closed,
    Exited,
    Lost,
    Failed,
}

impl SessionPhase {
    pub fn is_active(self) -> bool {
        !matches!(
            self,
            Self::Closed | Self::Exited | Self::Lost | Self::Failed
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use SessionPhase::*;
        matches!(
            (self, next),
            (Creating, OpeningPty)
                | (OpeningPty, StartingShell)
                | (StartingShell, Running)
                | (Creating, Resolving)
                | (Resolving, Connecting)
                | (Connecting, VerifyingHostKey)
                | (VerifyingHostKey, Authenticating)
                | (Authenticating, OpeningChannel)
                | (OpeningChannel, RequestingPty)
                | (RequestingPty, Running)
                | (Running, Closing | Exited | Lost)
                | (Closing, Closed | Lost)
                | (
                    Creating
                        | OpeningPty
                        | StartingShell
                        | Resolving
                        | Connecting
                        | VerifyingHostKey
                        | Authenticating
                        | OpeningChannel
                        | RequestingPty,
                    Failed | Closing
                )
        )
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    pub id: String,
    pub target_id: String,
    pub backend: SessionBackend,
    pub title: String,
    pub phase: SessionPhase,
    pub cols: u32,
    pub rows: u32,
    pub created_at: String,
    pub updated_at: String,
    pub error_code: Option<TerminalErrorCode>,
    pub error_message: Option<String>,
    pub exit_code: Option<u32>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResult {
    pub runtime_id: String,
    pub sessions: Vec<TerminalSession>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionInput {
    pub title: Option<String>,
    pub cols: Option<u32>,
    pub rows: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenameSessionInput {
    pub title: String,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAttachmentInput {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentRole {
    Controller,
    Viewer,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema, PartialEq, Eq)]
pub enum TerminalTransport {
    #[serde(rename = "http-polling")]
    HttpPolling,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalAttachment {
    pub id: String,
    pub session_id: String,
    pub role: AttachmentRole,
    pub transport: TerminalTransport,
    pub generation: u64,
    pub cursor: u64,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputRequest {
    pub data_base64: String,
    pub sequence: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResizeRequest {
    pub cols: u32,
    pub rows: u32,
    pub revision: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimControlRequest {
    pub generation: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct EventsQuery {
    pub after: Option<u64>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct ForceQuery {
    #[serde(default)]
    pub force: bool,
    pub confirmation_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct TargetDeleteQuery {
    pub revision: u64,
    #[serde(default)]
    pub force: bool,
    pub confirmation_token: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TerminalEventType {
    Output,
    Status,
    Control,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEvent {
    #[serde(rename = "type")]
    pub kind: TerminalEventType,
    pub cursor: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<SessionPhase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<TerminalErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<AttachmentRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventsResult {
    pub events: Vec<TerminalEvent>,
    pub next_cursor: u64,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalErrorCode {
    InvalidRequest,
    TargetNotFound,
    SessionNotFound,
    HostKeyRequired,
    HostKeyMismatch,
    AuthenticationFailed,
    PtyRejected,
    SessionLimitReached,
    SessionLost,
    AttachmentExpired,
    ControllerConflict,
    TargetRevisionConflict,
    LocalTerminalUnsupported,
    LocalTerminalDisabled,
    LocalTerminalRiskAcknowledgementRequired,
    LocalTerminalRevisionConflict,
    LocalShellUnavailable,
    LocalPtyStartFailed,
    ConnectTimeout,
    Conflict,
    UpstreamUnavailable,
    InternalError,
}

impl fmt::Display for TerminalErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "internal_error".to_string());
        formatter.write_str(&value)
    }
}

#[derive(Clone, Debug)]
pub struct TerminalError {
    pub code: TerminalErrorCode,
    pub message: String,
    pub active_session_count: Option<usize>,
    pub confirmation_token: Option<String>,
}

impl TerminalError {
    pub fn new(code: TerminalErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            active_session_count: None,
            confirmation_token: None,
        }
    }

    pub fn with_active_session_count(mut self, count: usize) -> Self {
        self.active_session_count = Some(count);
        self
    }

    pub fn with_confirmation_token(mut self, token: String) -> Self {
        self.confirmation_token = Some(token);
        self
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(TerminalErrorCode::InvalidRequest, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(TerminalErrorCode::InternalError, message)
    }
}

impl fmt::Display for TerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TerminalError {}

pub type TerminalResult<T> = Result<T, TerminalError>;
