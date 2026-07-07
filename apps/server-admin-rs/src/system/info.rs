use std::{env, fs, path::PathBuf};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use serde_json::Value;

use crate::{i18n::Translator, response, state::AppState};

fn system_info_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.systemInfoRoutes.{key}"))
}

#[derive(Debug, Serialize, PartialEq)]
struct AccessEntryInfo {
    env: &'static str,
    port: String,
    #[serde(rename = "isDefault")]
    is_default: bool,
}

pub fn system_info_routes() -> Router<AppState> {
    Router::new().route("/api/admin/system/access-entry", get(access_entry))
}

async fn access_entry(State(state): State<AppState>) -> Response {
    match state.redis.get_config().await {
        Ok(config) => response::ok(resolve_access_entry_info(&config)).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config for access entry");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                system_info_route_text(&translator, "loadAccessEntryFailed"),
            )
        }
    }
}

fn resolve_access_entry_info(config: &Value) -> AccessEntryInfo {
    resolve_access_entry_info_from_sources(
        config,
        resolve_frpc_remote_port(),
        env::var("GO_REPROXY_PORT").ok(),
    )
}

fn resolve_access_entry_info_from_sources(
    config: &Value,
    frpc_remote_port: Option<u16>,
    local_gateway_port: Option<String>,
) -> AccessEntryInfo {
    if is_reverse_proxy_subdomain_mode(config) {
        if let Some(port) = frpc_remote_port {
            return AccessEntryInfo {
                env: "FRP_REMOTE_PORT",
                port: port.to_string(),
                is_default: false,
            };
        }
    }
    resolve_local_gateway_port_from_env(local_gateway_port)
}

pub(crate) fn resolve_public_gateway_port(config: &Value) -> Option<i64> {
    resolve_public_gateway_port_from_sources(
        config,
        resolve_frpc_remote_port(),
        env::var("GO_REPROXY_PORT").ok(),
    )
}

pub(crate) fn resolve_access_entry_port(config: &Value) -> String {
    resolve_access_entry_info(config).port
}

fn resolve_public_gateway_port_from_sources(
    config: &Value,
    frpc_remote_port: Option<u16>,
    local_gateway_port: Option<String>,
) -> Option<i64> {
    parse_public_gateway_port(
        &resolve_access_entry_info_from_sources(config, frpc_remote_port, local_gateway_port).port,
    )
}

fn parse_public_gateway_port(value: &str) -> Option<i64> {
    parse_js_parse_int_radix_10(value.trim_start()).filter(|port| *port > 0)
}

fn resolve_local_gateway_port_from_env(value: Option<String>) -> AccessEntryInfo {
    match value.filter(|value| !value.is_empty()) {
        Some(port) => AccessEntryInfo {
            env: "GO_REPROXY_PORT",
            port,
            is_default: false,
        },
        None => AccessEntryInfo {
            env: "GO_REPROXY_PORT",
            port: "7999".to_string(),
            is_default: true,
        },
    }
}

fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    crate::proxy_utils::is_reverse_proxy_subdomain_mode(config)
}

fn resolve_frpc_remote_port() -> Option<u16> {
    let content = fs::read_to_string(data_dir().join("frp").join("frpc.toml")).ok()?;
    extract_frpc_remote_port(&content)
}

fn extract_frpc_remote_port(content: &str) -> Option<u16> {
    for line in content.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key != "remotePort" && key != "remote_port" {
            continue;
        }
        let value = value.trim();
        if value.is_empty() || value.chars().any(|ch| !ch.is_ascii_digit()) {
            continue;
        }
        let parsed = value.parse::<u32>().ok()?;
        if (1..=65535).contains(&parsed) {
            return Some(parsed as u16);
        }
    }
    None
}

fn parse_js_parse_int_radix_10(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix(value)
}

fn data_dir() -> PathBuf {
    if let Ok(path) = env::var("FN_KNOCK_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    match env::consts::OS {
        "macos" => PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("fn-knock"),
        "linux" => PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("fn-knock"),
        _ => PathBuf::from(home).join(".fn-knock"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_frpc_remote_port_like_node() {
        assert_eq!(extract_frpc_remote_port("remotePort = 12345"), Some(12345));
        assert_eq!(extract_frpc_remote_port("remote_port = 443"), Some(443));
        assert_eq!(extract_frpc_remote_port("remotePort = 70000"), None);
        assert_eq!(extract_frpc_remote_port("remotePort = 443 # nope"), None);
    }

    #[test]
    fn detects_reverse_proxy_subdomain_mode() {
        assert!(is_reverse_proxy_subdomain_mode(&json!({
            "run_type": 1,
            "reverse_proxy_submode": "subdomain"
        })));
        assert!(!is_reverse_proxy_subdomain_mode(&json!({
            "run_type": 3,
            "reverse_proxy_submode": "subdomain"
        })));
    }

    #[test]
    fn resolves_local_gateway_port_with_node_env_truthiness() {
        assert_eq!(
            resolve_local_gateway_port_from_env(None),
            AccessEntryInfo {
                env: "GO_REPROXY_PORT",
                port: "7999".to_string(),
                is_default: true
            }
        );
        assert_eq!(
            resolve_local_gateway_port_from_env(Some(String::new())),
            AccessEntryInfo {
                env: "GO_REPROXY_PORT",
                port: "7999".to_string(),
                is_default: true
            }
        );
        assert_eq!(
            resolve_local_gateway_port_from_env(Some("   ".to_string())),
            AccessEntryInfo {
                env: "GO_REPROXY_PORT",
                port: "   ".to_string(),
                is_default: false
            }
        );
        assert_eq!(
            resolve_local_gateway_port_from_env(Some(" 8000 ".to_string())),
            AccessEntryInfo {
                env: "GO_REPROXY_PORT",
                port: " 8000 ".to_string(),
                is_default: false
            }
        );
    }

    #[test]
    fn resolves_public_gateway_port_from_access_entry_like_node() {
        let direct = json!({ "run_type": 0 });
        assert_eq!(
            resolve_public_gateway_port_from_sources(
                &direct,
                Some(443),
                Some(" 8000x ".to_string())
            ),
            Some(8000)
        );
        assert_eq!(
            resolve_public_gateway_port_from_sources(&direct, None, Some("   ".to_string())),
            None
        );

        let reverse_subdomain = json!({
            "run_type": 1,
            "reverse_proxy_submode": "subdomain"
        });
        assert_eq!(
            resolve_access_entry_info_from_sources(
                &reverse_subdomain,
                Some(443),
                Some("7999".to_string())
            ),
            AccessEntryInfo {
                env: "FRP_REMOTE_PORT",
                port: "443".to_string(),
                is_default: false
            }
        );
        assert_eq!(
            resolve_public_gateway_port_from_sources(
                &reverse_subdomain,
                Some(443),
                Some("7999".to_string())
            ),
            Some(443)
        );
        assert_eq!(
            resolve_public_gateway_port_from_sources(
                &reverse_subdomain,
                None,
                Some("7999".to_string())
            ),
            Some(7999)
        );
    }

    #[test]
    fn localizes_system_info_route_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            system_info_route_text(&zh, "loadAccessEntryFailed"),
            "加载访问入口失败"
        );
    }
}
