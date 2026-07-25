use async_trait::async_trait;

use super::*;
use crate::tunnels::supervisor::{
    OutputStream, ProcessLaunch, SupervisorHandle, TunnelProcessAdapter,
};

pub(super) fn supervisor_key(id: &str) -> String {
    format!("frpc:{id}")
}

pub(super) async fn ensure_frpc_supervisor(
    state: &AppState,
    meta: &FrpcInstanceMeta,
) -> FrpcResult<SupervisorHandle> {
    let runtime = read_runtime(&state.store, &meta.id).await?;
    let mut initial = runtime.supervisor;
    initial.desired_running = runtime.desired_running;
    initial.pid = runtime.pid.or(initial.pid);
    initial.running = initial.pid.is_some();
    initial.started_at = runtime.started_at.or(initial.started_at);
    initial.stopped_at = runtime.stopped_at.or(initial.stopped_at);
    initial.last_message = runtime.last_message.or(initial.last_message);
    let secrets = read_config_for_meta(meta)
        .await
        .ok()
        .map(|content| extract_frpc_secrets(&content))
        .unwrap_or_default();
    let adapter = Arc::new(FrpcProcessAdapter {
        state: state.clone(),
        meta: meta.clone(),
        connection: Arc::new(Mutex::new(FrpcConnectionState::default())),
        secrets: std::sync::RwLock::new(secrets),
    });
    Ok(state
        .tunnel_supervisors
        .ensure(adapter, initial, state.shutdown.clone())
        .await)
}

struct FrpcProcessAdapter {
    state: AppState,
    meta: FrpcInstanceMeta,
    connection: Arc<Mutex<FrpcConnectionState>>,
    secrets: std::sync::RwLock<Vec<String>>,
}

impl FrpcProcessAdapter {
    async fn current_meta(&self) -> FrpcInstanceMeta {
        read_meta(&self.state.store, &self.state, &self.meta.id)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| self.meta.clone())
    }
}

#[async_trait]
impl TunnelProcessAdapter for FrpcProcessAdapter {
    fn key(&self) -> String {
        supervisor_key(&self.meta.id)
    }

    fn label(&self) -> String {
        "frpc".to_string()
    }

    async fn prepare_launch(&self) -> Result<ProcessLaunch, String> {
        let Some(bin) = frp_executable(&self.state) else {
            return Err("FRP is not initialized".to_string());
        };
        let content = read_config_for_meta(&self.meta)
            .await
            .map_err(|error| error.to_string())?;
        *self
            .secrets
            .write()
            .unwrap_or_else(|error| error.into_inner()) = extract_frpc_secrets(&content);
        verify_frpc_config(&self.state, &self.meta, &content)
            .await
            .map_err(|error| self.sanitize_output(&error.message))?;
        Ok(ProcessLaunch {
            executable: bin.into_os_string(),
            args: vec!["-c".into(), self.meta.config_path.clone().into()],
            current_dir: PathBuf::from(&self.meta.work_dir),
        })
    }

    async fn find_existing_pid(&self) -> Option<u32> {
        let runtime = read_runtime(&self.state.store, &self.meta.id)
            .await
            .ok()
            .unwrap_or_else(default_runtime);
        read_candidate_pid(&self.meta, &runtime).await
    }

    async fn owns_live_pid(&self, pid: u32) -> bool {
        is_owned_frpc_pid(pid, &self.meta.config_path).await
    }

    async fn persist_snapshot(&self, snapshot: &SupervisorSnapshot) -> Result<(), String> {
        let mut runtime = read_runtime(&self.state.store, &self.meta.id)
            .await
            .map_err(|error| error.to_string())?;
        let previous_runtime = runtime.clone();
        runtime.desired_running = snapshot.desired_running;
        runtime.pid = snapshot.pid;
        runtime.started_at = snapshot.started_at.clone();
        runtime.stopped_at = snapshot.stopped_at.clone();
        runtime.last_exit_code = if snapshot.running {
            None
        } else {
            snapshot
                .last_failure
                .as_ref()
                .and_then(|failure| failure.exit_code)
        };
        runtime.last_message = snapshot.last_message.clone();
        runtime.supervisor = snapshot.clone();
        write_runtime(&self.state.store, &self.meta.id, &runtime)
            .await
            .map_err(|error| error.to_string())?;
        if let Err(error) = update_aggregate_tunnel_state(&self.state).await {
            let rollback = write_runtime(&self.state.store, &self.meta.id, &previous_runtime).await;
            return Err(match rollback {
                Ok(()) => error.to_string(),
                Err(rollback_error) => {
                    format!("{error}; failed to roll back frpc runtime: {rollback_error}")
                }
            });
        }
        Ok(())
    }

    fn sanitize_output(&self, line: &str) -> String {
        self.secrets
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .filter(|secret| !secret.is_empty())
            .fold(line.to_string(), |line, secret| {
                line.replace(secret, "[REDACTED]")
            })
    }

    async fn append_output(&self, stream: OutputStream, line: String) {
        let meta = self.current_meta().await;
        let line = match stream {
            OutputStream::Stdout => format!("[OUT] {line}"),
            OutputStream::Stderr => format!("[ERR] {line}"),
        };
        if let Err(error) = append_logs(&self.state, &meta, std::slice::from_ref(&line)).await {
            tracing::warn!(instance_id = %self.meta.id, %error, "failed to append frpc process log");
        }
        handle_frpc_runtime_signal(&self.state, &meta, &self.connection, &line).await;
    }

    async fn append_supervisor_log(&self, line: String) {
        if let Err(error) = append_logs(&self.state, &self.meta, &[line]).await {
            tracing::warn!(instance_id = %self.meta.id, %error, "failed to append frpc supervisor log");
        }
    }

    async fn set_expected_stop(&self, expected: bool) {
        let mut connection = self.connection.lock().await;
        connection.stop_requested = expected;
        if expected {
            connection.connected = false;
        }
    }

    async fn on_unexpected_exit(&self, pid: Option<u32>, failure: &SupervisorFailure) {
        let meta = self.current_meta().await;
        let mut lines = vec![format_failure_summary("frpc", pid, failure)];
        lines.extend(
            failure
                .recent_stdout
                .iter()
                .map(|line| format!("[LAST stdout] {line}")),
        );
        lines.extend(
            failure
                .recent_stderr
                .iter()
                .map(|line| format!("[LAST stderr] {line}")),
        );
        let _ = append_logs(&self.state, &meta, &lines).await;
        emit_frpc_connectivity_with_state(
            &self.state,
            &meta,
            &self.connection,
            false,
            Some(&failure.reason),
            pid,
        )
        .await;
    }

    async fn remove_pid_file(&self) {
        remove_pid_file(&pid_path_for_meta(&self.meta)).await;
    }

    async fn write_pid_file(&self, pid: u32) {
        write_pid_file(&pid_path_for_meta(&self.meta), pid).await;
    }
}

pub(super) fn extract_frpc_secrets(content: &str) -> Vec<String> {
    let mut secrets = content
        .lines()
        .filter_map(extract_sensitive_assignment)
        .collect::<Vec<_>>();
    if let Ok(document) = toml::from_str::<toml::Value>(content) {
        collect_toml_secrets(&document, false, &mut secrets);
    }
    let mut unique = Vec::with_capacity(secrets.len());
    for secret in secrets {
        if !secret.is_empty() && !unique.iter().any(|value| value == &secret) {
            unique.push(secret);
        }
    }
    unique
}

fn extract_sensitive_assignment(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let key = key
        .rsplit('.')
        .next()
        .unwrap_or(key)
        .trim()
        .trim_matches(['"', '\'']);
    is_sensitive_frpc_key(key)
        .then(|| parse_toml_scalar_fallback(value))
        .flatten()
}

fn parse_toml_scalar_fallback(value: &str) -> Option<String> {
    let value = value.trim_start();
    let quote = value.chars().next();
    if matches!(quote, Some('"') | Some('\'')) {
        let quote = quote?;
        let mut escaped = false;
        for (index, character) in value.char_indices().skip(1) {
            if quote == '"' && character == '\\' && !escaped {
                escaped = true;
                continue;
            }
            if character == quote && !escaped {
                return Some(value[1..index].to_string()).filter(|value| !value.is_empty());
            }
            escaped = false;
        }
        return None;
    }
    value
        .split('#')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn collect_toml_secrets(value: &toml::Value, sensitive: bool, secrets: &mut Vec<String>) {
    match value {
        toml::Value::String(value) if sensitive => secrets.push(value.clone()),
        toml::Value::Array(values) => {
            for value in values {
                collect_toml_secrets(value, sensitive, secrets);
            }
        }
        toml::Value::Table(table) => {
            for (key, value) in table {
                collect_toml_secrets(value, sensitive || is_sensitive_frpc_key(key), secrets);
            }
        }
        _ => {}
    }
}

fn is_sensitive_frpc_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("passwd")
        || normalized.contains("credential")
}

fn format_failure_summary(label: &str, pid: Option<u32>, failure: &SupervisorFailure) -> String {
    let mut parts = vec![
        format!("{label} stopped unexpectedly"),
        format!(
            "pid={}",
            pid.map_or_else(|| "-".to_string(), |pid| pid.to_string())
        ),
        format!("startedAt={}", failure.started_at.as_deref().unwrap_or("-")),
        format!("exitedAt={}", failure.at),
        format!("reason={}", failure.reason),
        format!("uptimeMs={}", failure.uptime_ms),
    ];
    if let Some(signal) = failure.signal {
        parts.push(format!("signal={signal}"));
    }
    if let Some(code) = failure.exit_code {
        parts.push(format!("exitCode={code}"));
    }
    if let Some(diagnosis) = failure.diagnosis.as_deref() {
        parts.push(format!("diagnosis={diagnosis}"));
    }
    parts.join(" ")
}

async fn handle_frpc_runtime_signal(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    connection: &Arc<Mutex<FrpcConnectionState>>,
    line: &str,
) {
    let Some(message) = normalize_frpc_tunnel_event_message(line) else {
        return;
    };
    let normalized = message.to_ascii_lowercase();
    let pid = state
        .tunnel_supervisors
        .get(&supervisor_key(&meta.id))
        .await
        .and_then(|handle| handle.snapshot().pid);
    if FRPC_CONNECTED_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
    {
        emit_frpc_connectivity_with_state(state, meta, connection, true, Some(&message), pid).await;
    } else if FRPC_DISCONNECTED_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
    {
        emit_frpc_connectivity_with_state(state, meta, connection, false, Some(&message), pid)
            .await;
    }
}

async fn emit_frpc_connectivity_with_state(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    connection: &Arc<Mutex<FrpcConnectionState>>,
    connected: bool,
    message: Option<&str>,
    pid: Option<u32>,
) {
    {
        let mut connection = connection.lock().await;
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
