use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tonic::{
    Request,
    metadata::MetadataValue,
    transport::{Channel, Endpoint},
};
use tonic_health::pb::{
    HealthCheckRequest, health_check_response::ServingStatus, health_client::HealthClient,
};

use crate::app_version::APP_LOCAL_VERSION;
use crate::grpc_proto::{
    AdvancedAuthCondition, AdvancedAuthConfig, AdvancedAuthGroup, AuthConfig, BasicAuthConfig,
    BoolValue, CommonLocationExemptionsRuntime, ControlApiVersion, CrawlerBlockerConfig,
    FnosConnectIngressConfig, FnosConnectIngressStatus, FnosPortIconHijackConfig,
    GatewayListenerConfig, GatewayMemoryConfig, GatewayPortalConfig,
    GatewayTrustedClientIpsRuntime, GatewayUnmatchedRouteConfig, GatewayVisibilityConfig,
    HostActiveIpStats, HostLocation, HostLocationResponse, HostRule, HostRuleAvailability,
    HostRuleVisibility, HostRules, LocaleConfig, LoggingConfig, OmitTargetsConfig,
    ReverseProxyThrottleConfig, ReverseProxyThrottleExemptIpsRuntime, Rule, Rules, SslConfig,
    SslDeployedCertificate, StreamAvailability, StreamRule, StreamRules, StringValue, WafConfig,
    deep_monitor_service_client::DeepMonitorServiceClient,
    firewall_service_client::FirewallServiceClient,
    gateway_control_service_client::GatewayControlServiceClient,
    gateway_logs_service_client::GatewayLogsServiceClient,
    security_service_client::SecurityServiceClient, ssl_service_client::SslServiceClient,
    traffic_service_client::TrafficServiceClient, waf_service_client::WafServiceClient,
};

mod ack;
mod compiled_ipset;
pub(crate) mod deep_monitor;
mod firewall;
mod gateway_logs;
mod general_blacklist;
mod runtime_services;

pub(crate) use ack::{
    applied_response_data, applied_response_object, ensure_response_success, response_message,
};
use compiled_ipset::{
    compiled_ip_set_to_json, parse_compiled_ip_sets, parse_optional_compiled_ip_set,
};

const INTERNAL_TOKEN_METADATA_KEY: &str = "x-fn-knock-internal-rpc-token";
const INTERNAL_GRPC_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
pub(crate) const GATEWAY_CONTROL_API_VERSION: u64 = ControlApiVersion::Current as u64;
pub(crate) const GATEWAY_HEALTH_PROCESS: &str = "fnknock.gateway.process";
pub(crate) const GATEWAY_HEALTH_DATAPLANE: &str = "fnknock.gateway.dataplane";
pub(crate) const GATEWAY_HEALTH_AUTH_BRIDGE: &str = "fnknock.gateway.auth_bridge";

#[derive(Debug, thiserror::Error)]
pub(crate) enum BundleCompatibilityError {
    #[error("gateway compatibility probe failed: {0:#}")]
    Unavailable(#[source] anyhow::Error),
    #[error("{0}")]
    Incompatible(String),
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct GoBackendClient {
    control: GatewayControlServiceClient<Channel>,
    logs: GatewayLogsServiceClient<Channel>,
    deep_monitor: DeepMonitorServiceClient<Channel>,
    security: SecurityServiceClient<Channel>,
    traffic: TrafficServiceClient<Channel>,
    waf: WafServiceClient<Channel>,
    ssl: SslServiceClient<Channel>,
    firewall: FirewallServiceClient<Channel>,
    health: HealthClient<Channel>,
    timeout: Duration,
    token: MetadataValue<tonic::metadata::Ascii>,
}

#[allow(dead_code)]
impl GoBackendClient {
    pub fn new(addr: String, token: String, timeout: Duration) -> anyhow::Result<Self> {
        let token = token.trim();
        if token.is_empty() {
            anyhow::bail!("FN_KNOCK_INTERNAL_RPC_TOKEN must be set for Go gRPC backend");
        }
        let token = MetadataValue::try_from(token)
            .context("encode FN_KNOCK_INTERNAL_RPC_TOKEN metadata")?;
        let endpoint = Endpoint::from_shared(format!("http://{}", normalize_grpc_addr(&addr)))
            .with_context(|| format!("invalid GO_BACKEND_GRPC_ADDR: {addr}"))?
            .timeout(timeout)
            .connect_timeout(timeout);
        let channel = endpoint.connect_lazy();
        Ok(Self {
            control: GatewayControlServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            logs: GatewayLogsServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            deep_monitor: DeepMonitorServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            security: SecurityServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            traffic: TrafficServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            waf: WafServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            ssl: SslServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            firewall: FirewallServiceClient::new(channel.clone())
                .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
                .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE),
            health: HealthClient::new(channel),
            timeout,
            token,
        })
    }

    fn request<T>(&self, message: T) -> Request<T> {
        let mut request = Request::new(message);
        request.set_timeout(self.timeout);
        request
            .metadata_mut()
            .insert(INTERNAL_TOKEN_METADATA_KEY, self.token.clone());
        request
    }

    pub async fn get_server_info(&self) -> anyhow::Result<Value> {
        status_value("get_server_info", self.get_server_info_status().await?)
    }

    pub async fn get_runtime_info(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let info = client
            .get_runtime_info(self.request(()))
            .await
            .context("get Go gateway runtime info")?
            .into_inner();
        Ok(json!({
            "instance_id": info.instance_id,
            "pid": info.pid,
            "started_at_unix_ms": info.started_at_unix_ms,
            "uptime_ms": info.uptime_ms,
            "go_version": info.go_version,
            "goroutines": info.goroutines,
            "heap_alloc_bytes": info.heap_alloc_bytes,
            "heap_sys_bytes": info.heap_sys_bytes,
            "rss_bytes": info.rss_bytes,
            "gc_percent": info.gc_percent,
            "memory_limit_bytes": info.memory_limit_bytes,
            "num_gc": info.num_gc,
            "managed_memory_bytes": info.managed_memory_bytes,
            "active_proxy_requests": info.active_proxy_requests,
            "active_client_connections": info.active_client_connections,
            "idle_client_connections": info.idle_client_connections,
            "open_upstream_connections": info.open_upstream_connections,
            "udp_sessions": info.udp_sessions,
            "udp_queued_bytes": info.udp_queued_bytes,
            "udp_queued_bytes_peak": info.udp_queued_bytes_peak,
            "udp_queue_drops": info.udp_queue_drops,
        }))
    }

    pub async fn set_gateway_memory_config(
        &self,
        gc_percent: i32,
        memory_limit_bytes: i64,
    ) -> anyhow::Result<(i32, i64)> {
        let mut client = self.control.clone();
        let response = client
            .set_gateway_memory_config(self.request(GatewayMemoryConfig {
                gc_percent,
                memory_limit_bytes,
            }))
            .await
            .context("set Go gateway memory config")?
            .into_inner();
        Ok((response.gc_percent, response.memory_limit_bytes))
    }

    pub async fn reclaim_gateway_memory(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let info = client
            .reclaim_gateway_memory(self.request(()))
            .await
            .context("reclaim Go gateway memory")?
            .into_inner();
        Ok(json!({
            "heap_alloc_bytes": info.heap_alloc_bytes,
            "heap_sys_bytes": info.heap_sys_bytes,
            "rss_bytes": info.rss_bytes,
            "gc_percent": info.gc_percent,
            "memory_limit_bytes": info.memory_limit_bytes,
            "num_gc": info.num_gc,
            "managed_memory_bytes": info.managed_memory_bytes,
            "active_proxy_requests": info.active_proxy_requests,
            "active_client_connections": info.active_client_connections,
            "idle_client_connections": info.idle_client_connections,
            "open_upstream_connections": info.open_upstream_connections,
            "udp_sessions": info.udp_sessions,
            "udp_queued_bytes": info.udp_queued_bytes,
            "udp_queued_bytes_peak": info.udp_queued_bytes_peak,
            "udp_queue_drops": info.udp_queue_drops,
        }))
    }

    pub async fn verify_bundle_compatibility(&self) -> Result<Value, BundleCompatibilityError> {
        let response = self
            .get_server_info()
            .await
            .map_err(BundleCompatibilityError::Unavailable)?;
        let Some(info) = response.get("data") else {
            return Err(BundleCompatibilityError::Incompatible(
                "Go gateway server info response is missing data".to_string(),
            ));
        };
        let version = info.get("version").and_then(Value::as_str).unwrap_or("");
        if version != APP_LOCAL_VERSION {
            return Err(BundleCompatibilityError::Incompatible(format!(
                "bundle version mismatch: Rust={APP_LOCAL_VERSION}, Go={version}"
            )));
        }
        let control_api_version = info
            .get("control_api_version")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        if control_api_version != GATEWAY_CONTROL_API_VERSION {
            return Err(BundleCompatibilityError::Incompatible(format!(
                "control API mismatch: Rust={}, Go={control_api_version}",
                GATEWAY_CONTROL_API_VERSION
            )));
        }
        let Some(capabilities) = info.get("capabilities").and_then(Value::as_array) else {
            return Err(BundleCompatibilityError::Incompatible(
                "Go gateway server info is missing capabilities".to_string(),
            ));
        };
        for required in [
            "http",
            "https",
            "http2",
            "websocket",
            "tcp",
            "udp",
            "waf",
            "blacklist",
            "logs",
            "deep_monitor_v1",
            "lifecycle",
            "runtime_info_v1",
            "memory_control_v1",
            "host_rule_groups_v1",
            "compiled_visibility_ipset_v1",
            "trusted_client_ip_bypass_v1",
            "compiled_ipset_v2",
            "compiled_whitelist_firewall_v1",
            "compiled_trusted_client_ipset_v1",
        ] {
            if !capabilities
                .iter()
                .any(|value| value.as_str() == Some(required))
            {
                return Err(BundleCompatibilityError::Incompatible(format!(
                    "Go gateway is missing required capability {required}"
                )));
            }
        }
        let local_commit = option_env!("FN_KNOCK_GATEWAY_COMMIT").unwrap_or("").trim();
        let gateway_commit = info
            .get("commit")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if cfg!(any(target_os = "windows", target_os = "macos"))
            && (local_commit.is_empty() || gateway_commit.is_empty())
        {
            return Err(BundleCompatibilityError::Incompatible(
                "release gateway source commit metadata is missing".to_string(),
            ));
        }
        if !local_commit.is_empty() && !gateway_commit.is_empty() && local_commit != gateway_commit
        {
            return Err(BundleCompatibilityError::Incompatible(format!(
                "gateway source commit mismatch: expected={local_commit}, reported={gateway_commit}"
            )));
        }
        if cfg!(any(target_os = "windows", target_os = "macos")) {
            let gateway_os = info.get("os").and_then(Value::as_str).unwrap_or("");
            let gateway_arch = info.get("arch").and_then(Value::as_str).unwrap_or("");
            verify_gateway_platform(
                std::env::consts::OS,
                std::env::consts::ARCH,
                gateway_os,
                gateway_arch,
                capabilities,
            )?;
        }
        Ok(response)
    }

    async fn get_server_info_status(&self) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client.get_server_info(self.request(())).await {
            Ok(response) => {
                let info = response.into_inner();
                Ok(ok(json!({
                    "version": info.version,
                    "os": info.os,
                    "arch": info.arch,
                    "control_api_version": info.control_api_version,
                    "capabilities": info.capabilities,
                    "commit": info.commit,
                })))
            }
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn health_serving(&self, service: &str) -> anyhow::Result<bool> {
        let mut client = self.health.clone();
        let response = client
            .check(self.request(HealthCheckRequest {
                service: service.to_string(),
            }))
            .await
            .with_context(|| format!("check Go gRPC health service {service}"))?
            .into_inner();
        Ok(response.status == ServingStatus::Serving as i32)
    }

    pub async fn request_shutdown(&self) -> anyhow::Result<()> {
        let mut client = self.control.clone();
        let response = client
            .request_shutdown(self.request(()))
            .await
            .context("request graceful Go gateway shutdown")?
            .into_inner();
        if response.success {
            Ok(())
        } else {
            anyhow::bail!(
                "Go gateway rejected shutdown request: {}",
                response.message.trim()
            )
        }
    }

    pub async fn reset_all_data(&self) -> anyhow::Result<()> {
        let mut client = self.control.clone();
        let response = client
            .reset_all_data(self.request(()))
            .await
            .context("reset all Go gateway data")?
            .into_inner();
        if response.success {
            Ok(())
        } else {
            anyhow::bail!(
                "Go gateway rejected data reset request: {}",
                response.message.trim()
            )
        }
    }

    pub async fn set_gateway_listener_scope(&self, scope: &str) -> anyhow::Result<String> {
        let mut client = self.control.clone();
        let response = client
            .set_gateway_listener_config(self.request(GatewayListenerConfig {
                scope: scope.to_string(),
            }))
            .await
            .context("set Go gateway listener scope")?
            .into_inner();
        Ok(response.scope)
    }

    pub async fn get_gateway_listener_scope(&self) -> anyhow::Result<String> {
        let mut client = self.control.clone();
        let response = client
            .get_gateway_listener_config(self.request(()))
            .await
            .context("get Go gateway listener scope")?
            .into_inner();
        Ok(response.scope)
    }

    pub async fn set_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_rules(self.request(Rules {
                items: parse_rules(rules),
            }))
            .await
        {
            Ok(response) => ok(rules_to_json(response.into_inner().items)),
            Err(error) => grpc_error(error),
        };
        status_value("set_rules", result)
    }

    pub async fn set_host_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_host_rules(self.request(HostRules {
                items: parse_host_rules(rules.get("items").unwrap_or(rules)),
                visibility_policies: parse_compiled_ip_sets(rules.get("visibility_policies"))?,
            }))
            .await
        {
            Ok(response) => ok(host_rules_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_host_rules", result)
    }

    pub async fn set_stream_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_stream_rules(self.request(StreamRules {
                items: parse_stream_rules(rules.get("items").unwrap_or(rules)),
                availability: parse_stream_availability(rules.get("availability")),
            }))
            .await
        {
            Ok(response) => ok(stream_rules_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_stream_rules", result)
    }

    pub async fn flush_rules(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client.flush_rules(self.request(())).await {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("flush_rules", result)
    }

    pub async fn flush_host_rules(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client.flush_host_rules(self.request(())).await {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("flush_host_rules", result)
    }

    pub async fn flush_stream_rules(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client.flush_stream_rules(self.request(())).await {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("flush_stream_rules", result)
    }

    pub async fn set_auth_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_auth_config(self.request(parse_auth_config(config)))
            .await
        {
            Ok(response) => rpc_status_response(response.into_inner()),
            Err(error) => grpc_error(error),
        };
        status_value("set_auth_config", result)
    }

    pub async fn get_proxy_protocol_force(&self) -> anyhow::Result<Value> {
        status_value(
            "get_proxy_protocol_force",
            self.get_proxy_protocol_force_status().await?,
        )
    }

    async fn get_proxy_protocol_force_status(&self) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client.get_proxy_protocol_force(self.request(())).await {
            Ok(response) => Ok(ok(json!({
                "proxy_protocol_force": response.into_inner().value
            }))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn set_proxy_protocol_force(&self, force: bool) -> anyhow::Result<Value> {
        status_value(
            "set_proxy_protocol_force",
            self.set_proxy_protocol_force_status(force).await?,
        )
    }

    async fn set_proxy_protocol_force_status(
        &self,
        force: bool,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client
            .set_proxy_protocol_force(self.request(BoolValue { value: force }))
            .await
        {
            Ok(response) => Ok(ok(json!({
                "proxy_protocol_force": response.into_inner().value
            }))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn set_locale_config(&self, config: &Value) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client
            .set_locale_config(self.request(LocaleConfig {
                default_locale: string_field(config, "default_locale"),
            }))
            .await
        {
            Ok(response) => Ok(ok(locale_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn set_default_route(&self, route: &str) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client
            .set_default_route(self.request(StringValue {
                value: route.to_string(),
            }))
            .await
        {
            Ok(response) => Ok(rpc_status_response(response.into_inner())),
            Err(error) => Ok(grpc_error(error)),
        }
    }

    pub async fn set_reverse_proxy_throttle(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_reverse_proxy_throttle(self.request(parse_throttle(config)))
            .await
        {
            Ok(response) => ok(throttle_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_reverse_proxy_throttle", result)
    }

    pub async fn set_gateway_visibility(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_gateway_visibility(self.request(parse_visibility(config)?))
            .await
        {
            Ok(response) => ok(visibility_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_gateway_visibility", result)
    }

    pub async fn set_forwarded_headers_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_forwarded_headers_config(self.request(parse_omit_targets(config)))
            .await
        {
            Ok(response) => ok(omit_targets_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_forwarded_headers_config", result)
    }

    pub async fn set_preserve_host_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_preserve_host_config(self.request(parse_omit_targets(config)))
            .await
        {
            Ok(response) => ok(omit_targets_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_preserve_host_config", result)
    }

    pub async fn set_crawler_blocker_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_crawler_blocker_config(self.request(parse_crawler_blocker(config)))
            .await
        {
            Ok(response) => ok(crawler_blocker_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_crawler_blocker_config", result)
    }

    pub async fn set_gateway_portal_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_gateway_portal_config(self.request(parse_portal(config)))
            .await
        {
            Ok(response) => ok(portal_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_gateway_portal_config", result)
    }

    pub async fn set_gateway_unmatched_route_config(
        &self,
        config: &Value,
    ) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_gateway_unmatched_route_config(self.request(parse_gateway_unmatched_route(config)))
            .await
        {
            Ok(response) => ok(gateway_unmatched_route_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_gateway_unmatched_route_config", result)
    }

    pub async fn set_fnos_port_icon_hijack_config(&self, config: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_fnos_port_icon_hijack_config(self.request(parse_fnos_port_icon_hijack(config)))
            .await
        {
            Ok(response) => ok(fnos_port_icon_hijack_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_fnos_port_icon_hijack_config", result)
    }

    pub async fn get_fnos_connect_ingress_status(&self) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .get_fnos_connect_ingress_status(self.request(()))
            .await
        {
            Ok(response) => ok(fnos_connect_ingress_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("get_fnos_connect_ingress_status", result)
    }

    pub async fn set_fnos_connect_ingress_config(
        &self,
        enabled: bool,
        upstream_http_port: u16,
    ) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_fnos_connect_ingress_config(self.request(FnosConnectIngressConfig {
                enabled,
                upstream_http_port: i32::from(upstream_http_port),
            }))
            .await
        {
            Ok(response) => ok(fnos_connect_ingress_status_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_fnos_connect_ingress_config", result)
    }

    pub async fn set_reverse_proxy_throttle_exempt_ips(
        &self,
        runtime: &Value,
    ) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_reverse_proxy_throttle_exempt_ips(self.request(parse_throttle_exempt(runtime)?))
            .await
        {
            Ok(response) => ok(throttle_exempt_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_reverse_proxy_throttle_exempt_ips", result)
    }

    pub async fn set_gateway_trusted_client_ips(&self, runtime: &Value) -> anyhow::Result<Value> {
        let mut client = self.control.clone();
        let result = match client
            .set_gateway_trusted_client_ips(
                self.request(parse_gateway_trusted_client_ips(runtime)?),
            )
            .await
        {
            Ok(response) => ok(gateway_trusted_client_ips_to_json(response.into_inner())),
            Err(error) => grpc_error(error),
        };
        status_value("set_gateway_trusted_client_ips", result)
    }

    pub async fn set_common_location_exemptions(
        &self,
        runtime: &Value,
    ) -> anyhow::Result<(StatusCode, Value)> {
        let mut client = self.control.clone();
        match client
            .set_common_location_exemptions(self.request(parse_common_exemptions(runtime)?))
            .await
        {
            Ok(response) => Ok(ok(common_exemptions_to_json(response.into_inner()))),
            Err(error) => Ok(grpc_error(error)),
        }
    }
}

fn verify_gateway_platform(
    expected_os: &str,
    expected_arch: &str,
    gateway_os: &str,
    gateway_arch: &str,
    capabilities: &[Value],
) -> Result<(), BundleCompatibilityError> {
    let os_matches = match expected_os {
        "macos" => gateway_os == "darwin",
        other => gateway_os == other,
    };
    let arch_matches = match expected_arch {
        "x86_64" => matches!(gateway_arch, "amd64" | "x86_64"),
        "aarch64" => matches!(gateway_arch, "arm64" | "aarch64"),
        other => gateway_arch == other,
    };
    if !os_matches || !arch_matches {
        return Err(BundleCompatibilityError::Incompatible(format!(
            "gateway platform mismatch: expected {expected_os}/{expected_arch}, got {gateway_os}/{gateway_arch}"
        )));
    }
    if capabilities.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|capability| capability.starts_with("firewall."))
    }) {
        return Err(BundleCompatibilityError::Incompatible(format!(
            "{expected_os} gateway must not advertise host firewall capabilities"
        )));
    }
    Ok(())
}

fn normalize_grpc_addr(addr: &str) -> String {
    addr.trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string()
}

fn ok(data: Value) -> (StatusCode, Value) {
    (StatusCode::OK, success_envelope(data))
}

fn status_value(operation: &str, result: (StatusCode, Value)) -> anyhow::Result<Value> {
    let (status, value) = result;
    if !status.is_success() {
        anyhow::bail!("go backend gRPC request failed: {operation} returned {status}: {value}");
    }
    Ok(value)
}

fn success_envelope(data: Value) -> Value {
    envelope(true, 200, "success", data)
}

fn envelope(success: bool, code: u16, message: &str, data: Value) -> Value {
    json!({
        "success": success,
        "code": code,
        "message": if message.trim().is_empty() { if success { "success" } else { "error" } } else { message },
        "data": data
    })
}

fn rpc_status_response(status: crate::grpc_proto::RpcStatus) -> (StatusCode, Value) {
    let code = if status.code > 0 {
        status.code as u16
    } else if status.success {
        200
    } else {
        500
    };
    let http_status = StatusCode::from_u16(code).unwrap_or(if status.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    });
    (
        http_status,
        envelope(
            status.success,
            http_status.as_u16(),
            &status.message,
            Value::Null,
        ),
    )
}

fn grpc_error(error: tonic::Status) -> (StatusCode, Value) {
    let status = match error.code() {
        tonic::Code::InvalidArgument => StatusCode::BAD_REQUEST,
        tonic::Code::Unauthenticated => StatusCode::UNAUTHORIZED,
        tonic::Code::PermissionDenied => StatusCode::FORBIDDEN,
        tonic::Code::NotFound => StatusCode::NOT_FOUND,
        tonic::Code::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
        tonic::Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_GATEWAY,
    };
    (
        status,
        envelope(false, status.as_u16(), error.message(), Value::Null),
    )
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn bool_field(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn i32_field(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(default)
}

fn string_vec_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_vec_any_field(value: &Value, key: &str) -> Vec<String> {
    match value.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .or_else(|| item.as_i64().map(|value| value.to_string()))
            })
            .flat_map(|value| split_csv(&value))
            .collect(),
        Some(Value::String(value)) => split_csv(value),
        Some(Value::Number(value)) => vec![value.to_string()],
        _ => Vec::new(),
    }
}

fn int_vec_any_field(value: &Value, key: &str) -> Vec<i32> {
    string_vec_any_field(value, key)
        .into_iter()
        .filter_map(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .collect()
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn parent_chains_from_body(value: &Value) -> Vec<String> {
    let from_parent_chain = string_vec_any_field(value, "parent_chain");
    if from_parent_chain.is_empty() {
        string_vec_any_field(value, "parent_chains")
    } else {
        from_parent_chain
    }
}

fn parse_rules(value: &Value) -> Vec<Rule> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| Rule {
                    path: string_field(item, "path"),
                    target: string_field(item, "target"),
                    use_auth: bool_field(item, "use_auth", false),
                    strip_path: bool_field(item, "strip_path", true),
                    rewrite_html: bool_field(item, "rewrite_html", true),
                    use_root_mode: bool_field(item, "use_root_mode", false),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_basic_auth(value: &Value) -> BasicAuthConfig {
    BasicAuthConfig {
        enabled: bool_field(value, "enabled", false),
        username: string_field(value, "username"),
        password: string_field(value, "password"),
    }
}

fn parse_host_location_response(value: &Value) -> Option<HostLocationResponse> {
    if !value.is_object() {
        return None;
    }
    let headers = value
        .get("headers")
        .and_then(Value::as_object)
        .map(|headers| {
            headers
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    Some(HostLocationResponse {
        status: i32_field(value, "status", 0),
        content_type: string_field(value, "content_type"),
        headers,
        body: string_field(value, "body"),
    })
}

fn parse_host_locations(value: &Value) -> Vec<HostLocation> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| HostLocation {
                    path: string_field(item, "path"),
                    r#match: string_field(item, "match"),
                    action: string_field(item, "action"),
                    target: string_field(item, "target"),
                    strip_path: bool_field(item, "strip_path", true),
                    rewrite_html: bool_field(item, "rewrite_html", true),
                    response: item.get("response").and_then(parse_host_location_response),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_host_rule_availability(value: &Value) -> Option<HostRuleAvailability> {
    if !value.is_object() {
        return None;
    }
    if !bool_field(value, "enabled", false) {
        return None;
    }
    Some(HostRuleAvailability {
        enabled: true,
        start_time: string_field(value, "start_time"),
        end_time: string_field(value, "end_time"),
    })
}

#[allow(deprecated)] // Read legacy expanded CIDRs while upgrading old snapshots.
fn parse_host_rule_visibility(value: Option<&Value>) -> Option<HostRuleVisibility> {
    let value = value?.as_object()?;
    Some(HostRuleVisibility {
        mode: value
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("inherit")
            .to_string(),
        cidrs: value
            .get("cidrs")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        policy_id: value
            .get("policy_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[allow(deprecated)] // Read legacy CIDRs only while upgrading old snapshots.
fn parse_advanced_auth(value: Option<&Value>) -> Option<AdvancedAuthConfig> {
    let value = value?.as_object()?;
    let groups = value
        .get("groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(Value::as_object)
                .map(|group| AdvancedAuthGroup {
                    id: group
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    conditions: group
                        .get("conditions")
                        .and_then(Value::as_array)
                        .map(|conditions| {
                            conditions
                                .iter()
                                .filter_map(Value::as_object)
                                .map(|condition| AdvancedAuthCondition {
                                    id: condition
                                        .get("id")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    target: condition
                                        .get("target")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    operator: condition
                                        .get("operator")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    name: condition
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    values: string_array(condition.get("values")),
                                    cidrs: string_array(condition.get("cidrs")),
                                    policy_id: condition
                                        .get("policy_id")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(AdvancedAuthConfig {
        enabled: value
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        idle_ttl_seconds: value
            .get("idle_ttl_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        max_lifetime_seconds: value
            .get("max_lifetime_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        policy_version: value
            .get("policy_version")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        groups,
    })
}

fn parse_host_rules(value: &Value) -> Vec<HostRule> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| HostRule {
                    host: string_field(item, "host"),
                    target: string_field(item, "target"),
                    target_path_mode: string_field(item, "target_path_mode"),
                    use_auth: bool_field(item, "use_auth", true),
                    access_mode: string_field(item, "access_mode"),
                    suppress_toolbar: bool_field(item, "suppress_toolbar", false),
                    preserve_host: bool_field(item, "preserve_host", true),
                    is_default: bool_field(item, "is_default", false),
                    disabled: bool_field(item, "disabled", false),
                    availability: item
                        .get("availability")
                        .and_then(parse_host_rule_availability),
                    visibility: parse_host_rule_visibility(item.get("visibility")),
                    advanced_auth: parse_advanced_auth(item.get("advanced_auth")),
                    protocol_mode: string_field(item, "protocol_mode"),
                    group_id: item
                        .get("group_id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .map(ToString::to_string),
                    group_name: item
                        .get("group_name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .map(ToString::to_string),
                    title: string_field(item, "title"),
                    favicon: string_field(item, "favicon"),
                    basic_auth: item.get("basic_auth").map(parse_basic_auth),
                    locations: item
                        .get("locations")
                        .map(parse_host_locations)
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_stream_rules(value: &Value) -> Vec<StreamRule> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| StreamRule {
                    protocol: string_field(item, "protocol"),
                    listen_port: i32_field(item, "listen_port", 0),
                    target: string_field(item, "target"),
                    use_auth: bool_field(item, "use_auth", true),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_stream_availability(value: Option<&Value>) -> Option<StreamAvailability> {
    let value = value?;
    if value.get("enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    Some(StreamAvailability {
        enabled: true,
        start_time: string_field(value, "start_time"),
        end_time: string_field(value, "end_time"),
    })
}

fn parse_auth_config(value: &Value) -> AuthConfig {
    AuthConfig {
        auth_port: i32_field(value, "auth_port", 0),
        auth_url: string_field(value, "auth_url"),
        login_url: string_field(value, "login_url"),
        logout_url: string_field(value, "logout_url"),
        preflight_url: string_field(value, "preflight_url"),
        auth_cache_ttl_seconds: i32_field(value, "auth_cache_ttl_seconds", 0),
        auth_cache_unauthorized_ttl_seconds: i32_field(
            value,
            "auth_cache_unauthorized_ttl_seconds",
            0,
        ),
        edge_client_ip_enabled: bool_field(value, "edge_client_ip_enabled", false),
        aliyun_esa_enabled: bool_field(value, "aliyun_esa_enabled", false),
        tencent_edgeone_enabled: bool_field(value, "tencent_edgeone_enabled", false),
        public_auth_base_url: string_field(value, "public_auth_base_url"),
        public_http_port: i32_field(value, "public_http_port", 0),
        public_https_port: i32_field(value, "public_https_port", 0),
        auth_host: string_field(value, "auth_host"),
        trust_forwarded_proto: bool_field(value, "trust_forwarded_proto", false),
    }
}

fn parse_logging(value: &Value) -> LoggingConfig {
    LoggingConfig {
        enabled: bool_field(value, "enabled", false),
        record_localhost: bool_field(value, "record_localhost", false),
        max_days: i32_field(value, "max_days", 0),
        logs_dir: string_field(value, "logs_dir"),
        dropped_entries: 0,
        queue_size: 0,
        queue_depth: 0,
    }
}

fn parse_throttle(value: &Value) -> ReverseProxyThrottleConfig {
    ReverseProxyThrottleConfig {
        enabled: bool_field(value, "enabled", false),
        requests_per_second: i32_field(value, "requests_per_second", 0),
        burst: i32_field(value, "burst", 0),
        block_seconds: i32_field(value, "block_seconds", 0),
    }
}

#[allow(deprecated)] // Read legacy expanded CIDRs while upgrading old snapshots.
fn parse_visibility(value: &Value) -> anyhow::Result<GatewayVisibilityConfig> {
    let policy = parse_optional_compiled_ip_set(value.get("policy"))?;
    Ok(GatewayVisibilityConfig {
        enabled: bool_field(value, "enabled", false),
        cidrs: string_vec_field(value, "cidrs"),
        updated_at: string_field(value, "updated_at"),
        policy_id: string_field(value, "policy_id"),
        policy,
    })
}

fn parse_omit_targets(value: &Value) -> OmitTargetsConfig {
    OmitTargetsConfig {
        enabled: bool_field(value, "enabled", false),
        omit_targets: string_vec_field(value, "omit_targets"),
        updated_at: string_field(value, "updated_at"),
    }
}

fn parse_crawler_blocker(value: &Value) -> CrawlerBlockerConfig {
    CrawlerBlockerConfig {
        enabled: bool_field(value, "enabled", false),
        updated_at: string_field(value, "updated_at"),
    }
}

fn parse_portal(value: &Value) -> GatewayPortalConfig {
    GatewayPortalConfig {
        enabled: bool_field(value, "enabled", true),
        display_style: string_field(value, "display_style"),
        show_app_icon: bool_field(value, "show_app_icon", false),
        icon_drag_mode: string_field(value, "icon_drag_mode"),
        version: string_field(value, "version"),
        show_wol: bool_field(value, "show_wol", true),
    }
}

fn parse_gateway_unmatched_route(value: &Value) -> GatewayUnmatchedRouteConfig {
    GatewayUnmatchedRouteConfig {
        behavior: string_field(value, "behavior"),
        upstream_error_detail: string_field(value, "upstream_error_detail"),
    }
}

fn parse_fnos_port_icon_hijack(value: &Value) -> FnosPortIconHijackConfig {
    FnosPortIconHijackConfig {
        enabled: bool_field(value, "enabled", false),
        updated_at: string_field(value, "updated_at"),
    }
}

fn parse_throttle_exempt(value: &Value) -> anyhow::Result<ReverseProxyThrottleExemptIpsRuntime> {
    Ok(ReverseProxyThrottleExemptIpsRuntime {
        enabled: bool_field(value, "enabled", false),
        ips: string_vec_field(value, "ips"),
        cidrs: string_vec_field(value, "cidrs"),
        updated_at: string_field(value, "updated_at"),
        policy_id: string_field(value, "policy_id"),
        policy: parse_optional_compiled_ip_set(value.get("policy"))?,
    })
}

#[allow(deprecated)]
fn parse_gateway_trusted_client_ips(
    value: &Value,
) -> anyhow::Result<GatewayTrustedClientIpsRuntime> {
    Ok(GatewayTrustedClientIpsRuntime {
        ips: string_vec_field(value, "ips"),
        cidrs: string_vec_field(value, "cidrs"),
        updated_at: string_field(value, "updated_at"),
        policy_id: string_field(value, "policy_id"),
        policy: parse_optional_compiled_ip_set(value.get("policy"))?,
    })
}

fn parse_common_exemptions(value: &Value) -> anyhow::Result<CommonLocationExemptionsRuntime> {
    Ok(CommonLocationExemptionsRuntime {
        enabled: bool_field(value, "enabled", false),
        waf_enabled: bool_field(value, "waf_enabled", false),
        cidrs: string_vec_field(value, "cidrs"),
        updated_at: string_field(value, "updated_at"),
        policy_id: string_field(value, "policy_id"),
        policy: parse_optional_compiled_ip_set(value.get("policy"))?,
    })
}

fn parse_waf_config(value: &Value) -> WafConfig {
    WafConfig {
        enabled: bool_field(value, "enabled", false),
        mode: string_field(value, "mode"),
        rules_dir: string_field(value, "rules_dir"),
        active_bundle_id: string_field(value, "active_bundle_id"),
        paranoia_level: i32_field(value, "paranoia_level", 0),
        executing_paranoia_level: i32_field(value, "executing_paranoia_level", 0),
        inbound_anomaly_threshold: i32_field(value, "inbound_anomaly_threshold", 0),
        outbound_anomaly_threshold: i32_field(value, "outbound_anomaly_threshold", 0),
        request_body_access: bool_field(value, "request_body_access", false),
        request_body_limit_bytes: i32_field(value, "request_body_limit_bytes", 0),
        request_body_in_memory_limit_bytes: i32_field(
            value,
            "request_body_in_memory_limit_bytes",
            0,
        ),
        response_body_access: bool_field(value, "response_body_access", false),
        disabled_hosts: string_vec_field(value, "disabled_hosts"),
        disabled_path_prefixes: string_vec_field(value, "disabled_path_prefixes"),
        updated_at: string_field(value, "updated_at"),
    }
}

fn parse_ssl_config(value: &Value) -> SslConfig {
    SslConfig {
        deployment_mode: string_field(value, "deployment_mode"),
        certificates: value
            .get("certificates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| SslDeployedCertificate {
                        id: string_field(item, "id"),
                        label: string_field(item, "label"),
                        cert: string_field(item, "cert"),
                        key: string_field(item, "key"),
                        is_default: bool_field(item, "is_default", false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn rules_to_json(items: Vec<Rule>) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|item| {
                json!({
                    "path": item.path,
                    "target": item.target,
                    "use_auth": item.use_auth,
                    "strip_path": item.strip_path,
                    "rewrite_html": item.rewrite_html,
                    "use_root_mode": item.use_root_mode
                })
            })
            .collect(),
    )
}

fn host_location_response_to_json(response: Option<HostLocationResponse>) -> Value {
    match response {
        Some(response) => json!({
            "status": response.status,
            "content_type": response.content_type,
            "headers": response.headers,
            "body": response.body
        }),
        None => Value::Null,
    }
}

fn host_rule_availability_to_json(availability: Option<HostRuleAvailability>) -> Value {
    match availability {
        Some(availability) => json!({
            "enabled": availability.enabled,
            "start_time": availability.start_time,
            "end_time": availability.end_time,
        }),
        None => Value::Null,
    }
}

#[allow(deprecated)] // Echo legacy CIDRs and compiled policy references for compatibility validation.
fn advanced_auth_to_json(config: Option<AdvancedAuthConfig>) -> Value {
    let Some(config) = config else {
        return Value::Null;
    };
    json!({
        "enabled": config.enabled,
        "idle_ttl_seconds": config.idle_ttl_seconds,
        "max_lifetime_seconds": config.max_lifetime_seconds,
        "policy_version": config.policy_version,
        "groups": config.groups.into_iter().map(|group| json!({
            "id": group.id,
            "conditions": group.conditions.into_iter().map(|condition| json!({
                "id": condition.id,
                "target": condition.target,
                "operator": condition.operator,
                "name": condition.name,
                "values": condition.values,
                "cidrs": condition.cidrs,
                "policy_id": condition.policy_id,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[allow(deprecated)] // Echo the deprecated field only for compatibility validation.
fn host_rules_to_json(bundle: HostRules) -> Value {
    let items = Value::Array(
        bundle
            .items
            .into_iter()
            .map(|item| {
                json!({
                    "host": item.host,
                    "target": item.target,
                    "target_path_mode": item.target_path_mode,
                    "use_auth": item.use_auth,
                    "access_mode": item.access_mode,
                    "suppress_toolbar": item.suppress_toolbar,
                    "preserve_host": item.preserve_host,
                    "is_default": item.is_default,
                    "disabled": item.disabled,
                    "availability": host_rule_availability_to_json(item.availability),
                    "visibility": item.visibility.map(|visibility| json!({
                        "mode": visibility.mode,
                        "cidrs": visibility.cidrs,
                        "policy_id": visibility.policy_id,
                    })).unwrap_or_else(|| json!({ "mode": "inherit", "cidrs": [] })),
                    "advanced_auth": advanced_auth_to_json(item.advanced_auth),
                    "protocol_mode": item.protocol_mode,
                    "group_id": item.group_id,
                    "group_name": item.group_name,
                    "title": item.title,
                    "favicon": item.favicon,
                    "basic_auth": item.basic_auth.map(|auth| json!({
                        "enabled": auth.enabled,
                        "username": auth.username,
                        "password": auth.password
                    })).unwrap_or(Value::Null),
                    "locations": item.locations.into_iter().map(|location| json!({
                        "path": location.path,
                        "match": location.r#match,
                        "action": location.action,
                        "target": location.target,
                        "strip_path": location.strip_path,
                        "rewrite_html": location.rewrite_html,
                        "response": host_location_response_to_json(location.response)
                    })).collect::<Vec<_>>()
                })
            })
            .collect(),
    );
    let visibility_policies = bundle
        .visibility_policies
        .into_iter()
        .map(compiled_ip_set_to_json)
        .collect::<Vec<_>>();
    json!({
        "items": items,
        "visibility_policies": visibility_policies,
    })
}

fn stream_rules_to_json(rules: StreamRules) -> Value {
    let items = Value::Array(
        rules
            .items
            .into_iter()
            .map(|item| {
                json!({
                    "protocol": item.protocol,
                    "listen_port": item.listen_port,
                    "target": item.target,
                    "use_auth": item.use_auth
                })
            })
            .collect(),
    );
    let availability = rules
        .availability
        .map(|value| {
            json!({
                "enabled": value.enabled,
                "start_time": value.start_time,
                "end_time": value.end_time,
            })
        })
        .unwrap_or(Value::Null);
    json!({
        "items": items,
        "availability": availability,
    })
}

fn locale_to_json(config: LocaleConfig) -> Value {
    json!({ "default_locale": config.default_locale })
}

fn logging_to_json(config: LoggingConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "record_localhost": config.record_localhost,
        "max_days": config.max_days,
        "logs_dir": config.logs_dir,
        "dropped_entries": config.dropped_entries,
        "queue_size": config.queue_size,
        "queue_depth": config.queue_depth
    })
}

fn throttle_to_json(config: ReverseProxyThrottleConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "requests_per_second": config.requests_per_second,
        "burst": config.burst,
        "block_seconds": config.block_seconds
    })
}

#[allow(deprecated)] // Echo the deprecated field only for compatibility validation.
fn visibility_to_json(config: GatewayVisibilityConfig) -> Value {
    let policy = config.policy.map(compiled_ip_set_to_json);
    json!({
        "enabled": config.enabled,
        "cidrs": config.cidrs,
        "updated_at": config.updated_at,
        "policy_id": config.policy_id,
        "policy": policy,
    })
}

fn omit_targets_to_json(config: OmitTargetsConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "omit_targets": config.omit_targets,
        "updated_at": config.updated_at
    })
}

fn crawler_blocker_to_json(config: CrawlerBlockerConfig) -> Value {
    json!({ "enabled": config.enabled, "updated_at": config.updated_at })
}

fn portal_to_json(config: GatewayPortalConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "display_style": config.display_style,
        "show_app_icon": config.show_app_icon,
        "show_wol": config.show_wol,
        "icon_drag_mode": config.icon_drag_mode,
        "version": config.version
    })
}

fn gateway_unmatched_route_to_json(config: GatewayUnmatchedRouteConfig) -> Value {
    json!({
        "behavior": config.behavior,
        "upstream_error_detail": config.upstream_error_detail
    })
}

fn fnos_port_icon_hijack_to_json(config: FnosPortIconHijackConfig) -> Value {
    json!({ "enabled": config.enabled, "updated_at": config.updated_at })
}

fn fnos_connect_ingress_status_to_json(status: FnosConnectIngressStatus) -> Value {
    json!({
        "enabled": status.enabled,
        "listener_active": status.listener_active,
        "listen_port": status.listen_port,
        "upstream_http_port": status.upstream_http_port,
        "ipv4_active": status.ipv4_active,
        "ipv6_active": status.ipv6_active,
        "waf_active": status.waf_active,
        "waf_mode": status.waf_mode,
        "last_error": status.last_error,
    })
}

fn throttle_exempt_to_json(config: ReverseProxyThrottleExemptIpsRuntime) -> Value {
    json!({
        "enabled": config.enabled,
        "ips": config.ips,
        "cidrs": config.cidrs,
        "updated_at": config.updated_at,
        "policy_id": config.policy_id,
        "policy": config.policy.map(compiled_ip_set_to_json)
    })
}

#[allow(deprecated)]
fn gateway_trusted_client_ips_to_json(config: GatewayTrustedClientIpsRuntime) -> Value {
    json!({
        "ips": config.ips,
        "cidrs": config.cidrs,
        "updated_at": config.updated_at,
        "policy_id": config.policy_id,
        "policy": config.policy.map(compiled_ip_set_to_json)
    })
}

fn common_exemptions_to_json(config: CommonLocationExemptionsRuntime) -> Value {
    json!({
        "enabled": config.enabled,
        "waf_enabled": config.waf_enabled,
        "cidrs": config.cidrs,
        "updated_at": config.updated_at,
        "policy_id": config.policy_id,
        "policy": config.policy.map(compiled_ip_set_to_json)
    })
}

fn general_blacklist_record_to_json(record: crate::grpc_proto::GeneralBlacklistRecord) -> Value {
    json!({
        "ip": record.ip,
        "source": record.source,
        "comment": record.comment,
        "created_at": record.created_at,
        "updated_at": record.updated_at
    })
}

fn general_blacklist_list_to_json(list: crate::grpc_proto::GeneralBlacklistList) -> Value {
    json!({
        "total": list.total,
        "items": list.items.into_iter().map(general_blacklist_record_to_json).collect::<Vec<_>>()
    })
}

fn general_blacklist_mutation_to_json(
    result: crate::grpc_proto::GeneralBlacklistMutationResult,
) -> Value {
    json!({
        "added": result.added,
        "updated": result.updated,
        "removed": result.removed,
        "total": result.total,
        "items": result.items.into_iter().map(general_blacklist_record_to_json).collect::<Vec<_>>()
    })
}

fn general_blacklist_status_to_json(status: crate::grpc_proto::GeneralBlacklistStatus) -> Value {
    let records = status
        .records
        .into_iter()
        .map(|(key, value)| (key, general_blacklist_record_to_json(value)))
        .collect::<serde_json::Map<_, _>>();
    json!({ "records": records })
}

fn traffic_to_json(stats: crate::grpc_proto::TrafficStats) -> Value {
    json!({
        "total_in": stats.total_in,
        "total_out": stats.total_out,
        "active_conns": stats.active_conns,
        "error_5xx": stats.error_5xx,
        "by_host": stats.by_host.into_iter().map(|item| json!({
            "host": item.host,
            "total_in": item.total_in,
            "total_out": item.total_out,
            "error_5xx": item.error_5xx,
            "active_ip_count": item.active_ip_count
        })).collect::<Vec<_>>()
    })
}

fn active_ip_to_json(item: HostActiveIpStats) -> Value {
    json!({
        "ip": item.ip,
        "last_seen_at": item.last_seen_at,
        "active_conns": item.active_conns
    })
}

fn active_ips_to_json(stats: crate::grpc_proto::HostActiveIpsStats) -> Value {
    json!({
        "host": stats.host,
        "window_seconds": stats.window_seconds,
        "items": stats.items.into_iter().map(active_ip_to_json).collect::<Vec<_>>()
    })
}

fn waf_status_to_json(status: crate::grpc_proto::WafStatus) -> Value {
    json!({
        "enabled": status.enabled,
        "mode": status.mode,
        "loaded": status.loaded,
        "bundle_id": status.bundle_id,
        "bundle_hash": status.bundle_hash,
        "loaded_at": status.loaded_at,
        "rules_dir": status.rules_dir,
        "pending_events": status.pending_events,
        "last_error": status.last_error
    })
}

fn waf_drain_to_json(result: crate::grpc_proto::WafDrainResult) -> Value {
    json!({
        "events": result.events.into_iter().map(|event| json!({
            "trace_id": event.trace_id,
            "transaction_id": event.transaction_id,
            "time": event.time,
            "mode": event.mode,
            "action": event.action,
            "status": event.status,
            "client_ip": event.client_ip,
            "remote_addr": event.remote_addr,
            "method": event.method,
            "scheme": event.scheme,
            "host": event.host,
            "path": event.path,
            "query": event.query,
            "request_uri": event.request_uri,
            "user_agent": event.user_agent,
            "referer": event.referer,
            "route_type": event.route_type,
            "route_key": event.route_key,
            "upstream": event.upstream,
            "bundle_id": event.bundle_id,
            "bundle_hash": event.bundle_hash,
            "rule_ids": event.rule_ids,
            "rules": event.rules.into_iter().map(|rule| json!({
                "id": rule.id,
                "message": rule.message,
                "data": rule.data,
                "severity": rule.severity,
                "phase": rule.phase,
                "file": rule.file,
                "line": rule.line,
                "tags": rule.tags,
                "disruptive": rule.disruptive,
                "matched_variables": rule.matched_variables.into_iter().map(|matched| json!({
                    "variable": matched.variable,
                    "key": matched.key,
                    "value_preview": matched.value_preview
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "interruption": event.interruption.map(|value| json!({
                "rule_id": value.rule_id,
                "action": value.action,
                "status": value.status
            })).unwrap_or(Value::Null),
            "error": event.error
        })).collect::<Vec<_>>(),
        "drained": result.drained,
        "remaining": result.remaining
    })
}

fn ssl_info_to_json(info: crate::grpc_proto::SslInfo) -> Value {
    json!({
        "enabled": info.enabled,
        "deployment_mode": info.deployment_mode,
        "certificates": info.certificates.into_iter().map(|cert| json!({
            "id": cert.id,
            "label": cert.label,
            "domains": cert.domains,
            "is_default": cert.is_default
        })).collect::<Vec<_>>()
    })
}

fn log_dates_to_json(dates: crate::grpc_proto::GatewayLogDates) -> Value {
    json!({
        "today": dates.today,
        "logs_dir": dates.logs_dir,
        "dates": dates.dates
    })
}

fn log_entry_to_json(entry: crate::grpc_proto::GatewayLogEntry) -> Value {
    let mut object = serde_json::Map::new();
    object.insert("time".to_string(), Value::String(entry.time));
    object.insert("level".to_string(), Value::String(entry.level));
    object.insert("method".to_string(), Value::String(entry.method));
    object.insert("scheme".to_string(), Value::String(entry.scheme));
    object.insert("host".to_string(), Value::String(entry.host));
    object.insert("path".to_string(), Value::String(entry.path));
    object.insert("query".to_string(), Value::String(entry.query));
    object.insert("request_uri".to_string(), Value::String(entry.request_uri));
    object.insert("protocol".to_string(), Value::String(entry.protocol));
    object.insert("status".to_string(), json!(entry.status));
    object.insert("duration_ms".to_string(), json!(entry.duration_ms));
    object.insert("remote_ip".to_string(), Value::String(entry.remote_ip));
    object.insert("remote_addr".to_string(), Value::String(entry.remote_addr));
    object.insert("client_ip".to_string(), Value::String(entry.client_ip));
    object.insert("user_agent".to_string(), Value::String(entry.user_agent));
    object.insert("referer".to_string(), Value::String(entry.referer));
    object.insert("logged_in".to_string(), Value::Bool(entry.logged_in));
    object.insert(
        "auth_required".to_string(),
        Value::Bool(entry.auth_required),
    );
    object.insert(
        "auth_decision".to_string(),
        Value::String(entry.auth_decision),
    );
    object.insert(
        "auth_rule_group_id".to_string(),
        Value::String(entry.auth_rule_group_id),
    );
    object.insert(
        "auth_grant_state".to_string(),
        Value::String(entry.auth_grant_state),
    );
    object.insert(
        "auth_credential_id".to_string(),
        Value::String(entry.auth_credential_id),
    );
    object.insert(
        "auth_credential_name".to_string(),
        Value::String(entry.auth_credential_name),
    );
    object.insert(
        "auth_credential_method".to_string(),
        Value::String(entry.auth_credential_method),
    );
    object.insert(
        "auth_linked_totp_id".to_string(),
        Value::String(entry.auth_linked_totp_id),
    );
    object.insert(
        "auth_linked_totp_name".to_string(),
        Value::String(entry.auth_linked_totp_name),
    );
    object.insert("access_mode".to_string(), Value::String(entry.access_mode));
    object.insert("route_type".to_string(), Value::String(entry.route_type));
    object.insert("route_key".to_string(), Value::String(entry.route_key));
    object.insert("upstream".to_string(), Value::String(entry.upstream));
    object.insert("matched".to_string(), Value::Bool(entry.matched));
    object.insert("bytes_in".to_string(), json!(entry.bytes_in));
    object.insert("bytes_out".to_string(), json!(entry.bytes_out));
    object.insert("tls".to_string(), Value::Bool(entry.tls));
    object.insert("websocket".to_string(), Value::Bool(entry.websocket));
    object.insert(
        "ali_real_client_ip".to_string(),
        Value::String(entry.ali_real_client_ip),
    );
    object.insert(
        "eo_connecting_ip".to_string(),
        Value::String(entry.eo_connecting_ip),
    );
    object.insert(
        "x_forwarded_for".to_string(),
        Value::String(entry.x_forwarded_for),
    );
    object.insert("x_real_ip".to_string(), Value::String(entry.x_real_ip));
    object.insert("waf_blocked".to_string(), Value::Bool(entry.waf_blocked));
    object.insert(
        "waf_trace_id".to_string(),
        Value::String(entry.waf_trace_id),
    );
    object.insert("waf_mode".to_string(), Value::String(entry.waf_mode));
    object.insert("waf_rule_ids".to_string(), json!(entry.waf_rule_ids));
    object.insert("waf_action".to_string(), Value::String(entry.waf_action));
    object.insert("waf_bundle".to_string(), Value::String(entry.waf_bundle));
    object.insert(
        "general_blacklist_blocked".to_string(),
        Value::Bool(entry.general_blacklist_blocked),
    );
    Value::Object(object)
}

fn log_analytics_buckets_to_json(
    items: Vec<crate::grpc_proto::GatewayLogAnalyticsBucket>,
) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|item| json!({ "key": item.key, "count": item.count }))
            .collect(),
    )
}

fn log_analytics_to_json(result: crate::grpc_proto::GatewayLogAnalyticsResult) -> Value {
    let summary = result.summary.unwrap_or_default();
    json!({
        "range": {
            "from": result.from_date,
            "to": result.to_date,
            "timezone": result.timezone,
            "granularity": result.granularity,
            "available_dates": result.available_dates,
        },
        "summary": {
            "requests": summary.requests,
            "unique_clients": summary.unique_clients,
            "client_errors": summary.client_errors,
            "server_errors": summary.server_errors,
            "average_duration_ms": summary.average_duration_ms,
            "p95_duration_ms": summary.p95_duration_ms,
            "bytes_in": summary.bytes_in,
            "bytes_out": summary.bytes_out,
            "server_error_rate": summary.server_error_rate,
        },
        "series": result.series.into_iter().map(|point| json!({
            "bucket_start": point.bucket_start,
            "requests": point.requests,
            "client_errors": point.client_errors,
            "server_errors": point.server_errors,
        })).collect::<Vec<_>>(),
        "dimensions": {
            "paths": log_analytics_buckets_to_json(result.paths),
            "routes": log_analytics_buckets_to_json(result.routes),
            "hosts": log_analytics_buckets_to_json(result.hosts),
            "upstreams": log_analytics_buckets_to_json(result.upstreams),
            "referrers": log_analytics_buckets_to_json(result.referrers),
            "utm_sources": log_analytics_buckets_to_json(result.utm_sources),
            "utm_mediums": log_analytics_buckets_to_json(result.utm_mediums),
            "utm_campaigns": log_analytics_buckets_to_json(result.utm_campaigns),
            "devices": log_analytics_buckets_to_json(result.devices),
            "browsers": log_analytics_buckets_to_json(result.browsers),
            "operating_systems": log_analytics_buckets_to_json(result.operating_systems),
            "statuses": log_analytics_buckets_to_json(result.statuses),
            "methods": log_analytics_buckets_to_json(result.methods),
            "latency_bands": log_analytics_buckets_to_json(result.latency_bands),
            "auth_decisions": log_analytics_buckets_to_json(result.auth_decisions),
            "waf_actions": log_analytics_buckets_to_json(result.waf_actions),
        },
        "clients": result.clients.into_iter().map(|client| json!({
            "ip": client.ip,
            "count": client.count,
        })).collect::<Vec<_>>(),
        "quality": { "invalid_entries": result.invalid_entries },
    })
}

fn log_query_to_json(result: crate::grpc_proto::GatewayLogQueryResult) -> Value {
    json!({
        "date": result.date,
        "logs_dir": result.logs_dir,
        "available_dates": result.available_dates,
        "pagination": result.pagination,
        "page": result.page,
        "limit": result.limit,
        "total": result.total,
        "cursor": result.cursor,
        "next_cursor": result.next_cursor,
        "has_more": result.has_more,
        "items": result.items.into_iter().map(log_entry_to_json).collect::<Vec<_>>()
    })
}

fn log_delete_to_json(result: crate::grpc_proto::GatewayLogDeleteResult) -> Value {
    json!({
        "date": result.date,
        "logs_dir": result.logs_dir,
        "deleted": result.deleted,
        "available_dates": result.available_dates
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn go_backend_client_requires_internal_rpc_token() {
        let err = match GoBackendClient::new(
            "127.0.0.1:7996".to_string(),
            " ".to_string(),
            Duration::from_millis(1),
        ) {
            Ok(_) => panic!("GoBackendClient accepted an empty token"),
            Err(error) => error,
        };
        assert!(
            err.to_string()
                .contains("FN_KNOCK_INTERNAL_RPC_TOKEN must be set")
        );
    }

    #[tokio::test]
    async fn go_backend_client_accepts_internal_rpc_token() {
        if let Err(error) = GoBackendClient::new(
            "127.0.0.1:7996".to_string(),
            "token".to_string(),
            Duration::from_millis(1),
        ) {
            panic!("GoBackendClient rejected a non-empty token: {error}");
        }
    }

    #[test]
    fn macos_bundle_platform_accepts_native_darwin_architectures() {
        let capabilities = vec![json!("http")];
        assert!(
            verify_gateway_platform("macos", "aarch64", "darwin", "arm64", &capabilities).is_ok()
        );
        assert!(
            verify_gateway_platform("macos", "x86_64", "darwin", "amd64", &capabilities).is_ok()
        );
        assert!(
            verify_gateway_platform("macos", "aarch64", "darwin", "amd64", &capabilities).is_err()
        );
    }

    #[test]
    fn macos_bundle_rejects_host_firewall_capabilities() {
        let capabilities = vec![json!("firewall.iptables")];
        assert!(
            verify_gateway_platform("macos", "aarch64", "darwin", "arm64", &capabilities).is_err()
        );
    }

    #[test]
    fn advanced_auth_grpc_conversion_preserves_compiled_policy_id() {
        let config = json!({
            "enabled": true,
            "idle_ttl_seconds": 86_400,
            "max_lifetime_seconds": 2_592_000,
            "policy_version": "policy-v1",
            "groups": [{
                "id": "region",
                "conditions": [{
                    "id": "source-region",
                    "target": "source_region",
                    "operator": "in",
                    "name": "",
                    "values": [],
                    "policy_id": "ipset-v2:expected"
                }]
            }]
        });

        let parsed = parse_advanced_auth(Some(&config)).expect("advanced auth config");
        let echoed = advanced_auth_to_json(Some(parsed));

        assert_eq!(
            echoed.pointer("/groups/0/conditions/0/policy_id"),
            Some(&json!("ipset-v2:expected"))
        );
    }

    #[test]
    fn logging_config_json_includes_runtime_queue_metrics() {
        let value = logging_to_json(LoggingConfig {
            enabled: true,
            record_localhost: true,
            max_days: 9,
            logs_dir: "/var/log/fn-knock".to_string(),
            dropped_entries: 12,
            queue_size: 4096,
            queue_depth: 7,
        });

        assert_eq!(
            value,
            json!({
                "enabled": true,
                "record_localhost": true,
                "max_days": 9,
                "logs_dir": "/var/log/fn-knock",
                "dropped_entries": 12,
                "queue_size": 4096,
                "queue_depth": 7
            })
        );
    }

    #[test]
    fn gateway_log_json_preserves_advanced_auth_decision_metadata() {
        let value = log_entry_to_json(crate::grpc_proto::GatewayLogEntry {
            auth_rule_group_id: "admins".to_string(),
            auth_grant_state: "session".to_string(),
            ..Default::default()
        });

        assert_eq!(value["auth_rule_group_id"], "admins");
        assert_eq!(value["auth_grant_state"], "session");
    }

    #[test]
    fn log_analytics_grpc_conversion_preserves_internal_geo_candidates() {
        let value = log_analytics_to_json(crate::grpc_proto::GatewayLogAnalyticsResult {
            from_date: "2026-08-01".to_string(),
            to_date: "2026-08-02".to_string(),
            granularity: "hour".to_string(),
            summary: Some(crate::grpc_proto::GatewayLogAnalyticsSummary {
                requests: 4,
                unique_clients: 2,
                server_error_rate: 0.25,
                ..Default::default()
            }),
            paths: vec![crate::grpc_proto::GatewayLogAnalyticsBucket {
                key: "/".to_string(),
                count: 4,
            }],
            clients: vec![crate::grpc_proto::GatewayLogAnalyticsClient {
                ip: "203.0.113.7".to_string(),
                count: 3,
            }],
            ..Default::default()
        });

        assert_eq!(value.pointer("/summary/requests"), Some(&json!(4)));
        assert_eq!(
            value.pointer("/summary/server_error_rate"),
            Some(&json!(0.25))
        );
        assert_eq!(value.pointer("/dimensions/paths/0/key"), Some(&json!("/")));
        assert_eq!(value.pointer("/clients/0/ip"), Some(&json!("203.0.113.7")));
    }

    #[test]
    fn parse_logging_ignores_runtime_queue_metrics_for_set_requests() {
        let parsed = parse_logging(&json!({
            "enabled": true,
            "max_days": 14,
            "logs_dir": "/ignored",
            "dropped_entries": 99,
            "queue_size": 88,
            "queue_depth": 77
        }));

        assert!(parsed.enabled);
        assert_eq!(parsed.max_days, 14);
        assert_eq!(parsed.logs_dir, "/ignored");
        assert_eq!(parsed.dropped_entries, 0);
        assert_eq!(parsed.queue_size, 0);
        assert_eq!(parsed.queue_depth, 0);
    }

    #[test]
    fn gateway_unmatched_route_grpc_conversion_round_trips() {
        let parsed = parse_gateway_unmatched_route(&json!({
            "behavior": "reset_connection",
            "upstream_error_detail": "reset_connection"
        }));
        assert_eq!(parsed.behavior, "reset_connection");
        assert_eq!(parsed.upstream_error_detail, "reset_connection");
        assert_eq!(
            gateway_unmatched_route_to_json(parsed),
            json!({
                "behavior": "reset_connection",
                "upstream_error_detail": "reset_connection"
            })
        );
    }

    #[test]
    fn gateway_trusted_client_ips_grpc_conversion_round_trips() {
        let policy = crate::cidr::compile_ip_set(["127.0.0.1/32", "203.0.113.7/32"]).unwrap();
        let mut policy_json = policy.to_config_value();
        policy_json["id"] = json!(policy.id);
        let runtime = json!({
            "ips": ["127.0.0.1", "203.0.113.7"],
            "cidrs": [],
            "policy_id": policy.id,
            "policy": policy_json,
            "updated_at": "2026-07-31T01:00:00.123Z"
        });
        let parsed = parse_gateway_trusted_client_ips(&runtime).unwrap();

        assert_eq!(gateway_trusted_client_ips_to_json(parsed), runtime);
    }

    #[test]
    fn flat_host_rule_payload_explicitly_clears_group_metadata() {
        let rules = parse_host_rules(&json!([{
            "host": "app.example.test",
            "target": "http://127.0.0.1:8080/base",
            "target_path_mode": "prefix",
            "group_id": "",
            "group_name": ""
        }]));
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].target_path_mode, "prefix");
        assert_eq!(rules[0].group_id.as_deref(), Some(""));
        assert_eq!(rules[0].group_name.as_deref(), Some(""));
    }

    #[test]
    fn stream_rules_grpc_conversion_preserves_global_availability() {
        let payload = json!({
            "items": [{
                "protocol": "tcp",
                "listen_port": 3306,
                "target": "127.0.0.1:33060",
                "use_auth": true
            }],
            "availability": {
                "enabled": true,
                "start_time": "22:00",
                "end_time": "06:00"
            }
        });
        let rules = StreamRules {
            items: parse_stream_rules(&payload["items"]),
            availability: parse_stream_availability(payload.get("availability")),
        };

        assert_eq!(stream_rules_to_json(rules), payload);
    }
}
