use std::{
    env,
    net::{IpAddr, SocketAddr},
};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode, header},
    middleware::Next,
};
use ipnet::IpNet;
use subtle::ConstantTimeEq;

use crate::{
    admin_panel::resolve_panel_auth_context, http_utils, i18n::Translator, response,
    state::AppState,
};

const DOCKER_ADMIN_PROXY_HEADER_NAME: &str = "x-fn-knock-admin-proxy";
const DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME: &str = "x-fn-knock-docker-discover-ip";
const UPSTREAM_PRIVATE_IPV4_HEADER_NAME: &str = "x-reauth-upstream-private-ipv4";

pub(super) async fn admin_backend_proxy_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let path = req.uri().path();
    if !is_docker_admin_protected_path(path) {
        return next.run(req).await;
    }

    if is_docker_admin_backend_proxy_required(
        path,
        req.headers(),
        &state.settings.admin_proxy_secret,
    ) {
        let translator = Translator::from_state(&state).await;
        let mut response = response::error(
            StatusCode::FORBIDDEN,
            translator.t_params(
                "server.dockerAdminProxyRequired",
                &[(
                    "port",
                    state.settings.admin_view_port.unwrap_or(7991).to_string(),
                )],
            ),
        );
        apply_no_store_header(&mut response);
        return response;
    }

    if !is_docker_admin_backend_auth_required(path) {
        let mut response = next.run(req).await;
        apply_no_store_header(&mut response);
        return response;
    }

    match resolve_panel_auth_context(&state, req.headers()).await {
        Ok(context)
            if context
                .get("authenticated")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false) =>
        {
            let mut response = next.run(req).await;
            apply_no_store_header(&mut response);
            response
        }
        Ok(_) => {
            let translator = Translator::from_state(&state).await;
            let mut response = response::error(
                StatusCode::UNAUTHORIZED,
                translator.t("server.dockerAdminLoginRequired"),
            );
            apply_no_store_header(&mut response);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "failed to resolve docker admin backend auth context");
            let translator = Translator::from_state(&state).await;
            let mut response = response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.admin.adminPanelRoutes.resolveAuthFailed"),
            );
            apply_no_store_header(&mut response);
            response
        }
    }
}

fn apply_no_store_header(response: &mut axum::response::Response) {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
}

fn is_docker_admin_backend_proxy_required(path: &str, headers: &HeaderMap, secret: &str) -> bool {
    is_docker_admin_protected_path(path) && !is_docker_admin_proxy_request(headers, secret)
}

fn is_docker_admin_backend_auth_required(path: &str) -> bool {
    is_docker_admin_protected_path(path) && !is_docker_admin_public_path(path)
}

fn is_docker_admin_proxy_request(headers: &HeaderMap, secret: &str) -> bool {
    let header_value = headers
        .get(DOCKER_ADMIN_PROXY_HEADER_NAME)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    safe_equal_string(header_value, secret.trim())
}

fn safe_equal_string(left: &str, right: &str) -> bool {
    !left.is_empty() && left.len() == right.len() && left.as_bytes().ct_eq(right.as_bytes()).into()
}

fn is_docker_admin_public_path(path: &str) -> bool {
    matches!(
        path,
        "/api/admin/healthz"
            | "/api/admin/panel/bootstrap"
            | "/api/admin/panel/login"
            | "/api/admin/panel/password"
            | "/api/admin/panel/logout"
    )
}

fn is_docker_admin_protected_path(path: &str) -> bool {
    path.starts_with("/api/admin")
        || path == "/docs"
        || path.starts_with("/docs/")
        || path.starts_with("/swagger-ui")
}

pub(super) async fn admin_view_ingress_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let socket_ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_default();
    let access = resolve_admin_view_ingress(req.headers(), &socket_ip);
    if access.trusted_ingress {
        apply_admin_view_forwarded_headers(req.headers_mut(), &access);
        return next.run(req).await;
    }

    let translator = Translator::from_state(&state).await;
    let locale = translator.locale().to_string();
    let client_ip = if access.socket_ip.is_empty() {
        access.client_ip
    } else {
        access.socket_ip
    };
    let accepts_json = req.uri().path().starts_with("/api/")
        || req
            .headers()
            .get(header::ACCEPT)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().contains("application/json"));
    if accepts_json {
        let mut response = response::error(
            StatusCode::FORBIDDEN,
            translator.t("server.dockerAdminDenied"),
        );
        response.headers_mut().insert(
            header::CONTENT_LANGUAGE,
            axum::http::HeaderValue::from_str(&locale)
                .unwrap_or_else(|_| axum::http::HeaderValue::from_static("zh-CN")),
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        );
        response.headers_mut().insert(
            header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        );
        return response;
    }

    let body = build_docker_admin_denied_html(&translator, &client_ip);
    let mut response = axum::response::Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CONTENT_LANGUAGE, locale)
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'",
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::X_FRAME_OPTIONS, "DENY")
        .body(Body::from(body))
        .unwrap_or_else(|_| axum::response::Response::new(Body::empty()));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdminViewIngress {
    socket_ip: String,
    forwarded_ip: String,
    client_ip: String,
    trusted_ingress: bool,
    via_forwarded_headers: bool,
}

fn resolve_admin_view_ingress(headers: &HeaderMap, socket_ip: &str) -> AdminViewIngress {
    let socket_ip = http_utils::normalize_ip(socket_ip);
    let forwarded_ip = admin_view_forwarded_ip(headers);
    let trusted_ingress = is_trusted_admin_view_ingress_ip(&socket_ip);
    let via_forwarded_headers = trusted_ingress && !forwarded_ip.is_empty();
    let client_ip = if via_forwarded_headers {
        forwarded_ip.clone()
    } else if !socket_ip.is_empty() {
        socket_ip.clone()
    } else {
        forwarded_ip.clone()
    };

    AdminViewIngress {
        socket_ip,
        forwarded_ip,
        client_ip,
        trusted_ingress,
        via_forwarded_headers,
    }
}

fn admin_view_forwarded_ip(headers: &HeaderMap) -> String {
    for name in [
        "eo-connecting-ip",
        "ali-real-client-ip",
        "x-forwarded-for",
        "x-real-ip",
    ] {
        if let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) {
            let first = value.split(',').next().unwrap_or("").trim();
            let normalized = http_utils::normalize_ip(first);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }
    String::new()
}

fn apply_admin_view_forwarded_headers(headers: &mut HeaderMap, access: &AdminViewIngress) {
    if !access.client_ip.is_empty()
        && let Ok(value) = axum::http::HeaderValue::from_str(&access.client_ip)
    {
        headers.insert("x-forwarded-for", value.clone());
        headers.insert("x-real-ip", value);
    }

    if access.via_forwarded_headers {
        let discover_ip = headers
            .get(UPSTREAM_PRIVATE_IPV4_HEADER_NAME)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .map(http_utils::normalize_ip)
            .filter(|value| is_private_ipv4(value))
            .unwrap_or_default();
        if !discover_ip.is_empty()
            && let Ok(value) = axum::http::HeaderValue::from_str(&discover_ip)
        {
            headers.insert(DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME, value);
        }
    }
}

fn is_private_ipv4(value: &str) -> bool {
    matches!(
        http_utils::normalize_ip(value).parse::<IpAddr>(),
        Ok(IpAddr::V4(_))
    ) && http_utils::is_private_or_local_ip(value)
}

fn is_trusted_admin_view_ingress_ip(ip: &str) -> bool {
    let normalized = http_utils::normalize_ip(ip);
    if normalized.is_empty() {
        return false;
    }
    if http_utils::is_private_or_local_ip(&normalized) {
        return true;
    }
    let Ok(parsed_ip) = normalized.parse::<IpAddr>() else {
        return false;
    };
    trusted_admin_proxy_cidrs()
        .iter()
        .any(|network| network.contains(&parsed_ip))
}

fn trusted_admin_proxy_cidrs() -> Vec<IpNet> {
    env::var("DOCKER_ADMIN_TRUSTED_PROXY_CIDRS")
        .unwrap_or_default()
        .split(|value: char| value.is_whitespace() || value == ',')
        .filter_map(normalize_trusted_proxy_entry)
        .collect()
}

fn normalize_trusted_proxy_entry(value: &str) -> Option<IpNet> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some((address, prefix)) = raw.split_once('/') {
        let normalized = http_utils::normalize_ip(address);
        let prefix = prefix.trim().parse::<u8>().ok()?;
        return format!("{normalized}/{prefix}").parse::<IpNet>().ok();
    }
    let normalized = http_utils::normalize_ip(raw);
    let ip = normalized.parse::<IpAddr>().ok()?;
    Some(IpNet::from(ip))
}

fn build_docker_admin_denied_html(translator: &Translator, client_ip: &str) -> String {
    let locale = html_escape(translator.locale());
    let title = html_escape(&translator.t("server.dockerAdminDeniedTitle"));
    let description = html_escape(&translator.t("server.dockerAdminDeniedDescription"));
    let current_ip = html_escape(&translator.t_params(
        "server.dockerAdminCurrentIp",
        &[(
            "ip",
            if client_ip.trim().is_empty() {
                "unknown".to_string()
            } else {
                client_ip.to_string()
            },
        )],
    ));
    format!(
        r#"<!doctype html>
<html lang="{locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <style>
      :root {{ color-scheme: light; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
      body {{ margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center; background: #f5f7fb; color: #111827; }}
      .card {{ width: min(92vw, 420px); border: 1px solid rgba(15, 23, 42, 0.08); border-radius: 20px; background: rgba(255, 255, 255, 0.94); box-shadow: 0 22px 60px rgba(15, 23, 42, 0.12); padding: 28px 24px; }}
      .badge {{ display: inline-flex; align-items: center; justify-content: center; width: 48px; height: 48px; border-radius: 999px; background: rgba(239, 68, 68, 0.12); color: #dc2626; font-size: 22px; font-weight: 700; }}
      h1 {{ margin: 18px 0 10px; font-size: 24px; }}
      p {{ margin: 0; line-height: 1.7; color: #475569; }}
      .meta {{ margin-top: 18px; padding: 12px 14px; border-radius: 14px; background: #f8fafc; color: #334155; font-size: 14px; }}
    </style>
  </head>
  <body>
    <section class="card">
      <div class="badge">!</div>
      <h1>{title}</h1>
      <p>{description}</p>
      <div class="meta">{current_ip}</div>
    </section>
  </body>
</html>"#
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn normalizes_admin_view_trusted_proxy_entries() {
        assert_eq!(
            normalize_trusted_proxy_entry("203.0.113.10")
                .map(|network| network.to_string())
                .as_deref(),
            Some("203.0.113.10/32")
        );
        assert_eq!(
            normalize_trusted_proxy_entry("2001:db8::1")
                .map(|network| network.to_string())
                .as_deref(),
            Some("2001:db8::1/128")
        );
        assert!(normalize_trusted_proxy_entry("203.0.113.10/99").is_none());
        assert!(normalize_trusted_proxy_entry("not-an-ip").is_none());
    }

    #[test]
    fn admin_view_ingress_uses_forwarded_ip_only_for_trusted_socket() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.20, 10.0.0.2"),
        );

        let private_socket = resolve_admin_view_ingress(&headers, "192.168.1.10");
        assert!(private_socket.trusted_ingress);
        assert!(private_socket.via_forwarded_headers);
        assert_eq!(private_socket.client_ip, "198.51.100.20");

        let public_socket = resolve_admin_view_ingress(&headers, "198.51.100.10");
        assert!(!public_socket.trusted_ingress);
        assert!(!public_socket.via_forwarded_headers);
        assert_eq!(public_socket.client_ip, "198.51.100.10");
    }

    #[test]
    fn escapes_docker_admin_denied_html_values() {
        assert_eq!(
            html_escape("<tag attr=\"x\">'&"),
            "&lt;tag attr=&quot;x&quot;&gt;&#39;&amp;"
        );
    }

    #[test]
    fn docker_admin_backend_proxy_checks_match_node_paths() {
        let mut headers = HeaderMap::new();
        headers.insert(
            DOCKER_ADMIN_PROXY_HEADER_NAME,
            HeaderValue::from_static("secret"),
        );

        assert!(is_docker_admin_proxy_request(&headers, "secret"));
        assert!(!is_docker_admin_proxy_request(&headers, "other"));
        assert!(is_docker_admin_public_path("/api/admin/panel/bootstrap"));
        assert!(!is_docker_admin_public_path("/api/admin/config"));
        assert!(is_docker_admin_protected_path("/api/admin/config"));
        assert!(is_docker_admin_protected_path("/docs/json"));
        assert!(is_docker_admin_protected_path("/swagger-ui"));
        assert!(!is_docker_admin_protected_path("/api/auth/bootstrap"));

        assert!(is_docker_admin_backend_proxy_required(
            "/api/admin/panel/login",
            &HeaderMap::new(),
            "secret"
        ));
        assert!(!is_docker_admin_backend_proxy_required(
            "/api/admin/panel/login",
            &headers,
            "secret"
        ));
        assert!(!is_docker_admin_backend_auth_required(
            "/api/admin/panel/login"
        ));
        assert!(is_docker_admin_backend_auth_required("/api/admin/config"));
    }

    #[test]
    fn admin_view_forwarded_headers_expose_client_and_discover_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            UPSTREAM_PRIVATE_IPV4_HEADER_NAME,
            HeaderValue::from_static("192.168.31.98"),
        );
        let access = AdminViewIngress {
            socket_ip: "10.0.0.2".to_string(),
            forwarded_ip: "198.51.100.20".to_string(),
            client_ip: "198.51.100.20".to_string(),
            trusted_ingress: true,
            via_forwarded_headers: true,
        };

        apply_admin_view_forwarded_headers(&mut headers, &access);

        assert_eq!(
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok()),
            Some("198.51.100.20")
        );
        assert_eq!(
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok()),
            Some("198.51.100.20")
        );
        assert_eq!(
            headers
                .get(DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME)
                .and_then(|value| value.to_str().ok()),
            Some("192.168.31.98")
        );
    }
}
