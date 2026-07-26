use std::{
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
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{fs::File, io::AsyncReadExt, net::TcpStream, time};

use super::*;

const FNOS_GATEWAY_SETTINGS_PATH: &str = "/usr/trim/etc/network_gateway_setting.conf";
const FNOS_GATEWAY_SETTINGS_MAX_BYTES: u64 = 64 * 1024;
const FNOS_CONNECT_CGROUP_PATH: &str = "system.slice/trim_connect.service";
const FNOS_CONNECT_CHAIN: &str = "FNK_FNC_WAF";
const FNOS_CONNECT_RULE_COMMENT: &str = "fn-knock:fn-connect-waf";
const FNOS_CONNECT_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const FNOS_CONNECT_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    if let Err(error) = install_firewall_rules(detected.port, listener_port).await {
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
    let healthy_firewall = if let Some(listener_port) = listener_port {
        firewall_rules_active(detected.port, listener_port).await
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

fn firewall_rule_args(family: FirewallFamily, source_port: u16, target_port: u16) -> Vec<String> {
    firewall_rule_args_with_action(family, "-A", source_port, target_port)
}

fn firewall_rule_args_with_action(
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
        FNOS_CONNECT_CHAIN,
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

fn parent_jump_args(action: &str) -> Vec<String> {
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
        FNOS_CONNECT_CHAIN,
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

async fn install_firewall_rules(source_port: u16, target_port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || install_firewall_rules_blocking(source_port, target_port))
        .await
        .map_err(|error| format!("等待防火墙配置任务失败: {error}"))?
}

async fn cleanup_firewall_rules() -> Result<(), String> {
    tokio::task::spawn_blocking(cleanup_firewall_rules_blocking)
        .await
        .map_err(|error| format!("等待防火墙清理任务失败: {error}"))?
}

async fn firewall_rules_active(source_port: u16, target_port: u16) -> bool {
    tokio::task::spawn_blocking(move || {
        let mut executor = SystemFirewallExecutor;
        [FirewallFamily::Ipv4, FirewallFamily::Ipv6]
            .into_iter()
            .all(|family| {
                executor
                    .check(family, &parent_jump_args("-C"))
                    .unwrap_or(false)
                    && executor
                        .check(
                            family,
                            &firewall_rule_args_with_action(family, "-C", source_port, target_port),
                        )
                        .unwrap_or(false)
            })
    })
    .await
    .unwrap_or(false)
}

fn install_firewall_rules_blocking(source_port: u16, target_port: u16) -> Result<(), String> {
    install_firewall_rules_with(&mut SystemFirewallExecutor, source_port, target_port)
}

fn install_firewall_rules_with(
    executor: &mut impl FirewallExecutor,
    source_port: u16,
    target_port: u16,
) -> Result<(), String> {
    cleanup_firewall_rules_with(executor)?;
    let result = (|| {
        for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
            executor.run(
                family,
                &string_args(&["-w", "5", "-t", "nat", "-N", FNOS_CONNECT_CHAIN]),
            )?;
            executor.run(
                family,
                &firewall_rule_args(family, source_port, target_port),
            )?;
        }
        for family in [FirewallFamily::Ipv4, FirewallFamily::Ipv6] {
            executor.run(family, &parent_jump_args("-I"))?;
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
        let rules = match executor.list_nat_rules(family) {
            Ok(rules) => rules,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let chain_rule = format!("-N {FNOS_CONNECT_CHAIN}");
        if !rules.iter().any(|rule| rule == &chain_rule) {
            // A jump cannot reference a non-existent user chain. Avoid `-C`
            // here because iptables-nft reports that ordinary clean state as
            // exit status 2 instead of "rule absent".
            continue;
        }
        let mut parent_limit_reached = true;
        for _ in 0..8 {
            match executor.check(family, &parent_jump_args("-C")) {
                Ok(true) => {}
                Ok(false) => {
                    parent_limit_reached = false;
                    break;
                }
                Err(error) => {
                    errors.push(error);
                    parent_limit_reached = false;
                    break;
                }
            }
            if let Err(error) = executor.run(family, &parent_jump_args("-D")) {
                errors.push(error);
                parent_limit_reached = false;
                break;
            }
        }
        if parent_limit_reached {
            match executor.check(family, &parent_jump_args("-C")) {
                Ok(true) => errors.push(format!(
                    "{} 中存在超过 8 条 FN Connect WAF OUTPUT 跳转",
                    family.binary()
                )),
                Ok(false) => {}
                Err(error) => errors.push(error),
            }
        }
        if let Err(error) = executor.run(
            family,
            &string_args(&["-w", "5", "-t", "nat", "-F", FNOS_CONNECT_CHAIN]),
        ) {
            errors.push(error);
        }
        if let Err(error) = executor.run(
            family,
            &string_args(&["-w", "5", "-t", "nat", "-X", FNOS_CONNECT_CHAIN]),
        ) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn string_args(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

trait FirewallExecutor {
    fn run(&mut self, family: FirewallFamily, args: &[String]) -> Result<(), String>;
    fn check(&mut self, family: FirewallFamily, args: &[String]) -> Result<bool, String>;
    fn list_nat_rules(&mut self, family: FirewallFamily) -> Result<Vec<String>, String>;
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

    fn list_nat_rules(&mut self, family: FirewallFamily) -> Result<Vec<String>, String> {
        let args = string_args(&["-w", "5", "-t", "nat", "-S"]);
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
        ipv4_chain: bool,
        ipv6_chain: bool,
        ipv4_jump: bool,
        ipv6_jump: bool,
        fail_ipv6_attach: bool,
        fail_ipv4_rule_listing: bool,
        fail_parent_check_without_chain: bool,
        calls: Vec<(FirewallFamily, Vec<String>)>,
    }

    impl FakeFirewallExecutor {
        fn chain(&self, family: FirewallFamily) -> bool {
            match family {
                FirewallFamily::Ipv4 => self.ipv4_chain,
                FirewallFamily::Ipv6 => self.ipv6_chain,
            }
        }

        fn jump(&self, family: FirewallFamily) -> bool {
            match family {
                FirewallFamily::Ipv4 => self.ipv4_jump,
                FirewallFamily::Ipv6 => self.ipv6_jump,
            }
        }

        fn set_chain(&mut self, family: FirewallFamily, value: bool) {
            match family {
                FirewallFamily::Ipv4 => self.ipv4_chain = value,
                FirewallFamily::Ipv6 => self.ipv6_chain = value,
            }
        }

        fn set_jump(&mut self, family: FirewallFamily, value: bool) {
            match family {
                FirewallFamily::Ipv4 => self.ipv4_jump = value,
                FirewallFamily::Ipv6 => self.ipv6_jump = value,
            }
        }
    }

    impl FirewallExecutor for FakeFirewallExecutor {
        fn run(&mut self, family: FirewallFamily, args: &[String]) -> Result<(), String> {
            self.calls.push((family, args.to_vec()));
            let has = |value: &str| args.iter().any(|arg| arg == value);
            if self.fail_ipv6_attach && family == FirewallFamily::Ipv6 && has("-I") && has("OUTPUT")
            {
                self.fail_ipv6_attach = false;
                return Err("simulated IPv6 attach failure".to_string());
            }
            if has("-N") {
                self.set_chain(family, true);
            } else if has("-I") && has("OUTPUT") {
                self.set_jump(family, true);
            } else if has("-D") && has("OUTPUT") {
                self.set_jump(family, false);
            } else if has("-X") {
                self.set_chain(family, false);
            }
            Ok(())
        }

        fn check(&mut self, family: FirewallFamily, args: &[String]) -> Result<bool, String> {
            let has = |value: &str| args.iter().any(|arg| arg == value);
            if self.fail_parent_check_without_chain
                && !self.chain(family)
                && has("-C")
                && has("OUTPUT")
            {
                return Err("simulated iptables-nft missing target error".to_string());
            }
            if has("-S") {
                return Ok(self.chain(family));
            }
            if has("-C") && has("OUTPUT") {
                return Ok(self.jump(family));
            }
            if has("-C") && has(FNOS_CONNECT_CHAIN) {
                return Ok(self.chain(family));
            }
            Ok(false)
        }

        fn list_nat_rules(&mut self, family: FirewallFamily) -> Result<Vec<String>, String> {
            if self.fail_ipv4_rule_listing && family == FirewallFamily::Ipv4 {
                return Err("simulated IPv4 rule listing failure".to_string());
            }
            let mut rules = Vec::new();
            if self.chain(family) {
                rules.push(format!("-N {FNOS_CONNECT_CHAIN}"));
            }
            if self.jump(family) {
                rules.push(format!(
                    "-A OUTPUT -m comment --comment \"{FNOS_CONNECT_RULE_COMMENT}\" -j {FNOS_CONNECT_CHAIN}"
                ));
            }
            Ok(rules)
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
        let ipv4 = firewall_rule_args(FirewallFamily::Ipv4, 19122, 45678);
        let ipv6 = firewall_rule_args(FirewallFamily::Ipv6, 19122, 45678);
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
    fn parent_jump_is_exact_and_idempotently_addressable() {
        assert_eq!(
            parent_jump_args("-C"),
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
                "FNK_FNC_WAF",
            ]
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
        let error = install_firewall_rules_with(&mut executor, 19122, 45678)
            .expect_err("IPv6 attach should fail");
        assert!(error.contains("IPv6 attach failure"));
        assert!(!executor.ipv4_jump);
        assert!(!executor.ipv6_jump);
        assert!(!executor.ipv4_chain);
        assert!(!executor.ipv6_chain);
        assert!(executor.calls.iter().any(|(family, args)| {
            *family == FirewallFamily::Ipv4
                && args.iter().any(|arg| arg == "-D")
                && args.iter().any(|arg| arg == "OUTPUT")
        }));
    }

    #[test]
    fn repeated_install_replaces_owned_rules_without_accumulating_jumps() {
        let mut executor = FakeFirewallExecutor::default();
        install_firewall_rules_with(&mut executor, 5666, 40000).expect("first install");
        install_firewall_rules_with(&mut executor, 19122, 40000).expect("second install");
        assert!(executor.ipv4_chain && executor.ipv6_chain);
        assert!(executor.ipv4_jump && executor.ipv6_jump);
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
            ipv4_chain: true,
            ipv4_jump: true,
            fail_ipv4_rule_listing: true,
            ..Default::default()
        };
        let error = cleanup_firewall_rules_with(&mut executor)
            .expect_err("indeterminate firewall state must keep the ingress alive");
        assert!(error.contains("IPv4 rule listing failure"));
        assert!(executor.ipv4_jump);
        assert!(executor.ipv4_chain);
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
