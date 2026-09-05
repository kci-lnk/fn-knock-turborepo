use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    sync::{Arc, LazyLock, Weak},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use tokio::{
    sync::{Mutex, Notify, RwLock, Semaphore, mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};
use uuid::Uuid;

use crate::state::AppState;
use crate::time_utils::{iso_after_seconds, now_iso};

use super::{
    domain::{
        AttachmentRole, EventsResult, SessionBackend, SessionListResult, SessionPhase,
        TerminalAttachment, TerminalError, TerminalErrorCode, TerminalEvent, TerminalEventType,
        TerminalResult, TerminalSession, TerminalTransport,
    },
    local::{self, LOCAL_TARGET_ID, LocalTerminalDescriptor},
    shell::{BoxedShell, ShellEvent},
    ssh::{RusshConnector, SshConnector, SshCredential},
};

pub const MAX_TARGETS: usize = 100;
pub const MAX_GLOBAL_SESSIONS: usize = 12;
pub const MAX_TARGET_SESSIONS: usize = 4;
const MAX_RETAINED_SESSIONS: usize = 100;
const MAX_ATTACHMENTS: usize = 4;
const OUTPUT_CAPACITY_BYTES: usize = 4 * 1024 * 1024;
const OUTPUT_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const SCROLLBACK_ROWS: usize = 2_000;
const CLOSED_TERMINAL_COMPACT_AFTER: Duration = Duration::from_secs(120);
const MAX_ARCHIVED_TERMINAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_TERMINAL_SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;
static TERMINAL_ARCHIVE_WORK: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));
const ATTACHMENT_TTL_SECONDS: i64 = 120;
const MAX_POLL_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_QUEUE_CAPACITY: usize = 128;
const VERIFICATION_TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_VERIFICATION_GRANTS: usize = 1_024;
const FORCE_CONFIRMATION_TTL: Duration = Duration::from_secs(2 * 60);
const MAX_FORCE_CONFIRMATION_GRANTS: usize = 1_024;
const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

pub struct TerminalRuntime {
    pub(super) access: super::access::AccessRuntime,
    runtime_id: String,
    sessions: Arc<RwLock<HashMap<String, Arc<RuntimeSession>>>>,
    actor_tasks: Mutex<HashMap<String, JoinHandle<()>>>,
    target_operations: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    catalog_operation: Mutex<()>,
    quota_operation: Mutex<()>,
    connector: Arc<dyn SshConnector>,
    verification_grants: Mutex<HashMap<String, VerificationGrant>>,
    force_confirmation_grants: Mutex<HashMap<String, ForceConfirmationGrant>>,
}

struct VerificationGrant {
    fingerprint: String,
    expires_at: Instant,
}

struct ForceConfirmationGrant {
    target_id: String,
    target_revision: u64,
    session_ids: Vec<String>,
    expires_at: Instant,
}

struct RuntimeSession {
    state: Mutex<SessionState>,
    api_operation: Mutex<()>,
    output_notify: Notify,
    commands: mpsc::Sender<SessionCommand>,
    cancel: CancellationToken,
}

struct SessionState {
    session: TerminalSession,
    output: OutputBuffer,
    terminal: RuntimeTerminal,
    closed_at: Option<Instant>,
    attachments: HashMap<String, AttachmentState>,
    controller_id: Option<String>,
    controller_generation: u64,
}

struct AttachmentState {
    role: AttachmentRole,
    generation: u64,
    expires_at: String,
    last_seen: Instant,
    needs_snapshot: bool,
    last_input_sequence: u64,
    last_resize_revision: u64,
    control_dirty: bool,
    snapshot: Option<SnapshotTransfer>,
    transport_cursor: u64,
    output_cursor: u64,
    pending_poll: Option<PendingPoll>,
}

struct SnapshotTransfer {
    data: Arc<Bytes>,
    cursor: u64,
    index: usize,
}

impl SnapshotTransfer {
    fn chunk_count(&self) -> usize {
        self.data.len().div_ceil(OUTPUT_RESPONSE_BYTES).max(1)
    }
}

struct PendingPoll {
    request_cursor: u64,
    result: EventsResult,
    next_output_cursor: u64,
    advance_snapshot: bool,
    control: Option<(AttachmentRole, u64)>,
}

struct BufferedEvent {
    cursor: u64,
    kind: TerminalEventType,
    data: Option<Vec<u8>>,
    phase: Option<SessionPhase>,
    error_code: Option<TerminalErrorCode>,
    error_message: Option<String>,
    exit_code: Option<u32>,
}

pub(super) struct PendingSession {
    id: String,
    runtime: Arc<RuntimeSession>,
    receiver: Option<mpsc::Receiver<SessionCommand>>,
    sessions: Arc<RwLock<HashMap<String, Arc<RuntimeSession>>>>,
    activated: bool,
}

pub(super) struct SessionStartup {
    pub pending: PendingSession,
    pub backend: SessionStartupBackend,
    pub initial_cols: u32,
    pub initial_rows: u32,
    pub shutdown: CancellationToken,
    pub target_guard: tokio::sync::OwnedMutexGuard<()>,
    pub audit_state: Option<AppState>,
}

pub(super) enum SessionStartupBackend {
    Ssh {
        target: super::domain::TargetRecord,
        credential: SshCredential,
    },
    Local {
        descriptor: LocalTerminalDescriptor,
    },
}

struct OutputBuffer {
    events: VecDeque<BufferedEvent>,
    retained_bytes: usize,
    next_cursor: u64,
}

enum SessionCommand {
    Input {
        attachment_id: String,
        generation: u64,
        sequence: u64,
        data: Vec<u8>,
        response: oneshot::Sender<TerminalResult<()>>,
    },
    Resize {
        attachment_id: String,
        generation: u64,
        revision: u64,
        cols: u32,
        rows: u32,
        response: oneshot::Sender<TerminalResult<()>>,
    },
}

impl TerminalRuntime {
    pub fn new() -> Self {
        Self::with_connector(Arc::new(RusshConnector))
    }

    fn with_connector(connector: Arc<dyn SshConnector>) -> Self {
        Self {
            access: super::access::AccessRuntime::default(),
            runtime_id: Uuid::new_v4().to_string(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            actor_tasks: Mutex::new(HashMap::new()),
            target_operations: Mutex::new(HashMap::new()),
            catalog_operation: Mutex::new(()),
            quota_operation: Mutex::new(()),
            connector,
            verification_grants: Mutex::new(HashMap::new()),
            force_confirmation_grants: Mutex::new(HashMap::new()),
        }
    }

    pub async fn issue_verification(&self, fingerprint: String) -> String {
        let token = Uuid::new_v4().to_string();
        let mut grants = self.verification_grants.lock().await;
        let now = Instant::now();
        grants.retain(|_, grant| grant.expires_at > now);
        if grants.len() >= MAX_VERIFICATION_GRANTS
            && let Some(oldest) = grants
                .iter()
                .min_by_key(|(_, grant)| grant.expires_at)
                .map(|(token, _)| token.clone())
        {
            grants.remove(&oldest);
        }
        grants.insert(
            token.clone(),
            VerificationGrant {
                fingerprint,
                expires_at: now + VERIFICATION_TOKEN_TTL,
            },
        );
        token
    }

    pub async fn consume_verification(&self, token: &str, fingerprint: &str) -> bool {
        let mut grants = self.verification_grants.lock().await;
        let now = Instant::now();
        grants.retain(|_, grant| grant.expires_at > now);
        grants
            .remove(token)
            .is_some_and(|grant| grant.fingerprint == fingerprint)
    }

    pub async fn clear_verifications(&self) {
        self.verification_grants.lock().await.clear();
        self.force_confirmation_grants.lock().await.clear();
    }

    pub async fn issue_force_confirmation(
        &self,
        target_id: &str,
        target_revision: u64,
        mut session_ids: Vec<String>,
    ) -> String {
        session_ids.sort();
        session_ids.dedup();
        let token = Uuid::new_v4().to_string();
        let mut grants = self.force_confirmation_grants.lock().await;
        let now = Instant::now();
        grants.retain(|_, grant| grant.expires_at > now);
        if grants.len() >= MAX_FORCE_CONFIRMATION_GRANTS
            && let Some(oldest) = grants
                .iter()
                .min_by_key(|(_, grant)| grant.expires_at)
                .map(|(token, _)| token.clone())
        {
            grants.remove(&oldest);
        }
        grants.insert(
            token.clone(),
            ForceConfirmationGrant {
                target_id: target_id.to_string(),
                target_revision,
                session_ids,
                expires_at: now + FORCE_CONFIRMATION_TTL,
            },
        );
        token
    }

    pub async fn consume_force_confirmation(
        &self,
        token: &str,
        target_id: &str,
        target_revision: u64,
        current_session_ids: &[String],
    ) -> bool {
        let mut grants = self.force_confirmation_grants.lock().await;
        let now = Instant::now();
        grants.retain(|_, grant| grant.expires_at > now);
        let Some(grant) = grants.remove(token) else {
            return false;
        };
        grant.target_id == target_id
            && grant.target_revision == target_revision
            && current_session_ids.iter().all(|id| {
                grant
                    .session_ids
                    .binary_search_by(|confirmed| confirmed.as_str().cmp(id.as_str()))
                    .is_ok()
            })
    }

    pub async fn target_operation(&self, target_id: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.target_operations.lock().await;
            locks.retain(|_, lock| lock.strong_count() > 0);
            if let Some(lock) = locks.get(target_id).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(Mutex::new(()));
                locks.insert(target_id.to_string(), Arc::downgrade(&lock));
                lock
            }
        };
        lock.lock_owned().await
    }

    pub async fn catalog_operation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.catalog_operation.lock().await
    }

    pub async fn list(&self) -> SessionListResult {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut snapshots = Vec::with_capacity(sessions.len());
        for session in sessions {
            snapshots.push(session.state.lock().await.session.clone());
        }
        snapshots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        SessionListResult {
            runtime_id: self.runtime_id.clone(),
            sessions: snapshots,
        }
    }

    pub async fn active_counts(&self, target_id: Option<&str>) -> usize {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut count = 0;
        for session in sessions {
            let state = session.state.lock().await;
            if state.session.phase.is_active()
                && target_id.is_none_or(|target_id| state.session.target_id == target_id)
            {
                count += 1;
            }
        }
        count
    }

    pub async fn active_session_ids(&self, target_id: &str) -> Vec<String> {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut ids = Vec::new();
        for session in sessions {
            let state = session.state.lock().await;
            if state.session.phase.is_active() && state.session.target_id == target_id {
                ids.push(state.session.id.clone());
            }
        }
        ids.sort();
        ids
    }

    pub(super) async fn begin_session(
        &self,
        target_id: String,
        title: String,
        cols: u32,
        rows: u32,
    ) -> TerminalResult<PendingSession> {
        let _quota_guard = self.quota_operation.lock().await;
        self.make_room_for_session().await?;
        if self.active_counts(None).await >= MAX_GLOBAL_SESSIONS
            || self.active_counts(Some(&target_id)).await >= MAX_TARGET_SESSIONS
        {
            return Err(TerminalError::new(
                TerminalErrorCode::SessionLimitReached,
                "terminal session limit reached",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        let snapshot = TerminalSession {
            id: id.clone(),
            backend: if target_id == LOCAL_TARGET_ID {
                SessionBackend::Local
            } else {
                SessionBackend::Ssh
            },
            target_id,
            title,
            phase: SessionPhase::Creating,
            cols,
            rows,
            created_at: now.clone(),
            updated_at: now,
            error_code: None,
            error_message: None,
            exit_code: None,
        };
        let (commands, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let runtime_session = Arc::new(RuntimeSession {
            state: Mutex::new(SessionState {
                session: snapshot.clone(),
                output: OutputBuffer::new(),
                terminal: RuntimeTerminal::Live(Box::new(vt100::Parser::new(
                    rows.min(u16::MAX.into()) as u16,
                    cols.min(u16::MAX.into()) as u16,
                    SCROLLBACK_ROWS,
                ))),
                closed_at: None,
                attachments: HashMap::new(),
                controller_id: None,
                controller_generation: 1,
            }),
            api_operation: Mutex::new(()),
            output_notify: Notify::new(),
            commands,
            cancel: CancellationToken::new(),
        });
        self.sessions
            .write()
            .await
            .insert(id.clone(), Arc::clone(&runtime_session));
        Ok(PendingSession {
            id,
            runtime: runtime_session,
            receiver: Some(receiver),
            sessions: Arc::clone(&self.sessions),
            activated: false,
        })
    }

    async fn make_room_for_session(&self) -> TerminalResult<()> {
        let current_len = self.sessions.read().await.len();
        if current_len < MAX_RETAINED_SESSIONS {
            return Ok(());
        }
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut candidates = Vec::new();
        for session in sessions {
            let mut state = session.state.lock().await;
            state.expire_attachments();
            if !state.session.phase.is_active() && state.attachments.is_empty() {
                candidates.push((state.session.updated_at.clone(), state.session.id.clone()));
            }
        }
        candidates.sort();
        for (_, id) in candidates {
            if self.sessions.read().await.len() < MAX_RETAINED_SESSIONS {
                return Ok(());
            }
            self.evict_unattached_session(&id, MAX_RETAINED_SESSIONS)
                .await;
        }
        if self.sessions.read().await.len() >= MAX_RETAINED_SESSIONS {
            return Err(TerminalError::new(
                TerminalErrorCode::SessionLimitReached,
                "terminal retained-session limit reached",
            ));
        }
        Ok(())
    }

    // Candidate selection races with reconnects. Recheck under the same
    // operation lock as attachment creation before removing the session.
    async fn evict_unattached_session(&self, id: &str, minimum_count: usize) -> Option<usize> {
        let session = self.session(id).await.ok()?;
        let api = session.api_operation.lock().await;
        let mut state = session.state.lock().await;
        state.expire_attachments();
        if state.session.phase.is_active() || !state.attachments.is_empty() {
            return None;
        }
        let size = state.terminal.archived_bytes();
        {
            let mut sessions = self.sessions.write().await;
            if sessions.len() < minimum_count {
                return None;
            }
            sessions.remove(id)?;
        }
        drop(state);
        drop(api);
        let task = self.actor_tasks.lock().await.remove(id);
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
        Some(size)
    }

    #[cfg(test)]
    pub(super) async fn reserve_active_test_session(
        &self,
        target_id: &str,
    ) -> TerminalResult<String> {
        let mut pending = self
            .begin_session(target_id.to_string(), "test shell".to_string(), 80, 24)
            .await?;
        let id = pending.id.clone();
        pending.activated = true;
        Ok(id)
    }

    /// Registers the complete backend initialization and shell loop as one
    /// owned runtime task. The HTTP request can return the `creating` snapshot
    /// while connection or PTY setup continues through observable phases.
    pub(super) async fn start_session(
        &self,
        startup: SessionStartup,
    ) -> TerminalResult<TerminalSession> {
        let SessionStartup {
            mut pending,
            backend,
            initial_cols,
            initial_rows,
            shutdown,
            target_guard,
            audit_state,
        } = startup;
        let receiver = pending.receiver.take().ok_or_else(|| {
            TerminalError::internal("terminal session reservation was already activated")
        })?;
        pending.activated = true;
        let snapshot = pending.runtime.state.lock().await.session.clone();
        let id = pending.id.clone();
        let runtime = Arc::clone(&pending.runtime);
        let connector = Arc::clone(&self.connector);
        let (progress, progress_task) = pending.progress_channel();
        let task = tokio::spawn(async move {
            // Target mutation/deletion or local disable is serialized with
            // backend initialization, but never blocks unrelated targets.
            let _target_guard = target_guard;
            let cancelled = runtime.cancel.clone();
            let connected = {
                let connection = async {
                    match backend {
                        SessionStartupBackend::Ssh { target, credential } => {
                            connector
                                .open_shell(
                                    &target,
                                    credential,
                                    initial_cols,
                                    initial_rows,
                                    Some(&progress),
                                )
                                .await
                        }
                        SessionStartupBackend::Local { descriptor } => {
                            local::open_shell(
                                descriptor,
                                initial_cols,
                                initial_rows,
                                Some(&progress),
                            )
                            .await
                        }
                    }
                };
                tokio::pin!(connection);
                tokio::select! {
                    connected = &mut connection => Some(connected),
                    _ = cancelled.cancelled() => None,
                    _ = shutdown.cancelled() => None,
                }
            };
            drop(progress);
            let _ = progress_task.await;
            let Some(connected) = connected else {
                drop(_target_guard);
                set_phase(&runtime, SessionPhase::Closing, None, None).await;
                set_phase(&runtime, SessionPhase::Closed, None, None).await;
                let session = runtime.state.lock().await.session.clone();
                if let Some(audit_state) = audit_state.as_ref() {
                    publish_session_audit(
                        audit_state,
                        "session_ended",
                        &session.target_id,
                        &session.id,
                        None,
                    )
                    .await;
                }
                return;
            };
            let mut shell = match connected {
                Ok(shell) => shell,
                Err(error) => {
                    set_phase(
                        &runtime,
                        SessionPhase::Failed,
                        Some((error.code, error.message.clone())),
                        None,
                    )
                    .await;
                    let session = runtime.state.lock().await.session.clone();
                    tracing::warn!(
                        target_id = %session.target_id,
                        session_id = %session.id,
                        backend = ?session.backend,
                        error_code = %error.code,
                        "terminal session initialization failed"
                    );
                    if let Some(audit_state) = audit_state.as_ref() {
                        publish_session_audit(
                            audit_state,
                            "session_creation_failed",
                            &session.target_id,
                            &session.id,
                            Some(error.code),
                        )
                        .await;
                    }
                    return;
                }
            };

            // An attachment can reserve control and provide a newer viewport
            // while the backend is still initializing. Synchronize that final
            // size once the shell exists, before accepting input.
            let (cols, rows) = {
                let state = runtime.state.lock().await;
                (state.session.cols, state.session.rows)
            };
            let initial_resize = if cols != initial_cols || rows != initial_rows {
                tokio::time::timeout(SHELL_COMMAND_TIMEOUT, shell.resize(cols, rows))
                    .await
                    .unwrap_or_else(|_| {
                        Err(TerminalError::new(
                            TerminalErrorCode::SessionLost,
                            "terminal initial resize timed out",
                        ))
                    })
            } else {
                Ok(())
            };
            if let Err(error) = initial_resize {
                set_phase(
                    &runtime,
                    SessionPhase::Failed,
                    Some((error.code, error.message)),
                    None,
                )
                .await;
                shell.disconnect().await;
                let session = runtime.state.lock().await.session.clone();
                if let Some(audit_state) = audit_state.as_ref() {
                    publish_session_audit(
                        audit_state,
                        "session_creation_failed",
                        &session.target_id,
                        &session.id,
                        Some(error.code),
                    )
                    .await;
                }
                return;
            }
            drop(_target_guard);
            set_phase(&runtime, SessionPhase::Running, None, None).await;
            // Session creation was already audited when the runtime-owned
            // initializer was registered. Start draining SSH output
            // immediately; persistent audit I/O must never precede the actor.
            run_session_actor(runtime, receiver, shell, shutdown, audit_state).await;
        });
        self.actor_tasks.lock().await.insert(id, task);
        Ok(snapshot)
    }

    #[cfg(test)]
    async fn activate_session(
        &self,
        mut pending: PendingSession,
        shell: BoxedShell,
        shutdown: CancellationToken,
    ) -> TerminalResult<TerminalSession> {
        {
            let mut state = pending.runtime.state.lock().await;
            if state.session.phase != SessionPhase::Running {
                state.set_phase(SessionPhase::Running, None, None);
            }
        }
        let receiver = pending.receiver.take().ok_or_else(|| {
            TerminalError::internal("terminal session reservation was already activated")
        })?;
        pending.activated = true;
        let snapshot = pending.runtime.state.lock().await.session.clone();
        let task = tokio::spawn(run_session_actor(
            Arc::clone(&pending.runtime),
            receiver,
            shell,
            shutdown,
            None,
        ));
        self.actor_tasks
            .lock()
            .await
            .insert(pending.id.clone(), task);
        Ok(snapshot)
    }

    pub async fn rename(&self, id: &str, title: &str) -> TerminalResult<TerminalSession> {
        let title = sanitize_title(title)?;
        let session = self.session(id).await?;
        let _guard = session.api_operation.lock().await;
        let mut state = session.state.lock().await;
        state.session.title = title;
        state.session.updated_at = now_iso();
        Ok(state.session.clone())
    }

    pub async fn create_attachment(
        &self,
        session_id: &str,
        requested_cols: Option<u32>,
        requested_rows: Option<u32>,
    ) -> TerminalResult<TerminalAttachment> {
        if requested_cols.is_some_and(|value| !(40..=400).contains(&value))
            || requested_rows.is_some_and(|value| !(12..=200).contains(&value))
        {
            return Err(TerminalError::invalid(
                "terminal dimensions are out of range",
            ));
        }
        let session = self.session(session_id).await?;
        let _guard = session.api_operation.lock().await;
        // An archive-budget eviction may have won the operation lock after
        // the lookup; do not create an attachment on an evicted session.
        self.session(session_id).await?;
        let mut state = session.state.lock().await;
        state.expire_attachments();
        if state.attachments.len() >= MAX_ATTACHMENTS {
            return Err(TerminalError::new(
                TerminalErrorCode::Conflict,
                "terminal attachment limit reached",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let role = if matches!(
            state.session.phase,
            SessionPhase::Creating
                | SessionPhase::OpeningPty
                | SessionPhase::StartingShell
                | SessionPhase::Resolving
                | SessionPhase::Connecting
                | SessionPhase::VerifyingHostKey
                | SessionPhase::Authenticating
                | SessionPhase::OpeningChannel
                | SessionPhase::RequestingPty
                | SessionPhase::Running
        ) && state.controller_id.is_none()
        {
            state.controller_id = Some(id.clone());
            AttachmentRole::Controller
        } else {
            AttachmentRole::Viewer
        };
        let generation = state.controller_generation;
        let expires_at = iso_after_seconds(ATTACHMENT_TTL_SECONDS);
        state.attachments.insert(
            id.clone(),
            AttachmentState {
                role,
                generation,
                expires_at: expires_at.clone(),
                last_seen: Instant::now(),
                needs_snapshot: true,
                last_input_sequence: 0,
                last_resize_revision: 0,
                control_dirty: false,
                snapshot: None,
                transport_cursor: 0,
                output_cursor: 0,
                pending_poll: None,
            },
        );
        let attachment = TerminalAttachment {
            id,
            session_id: session_id.to_string(),
            role,
            transport: TerminalTransport::HttpPolling,
            generation,
            cursor: 0,
            expires_at,
        };
        let resize = if role == AttachmentRole::Controller {
            let cols = requested_cols.unwrap_or(state.session.cols);
            let rows = requested_rows.unwrap_or(state.session.rows);
            let changed = cols != state.session.cols || rows != state.session.rows;
            if changed {
                state.session.cols = cols;
                state.session.rows = rows;
                state.session.updated_at = now_iso();
                state.terminal.resize(rows as u16, cols as u16);
            }
            (changed && state.session.phase == SessionPhase::Running).then_some((cols, rows))
        } else {
            None
        };
        drop(state);
        if let Some((cols, rows)) = resize {
            let (response, received) = oneshot::channel();
            session
                .commands
                .send(SessionCommand::Resize {
                    attachment_id: attachment.id.clone(),
                    generation: attachment.generation,
                    revision: 1,
                    cols,
                    rows,
                    response,
                })
                .await
                .map_err(|_| session_lost())?;
            received.await.map_err(|_| session_lost())??;
            if let Some(state) = session
                .state
                .lock()
                .await
                .attachments
                .get_mut(&attachment.id)
                && state.generation == attachment.generation
            {
                // This internal resize does not consume the caller's first
                // revision (which also starts at one).
                state.last_resize_revision = 0;
            }
        }
        Ok(attachment)
    }

    pub async fn detach(&self, attachment_id: &str) -> TerminalResult<()> {
        let session = self.session_for_attachment(attachment_id).await?;
        let _guard = session.api_operation.lock().await;
        let mut state = session.state.lock().await;
        if state.attachments.remove(attachment_id).is_none() {
            return Err(attachment_expired());
        }
        if state.controller_id.as_deref() == Some(attachment_id) {
            state.controller_id = None;
        }
        state.reclaim_acknowledged_output();
        Ok(())
    }

    pub async fn claim_control(
        &self,
        attachment_id: &str,
        expected_generation: Option<u64>,
    ) -> TerminalResult<TerminalAttachment> {
        let session = self.session_for_attachment(attachment_id).await?;
        let _guard = session.api_operation.lock().await;
        let mut state = session.state.lock().await;
        state.expire_attachments();
        if state.session.phase != SessionPhase::Running {
            return Err(TerminalError::new(
                TerminalErrorCode::SessionLost,
                "terminal session is no longer running",
            ));
        }
        if let Some(expected) = expected_generation
            && expected != state.controller_generation
        {
            return Err(TerminalError::new(
                TerminalErrorCode::ControllerConflict,
                "terminal controller generation changed",
            ));
        }
        if !state.attachments.contains_key(attachment_id) {
            return Err(attachment_expired());
        }
        state.controller_generation = state.controller_generation.saturating_add(1).max(1);
        let generation = state.controller_generation;
        let mut expires_at = None;
        for (id, attachment) in &mut state.attachments {
            let is_controller = id == attachment_id;
            attachment.role = if is_controller {
                AttachmentRole::Controller
            } else {
                AttachmentRole::Viewer
            };
            attachment.generation = generation;
            attachment.last_input_sequence = 0;
            attachment.last_resize_revision = 0;
            attachment.control_dirty = !is_controller;
            if is_controller {
                expires_at = Some(attachment.expires_at.clone());
            }
        }
        let expires_at = expires_at.ok_or_else(attachment_expired)?;
        state.controller_id = Some(attachment_id.to_string());
        tracing::info!(
            session_id = %state.session.id,
            attachment_id,
            generation,
            "terminal controller claimed"
        );
        Ok(TerminalAttachment {
            id: attachment_id.to_string(),
            session_id: state.session.id.clone(),
            role: AttachmentRole::Controller,
            transport: TerminalTransport::HttpPolling,
            generation,
            cursor: 0,
            expires_at,
        })
    }

    pub async fn send_input(
        &self,
        attachment_id: &str,
        generation: u64,
        sequence: u64,
        data: Vec<u8>,
    ) -> TerminalResult<()> {
        if data.len() > MAX_INPUT_BYTES {
            return Err(TerminalError::invalid("terminal input exceeds 64 KiB"));
        }
        if data.is_empty() {
            return Ok(());
        }
        let session = self.session_for_attachment(attachment_id).await?;
        let _guard = session.api_operation.lock().await;
        let (response, received) = oneshot::channel();
        session
            .commands
            .send(SessionCommand::Input {
                attachment_id: attachment_id.to_string(),
                generation,
                sequence,
                data,
                response,
            })
            .await
            .map_err(|_| session_lost())?;
        received.await.map_err(|_| session_lost())?
    }

    pub async fn resize(
        &self,
        attachment_id: &str,
        generation: u64,
        revision: u64,
        cols: u32,
        rows: u32,
    ) -> TerminalResult<TerminalSession> {
        if !(40..=400).contains(&cols) || !(12..=200).contains(&rows) {
            return Err(TerminalError::invalid(
                "terminal dimensions are out of range",
            ));
        }
        let session = self.session_for_attachment(attachment_id).await?;
        let _guard = session.api_operation.lock().await;
        let (response, received) = oneshot::channel();
        session
            .commands
            .send(SessionCommand::Resize {
                attachment_id: attachment_id.to_string(),
                generation,
                revision,
                cols,
                rows,
                response,
            })
            .await
            .map_err(|_| session_lost())?;
        received.await.map_err(|_| session_lost())??;
        let snapshot = session.state.lock().await.session.clone();
        Ok(snapshot)
    }

    pub async fn events(
        &self,
        attachment_id: &str,
        after: u64,
        timeout: Duration,
    ) -> TerminalResult<EventsResult> {
        let session = self.session_for_attachment(attachment_id).await?;
        let deadline = Instant::now() + timeout.min(MAX_POLL_TIMEOUT);
        loop {
            let notified = session.output_notify.notified();
            if let Some(result) = read_events(&session, attachment_id, after).await? {
                return Ok(result);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(EventsResult {
                    events: Vec::new(),
                    next_cursor: after,
                });
            }
            if tokio::time::timeout(deadline.saturating_duration_since(now), notified)
                .await
                .is_err()
            {
                return Ok(EventsResult {
                    events: Vec::new(),
                    next_cursor: after,
                });
            }
        }
    }

    pub async fn terminate(&self, id: &str) -> TerminalResult<()> {
        let session = self.session(id).await?;
        let _guard = session.api_operation.lock().await;
        session.cancel.cancel();
        // Remove the directory entry before taking ownership of the actor.
        // If this request is cancelled while joining/aborting it, an inert
        // session must not remain marked running and consume an active slot.
        self.sessions.write().await.remove(id);
        let task = self.actor_tasks.lock().await.remove(id);
        if let Some(task) = task {
            finish_session_task(AbortOnDropHandle::new(task), SESSION_SHUTDOWN_TIMEOUT).await;
        }
        tracing::info!(session_id = id, "terminal session terminated");
        Ok(())
    }

    pub async fn terminate_target(&self, target_id: &str) {
        let sessions = self.list().await.sessions;
        for id in sessions
            .into_iter()
            .filter(|session| session.target_id == target_id)
            .map(|session| session.id)
        {
            let _ = self.terminate(&id).await;
        }
    }

    pub async fn shutdown_all(&self) {
        self.clear_verifications().await;
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            session.cancel.cancel();
        }
        self.sessions.write().await.clear();
        let tasks = std::mem::take(&mut *self.actor_tasks.lock().await);
        // Own every removed task before awaiting any one of them. Cancellation
        // must abort the remaining actors instead of detaching their handles.
        let tasks = tasks
            .into_values()
            .map(AbortOnDropHandle::new)
            .collect::<Vec<_>>();
        for task in tasks {
            finish_session_task(task, SESSION_SHUTDOWN_TIMEOUT).await;
        }
    }

    pub async fn expire_attachments(&self) {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            let should_compact = {
                let mut state = session.state.lock().await;
                state.expire_attachments();
                state.should_compact_terminal()
            };
            if should_compact {
                // Run one compaction at a time. The guard lives in the blocking
                // task, so cancellation cannot leave a parser half archived.
                let permit = Arc::clone(&TERMINAL_ARCHIVE_WORK)
                    .acquire_owned()
                    .await
                    .expect("terminal archive semaphore is never closed");
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let mut state = session.state.blocking_lock();
                    if state.should_compact_terminal() {
                        state.terminal.compact()?;
                    }
                    Ok::<_, TerminalError>(())
                })
                .await;
                if !matches!(result, Ok(Ok(()))) {
                    tracing::warn!(?result, "failed to compact closed terminal");
                }
                // Do not accumulate every compressed session before enforcing
                // the global history budget during a maintenance sweep.
                self.enforce_archive_budget(MAX_ARCHIVED_TERMINAL_BYTES)
                    .await;
            }
        }
        self.enforce_archive_budget(MAX_ARCHIVED_TERMINAL_BYTES)
            .await;
        let finished_ids = {
            let tasks = self.actor_tasks.lock().await;
            tasks
                .iter()
                .filter(|(_, task)| task.is_finished())
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        };
        for id in finished_ids {
            if let Some(task) = self.actor_tasks.lock().await.remove(&id) {
                let _ = task.await;
            }
        }
    }

    async fn enforce_archive_budget(&self, budget: usize) {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut bytes = 0_usize;
        let mut candidates = Vec::new();
        for session in sessions {
            let state = session.state.lock().await;
            let size = state.terminal.archived_bytes();
            bytes = bytes.saturating_add(size);
            if size > 0 && !state.session.phase.is_active() && state.attachments.is_empty() {
                candidates.push((
                    state.session.updated_at.clone(),
                    state.session.id.clone(),
                    size,
                ));
            }
        }
        candidates.sort();
        for (_, id, _) in candidates {
            if bytes <= budget {
                break;
            }
            if let Some(size) = self.evict_unattached_session(&id, 0).await {
                bytes = bytes.saturating_sub(size);
            }
        }
    }

    async fn session(&self, id: &str) -> TerminalResult<Arc<RuntimeSession>> {
        self.sessions.read().await.get(id).cloned().ok_or_else(|| {
            TerminalError::new(
                TerminalErrorCode::SessionNotFound,
                "terminal session not found",
            )
        })
    }

    async fn session_for_attachment(
        &self,
        attachment_id: &str,
    ) -> TerminalResult<Arc<RuntimeSession>> {
        let sessions = self
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            let mut state = session.state.lock().await;
            state.expire_attachments();
            if state.attachments.contains_key(attachment_id) {
                return Ok(Arc::clone(&session));
            }
        }
        Err(attachment_expired())
    }
}

impl Default for TerminalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingSession {
    pub fn progress_channel(&self) -> (mpsc::UnboundedSender<SessionPhase>, JoinHandle<()>) {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let runtime = Arc::clone(&self.runtime);
        let task = tokio::spawn(async move {
            while let Some(phase) = receiver.recv().await {
                let mut state = runtime.state.lock().await;
                state.set_phase(phase, None, None);
                drop(state);
                runtime.output_notify.notify_waiters();
            }
        });
        (sender, task)
    }
}

impl Drop for PendingSession {
    fn drop(&mut self) {
        if self.activated {
            return;
        }
        let id = self.id.clone();
        let sessions = Arc::clone(&self.sessions);
        // A cancelled HTTP request drops its reservation. Removing it in an
        // owned task prevents an indefinitely active `creating` session.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                sessions.write().await.remove(&id);
            });
        }
    }
}

async fn finish_session_task(mut task: AbortOnDropHandle<()>, grace: Duration) {
    if tokio::time::timeout(grace, &mut task).await.is_err() {
        task.abort();
        // A successful termination response includes dropping the shell and
        // its queues; abort alone only schedules that cleanup for a later poll.
        let _ = task.await;
    }
}

impl SessionState {
    fn should_compact_terminal(&self) -> bool {
        !self.session.phase.is_active()
            && self.attachments.is_empty()
            && matches!(self.terminal, RuntimeTerminal::Live(_))
            && self
                .closed_at
                .is_some_and(|closed| closed.elapsed() >= CLOSED_TERMINAL_COMPACT_AFTER)
    }

    fn expire_attachments(&mut self) {
        let now = Instant::now();
        self.attachments.retain(|_, attachment| {
            now.saturating_duration_since(attachment.last_seen)
                < Duration::from_secs(ATTACHMENT_TTL_SECONDS as u64)
        });
        if self
            .controller_id
            .as_ref()
            .is_some_and(|id| !self.attachments.contains_key(id))
        {
            self.controller_id = None;
        }
        self.reclaim_acknowledged_output();
    }

    fn reclaim_acknowledged_output(&mut self) {
        let Some(cursor) = self
            .attachments
            .values()
            .map(|attachment| attachment.output_cursor)
            .min()
        else {
            // Browser attachments are observers of the PTY, not owners of it.
            // Keep the parser (and therefore reconnect scrollback) alive while
            // releasing the redundant incremental delivery buffer.
            self.output.clear_retained();
            return;
        };
        self.output.discard_through(cursor);
    }

    fn validate_controller(&mut self, attachment_id: &str, generation: u64) -> TerminalResult<()> {
        self.expire_attachments();
        if self.session.phase != SessionPhase::Running {
            return Err(session_lost());
        }
        let Some(attachment) = self.attachments.get_mut(attachment_id) else {
            return Err(attachment_expired());
        };
        if attachment.role != AttachmentRole::Controller
            || self.controller_id.as_deref() != Some(attachment_id)
            || attachment.generation != generation
            || self.controller_generation != generation
        {
            return Err(TerminalError::new(
                TerminalErrorCode::ControllerConflict,
                "terminal attachment does not own the current controller generation",
            ));
        }
        Ok(())
    }

    fn set_phase(
        &mut self,
        phase: SessionPhase,
        error: Option<(TerminalErrorCode, String)>,
        exit_code: Option<u32>,
    ) {
        if self.session.phase != phase && !self.session.phase.can_transition_to(phase) {
            tracing::warn!(
                from = ?self.session.phase,
                to = ?phase,
                session_id = %self.session.id,
                "rejected invalid terminal session transition"
            );
            return;
        }
        self.session.phase = phase;
        if phase.is_active() {
            self.closed_at = None;
        } else {
            self.closed_at.get_or_insert_with(Instant::now);
        }
        self.session.updated_at = now_iso();
        self.session.error_code = error.as_ref().map(|value| value.0);
        self.session.error_message = error.map(|value| value.1);
        self.session.exit_code = exit_code;
        if matches!(
            phase,
            SessionPhase::Closing
                | SessionPhase::Closed
                | SessionPhase::Exited
                | SessionPhase::Lost
                | SessionPhase::Failed
        ) {
            self.controller_generation = self.controller_generation.saturating_add(1).max(1);
            self.controller_id = None;
            for attachment in self.attachments.values_mut() {
                attachment.role = AttachmentRole::Viewer;
                attachment.generation = self.controller_generation;
                attachment.last_input_sequence = 0;
                attachment.last_resize_revision = 0;
                attachment.control_dirty = true;
            }
        }
        if self.attachments.is_empty() {
            self.output.discard_status();
        } else {
            self.output.push_status(
                phase,
                self.session.error_code,
                self.session.error_message.clone(),
                self.session.exit_code,
            );
        }
    }
}

impl OutputBuffer {
    fn new() -> Self {
        Self {
            events: VecDeque::new(),
            retained_bytes: 0,
            next_cursor: 1,
        }
    }

    fn push_output(&mut self, data: Vec<u8>) {
        if data.is_empty() {
            return;
        }
        for chunk in data.chunks(OUTPUT_RESPONSE_BYTES) {
            self.retained_bytes = self.retained_bytes.saturating_add(chunk.len());
            let cursor = self.allocate_cursor();
            self.events.push_back(BufferedEvent {
                cursor,
                kind: TerminalEventType::Output,
                data: Some(chunk.to_vec()),
                phase: None,
                error_code: None,
                error_message: None,
                exit_code: None,
            });
            self.trim();
        }
    }

    fn discard_output(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.clear_retained();
        for _ in data.chunks(OUTPUT_RESPONSE_BYTES) {
            self.allocate_cursor();
        }
    }

    fn push_status(
        &mut self,
        phase: SessionPhase,
        error_code: Option<TerminalErrorCode>,
        error_message: Option<String>,
        exit_code: Option<u32>,
    ) {
        let cursor = self.allocate_cursor();
        self.events.push_back(BufferedEvent {
            cursor,
            kind: TerminalEventType::Status,
            data: None,
            phase: Some(phase),
            error_code,
            error_message,
            exit_code,
        });
        self.trim();
    }

    fn discard_status(&mut self) {
        self.clear_retained();
        self.allocate_cursor();
    }

    fn discard_through(&mut self, cursor: u64) {
        while self
            .events
            .front()
            .is_some_and(|event| event.cursor <= cursor)
        {
            let event = self.events.pop_front().expect("front event exists");
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(event.data.as_ref().map_or(0, Vec::len));
        }
    }

    fn clear_retained(&mut self) {
        self.events.clear();
        self.events.shrink_to_fit();
        self.retained_bytes = 0;
    }

    fn allocate_cursor(&mut self) -> u64 {
        let cursor = self.next_cursor;
        self.next_cursor = self.next_cursor.saturating_add(1);
        cursor
    }

    fn trim(&mut self) {
        while self.retained_bytes > OUTPUT_CAPACITY_BYTES || self.events.len() > 8_192 {
            let Some(event) = self.events.pop_front() else {
                break;
            };
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(event.data.as_ref().map_or(0, Vec::len));
        }
    }

    fn latest_cursor(&self) -> u64 {
        self.next_cursor.saturating_sub(1)
    }

    fn first_cursor(&self) -> u64 {
        self.events
            .front()
            .map(|event| event.cursor)
            .unwrap_or(self.next_cursor)
    }
}

async fn read_events(
    session: &RuntimeSession,
    attachment_id: &str,
    after: u64,
) -> TerminalResult<Option<EventsResult>> {
    let mut state = session.state.lock().await;
    state.expire_attachments();
    let (needs_snapshot, control_update, snapshot_pending, output_after) = {
        let attachment = state
            .attachments
            .get_mut(attachment_id)
            .ok_or_else(attachment_expired)?;
        if let Some(pending) = attachment.pending_poll.as_ref() {
            if after == pending.request_cursor {
                attachment.last_seen = Instant::now();
                attachment.expires_at = iso_after_seconds(ATTACHMENT_TTL_SECONDS);
                return Ok(Some(pending.result.clone()));
            }
            if after != pending.result.next_cursor {
                return Err(TerminalError::invalid(
                    "terminal event cursor is not valid for this attachment",
                ));
            }
        }
        if let Some(pending) = attachment.pending_poll.take() {
            attachment.transport_cursor = after;
            attachment.output_cursor = pending.next_output_cursor;
            if pending.advance_snapshot
                && let Some(snapshot) = attachment.snapshot.as_mut()
            {
                snapshot.index = snapshot.index.saturating_add(1);
                if snapshot.index >= snapshot.chunk_count() {
                    attachment.output_cursor = snapshot.cursor;
                    attachment.snapshot = None;
                }
            }
            if pending
                .control
                .is_some_and(|control| control == (attachment.role, attachment.generation))
            {
                attachment.control_dirty = false;
            }
        }
        if after != attachment.transport_cursor {
            return Err(TerminalError::invalid(
                "terminal event cursor is not valid for this attachment",
            ));
        }
        attachment.last_seen = Instant::now();
        attachment.expires_at = iso_after_seconds(ATTACHMENT_TTL_SECONDS);
        let needs_snapshot = std::mem::take(&mut attachment.needs_snapshot);
        let control_update = attachment
            .control_dirty
            .then_some((attachment.role, attachment.generation));
        (
            needs_snapshot,
            control_update,
            attachment.snapshot.is_some(),
            attachment.output_cursor,
        )
    };
    state.reclaim_acknowledged_output();
    if snapshot_pending {
        return build_snapshot_events(&mut state, attachment_id, after, control_update).map(Some);
    }
    let latest = state.output.latest_cursor();
    let cursor_gap =
        output_after > latest || output_after.saturating_add(1) < state.output.first_cursor();
    if cursor_gap || (needs_snapshot && state.output.first_cursor() > 1) {
        let snapshot = state.terminal.snapshot().await?;
        state
            .attachments
            .get_mut(attachment_id)
            .ok_or_else(attachment_expired)?
            .snapshot = Some(SnapshotTransfer {
            data: snapshot,
            cursor: latest,
            index: 0,
        });
        return build_snapshot_events(&mut state, attachment_id, after, control_update).map(Some);
    }
    let mut bytes = 0_usize;
    let mut events = Vec::new();
    if let Some((role, generation)) = control_update {
        events.push(control_event(output_after, role, generation));
    }
    let mut next_output_cursor = output_after;
    for event in state
        .output
        .events
        .iter()
        .filter(|event| event.cursor > output_after)
    {
        let size = event.data.as_ref().map_or(0, Vec::len);
        if !events.is_empty() && bytes.saturating_add(size) > OUTPUT_RESPONSE_BYTES {
            break;
        }
        bytes = bytes.saturating_add(size);
        next_output_cursor = event.cursor;
        events.push(TerminalEvent {
            kind: event.kind,
            cursor: event.cursor,
            data_base64: event.data.as_ref().map(|data| STANDARD.encode(data)),
            reset: false,
            phase: event.phase,
            error_code: event.error_code,
            error_message: event.error_message.clone(),
            exit_code: event.exit_code,
            role: None,
            generation: None,
        });
    }
    if events.is_empty() {
        return Ok(None);
    }
    cache_poll_result(
        &mut state,
        attachment_id,
        after,
        events,
        next_output_cursor,
        false,
        control_update,
    )
    .map(Some)
}

fn build_snapshot_events(
    state: &mut SessionState,
    attachment_id: &str,
    after: u64,
    control_update: Option<(AttachmentRole, u64)>,
) -> TerminalResult<EventsResult> {
    let session = state.session.clone();
    let (chunk, cursor, reset, complete) = {
        let attachment = state
            .attachments
            .get_mut(attachment_id)
            .ok_or_else(attachment_expired)?;
        let transfer = attachment
            .snapshot
            .as_ref()
            .ok_or_else(|| TerminalError::internal("terminal snapshot transfer is missing"))?;
        let start = transfer
            .index
            .saturating_mul(OUTPUT_RESPONSE_BYTES)
            .min(transfer.data.len());
        let end = start
            .saturating_add(OUTPUT_RESPONSE_BYTES)
            .min(transfer.data.len());
        let chunk = transfer.data.slice(start..end);
        let cursor = transfer.cursor;
        let reset = transfer.index == 0;
        let complete = transfer.index.saturating_add(1) >= transfer.chunk_count();
        (chunk, cursor, reset, complete)
    };
    let mut events = vec![TerminalEvent {
        kind: TerminalEventType::Output,
        cursor,
        data_base64: Some(STANDARD.encode(chunk)),
        reset,
        phase: None,
        error_code: None,
        error_message: None,
        exit_code: None,
        role: None,
        generation: None,
    }];
    if complete {
        events.push(TerminalEvent {
            kind: TerminalEventType::Status,
            cursor,
            data_base64: None,
            reset: false,
            phase: Some(session.phase),
            error_code: session.error_code,
            error_message: session.error_message,
            exit_code: session.exit_code,
            role: None,
            generation: None,
        });
    }
    if let Some((role, generation)) = control_update {
        events.push(control_event(cursor, role, generation));
    }
    let output_cursor = if complete {
        cursor
    } else {
        state
            .attachments
            .get(attachment_id)
            .map_or(0, |attachment| attachment.output_cursor)
    };
    cache_poll_result(
        state,
        attachment_id,
        after,
        events,
        output_cursor,
        true,
        control_update,
    )
}

fn cache_poll_result(
    state: &mut SessionState,
    attachment_id: &str,
    request_cursor: u64,
    events: Vec<TerminalEvent>,
    next_output_cursor: u64,
    advance_snapshot: bool,
    control: Option<(AttachmentRole, u64)>,
) -> TerminalResult<EventsResult> {
    let attachment = state
        .attachments
        .get_mut(attachment_id)
        .ok_or_else(attachment_expired)?;
    let next_cursor = attachment.transport_cursor.checked_add(1).ok_or_else(|| {
        TerminalError::new(
            TerminalErrorCode::AttachmentExpired,
            "terminal attachment event cursor is exhausted",
        )
    })?;
    let result = EventsResult {
        events,
        next_cursor,
    };
    attachment.pending_poll = Some(PendingPoll {
        request_cursor,
        result: result.clone(),
        next_output_cursor,
        advance_snapshot,
        control,
    });
    Ok(result)
}

/// Produces a replayable VT stream containing up to the parser's configured
/// main-screen scrollback plus the exact visible state. Alternate-screen
/// applications intentionally expose only their active screen; replaying the
/// hidden main-screen history while an alternate screen is active would
/// corrupt full-screen programs.
fn terminal_snapshot(parser: &mut vt100::Parser) -> Vec<u8> {
    let mut snapshot = Vec::new();
    write_terminal_snapshot(parser, &mut snapshot).expect("writing to Vec cannot fail");
    snapshot
}

fn write_terminal_snapshot(
    parser: &mut vt100::Parser,
    output: &mut impl Write,
) -> std::io::Result<()> {
    if parser.screen().alternate_screen() {
        return output.write_all(&parser.screen().state_formatted());
    }
    let (screen_rows, cols) = parser.screen().size();
    parser.screen_mut().set_scrollback(usize::MAX);
    let mut remaining = parser.screen().scrollback();
    let result: std::io::Result<()> = (|| {
        while remaining > 0 {
            parser.screen_mut().set_scrollback(remaining);
            let take = remaining.min(usize::from(screen_rows));
            for row in parser.screen().rows_formatted(0, cols).take(take) {
                output.write_all(&row)?;
                output.write_all(b"\x1b[0m\r\n")?;
            }
            remaining = remaining.saturating_sub(take);
        }
        Ok(())
    })();
    parser.screen_mut().set_scrollback(0);
    result?;
    output.write_all(&parser.screen().state_formatted())
}

enum RuntimeTerminal {
    Live(Box<vt100::Parser>),
    Archived(ArchivedTerminal),
}

struct ArchivedTerminal {
    compressed: Bytes,
    // Decoded replay is shared only while a transfer still owns it. Keeping a
    // weak reference avoids turning a reconnect into permanent decoded RSS.
    replay: Weak<Bytes>,
}

impl From<Bytes> for ArchivedTerminal {
    fn from(compressed: Bytes) -> Self {
        Self {
            compressed,
            replay: Weak::new(),
        }
    }
}

impl RuntimeTerminal {
    fn process(&mut self, data: &[u8]) {
        if let Self::Live(parser) = self {
            parser.process(data);
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if let Self::Live(parser) = self {
            parser.screen_mut().set_size(rows, cols);
        }
    }

    fn archived_bytes(&self) -> usize {
        match self {
            Self::Live(_) => 0,
            Self::Archived(archive) => archive.compressed.len(),
        }
    }

    async fn snapshot(&mut self) -> TerminalResult<Arc<Bytes>> {
        match self {
            Self::Live(parser) => Ok(Arc::new(Bytes::from(
                terminal_snapshot(parser).into_boxed_slice(),
            ))),
            Self::Archived(archive) => {
                if let Some(replay) = archive.replay.upgrade() {
                    return Ok(replay);
                }
                let compressed = archive.compressed.clone();
                let permit = Arc::clone(&TERMINAL_ARCHIVE_WORK)
                    .acquire_owned()
                    .await
                    .map_err(|_| {
                        TerminalError::internal("terminal archive worker is unavailable")
                    })?;
                let replay = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    decode_terminal_archive(&compressed, MAX_TERMINAL_SNAPSHOT_BYTES)
                })
                .await
                .map_err(|error| {
                    TerminalError::internal(format!("terminal replay failed: {error}"))
                })??;
                let replay = Arc::new(replay);
                archive.replay = Arc::downgrade(&replay);
                Ok(replay)
            }
        }
    }

    fn compact(&mut self) -> TerminalResult<()> {
        let Self::Live(parser) = self else {
            return Ok(());
        };
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        write_terminal_snapshot(parser, &mut encoder).map_err(|error| {
            TerminalError::internal(format!("failed to archive terminal: {error}"))
        })?;
        let compressed = encoder.finish().map_err(|error| {
            TerminalError::internal(format!("failed to finish terminal archive: {error}"))
        })?;
        *self = Self::Archived(Bytes::from(compressed.into_boxed_slice()).into());
        Ok(())
    }
}

fn decode_terminal_archive(compressed: &[u8], limit: usize) -> TerminalResult<Bytes> {
    let mut snapshot = Vec::new();
    GzDecoder::new(compressed)
        .take(limit as u64 + 1)
        .read_to_end(&mut snapshot)
        .map_err(|error| TerminalError::internal(format!("invalid terminal archive: {error}")))?;
    if snapshot.len() > limit {
        return Err(TerminalError::internal(
            "terminal archive exceeds replay limit",
        ));
    }
    Ok(Bytes::from(snapshot.into_boxed_slice()))
}

fn control_event(cursor: u64, role: AttachmentRole, generation: u64) -> TerminalEvent {
    TerminalEvent {
        kind: TerminalEventType::Control,
        cursor,
        data_base64: None,
        reset: false,
        phase: None,
        error_code: None,
        error_message: None,
        exit_code: None,
        role: Some(role),
        generation: Some(generation),
    }
}

async fn run_session_actor(
    runtime: Arc<RuntimeSession>,
    mut commands: mpsc::Receiver<SessionCommand>,
    mut shell: BoxedShell,
    shutdown: CancellationToken,
    audit_state: Option<AppState>,
) {
    let mut explicit_close = false;
    'actor: loop {
        tokio::select! {
            _ = runtime.cancel.cancelled() => {
                explicit_close = true;
                set_phase(&runtime, SessionPhase::Closing, None, None).await;
                shell.close().await;
                break;
            }
            _ = shutdown.cancelled() => {
                explicit_close = true;
                set_phase(&runtime, SessionPhase::Closing, None, None).await;
                shell.close().await;
                break;
            }
            command = commands.recv() => {
                let Some(command) = command else {
                    explicit_close = true;
                    shell.close().await;
                    break;
                };
                match command {
                    SessionCommand::Input { attachment_id, generation, sequence, data, response } => {
                        let validation = {
                            let mut state = runtime.state.lock().await;
                            state.validate_controller(&attachment_id, generation).and_then(|()| {
                                if sequence == 0 {
                                    Err(TerminalError::invalid("terminal input sequence must be positive"))
                                } else {
                                    Ok(sequence > state.attachments
                                        .get(&attachment_id)
                                        .map_or(0, |attachment| attachment.last_input_sequence))
                                }
                            })
                        };
                        match validation {
                            Ok(false) => { let _ = response.send(Ok(())); }
                            Ok(true) => {
                                let result = tokio::time::timeout(
                                    SHELL_COMMAND_TIMEOUT,
                                    shell.input(data),
                                )
                                .await
                                .unwrap_or_else(|_| {
                                    Err(TerminalError::new(
                                        TerminalErrorCode::SessionLost,
                                        "terminal input timed out",
                                    ))
                                });
                                let failure = result.as_ref().err().cloned();
                                if result.is_ok() {
                                    let mut state = runtime.state.lock().await;
                                    if let Some(attachment) =
                                        state.attachments.get_mut(&attachment_id)
                                        && attachment.generation == generation
                                    {
                                        attachment.last_input_sequence = sequence;
                                    }
                                }
                                if let Some(error) = failure {
                                    set_phase(
                                        &runtime,
                                        SessionPhase::Lost,
                                        Some((error.code, error.message.clone())),
                                        None,
                                    )
                                    .await;
                                    let _ = response.send(Err(error));
                                    break 'actor;
                                }
                                let _ = response.send(result);
                            }
                            Err(error) => { let _ = response.send(Err(error)); }
                        }
                    }
                    SessionCommand::Resize { attachment_id, generation, revision, cols, rows, response } => {
                        let validation = {
                            let mut state = runtime.state.lock().await;
                            state.validate_controller(&attachment_id, generation).map(|()| {
                                revision > state.attachments
                                    .get(&attachment_id)
                                    .map_or(0, |attachment| attachment.last_resize_revision)
                            })
                        };
                        match validation {
                            Ok(false) => { let _ = response.send(Ok(())); }
                            Ok(true) => {
                                let result = tokio::time::timeout(
                                    SHELL_COMMAND_TIMEOUT,
                                    shell.resize(cols, rows),
                                )
                                .await
                                .unwrap_or_else(|_| {
                                    Err(TerminalError::new(
                                        TerminalErrorCode::SessionLost,
                                        "terminal resize timed out",
                                    ))
                                });
                                let failure = result.as_ref().err().cloned();
                                if result.is_ok() {
                                    let mut state = runtime.state.lock().await;
                                    if let Some(attachment) = state.attachments.get_mut(&attachment_id)
                                        && attachment.generation == generation
                                    {
                                        attachment.last_resize_revision = revision;
                                    }
                                    state.session.cols = cols;
                                    state.session.rows = rows;
                                    state.session.updated_at = now_iso();
                                    state.terminal.resize(rows as u16, cols as u16);
                                }
                                if let Some(error) = failure {
                                    set_phase(
                                        &runtime,
                                        SessionPhase::Lost,
                                        Some((error.code, error.message.clone())),
                                        None,
                                    )
                                    .await;
                                    let _ = response.send(Err(error));
                                    break 'actor;
                                }
                                let _ = response.send(result);
                            }
                            Err(error) => { let _ = response.send(Err(error)); }
                        }
                    }
                }
            }
            message = shell.next_event() => {
                match message {
                    ShellEvent::Data(data) => {
                        let mut state = runtime.state.lock().await;
                        state.terminal.process(&data);
                        if state.attachments.is_empty() {
                            state.output.discard_output(&data);
                        } else {
                            state.output.push_output(data);
                        }
                        state.session.updated_at = now_iso();
                        drop(state);
                        runtime.output_notify.notify_waiters();
                    }
                    ShellEvent::Exited(exit_status) => {
                        set_phase(&runtime, SessionPhase::Exited, None, Some(exit_status)).await;
                        break;
                    }
                    ShellEvent::Signaled(error_message) => {
                        set_phase(
                            &runtime,
                            SessionPhase::Exited,
                            Some((TerminalErrorCode::SessionLost, error_message)),
                            None,
                        ).await;
                        break;
                    }
                    ShellEvent::Closed => break,
                    ShellEvent::Other => {}
                }
            }
        }
    }
    shell.disconnect().await;
    let phase = runtime.state.lock().await.session.phase;
    if explicit_close {
        if phase == SessionPhase::Closing {
            set_phase(&runtime, SessionPhase::Closed, None, None).await;
        }
    } else if phase == SessionPhase::Running {
        set_phase(
            &runtime,
            SessionPhase::Lost,
            Some((
                TerminalErrorCode::SessionLost,
                "terminal connection was lost".to_string(),
            )),
            None,
        )
        .await;
    }
    let snapshot = runtime.state.lock().await.session.clone();
    let action = match snapshot.phase {
        SessionPhase::Lost => Some("session_lost"),
        SessionPhase::Exited => Some("session_exited"),
        SessionPhase::Closed => Some("session_ended"),
        _ => None,
    };
    if let (Some(action), Some(audit_state)) = (action, audit_state.as_ref()) {
        publish_session_audit(
            audit_state,
            action,
            &snapshot.target_id,
            &snapshot.id,
            snapshot.error_code,
        )
        .await;
    }
}

async fn publish_session_audit(
    state: &AppState,
    action: &str,
    target_id: &str,
    session_id: &str,
    error_code: Option<TerminalErrorCode>,
) {
    let error_code = error_code.map(|code| code.to_string());
    let result = if target_id == LOCAL_TARGET_ID {
        let (execution_identity, privileged) = local::audit_context(state);
        crate::system_events::publish_local_terminal_audit_event(
            state,
            action,
            Some(target_id),
            Some(session_id),
            None,
            error_code.as_deref(),
            (&execution_identity, privileged),
        )
        .await
    } else {
        crate::system_events::publish_terminal_audit_event(
            state,
            action,
            Some(target_id),
            Some(session_id),
            None,
            error_code.as_deref(),
        )
        .await
    };
    if let Err(error) = result {
        tracing::warn!(action, %error, "failed to publish terminal session audit event");
    }
}

async fn set_phase(
    runtime: &RuntimeSession,
    phase: SessionPhase,
    error: Option<(TerminalErrorCode, String)>,
    exit_code: Option<u32>,
) {
    let (session_id, target_id, error_code) = {
        let mut state = runtime.state.lock().await;
        state.set_phase(phase, error, exit_code);
        (
            state.session.id.clone(),
            state.session.target_id.clone(),
            state.session.error_code,
        )
    };
    if matches!(
        phase,
        SessionPhase::Closed | SessionPhase::Exited | SessionPhase::Lost
    ) {
        tracing::info!(
            %session_id,
            %target_id,
            ?phase,
            error_code = error_code.map(|value| value.to_string()),
            "terminal session reached a terminal phase"
        );
    }
    runtime.output_notify.notify_waiters();
}

fn sanitize_title(value: &str) -> TerminalResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(TerminalError::invalid("terminal session title is invalid"));
    }
    Ok(value.to_string())
}

fn attachment_expired() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::AttachmentExpired,
        "terminal attachment expired",
    )
}

fn session_lost() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::SessionLost,
        "terminal session is no longer connected",
    )
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    use async_trait::async_trait;

    use crate::system::terminal::{
        domain::{AuthMethod, HostKeyProbeResult, TargetRecord},
        shell::InteractiveShell,
        ssh::SshConnector,
    };

    #[test]
    fn terminal_osc_callback_buffer_stays_bounded_in_application_feature_graph() {
        #[derive(Default)]
        struct Observed(Vec<usize>);
        impl vt100::Callbacks for Observed {
            fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
                self.0.push(title.len());
            }
        }
        let mut parser = vt100::Parser::new_with_callbacks(4, 40, 8, Observed::default());
        parser.process(b"\x1b]2;");
        for _ in 0..4 {
            parser.process(&[b'A'; 16 * 1024]);
        }
        parser.process(b"\x1b");
        parser.process(b"\\visible\x1b]2;normal\x07");
        assert!(parser.callbacks().0[0] > 0);
        assert!(
            parser.callbacks().0[0] <= 1024,
            "dependency feature unification must not enable unbounded vte OSC storage"
        );
        assert_eq!(parser.callbacks().0[1], 6);
        assert_eq!(parser.screen().contents(), "visible");
    }

    #[test]
    fn terminal_parser_survives_resize_that_truncates_a_wide_character() {
        let mut parser = vt100::Parser::new(2, 40, 0);
        parser.process(format!("{}你", "x".repeat(38)).as_bytes());

        parser.screen_mut().set_size(2, 39);
        parser.process(b"\x1b[K");

        assert_eq!(parser.screen().size(), (2, 39));
    }

    struct FailingConnector;

    #[async_trait]
    impl SshConnector for FailingConnector {
        async fn probe_host_key(
            &self,
            _host: &str,
            _port: u16,
        ) -> TerminalResult<HostKeyProbeResult> {
            unreachable!()
        }

        async fn test_connection(
            &self,
            _target: &TargetRecord,
            _credential: SshCredential,
        ) -> TerminalResult<u64> {
            unreachable!()
        }

        async fn open_shell(
            &self,
            _target: &TargetRecord,
            _credential: SshCredential,
            _cols: u32,
            _rows: u32,
            progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
        ) -> TerminalResult<BoxedShell> {
            for phase in [
                SessionPhase::Resolving,
                SessionPhase::Connecting,
                SessionPhase::VerifyingHostKey,
                SessionPhase::Authenticating,
            ] {
                if let Some(progress) = progress {
                    progress.send(phase).unwrap();
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
            Err(TerminalError::new(
                TerminalErrorCode::AuthenticationFailed,
                "mock authentication failed",
            ))
        }
    }

    struct MockShell {
        events: mpsc::UnboundedReceiver<ShellEvent>,
        inputs: StdArc<StdMutex<Vec<Vec<u8>>>>,
        resizes: StdArc<StdMutex<Vec<(u32, u32)>>>,
    }

    struct FailingInputShell;

    #[async_trait]
    impl InteractiveShell for FailingInputShell {
        async fn next_event(&mut self) -> ShellEvent {
            std::future::pending().await
        }

        async fn input(&mut self, _data: Vec<u8>) -> TerminalResult<()> {
            Err(session_lost())
        }

        async fn resize(&mut self, _cols: u32, _rows: u32) -> TerminalResult<()> {
            Ok(())
        }

        async fn close(&mut self) {}

        async fn disconnect(&mut self) {}
    }

    #[async_trait]
    impl InteractiveShell for MockShell {
        async fn next_event(&mut self) -> ShellEvent {
            self.events.recv().await.unwrap_or(ShellEvent::Closed)
        }

        async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()> {
            self.inputs.lock().unwrap().push(data);
            Ok(())
        }

        async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()> {
            self.resizes.lock().unwrap().push((cols, rows));
            Ok(())
        }

        async fn close(&mut self) {}

        async fn disconnect(&mut self) {}
    }

    async fn running_mock_session(
        runtime: &TerminalRuntime,
    ) -> (
        TerminalSession,
        mpsc::UnboundedSender<ShellEvent>,
        StdArc<StdMutex<Vec<Vec<u8>>>>,
        StdArc<StdMutex<Vec<(u32, u32)>>>,
    ) {
        let pending = runtime
            .begin_session("target-a".to_string(), "shell".to_string(), 120, 32)
            .await
            .unwrap();
        let (progress, progress_task) = pending.progress_channel();
        for phase in [
            SessionPhase::Resolving,
            SessionPhase::Connecting,
            SessionPhase::VerifyingHostKey,
            SessionPhase::Authenticating,
            SessionPhase::OpeningChannel,
            SessionPhase::RequestingPty,
            SessionPhase::Running,
        ] {
            progress.send(phase).unwrap();
        }
        drop(progress);
        progress_task.await.unwrap();
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let inputs = StdArc::new(StdMutex::new(Vec::new()));
        let resizes = StdArc::new(StdMutex::new(Vec::new()));
        let shell: BoxedShell = Box::new(MockShell {
            events: events_rx,
            inputs: StdArc::clone(&inputs),
            resizes: StdArc::clone(&resizes),
        });
        let session = runtime
            .activate_session(pending, shell, CancellationToken::new())
            .await
            .unwrap();
        (session, events_tx, inputs, resizes)
    }

    #[tokio::test]
    async fn session_task_cleanup_joins_abort_and_owns_cancelled_waits() {
        struct Dropped(Option<oneshot::Sender<()>>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }

        for cancel_wait in [false, true] {
            let (dropped_tx, mut dropped_rx) = oneshot::channel();
            let resource = Dropped(Some(dropped_tx));
            let task = tokio::spawn(async move {
                let _resource = resource;
                std::future::pending::<()>().await;
            });
            let task = AbortOnDropHandle::new(task);
            if cancel_wait {
                let wait = finish_session_task(task, Duration::from_secs(60));
                assert!(
                    tokio::time::timeout(Duration::from_millis(20), wait)
                        .await
                        .is_err()
                );
                tokio::time::timeout(Duration::from_secs(1), dropped_rx)
                    .await
                    .expect("cancelling cleanup must still abort its actor")
                    .unwrap();
            } else {
                finish_session_task(task, Duration::from_millis(20)).await;
                dropped_rx
                    .try_recv()
                    .expect("cleanup must join the abort before returning");
            }
        }
    }

    #[tokio::test]
    async fn cancelled_termination_does_not_leave_an_actorless_running_session() {
        for shutdown_all in [false, true] {
            let runtime = TerminalRuntime::new();
            let (session, events, _, _) = running_mock_session(&runtime).await;
            let retained = runtime.session(&session.id).await.unwrap();
            // Prevent the actor from completing its normal Closing transition,
            // then cancel the API future while it waits for actor cleanup.
            let state = retained.state.lock().await;
            assert_eq!(state.session.phase, SessionPhase::Running);
            let cleanup = async {
                if shutdown_all {
                    runtime.shutdown_all().await;
                } else {
                    runtime.terminate(&session.id).await.unwrap();
                }
            };
            assert!(
                tokio::time::timeout(Duration::from_millis(20), cleanup)
                    .await
                    .is_err()
            );
            assert!(runtime.session(&session.id).await.is_err());
            assert!(runtime.actor_tasks.lock().await.is_empty());
            drop(state);
            tokio::time::timeout(Duration::from_secs(1), events.closed())
                .await
                .expect("cancelled cleanup must release the shell event receiver");
        }
    }

    #[test]
    fn output_buffer_is_bounded_and_detects_cursor_gaps() {
        let mut output = OutputBuffer::new();
        output.push_output(vec![1; OUTPUT_CAPACITY_BYTES]);
        output.push_output(vec![2; 16]);
        assert!(output.retained_bytes <= OUTPUT_CAPACITY_BYTES);
        assert!(output.first_cursor() > 1);
        assert!(output.events.iter().all(|event| {
            event
                .data
                .as_ref()
                .is_none_or(|data| data.len() <= OUTPUT_RESPONSE_BYTES)
        }));
        assert_eq!(output.latest_cursor(), 17);
    }

    #[test]
    fn output_buffer_releases_acknowledged_and_unobserved_data() {
        let mut output = OutputBuffer::new();
        output.push_output(vec![1; OUTPUT_RESPONSE_BYTES + 1]);
        let latest = output.latest_cursor();
        assert_eq!(output.events.len(), 2);

        output.discard_through(latest);
        assert!(output.events.is_empty());
        assert_eq!(output.retained_bytes, 0);
        assert_eq!(output.latest_cursor(), latest);

        output.push_output(vec![2; OUTPUT_RESPONSE_BYTES + 1]);
        let before_discard = output.latest_cursor();
        output.discard_output(&vec![3; OUTPUT_RESPONSE_BYTES + 1]);
        assert!(output.events.is_empty());
        assert_eq!(output.retained_bytes, 0);
        assert_eq!(output.latest_cursor(), before_discard + 2);
    }

    #[test]
    fn vt_snapshot_contains_main_scrollback_but_not_hidden_main_screen_in_alt_mode() {
        let mut parser = vt100::Parser::new(3, 24, SCROLLBACK_ROWS);
        parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
        let snapshot = terminal_snapshot(&mut parser);
        let rendered = String::from_utf8_lossy(&snapshot);
        assert!(rendered.contains("one"));
        assert!(rendered.contains("five"));

        parser.process(b"\x1b[?1049hALT-SCREEN");
        let alternate_snapshot = terminal_snapshot(&mut parser);
        let alternate = String::from_utf8_lossy(&alternate_snapshot);
        assert!(alternate.contains("ALT-SCREEN"));
        assert!(!alternate.contains("one"));
    }

    #[tokio::test]
    async fn terminal_archives_preserve_replay_and_bound_decoding() {
        for alternate in [false, true] {
            let mut parser = vt100::Parser::new(3, 40, SCROLLBACK_ROWS);
            parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
            if alternate {
                parser.process(b"\x1b[?1049hALT\x1b[31m-red");
            }
            let expected = terminal_snapshot(&mut parser);
            let mut terminal = RuntimeTerminal::Live(Box::new(parser));
            terminal.compact().unwrap();
            assert_eq!(
                terminal.snapshot().await.unwrap().as_ref().as_ref(),
                expected
            );
            let RuntimeTerminal::Archived(compressed) = &terminal else {
                panic!("not archived");
            };
            assert!(decode_terminal_archive(&compressed.compressed, expected.len() - 1).is_err());
            assert_eq!(
                decode_terminal_archive(&compressed.compressed, expected.len())
                    .unwrap()
                    .as_ref(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn archived_replay_is_shared_between_viewers_and_released_after_acknowledgement() {
        let runtime = TerminalRuntime::new();
        let mut pending = runtime
            .begin_session("archived".into(), "archived".into(), 80, 24)
            .await
            .unwrap();
        let id = pending.id.clone();
        let payload = vec![b'x'; OUTPUT_RESPONSE_BYTES * 2 + 17];
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&payload).unwrap();
        {
            let mut state = pending.runtime.state.lock().await;
            state.set_phase(SessionPhase::Failed, None, None);
            state.terminal =
                RuntimeTerminal::Archived(Bytes::from(encoder.finish().unwrap()).into());
        }
        pending.activated = true;
        let mut viewers = Vec::new();
        let mut last_response = None;
        for _ in 0..MAX_ATTACHMENTS {
            let viewer = runtime.create_attachment(&id, None, None).await.unwrap();
            last_response = Some(runtime.events(&viewer.id, 0, Duration::ZERO).await.unwrap());
            viewers.push(viewer);
        }
        let replay = {
            let state = pending.runtime.state.lock().await;
            let transfers = state
                .attachments
                .values()
                .map(|attachment| attachment.snapshot.as_ref().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(Arc::strong_count(&transfers[0].data), MAX_ATTACHMENTS);
            assert!(
                transfers
                    .iter()
                    .all(|transfer| Arc::ptr_eq(&transfers[0].data, &transfer.data))
            );
            Arc::downgrade(&transfers[0].data)
        };
        for viewer in &viewers[..MAX_ATTACHMENTS - 1] {
            runtime.detach(&viewer.id).await.unwrap();
        }
        assert_eq!(replay.strong_count(), 1);
        let viewer = viewers.last().unwrap();
        let mut response = last_response.unwrap();
        let mut received = Vec::new();
        loop {
            for event in &response.events {
                if let Some(data) = &event.data_base64 {
                    received.extend(STANDARD.decode(data).unwrap());
                }
            }
            response = runtime
                .events(&viewer.id, response.next_cursor, Duration::ZERO)
                .await
                .unwrap();
            if response.events.is_empty() {
                break;
            }
        }
        assert_eq!(received, payload);
        assert!(
            replay.upgrade().is_none(),
            "the last acknowledgement must release decoded history"
        );
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn ended_terminal_compacts_after_grace_and_reconnects_without_losing_output() {
        let runtime = TerminalRuntime::new();
        let (session, events_tx, _, _) = running_mock_session(&runtime).await;
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let mut output = Vec::new();
        for line in 0..2_100 {
            output.extend_from_slice(format!("{line:04} replay-history\r\n").as_bytes());
        }
        events_tx.send(ShellEvent::Data(output)).unwrap();
        events_tx.send(ShellEvent::Exited(7)).unwrap();
        let task = runtime
            .actor_tasks
            .lock()
            .await
            .remove(&session.id)
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        let retained = runtime.session(&session.id).await.unwrap();
        let expected = {
            let mut state = retained.state.lock().await;
            let expected = state.terminal.snapshot().await.unwrap();
            state.closed_at = Some(Instant::now() - CLOSED_TERMINAL_COMPACT_AFTER);
            expected
        };
        runtime.expire_attachments().await;
        assert!(
            matches!(
                retained.state.lock().await.terminal,
                RuntimeTerminal::Live(_)
            ),
            "a present viewer keeps the parser until it detaches"
        );
        runtime.detach(&attachment.id).await.unwrap();
        {
            let mut state = retained.state.lock().await;
            state.closed_at = Some(Instant::now());
        }
        runtime.expire_attachments().await;
        assert!(
            matches!(
                retained.state.lock().await.terminal,
                RuntimeTerminal::Live(_)
            ),
            "recently ended sessions keep their grace period"
        );
        retained.state.lock().await.closed_at =
            Some(Instant::now() - CLOSED_TERMINAL_COMPACT_AFTER);
        runtime.expire_attachments().await;
        {
            let state = retained.state.lock().await;
            assert!(matches!(state.terminal, RuntimeTerminal::Archived(_)));
            assert!(state.terminal.archived_bytes() < expected.len());
            assert_eq!(state.session.phase, SessionPhase::Exited);
            assert_eq!(state.session.exit_code, Some(7));
        }
        let viewer = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        assert_eq!(viewer.role, AttachmentRole::Viewer);
        let replay = runtime.events(&viewer.id, 0, Duration::ZERO).await.unwrap();
        let retry = runtime.events(&viewer.id, 0, Duration::ZERO).await.unwrap();
        assert_eq!(
            serde_json::to_value(&replay).unwrap(),
            serde_json::to_value(&retry).unwrap()
        );
        let bytes = replay
            .events
            .iter()
            .filter_map(|event| event.data_base64.as_ref())
            .flat_map(|value| STANDARD.decode(value).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bytes.as_slice(), expected.as_ref().as_ref());
        assert!(replay.events.iter().any(|event| event.reset));
        assert!(replay.events.iter().any(|event| event.exit_code == Some(7)));
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn archive_budget_evicts_oldest_unattached_and_preserves_live_sessions() {
        let runtime = TerminalRuntime::new();
        let mut ids = Vec::new();
        for index in 0..3 {
            let (session, events, _, _) = running_mock_session(&runtime).await;
            events.send(ShellEvent::Exited(0)).unwrap();
            let task = runtime
                .actor_tasks
                .lock()
                .await
                .remove(&session.id)
                .unwrap();
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap();
            let retained = runtime.session(&session.id).await.unwrap();
            let mut state = retained.state.lock().await;
            state.terminal = RuntimeTerminal::Archived(Bytes::from(vec![0; 128]).into());
            state.session.updated_at = index.to_string();
            ids.push(session.id);
        }
        let _viewer = runtime
            .create_attachment(&ids[2], None, None)
            .await
            .unwrap();
        let (live, events, _, _) = running_mock_session(&runtime).await;
        let retained = runtime.session(&live.id).await.unwrap();
        retained.state.lock().await.closed_at =
            Some(Instant::now() - CLOSED_TERMINAL_COMPACT_AFTER);
        runtime.expire_attachments().await;
        assert!(matches!(
            retained.state.lock().await.terminal,
            RuntimeTerminal::Live(_)
        ));
        assert!(
            !events.is_closed(),
            "detached live shell must remain running"
        );
        runtime.enforce_archive_budget(256).await;
        assert!(runtime.session(&ids[0]).await.is_err());
        assert!(runtime.session(&ids[1]).await.is_ok());
        runtime.enforce_archive_budget(0).await;
        assert!(runtime.session(&ids[1]).await.is_err());
        assert!(
            runtime.session(&ids[2]).await.is_ok(),
            "attached history cannot be evicted"
        );
        assert!(runtime.session(&live.id).await.is_ok());
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn long_osc_output_is_forwarded_unchanged_while_parser_recovers() {
        let runtime = TerminalRuntime::new();
        let (session, events_tx, _, _) = running_mock_session(&runtime).await;
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let initial = runtime
            .events(&attachment.id, 0, Duration::ZERO)
            .await
            .unwrap();
        runtime
            .events(&attachment.id, initial.next_cursor, Duration::ZERO)
            .await
            .unwrap();
        let mut raw = b"\x1b]52;c;".to_vec();
        raw.extend(vec![b'A'; 64 * 1024]);
        raw.extend_from_slice(b"\x1b\\visible-after-osc");
        for chunk in raw.chunks(1024) {
            events_tx.send(ShellEvent::Data(chunk.to_vec())).unwrap();
        }
        let mut collected = Vec::new();
        let mut cursor = initial.next_cursor;
        tokio::time::timeout(Duration::from_secs(2), async {
            while collected.len() < raw.len() {
                let result = runtime
                    .events(&attachment.id, cursor, Duration::from_millis(50))
                    .await
                    .unwrap();
                cursor = result.next_cursor;
                for event in result.events {
                    if let Some(data) = event.data_base64 {
                        collected.extend(STANDARD.decode(data).unwrap());
                    }
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(collected, raw);
        let retained = runtime.session(&session.id).await.unwrap();
        let state = retained.state.lock().await;
        let RuntimeTerminal::Live(parser) = &state.terminal else {
            panic!("live terminal archived");
        };
        assert_eq!(parser.screen().contents(), "visible-after-osc");
        drop(state);
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn asynchronous_initialization_exposes_progress_and_retains_failure() {
        let runtime = TerminalRuntime::with_connector(Arc::new(FailingConnector));
        let pending = runtime
            .begin_session("target-a".to_string(), "shell".to_string(), 120, 32)
            .await
            .unwrap();
        let attachment = runtime
            .create_attachment(&pending.id, None, None)
            .await
            .unwrap();
        let target = TargetRecord {
            id: "target-a".to_string(),
            name: "mock".to_string(),
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "operator".to_string(),
            auth_method: AuthMethod::Password,
            trusted_host_key: None,
            revision: 1,
            last_verified_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let guard = runtime.target_operation("target-a").await;
        let creating = runtime
            .start_session(SessionStartup {
                pending,
                backend: SessionStartupBackend::Ssh {
                    target,
                    credential: SshCredential::Password("secret".to_string()),
                },
                initial_cols: 120,
                initial_rows: 32,
                shutdown: CancellationToken::new(),
                target_guard: guard,
                audit_state: None,
            })
            .await
            .unwrap();
        assert_eq!(creating.phase, SessionPhase::Creating);

        for _ in 0..40 {
            if runtime
                .list()
                .await
                .sessions
                .iter()
                .any(|session| session.id == creating.id && session.phase == SessionPhase::Failed)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let failed = runtime
            .list()
            .await
            .sessions
            .into_iter()
            .find(|session| session.id == creating.id)
            .unwrap();
        assert_eq!(failed.phase, SessionPhase::Failed);
        assert_eq!(
            failed.error_code,
            Some(TerminalErrorCode::AuthenticationFailed)
        );
        let events = runtime
            .events(&attachment.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(events.events.iter().any(|event| {
            event.kind == TerminalEventType::Status
                && event.phase == Some(SessionPhase::Authenticating)
        }));
        assert!(events.events.iter().any(|event| {
            event.kind == TerminalEventType::Status
                && event.phase == Some(SessionPhase::Failed)
                && event.error_code == Some(TerminalErrorCode::AuthenticationFailed)
        }));
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn retained_terminal_sessions_are_bounded_and_oldest_unattached_is_evicted() {
        let runtime = TerminalRuntime::new();
        for index in 0..MAX_RETAINED_SESSIONS {
            let mut pending = runtime
                .begin_session(format!("target-{index}"), format!("failed-{index}"), 80, 24)
                .await
                .unwrap();
            pending.runtime.state.lock().await.set_phase(
                SessionPhase::Failed,
                Some((TerminalErrorCode::ConnectTimeout, "failed".to_string())),
                None,
            );
            pending.activated = true;
        }
        assert_eq!(runtime.list().await.sessions.len(), MAX_RETAINED_SESSIONS);
        let mut replacement = runtime
            .begin_session("replacement".to_string(), "replacement".to_string(), 80, 24)
            .await
            .unwrap();
        replacement.runtime.state.lock().await.set_phase(
            SessionPhase::Failed,
            Some((TerminalErrorCode::ConnectTimeout, "failed".to_string())),
            None,
        );
        replacement.activated = true;
        let sessions = runtime.list().await.sessions;
        assert_eq!(sessions.len(), MAX_RETAINED_SESSIONS);
        assert!(
            sessions
                .iter()
                .any(|session| session.target_id == "replacement")
        );
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn retained_session_eviction_rechecks_a_reconnected_viewer() {
        let runtime = TerminalRuntime::new();
        for index in 0..MAX_RETAINED_SESSIONS {
            let mut pending = runtime
                .begin_session(format!("failed-{index}"), "failed".into(), 80, 24)
                .await
                .unwrap();
            pending
                .runtime
                .state
                .lock()
                .await
                .set_phase(SessionPhase::Failed, None, None);
            pending.activated = true;
        }
        // Freeze the last candidate scan after the first candidate was read,
        // then reconnect to that first (oldest) session before eviction.
        let sessions = runtime
            .sessions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let victim = &sessions[0];
        let id = {
            let mut state = victim.state.lock().await;
            state.session.updated_at = "0000".into();
            state.session.id.clone()
        };
        let blocked = sessions.last().unwrap().state.lock().await;
        let mut eviction = Box::pin(runtime.make_room_for_session());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut eviction)
                .await
                .is_err()
        );
        let viewer = runtime.create_attachment(&id, None, None).await.unwrap();
        drop(blocked);
        eviction.await.unwrap();
        assert!(
            runtime.session(&id).await.is_ok(),
            "a reconnected viewer must not be evicted"
        );
        assert!(runtime.events(&viewer.id, 0, Duration::ZERO).await.is_ok());
        assert_eq!(
            runtime.sessions.read().await.len(),
            MAX_RETAINED_SESSIONS - 1
        );
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn local_and_ssh_sessions_share_global_and_per_target_limits() {
        let runtime = TerminalRuntime::new();
        for _ in 0..MAX_TARGET_SESSIONS {
            runtime
                .reserve_active_test_session(LOCAL_TARGET_ID)
                .await
                .unwrap();
        }
        let local_limit = match runtime.reserve_active_test_session(LOCAL_TARGET_ID).await {
            Ok(_) => panic!("fifth local terminal session should be rejected"),
            Err(error) => error,
        };
        assert_eq!(local_limit.code, TerminalErrorCode::SessionLimitReached);
        for target_id in ["ssh-a", "ssh-b"] {
            for _ in 0..MAX_TARGET_SESSIONS {
                runtime
                    .reserve_active_test_session(target_id)
                    .await
                    .unwrap();
            }
        }
        let global_limit = match runtime.reserve_active_test_session("ssh-c").await {
            Ok(_) => panic!("thirteenth terminal session should be rejected"),
            Err(error) => error,
        };
        assert_eq!(global_limit.code, TerminalErrorCode::SessionLimitReached);
        let sessions = runtime.list().await.sessions;
        assert_eq!(sessions.len(), MAX_GLOBAL_SESSIONS);
        assert_eq!(
            sessions
                .iter()
                .filter(|session| session.backend == SessionBackend::Local)
                .count(),
            MAX_TARGET_SESSIONS
        );
        assert_eq!(
            sessions
                .iter()
                .filter(|session| session.backend == SessionBackend::Ssh)
                .count(),
            MAX_GLOBAL_SESSIONS - MAX_TARGET_SESSIONS
        );
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn keyed_locks_and_verification_grants_remain_bounded() {
        let runtime = TerminalRuntime::new();
        for _ in 0..256 {
            let guard = runtime.target_operation(&Uuid::new_v4().to_string()).await;
            drop(guard);
        }
        // Dead weak entries are pruned on every lookup; only the most recent
        // entry can remain after its guard is dropped.
        assert!(runtime.target_operations.lock().await.len() <= 1);

        let token = runtime.issue_verification("one".to_string()).await;
        assert!(runtime.consume_verification(&token, "one").await);
        assert!(!runtime.consume_verification(&token, "one").await);
        for index in 0..=MAX_VERIFICATION_GRANTS {
            runtime.issue_verification(format!("grant-{index}")).await;
        }
        assert_eq!(
            runtime.verification_grants.lock().await.len(),
            MAX_VERIFICATION_GRANTS
        );

        let confirmation = runtime
            .issue_force_confirmation(
                "target-a",
                7,
                vec!["session-b".to_string(), "session-a".to_string()],
            )
            .await;
        assert!(
            runtime
                .consume_force_confirmation(
                    &confirmation,
                    "target-a",
                    7,
                    &["session-a".to_string()],
                )
                .await,
            "a previously confirmed session may exit before force retry"
        );
        let confirmation = runtime
            .issue_force_confirmation("target-a", 7, vec!["session-a".to_string()])
            .await;
        assert!(
            !runtime
                .consume_force_confirmation(
                    &confirmation,
                    "target-a",
                    7,
                    &["session-a".to_string(), "session-new".to_string()],
                )
                .await,
            "a new session must invalidate the destructive confirmation"
        );
        for index in 0..=MAX_FORCE_CONFIRMATION_GRANTS {
            runtime
                .issue_force_confirmation("target-a", 7, vec![format!("session-{index}")])
                .await;
        }
        assert_eq!(
            runtime.force_confirmation_grants.lock().await.len(),
            MAX_FORCE_CONFIRMATION_GRANTS
        );
    }

    #[test]
    fn phase_machine_rejects_resurrection() {
        assert!(SessionPhase::Running.can_transition_to(SessionPhase::Lost));
        assert!(!SessionPhase::Lost.can_transition_to(SessionPhase::Running));
        assert!(!SessionPhase::Exited.is_active());
    }

    #[tokio::test]
    async fn actor_fences_controllers_and_scopes_sequences_per_attachment() {
        let runtime = TerminalRuntime::new();
        let (session, events_tx, inputs, resizes) = running_mock_session(&runtime).await;
        let first = runtime
            .create_attachment(&session.id, Some(110), Some(31))
            .await
            .unwrap();
        runtime
            .send_input(&first.id, first.generation, 1, b"one".to_vec())
            .await
            .unwrap();
        runtime
            .send_input(&first.id, first.generation, 1, b"duplicate".to_vec())
            .await
            .unwrap();
        runtime
            .resize(&first.id, first.generation, 1, 100, 30)
            .await
            .unwrap();
        assert_eq!(inputs.lock().unwrap().as_slice(), [b"one".to_vec()]);
        assert_eq!(resizes.lock().unwrap().as_slice(), [(110, 31), (100, 30)]);

        let second = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let third = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        assert_eq!(second.role, AttachmentRole::Viewer);
        assert_eq!(third.role, AttachmentRole::Viewer);
        let second = runtime
            .claim_control(&second.id, Some(second.generation))
            .await
            .unwrap();
        runtime
            .send_input(&second.id, second.generation, 1, b"two".to_vec())
            .await
            .unwrap();
        assert!(matches!(
            runtime
                .send_input(&first.id, first.generation, 2, b"stale".to_vec())
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::ControllerConflict,
                ..
            })
        ));
        assert_eq!(
            inputs.lock().unwrap().as_slice(),
            [b"one".to_vec(), b"two".to_vec()]
        );

        let third_events = runtime
            .events(&third.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        let third_generation = third_events
            .events
            .iter()
            .find(|event| event.kind == TerminalEventType::Control)
            .and_then(|event| event.generation)
            .unwrap();
        let third = runtime
            .claim_control(&third.id, Some(third_generation))
            .await
            .unwrap();
        runtime
            .send_input(&third.id, third.generation, 1, b"three".to_vec())
            .await
            .unwrap();
        assert_eq!(
            inputs.lock().unwrap().as_slice(),
            [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );

        let first_events = runtime
            .events(&first.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(first_events.events.iter().any(|event| {
            event.kind == TerminalEventType::Control && event.role == Some(AttachmentRole::Viewer)
        }));

        events_tx.send(ShellEvent::Closed).unwrap();
        for _ in 0..20 {
            if runtime
                .list()
                .await
                .sessions
                .iter()
                .any(|item| item.id == session.id && item.phase == SessionPhase::Lost)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            runtime
                .list()
                .await
                .sessions
                .into_iter()
                .find(|item| item.id == session.id)
                .unwrap()
                .phase,
            SessionPhase::Lost
        );
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn actor_retains_output_and_reports_remote_exit() {
        let runtime = TerminalRuntime::new();
        let (session, events_tx, _, _) = running_mock_session(&runtime).await;
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let initial = runtime
            .events(&attachment.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        runtime
            .events(
                &attachment.id,
                initial.next_cursor,
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        events_tx
            .send(ShellEvent::Data(b"hello\r\n".to_vec()))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let events = runtime
            .events(
                &attachment.id,
                initial.next_cursor,
                Duration::from_millis(10),
            )
            .await
            .unwrap();
        assert!(events.events.iter().any(|event| {
            event.kind == TerminalEventType::Output
                && event
                    .data_base64
                    .as_ref()
                    .and_then(|data| STANDARD.decode(data).ok())
                    .is_some_and(|data| data == b"hello\r\n")
                && !event.reset
        }));
        runtime
            .events(&attachment.id, events.next_cursor, Duration::from_millis(1))
            .await
            .unwrap();
        let retained = runtime.session(&session.id).await.unwrap();
        let retained = retained.state.lock().await;
        assert_eq!(retained.output.retained_bytes, 0);
        assert!(retained.output.events.is_empty());
        drop(retained);

        events_tx.send(ShellEvent::Exited(7)).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let snapshot = runtime
            .list()
            .await
            .sessions
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(snapshot.phase, SessionPhase::Exited);
        assert_eq!(snapshot.exit_code, Some(7));
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn detaching_the_browser_does_not_end_the_terminal_session() {
        let runtime = TerminalRuntime::new();
        let (session, events_tx, _, _) = running_mock_session(&runtime).await;
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        runtime.detach(&attachment.id).await.unwrap();
        events_tx
            .send(ShellEvent::Data(b"still-running\r\n".to_vec()))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;

        let runtime_session = runtime.session(&session.id).await.unwrap();
        let state = runtime_session.state.lock().await;
        assert!(state.attachments.is_empty());
        assert_eq!(state.output.retained_bytes, 0);
        assert!(state.output.events.is_empty());
        drop(state);

        let snapshot = runtime
            .list()
            .await
            .sessions
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(snapshot.phase, SessionPhase::Running);
        let reattached = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let replay = runtime
            .events(&reattached.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(replay.events.iter().any(|event| {
            event.kind == TerminalEventType::Output
                && event
                    .data_base64
                    .as_ref()
                    .and_then(|data| STANDARD.decode(data).ok())
                    .is_some_and(|data| String::from_utf8_lossy(&data).contains("still-running"))
        }));
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn shell_input_failure_transitions_running_session_to_lost() {
        let runtime = TerminalRuntime::new();
        let pending = runtime
            .begin_session("target-a".to_string(), "shell".to_string(), 80, 24)
            .await
            .unwrap();
        let (progress, progress_task) = pending.progress_channel();
        for phase in [
            SessionPhase::Resolving,
            SessionPhase::Connecting,
            SessionPhase::VerifyingHostKey,
            SessionPhase::Authenticating,
            SessionPhase::OpeningChannel,
            SessionPhase::RequestingPty,
            SessionPhase::Running,
        ] {
            progress.send(phase).unwrap();
        }
        drop(progress);
        progress_task.await.unwrap();
        let session = runtime
            .activate_session(
                pending,
                Box::new(FailingInputShell),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        assert!(matches!(
            runtime
                .send_input(&attachment.id, attachment.generation, 1, b"x".to_vec())
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::SessionLost,
                ..
            })
        ));
        let snapshot = runtime
            .list()
            .await
            .sessions
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(snapshot.phase, SessionPhase::Lost);
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn cursor_gap_snapshot_is_paged_under_output_response_limit() {
        let runtime = TerminalRuntime::new();
        let (session, _events_tx, _, _) = running_mock_session(&runtime).await;
        let runtime_session = runtime.session(&session.id).await.unwrap();
        {
            let mut state = runtime_session.state.lock().await;
            state.terminal.resize(12, 400);
            let mut flood = Vec::new();
            for row in 0..2_050 {
                flood.extend_from_slice(format!("{row:04}{}\r\n", "x".repeat(390)).as_bytes());
            }
            state.terminal.process(&flood);
            state.output.events.pop_front();
        }
        let attachment = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let mut after = 0;
        let mut chunks = 0;
        let mut reset_count = 0;
        let mut snapshot_cursor = None;
        loop {
            let result = runtime
                .events(&attachment.id, after, Duration::from_millis(1))
                .await
                .unwrap();
            let retry = runtime
                .events(&attachment.id, after, Duration::from_millis(1))
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(&result).unwrap(),
                serde_json::to_value(&retry).unwrap(),
                "a lost HTTP response must be replayed byte-for-byte"
            );
            for event in result
                .events
                .iter()
                .filter(|event| event.kind == TerminalEventType::Output)
            {
                let bytes = STANDARD
                    .decode(event.data_base64.as_ref().unwrap())
                    .unwrap();
                assert!(bytes.len() <= OUTPUT_RESPONSE_BYTES);
                chunks += 1;
                reset_count += usize::from(event.reset);
                assert_eq!(*snapshot_cursor.get_or_insert(event.cursor), event.cursor);
            }
            let complete = result
                .events
                .iter()
                .any(|event| event.kind == TerminalEventType::Status);
            after = result.next_cursor;
            if complete {
                break;
            }
            assert!(chunks < 16, "snapshot transfer did not complete");
        }
        assert!(chunks > 1);
        assert_eq!(reset_count, 1);

        // Acknowledge the last snapshot chunk. Subsequent output is resumed
        // strictly after the cursor captured for the snapshot.
        let completed = runtime
            .events(&attachment.id, after, Duration::from_millis(1))
            .await
            .unwrap();
        assert!(completed.events.is_empty());
        assert_eq!(completed.next_cursor, after);
        runtime.shutdown_all().await;
    }

    #[tokio::test]
    async fn control_updates_are_replayed_until_acknowledged() {
        let runtime = TerminalRuntime::new();
        let (session, _events_tx, _, _) = running_mock_session(&runtime).await;
        let controller = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();
        let viewer = runtime
            .create_attachment(&session.id, None, None)
            .await
            .unwrap();

        let initial = runtime
            .events(&controller.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        let initial_retry = runtime
            .events(&controller.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&initial).unwrap(),
            serde_json::to_value(&initial_retry).unwrap()
        );

        let claimed = runtime
            .claim_control(&viewer.id, Some(viewer.generation))
            .await
            .unwrap();
        let stale_retry = runtime
            .events(&controller.id, 0, Duration::from_millis(1))
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&initial).unwrap(),
            serde_json::to_value(&stale_retry).unwrap(),
            "an unacknowledged response stays stable even when control changes"
        );

        let demoted = runtime
            .events(
                &controller.id,
                initial.next_cursor,
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        assert!(demoted.events.iter().any(|event| {
            event.kind == TerminalEventType::Control
                && event.role == Some(AttachmentRole::Viewer)
                && event.generation == Some(claimed.generation)
        }));
        let demoted_retry = runtime
            .events(
                &controller.id,
                initial.next_cursor,
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&demoted).unwrap(),
            serde_json::to_value(&demoted_retry).unwrap()
        );
        runtime.shutdown_all().await;
    }
}
