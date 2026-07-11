use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use super::docker_admin_view;
use crate::{
    acme::acme_routes,
    admin_control::admin_control_routes,
    admin_panel::{admin_auth_middleware, admin_routes},
    auth::auth_api_routes,
    backoff::backoff_routes,
    cloudflared::cloudflared_routes,
    dashboard::dashboard_routes,
    ddns_status::ddns_status_routes,
    frpc::frpc_routes,
    gateway_logs::gateway_logs_routes,
    gateway_settings::gateway_settings_routes,
    general_blacklist::general_blacklist_routes,
    hmac_auth::hmac_middleware,
    i18n::Translator,
    ip_location::ip_location_routes,
    ip_location_config::ip_location_config_routes,
    maintenance::maintenance_routes,
    notifications::notification_routes,
    oidc_admin::oidc_admin_routes,
    openapi_docs::openapi_docs_routes,
    proxy_config::proxy_config_routes,
    response,
    runtime_config::runtime_config_routes,
    runtime_profile,
    scan_assets::scan_asset_routes,
    scanner::{cidr_routes, scanner_routes},
    security_overview::security_overview_routes,
    ssh_security::ssh_security_routes,
    ssl::ssl_routes,
    state::AppState,
    static_files,
    static_files::{admin_static_routes, auth_static_routes},
    system_assets::system_asset_routes,
    system_events::{admin_event_routes, internal_system_event_routes},
    system_info::system_info_routes,
    terminal::terminal_routes,
    update::update_routes,
    waf::waf_routes,
    whitelist::whitelist_routes,
};

pub(super) fn backend_router(state: AppState, protected_admin_view: bool) -> Router {
    let capabilities =
        runtime_profile::get_runtime_capabilities(&runtime_profile::get_runtime_profile(&state));
    let mut api = Router::new()
        .route("/api/admin/healthz", axum::routing::get(response::healthz))
        .merge(openapi_docs_routes())
        .merge(admin_routes(protected_admin_view))
        .merge(admin_control_routes())
        .merge(backoff_routes())
        .merge(whitelist_routes())
        .merge(proxy_config_routes())
        .merge(runtime_config_routes())
        .merge(dashboard_routes())
        .merge(ddns_status_routes())
        .merge(scan_asset_routes())
        .merge(cidr_routes())
        .merge(scanner_routes())
        .merge(security_overview_routes())
        .merge(admin_event_routes())
        .merge(internal_system_event_routes())
        .merge(system_info_routes())
        .merge(system_asset_routes())
        .merge(ssl_routes())
        .merge(general_blacklist_routes())
        .merge(gateway_logs_routes())
        .merge(gateway_settings_routes())
        .merge(ip_location_routes())
        .merge(ip_location_config_routes())
        .merge(maintenance_routes())
        .merge(notification_routes())
        .merge(oidc_admin_routes())
        .merge(waf_routes());
    if capabilities.acme_available {
        api = api.merge(acme_routes());
    }
    if capabilities.terminal_available {
        api = api.merge(terminal_routes());
    }
    if capabilities.ssh_security_available {
        api = api.merge(ssh_security_routes());
    }
    if capabilities.cloudflared_available {
        api = api.merge(cloudflared_routes());
    }
    if capabilities.frpc_available {
        api = api.merge(frpc_routes());
    }
    if !capabilities.desktop_update_managed {
        api = api.merge(update_routes());
    }
    let api = api.fallback(api_not_found);
    let api = if protected_admin_view {
        api.layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            docker_admin_view::admin_view_ingress_middleware,
        ))
    } else if runtime_profile::admin_panel_protected_runtime(&state)
        && state.settings.admin_view_port.is_some()
    {
        api.layer(middleware::from_fn_with_state(
            state.clone(),
            docker_admin_view::admin_backend_proxy_middleware,
        ))
    } else {
        api
    };

    let router = Router::new()
        .route("/__fn-knock/readyz", axum::routing::get(response::readyz))
        .merge(api)
        .merge(admin_static_routes())
        .fallback(static_files::admin_fallback)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    if runtime_profile::deployment_target(&state) == "windows" {
        let expected_port = if protected_admin_view {
            state.settings.admin_view_port.unwrap_or(7991)
        } else {
            state.settings.backend_port
        };
        router
            .layer(middleware::from_fn_with_state(
                WindowsAdminSecurityState { expected_port },
                windows_admin_origin_middleware,
            ))
            .with_state(state)
    } else {
        router.layer(CorsLayer::permissive()).with_state(state)
    }
}

#[derive(Clone)]
struct WindowsAdminSecurityState {
    expected_port: u16,
}

async fn windows_admin_origin_middleware(
    State(security): State<WindowsAdminSecurityState>,
    req: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let expected_port = security.expected_port;
    let authority = req
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_loopback_authority(value, expected_port));
    let origin_valid = match req.headers().get(header::ORIGIN) {
        None => true,
        Some(value) => value.to_str().ok().is_some_and(|value| {
            authority
                .as_ref()
                .is_some_and(|authority| loopback_origin_matches(value, authority))
        }),
    };
    let fetch_site_valid = match req.headers().get("sec-fetch-site") {
        None => true,
        Some(value) => value
            .to_str()
            .ok()
            .is_some_and(|value| matches!(value.trim(), "same-origin" | "none")),
    };

    // Browser top-level navigation and the local ready probe normally omit
    // Origin, so absence remains valid. When Origin is present, bind it to the
    // exact Host authority: localhost and 127.0.0.1 are both loopback, but are
    // intentionally different web origins.
    if authority.is_some() && origin_valid && fetch_site_valid {
        next.run(req).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoopbackAuthority {
    host: String,
    port: u16,
}

fn loopback_origin_matches(value: &str, authority: &LoopbackAuthority) -> bool {
    let Ok(url) = url::Url::parse(value.trim()) else {
        return false;
    };
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port_or_known_default() == Some(authority.port)
        && url.host_str().is_some_and(|host| {
            normalize_loopback_host(host).as_deref() == Some(authority.host.as_str())
        })
}

fn parse_loopback_authority(value: &str, expected_port: u16) -> Option<LoopbackAuthority> {
    let Ok(authority) = value.trim().parse::<http::uri::Authority>() else {
        return None;
    };
    let port = authority
        .port_u16()
        .or_else(|| (expected_port == 80).then_some(80))?;
    if port != expected_port {
        return None;
    }
    Some(LoopbackAuthority {
        host: normalize_loopback_host(authority.host())?,
        port,
    })
}

fn normalize_loopback_host(host: &str) -> Option<String> {
    let normalized = host.trim_matches(['[', ']']).trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "localhost" | "127.0.0.1" | "::1").then_some(normalized)
}

async fn api_not_found(State(state): State<AppState>) -> axum::response::Response {
    let translator = Translator::from_state(&state).await;
    response::error(
        axum::http::StatusCode::NOT_FOUND,
        translator.t("server.apiPathNotFound"),
    )
}

pub(super) fn auth_router(state: AppState) -> Router {
    let api = Router::new()
        .nest("/api/auth", auth_api_routes())
        .nest("/auth/api/auth", auth_api_routes())
        .nest("/__auth__/api/auth", auth_api_routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            hmac_middleware,
        ));

    Router::new()
        .merge(api)
        .merge(auth_static_routes())
        .fallback(static_files::auth_fallback)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_admin_authority_requires_loopback_and_expected_port() {
        assert_eq!(
            parse_loopback_authority("127.0.0.1:7991", 7991),
            Some(LoopbackAuthority {
                host: "127.0.0.1".to_string(),
                port: 7991,
            })
        );
        assert!(parse_loopback_authority("localhost:7991", 7991).is_some());
        assert!(parse_loopback_authority("[::1]:7991", 7991).is_some());
        assert!(parse_loopback_authority("localhost", 7991).is_none());
        assert!(parse_loopback_authority("localhost:7998", 7991).is_none());
        assert!(parse_loopback_authority("evil.example:7991", 7991).is_none());
    }

    #[test]
    fn windows_admin_origin_must_match_host_authority_exactly() {
        let localhost = parse_loopback_authority("localhost:7991", 7991).unwrap();
        let ipv4 = parse_loopback_authority("127.0.0.1:7991", 7991).unwrap();
        let ipv6 = parse_loopback_authority("[::1]:7991", 7991).unwrap();

        assert!(loopback_origin_matches("http://localhost:7991", &localhost));
        assert!(loopback_origin_matches("http://127.0.0.1:7991", &ipv4));
        assert!(loopback_origin_matches("http://[::1]:7991", &ipv6));
        assert!(!loopback_origin_matches(
            "http://127.0.0.1:7991",
            &localhost
        ));
        assert!(!loopback_origin_matches("http://localhost:7991", &ipv4));
        assert!(!loopback_origin_matches(
            "https://localhost:7991",
            &localhost
        ));
        assert!(!loopback_origin_matches(
            "http://localhost:7998",
            &localhost
        ));
        assert!(!loopback_origin_matches(
            "http://localhost:7991/path",
            &localhost
        ));
        assert!(!loopback_origin_matches("null", &localhost));
    }
}
