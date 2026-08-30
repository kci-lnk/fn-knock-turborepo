use std::path::PathBuf;

#[cfg(unix)]
use std::path::Path;

use tokio::sync::mpsc;

use crate::{runtime_profile, state::AppState};

use super::{
    domain::{
        LocalTerminalBlockedReason, LocalTerminalStatus, SessionPhase, TerminalError,
        TerminalErrorCode, TerminalResult,
    },
    repository::LocalSettingsRecord,
    shell::BoxedShell,
};

pub(super) const LOCAL_TARGET_ID: &str = "local";

#[derive(Clone, Debug)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(super) struct LocalTerminalDescriptor {
    pub execution_identity: String,
    pub privileged: bool,
    pub shell: PathBuf,
    pub working_directory: PathBuf,
    #[cfg(unix)]
    account: UnixAccount,
}

#[derive(Clone, Debug)]
struct LocalInspection {
    supported: bool,
    execution_identity: String,
    privileged: bool,
    shell: Option<PathBuf>,
    working_directory: Option<PathBuf>,
}

pub(super) fn status(state: &AppState, settings: LocalSettingsRecord) -> LocalTerminalStatus {
    let inspection = inspect(state);
    let ready = inspection.supported && inspection.shell.is_some();
    let blocked_reason = if !inspection.supported {
        Some(LocalTerminalBlockedReason::UnsupportedPlatform)
    } else if inspection.shell.is_none() {
        Some(LocalTerminalBlockedReason::ShellUnavailable)
    } else {
        None
    };
    LocalTerminalStatus {
        target_id: LOCAL_TARGET_ID.to_string(),
        supported: inspection.supported,
        enabled: settings.enabled,
        ready,
        execution_identity: inspection.execution_identity,
        privileged: inspection.privileged,
        shell: inspection
            .shell
            .map(|path| path.to_string_lossy().into_owned()),
        working_directory: inspection
            .working_directory
            .map(|path| path.to_string_lossy().into_owned()),
        blocked_reason,
        revision: settings.revision,
    }
}

pub(super) fn audit_context(state: &AppState) -> (String, bool) {
    let inspection = inspect(state);
    (inspection.execution_identity, inspection.privileged)
}

pub(super) fn descriptor(state: &AppState) -> TerminalResult<LocalTerminalDescriptor> {
    let inspection = inspect(state);
    if !inspection.supported {
        return Err(TerminalError::new(
            TerminalErrorCode::LocalTerminalUnsupported,
            "local terminal is not supported on this platform",
        ));
    }
    let shell = inspection.shell.ok_or_else(|| {
        TerminalError::new(
            TerminalErrorCode::LocalShellUnavailable,
            "no supported local login shell is available",
        )
    })?;
    let working_directory = inspection
        .working_directory
        .unwrap_or_else(|| PathBuf::from("/"));
    Ok(LocalTerminalDescriptor {
        execution_identity: inspection.execution_identity,
        privileged: inspection.privileged,
        shell,
        working_directory,
        #[cfg(unix)]
        account: current_unix_account(),
    })
}

fn inspect(state: &AppState) -> LocalInspection {
    let supported = local_terminal_supported(state);
    #[cfg(unix)]
    {
        let account = current_unix_account();
        let shell = supported.then(|| resolve_shell(&account)).flatten();
        let working_directory = supported.then(|| resolve_working_directory(&account, state));
        LocalInspection {
            supported,
            execution_identity: account.identity,
            privileged: unsafe { libc::geteuid() } == 0,
            shell,
            working_directory,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = state;
        LocalInspection {
            supported,
            execution_identity: "service".to_string(),
            privileged: false,
            shell: None,
            working_directory: None,
        }
    }
}

fn local_terminal_supported(state: &AppState) -> bool {
    supported_target(&runtime_profile::deployment_target(state))
}

fn supported_target(target: &str) -> bool {
    cfg!(unix) && matches!(target, "fpk" | "linux" | "macos" | "openwrt")
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct UnixAccount {
    identity: String,
    home: Option<PathBuf>,
    shell: Option<PathBuf>,
}

#[cfg(unix)]
fn current_unix_account() -> UnixAccount {
    use std::{ffi::CStr, os::unix::ffi::OsStringExt};

    let uid = unsafe { libc::geteuid() };
    let mut passwd = std::mem::MaybeUninit::<libc::passwd>::zeroed();
    let mut result = std::ptr::null_mut();
    let requested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_len = if requested > 0 {
        usize::try_from(requested).unwrap_or(16 * 1024)
    } else {
        16 * 1024
    }
    .clamp(4 * 1024, 1024 * 1024);
    let mut buffer = vec![0_u8; buffer_len];
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            passwd.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return UnixAccount {
            identity: format!("uid:{uid}"),
            home: None,
            shell: None,
        };
    }
    let passwd = unsafe { passwd.assume_init() };
    let bytes = |pointer: *const libc::c_char| {
        (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
    };
    let identity = bytes(passwd.pw_name)
        .map(|value| String::from_utf8_lossy(&value).into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("uid:{uid}"));
    let home = bytes(passwd.pw_dir)
        .filter(|value| !value.is_empty())
        .map(|value| PathBuf::from(std::ffi::OsString::from_vec(value)));
    let shell = bytes(passwd.pw_shell)
        .filter(|value| !value.is_empty())
        .map(|value| PathBuf::from(std::ffi::OsString::from_vec(value)));
    UnixAccount {
        identity,
        home,
        shell,
    }
}

#[cfg(unix)]
fn resolve_shell(account: &UnixAccount) -> Option<PathBuf> {
    shell_candidates(account)
        .into_iter()
        .find(|path| executable_file(path))
}

#[cfg(unix)]
fn shell_candidates(account: &UnixAccount) -> Vec<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();
    if account
        .shell
        .as_ref()
        .and_then(|path| path.file_name())
        .is_some_and(|name| name == "zsh")
        && let Some(shell) = account.shell.as_ref()
    {
        candidates.push(shell.clone());
    }
    candidates.extend(
        [
            "/bin/zsh",
            "/usr/bin/zsh",
            "/usr/local/bin/zsh",
            "/opt/homebrew/bin/zsh",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    if let Some(shell) = account.shell.as_ref() {
        candidates.push(shell.clone());
    }
    candidates.extend(
        [
            "/bin/bash",
            "/usr/bin/bash",
            "/usr/local/bin/bash",
            "/opt/homebrew/bin/bash",
            "/bin/ash",
            "/usr/bin/ash",
            "/bin/sh",
            "/usr/bin/sh",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
    candidates
}

#[cfg(unix)]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_absolute()
        && std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(unix)]
fn resolve_working_directory(account: &UnixAccount, state: &AppState) -> PathBuf {
    resolve_working_directory_from(account, &state.settings.data_dir)
}

#[cfg(unix)]
fn resolve_working_directory_from(account: &UnixAccount, data_dir: &Path) -> PathBuf {
    account
        .home
        .as_ref()
        .filter(|path| path.is_dir())
        .cloned()
        .or_else(|| data_dir.is_dir().then(|| data_dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(unix)]
pub(super) async fn open_shell(
    descriptor: LocalTerminalDescriptor,
    cols: u32,
    rows: u32,
    progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
) -> TerminalResult<BoxedShell> {
    use portable_pty::{PtySize, native_pty_system};

    let progress = progress.cloned();
    tokio::task::spawn_blocking(move || {
        emit_progress(progress.as_ref(), SessionPhase::OpeningPty);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: rows.min(u16::MAX.into()) as u16,
                cols: cols.min(u16::MAX.into()) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| local_pty_error("failed to open local PTY", error))?;
        let command = local_command(&descriptor);
        emit_progress(progress.as_ref(), SessionPhase::StartingShell);
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| local_pty_error("failed to start local login shell", error))?;
        let child = SpawnedChild::new(child);
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| local_pty_error("failed to open local PTY reader", error))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| local_pty_error("failed to open local PTY writer", error))?;
        LocalShell::spawn(pair.master, reader, writer, child)
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "local PTY initializer panicked");
        TerminalError::new(
            TerminalErrorCode::LocalPtyStartFailed,
            "local PTY initializer failed",
        )
    })?
}

#[cfg(not(unix))]
pub(super) async fn open_shell(
    _descriptor: LocalTerminalDescriptor,
    _cols: u32,
    _rows: u32,
    _progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
) -> TerminalResult<BoxedShell> {
    Err(TerminalError::new(
        TerminalErrorCode::LocalTerminalUnsupported,
        "local terminal is not supported on this platform",
    ))
}

#[cfg(unix)]
fn local_command(descriptor: &LocalTerminalDescriptor) -> portable_pty::CommandBuilder {
    let mut command = portable_pty::CommandBuilder::new(&descriptor.shell);
    command.arg("-l");
    command.cwd(&descriptor.working_directory);
    command.env_clear();
    let home = descriptor
        .account
        .home
        .as_deref()
        .unwrap_or(&descriptor.working_directory);
    command.env("HOME", home);
    command.env("USER", &descriptor.execution_identity);
    command.env("LOGNAME", &descriptor.execution_identity);
    command.env("SHELL", &descriptor.shell);
    command.env(
        "PATH",
        "/opt/homebrew/bin:/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin",
    );
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    for (key, value) in std::env::vars_os() {
        if let (Some(key), Some(value)) = (key.to_str(), value.to_str())
            && valid_inherited_environment(key, value)
        {
            command.env(key, value);
        }
    }
    command
}

#[cfg(unix)]
fn valid_inherited_environment(key: &str, value: &str) -> bool {
    let locale = matches!(
        key,
        "LANG"
            | "LC_ALL"
            | "LC_CTYPE"
            | "LC_NUMERIC"
            | "LC_TIME"
            | "LC_COLLATE"
            | "LC_MONETARY"
            | "LC_MESSAGES"
            | "LC_PAPER"
            | "LC_NAME"
            | "LC_ADDRESS"
            | "LC_TELEPHONE"
            | "LC_MEASUREMENT"
            | "LC_IDENTIFICATION"
    );
    let timezone = key == "TZ";
    if (!locale && !timezone) || value.is_empty() {
        return false;
    }
    let max_len = if timezone { 256 } else { 128 };
    value.len() <= max_len
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'-' | b'+' | b'.' | b'/' | b':' | b'@')
        })
        && (!timezone || (!value.contains("..") && !value.trim_start_matches(':').starts_with('/')))
}

#[cfg(unix)]
fn emit_progress(progress: Option<&mpsc::UnboundedSender<SessionPhase>>, phase: SessionPhase) {
    if let Some(progress) = progress {
        let _ = progress.send(phase);
    }
}

#[cfg(unix)]
fn local_pty_error(context: &str, error: impl std::fmt::Display) -> TerminalError {
    tracing::warn!(%error, context, "local PTY operation failed");
    TerminalError::new(TerminalErrorCode::LocalPtyStartFailed, context)
}

#[cfg(unix)]
mod unix_shell {
    use std::{
        io::{Read, Write},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self as std_mpsc, SyncSender, TrySendError},
        },
        thread,
        time::{Duration, Instant},
    };

    use async_trait::async_trait;
    use portable_pty::{Child, MasterPty, PtySize};
    use tokio::sync::{mpsc, oneshot};

    use super::super::{
        domain::{TerminalError, TerminalErrorCode, TerminalResult},
        shell::{InteractiveShell, ShellEvent},
    };

    const EVENT_QUEUE_CAPACITY: usize = 128;
    const CONTROL_QUEUE_CAPACITY: usize = 128;
    const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(25);
    const SIGNAL_GRACE: Duration = Duration::from_millis(750);

    enum InputCommand {
        Input(Vec<u8>, oneshot::Sender<TerminalResult<()>>),
    }

    enum ControlCommand {
        Resize(u32, u32, oneshot::Sender<TerminalResult<()>>),
        Close(oneshot::Sender<()>),
    }

    #[derive(Clone, Copy)]
    struct ProcessSession {
        id: Option<libc::pid_t>,
    }

    impl ProcessSession {
        fn for_child(child: &dyn Child) -> Self {
            let id = child
                .process_id()
                .and_then(|pid| libc::pid_t::try_from(pid).ok())
                .filter(|pid| *pid > 1 && *pid != unsafe { libc::getpid() });
            Self { id }
        }

        fn signal_primary(self, signal: libc::c_int) {
            let Some(id) = self.id else {
                return;
            };
            let own_group = unsafe { libc::getpgrp() };
            if id != own_group {
                let _ = unsafe { libc::kill(-id, signal) };
            }
            let _ = unsafe { libc::kill(id, signal) };
        }

        fn signal_all(self, signal: libc::c_int) {
            self.signal_primary(signal);
            let Some(id) = self.id else {
                return;
            };
            if let Some(processes) = session_processes(id) {
                let own_pid = unsafe { libc::getpid() };
                for pid in processes {
                    if pid > 1 && pid != own_pid {
                        let _ = unsafe { libc::kill(pid, signal) };
                    }
                }
            }
        }

        fn has_members(self) -> bool {
            let Some(id) = self.id else {
                return false;
            };
            match session_processes(id) {
                Some(processes) => !processes.is_empty(),
                None => unsafe { libc::kill(-id, 0) == 0 || libc::kill(id, 0) == 0 },
            }
        }
    }

    pub(super) struct SpawnedChild {
        child: Option<Box<dyn Child + Send + Sync>>,
        session: ProcessSession,
    }

    impl SpawnedChild {
        pub(super) fn new(child: Box<dyn Child + Send + Sync>) -> Self {
            let session = ProcessSession::for_child(child.as_ref());
            Self {
                child: Some(child),
                session,
            }
        }

        fn into_parts(mut self) -> (Box<dyn Child + Send + Sync>, ProcessSession) {
            let child = self.child.take().expect("spawned child is present");
            (child, self.session)
        }
    }

    impl Drop for SpawnedChild {
        fn drop(&mut self) {
            if let Some(child) = self.child.as_mut() {
                terminate_process_session(self.session, child.as_mut());
            }
        }
    }

    pub(super) struct LocalShell {
        events: mpsc::Receiver<ShellEvent>,
        inputs: SyncSender<InputCommand>,
        controls: SyncSender<ControlCommand>,
        session: ProcessSession,
        writer_shutdown: Arc<AtomicBool>,
        closed: bool,
    }

    impl LocalShell {
        pub(super) fn spawn(
            master: Box<dyn MasterPty + Send>,
            mut reader: Box<dyn Read + Send>,
            writer: Box<dyn Write + Send>,
            child: SpawnedChild,
        ) -> TerminalResult<Box<dyn InteractiveShell>> {
            let (event_tx, event_rx) = mpsc::channel(EVENT_QUEUE_CAPACITY);
            let (input_tx, input_rx) = std_mpsc::sync_channel(CONTROL_QUEUE_CAPACITY);
            let (control_tx, control_rx) = std_mpsc::sync_channel(CONTROL_QUEUE_CAPACITY);
            let session = child.session;
            let writer_shutdown = Arc::new(AtomicBool::new(false));
            let reader_events = event_tx.clone();
            thread::Builder::new()
                .name("fn-knock-local-pty-reader".to_string())
                .spawn(move || {
                    let mut buffer = vec![0_u8; 16 * 1024];
                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(read) => {
                                if reader_events
                                    .blocking_send(ShellEvent::Data(buffer[..read].to_vec()))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                            Err(_) => break,
                        }
                    }
                })
                .map_err(|error| {
                    super::local_pty_error("failed to start local PTY reader", error)
                })?;
            let input_shutdown = writer_shutdown.clone();
            thread::Builder::new()
                .name("fn-knock-local-pty-writer".to_string())
                .spawn(move || input_loop(writer, input_rx, input_shutdown))
                .map_err(|error| {
                    super::local_pty_error("failed to start local PTY writer", error)
                })?;
            let control_shutdown = writer_shutdown.clone();
            thread::Builder::new()
                .name("fn-knock-local-pty-control".to_string())
                .spawn(move || {
                    control_loop(master, child, control_rx, event_tx, control_shutdown);
                })
                .map_err(|error| {
                    super::local_pty_error("failed to start local PTY controller", error)
                })?;
            Ok(Box::new(Self {
                events: event_rx,
                inputs: input_tx,
                controls: control_tx,
                session,
                writer_shutdown,
                closed: false,
            }))
        }

        async fn control_request<T>(
            &self,
            command: impl FnOnce(oneshot::Sender<T>) -> ControlCommand,
        ) -> Result<T, TerminalError> {
            let (sender, receiver) = oneshot::channel();
            self.controls
                .try_send(command(sender))
                .map_err(control_send_error)?;
            receiver.await.map_err(|_| disconnected())
        }

        async fn input_request(&self, data: Vec<u8>) -> TerminalResult<()> {
            let (sender, receiver) = oneshot::channel();
            self.inputs
                .try_send(InputCommand::Input(data, sender))
                .map_err(control_send_error)?;
            receiver.await.map_err(|_| disconnected())?
        }
    }

    #[async_trait]
    impl InteractiveShell for LocalShell {
        async fn next_event(&mut self) -> ShellEvent {
            self.events.recv().await.unwrap_or(ShellEvent::Closed)
        }

        async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()> {
            self.input_request(data).await
        }

        async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()> {
            self.control_request(|response| ControlCommand::Resize(cols, rows, response))
                .await?
        }

        async fn close(&mut self) {
            if self.closed {
                return;
            }
            self.closed = true;
            self.writer_shutdown.store(true, Ordering::Release);
            // This signal is deliberately issued by the caller rather than the
            // writer thread. A full PTY input buffer must never make shutdown
            // unreachable.
            self.session.signal_primary(libc::SIGHUP);
            if self.control_request(ControlCommand::Close).await.is_err() {
                let session = self.session;
                let _ = tokio::task::spawn_blocking(move || {
                    terminate_remaining_session(session);
                })
                .await;
            }
        }

        async fn disconnect(&mut self) {
            self.close().await;
        }
    }

    impl Drop for LocalShell {
        fn drop(&mut self) {
            if !self.closed {
                self.writer_shutdown.store(true, Ordering::Release);
                self.session.signal_primary(libc::SIGHUP);
                let (sender, _) = oneshot::channel();
                if self
                    .controls
                    .try_send(ControlCommand::Close(sender))
                    .is_err()
                {
                    let session = self.session;
                    let _ = thread::Builder::new()
                        .name("fn-knock-local-pty-cleanup".to_string())
                        .spawn(move || terminate_remaining_session(session));
                }
            }
        }
    }

    fn input_loop(
        mut writer: Box<dyn Write + Send>,
        inputs: std_mpsc::Receiver<InputCommand>,
        shutdown: Arc<AtomicBool>,
    ) {
        while !shutdown.load(Ordering::Acquire) {
            match inputs.recv_timeout(CHILD_POLL_INTERVAL) {
                Ok(InputCommand::Input(data, response)) => {
                    let result = writer
                        .write_all(&data)
                        .and_then(|()| writer.flush())
                        .map_err(|_| disconnected());
                    let _ = response.send(result);
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn control_loop(
        master: Box<dyn MasterPty + Send>,
        child: SpawnedChild,
        controls: std_mpsc::Receiver<ControlCommand>,
        events: mpsc::Sender<ShellEvent>,
        writer_shutdown: Arc<AtomicBool>,
    ) {
        let (mut child, session) = child.into_parts();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    writer_shutdown.store(true, Ordering::Release);
                    terminate_remaining_session(session);
                    let event = match status.signal() {
                        Some(signal) => ShellEvent::Signaled(signal.to_string()),
                        None => ShellEvent::Exited(status.exit_code()),
                    };
                    let _ = events.blocking_send(event);
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    writer_shutdown.store(true, Ordering::Release);
                    terminate_process_session(session, child.as_mut());
                    let _ = events.blocking_send(ShellEvent::Signaled(format!(
                        "local terminal process status failed: {error}"
                    )));
                    return;
                }
            }
            match controls.recv_timeout(CHILD_POLL_INTERVAL) {
                Ok(ControlCommand::Resize(cols, rows, response)) => {
                    let result = master
                        .resize(PtySize {
                            rows: rows.min(u16::MAX.into()) as u16,
                            cols: cols.min(u16::MAX.into()) as u16,
                            pixel_width: 0,
                            pixel_height: 0,
                        })
                        .map_err(|_| disconnected());
                    let _ = response.send(result);
                }
                Ok(ControlCommand::Close(response)) => {
                    writer_shutdown.store(true, Ordering::Release);
                    terminate_process_session(session, child.as_mut());
                    let _ = response.send(());
                    return;
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    writer_shutdown.store(true, Ordering::Release);
                    terminate_process_session(session, child.as_mut());
                    return;
                }
            }
        }
    }

    fn terminate_process_session(session: ProcessSession, child: &mut dyn Child) {
        session.signal_all(libc::SIGHUP);
        if wait_for_process_session(child, session, SIGNAL_GRACE) {
            return;
        }
        session.signal_all(libc::SIGTERM);
        if wait_for_process_session(child, session, SIGNAL_GRACE) {
            return;
        }
        session.signal_all(libc::SIGKILL);
        let _ = child.kill();
        if !wait_for_process_session(child, session, SIGNAL_GRACE) {
            tracing::warn!(session_id = ?session.id, "local PTY process session survived SIGKILL grace period");
        }
    }

    fn terminate_remaining_session(session: ProcessSession) {
        for signal in [libc::SIGHUP, libc::SIGTERM, libc::SIGKILL] {
            if !session.has_members() {
                return;
            }
            session.signal_all(signal);
            if wait_for_session_exit(session, SIGNAL_GRACE) {
                return;
            }
        }
        tracing::warn!(session_id = ?session.id, "local PTY descendants survived SIGKILL grace period");
    }

    fn wait_for_process_session(
        child: &mut dyn Child,
        session: ProcessSession,
        timeout: Duration,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        let mut child_exited = false;
        loop {
            if !child_exited {
                child_exited = matches!(child.try_wait(), Ok(Some(_)));
            }
            if child_exited && !session.has_members() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(CHILD_POLL_INTERVAL);
        }
    }

    fn wait_for_session_exit(session: ProcessSession, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while session.has_members() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(CHILD_POLL_INTERVAL);
        }
        true
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn session_processes(session_id: libc::pid_t) -> Option<Vec<libc::pid_t>> {
        let entries = std::fs::read_dir("/proc").ok()?;
        Some(
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().to_string_lossy().parse().ok())
                .filter(|pid| unsafe { libc::getsid(*pid) } == session_id)
                .collect(),
        )
    }

    #[cfg(target_os = "macos")]
    fn session_processes(session_id: libc::pid_t) -> Option<Vec<libc::pid_t>> {
        let estimated = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if estimated <= 0 {
            return None;
        }
        let capacity = usize::try_from(estimated).ok()?.saturating_add(64);
        let mut pids = vec![0 as libc::pid_t; capacity];
        let buffer_size =
            i32::try_from(pids.len().checked_mul(std::mem::size_of::<libc::pid_t>())?).ok()?;
        let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), buffer_size) };
        if count < 0 {
            return None;
        }
        pids.truncate(usize::try_from(count).ok()?.min(pids.len()));
        pids.retain(|pid| *pid > 1 && unsafe { libc::getsid(*pid) } == session_id);
        Some(pids)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
    fn session_processes(_session_id: libc::pid_t) -> Option<Vec<libc::pid_t>> {
        None
    }

    fn control_send_error<T>(error: TrySendError<T>) -> TerminalError {
        match error {
            TrySendError::Full(_) => TerminalError::new(
                TerminalErrorCode::SessionLost,
                "local terminal command queue is full",
            ),
            TrySendError::Disconnected(_) => disconnected(),
        }
    }

    fn disconnected() -> TerminalError {
        TerminalError::new(
            TerminalErrorCode::SessionLost,
            "local terminal session is no longer connected",
        )
    }
}

#[cfg(unix)]
use unix_shell::{LocalShell, SpawnedChild};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    async fn read_marked_pid(shell: &mut BoxedShell, marker: &str) -> Result<libc::pid_t, String> {
        use crate::system::terminal::shell::ShellEvent;

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut output = Vec::new();
        loop {
            let event = tokio::time::timeout_at(deadline, shell.next_event())
                .await
                .map_err(|_| String::from_utf8_lossy(&output).into_owned())?;
            match event {
                ShellEvent::Data(data) => {
                    output.extend(data);
                    let text = String::from_utf8_lossy(&output);
                    if let Some(pid) = text
                        .match_indices(marker)
                        .filter_map(|(offset, matched)| {
                            text[offset + matched.len()..]
                                .trim_start()
                                .chars()
                                .take_while(char::is_ascii_digit)
                                .collect::<String>()
                                .parse::<libc::pid_t>()
                                .ok()
                        })
                        .next()
                    {
                        return Ok(pid);
                    }
                }
                event => {
                    return Err(format!(
                        "unexpected event {event:?}; output={:?}",
                        String::from_utf8_lossy(&output)
                    ));
                }
            }
        }
    }

    #[test]
    fn local_terminal_platform_matrix_is_explicit() {
        for target in ["fpk", "linux", "macos", "openwrt"] {
            assert_eq!(supported_target(target), cfg!(unix), "{target}");
        }
        for target in ["fpk-lite", "synology", "windows", "docker", "dev", ""] {
            assert!(!supported_target(target), "{target}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn safe_environment_excludes_process_secrets() {
        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let command = local_command(&descriptor);
        assert_eq!(
            command.get_env("TERM").and_then(|v| v.to_str()),
            Some("xterm-256color")
        );
        assert!(command.get_env("HMAC_SECRET").is_none());
        assert!(command.get_env("FN_KNOCK_INTERNAL_RPC_TOKEN").is_none());
        let keys = command
            .iter_full_env_as_str()
            .map(|(key, _)| key)
            .collect::<Vec<_>>();
        assert!(
            keys.iter().all(|key| matches!(
                *key,
                "HOME"
                    | "USER"
                    | "LOGNAME"
                    | "SHELL"
                    | "PATH"
                    | "TERM"
                    | "COLORTERM"
                    | "LANG"
                    | "TZ"
            ) || key.starts_with("LC_")),
            "{keys:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn inherited_locale_and_timezone_values_are_strictly_validated() {
        assert!(valid_inherited_environment("LANG", "zh_CN.UTF-8"));
        assert!(valid_inherited_environment("LC_ALL", "C.UTF-8"));
        assert!(valid_inherited_environment("TZ", "Asia/Shanghai"));
        assert!(!valid_inherited_environment("API_TOKEN", "secret"));
        assert!(!valid_inherited_environment("LC_SECRET", "secret"));
        assert!(!valid_inherited_environment("LANG", "C\nAPI_TOKEN=secret"));
        assert!(!valid_inherited_environment("TZ", ":/etc/localtime"));
        assert!(!valid_inherited_environment("TZ", "../secret"));
    }

    #[cfg(unix)]
    #[test]
    fn shell_candidates_follow_the_documented_priority_and_deduplicate() {
        let account_zsh = PathBuf::from("/custom/account/zsh");
        let account = UnixAccount {
            identity: "tester".to_string(),
            home: None,
            shell: Some(account_zsh.clone()),
        };
        let candidates = shell_candidates(&account);
        assert_eq!(candidates.first(), Some(&account_zsh));
        assert_eq!(candidates.get(1), Some(&PathBuf::from("/bin/zsh")));
        assert_eq!(candidates.last(), Some(&PathBuf::from("/usr/bin/sh")));
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| **candidate == account_zsh)
                .count(),
            1
        );

        let account_shell = PathBuf::from("/custom/account/fish");
        let candidates = shell_candidates(&UnixAccount {
            identity: "tester".to_string(),
            home: None,
            shell: Some(account_shell.clone()),
        });
        assert_eq!(candidates.get(4), Some(&account_shell));
        assert!(
            candidates
                .windows(2)
                .any(|pair| pair == [PathBuf::from("/bin/bash"), PathBuf::from("/usr/bin/bash")])
        );
        assert!(
            candidates
                .windows(2)
                .any(|pair| pair == [PathBuf::from("/bin/ash"), PathBuf::from("/usr/bin/ash")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_resolution_rejects_invalid_paths_and_falls_back_to_standard_shell() {
        let account = UnixAccount {
            identity: "tester".to_string(),
            home: None,
            shell: Some(PathBuf::from("relative-shell")),
        };
        assert!(!executable_file(Path::new("relative-shell")));
        let shell = resolve_shell(&account).expect("standard Unix shell");
        assert!(executable_file(&shell));
    }

    #[cfg(unix)]
    #[test]
    fn working_directory_prefers_home_then_data_then_root() {
        let home = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let account = UnixAccount {
            identity: "tester".to_string(),
            home: Some(home.path().to_path_buf()),
            shell: None,
        };
        assert_eq!(
            resolve_working_directory_from(&account, data.path()),
            home.path()
        );
        let no_home = UnixAccount {
            home: Some(home.path().join("missing")),
            ..account
        };
        assert_eq!(
            resolve_working_directory_from(&no_home, data.path()),
            data.path()
        );
        assert_eq!(
            resolve_working_directory_from(&no_home, &data.path().join("missing")),
            PathBuf::from("/")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_pty_supports_io_resize_and_exit_status() {
        use crate::system::terminal::shell::ShellEvent;

        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let mut shell = open_shell(descriptor, 80, 24, None).await.unwrap();
        shell.resize(100, 40).await.unwrap();
        shell
            .input(
                "printf '\\n__FN_KNOCK_OUTPUT__终端:%s\\n' \"$(stty size)\"; exit 7\n"
                    .as_bytes()
                    .to_vec(),
            )
            .await
            .unwrap();
        let (output, exit_code) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let mut output = Vec::new();
            loop {
                match shell.next_event().await {
                    ShellEvent::Data(data) => output.extend(data),
                    ShellEvent::Exited(code) => break (output, code),
                    ShellEvent::Signaled(signal) => panic!("shell signaled: {signal}"),
                    ShellEvent::Closed => panic!("shell closed before exit status"),
                    ShellEvent::Other => {}
                }
            }
        })
        .await
        .expect("local shell exit");
        let output = String::from_utf8_lossy(&output);
        assert!(
            output.contains("__FN_KNOCK_OUTPUT__终端:40 100"),
            "{output}"
        );
        assert_eq!(exit_code, 7);
        shell.disconnect().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn closing_local_pty_terminates_background_children() {
        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let mut shell = open_shell(descriptor, 80, 24, None).await.unwrap();
        shell
            .input(
                b"sh -c 'trap \"\" HUP TERM; printf \"__FN_CHILD_PID__%s\\n\" \"$$\"; while :; do sleep 60; done' &\n"
                    .to_vec(),
            )
            .await
            .unwrap();
        let child_pid = read_marked_pid(&mut shell, "__FN_CHILD_PID__")
            .await
            .unwrap_or_else(|output| panic!("background child PID: {output:?}"));
        assert_eq!(unsafe { libc::kill(child_pid, 0) }, 0);

        shell.close().await;
        let terminated = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if unsafe { libc::kill(child_pid, 0) } != 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await;
        assert!(
            terminated.is_ok(),
            "background child {child_pid} survived PTY close"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_leader_exit_still_cleans_resistant_session_members() {
        struct TestProcessCleanup(libc::pid_t);

        impl Drop for TestProcessCleanup {
            fn drop(&mut self) {
                let _ = unsafe { libc::kill(self.0, libc::SIGKILL) };
            }
        }

        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let mut shell = open_shell(descriptor, 80, 24, None).await.unwrap();
        shell
            .input(
                b"sh -c 'trap \"\" HUP TERM; printf \"__FN_ORPHAN_PID__%s\\n\" \"$$\"; kill -KILL \"$PPID\"; while :; do sleep 60; done' &\n"
                    .to_vec(),
            )
            .await
            .unwrap();
        let child_pid = read_marked_pid(&mut shell, "__FN_ORPHAN_PID__")
            .await
            .unwrap_or_else(|output| panic!("resistant child PID: {output:?}"));
        let _cleanup = TestProcessCleanup(child_pid);

        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while unsafe { libc::kill(child_pid, 0) } == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("resistant child must not survive its shell leader");
        shell.disconnect().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn blocked_local_pty_input_cannot_block_process_termination() {
        use std::{
            io::Write,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };

        use portable_pty::{PtySize, native_pty_system};

        struct BlockingWriter {
            entered: Arc<AtomicBool>,
            release: Arc<AtomicBool>,
        }

        impl Write for BlockingWriter {
            fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
                self.entered.store(true, Ordering::Release);
                while !self.release.load(Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(buffer.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let child = pair
            .slave
            .spawn_command(local_command(&descriptor))
            .unwrap();
        let child_pid = libc::pid_t::try_from(child.process_id().unwrap()).unwrap();
        let child = SpawnedChild::new(child);
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().unwrap();
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let writer = BlockingWriter {
            entered: entered.clone(),
            release: release.clone(),
        };
        let mut shell = LocalShell::spawn(pair.master, reader, Box::new(writer), child).unwrap();

        let pending_input = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            shell.input(vec![b'x'; 64 * 1024]),
        )
        .await;
        assert!(pending_input.is_err(), "injected PTY writer did not block");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !entered.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("writer entered its blocking section");

        let close_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), shell.close()).await;
        release.store(true, Ordering::Release);
        close_result.expect("PTY close must bypass the blocked writer");
        let terminated = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while unsafe { libc::kill(child_pid, 0) } == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await;
        assert!(
            terminated.is_ok(),
            "shell {child_pid} survived close while input was blocked"
        );
    }

    #[cfg(unix)]
    #[test]
    fn uncommitted_local_pty_startup_is_rolled_back() {
        use portable_pty::{PtySize, native_pty_system};

        let descriptor = LocalTerminalDescriptor {
            execution_identity: "tester".to_string(),
            privileged: false,
            shell: PathBuf::from("/bin/sh"),
            working_directory: PathBuf::from("/"),
            account: UnixAccount {
                identity: "tester".to_string(),
                home: Some(PathBuf::from("/tmp")),
                shell: Some(PathBuf::from("/bin/sh")),
            },
        };
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let child = pair
            .slave
            .spawn_command(local_command(&descriptor))
            .unwrap();
        let child_pid = libc::pid_t::try_from(child.process_id().unwrap()).unwrap();
        let guard = SpawnedChild::new(child);
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().unwrap();

        // Model a failure after the reader has been cloned but before the
        // controller thread takes ownership. The cloned master must not keep
        // an otherwise-unmanaged shell alive.
        drop(guard);
        assert_ne!(unsafe { libc::kill(child_pid, 0) }, 0);
        drop(reader);
        drop(pair.master);
    }
}
