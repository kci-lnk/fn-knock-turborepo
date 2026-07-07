use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{runtime::Handle, task};

use crate::{i18n::Translator, response, state::AppState, system_events, time_utils};

const LOG_KEY: &str = "fn_knock:cloudflared:logs";
const LOG_TTL_SECONDS: usize = 24 * 3600;
const LOG_MAX_LEN: usize = 1000;
const TUNNEL_RUNTIME_KEY: &str = "fn_knock:tunnel:runtime";
const CONNECTED_PATTERNS: &[&str] = &["registered tunnel connection", "connection "];
const DISCONNECTED_PATTERNS: &[&str] = &[
    "serve tunnel error",
    "tunnel disconnected",
    "failed to serve tunnel",
];

static CLOUDFLARED_MANAGER: OnceLock<CloudflaredManager> = OnceLock::new();

fn cloudflared_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.cloudflared.{key}"))
}

fn tunnel_manager_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.tunnelManagers.cloudflared.{key}"))
}

fn localize_cloudflared_error(translator: &Translator, message: &str) -> String {
    match message.trim() {
        "Cloudflared token is required" => cloudflared_text(translator, "missingToken"),
        "Cloudflared is not initialized" => cloudflared_text(translator, "notInitialized"),
        "Cloudflared platform is unsupported" => {
            tunnel_manager_text(translator, "platformUnsupported")
        }
        "Cloudflared is not installed. Install it with Homebrew first." => {
            tunnel_manager_text(translator, "notInstalledBrew")
        }
        value => value.to_string(),
    }
}

#[derive(Default)]
struct RunState {
    running: bool,
    pid: Option<u32>,
    connected: bool,
    stop_requested: bool,
}

struct CloudflaredManager {
    dir: PathBuf,
    config_path: PathBuf,
    bin_path: PathBuf,
    state: Mutex<RunState>,
}

#[derive(Deserialize)]
struct LogsQuery {
    limit: Option<String>,
    cursor: Option<String>,
}

#[derive(Clone)]
struct CloudflaredConfig {
    token: String,
    protocol: String,
}

pub fn cloudflared_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/cloudflared/status", get(status))
        .route(
            "/api/admin/cloudflared/config",
            get(config).post(save_config),
        )
        .route("/api/admin/cloudflared/start", post(start))
        .route("/api/admin/cloudflared/stop", post(stop))
        .route("/api/admin/cloudflared/logs", get(logs).delete(clear_logs))
        .route("/api/admin/cloudflared/poll", get(poll))
}

pub fn start_cloudflared_tasks(state: AppState) {
    manager(&state).ensure_dir();
    tokio::spawn(async move {
        match should_resume_tunnel(&state).await {
            Ok(true) => {
                let translator = Translator::from_state(&state).await;
                if let Err(error) =
                    append_logs(&state, vec![cloudflared_text(&translator, "resumeOnBoot")]).await
                {
                    tracing::warn!(%error, "failed to append cloudflared resume log");
                }
                if let Err(error) = manager(&state).start(state.clone()).await {
                    let _ = append_logs(&state, vec![format!("resume error: {error}")]).await;
                }
            }
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, "failed to load cloudflared resume state"),
        }
    });
}

async fn status(State(state): State<AppState>) -> Response {
    let manager = manager(&state);
    let asset = manager.asset_status();
    let run = manager.run_status();
    response::ok(json!({
        "initialized": asset.get("downloaded").and_then(Value::as_bool).unwrap_or(false),
        "platform": asset.get("platform").cloned().unwrap_or_else(|| json!("unsupported")),
        "running": run.0,
        "pid": run.1,
    }))
    .into_response()
}

async fn config(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match manager(&state).read_config() {
        Ok(config) => response::ok(json!({
            "token": config.token,
            "protocol": config.protocol,
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read cloudflared config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "configReadFailed"),
            )
        }
    }
}

async fn save_config(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = CloudflaredConfig {
        token: body
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        protocol: normalize_protocol(body.get("protocol").and_then(Value::as_str)),
    };
    match manager(&state).write_config(&config) {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to write cloudflared config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "configWriteFailed"),
            )
        }
    }
}

async fn start(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    if !manager.downloaded() {
        return response::error(
            StatusCode::BAD_REQUEST,
            cloudflared_text(&translator, "notInitialized"),
        );
    }
    match manager.start(state.clone()).await {
        Ok(pid) => response::ok(json!({ "pid": pid })).into_response(),
        Err(error) => {
            let _ = append_logs(&state, vec![format!("start error: {error}")]).await;
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                localize_cloudflared_error(&translator, &error),
            )
        }
    }
}

async fn stop(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match manager(&state).stop(&state).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to stop cloudflared");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "stopFailed"),
            )
        }
    }
}

async fn logs(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let limit = parse_log_limit(query.limit.as_deref(), 200, LOG_MAX_LEN);
    match state
        .redis
        .list_log_buffer(LOG_KEY, limit, LOG_MAX_LEN)
        .await
    {
        Ok(logs) => response::ok(logs).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsListFailed"),
            )
        }
    }
}

async fn clear_logs(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.clear_log_buffer(LOG_KEY).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to clear cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsClearFailed"),
            )
        }
    }
}

async fn poll(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
        .poll_log_buffer(LOG_KEY, query.cursor.as_deref())
        .await
    {
        Ok(mut result) => {
            let run = manager(&state).run_status();
            let cursor = result.get("cursor").cloned().unwrap_or_else(|| json!(0));
            let reset = result
                .get("reset")
                .cloned()
                .unwrap_or_else(|| Value::Bool(false));
            let logs = result
                .as_object_mut()
                .and_then(|object| object.remove("items"))
                .unwrap_or_else(|| json!([]));
            response::ok(json!({
                "cursor": cursor,
                "reset": reset,
                "logs": logs,
                "status": {
                    "running": run.0,
                    "pid": run.1,
                }
            }))
            .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to poll cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsPollFailed"),
            )
        }
    }
}

impl CloudflaredManager {
    fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("cloudflared");
        Self {
            config_path: dir.join("cloudflared.json"),
            bin_path: dir.join("cloudflared"),
            dir,
            state: Mutex::new(RunState::default()),
        }
    }

    fn ensure_dir(&self) {
        let _ = fs::create_dir_all(&self.dir);
    }

    fn run_status(&self) -> (bool, Value) {
        let state = self.state.lock().unwrap();
        (
            state.running,
            state.pid.map(Value::from).unwrap_or(Value::Null),
        )
    }

    fn asset_status(&self) -> Value {
        let platform = detect_platform();
        let downloaded = match platform.as_str() {
            "darwin" => command_exists("cloudflared"),
            "linux-amd64" | "linux-arm64" | "linux-arm" => self.bin_path.exists(),
            _ => false,
        };
        json!({
            "supported": platform != "unsupported",
            "platform": platform,
            "downloaded": downloaded,
            "progress": { "status": "idle", "percent": 0 }
        })
    }

    fn downloaded(&self) -> bool {
        self.asset_status()
            .get("downloaded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn executable(&self) -> Result<String, String> {
        match detect_platform().as_str() {
            "darwin" => which("cloudflared").ok_or_else(|| {
                "Cloudflared is not installed. Install it with Homebrew first.".to_string()
            }),
            "linux-amd64" | "linux-arm64" | "linux-arm" => {
                if self.bin_path.exists() {
                    Ok(self.bin_path.to_string_lossy().to_string())
                } else {
                    Err("Cloudflared is not initialized".to_string())
                }
            }
            _ => Err("Cloudflared platform is unsupported".to_string()),
        }
    }

    fn read_config(&self) -> Result<CloudflaredConfig, String> {
        self.ensure_dir();
        if !self.config_path.exists() {
            let config = CloudflaredConfig {
                token: String::new(),
                protocol: "auto".to_string(),
            };
            self.write_config(&config)?;
            return Ok(config);
        }
        let raw = fs::read_to_string(&self.config_path).map_err(|error| error.to_string())?;
        let value = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}));
        Ok(CloudflaredConfig {
            token: value
                .get("token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            protocol: normalize_protocol(value.get("protocol").and_then(Value::as_str)),
        })
    }

    fn write_config(&self, config: &CloudflaredConfig) -> Result<(), String> {
        self.ensure_dir();
        let value = json!({
            "token": config.token,
            "protocol": normalize_protocol(Some(&config.protocol)),
        });
        fs::write(
            &self.config_path,
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()),
        )
        .map_err(|error| error.to_string())
    }

    async fn start(&'static self, state: AppState) -> Result<u32, String> {
        {
            let run = self.state.lock().unwrap();
            if run.running {
                return Ok(run.pid.unwrap_or_default());
            }
        }
        let config = self.read_config()?;
        if !cloudflared_token_configured(&config.token) {
            return Err("Cloudflared token is required".to_string());
        }
        let executable = self.executable()?;
        let args = build_args(&config);
        let mut child = Command::new(&executable)
            .args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        {
            let mut run = self.state.lock().unwrap();
            run.running = true;
            run.pid = Some(pid);
            run.stop_requested = false;
        }
        mark_tunnel_running(&state).await?;
        append_logs(&state, vec![format!("cloudflared started pid={pid}")])
            .await
            .map_err(|error| error.to_string())?;
        if let Some(stdout) = stdout {
            spawn_log_reader(state.clone(), stdout, "stdout");
        }
        if let Some(stderr) = stderr {
            spawn_log_reader(state.clone(), stderr, "stderr");
        }
        let handle = Handle::current();
        task::spawn_blocking(move || {
            let exit_message = match child.wait() {
                Ok(status) => format!(
                    "cloudflared exited with code {}",
                    status.code().unwrap_or_default()
                ),
                Err(error) => format!("cloudflared process error: {error}"),
            };
            let (expected_stop, was_connected) = {
                let mut run = self.state.lock().unwrap();
                let expected_stop = run.stop_requested;
                let was_connected = run.connected;
                if run.pid == Some(pid) {
                    run.running = false;
                    run.pid = None;
                }
                run.connected = false;
                run.stop_requested = false;
                (expected_stop, was_connected)
            };
            let state_for_async = state.clone();
            handle.block_on(async move {
                let _ = mark_tunnel_stopped(&state_for_async).await;
                let _ = append_logs(&state_for_async, vec![exit_message.clone()]).await;
                if !expected_stop && was_connected {
                    if let Err(error) = system_events::publish_tunnel_connectivity_event(
                        &state_for_async,
                        "cloudflared",
                        false,
                        Some(pid),
                        Some(&exit_message),
                        None,
                        None,
                        None,
                    )
                    .await
                    {
                        tracing::warn!(%error, pid, "failed to publish cloudflared disconnect event");
                    }
                }
                if !expected_stop {
                    tracing::info!(pid, "cloudflared stopped unexpectedly");
                }
            });
        });
        Ok(pid)
    }

    async fn stop(&'static self, state: &AppState) -> Result<(), String> {
        let pid = {
            let mut run = self.state.lock().unwrap();
            run.stop_requested = true;
            run.connected = false;
            run.running = false;
            run.pid.take()
        };
        if let Some(pid) = pid {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        mark_tunnel_stopped(state).await?;
        Ok(())
    }
}

fn manager(state: &AppState) -> &'static CloudflaredManager {
    CLOUDFLARED_MANAGER.get_or_init(|| CloudflaredManager::new(&state.settings.data_dir))
}

fn spawn_log_reader<R>(state: AppState, reader: R, source: &'static str)
where
    R: Read + Send + 'static,
{
    let handle = Handle::current();
    task::spawn_blocking(move || {
        let mut buffered = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match buffered.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let normalized = line.trim_end().to_string();
                    if normalized.is_empty() {
                        continue;
                    }
                    let state_for_async = state.clone();
                    let text = normalized.clone();
                    handle.block_on(async move {
                        let _ = append_logs(&state_for_async, vec![text]).await;
                    });
                }
                Err(error) => {
                    let state_for_async = state.clone();
                    handle.block_on(async move {
                        let _ = append_logs(
                            &state_for_async,
                            vec![format!("cloudflared {source} read error: {error}")],
                        )
                        .await;
                    });
                    break;
                }
            }
        }
    });
}

async fn append_logs(state: &AppState, lines: Vec<String>) -> redis::RedisResult<()> {
    let normalized = lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Ok(());
    }
    handle_runtime_signals(state, &normalized).await;
    state
        .redis
        .append_log_buffer(LOG_KEY, &normalized, LOG_TTL_SECONDS, LOG_MAX_LEN)
        .await
}

async fn handle_runtime_signals(state: &AppState, lines: &[String]) {
    for line in lines {
        let Some(message) = normalize_tunnel_event_message(line) else {
            continue;
        };
        let normalized = message.to_ascii_lowercase();
        if is_cloudflared_connected_message(&normalized) {
            emit_cloudflared_connectivity(state, true, Some(&message), None).await;
            continue;
        }
        if is_cloudflared_disconnected_message(&normalized) {
            emit_cloudflared_connectivity(state, false, Some(&message), None).await;
        }
    }
}

async fn emit_cloudflared_connectivity(
    state: &AppState,
    connected: bool,
    message: Option<&str>,
    pid: Option<u32>,
) {
    let (should_emit, event_pid) = {
        let mut run = manager(state).state.lock().unwrap();
        if connected {
            if run.connected {
                return;
            }
            run.connected = true;
        } else {
            if !run.connected {
                return;
            }
            run.connected = false;
            if run.stop_requested {
                return;
            }
        }
        (true, pid.or(run.pid))
    };
    if !should_emit {
        return;
    }
    if let Err(error) = system_events::publish_tunnel_connectivity_event(
        state,
        "cloudflared",
        connected,
        event_pid,
        message,
        None,
        None,
        None,
    )
    .await
    {
        tracing::warn!(%error, "failed to publish cloudflared connectivity event");
    }
}

fn is_cloudflared_connected_message(normalized: &str) -> bool {
    CONNECTED_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
        && normalized.contains("registered")
}

fn is_cloudflared_disconnected_message(normalized: &str) -> bool {
    DISCONNECTED_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
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

fn parse_log_limit(value: Option<&str>, fallback: usize, max: usize) -> usize {
    let parsed = value
        .and_then(parse_node_parse_int)
        .unwrap_or(fallback as i64);
    parsed.clamp(1, max as i64) as usize
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

async fn should_resume_tunnel(state: &AppState) -> redis::RedisResult<bool> {
    let runtime = tunnel_runtime_state(state).await?;
    Ok(runtime
        .get("cloudflared_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

async fn mark_tunnel_running(state: &AppState) -> Result<(), String> {
    let mut runtime = tunnel_runtime_state(state)
        .await
        .map_err(|error| error.to_string())?;
    let object = runtime.as_object_mut().unwrap();
    object.insert("cloudflared_enabled".to_string(), Value::Bool(true));
    object.insert(
        "last_tunnel".to_string(),
        Value::String("cloudflared".to_string()),
    );
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    state
        .redis
        .set_json_value(TUNNEL_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())
}

async fn mark_tunnel_stopped(state: &AppState) -> Result<(), String> {
    let mut runtime = tunnel_runtime_state(state)
        .await
        .map_err(|error| error.to_string())?;
    let object = runtime.as_object_mut().unwrap();
    object.insert("cloudflared_enabled".to_string(), Value::Bool(false));
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    state
        .redis
        .set_json_value(TUNNEL_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())
}

async fn tunnel_runtime_state(state: &AppState) -> redis::RedisResult<Value> {
    let raw = state.redis.get_json_value(TUNNEL_RUNTIME_KEY).await?;
    Ok(normalize_tunnel_runtime_state(raw))
}

fn normalize_tunnel_runtime_state(value: Option<Value>) -> Value {
    let raw = value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if !raw.contains_key("frp_enabled")
        && !raw.contains_key("cloudflared_enabled")
        && raw.contains_key("tunnel")
        && raw.contains_key("enabled")
    {
        let tunnel = raw.get("tunnel").and_then(Value::as_str).unwrap_or("frp");
        let enabled = raw.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        return json!({
            "frp_enabled": tunnel == "frp" && enabled,
            "cloudflared_enabled": tunnel == "cloudflared" && enabled,
            "last_tunnel": if tunnel == "cloudflared" { "cloudflared" } else { "frp" },
            "updated_at": raw.get("updated_at").and_then(Value::as_str).unwrap_or("1970-01-01T00:00:00Z")
        });
    }
    json!({
        "frp_enabled": raw.get("frp_enabled").and_then(Value::as_bool).unwrap_or(false),
        "cloudflared_enabled": raw.get("cloudflared_enabled").and_then(Value::as_bool).unwrap_or(false),
        "last_tunnel": if raw.get("last_tunnel").and_then(Value::as_str) == Some("cloudflared") { "cloudflared" } else { "frp" },
        "updated_at": raw.get("updated_at").and_then(Value::as_str).unwrap_or("1970-01-01T00:00:00Z")
    })
}

fn normalize_protocol(value: Option<&str>) -> String {
    match value.unwrap_or("auto") {
        "http2" => "http2".to_string(),
        "quic" => "quic".to_string(),
        _ => "auto".to_string(),
    }
}

fn build_args(config: &CloudflaredConfig) -> Vec<String> {
    let mut args = vec!["tunnel".to_string(), "--no-autoupdate".to_string()];
    if config.protocol != "auto" {
        args.push("--protocol".to_string());
        args.push(config.protocol.clone());
    }
    args.push("run".to_string());
    args.push("--token".to_string());
    args.push(config.token.clone());
    args
}

fn cloudflared_token_configured(token: &str) -> bool {
    !token.is_empty()
}

fn detect_platform() -> String {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => "darwin".to_string(),
        ("linux", "x86_64" | "amd64") => "linux-amd64".to_string(),
        ("linux", "aarch64" | "arm64") => "linux-arm64".to_string(),
        ("linux", "arm") => "linux-arm".to_string(),
        _ => "unsupported".to_string(),
    }
}

fn command_exists(command: &str) -> bool {
    which(command).is_some()
}

fn which(command: &str) -> Option<String> {
    let output = Command::new("which").arg(command).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cloudflared_protocol() {
        assert_eq!(normalize_protocol(Some("http2")), "http2");
        assert_eq!(normalize_protocol(Some("bad")), "auto");
    }

    #[test]
    fn builds_cloudflared_args() {
        let args = build_args(&CloudflaredConfig {
            token: "tok".to_string(),
            protocol: "quic".to_string(),
        });
        assert_eq!(
            args,
            vec![
                "tunnel",
                "--no-autoupdate",
                "--protocol",
                "quic",
                "run",
                "--token",
                "tok"
            ]
        );
    }

    #[test]
    fn token_presence_matches_node_truthiness() {
        assert!(!cloudflared_token_configured(""));
        assert!(cloudflared_token_configured("   "));
    }

    #[test]
    fn normalizes_legacy_tunnel_runtime_state() {
        let state = normalize_tunnel_runtime_state(Some(json!({
            "tunnel": "cloudflared",
            "enabled": true,
            "updated_at": "2026-01-01T00:00:00Z"
        })));
        assert_eq!(state["cloudflared_enabled"], true);
        assert_eq!(state["last_tunnel"], "cloudflared");
    }

    #[test]
    fn log_limit_parser_matches_node_parse_int_prefixes() {
        assert_eq!(parse_log_limit(None, 200, LOG_MAX_LEN), 200);
        assert_eq!(parse_log_limit(Some(""), 200, LOG_MAX_LEN), 200);
        assert_eq!(parse_log_limit(Some("10x"), 200, LOG_MAX_LEN), 10);
        assert_eq!(parse_log_limit(Some("0x10"), 200, LOG_MAX_LEN), 1);
        assert_eq!(parse_log_limit(Some("-5"), 200, LOG_MAX_LEN), 1);
        assert_eq!(parse_log_limit(Some("5000"), 200, LOG_MAX_LEN), 1000);
        assert_eq!(parse_log_limit(Some("abc"), 200, LOG_MAX_LEN), 200);
    }

    #[test]
    fn localizes_cloudflared_route_errors() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_cloudflared_error(&translator, "Cloudflared token is required"),
            "请先配置 Cloudflare Token"
        );
        assert_eq!(
            localize_cloudflared_error(&translator, "Cloudflared is not initialized"),
            "Cloudflared 未初始化"
        );
        assert_eq!(
            cloudflared_text(&translator, "logsPollFailed"),
            "轮询 Cloudflared 日志失败"
        );
    }
}
