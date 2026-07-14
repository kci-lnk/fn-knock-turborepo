use axum::{Router, extract::State, middleware};
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
    fnos_certificate_sync::fnos_certificate_sync_routes,
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
    if capabilities.fnos_certificate_sync_available {
        api = api.merge(fnos_certificate_sync_routes());
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
    api = api.merge(update_routes());
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

    router.layer(CorsLayer::permissive()).with_state(state)
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
