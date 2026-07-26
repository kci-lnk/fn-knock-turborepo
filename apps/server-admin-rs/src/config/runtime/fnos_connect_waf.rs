use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
    process::Command,
    time::Duration,
};

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ipnet::IpNet;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{fs::File, io::AsyncReadExt, net::TcpStream, time};

use super::*;

const FNOS_GATEWAY_SETTINGS_PATH: &str = "/usr/trim/etc/network_gateway_setting.conf";
const FNOS_GATEWAY_SETTINGS_MAX_BYTES: u64 = 64 * 1024;
const FNOS_CONNECT_CGROUP_PATH: &str = "system.slice/trim_connect.service";
const FNOS_CONNECT_OUTPUT_CHAIN: &str = "FNK_FNC_OUT";
const FNOS_CONNECT_PREROUTING_CHAIN: &str = "FNK_FNC_PRE";
const FNOS_CONNECT_INPUT_CHAIN: &str = "FNK_FNC_IN";
const FNOS_CONNECT_LEGACY_CHAIN: &str = "FNK_FNC_WAF";
const FNOS_CONNECT_RULE_COMMENT: &str = "fn-knock:fn-connect-waf";
const FNOS_CONNECT_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const FNOS_CONNECT_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FirewallFamily {
    Ipv4,
    Ipv6,
}

impl FirewallFamily {
    fn binary(self) -> &'static str {
        match self {
            Self::Ipv4 => "iptables",
            Self::Ipv6 => "ip6tables",
        }
    }

    fn destination(self) -> &'static str {
        match self {
            Self::Ipv4 => "127.0.0.1/32",
            Self::Ipv6 => "::1/128",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FirewallTable {
    Nat,
    Filter,
}

impl FirewallTable {
    fn name(self) -> &'static str {
        match self {
            Self::Nat => "nat",
            Self::Filter => "filter",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LocalNetworks {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

impl LocalNetworks {
    fn for_family(&self, family: FirewallFamily) -> &[String] {
        match family {
            FirewallFamily::Ipv4 => &self.ipv4,
            FirewallFamily::Ipv6 => &self.ipv6,
        }
    }
}

#[derive(Debug, Deserialize)]
struct IpAddressLink {
    #[serde(default)]
    addr_info: Vec<IpAddressInfo>,
}

#[derive(Debug, Deserialize)]
struct IpAddressInfo {
    family: String,
    local: String,
    prefixlen: u8,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct FnosGatewaySettings {
    schema: FnosGatewaySchema,
    #[serde(default)]
    force_https: bool,
}

#[derive(Debug, Deserialize)]
struct FnosGatewaySchema {
    http: FnosGatewayPort,
}

#[derive(Debug, Deserialize)]
struct FnosGatewayPort {
    port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DetectedFnosHttpPort {
    port: u16,
    source: String,
}

pub(super) async fn get_fnos_connect_waf(State(state): State<AppState>) -> Response {
    match build_fnos_connect_waf_response(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load FN Connect WAF status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "读取 FN Connect WAF 状态失败",
            )
        }
    }
}

pub(super) async fn update_fnos_connect_waf(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    if !fnos_connect_waf_available(&state) {
        return response::error(StatusCode::FORBIDDEN, "FN Connect WAF 仅支持标准版 FPK");
    }
    let Some(enabled) = body.get("enabled").and_then(Value::as_bool) else {
        return response::error(StatusCode::BAD_REQUEST, "enabled 必须是布尔值");
    };

    let _guard = state.fnos_connect_waf_update_lock.lock().await;
    let previous_config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before FN Connect WAF update");
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, "读取配置失败");
        }
    };
    let previous_enabled = normalize_fnos_connect_waf(previous_config.get("fnos_connect_waf"))
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if let Err(error) = apply_fnos_connect_waf_runtime(&state, enabled).await {
        tracing::warn!(%error, enabled, "failed to apply FN Connect WAF runtime");
        return response::error(StatusCode::BAD_GATEWAY, error.to_string());
    }

    let next = json!({
        "enabled": enabled,
        "updated_at": time_utils::now_iso(),
    });
    let mut next_config = previous_config.clone();
    if !next_config.is_object() {
        next_config = app_store::default_config();
    }
    ensure_config_object(&mut next_config).insert("fnos_connect_waf".to_string(), next);
    if let Err(error) = state.store.save_config(&next_config).await {
        tracing::warn!(%error, "failed to persist FN Connect WAF config; restoring runtime");
        if let Err(rollback_error) = apply_fnos_connect_waf_runtime(&state, previous_enabled).await
        {
            tracing::warn!(%rollback_error, "failed to restore FN Connect WAF runtime");
        }
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, "保存配置失败");
    }

    state.fnos_connect_waf_notify.notify_one();
    match build_fnos_connect_waf_response(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

pub(crate) fn start_fnos_connect_waf_reconciler(state: AppState) {
    if !fnos_connect_waf_available(&state) {
        return;
    }
    tokio::spawn(async move {
        let mut interval = time::interval(FNOS_CONNECT_RECONCILE_INTERVAL);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        let mut first = true;
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => {
                    let _guard = state.fnos_connect_waf_update_lock.lock().await;
                    if let Err(error) = disable_fnos_connect_waf_runtime(&state).await {
                        tracing::warn!(%error, "failed to clean FN Connect WAF runtime on shutdown");
                    }
                    break;
                }
                _ = interval.tick() => {}
                _ = state.fnos_connect_waf_notify.notified() => {}
            }

            let _guard = state.fnos_connect_waf_update_lock.lock().await;
            let config = match state.store.get_config().await {
                Ok(config) => config,
                Err(error) => {
                    tracing::warn!(%error, "failed to load FN Connect WAF config during reconcile");
                    continue;
                }
            };
            let enabled = normalize_fnos_connect_waf(config.get("fnos_connect_waf"))
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !enabled && !first {
                let current = state.fnos_connect_waf_status.read().await;
                if !disabled_fnos_connect_waf_needs_reconcile(&current) {
                    continue;
                }
            }
            first = false;
            let result = if enabled {
                reconcile_enabled_fnos_connect_waf_runtime(&state).await
            } else {
                disable_fnos_connect_waf_runtime(&state).await
            };
            if let Err(error) = result {
                tracing::warn!(%error, enabled, "failed to reconcile FN Connect WAF runtime");
            }
        }
    });
}

pub(crate) fn normalize_fnos_connect_waf(value: Option<&Value>) -> Value {
    json!({
        "enabled": value
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "updated_at": value
            .and_then(|value| value.get("updated_at"))
            .and_then(Value::as_str)
            .map(Value::from)
            .unwrap_or(Value::Null),
    })
}

async fn build_fnos_connect_waf_response(state: &AppState) -> anyhow::Result<Value> {
    let config = state.store.get_config().await?;
    let runtime = state.fnos_connect_waf_status.read().await.clone();
    let available = fnos_connect_waf_available(state);
    Ok(json!({
        "availability": {
            "available": available,
            "reason_code": if available { Value::Null } else { Value::String("standard_fpk_required".to_string()) },
        },
        "config": normalize_fnos_connect_waf(config.get("fnos_connect_waf")),
        "runtime": runtime,
    }))
}

fn fnos_connect_waf_available(state: &AppState) -> bool {
    let profile = runtime_profile::get_runtime_profile(state);
    runtime_profile::get_runtime_capabilities(&profile).fnos_connect_waf_available
}

async fn apply_fnos_connect_waf_runtime(state: &AppState, enabled: bool) -> anyhow::Result<()> {
    if !enabled {
        return disable_fnos_connect_waf_runtime(state).await;
    }

    let detected = match detect_fnos_http_port(Path::new(FNOS_GATEWAY_SETTINGS_PATH)).await {
        Ok(detected) => detected,
        Err(error) => {
            fail_open_fnos_connect_waf(state, &error.to_string()).await;
            return Err(error);
        }
    };
    let local_networks = match detect_local_networks_async().await {
        Ok(networks) => networks,
        Err(error) => {
            fail_open_fnos_connect_waf(state, &error).await;
            anyhow::bail!(error);
        }
    };
    let go_response = match state
        .go_backend
        .set_fnos_connect_ingress_config(true, detected.port)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            fail_open_fnos_connect_waf(state, &error.to_string()).await;
            return Err(error);
        }
    };
    let go_status = go_response
        .get("data")
        .cloned()
        .unwrap_or_else(|| go_response.clone());
    let listener_port = match go_status
        .get("listen_port")
        .and_then(Value::as_i64)
        .filter(|port| (1..=65535).contains(port))
        .map(|port| port as u16)
    {
        Some(port) => port,
        None => {
            let error = anyhow::anyhow!("Go FN Connect 入口未返回有效监听端口");
            fail_open_fnos_connect_waf(state, &error.to_string()).await;
            return Err(error);
        }
    };
    if !go_status
        .get("listener_active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || !go_status
            .get("ipv4_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || !go_status
            .get("ipv6_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        let error = anyhow::anyhow!("Go FN Connect 双栈入口未完全就绪");
        fail_open_fnos_connect_waf(state, &error.to_string()).await;
        return Err(error);
    }

    if let Err(error) =
        install_firewall_rules(detected.port, listener_port, local_networks.clone()).await
    {
        fail_open_fnos_connect_waf(state, &error).await;
        anyhow::bail!(error);
    }
    let waf_active = go_status
        .get("waf_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let waf_mode = go_status
        .get("waf_mode")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let protected = fnos_connect_waf_protected(waf_active, waf_mode.as_deref());
    *state.fnos_connect_waf_status.write().await = json!({
        "effective": true,
        "protected": protected,
        "detected_http_port": detected.port,
        "listener_port": listener_port,
        "ipv4_redirect_active": true,
        "ipv6_redirect_active": true,
        "ipv4_relay_redirect_active": true,
        "ipv6_relay_redirect_active": true,
        "ipv4_direct_redirect_active": true,
        "ipv6_direct_redirect_active": true,
        "listener_guard_active": true,
        "local_networks": local_networks_json(&local_networks),
        "waf_active": waf_active,
        "waf_mode": waf_mode,
        "cgroup_path": FNOS_CONNECT_CGROUP_PATH,
        "source": detected.source,
        "last_sync_at": time_utils::now_iso(),
        "last_error": null,
    });
    Ok(())
}

async fn reconcile_enabled_fnos_connect_waf_runtime(state: &AppState) -> anyhow::Result<()> {
    let detected = match detect_fnos_http_port(Path::new(FNOS_GATEWAY_SETTINGS_PATH)).await {
        Ok(detected) => detected,
        Err(error) => {
            fail_open_fnos_connect_waf(state, &error.to_string()).await;
            return Err(error);
        }
    };
    let local_networks = match detect_local_networks_async().await {
        Ok(networks) => networks,
        Err(error) => {
            fail_open_fnos_connect_waf(state, &error).await;
            anyhow::bail!(error);
        }
    };
    let go_response = match state.go_backend.get_fnos_connect_ingress_status().await {
        Ok(response) => response,
        Err(_) => return apply_fnos_connect_waf_runtime(state, true).await,
    };
    let go_status = go_response
        .get("data")
        .cloned()
        .unwrap_or_else(|| go_response.clone());
    let listener_port = go_status
        .get("listen_port")
        .and_then(Value::as_i64)
        .filter(|port| (1..=65535).contains(port))
        .map(|port| port as u16);
    let healthy_go = go_status
        .get("listener_active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && go_status
            .get("ipv4_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && go_status
            .get("ipv6_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && go_status.get("upstream_http_port").and_then(Value::as_i64)
            == Some(i64::from(detected.port));
    let local_networks_unchanged = {
        let current = state.fnos_connect_waf_status.read().await;
        current.get("local_networks") == Some(&local_networks_json(&local_networks))
    };
    let healthy_firewall = if let Some(listener_port) = listener_port {
        local_networks_unchanged
            && firewall_rules_active(detected.port, listener_port, local_networks.clone()).await
    } else {
        false
    };
    if !healthy_go || !healthy_firewall {
        return apply_fnos_connect_waf_runtime(state, true).await;
    }

    let Some(listener_port) = listener_port else {
        return apply_fnos_connect_waf_runtime(state, true).await;
    };
    let waf_active = go_status
        .get("waf_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let waf_mode = go_status
        .get("waf_mode")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let protected = fnos_connect_waf_protected(waf_active, waf_mode.as_deref());
    *state.fnos_connect_waf_status.write().await = json!({
        "effective": true,
        "protected": protected,
        "detected_http_port": detected.port,
        "listener_port": listener_port,
        "ipv4_redirect_active": true,
        "ipv6_redirect_active": true,
        "ipv4_relay_redirect_active": true,
        "ipv6_relay_redirect_active": true,
        "ipv4_direct_redirect_active": true,
        "ipv6_direct_redirect_active": true,
        "listener_guard_active": true,
        "local_networks": local_networks_json(&local_networks),
        "waf_active": waf_active,
        "waf_mode": waf_mode,
        "cgroup_path": FNOS_CONNECT_CGROUP_PATH,
        "source": detected.source,
        "last_sync_at": time_utils::now_iso(),
        "last_error": null,
    });
    Ok(())
}

async fn disable_fnos_connect_waf_runtime(state: &AppState) -> anyhow::Result<()> {
    if let Err(error) = cleanup_firewall_rules().await {
        let mut current = state.fnos_connect_waf_status.write().await;
        ensure_config_object(&mut current).insert(
            "last_error".to_string(),
            Value::String(format!("清理 FN Connect 防火墙规则失败: {error}")),
        );
        ensure_config_object(&mut current).insert(
            "last_sync_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        // Keep the Go listener alive: an undeleted redirect must never point
        // at a closed port and turn a cleanup failure into an outage.
        anyhow::bail!(error);
    }
    let go_result = state
        .go_backend
        .set_fnos_connect_ingress_config(false, 0)
        .await;
    let error = go_result.err().map(|error| error.to_string());
    *state.fnos_connect_waf_status.write().await = json!({
        "effective": false,
        "protected": false,
        "detected_http_port": null,
        "listener_port": null,
        "ipv4_redirect_active": false,
        "ipv6_redirect_active": false,
        "ipv4_relay_redirect_active": false,
        "ipv6_relay_redirect_active": false,
        "ipv4_direct_redirect_active": false,
        "ipv6_direct_redirect_active": false,
        "listener_guard_active": false,
        "local_networks": null,
        "waf_active": false,
        "waf_mode": null,
        "cgroup_path": FNOS_CONNECT_CGROUP_PATH,
        "source": FNOS_GATEWAY_SETTINGS_PATH,
        "last_sync_at": time_utils::now_iso(),
        "last_error": error.clone(),
    });
    if let Some(error) = error {
        anyhow::bail!(error);
    }
    Ok(())
}

fn fnos_connect_waf_protected(waf_active: bool, waf_mode: Option<&str>) -> bool {
    waf_active && waf_mode.is_some_and(|mode| mode.eq_ignore_ascii_case("blocking"))
}

fn disabled_fnos_connect_waf_needs_reconcile(status: &Value) -> bool {
    status
        .get("effective")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || status
            .get("last_error")
            .is_some_and(|error| !error.is_null())
}

async fn fail_open_fnos_connect_waf(state: &AppState, message: &str) {
    let cleanup_error = cleanup_firewall_rules().await.err();
    if let Some(error) = cleanup_error.as_ref() {
        tracing::warn!(%error, "failed to clean FN Connect WAF rules during fail-open");
    } else if let Err(error) = state
        .go_backend
        .set_fnos_connect_ingress_config(false, 0)
        .await
    {
        tracing::warn!(%error, "failed to stop FN Connect ingress during fail-open");
    }
    let last_error = cleanup_error
        .map(|error| format!("{message}; 清理防火墙规则失败，已保留本地入口: {error}"))
        .unwrap_or_else(|| message.to_string());
    *state.fnos_connect_waf_status.write().await = json!({
        "effective": false,
        "protected": false,
        "detected_http_port": null,
        "listener_port": null,
        "ipv4_redirect_active": false,
        "ipv6_redirect_active": false,
        "ipv4_relay_redirect_active": false,
        "ipv6_relay_redirect_active": false,
        "ipv4_direct_redirect_active": false,
        "ipv6_direct_redirect_active": false,
        "listener_guard_active": false,
        "local_networks": null,
        "waf_active": false,
        "waf_mode": null,
        "cgroup_path": FNOS_CONNECT_CGROUP_PATH,
        "source": FNOS_GATEWAY_SETTINGS_PATH,
        "last_sync_at": time_utils::now_iso(),
        "last_error": last_error,
    });
}

async fn detect_fnos_http_port(path: &Path) -> anyhow::Result<DetectedFnosHttpPort> {
    let bytes = read_fnos_gateway_settings(path).await?;
    let settings = parse_fnos_gateway_settings(&bytes)?;
    let port = validated_fnos_http_port(&settings)?;
    probe_fnos_loopback(port).await?;
    Ok(DetectedFnosHttpPort {
        port,
        source: path.display().to_string(),
    })
}

async fn read_fnos_gateway_settings(path: &Path) -> anyhow::Result<Vec<u8>> {
    let file = File::open(path)
        .await
        .map_err(|error| anyhow::anyhow!("读取 fnOS 网关设置失败: {error}"))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|error| anyhow::anyhow!("读取 fnOS 网关设置元数据失败: {error}"))?;
    if metadata.len() > FNOS_GATEWAY_SETTINGS_MAX_BYTES {
        anyhow::bail!("fnOS 网关设置文件超过 64 KiB 安全上限");
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(FNOS_GATEWAY_SETTINGS_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| anyhow::anyhow!("读取 fnOS 网关设置失败: {error}"))?;
    if bytes.len() as u64 > FNOS_GATEWAY_SETTINGS_MAX_BYTES {
        anyhow::bail!("fnOS 网关设置文件超过 64 KiB 安全上限");
    }
    Ok(bytes)
}

fn validated_fnos_http_port(settings: &FnosGatewaySettings) -> anyhow::Result<u16> {
    if settings.force_https {
        anyhow::bail!("fnOS 已启用强制 HTTPS，当前版本不会重定向明文 FN Connect 流量");
    }
    Ok(settings.schema.http.port)
}

fn parse_fnos_gateway_settings(bytes: &[u8]) -> anyhow::Result<FnosGatewaySettings> {
    if bytes.is_empty() {
        anyhow::bail!("fnOS 网关设置文件为空");
    }
    let settings: FnosGatewaySettings = serde_json::from_slice(bytes)
        .map_err(|error| anyhow::anyhow!("解析 fnOS 网关设置失败: {error}"))?;
    if settings.schema.http.port == 0 {
        anyhow::bail!("fnOS HTTP 端口必须在 1 到 65535 之间");
    }
    Ok(settings)
}

async fn probe_fnos_loopback(port: u16) -> anyhow::Result<()> {
    for address in [
        std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        std::net::SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ] {
        match time::timeout(FNOS_CONNECT_PROBE_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => drop(stream),
            Ok(Err(error)) => anyhow::bail!("fnOS HTTP 端口 {address} 不可连接: {error}"),
            Err(_) => anyhow::bail!("连接 fnOS HTTP 端口 {address} 超时"),
        }
    }
    Ok(())
}

fn relay_rule_args(
    family: FirewallFamily,
    action: &str,
    source_port: u16,
    target_port: u16,
) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "nat",
        action,
        FNOS_CONNECT_OUTPUT_CHAIN,
        "-m",
        "cgroup",
        "--path",
        FNOS_CONNECT_CGROUP_PATH,
        "-d",
        family.destination(),
        "-p",
        "tcp",
        "--dport",
        &source_port.to_string(),
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "REDIRECT",
        "--to-ports",
        &target_port.to_string(),
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn direct_exemption_rule_args(action: &str, cidr: &str) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "nat",
        action,
        FNOS_CONNECT_PREROUTING_CHAIN,
        "-s",
        cidr,
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "RETURN",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn direct_redirect_rule_args(action: &str, source_port: u16, target_port: u16) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "nat",
        action,
        FNOS_CONNECT_PREROUTING_CHAIN,
        "-p",
        "tcp",
        "--dport",
        &source_port.to_string(),
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "REDIRECT",
        "--to-ports",
        &target_port.to_string(),
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn output_parent_jump_args(action: &str, chain: &str) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "nat",
        action,
        "OUTPUT",
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        chain,
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn prerouting_parent_jump_args(action: &str) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "nat",
        action,
        "PREROUTING",
        "-m",
        "addrtype",
        "--dst-type",
        "LOCAL",
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        FNOS_CONNECT_PREROUTING_CHAIN,
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn input_parent_jump_args(action: &str) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "filter",
        action,
        "INPUT",
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        FNOS_CONNECT_INPUT_CHAIN,
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn loopback_input_rule_args(family: FirewallFamily, action: &str, target_port: u16) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "filter",
        action,
        FNOS_CONNECT_INPUT_CHAIN,
        "-i",
        "lo",
        "-s",
        family.destination(),
        "-p",
        "tcp",
        "--dport",
        &target_port.to_string(),
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "ACCEPT",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn redirected_input_rule_args(action: &str, source_port: u16, target_port: u16) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "filter",
        action,
        FNOS_CONNECT_INPUT_CHAIN,
        "-p",
        "tcp",
        "--dport",
        &target_port.to_string(),
        "-m",
        "conntrack",
        "--ctstate",
        "DNAT",
        "--ctdir",
        "ORIGINAL",
        "--ctorigdstport",
        &source_port.to_string(),
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "ACCEPT",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn private_listener_drop_rule_args(action: &str, target_port: u16) -> Vec<String> {
    [
        "-w",
        "5",
        "-t",
        "filter",
        action,
        FNOS_CONNECT_INPUT_CHAIN,
        "-p",
        "tcp",
        "--dport",
        &target_port.to_string(),
        "-m",
        "comment",
        "--comment",
        FNOS_CONNECT_RULE_COMMENT,
        "-j",
        "DROP",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

async fn install_firewall_rules(
    source_port: u16,
    target_port: u16,
    local_networks: LocalNetworks,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        install_firewall_rules_blocking(source_port, target_port, &local_networks)
    })
    .await
    .map_err(|error| format!("等待防火墙配置任务失败: {error}"))?
}

async fn cleanup_firewall_rules() -> Result<(), String> {
    tokio::task::spawn_blocking(cleanup_firewall_rules_blocking)
        .await
        .map_err(|error| format!("等待防火墙清理任务失败: {error}"))?
}

async fn firewall_rules_active(
    source_port: u16,
    target_port: u16,
    local_networks: LocalNetworks,
) -> bool {
    tokio::task::spawn_blocking(move || {
        let mut executor = SystemFirewallExecutor;
        [FirewallFamily::Ipv4, FirewallFamily::Ipv6]
            .into_iter()
            .all(|family| {
                executor
                    .check(
                        family,
                        &output_parent_jump_args("-C", FNOS_CONNECT_OUTPUT_CHAIN),
                    )
                    .unwrap_or(false)
                    && executor
                        .check(
                            family,
                            &relay_rule_args(family, "-C", source_port, target_port),
                        )
                        .unwrap_or(false)
                    && executor
                        .check(family, &prerouting_parent_jump_args("-C"))
                        .unwrap_or(false)
                    && local_networks.for_family(family).iter().all(|cidr| {
                        executor
                            .check(family, &direct_exemption_rule_args("-C", cidr))
                            .unwrap_or(false)
                    })
                    && executor
                        .check(
                            family,
                            &direct_redirect_rule_args("-C", source_port, target_port),
                        )
                        .unwrap_or(false)
                    && executor
                        .check(family, &input_parent_jump_args("-C"))
                        .unwrap_or(false)
                    && executor
                        .check(family, &loopback_input_rule_args(family, "-C", target_port))
                        .unwrap_or(false)
                    && executor
                        .check(
                            family,
                            &redirected_input_rule_args("-C", source_port, target_port),
                        )
                        .unwrap_or(false)
                    && executor
                        .check(family, &private_listener_drop_rule_args("-C", target_port))
                        .unwrap_or(false)
            })
    })
    .await
    .unwrap_or(false)
}

fn install_firewall_rules_blocking(
    source_port: u16,
    target_port: u16,
    local_networks: &LocalNetworks,
) -> Result<(), String> {
    install_firewall_rules_with(
        &mut SystemFirewallExecutor,
        source_port,
        target_port,
        local_networks,
    )
}

fn install_firewall_rules_with(
    executor: &mut impl FirewallExecutor,
    source_port: u16,
    target_port: u16,
    local_networks: &LocalNetworks,
) -> Result<(), String> {
    cleanup_firewall_rules_with(executor)?;
    let result = (|| {
        for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
            executor.run(
                family,
                &string_args(&["-w", "5", "-t", "nat", "-N", FNOS_CONNECT_OUTPUT_CHAIN]),
            )?;
            executor.run(
                family,
                &string_args(&["-w", "5", "-t", "nat", "-N", FNOS_CONNECT_PREROUTING_CHAIN]),
            )?;
            executor.run(
                family,
                &string_args(&["-w", "5", "-t", "filter", "-N", FNOS_CONNECT_INPUT_CHAIN]),
            )?;
            executor.run(
                family,
                &relay_rule_args(family, "-A", source_port, target_port),
            )?;
            for cidr in local_networks.for_family(family) {
                executor.run(family, &direct_exemption_rule_args("-A", cidr))?;
            }
            executor.run(
                family,
                &direct_redirect_rule_args("-A", source_port, target_port),
            )?;
            executor.run(family, &loopback_input_rule_args(family, "-A", target_port))?;
            executor.run(
                family,
                &redirected_input_rule_args("-A", source_port, target_port),
            )?;
            executor.run(family, &private_listener_drop_rule_args("-A", target_port))?;
        }
        for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
            // Guard the wildcard listener before attaching either redirect.
            executor.run(family, &input_parent_jump_args("-I"))?;
            executor.run(
                family,
                &output_parent_jump_args("-I", FNOS_CONNECT_OUTPUT_CHAIN),
            )?;
            // Attach public direct interception last. Cleanup reverses this
            // ordering so no redirect can outlive its protected listener.
            executor.run(family, &prerouting_parent_jump_args("-I"))?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = cleanup_firewall_rules_with(executor);
    }
    result
}

fn cleanup_firewall_rules_blocking() -> Result<(), String> {
    cleanup_firewall_rules_with(&mut SystemFirewallExecutor)
}

fn cleanup_firewall_rules_with(executor: &mut impl FirewallExecutor) -> Result<(), String> {
    let mut errors = Vec::new();
    for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
        let nat_rules = match executor.list_rules(family, FirewallTable::Nat) {
            Ok(rules) => Some(rules),
            Err(error) => {
                errors.push(error);
                None
            }
        };
        if let Some(rules) = nat_rules.as_ref() {
            cleanup_chain_from_snapshot(
                executor,
                family,
                FirewallTable::Nat,
                FNOS_CONNECT_PREROUTING_CHAIN,
                &[prerouting_parent_jump_args("-C")],
                rules,
                &mut errors,
            );
            cleanup_chain_from_snapshot(
                executor,
                family,
                FirewallTable::Nat,
                FNOS_CONNECT_OUTPUT_CHAIN,
                &[output_parent_jump_args("-C", FNOS_CONNECT_OUTPUT_CHAIN)],
                rules,
                &mut errors,
            );
            cleanup_chain_from_snapshot(
                executor,
                family,
                FirewallTable::Nat,
                FNOS_CONNECT_LEGACY_CHAIN,
                &[output_parent_jump_args("-C", FNOS_CONNECT_LEGACY_CHAIN)],
                rules,
                &mut errors,
            );
        }

        let filter_rules = match executor.list_rules(family, FirewallTable::Filter) {
            Ok(rules) => Some(rules),
            Err(error) => {
                errors.push(error);
                None
            }
        };
        if let Some(rules) = filter_rules.as_ref() {
            cleanup_chain_from_snapshot(
                executor,
                family,
                FirewallTable::Filter,
                FNOS_CONNECT_INPUT_CHAIN,
                &[input_parent_jump_args("-C")],
                rules,
                &mut errors,
            );
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn cleanup_chain_from_snapshot(
    executor: &mut impl FirewallExecutor,
    family: FirewallFamily,
    table: FirewallTable,
    chain: &str,
    parent_checks: &[Vec<String>],
    rules: &[String],
    errors: &mut Vec<String>,
) {
    let chain_rule = format!("-N {chain}");
    if !rules.iter().any(|rule| rule == &chain_rule) {
        // A jump cannot reference a non-existent user chain. Avoid `-C`
        // because iptables-nft reports that ordinary clean state as status 2.
        return;
    }
    for parent_check in parent_checks {
        remove_parent_jump_copies(executor, family, parent_check, errors);
    }
    if let Err(error) = executor.run(
        family,
        &string_args(&["-w", "5", "-t", table.name(), "-F", chain]),
    ) {
        errors.push(error);
    }
    if let Err(error) = executor.run(
        family,
        &string_args(&["-w", "5", "-t", table.name(), "-X", chain]),
    ) {
        errors.push(error);
    }
}

fn remove_parent_jump_copies(
    executor: &mut impl FirewallExecutor,
    family: FirewallFamily,
    check_args: &[String],
    errors: &mut Vec<String>,
) {
    let mut delete_args = check_args.to_vec();
    if let Some(action) = delete_args.iter_mut().find(|arg| arg.as_str() == "-C") {
        *action = "-D".to_string();
    }
    let mut limit_reached = true;
    for _ in 0..8 {
        match executor.check(family, check_args) {
            Ok(true) => {}
            Ok(false) => {
                limit_reached = false;
                break;
            }
            Err(error) => {
                errors.push(error);
                limit_reached = false;
                break;
            }
        }
        if let Err(error) = executor.run(family, &delete_args) {
            errors.push(error);
            limit_reached = false;
            break;
        }
    }
    if limit_reached {
        match executor.check(family, check_args) {
            Ok(true) => errors.push(format!(
                "{} 中存在超过 8 条 FN Connect WAF 父链跳转",
                family.binary()
            )),
            Ok(false) => {}
            Err(error) => errors.push(error),
        }
    }
}

fn detect_local_networks() -> Result<LocalNetworks, String> {
    let output = Command::new("ip")
        .args(["-j", "address", "show", "up"])
        .output()
        .map_err(|error| format!("枚举本机网络地址失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "枚举本机网络地址失败（状态 {}）{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }
    parse_local_networks(&output.stdout)
}

async fn detect_local_networks_async() -> Result<LocalNetworks, String> {
    tokio::task::spawn_blocking(detect_local_networks)
        .await
        .map_err(|error| format!("等待本机网络地址枚举任务失败: {error}"))?
}

fn local_networks_json(networks: &LocalNetworks) -> Value {
    json!({
        "ipv4": networks.ipv4,
        "ipv6": networks.ipv6,
    })
}

fn parse_local_networks(bytes: &[u8]) -> Result<LocalNetworks, String> {
    let links: Vec<IpAddressLink> =
        serde_json::from_slice(bytes).map_err(|error| format!("解析本机网络地址失败: {error}"))?;
    let mut ipv4 = BTreeSet::from([
        "10.0.0.0/8".to_string(),
        "127.0.0.0/8".to_string(),
        "169.254.0.0/16".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
    ]);
    let mut ipv6 = BTreeSet::from([
        "::1/128".to_string(),
        "fc00::/7".to_string(),
        "fe80::/10".to_string(),
    ]);
    for address in links.into_iter().flat_map(|link| link.addr_info) {
        if address.scope != "global" && address.scope != "link" {
            continue;
        }
        let Ok(network) = format!("{}/{}", address.local, address.prefixlen).parse::<IpNet>()
        else {
            continue;
        };
        match network {
            IpNet::V4(network) if address.family == "inet" && network.prefix_len() >= 8 => {
                ipv4.insert(format!("{}/{}", network.network(), network.prefix_len()));
            }
            IpNet::V6(network) if address.family == "inet6" && network.prefix_len() >= 16 => {
                ipv6.insert(format!("{}/{}", network.network(), network.prefix_len()));
            }
            _ => {}
        }
    }
    Ok(LocalNetworks {
        ipv4: ipv4.into_iter().collect(),
        ipv6: ipv6.into_iter().collect(),
    })
}

fn string_args(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

trait FirewallExecutor {
    fn run(&mut self, family: FirewallFamily, args: &[String]) -> Result<(), String>;
    fn check(&mut self, family: FirewallFamily, args: &[String]) -> Result<bool, String>;
    fn list_rules(
        &mut self,
        family: FirewallFamily,
        table: FirewallTable,
    ) -> Result<Vec<String>, String>;
}

struct SystemFirewallExecutor;

impl FirewallExecutor for SystemFirewallExecutor {
    fn run(&mut self, family: FirewallFamily, args: &[String]) -> Result<(), String> {
        let output = Command::new(family.binary())
            .args(args)
            .output()
            .map_err(|error| format!("执行 {} 失败: {error}", family.binary()))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!(
            "{} 配置失败（状态 {}）{}",
            family.binary(),
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ))
    }

    fn check(&mut self, family: FirewallFamily, args: &[String]) -> Result<bool, String> {
        let output = Command::new(family.binary())
            .args(args)
            .output()
            .map_err(|error| format!("执行 {} 检查失败: {error}", family.binary()))?;
        if output.status.success() {
            return Ok(true);
        }
        if output.status.code() == Some(1) {
            return Ok(false);
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!(
            "{} 检查失败（状态 {}）{}",
            family.binary(),
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ))
    }

    fn list_rules(
        &mut self,
        family: FirewallFamily,
        table: FirewallTable,
    ) -> Result<Vec<String>, String> {
        let args = string_args(&["-w", "5", "-t", table.name(), "-S"]);
        let output = Command::new(family.binary())
            .args(&args)
            .output()
            .map_err(|error| format!("执行 {} 规则枚举失败: {error}", family.binary()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!(
                "{} 规则枚举失败（状态 {}）{}",
                family.binary(),
                output.status,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeFirewallExecutor {
        chains: BTreeSet<(FirewallFamily, FirewallTable, String)>,
        jumps: BTreeSet<(FirewallFamily, FirewallTable, String, String)>,
        fail_ipv6_attach: bool,
        fail_ipv4_rule_listing: bool,
        fail_parent_check_without_chain: bool,
        calls: Vec<(FirewallFamily, Vec<String>)>,
    }

    impl FakeFirewallExecutor {
        fn table(args: &[String]) -> FirewallTable {
            match value_after(args, "-t").map(String::as_str) {
                Some("filter") => FirewallTable::Filter,
                _ => FirewallTable::Nat,
            }
        }

        fn chain(&self, family: FirewallFamily, table: FirewallTable, chain: &str) -> bool {
            self.chains.contains(&(family, table, chain.to_string()))
        }

        fn jump(
            &self,
            family: FirewallFamily,
            table: FirewallTable,
            parent: &str,
            chain: &str,
        ) -> bool {
            self.jumps
                .contains(&(family, table, parent.to_string(), chain.to_string()))
        }
    }

    fn value_after<'a>(args: &'a [String], key: &str) -> Option<&'a String> {
        args.iter()
            .position(|arg| arg == key)
            .and_then(|index| args.get(index + 1))
    }

    impl FirewallExecutor for FakeFirewallExecutor {
        fn run(&mut self, family: FirewallFamily, args: &[String]) -> Result<(), String> {
            self.calls.push((family, args.to_vec()));
            let has = |value: &str| args.iter().any(|arg| arg == value);
            let table = Self::table(args);
            if self.fail_ipv6_attach
                && family == FirewallFamily::Ipv6
                && has("-I")
                && has("PREROUTING")
            {
                self.fail_ipv6_attach = false;
                return Err("simulated IPv6 attach failure".to_string());
            }
            if let Some(chain) = value_after(args, "-N") {
                self.chains.insert((family, table, chain.clone()));
            } else if let (Some(parent), Some(chain)) =
                (value_after(args, "-I"), value_after(args, "-j"))
            {
                self.jumps
                    .insert((family, table, parent.clone(), chain.clone()));
            } else if let (Some(parent), Some(chain)) =
                (value_after(args, "-D"), value_after(args, "-j"))
            {
                self.jumps
                    .remove(&(family, table, parent.clone(), chain.clone()));
            } else if let Some(chain) = value_after(args, "-X") {
                self.chains.remove(&(family, table, chain.clone()));
            }
            Ok(())
        }

        fn check(&mut self, family: FirewallFamily, args: &[String]) -> Result<bool, String> {
            let has = |value: &str| args.iter().any(|arg| arg == value);
            let table = Self::table(args);
            if self.fail_parent_check_without_chain
                && self
                    .chains
                    .iter()
                    .all(|(candidate_family, _, _)| *candidate_family != family)
                && has("-C")
            {
                return Err("simulated iptables-nft missing target error".to_string());
            }
            if let (Some(parent), Some(chain)) = (value_after(args, "-C"), value_after(args, "-j"))
            {
                return Ok(self.jump(family, table, parent, chain));
            }
            Ok(false)
        }

        fn list_rules(
            &mut self,
            family: FirewallFamily,
            table: FirewallTable,
        ) -> Result<Vec<String>, String> {
            if self.fail_ipv4_rule_listing
                && family == FirewallFamily::Ipv4
                && table == FirewallTable::Nat
            {
                return Err("simulated IPv4 rule listing failure".to_string());
            }
            let mut rules = self
                .chains
                .iter()
                .filter(|(candidate_family, candidate_table, _)| {
                    *candidate_family == family && *candidate_table == table
                })
                .map(|(_, _, chain)| format!("-N {chain}"))
                .collect::<Vec<_>>();
            rules.extend(
                self.jumps
                    .iter()
                    .filter(|(candidate_family, candidate_table, _, _)| {
                        *candidate_family == family && *candidate_table == table
                    })
                    .map(|(_, _, parent, chain)| format!("-A {parent} -j {chain}")),
            );
            Ok(rules)
        }
    }

    fn test_local_networks() -> LocalNetworks {
        LocalNetworks {
            ipv4: vec!["10.0.0.0/8".to_string(), "192.168.0.0/16".to_string()],
            ipv6: vec![
                "2409:8a74:5c90:5780::/64".to_string(),
                "fc00::/7".to_string(),
            ],
        }
    }

    #[test]
    fn parses_dynamic_http_port() {
        let parsed = parse_fnos_gateway_settings(
            br#"{"schema":{"http":{"port":19122},"https":{"port":19123}},"force_https":false}"#,
        )
        .expect("valid settings");
        assert_eq!(parsed.schema.http.port, 19122);
        assert!(!parsed.force_https);
    }

    #[test]
    fn rejects_missing_invalid_and_out_of_range_ports() {
        for payload in [
            br#"{}"#.as_slice(),
            br#"{"schema":{"http":{}}}"#.as_slice(),
            br#"{"schema":{"http":{"port":0}}}"#.as_slice(),
            br#"{"schema":{"http":{"port":65536}}}"#.as_slice(),
            b"not-json".as_slice(),
            b"".as_slice(),
        ] {
            assert!(
                parse_fnos_gateway_settings(payload).is_err(),
                "payload unexpectedly accepted: {}",
                String::from_utf8_lossy(payload)
            );
        }
    }

    #[tokio::test]
    async fn rejects_oversized_gateway_settings_before_parsing() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("network_gateway_setting.conf");
        tokio::fs::write(
            &path,
            vec![b' '; FNOS_GATEWAY_SETTINGS_MAX_BYTES as usize + 1],
        )
        .await
        .expect("write oversized settings");
        let error = read_fnos_gateway_settings(&path)
            .await
            .expect_err("oversized settings must be rejected");
        assert!(error.to_string().contains("64 KiB"));
    }

    #[test]
    fn detects_force_https_boundary() {
        let parsed =
            parse_fnos_gateway_settings(br#"{"schema":{"http":{"port":5666}},"force_https":true}"#)
                .expect("valid settings");
        assert!(parsed.force_https);
        assert!(validated_fnos_http_port(&parsed).is_err());
    }

    #[test]
    fn builds_cgroup_scoped_dual_stack_rules_without_user_input() {
        let ipv4 = relay_rule_args(FirewallFamily::Ipv4, "-A", 19122, 45678);
        let ipv6 = relay_rule_args(FirewallFamily::Ipv6, "-A", 19122, 45678);
        let joined4 = ipv4.join(" ");
        let joined6 = ipv6.join(" ");
        assert!(joined4.contains("--path system.slice/trim_connect.service"));
        assert!(joined4.contains("-d 127.0.0.1/32"));
        assert!(joined6.contains("-d ::1/128"));
        assert!(joined4.contains("--dport 19122"));
        assert!(joined4.contains("--to-ports 45678"));
        assert!(!joined4.contains("PREROUTING"));
    }

    #[test]
    fn builds_dual_stack_direct_redirect_and_private_listener_guard() {
        let parent = prerouting_parent_jump_args("-C").join(" ");
        let redirect = direct_redirect_rule_args("-C", 19122, 45678).join(" ");
        let guard = redirected_input_rule_args("-C", 19122, 45678).join(" ");
        let drop = private_listener_drop_rule_args("-C", 45678).join(" ");
        assert!(parent.contains("-t nat -C PREROUTING"));
        assert!(parent.contains("--dst-type LOCAL"));
        assert!(redirect.contains("-C FNK_FNC_PRE"));
        assert!(redirect.contains("--dport 19122"));
        assert!(redirect.contains("--to-ports 45678"));
        assert!(guard.contains("-t filter -C FNK_FNC_IN"));
        assert!(guard.contains("--ctstate DNAT"));
        assert!(guard.contains("--ctorigdstport 19122"));
        assert!(drop.ends_with("-j DROP"));
    }

    #[test]
    fn parent_jumps_are_exact_and_idempotently_addressable() {
        assert_eq!(
            output_parent_jump_args("-C", FNOS_CONNECT_OUTPUT_CHAIN),
            vec![
                "-w",
                "5",
                "-t",
                "nat",
                "-C",
                "OUTPUT",
                "-m",
                "comment",
                "--comment",
                "fn-knock:fn-connect-waf",
                "-j",
                "FNK_FNC_OUT",
            ]
        );
        assert!(
            input_parent_jump_args("-C")
                .windows(2)
                .any(|pair| pair == ["-C", "INPUT"])
        );
        assert!(
            prerouting_parent_jump_args("-C")
                .windows(2)
                .any(|pair| pair == ["-C", "PREROUTING"])
        );
    }

    #[test]
    fn local_network_detection_preserves_private_and_on_link_dual_stack_access() {
        let parsed = parse_local_networks(
            br#"[
                {"addr_info":[
                    {"family":"inet","local":"192.168.31.98","prefixlen":24,"scope":"global"},
                    {"family":"inet6","local":"2409:8a74:5c90:5780:20c:29ff:fe0f:5c24","prefixlen":64,"scope":"global"},
                    {"family":"inet6","local":"2409:8a74:5c90:5780::dce","prefixlen":128,"scope":"global"}
                ]}
            ]"#,
        )
        .expect("parse local networks");
        assert!(parsed.ipv4.iter().any(|cidr| cidr == "10.0.0.0/8"));
        assert!(parsed.ipv4.iter().any(|cidr| cidr == "192.168.31.0/24"));
        assert!(parsed.ipv6.iter().any(|cidr| cidr == "fc00::/7"));
        assert!(
            parsed
                .ipv6
                .iter()
                .any(|cidr| cidr == "2409:8a74:5c90:5780::/64")
        );
        assert!(
            parsed
                .ipv6
                .iter()
                .any(|cidr| cidr == "2409:8a74:5c90:5780::dce/128")
        );
    }

    #[test]
    fn normalizes_only_boolean_enablement() {
        assert_eq!(
            normalize_fnos_connect_waf(Some(&json!({"enabled": true, "updated_at": "now"}))),
            json!({"enabled": true, "updated_at": "now"})
        );
        assert_eq!(
            normalize_fnos_connect_waf(Some(&json!({"enabled": "true"}))),
            json!({"enabled": false, "updated_at": null})
        );
    }

    #[test]
    fn reports_protection_only_for_active_blocking_waf() {
        assert!(fnos_connect_waf_protected(true, Some("blocking")));
        assert!(fnos_connect_waf_protected(true, Some("BLOCKING")));
        assert!(!fnos_connect_waf_protected(true, Some("detection")));
        assert!(!fnos_connect_waf_protected(true, Some("off")));
        assert!(!fnos_connect_waf_protected(false, Some("blocking")));
        assert!(!fnos_connect_waf_protected(true, None));
    }

    #[test]
    fn disabled_runtime_retries_cleanup_after_an_error() {
        assert!(!disabled_fnos_connect_waf_needs_reconcile(
            &json!({"effective": false, "last_error": null})
        ));
        assert!(disabled_fnos_connect_waf_needs_reconcile(
            &json!({"effective": true, "last_error": null})
        ));
        assert!(disabled_fnos_connect_waf_needs_reconcile(
            &json!({"effective": false, "last_error": "cleanup failed"})
        ));
    }

    #[test]
    fn dual_stack_install_rolls_back_every_jump_and_chain_on_second_family_failure() {
        let mut executor = FakeFirewallExecutor {
            fail_ipv6_attach: true,
            ..Default::default()
        };
        let error =
            install_firewall_rules_with(&mut executor, 19122, 45678, &test_local_networks())
                .expect_err("IPv6 attach should fail");
        assert!(error.contains("IPv6 attach failure"));
        assert!(executor.jumps.is_empty());
        assert!(executor.chains.is_empty());
        assert!(executor.calls.iter().any(|(family, args)| {
            *family == FirewallFamily::Ipv4
                && args.iter().any(|arg| arg == "-D")
                && args.iter().any(|arg| arg == "PREROUTING")
        }));
    }

    #[test]
    fn repeated_install_replaces_owned_rules_without_accumulating_jumps() {
        let mut executor = FakeFirewallExecutor::default();
        let networks = test_local_networks();
        install_firewall_rules_with(&mut executor, 5666, 40000, &networks).expect("first install");
        install_firewall_rules_with(&mut executor, 19122, 40000, &networks)
            .expect("second install");
        for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
            assert!(executor.chain(family, FirewallTable::Nat, FNOS_CONNECT_OUTPUT_CHAIN));
            assert!(executor.chain(family, FirewallTable::Nat, FNOS_CONNECT_PREROUTING_CHAIN));
            assert!(executor.chain(family, FirewallTable::Filter, FNOS_CONNECT_INPUT_CHAIN));
            assert!(executor.jump(
                family,
                FirewallTable::Nat,
                "OUTPUT",
                FNOS_CONNECT_OUTPUT_CHAIN
            ));
            assert!(executor.jump(
                family,
                FirewallTable::Nat,
                "PREROUTING",
                FNOS_CONNECT_PREROUTING_CHAIN
            ));
            assert!(executor.jump(
                family,
                FirewallTable::Filter,
                "INPUT",
                FNOS_CONNECT_INPUT_CHAIN
            ));
        }
        let ipv4_deletes = executor
            .calls
            .iter()
            .filter(|(family, args)| {
                *family == FirewallFamily::Ipv4
                    && args.iter().any(|arg| arg == "-D")
                    && args.iter().any(|arg| arg == "OUTPUT")
            })
            .count();
        assert_eq!(ipv4_deletes, 1);
    }

    #[test]
    fn cleanup_does_not_treat_a_firewall_listing_failure_as_rule_absence() {
        let mut executor = FakeFirewallExecutor {
            fail_ipv4_rule_listing: true,
            ..Default::default()
        };
        executor.chains.insert((
            FirewallFamily::Ipv4,
            FirewallTable::Nat,
            FNOS_CONNECT_OUTPUT_CHAIN.to_string(),
        ));
        executor.jumps.insert((
            FirewallFamily::Ipv4,
            FirewallTable::Nat,
            "OUTPUT".to_string(),
            FNOS_CONNECT_OUTPUT_CHAIN.to_string(),
        ));
        let error = cleanup_firewall_rules_with(&mut executor)
            .expect_err("indeterminate firewall state must keep the ingress alive");
        assert!(error.contains("IPv4 rule listing failure"));
        assert!(executor.jump(
            FirewallFamily::Ipv4,
            FirewallTable::Nat,
            "OUTPUT",
            FNOS_CONNECT_OUTPUT_CHAIN
        ));
        assert!(executor.chain(
            FirewallFamily::Ipv4,
            FirewallTable::Nat,
            FNOS_CONNECT_OUTPUT_CHAIN
        ));
    }

    #[test]
    fn clean_state_does_not_check_a_missing_iptables_nft_target() {
        let mut executor = FakeFirewallExecutor {
            fail_parent_check_without_chain: true,
            ..Default::default()
        };
        cleanup_firewall_rules_with(&mut executor)
            .expect("a missing custom chain is already clean");
    }
}
