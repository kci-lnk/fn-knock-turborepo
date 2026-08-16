use russh::{
    ChannelMsg, Disconnect, client,
    keys::{HashAlg, PrivateKeyWithHashAlg, decode_secret_key, ssh_key},
};
use serde::Serialize;
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{net::lookup_host, time};

use super::store::TargetSshConfig;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const AUTH_TIMEOUT: Duration = Duration::from_secs(8);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REMOTE_OUTPUT: usize = 8 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostKeyInfo {
    pub algorithm: String,
    pub fingerprint: String,
    #[serde(skip)]
    pub endpoint: SocketAddr,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConnectionTestResult {
    pub authenticated: bool,
    pub privilege_ready: bool,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ShutdownResult {
    pub status: &'static str,
    pub platform: String,
    pub latency_ms: u64,
}

pub(super) enum Credentials {
    Password(String),
    PrivateKey {
        key: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SshError {
    #[error("SSH host or port is invalid")]
    InvalidEndpoint,
    #[error("SSH endpoint resolves to a protected local address")]
    ProtectedAddress,
    #[error("SSH host key does not match the trusted fingerprint")]
    HostKeyMismatch,
    #[error("SSH host key is unavailable")]
    HostKeyUnavailable,
    #[error("SSH credential is invalid")]
    InvalidCredential,
    #[error("SSH authentication failed")]
    AuthenticationFailed,
    #[error("SSH connection failed")]
    ConnectionFailed,
    #[error("SSH command failed")]
    CommandFailed,
    #[error("SSH command result is unknown")]
    CommandUnknown,
}

#[derive(Clone)]
struct HostKeyHandler {
    expected: Option<(String, String)>,
    observed: Arc<Mutex<Option<(String, String)>>>,
}

impl client::Handler for HostKeyHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();
        *self
            .observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some((algorithm.clone(), fingerprint.clone()));
        Ok(self
            .expected
            .as_ref()
            .is_none_or(|expected| expected == &(algorithm, fingerprint)))
    }
}

pub(super) async fn probe_host_key(host: &str, port: u16) -> Result<HostKeyInfo, SshError> {
    let endpoint = resolve_endpoint(host, port).await?;
    let observed = Arc::new(Mutex::new(None));
    let handler = HostKeyHandler {
        expected: None,
        observed: Arc::clone(&observed),
    };
    let session = connect(endpoint, handler).await?;
    let _ = session
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;
    let key = observed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .ok_or(SshError::HostKeyUnavailable)?;
    Ok(HostKeyInfo {
        algorithm: key.0,
        fingerprint: key.1,
        endpoint,
    })
}

pub(super) async fn test_connection(
    config: &TargetSshConfig,
    credentials: Credentials,
    endpoint: SocketAddr,
) -> Result<ConnectionTestResult, SshError> {
    let started = Instant::now();
    let mut session = authenticated_session_at(config, credentials, endpoint).await?;
    let exit_status = execute(&mut session, test_command(&config.platform)).await?;
    let _ = session
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;
    Ok(ConnectionTestResult {
        authenticated: true,
        privilege_ready: exit_status == 0,
        latency_ms: elapsed_ms(started),
    })
}

pub(super) async fn shutdown(
    config: &TargetSshConfig,
    credentials: Credentials,
) -> Result<ShutdownResult, SshError> {
    let started = Instant::now();
    let mut session = authenticated_session(config, credentials).await?;
    let exit_status = execute(&mut session, shutdown_command(&config.platform)).await?;
    let _ = session
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;
    if exit_status != 0 {
        return Err(SshError::CommandFailed);
    }
    Ok(ShutdownResult {
        status: "accepted",
        platform: config.platform.clone(),
        latency_ms: elapsed_ms(started),
    })
}

async fn authenticated_session(
    config: &TargetSshConfig,
    credentials: Credentials,
) -> Result<client::Handle<HostKeyHandler>, SshError> {
    let endpoint = resolve_endpoint(&config.host, config.port).await?;
    authenticated_session_at(config, credentials, endpoint).await
}

async fn authenticated_session_at(
    config: &TargetSshConfig,
    credentials: Credentials,
    endpoint: SocketAddr,
) -> Result<client::Handle<HostKeyHandler>, SshError> {
    let expected = trusted_host_key(config)?;
    let observed = Arc::new(Mutex::new(None));
    let handler = HostKeyHandler {
        expected: Some(expected.clone()),
        observed: Arc::clone(&observed),
    };
    let mut session = match connect(endpoint, handler).await {
        Ok(session) => session,
        Err(error) => {
            if observed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .is_some_and(|value| value != &expected)
            {
                return Err(SshError::HostKeyMismatch);
            }
            return Err(error);
        }
    };
    let authenticated = time::timeout(AUTH_TIMEOUT, async {
        let success = match credentials {
            Credentials::Password(password) => session
                .authenticate_password(&config.username, password)
                .await
                .map_err(|_| SshError::ConnectionFailed)?
                .success(),
            Credentials::PrivateKey { key, passphrase } => {
                let key = decode_secret_key(&key, passphrase.as_deref())
                    .map_err(|_| SshError::InvalidCredential)?;
                let hash = session
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|_| SshError::ConnectionFailed)?
                    .flatten();
                session
                    .authenticate_publickey(
                        &config.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
                    .map_err(|_| SshError::ConnectionFailed)?
                    .success()
            }
        };
        Ok::<bool, SshError>(success)
    })
    .await
    .map_err(|_| SshError::ConnectionFailed)??;
    if !authenticated {
        return Err(SshError::AuthenticationFailed);
    }
    Ok(session)
}

async fn connect(
    endpoint: SocketAddr,
    handler: HostKeyHandler,
) -> Result<client::Handle<HostKeyHandler>, SshError> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(COMMAND_TIMEOUT),
        ..Default::default()
    });
    time::timeout(CONNECT_TIMEOUT, client::connect(config, endpoint, handler))
        .await
        .map_err(|_| SshError::ConnectionFailed)?
        .map_err(|_| SshError::ConnectionFailed)
}

async fn execute(
    session: &mut client::Handle<HostKeyHandler>,
    command: &str,
) -> Result<u32, SshError> {
    let future = async {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|_| SshError::CommandFailed)?;
        channel
            .exec(true, command)
            .await
            .map_err(|_| SshError::CommandUnknown)?;
        let mut output_size = 0_usize;
        let mut exit_status = None;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    output_size = output_size.saturating_add(data.len());
                    if output_size > MAX_REMOTE_OUTPUT {
                        return Err(SshError::CommandUnknown);
                    }
                }
                ChannelMsg::ExitStatus { exit_status: value } => exit_status = Some(value),
                _ => {}
            }
        }
        exit_status.ok_or(SshError::CommandUnknown)
    };
    time::timeout(COMMAND_TIMEOUT, future)
        .await
        .map_err(|_| SshError::CommandUnknown)?
}

async fn resolve_endpoint(host: &str, port: u16) -> Result<SocketAddr, SshError> {
    let host = host.trim();
    if host.is_empty() || host.len() > 253 || port == 0 || host.chars().any(char::is_control) {
        return Err(SshError::InvalidEndpoint);
    }
    let local_addresses = get_if_addrs::get_if_addrs()
        .map_err(|_| SshError::ConnectionFailed)?
        .into_iter()
        .map(|interface| interface.ip())
        .collect::<HashSet<_>>();
    let addresses = time::timeout(CONNECT_TIMEOUT, lookup_host((host, port)))
        .await
        .map_err(|_| SshError::ConnectionFailed)?
        .map_err(|_| SshError::InvalidEndpoint)?
        .collect::<Vec<_>>();
    let endpoint = addresses
        .into_iter()
        .find(|address| !protected_address(address.ip(), &local_addresses))
        .ok_or(SshError::ProtectedAddress)?;
    Ok(endpoint)
}

fn protected_address(address: IpAddr, local_addresses: &HashSet<IpAddr>) -> bool {
    if let IpAddr::V6(value) = address
        && let Some(mapped) = value.to_ipv4_mapped()
    {
        return protected_address(IpAddr::V4(mapped), local_addresses);
    }
    address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || matches!(address, IpAddr::V4(value) if value.is_broadcast())
        || local_addresses.contains(&address)
}

fn trusted_host_key(config: &TargetSshConfig) -> Result<(String, String), SshError> {
    let algorithm = config.host_key_algorithm.trim();
    let fingerprint = config.host_key_fingerprint.trim();
    if algorithm.is_empty()
        || algorithm.len() > 64
        || fingerprint.len() > 128
        || !fingerprint.starts_with("SHA256:")
    {
        return Err(SshError::HostKeyUnavailable);
    }
    Ok((algorithm.to_string(), fingerprint.to_string()))
}

pub(super) fn shutdown_command(platform: &str) -> &'static str {
    match platform {
        "macos" => "sudo -n /sbin/shutdown -h now",
        "windows" => "shutdown.exe /s /t 0",
        _ => "sudo -n /usr/bin/systemctl poweroff --no-block",
    }
}

fn test_command(platform: &str) -> &'static str {
    match platform {
        "macos" => "sudo -n -l -- /sbin/shutdown -h now",
        "windows" => "cmd.exe /d /s /c \"ver >NUL\"",
        _ => "sudo -n -l -- /usr/bin/systemctl poweroff --no-block",
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::{
        Channel, ChannelId,
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

    #[derive(Clone)]
    struct MockSshServer {
        commands: Arc<Mutex<Vec<String>>>,
        exit_status: Option<u32>,
        output_size: usize,
    }

    impl server::Server for MockSshServer {
        type Handler = Self;

        fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
            self.clone()
        }
    }

    impl server::Handler for MockSshServer {
        type Error = russh::Error;

        async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
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

        async fn exec_request(
            &mut self,
            channel: ChannelId,
            data: &[u8],
            session: &mut Session,
        ) -> Result<(), Self::Error> {
            self.commands
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(String::from_utf8_lossy(data).into_owned());
            session.channel_success(channel)?;
            if self.output_size > 0 {
                session.data(channel, vec![b'x'; self.output_size])?;
            }
            if let Some(exit_status) = self.exit_status {
                session.exit_status_request(channel, exit_status)?;
            }
            session.eof(channel)?;
            session.close(channel)?;
            Ok(())
        }
    }

    async fn start_mock_server_with(
        exit_status: Option<u32>,
        output_size: usize,
    ) -> (
        SocketAddr,
        TargetSshConfig,
        Arc<Mutex<Vec<String>>>,
        JoinHandle<()>,
    ) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let host_key = decode_secret_key(TEST_PRIVATE_KEY, None).unwrap();
        let public_key = host_key.public_key();
        let config = TargetSshConfig {
            enabled: true,
            host: endpoint.ip().to_string(),
            port: endpoint.port(),
            username: "operator".to_string(),
            platform: "linux".to_string(),
            auth_method: "password".to_string(),
            host_key_algorithm: public_key.algorithm().to_string(),
            host_key_fingerprint: public_key.fingerprint(HashAlg::Sha256).to_string(),
        };
        let commands = Arc::new(Mutex::new(Vec::new()));
        let mut server = MockSshServer {
            commands: Arc::clone(&commands),
            exit_status,
            output_size,
        };
        let server_config = Arc::new(server::Config {
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            keys: vec![host_key],
            ..Default::default()
        });
        let task = tokio::spawn(async move {
            let _ = server.run_on_socket(server_config, &listener).await;
        });
        (endpoint, config, commands, task)
    }

    async fn start_mock_server() -> (
        SocketAddr,
        TargetSshConfig,
        Arc<Mutex<Vec<String>>>,
        JoinHandle<()>,
    ) {
        start_mock_server_with(Some(0), 0).await
    }

    #[test]
    fn uses_fixed_platform_shutdown_commands() {
        assert_eq!(
            shutdown_command("linux"),
            "sudo -n /usr/bin/systemctl poweroff --no-block"
        );
        assert_eq!(shutdown_command("macos"), "sudo -n /sbin/shutdown -h now");
        assert_eq!(shutdown_command("windows"), "shutdown.exe /s /t 0");
        assert_eq!(
            test_command("linux"),
            "sudo -n -l -- /usr/bin/systemctl poweroff --no-block"
        );
        assert_eq!(test_command("macos"), "sudo -n -l -- /sbin/shutdown -h now");
    }

    #[test]
    fn rejects_local_and_non_routable_addresses() {
        let local = HashSet::from(["192.0.2.10".parse().unwrap()]);
        assert!(protected_address("127.0.0.1".parse().unwrap(), &local));
        assert!(protected_address("0.0.0.0".parse().unwrap(), &local));
        assert!(protected_address("224.0.0.1".parse().unwrap(), &local));
        assert!(protected_address(
            "255.255.255.255".parse().unwrap(),
            &local
        ));
        assert!(protected_address(
            "::ffff:127.0.0.1".parse().unwrap(),
            &local
        ));
        assert!(protected_address("192.0.2.10".parse().unwrap(), &local));
        assert!(!protected_address("192.0.2.20".parse().unwrap(), &local));
    }

    #[tokio::test]
    async fn authenticates_before_executing_fixed_commands_and_checks_host_key() {
        let (endpoint, mut config, commands, task) = start_mock_server().await;

        let mut session = authenticated_session_at(
            &config,
            Credentials::Password("secret".to_string()),
            endpoint,
        )
        .await
        .unwrap();
        assert_eq!(
            execute(&mut session, shutdown_command("linux"))
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            commands
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_slice(),
            ["sudo -n /usr/bin/systemctl poweroff --no-block"]
        );

        config.auth_method = "privateKey".to_string();
        assert!(
            authenticated_session_at(
                &config,
                Credentials::PrivateKey {
                    key: TEST_PRIVATE_KEY.to_string(),
                    passphrase: None,
                },
                endpoint,
            )
            .await
            .is_ok()
        );
        assert!(
            authenticated_session_at(
                &config,
                Credentials::PrivateKey {
                    key: TEST_ENCRYPTED_PRIVATE_KEY.to_string(),
                    passphrase: Some("hunter42".to_string()),
                },
                endpoint,
            )
            .await
            .is_ok()
        );

        config.host_key_fingerprint = "SHA256:wrong".to_string();
        assert!(matches!(
            authenticated_session_at(
                &config,
                Credentials::Password("secret".to_string()),
                endpoint,
            )
            .await,
            Err(SshError::HostKeyMismatch)
        ));

        task.abort();
    }

    #[tokio::test]
    async fn rejects_invalid_password_without_executing_a_command() {
        let (endpoint, config, commands, task) = start_mock_server().await;
        assert!(matches!(
            authenticated_session_at(
                &config,
                Credentials::Password("wrong".to_string()),
                endpoint,
            )
            .await,
            Err(SshError::AuthenticationFailed)
        ));
        assert!(
            commands
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty()
        );
        task.abort();
    }

    #[tokio::test]
    async fn distinguishes_nonzero_exit_from_unknown_post_dispatch_results() {
        let (endpoint, config, _, task) = start_mock_server_with(Some(7), 0).await;
        let mut session = authenticated_session_at(
            &config,
            Credentials::Password("secret".to_string()),
            endpoint,
        )
        .await
        .unwrap();
        assert_eq!(
            execute(&mut session, shutdown_command("linux"))
                .await
                .unwrap(),
            7
        );
        task.abort();

        let (endpoint, config, _, task) = start_mock_server_with(None, 0).await;
        let mut session = authenticated_session_at(
            &config,
            Credentials::Password("secret".to_string()),
            endpoint,
        )
        .await
        .unwrap();
        assert!(matches!(
            execute(&mut session, shutdown_command("linux")).await,
            Err(SshError::CommandUnknown)
        ));
        task.abort();

        let (endpoint, config, _, task) =
            start_mock_server_with(Some(0), MAX_REMOTE_OUTPUT + 1).await;
        let mut session = authenticated_session_at(
            &config,
            Credentials::Password("secret".to_string()),
            endpoint,
        )
        .await
        .unwrap();
        assert!(matches!(
            execute(&mut session, shutdown_command("linux")).await,
            Err(SshError::CommandUnknown)
        ));
        task.abort();
    }
}
