use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::platform;

pub const SERVICE_NAME: &str = "FnKnock";
const READY_PATH: &str = "/__fn-knock/readyz";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListenerScope {
    Loopback,
    All,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub schema_version: u32,
    pub onboarding_complete: bool,
    pub admin_port: u16,
    pub backend_port: u16,
    pub auth_port: u16,
    pub grpc_port: u16,
    pub proxy_port: u16,
    pub listener_scope: ListenerScope,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            onboarding_complete: false,
            admin_port: 7991,
            backend_port: 7998,
            auth_port: 7997,
            grpc_port: 7996,
            proxy_port: 7999,
            listener_scope: ListenerScope::Loopback,
        }
    }
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported runtime configuration schema {}",
                self.schema_version
            ));
        }
        let ports = [
            self.admin_port,
            self.backend_port,
            self.auth_port,
            self.grpc_port,
            self.proxy_port,
        ];
        if ports.iter().any(|port| *port == 0) {
            return Err("ports must be between 1 and 65535".to_string());
        }
        if [
            self.admin_port,
            self.backend_port,
            self.auth_port,
            self.grpc_port,
        ]
        .iter()
        .any(|port| *port < 1024)
        {
            return Err("internal ports must be at least 1024".to_string());
        }
        if ports.into_iter().collect::<HashSet<_>>().len() != ports.len() {
            return Err("the five runtime ports must be unique".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PortStatus {
    pub name: &'static str,
    pub port: u16,
    pub available: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DesktopStatus {
    pub version: String,
    pub service_state: String,
    pub ready: bool,
    pub ready_detail: Option<String>,
    pub failure: Option<String>,
    pub data_dir: String,
    pub install_dir: String,
    pub firewall_rule_enabled: bool,
    pub config: RuntimeConfig,
    pub ports: Vec<PortStatus>,
}

pub fn data_dir() -> PathBuf {
    platform::program_data_dir().unwrap_or_else(|_| {
        #[cfg(windows)]
        {
            PathBuf::from(r"C:\ProgramData\FnKnock")
        }
        #[cfg(not(windows))]
        {
            std::env::temp_dir().join("FnKnock")
        }
    })
}

pub fn config_path() -> PathBuf {
    data_dir().join("config").join("runtime.json")
}

pub fn status_path() -> PathBuf {
    data_dir().join("state").join("status.json")
}

pub fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn load_runtime_config() -> Result<RuntimeConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(RuntimeConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let config: RuntimeConfig = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    config.validate()?;
    Ok(config)
}

/// The desktop process runs as the interactive user and is intentionally not
/// allowed to read ProgramData configuration or secrets. The service mirrors
/// only the non-sensitive ports and listener scope into state/status.json.
pub fn load_public_runtime_config() -> Result<RuntimeConfig, String> {
    load_runtime_config().or_else(|config_error| {
        read_status_document()
            .as_ref()
            .and_then(runtime_config_from_status)
            .ok_or(config_error)
    })
}

pub fn save_runtime_config(config: &RuntimeConfig) -> Result<(), String> {
    config.validate()?;
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("failed to encode runtime configuration: {error}"))?;
    platform::write_runtime_config_and_restart(&config_path(), &bytes)
}

pub fn collect_status() -> DesktopStatus {
    let status_document = read_status_document();
    let (config, config_failure) = match load_runtime_config().or_else(|config_error| {
        status_document
            .as_ref()
            .and_then(runtime_config_from_status)
            .ok_or(config_error)
    }) {
        Ok(config) => (config, None),
        Err(error) => (RuntimeConfig::default(), Some(error)),
    };
    let (ready, ready_detail) = check_ready(config.admin_port);
    let failure = config_failure.or_else(|| extract_failure(status_document.as_ref(), ready));

    DesktopStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        service_state: platform::service_state(SERVICE_NAME),
        ready,
        ready_detail,
        failure,
        data_dir: data_dir().display().to_string(),
        install_dir: install_dir().display().to_string(),
        firewall_rule_enabled: platform::firewall_rule_enabled(),
        ports: collect_port_status(&config),
        config,
    }
}

pub fn check_ready(port: u16) -> (bool, Option<String>) {
    if let Err(error) = platform::verify_service_listener(SERVICE_NAME, port) {
        return (false, Some(error));
    }
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut stream = match TcpStream::connect_timeout(&address, Duration::from_millis(550)) {
        Ok(stream) => stream,
        Err(error) => return (false, Some(format!("管理服务未响应：{error}"))),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let request = format!(
        "GET {READY_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return (false, Some(format!("就绪检查发送失败：{error}")));
    }
    let mut response = String::new();
    if let Err(error) = stream.read_to_string(&mut response) {
        return (false, Some(format!("就绪检查读取失败：{error}")));
    }
    let status_line = response.lines().next().unwrap_or_default();
    let http_ok = status_line.contains(" 200 ");
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim())
        .filter(|body| !body.is_empty());
    let listener_still_trusted = platform::verify_service_listener(SERVICE_NAME, port).is_ok();
    let ready = listener_still_trusted && http_ok && body.is_some_and(ready_document_is_complete);
    let detail = if ready {
        None
    } else if !listener_still_trusted {
        Some("就绪检查期间 FnKnock 服务或管理端口所有者发生变化".to_string())
    } else {
        body.and_then(read_ready_detail).or_else(|| {
            http_ok
                .then(|| "就绪响应未确认完整运行组件".to_string())
                .or_else(|| Some(status_line.to_string()))
        })
    };
    (ready, detail)
}

fn ready_document_is_complete(body: &str) -> bool {
    let Ok(document) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    let Some(components) = document.get("components") else {
        return false;
    };
    document.get("ready").and_then(Value::as_bool) == Some(true)
        && document.get("version").and_then(Value::as_str) == Some(env!("CARGO_PKG_VERSION"))
        && document.get("control_api_version").and_then(Value::as_u64) == Some(1)
        && [
            "storage",
            "gateway_bundle",
            "gateway_process",
            "gateway_dataplane",
            "auth_bridge",
        ]
        .into_iter()
        .all(|name| components.get(name).and_then(Value::as_bool) == Some(true))
}

fn read_ready_detail(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            ["message", "error", "detail", "status"]
                .iter()
                .find_map(|key| value.get(key).and_then(Value::as_str))
                .map(str::to_string)
        })
        .or_else(|| Some(body.chars().take(240).collect()))
}

fn read_status_document() -> Option<Value> {
    let raw = fs::read_to_string(status_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn runtime_config_from_status(status: &Value) -> Option<RuntimeConfig> {
    let ports = status.get("ports")?;
    let config = RuntimeConfig {
        schema_version: 1,
        onboarding_complete: status
            .get("onboarding_complete")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        admin_port: u16::try_from(ports.get("admin")?.as_u64()?).ok()?,
        backend_port: u16::try_from(ports.get("backend")?.as_u64()?).ok()?,
        auth_port: u16::try_from(ports.get("auth")?.as_u64()?).ok()?,
        grpc_port: u16::try_from(ports.get("grpc")?.as_u64()?).ok()?,
        proxy_port: u16::try_from(ports.get("proxy")?.as_u64()?).ok()?,
        listener_scope: match status.get("listener_scope")?.as_str()? {
            "loopback" => ListenerScope::Loopback,
            "all" => ListenerScope::All,
            _ => return None,
        },
    };
    config.validate().ok()?;
    Some(config)
}

fn extract_failure(status: Option<&Value>, ready: bool) -> Option<String> {
    if ready {
        return None;
    }
    let status = status?;
    if status.get("state").and_then(Value::as_str) != Some("faulted") {
        return None;
    }
    ["failure", "error", "message", "detail"]
        .iter()
        .find_map(|key| status.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

fn collect_port_status(config: &RuntimeConfig) -> Vec<PortStatus> {
    [
        ("管理", config.admin_port),
        ("Rust API", config.backend_port),
        ("认证", config.auth_port),
        ("Go gRPC", config.grpc_port),
        ("代理", config.proxy_port),
    ]
    .into_iter()
    .map(|(name, port)| PortStatus {
        name,
        port,
        available: TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok(),
    })
    .collect()
}

pub fn export_diagnostics() -> Result<PathBuf, String> {
    let status = collect_status();
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let document = json!({
        "schema_version": 1,
        "created_at_unix": created_at,
        "product": "FnKnock",
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "status": status,
        "note": "This diagnostic export intentionally excludes configuration secrets, SQLite data, certificates, tokens, and log content."
    });
    let directory = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
    let path = directory.join(format!("fn-knock-diagnostics-{created_at}.json"));
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("failed to encode diagnostics: {error}"))?;
    fs::write(&path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_are_stable_and_unique() {
        let config = RuntimeConfig::default();
        assert_eq!(config.admin_port, 7991);
        assert_eq!(config.backend_port, 7998);
        assert_eq!(config.auth_port, 7997);
        assert_eq!(config.grpc_port, 7996);
        assert_eq!(config.proxy_port, 7999);
        assert!(!config.onboarding_complete);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn duplicate_ports_are_rejected() {
        let mut config = RuntimeConfig::default();
        config.grpc_port = config.admin_port;
        assert!(config.validate().unwrap_err().contains("unique"));
    }

    #[test]
    fn public_status_reconstructs_non_sensitive_runtime_config() {
        let status = json!({
            "state": "running",
            "onboarding_complete": true,
            "ports": {
                "admin": 17991,
                "backend": 17998,
                "auth": 17997,
                "grpc": 17996,
                "proxy": 17999
            },
            "listener_scope": "all"
        });
        let config = runtime_config_from_status(&status).unwrap();
        assert_eq!(config.admin_port, 17991);
        assert_eq!(config.proxy_port, 17999);
        assert!(config.onboarding_complete);
        assert!(matches!(config.listener_scope, ListenerScope::All));
    }

    #[test]
    fn ready_document_requires_full_matching_runtime() {
        let complete = json!({
            "ready": true,
            "version": env!("CARGO_PKG_VERSION"),
            "control_api_version": 1,
            "components": {
                "storage": true,
                "gateway_bundle": true,
                "gateway_process": true,
                "gateway_dataplane": true,
                "auth_bridge": true
            }
        });
        assert!(ready_document_is_complete(&complete.to_string()));

        let mut incomplete = complete;
        incomplete["components"]["auth_bridge"] = Value::Bool(false);
        assert!(!ready_document_is_complete(&incomplete.to_string()));
        assert!(!ready_document_is_complete("not-json"));
    }
}
