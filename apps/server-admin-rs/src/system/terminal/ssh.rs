use std::{
    collections::VecDeque,
    net::{IpAddr, SocketAddr},
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use russh::{
    Channel, ChannelMsg, Disconnect, client,
    keys::{HashAlg, PrivateKeyWithHashAlg, decode_secret_key, ssh_key},
};
use tokio::{
    net::lookup_host,
    sync::{Semaphore, mpsc},
    time,
};

use super::domain::{
    HostKeyProbeResult, SessionPhase, TargetRecord, TerminalError, TerminalErrorCode,
    TerminalResult, TrustedHostKey,
};
use super::shell::{BoxedShell, InteractiveShell, ShellEvent};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const CHANNEL_TIMEOUT: Duration = Duration::from_secs(10);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const PRIVATE_KEY_DECODE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PRIVATE_KEY_BYTES: usize = 256 * 1024;
const MAX_PASSPHRASE_BYTES: usize = 4 * 1024;
const MAX_BCRYPT_KDF_ROUNDS: u32 = 64;
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const MAX_CONCURRENT_SSH_OPERATIONS: usize = 16;
const MAX_CONCURRENT_PRIVATE_KEY_DECODES: usize = 2;
static SSH_OPERATION_LIMIT: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_SSH_OPERATIONS)));
static PRIVATE_KEY_DECODE_LIMIT: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PRIVATE_KEY_DECODES)));

pub enum SshCredential {
    Password(String),
    PrivateKey {
        key: String,
        passphrase: Option<String>,
    },
}

#[derive(Clone)]
pub(super) struct HostKeyHandler {
    expected: Option<TrustedHostKey>,
    observed: Arc<Mutex<Option<TrustedHostKey>>>,
}

impl client::Handler for HostKeyHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let observed = TrustedHostKey {
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint: server_public_key.fingerprint(HashAlg::Sha256).to_string(),
        };
        *self
            .observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(observed.clone());
        Ok(self
            .expected
            .as_ref()
            .is_none_or(|value| value == &observed))
    }
}

struct ConnectedShell {
    pub session: client::Handle<HostKeyHandler>,
    pub channel: Channel<client::Msg>,
    pub pending_events: VecDeque<ShellEvent>,
}

#[async_trait]
impl InteractiveShell for ConnectedShell {
    async fn next_event(&mut self) -> ShellEvent {
        if let Some(event) = self.pending_events.pop_front() {
            return event;
        }
        match self.channel.wait().await {
            Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                ShellEvent::Data(data.to_vec())
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => ShellEvent::Exited(exit_status),
            Some(ChannelMsg::ExitSignal { error_message, .. }) => {
                ShellEvent::Signaled(error_message)
            }
            Some(ChannelMsg::Close) | None => ShellEvent::Closed,
            Some(_) => ShellEvent::Other,
        }
    }

    async fn input(&mut self, data: Vec<u8>) -> TerminalResult<()> {
        self.channel.data_bytes(data).await.map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::SessionLost,
                "terminal session is no longer connected",
            )
        })
    }

    async fn resize(&mut self, cols: u32, rows: u32) -> TerminalResult<()> {
        self.channel
            .window_change(cols, rows, 0, 0)
            .await
            .map_err(|_| {
                TerminalError::new(
                    TerminalErrorCode::SessionLost,
                    "terminal session is no longer connected",
                )
            })
    }

    async fn close(&mut self) {
        let deadline = time::Instant::now() + CLEANUP_TIMEOUT;
        let _ = time::timeout_at(deadline, self.channel.eof()).await;
        let _ = time::timeout_at(deadline, self.channel.close()).await;
    }

    async fn disconnect(&mut self) {
        let _ = time::timeout(
            CLEANUP_TIMEOUT,
            self.session
                .disconnect(Disconnect::ByApplication, "", "English"),
        )
        .await;
    }
}

#[async_trait]
pub(super) trait SshConnector: Send + Sync {
    async fn probe_host_key(&self, host: &str, port: u16) -> TerminalResult<HostKeyProbeResult>;
    async fn test_connection(
        &self,
        target: &TargetRecord,
        credential: SshCredential,
    ) -> TerminalResult<u64>;
    async fn open_shell(
        &self,
        target: &TargetRecord,
        credential: SshCredential,
        cols: u32,
        rows: u32,
        progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
    ) -> TerminalResult<BoxedShell>;
}

pub(super) struct RusshConnector;

#[async_trait]
impl SshConnector for RusshConnector {
    async fn probe_host_key(&self, host: &str, port: u16) -> TerminalResult<HostKeyProbeResult> {
        let _permit = acquire_ssh_operation().await?;
        probe_host_key(host, port).await
    }

    async fn test_connection(
        &self,
        target: &TargetRecord,
        credential: SshCredential,
    ) -> TerminalResult<u64> {
        let _permit = acquire_ssh_operation().await?;
        test_connection(target, credential).await
    }

    async fn open_shell(
        &self,
        target: &TargetRecord,
        credential: SshCredential,
        cols: u32,
        rows: u32,
        progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
    ) -> TerminalResult<BoxedShell> {
        let _permit = acquire_ssh_operation().await?;
        open_shell(target, credential, cols, rows, progress).await
    }
}

async fn acquire_ssh_operation() -> TerminalResult<tokio::sync::OwnedSemaphorePermit> {
    time::timeout(
        CONNECT_TIMEOUT,
        Arc::clone(&SSH_OPERATION_LIMIT).acquire_owned(),
    )
    .await
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::ConnectTimeout,
            "SSH operation concurrency wait timed out",
        )
    })?
    .map_err(|_| TerminalError::internal("SSH operation limiter is closed"))
}

pub async fn probe_host_key(host: &str, port: u16) -> TerminalResult<HostKeyProbeResult> {
    let deadline = time::Instant::now() + CONNECT_TIMEOUT;
    let endpoints = resolve_endpoints(host, port, deadline).await?;
    let endpoint_count = endpoints.len();
    let mut last_error = None;
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        let observed = Arc::new(Mutex::new(None));
        let handler = HostKeyHandler {
            expected: None,
            observed: Arc::clone(&observed),
        };
        match connect(
            endpoint,
            handler,
            address_attempt_deadline(deadline, endpoint_count.saturating_sub(index)),
        )
        .await
        {
            Ok(session) => {
                let _ = time::timeout(
                    CLEANUP_TIMEOUT,
                    session.disconnect(Disconnect::ByApplication, "", "English"),
                )
                .await;
                let key = observed
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone()
                    .ok_or_else(|| {
                        TerminalError::new(
                            TerminalErrorCode::UpstreamUnavailable,
                            "SSH host key was not provided",
                        )
                    })?;
                return Ok(HostKeyProbeResult {
                    host: host.trim().to_string(),
                    port,
                    algorithm: key.algorithm,
                    fingerprint: key.fingerprint,
                });
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        TerminalError::new(
            TerminalErrorCode::UpstreamUnavailable,
            "SSH connection failed",
        )
    }))
}

pub async fn test_connection(
    target: &TargetRecord,
    credential: SshCredential,
) -> TerminalResult<u64> {
    let started = Instant::now();
    let mut shell = open_shell(target, credential, 80, 24, None).await?;
    shell.close().await;
    shell.disconnect().await;
    Ok(started.elapsed().as_millis().try_into().unwrap_or(u64::MAX))
}

pub(super) async fn open_shell(
    target: &TargetRecord,
    credential: SshCredential,
    cols: u32,
    rows: u32,
    progress: Option<&mpsc::UnboundedSender<SessionPhase>>,
) -> TerminalResult<BoxedShell> {
    emit_progress(progress, SessionPhase::Resolving);
    let expected = validate_trusted_host_key(target.trusted_host_key.as_ref())?;
    let deadline = time::Instant::now() + CONNECT_TIMEOUT;
    let endpoints = resolve_endpoints(&target.host, target.port, deadline).await?;
    let endpoint_count = endpoints.len();
    emit_progress(progress, SessionPhase::Connecting);
    emit_progress(progress, SessionPhase::VerifyingHostKey);
    let mut last_error = None;
    let mut connected = None;
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        let observed = Arc::new(Mutex::new(None));
        let handler = HostKeyHandler {
            expected: Some(expected.clone()),
            observed: Arc::clone(&observed),
        };
        match connect(
            endpoint,
            handler,
            address_attempt_deadline(deadline, endpoint_count.saturating_sub(index)),
        )
        .await
        {
            Ok(session) => {
                connected = Some(session);
                break;
            }
            Err(error) => {
                if observed
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                    .is_some_and(|value| value != &expected)
                {
                    return Err(TerminalError::new(
                        TerminalErrorCode::HostKeyMismatch,
                        "SSH host key does not match the trusted fingerprint",
                    ));
                }
                last_error = Some(error);
            }
        }
    }
    let mut session = connected.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            TerminalError::new(
                TerminalErrorCode::UpstreamUnavailable,
                "SSH connection failed",
            )
        })
    })?;

    emit_progress(progress, SessionPhase::Authenticating);
    let authenticated = time::timeout(AUTH_TIMEOUT, async {
        match credential {
            SshCredential::Password(password) => session
                .authenticate_password(&target.username, password)
                .await
                .map(|result| result.success())
                .map_err(|_| {
                    TerminalError::new(
                        TerminalErrorCode::UpstreamUnavailable,
                        "SSH authentication exchange failed",
                    )
                }),
            SshCredential::PrivateKey { key, passphrase } => {
                let key = decode_private_key(key, passphrase).await?;
                let hash = session
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|_| {
                        TerminalError::new(
                            TerminalErrorCode::UpstreamUnavailable,
                            "SSH key negotiation failed",
                        )
                    })?
                    .flatten();
                session
                    .authenticate_publickey(
                        &target.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
                    .map(|result| result.success())
                    .map_err(|_| {
                        TerminalError::new(
                            TerminalErrorCode::UpstreamUnavailable,
                            "SSH authentication exchange failed",
                        )
                    })
            }
        }
    })
    .await
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::ConnectTimeout,
            "SSH authentication timed out",
        )
    })??;
    if !authenticated {
        return Err(TerminalError::new(
            TerminalErrorCode::AuthenticationFailed,
            "SSH authentication failed",
        ));
    }

    emit_progress(progress, SessionPhase::OpeningChannel);
    let mut channel = time::timeout(CHANNEL_TIMEOUT, session.channel_open_session())
        .await
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::ConnectTimeout,
                "SSH channel open timed out",
            )
        })?
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::UpstreamUnavailable,
                "SSH session channel could not be opened",
            )
        })?;
    emit_progress(progress, SessionPhase::RequestingPty);
    let pty_deadline = time::Instant::now() + CHANNEL_TIMEOUT;
    time::timeout_at(
        pty_deadline,
        channel.request_pty(true, "xterm-256color", cols, rows, 0, 0, &[]),
    )
    .await
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::ConnectTimeout,
            "SSH PTY request timed out",
        )
    })?
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::PtyRejected,
            "SSH server rejected the terminal request",
        )
    })?;
    let mut pending_events = await_request_reply_until(
        &mut channel,
        TerminalErrorCode::PtyRejected,
        "PTY",
        pty_deadline,
    )
    .await?;
    let shell_deadline = time::Instant::now() + CHANNEL_TIMEOUT;
    time::timeout_at(shell_deadline, channel.request_shell(true))
        .await
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::ConnectTimeout,
                "SSH interactive shell request timed out",
            )
        })?
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::PtyRejected,
                "SSH server rejected the interactive shell",
            )
        })?;
    pending_events.extend(
        await_request_reply_until(
            &mut channel,
            TerminalErrorCode::PtyRejected,
            "interactive shell",
            shell_deadline,
        )
        .await?,
    );

    Ok(Box::new(ConnectedShell {
        session,
        channel,
        pending_events,
    }))
}

async fn await_request_reply_until(
    channel: &mut Channel<client::Msg>,
    code: TerminalErrorCode,
    request: &str,
    deadline: time::Instant,
) -> TerminalResult<VecDeque<ShellEvent>> {
    time::timeout_at(deadline, async {
        let mut pending_events = VecDeque::new();
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Success) => return Ok(pending_events),
                Some(ChannelMsg::Failure) | Some(ChannelMsg::Close) | None => {
                    return Err(TerminalError::new(
                        code,
                        format!("SSH server rejected the {request} request"),
                    ));
                }
                Some(ChannelMsg::Data { data }) => {
                    pending_events.push_back(ShellEvent::Data(data.to_vec()));
                }
                Some(ChannelMsg::ExtendedData { data, .. }) => {
                    pending_events.push_back(ShellEvent::Data(data.to_vec()));
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    pending_events.push_back(ShellEvent::Exited(exit_status));
                }
                Some(ChannelMsg::ExitSignal { error_message, .. }) => {
                    pending_events.push_back(ShellEvent::Signaled(error_message));
                }
                Some(_) => {}
            }
        }
    })
    .await
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::ConnectTimeout,
            format!("SSH {request} request timed out"),
        )
    })?
}

async fn decode_private_key(
    key: String,
    passphrase: Option<String>,
) -> TerminalResult<ssh_key::PrivateKey> {
    if key.is_empty()
        || key.len() > MAX_PRIVATE_KEY_BYTES
        || passphrase
            .as_ref()
            .is_some_and(|value| value.len() > MAX_PASSPHRASE_BYTES)
    {
        return Err(TerminalError::new(
            TerminalErrorCode::AuthenticationFailed,
            "SSH private key or passphrase is invalid",
        ));
    }
    validate_private_key_cost(&key)?;
    let permit = time::timeout(
        PRIVATE_KEY_DECODE_TIMEOUT,
        Arc::clone(&PRIVATE_KEY_DECODE_LIMIT).acquire_owned(),
    )
    .await
    .map_err(|_| {
        TerminalError::new(
            TerminalErrorCode::ConnectTimeout,
            "SSH private key decoder is busy",
        )
    })?
    .map_err(|_| TerminalError::internal("SSH private key decoder is unavailable"))?;
    let decode = tokio::task::spawn_blocking(move || {
        // Keep the permit inside the blocking job. If the async caller times
        // out, the non-cancellable KDF still occupies one bounded slot until
        // it actually finishes instead of allowing unbounded jobs to pile up.
        let _permit = permit;
        decode_secret_key(&key, passphrase.as_deref())
    });
    time::timeout(PRIVATE_KEY_DECODE_TIMEOUT, decode)
        .await
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::ConnectTimeout,
                "SSH private key decoding timed out",
            )
        })?
        .map_err(|_| TerminalError::internal("SSH private key decoder failed"))?
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::AuthenticationFailed,
                "SSH private key or passphrase is invalid",
            )
        })
}

fn validate_private_key_cost(key: &str) -> TerminalResult<()> {
    let Ok(parsed) = ssh_key::PrivateKey::from_openssh(key) else {
        // PKCS#8/PEM and malformed inputs are still bounded by the global
        // decoder semaphore and timeout. Only OpenSSH exposes a cheap KDF
        // preflight through ssh-key's container parser.
        return Ok(());
    };
    if matches!(
        parsed.kdf(),
        ssh_key::Kdf::Bcrypt { rounds, .. } if *rounds > MAX_BCRYPT_KDF_ROUNDS
    ) {
        return Err(TerminalError::new(
            TerminalErrorCode::AuthenticationFailed,
            "SSH private key KDF cost exceeds the supported limit",
        ));
    }
    Ok(())
}

async fn connect(
    endpoint: SocketAddr,
    handler: HostKeyHandler,
    deadline: time::Instant,
) -> TerminalResult<client::Handle<HostKeyHandler>> {
    let config = Arc::new(client::Config {
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    });
    time::timeout_at(deadline, client::connect(config, endpoint, handler))
        .await
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::ConnectTimeout,
                "SSH connection timed out",
            )
        })?
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::UpstreamUnavailable,
                "SSH connection failed",
            )
        })
}

fn address_attempt_deadline(deadline: time::Instant, attempts_remaining: usize) -> time::Instant {
    let now = time::Instant::now();
    let remaining = deadline.saturating_duration_since(now);
    if attempts_remaining <= 1 || remaining.is_zero() {
        return deadline;
    }
    let divisor = u32::try_from(attempts_remaining).unwrap_or(u32::MAX);
    let fair_share = remaining / divisor;
    now + fair_share.max(Duration::from_millis(250)).min(remaining)
}

async fn resolve_endpoints(
    host: &str,
    port: u16,
    deadline: time::Instant,
) -> TerminalResult<Vec<SocketAddr>> {
    let host = host.trim();
    if host.is_empty() || host.len() > 253 || port == 0 || host.chars().any(char::is_control) {
        return Err(TerminalError::invalid("SSH host or port is invalid"));
    }
    let addresses = time::timeout_at(deadline, lookup_host((host, port)))
        .await
        .map_err(|_| {
            TerminalError::new(
                TerminalErrorCode::ConnectTimeout,
                "SSH address resolution timed out",
            )
        })?
        .map_err(|_| TerminalError::invalid("SSH host cannot be resolved"))?
        .collect::<Vec<_>>();
    let addresses = addresses
        .into_iter()
        .filter(|address| usable_address(address.ip()))
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(TerminalError::invalid("SSH host has no usable address"));
    }
    Ok(addresses)
}

fn usable_address(address: IpAddr) -> bool {
    if let IpAddr::V6(value) = address
        && let Some(mapped) = value.to_ipv4_mapped()
    {
        return usable_address(IpAddr::V4(mapped));
    }
    !address.is_unspecified()
        && !address.is_multicast()
        && !matches!(address, IpAddr::V4(value) if value.is_broadcast())
}

fn emit_progress(progress: Option<&mpsc::UnboundedSender<SessionPhase>>, phase: SessionPhase) {
    if let Some(progress) = progress {
        let _ = progress.send(phase);
    }
}

fn validate_trusted_host_key(value: Option<&TrustedHostKey>) -> TerminalResult<TrustedHostKey> {
    let Some(value) = value else {
        return Err(TerminalError::new(
            TerminalErrorCode::HostKeyRequired,
            "SSH host key must be explicitly trusted before authentication",
        ));
    };
    let algorithm = value.algorithm.trim();
    let fingerprint = value.fingerprint.trim();
    if algorithm.is_empty()
        || algorithm.len() > 64
        || fingerprint.len() > 128
        || !fingerprint.starts_with("SHA256:")
    {
        return Err(TerminalError::new(
            TerminalErrorCode::HostKeyRequired,
            "trusted SSH host key is invalid",
        ));
    }
    Ok(TrustedHostKey {
        algorithm: algorithm.to_string(),
        fingerprint: fingerprint.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    use russh::{
        Channel, ChannelId, Pty,
        server::{self, Auth, Msg, Server as _, Session},
    };
    use tokio::{net::TcpListener, task::JoinHandle};

    const TEST_PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n\
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM\n\
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg\n\
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf\n\
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==\n\
-----END OPENSSH PRIVATE KEY-----";
    const TEST_ENCRYPTED_PRIVATE_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABBKH96ujW\n\
umB6/WnTNPjTeaAAAAEAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN\n\
796jTiQfZfG1KaT0PtFDJ/XFSqtiAAAAoFzvbvyFMhAiwBOXF0mhUUacPUCMZXivG2up2c\n\
hEnAw1b6BLRPyWbY5cC2n9ggD4ivJ1zSts6sBgjyiXQAReyrP35myYvT/OIB/NpwZM/xIJ\n\
N7MHSUzlkX4adBrga3f7GS4uv4ChOoxC4XsE5HsxtGsq1X8jzqLlZTmOcxkcEneYQexrUc\n\
bQP0o+gL5aKK8cQgiIlXeDbRjqhc4+h4EF6lY=\n\
-----END OPENSSH PRIVATE KEY-----";

    #[derive(Default)]
    struct Observed {
        password_auth_attempts: usize,
        public_key_auth_attempts: usize,
        pty: Vec<(String, u32, u32)>,
        shells: usize,
        inputs: Vec<Vec<u8>>,
        resizes: Vec<(u32, u32)>,
    }

    #[derive(Clone)]
    struct MockServer {
        observed: Arc<StdMutex<Observed>>,
        accept_pty: bool,
    }

    impl server::Server for MockServer {
        type Handler = Self;

        fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
            self.clone()
        }
    }

    impl server::Handler for MockServer {
        type Error = russh::Error;

        async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
            self.observed.lock().unwrap().password_auth_attempts += 1;
            Ok(if user == "operator" && password == "secret" {
                Auth::Accept
            } else {
                Auth::reject()
            })
        }

        async fn auth_publickey(
            &mut self,
            user: &str,
            _key: &ssh_key::PublicKey,
        ) -> Result<Auth, Self::Error> {
            self.observed.lock().unwrap().public_key_auth_attempts += 1;
            Ok(if user == "operator" {
                Auth::Accept
            } else {
                Auth::reject()
            })
        }

        async fn channel_open_session(
            &mut self,
            _channel: Channel<Msg>,
            reply: server::ChannelOpenHandle,
            _session: &mut Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn pty_request(
            &mut self,
            channel: ChannelId,
            term: &str,
            col_width: u32,
            row_height: u32,
            _pix_width: u32,
            _pix_height: u32,
            _modes: &[(Pty, u32)],
            session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.observed
                .lock()
                .unwrap()
                .pty
                .push((term.to_string(), col_width, row_height));
            if self.accept_pty {
                session.channel_success(channel)?;
            } else {
                session.channel_failure(channel)?;
            }
            Ok(())
        }

        async fn shell_request(
            &mut self,
            channel: ChannelId,
            session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.observed.lock().unwrap().shells += 1;
            // Dropbear and other compact servers can emit the initial prompt
            // before acknowledging the shell request. The client must retain
            // those bytes while it waits for the request reply.
            session.data(channel, b"ready\r\n".to_vec())?;
            session.channel_success(channel)?;
            Ok(())
        }

        async fn data(
            &mut self,
            _channel: ChannelId,
            data: &[u8],
            _session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.observed.lock().unwrap().inputs.push(data.to_vec());
            Ok(())
        }

        async fn window_change_request(
            &mut self,
            _channel: ChannelId,
            col_width: u32,
            row_height: u32,
            _pix_width: u32,
            _pix_height: u32,
            _session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.observed
                .lock()
                .unwrap()
                .resizes
                .push((col_width, row_height));
            Ok(())
        }
    }

    async fn start_server(
        accept_pty: bool,
    ) -> (
        SocketAddr,
        TargetRecord,
        Arc<StdMutex<Observed>>,
        JoinHandle<()>,
    ) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let host_key = decode_secret_key(TEST_PRIVATE_KEY, None).unwrap();
        let public_key = host_key.public_key().clone();
        let observed = Arc::new(StdMutex::new(Observed::default()));
        let mut server = MockServer {
            observed: Arc::clone(&observed),
            accept_pty,
        };
        let config = Arc::new(server::Config {
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            keys: vec![host_key],
            ..Default::default()
        });
        let task = tokio::spawn(async move {
            let _ = server.run_on_socket(config, &listener).await;
        });
        let target = TargetRecord {
            id: "target-a".to_string(),
            name: "test".to_string(),
            host: endpoint.ip().to_string(),
            port: endpoint.port(),
            username: "operator".to_string(),
            auth_method: super::super::domain::AuthMethod::Password,
            trusted_host_key: Some(TrustedHostKey {
                algorithm: public_key.algorithm().to_string(),
                fingerprint: public_key.fingerprint(HashAlg::Sha256).to_string(),
            }),
            revision: 1,
            last_verified_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        (endpoint, target, observed, task)
    }

    #[tokio::test]
    async fn russh_connector_probes_authenticates_and_opens_interactive_pty() {
        let (endpoint, target, observed, task) = start_server(true).await;
        let probe = RusshConnector
            .probe_host_key(&target.host, target.port)
            .await
            .unwrap();
        assert_eq!(probe.host, endpoint.ip().to_string());
        assert_eq!(probe.port, endpoint.port());
        assert_eq!(
            probe.fingerprint,
            target.trusted_host_key.as_ref().unwrap().fingerprint
        );

        let mut shell = RusshConnector
            .open_shell(
                &target,
                SshCredential::Password("secret".to_string()),
                120,
                32,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(shell.next_event().await, ShellEvent::Data(data) if data == b"ready\r\n"));
        shell.input(b"echo test\n".to_vec()).await.unwrap();
        shell.resize(100, 30).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        {
            let state = observed.lock().unwrap();
            assert_eq!(
                state.pty.as_slice(),
                [("xterm-256color".to_string(), 120, 32)]
            );
            assert_eq!(state.shells, 1);
            assert_eq!(state.inputs.as_slice(), [b"echo test\n".to_vec()]);
            assert_eq!(state.resizes.as_slice(), [(100, 30)]);
        }
        shell.close().await;
        shell.disconnect().await;
        task.abort();
    }

    #[tokio::test]
    async fn russh_connector_supports_private_keys_and_rejects_bad_auth_or_pty() {
        let (_, mut target, _, task) = start_server(true).await;
        let shell = RusshConnector
            .open_shell(
                &target,
                SshCredential::PrivateKey {
                    key: TEST_PRIVATE_KEY.to_string(),
                    passphrase: None,
                },
                80,
                24,
                None,
            )
            .await;
        assert!(shell.is_ok());
        let encrypted_shell = RusshConnector
            .open_shell(
                &target,
                SshCredential::PrivateKey {
                    key: TEST_ENCRYPTED_PRIVATE_KEY.to_string(),
                    passphrase: Some("hunter42".to_string()),
                },
                80,
                24,
                None,
            )
            .await;
        assert!(encrypted_shell.is_ok());
        let mut rng = getrandom::rand_core::UnwrapErr(getrandom::SysRng);
        let rsa_key = ssh_key::PrivateKey::random(&mut rng, ssh_key::Algorithm::Rsa { hash: None })
            .unwrap()
            .to_openssh(ssh_key::LineEnding::LF)
            .unwrap()
            .to_string();
        let rsa_shell = RusshConnector
            .open_shell(
                &target,
                SshCredential::PrivateKey {
                    key: rsa_key,
                    passphrase: None,
                },
                80,
                24,
                None,
            )
            .await;
        assert!(rsa_shell.is_ok());
        assert!(matches!(
            RusshConnector
                .open_shell(
                    &target,
                    SshCredential::PrivateKey {
                        key: TEST_ENCRYPTED_PRIVATE_KEY.to_string(),
                        passphrase: Some("wrong-passphrase".to_string()),
                    },
                    80,
                    24,
                    None,
                )
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::AuthenticationFailed,
                ..
            })
        ));
        assert!(matches!(
            RusshConnector
                .open_shell(
                    &target,
                    SshCredential::Password("wrong".to_string()),
                    80,
                    24,
                    None,
                )
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::AuthenticationFailed,
                ..
            })
        ));
        task.abort();

        let (_, rejected_target, _, rejected_task) = start_server(false).await;
        target = rejected_target;
        assert!(matches!(
            RusshConnector
                .open_shell(
                    &target,
                    SshCredential::Password("secret".to_string()),
                    80,
                    24,
                    None,
                )
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::PtyRejected,
                ..
            })
        ));
        rejected_task.abort();
    }

    #[tokio::test]
    async fn host_key_mismatch_blocks_before_authentication() {
        let (_, mut target, observed, task) = start_server(true).await;
        target.trusted_host_key = Some(TrustedHostKey {
            algorithm: "ssh-ed25519".to_string(),
            fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        });
        assert!(matches!(
            RusshConnector
                .open_shell(
                    &target,
                    SshCredential::Password("secret".to_string()),
                    80,
                    24,
                    None,
                )
                .await,
            Err(TerminalError {
                code: TerminalErrorCode::HostKeyMismatch,
                ..
            })
        ));
        let observed = observed.lock().unwrap();
        assert_eq!(observed.password_auth_attempts, 0);
        assert_eq!(observed.public_key_auth_attempts, 0);
        drop(observed);
        task.abort();
    }

    #[test]
    fn permits_private_and_loopback_but_filters_non_endpoints() {
        assert!(usable_address("127.0.0.1".parse().unwrap()));
        assert!(usable_address("192.168.1.2".parse().unwrap()));
        assert!(!usable_address("0.0.0.0".parse().unwrap()));
        assert!(!usable_address("224.0.0.1".parse().unwrap()));
        assert!(!usable_address("255.255.255.255".parse().unwrap()));
        assert!(!usable_address("::ffff:255.255.255.255".parse().unwrap()));
    }

    #[test]
    fn rejects_excessive_openssh_bcrypt_rounds_before_blocking_decode() {
        let key = ssh_key::PrivateKey::from_openssh(TEST_PRIVATE_KEY).unwrap();
        let encrypted = key
            .encrypt_with(
                ssh_key::Cipher::Aes256Ctr,
                ssh_key::Kdf::Bcrypt {
                    salt: vec![7; 16],
                    rounds: MAX_BCRYPT_KDF_ROUNDS + 1,
                },
                42,
                "password",
            )
            .unwrap()
            .to_openssh(ssh_key::LineEnding::LF)
            .unwrap();
        assert!(matches!(
            validate_private_key_cost(&encrypted),
            Err(TerminalError {
                code: TerminalErrorCode::AuthenticationFailed,
                ..
            })
        ));
    }
}
