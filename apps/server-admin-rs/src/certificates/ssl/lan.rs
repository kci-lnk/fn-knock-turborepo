use super::*;
use axum::http::HeaderMap;
use get_if_addrs::{IfAddr, get_if_addrs};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr};
use utoipa::ToSchema;

pub(super) const SSL_LAN_DEPLOYMENT_KEY: &str = "ssl_lan_deployment";
const MAX_LAN_ADDRESSES: usize = 16;
const DOCKER_DISCOVER_IP_HEADER: &str = "x-fn-knock-docker-discover-ip";

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct LanCertificateDeploymentUpdateBody {
    enabled: bool,
    addresses: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(super) struct LanCertificateDeploymentData {
    enabled: bool,
    configured_addresses: Vec<String>,
    detected_addresses: Vec<String>,
    gateway_port: u16,
    listener_scope: String,
    status: String,
}

pub(super) fn normalize_lan_deployment(value: Option<&Value>) -> Value {
    let enabled = value
        .and_then(|item| item.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let addresses = value
        .and_then(|item| item.get("addresses"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|item| normalize_rfc1918_ipv4(item).ok())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "enabled": enabled,
        "addresses": addresses,
        "updated_at": value.and_then(|item| item.get("updated_at")).and_then(Value::as_str).unwrap_or("")
    })
}

fn normalize_rfc1918_ipv4(value: &str) -> Result<String, String> {
    let address = value
        .trim()
        .parse::<Ipv4Addr>()
        .map_err(|_| format!("{value:?} is not a valid IPv4 address"))?;
    if !address.is_private() {
        return Err(format!("{address} is not an RFC1918 IPv4 address"));
    }
    Ok(address.to_string())
}

fn normalize_lan_addresses(values: &[String]) -> Result<Vec<String>, String> {
    let result = values
        .iter()
        .map(|value| normalize_rfc1918_ipv4(value))
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .collect::<Vec<_>>();
    if result.is_empty() {
        return Err("At least one LAN address is required".to_string());
    }
    if result.len() > MAX_LAN_ADDRESSES {
        return Err(format!(
            "At most {MAX_LAN_ADDRESSES} LAN addresses are supported"
        ));
    }
    Ok(result)
}

fn excluded_interface(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "lo"
        || name.starts_with("docker")
        || name.starts_with("br-")
        || name.starts_with("veth")
        || name.starts_with("tun")
        || name.starts_with("tap")
        || name.starts_with("tailscale")
        || name.starts_with("wg")
        || name.starts_with("zt")
}

fn detected_lan_addresses(headers: &HeaderMap) -> Vec<String> {
    let mut addresses = BTreeSet::new();
    if let Ok(interfaces) = get_if_addrs() {
        for interface in interfaces {
            if interface.is_loopback() || excluded_interface(&interface.name) {
                continue;
            }
            if let IfAddr::V4(address) = interface.addr
                && address.ip.is_private()
            {
                addresses.insert(address.ip.to_string());
            }
        }
    }
    if let Some(address) = headers
        .get(DOCKER_DISCOVER_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| normalize_rfc1918_ipv4(value).ok())
    {
        addresses.insert(address);
    }
    addresses.into_iter().take(MAX_LAN_ADDRESSES).collect()
}

pub(super) fn gateway_port() -> u16 {
    std::env::var("GO_REPROXY_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(7999)
}

pub(super) fn default_ssl_available(config: &Value) -> bool {
    let ssl = normalize_ssl_config(config.get("ssl"));
    !ssl.get("cert")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
}

async fn lan_data(
    state: &AppState,
    headers: &HeaderMap,
) -> anyhow::Result<LanCertificateDeploymentData> {
    let config = state.storage.store.get_config().await?;
    let lan = normalize_lan_deployment(config.get(SSL_LAN_DEPLOYMENT_KEY));
    let enabled = lan.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let configured_addresses = lan
        .get("addresses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let listener = state.gateway.client.get_gateway_listener_scope().await;
    let (listener_scope, status) = match listener {
        Err(_) if enabled => ("unknown".to_string(), "gateway_unavailable".to_string()),
        Err(_) => ("unknown".to_string(), "disabled".to_string()),
        Ok(scope) if !enabled => (scope, "disabled".to_string()),
        Ok(scope) if scope == "loopback" => (scope, "listener_loopback".to_string()),
        Ok(scope) if !default_ssl_available(&config) => (scope, "ssl_unavailable".to_string()),
        Ok(scope) => (scope, "ready".to_string()),
    };
    Ok(LanCertificateDeploymentData {
        enabled,
        configured_addresses,
        detected_addresses: detected_lan_addresses(headers),
        gateway_port: gateway_port(),
        listener_scope,
        status,
    })
}

#[utoipa::path(get, path = "/api/admin/ssl/external-bindings/lan", tag = "ssl", operation_id = "get_api_admin_ssl_external_bindings_lan", responses((status = 200, body = LanCertificateDeploymentData)))]
pub(super) async fn get_lan_certificate_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    match lan_data(&state, &headers).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load LAN certificate deployment settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load LAN certificate deployment settings",
            )
        }
    }
}

#[utoipa::path(put, path = "/api/admin/ssl/external-bindings/lan", tag = "ssl", operation_id = "put_api_admin_ssl_external_bindings_lan", request_body = LanCertificateDeploymentUpdateBody, responses((status = 200, body = LanCertificateDeploymentData)))]
pub(super) async fn update_lan_certificate_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LanCertificateDeploymentUpdateBody>,
) -> Response {
    let addresses = if body.enabled {
        match normalize_lan_addresses(&body.addresses) {
            Ok(addresses) => addresses,
            Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
        }
    } else {
        body.addresses
            .iter()
            .filter_map(|address| normalize_rfc1918_ipv4(address).ok())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(MAX_LAN_ADDRESSES)
            .collect()
    };
    let _guard = state.gateway.ssl_update_lock.lock().await;
    if body.enabled {
        match state.gateway.client.get_gateway_listener_scope().await {
            Ok(scope) if scope == "loopback" => {
                return response::error(
                    StatusCode::CONFLICT,
                    "The gateway listener is limited to loopback; change its scope before enabling LAN certificate deployment",
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to inspect gateway listener before LAN certificate update");
                return response::error(StatusCode::BAD_GATEWAY, "The gateway is unavailable");
            }
        }
    }
    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if body.enabled && !default_ssl_available(&previous_config) {
        return response::error(
            StatusCode::CONFLICT,
            "Install and activate a default SSL certificate before enabling LAN certificate deployment",
        );
    }
    let previous_lan = previous_config
        .get(SSL_LAN_DEPLOYMENT_KEY)
        .cloned()
        .unwrap_or(Value::Null);
    let next_lan = json!({
        "enabled": body.enabled,
        "addresses": addresses,
        "updated_at": time_utils::node_iso_now()
    });
    let next_config = match state
        .storage
        .store
        .set_config_top_level_value(SSL_LAN_DEPLOYMENT_KEY, next_lan)
        .await
    {
        Ok(config) => config,
        Err(error) => return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if let Err(error) = sync_ssl_deployment_to_gateway(&state, Some(&next_config)).await {
        let restore_config = state
            .storage
            .store
            .set_config_top_level_value(SSL_LAN_DEPLOYMENT_KEY, previous_lan)
            .await;
        let restore_gateway = match &restore_config {
            Ok(config) => sync_ssl_deployment_to_gateway(&state, Some(config)).await,
            Err(_) => Err(anyhow!("LAN configuration rollback failed")),
        };
        if restore_config.is_err() || restore_gateway.is_err() {
            tracing::error!(%error, "LAN certificate deployment update and rollback failed");
            return response::error(
                StatusCode::BAD_GATEWAY,
                "LAN certificate deployment failed and the previous gateway state could not be confirmed",
            );
        }
        return response::error(
            StatusCode::BAD_GATEWAY,
            "LAN certificate deployment failed; the previous configuration was restored",
        );
    }
    crate::panel_sync::notify_source_changed(&state);
    match lan_data(&state, &headers).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

pub(super) fn lan_deploy_request_matches(config: &Value, headers: &HeaderMap) -> bool {
    let Some(host) = super::external::normalized_request_host(headers) else {
        return false;
    };
    let lan = normalize_lan_deployment(config.get(SSL_LAN_DEPLOYMENT_KEY));
    if !lan.get("enabled").and_then(Value::as_bool).unwrap_or(false)
        || headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            != Some("https")
    {
        return false;
    }
    if !lan_host_allowed(&lan, &host) {
        return false;
    }
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .is_some_and(|address| match address {
            IpAddr::V4(address) => {
                address.is_private()
                    || address.is_loopback()
                    || (u32::from(address) & 0xffc0_0000) == 0x6440_0000
            }
            IpAddr::V6(address) => address.is_loopback() || address.is_unique_local(),
        })
}

pub(super) fn configured_lan_host_matches(config: &Value, host: &str) -> bool {
    let lan = normalize_lan_deployment(config.get(SSL_LAN_DEPLOYMENT_KEY));
    lan.get("enabled").and_then(Value::as_bool).unwrap_or(false) && lan_host_allowed(&lan, host)
}

fn lan_host_allowed(lan: &Value, host: &str) -> bool {
    lan.get("addresses")
        .and_then(Value::as_array)
        .is_some_and(|addresses| {
            addresses
                .iter()
                .any(|address| address.as_str() == Some(host))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, header::HOST};

    #[test]
    fn lan_addresses_accept_only_canonical_rfc1918_ipv4() {
        assert_eq!(
            normalize_lan_addresses(&[
                "192.168.31.98".to_string(),
                "10.0.0.2".to_string(),
                "192.168.31.98".to_string(),
            ])
            .unwrap(),
            vec!["10.0.0.2", "192.168.31.98"]
        );
        assert!(normalize_lan_addresses(&["100.64.0.1".to_string()]).is_err());
        assert!(normalize_lan_addresses(&["8.8.8.8".to_string()]).is_err());
        assert!(normalize_lan_addresses(&["fd00::1".to_string()]).is_err());
    }

    #[test]
    fn lan_request_rechecks_host_https_and_rebuilt_client_address() {
        let config = json!({
            "ssl_lan_deployment": {
                "enabled": true,
                "addresses": ["192.168.31.98"]
            }
        });
        assert!(configured_lan_host_matches(&config, "192.168.31.98"));
        assert!(!configured_lan_host_matches(&config, "192.168.31.99"));
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("192.168.31.98:7999"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.31.50"));
        assert!(lan_deploy_request_matches(&config, &headers));

        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.8"));
        assert!(!lan_deploy_request_matches(&config, &headers));
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.31.50"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));
        assert!(!lan_deploy_request_matches(&config, &headers));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(HOST, HeaderValue::from_static("192.168.31.99:7999"));
        assert!(!lan_deploy_request_matches(&config, &headers));
    }
}
