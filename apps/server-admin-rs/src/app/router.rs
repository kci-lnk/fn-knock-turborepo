use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::Response as AxumResponse,
};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

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
    deep_monitor::deep_monitor_routes,
    fnos_certificate_sync::fnos_certificate_sync_routes,
    frpc::frpc_routes,
    gateway_logs::gateway_logs_routes,
    gateway_settings::gateway_settings_routes,
    general_blacklist::general_blacklist_routes,
    hmac_auth::hmac_middleware,
    i18n::Translator,
    ip_location::ip_location_routes,
    ip_location_config::ip_location_config_routes,
    ldap_auth::ldap_admin_routes,
    maintenance::maintenance_routes,
    notifications::notification_routes,
    oidc_admin::oidc_admin_routes,
    openapi_docs::openapi_docs_routes,
    proxy_config::proxy_config_routes,
    response,
    runtime_config::{
        auto_https_config_routes, captcha_config_routes, default_route_config_routes,
        firewall_runtime_routes, fnos_connect_waf_routes, fnos_network_tuning_routes,
        fnos_port_icon_hijack_routes, fnos_share_bypass_routes, protocol_mapping_feature_routes,
        proxy_protocol_force_routes, run_mode_prompt_preferences_routes, run_type_config_routes,
        smart_connect_config_routes, sync_routes_config_routes, terminal_feature_routes,
        welcome_guide_routes, wol_feature_config_routes,
    },
    runtime_health::routes::runtime_health_routes,
    runtime_profile,
    scan_assets::scan_asset_routes,
    scanner::{cidr_routes, scanner_routes},
    security_overview::security_overview_routes,
    ssh_security::ssh_security_routes,
    ssl::ssl_routes,
    state::AppState,
    static_files,
    static_files::{admin_static_routes, auth_static_routes},
    system_assets::{smart_connect_asset_routes, system_asset_routes},
    system_events::{admin_event_routes, internal_system_event_routes},
    system_info::system_info_routes,
    terminal::terminal_routes,
    update::update_routes,
    waf::waf_routes,
    whitelist::whitelist_routes,
    wol_routes,
};

pub(super) fn backend_router(state: AppState, protected_admin_view: bool) -> Router {
    let capabilities =
        runtime_profile::get_runtime_capabilities(&runtime_profile::get_runtime_profile(&state));
    backend_router_with_capabilities(state, protected_admin_view, capabilities)
}

fn backend_router_with_capabilities(
    state: AppState,
    protected_admin_view: bool,
    capabilities: runtime_profile::RuntimeCapabilities,
) -> Router {
    // These domains register runtime routes and OpenAPI operations from the same
    // annotated handlers. Remaining domains migrate in similarly scoped batches.
    let dashboard_routes: Router<AppState> = dashboard_routes().into();
    let cidr_routes: Router<AppState> = cidr_routes().into();
    let ip_location_routes: Router<AppState> = ip_location_routes().into();
    let ip_location_config_routes: Router<AppState> = ip_location_config_routes().into();
    let backoff_routes: Router<AppState> = backoff_routes().into();
    let internal_system_event_routes: Router<AppState> = internal_system_event_routes().into();
    let admin_event_routes: Router<AppState> = admin_event_routes().into();
    let runtime_health_routes: Router<AppState> = runtime_health_routes().into();
    let general_blacklist_routes: Router<AppState> = general_blacklist_routes().into();
    let scanner_routes: Router<AppState> = scanner_routes().into();
    let gateway_settings_routes: Router<AppState> = gateway_settings_routes().into();
    let system_info_routes: Router<AppState> = system_info_routes().into();
    let security_overview_routes: Router<AppState> = security_overview_routes().into();
    let update_routes: Router<AppState> = update_routes().into();
    let fnos_port_icon_hijack_routes: Router<AppState> = fnos_port_icon_hijack_routes().into();
    let fnos_network_tuning_routes: Router<AppState> = fnos_network_tuning_routes().into();
    let fnos_share_bypass_routes: Router<AppState> = fnos_share_bypass_routes().into();
    let welcome_guide_routes: Router<AppState> = welcome_guide_routes().into();
    let proxy_protocol_force_routes: Router<AppState> = proxy_protocol_force_routes().into();
    let run_mode_prompt_preferences_routes: Router<AppState> =
        run_mode_prompt_preferences_routes().into();
    let protocol_mapping_feature_routes: Router<AppState> =
        protocol_mapping_feature_routes().into();
    let auto_https_config_routes: Router<AppState> = auto_https_config_routes().into();
    let default_route_config_routes: Router<AppState> = default_route_config_routes().into();
    let captcha_config_routes: Router<AppState> = captcha_config_routes().into();
    let run_type_config_routes: Router<AppState> = run_type_config_routes().into();
    let wol_feature_config_routes: Router<AppState> = wol_feature_config_routes().into();
    let sync_routes_config_routes: Router<AppState> = sync_routes_config_routes().into();
    let mut api = Router::new()
        .route("/api/admin/healthz", axum::routing::get(response::healthz))
        .merge(openapi_docs_routes())
        .merge(admin_routes(protected_admin_view))
        .merge(admin_control_routes())
        .merge(backoff_routes)
        .merge(whitelist_routes())
        .merge(proxy_config_routes())
        .merge(sync_routes_config_routes)
        .merge(fnos_port_icon_hijack_routes)
        .merge(fnos_network_tuning_routes)
        .merge(fnos_share_bypass_routes)
        .merge(welcome_guide_routes)
        .merge(proxy_protocol_force_routes)
        .merge(run_mode_prompt_preferences_routes)
        .merge(protocol_mapping_feature_routes)
        .merge(auto_https_config_routes)
        .merge(default_route_config_routes)
        .merge(captcha_config_routes)
        .merge(run_type_config_routes)
        .merge(wol_feature_config_routes)
        .merge(dashboard_routes)
        .merge(ddns_status_routes())
        .merge(scan_asset_routes())
        .merge(cidr_routes)
        .merge(scanner_routes)
        .merge(security_overview_routes)
        .merge(admin_event_routes)
        .merge(runtime_health_routes)
        .merge(internal_system_event_routes)
        .merge(system_info_routes)
        .merge(system_asset_routes())
        .merge(ssl_routes())
        .merge(general_blacklist_routes)
        .merge(gateway_logs_routes())
        .merge(deep_monitor_routes())
        .merge(gateway_settings_routes)
        .merge(ip_location_routes)
        .merge(ip_location_config_routes)
        .merge(maintenance_routes())
        .merge(notification_routes())
        .merge(oidc_admin_routes())
        .merge(ldap_admin_routes())
        .merge(wol_routes(state.clone()))
        .merge(waf_routes());
    if capabilities.acme_available {
        api = api.merge(acme_routes());
    }
    if capabilities.fnos_certificate_sync_available {
        let fnos_certificate_sync_routes: Router<AppState> = fnos_certificate_sync_routes().into();
        api = api.merge(fnos_certificate_sync_routes);
    }
    if capabilities.host_firewall_available {
        let firewall_runtime_routes: Router<AppState> = firewall_runtime_routes().into();
        api = api.merge(firewall_runtime_routes);
    }
    if capabilities.fnos_connect_waf_available {
        let fnos_connect_waf_routes: Router<AppState> = fnos_connect_waf_routes().into();
        api = api.merge(fnos_connect_waf_routes);
    }
    if capabilities.smart_connect_available {
        let smart_connect_config_routes: Router<AppState> = smart_connect_config_routes().into();
        api = api
            .merge(smart_connect_config_routes)
            .merge(smart_connect_asset_routes());
    }
    if capabilities.terminal_available {
        let terminal_feature_routes: Router<AppState> = terminal_feature_routes().into();
        api = api.merge(terminal_feature_routes).merge(terminal_routes());
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
    api = api.merge(update_routes);
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
    let api = api.layer(middleware::from_fn(admin_same_origin_middleware));
    #[cfg(test)]
    let api = api.layer(middleware::from_fn(route_contract_probe_middleware));

    let router = Router::new()
        .route("/__fn-knock/readyz", axum::routing::get(response::readyz))
        .merge(api)
        .merge(admin_static_routes())
        .fallback(static_files::admin_fallback)
        .layer(response_compression_layer())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(browser_security_headers_middleware));

    router.with_state(state)
}

#[cfg(test)]
async fn route_contract_probe_middleware(req: Request<Body>, next: Next) -> AxumResponse {
    use axum::extract::MatchedPath;

    if req
        .headers()
        .contains_key("x-fn-knock-route-contract-probe")
    {
        let matched_path = req
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str)
            .unwrap_or_default();
        let mut response = AxumResponse::new(Body::empty());
        *response.status_mut() = StatusCode::NO_CONTENT;
        if let Ok(value) = HeaderValue::from_str(matched_path) {
            response
                .headers_mut()
                .insert("x-fn-knock-matched-path", value);
        }
        return response;
    }

    next.run(req).await
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
        ))
        .layer(middleware::from_fn(auth_same_origin_middleware));

    Router::new()
        .merge(api)
        .merge(auth_static_routes())
        .fallback(static_files::auth_fallback)
        .layer(response_compression_layer())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(browser_security_headers_middleware))
        .with_state(state)
}

fn response_compression_layer() -> CompressionLayer {
    // Brotli wins equal-quality negotiation in tower-http, while gzip remains
    // available for older clients and explicit client preference.
    CompressionLayer::new().br(true).gzip(true)
}

async fn browser_security_headers_middleware(req: Request<Body>, next: Next) -> AxumResponse {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("frame-ancestors 'none'"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn auth_same_origin_middleware(req: Request<Body>, next: Next) -> AxumResponse {
    if browser_request_origin_allowed(req.method(), req.headers()) {
        return next.run(req).await;
    }

    response::error(
        StatusCode::FORBIDDEN,
        "Cross-origin authentication request denied",
    )
}

async fn admin_same_origin_middleware(req: Request<Body>, next: Next) -> AxumResponse {
    if browser_request_origin_allowed(req.method(), req.headers()) {
        return next.run(req).await;
    }

    response::error(
        StatusCode::FORBIDDEN,
        "Cross-origin management request denied",
    )
}

fn browser_request_origin_allowed(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }

    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("cross-site"))
    {
        return false;
    }

    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        // Non-browser clients do not necessarily send Origin. Endpoint-level
        // credentials, login proofs and session checks remain authoritative.
        return true;
    };
    let Ok(origin) = url::Url::parse(origin) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https")
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return false;
    }

    let direct_authority = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(direct_authority) = direct_authority else {
        return false;
    };
    // fnOS rewrites Host while proxying /cgi through trim_http_cgi. The admin
    // frontend therefore supplies its browser origin in a non-safelisted
    // header. Cross-origin browser callers cannot attach this header unless a
    // CORS preflight succeeds, and these routers never grant CORS access.
    let cgi_browser_origin_matches = authority_is_loopback(direct_authority)
        && headers
            .get("x-fn-knock-browser-origin")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .is_some_and(|value| value == origin.as_str().trim_end_matches('/'));
    if cgi_browser_origin_matches {
        return true;
    }
    let internal_signed_candidate = authority_is_loopback(direct_authority)
        && ["x-timestamp", "x-nonce", "x-signature"]
            .iter()
            .all(|name| headers.contains_key(*name));
    let authority = if internal_signed_candidate {
        headers
            .get("x-forwarded-host")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(direct_authority)
    } else {
        direct_authority
    };
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| matches!(*value, "http" | "https"))
        .unwrap_or_else(|| origin.scheme());
    let Ok(expected) = url::Url::parse(&format!("{scheme}://{authority}")) else {
        return false;
    };

    origin.scheme().eq_ignore_ascii_case(expected.scheme())
        && origin
            .host_str()
            .zip(expected.host_str())
            .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
        && origin.port_or_known_default() == expected.port_or_known_default()
}

fn authority_is_loopback(authority: &str) -> bool {
    let Ok(authority) = authority.parse::<axum::http::uri::Authority>() else {
        return false;
    };
    let host = authority.host();
    let ip_host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || ip_host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn dynamic_responses_prefer_brotli_and_keep_gzip_fallback() {
        let app = Router::new()
            .route(
                "/compression-probe",
                axum::routing::get(|| async { "compression-probe".repeat(256) }),
            )
            .layer(response_compression_layer());

        let brotli = app
            .clone()
            .oneshot(
                Request::get("/compression-probe")
                    .header(header::ACCEPT_ENCODING, "gzip, br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            brotli.headers().get(header::CONTENT_ENCODING).unwrap(),
            "br"
        );

        let gzip = app
            .oneshot(
                Request::get("/compression-probe")
                    .header(header::ACCEPT_ENCODING, "br;q=0.2, gzip;q=0.9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            gzip.headers().get(header::CONTENT_ENCODING).unwrap(),
            "gzip"
        );
    }

    async fn openwrt_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary OpenWrt router database");
        let mut settings = crate::settings::Settings::from_env();
        settings.runtime_target = "openwrt".to_string();
        settings.admin_view_port = None;
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "openwrt-router-test".to_string();
        settings.hmac_secret = "openwrt-router-hmac-test".to_string();
        settings.altcha_hmac_key = Some("openwrt-router-altcha-test-key".to_string());
        let state = AppState::new(settings)
            .await
            .expect("OpenWrt router test state");
        (directory, state)
    }

    fn materialize_contract_path(path: &str) -> String {
        let mut materialized = String::with_capacity(path.len());
        let mut remaining = path;
        while let Some(start) = remaining.find('{') {
            materialized.push_str(&remaining[..start]);
            let parameter = &remaining[start + 1..];
            let Some(end) = parameter.find('}') else {
                materialized.push_str(&remaining[start..]);
                return materialized;
            };
            materialized.push_str("route-probe");
            remaining = &parameter[end + 1..];
        }
        materialized.push_str(remaining);
        materialized
    }

    async fn auth_router_test_state(hmac_secret: &str) -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary auth router database");
        let mut settings = crate::settings::Settings::from_env();
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "auth-router-test".to_string();
        settings.hmac_secret = hmac_secret.to_string();
        settings.altcha_hmac_key = Some("auth-router-altcha-test-key".to_string());
        let state = AppState::new(settings)
            .await
            .expect("auth router test state");
        (directory, state)
    }

    #[tokio::test]
    async fn auth_router_does_not_expose_hmac_secret_or_permissive_cors() {
        let (_directory, state) = auth_router_test_state("server-only-secret").await;
        let response = auth_router(state)
            .oneshot(
                Request::get("/__fn-knock/runtime-hmac-secret")
                    .header("origin", "https://attacker.invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
    }

    #[tokio::test]
    async fn direct_browser_auth_routes_do_not_require_internal_hmac() {
        let (_directory, state) = auth_router_test_state("").await;
        let app = auth_router(state);
        let bootstrap = app
            .clone()
            .oneshot(
                Request::get("/api/auth/bootstrap?redirect_uri=https%3A%2F%2Ffnos.example.com%2Flogin&_ts=1786274605858")
                    .header(header::HOST, "auth.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bootstrap.status(), StatusCode::OK);
        assert_eq!(
            bootstrap
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("frame-ancestors 'none'")
        );
        assert_eq!(
            bootstrap
                .headers()
                .get(header::X_FRAME_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            bootstrap
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            bootstrap
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );

        let session = app
            .oneshot(
                Request::get("/api/auth/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(session.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn signed_channel_still_fails_closed_without_hmac_secret() {
        let (_directory, state) = auth_router_test_state("").await;
        let response = auth_router(state)
            .oneshot(
                Request::get("/api/auth/session")
                    .header(header::HOST, "127.0.0.1:7997")
                    .header("x-timestamp", crate::time_utils::now_ms().to_string())
                    .header("x-nonce", "0011223344556677")
                    .header("x-signature", "invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn unsigned_loopback_channel_cannot_downgrade_hmac() {
        let (_directory, state) = auth_router_test_state("server-only-secret").await;
        let response = auth_router(state)
            .oneshot(
                Request::get("/api/auth/session")
                    .header(header::HOST, "127.0.0.1:7997")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn valid_loopback_gateway_signature_reaches_auth_handler() {
        const SECRET: &str = "server-only-secret";
        let (_directory, state) = auth_router_test_state(SECRET).await;
        let timestamp = crate::time_utils::now_ms().to_string();
        let nonce = "00112233445566778899aabbccddeeff";
        let request_uri = "/api/auth/bootstrap?_ts=1786274605858";
        let message = format!(
            "fn-knock-v1\nGET\n{request_uri}\n{}\n{timestamp}\n{nonce}",
            crate::crypto_utils::sha256_hex_bytes([])
        );
        let signature = crate::crypto_utils::hmac_sha256_hex(SECRET.as_bytes(), message.as_bytes());

        let response = auth_router(state)
            .oneshot(
                Request::get(request_uri)
                    .header(header::HOST, "127.0.0.1:7997")
                    .header("x-timestamp", timestamp)
                    .header("x-nonce", nonce)
                    .header("x-signature", signature)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_router_rejects_cross_origin_mutations_before_wol_authorization() {
        let (_directory, state) = auth_router_test_state("server-only-secret").await;
        let app = auth_router(state);

        let cross_origin = app
            .clone()
            .oneshot(
                Request::post("/api/auth/wol/targets/device-1/wake")
                    .header(header::HOST, "auth.example.com")
                    .header(header::ORIGIN, "https://attacker.invalid")
                    .header("x-forwarded-proto", "https")
                    .header("sec-fetch-site", "cross-site")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
        assert!(
            cross_origin
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("frame-ancestors 'none'")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::X_FRAME_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );

        let cross_site_without_origin = Request::post("/api/auth/wol/targets/device-1/wake")
            .header(header::HOST, "auth.example.com")
            .header("sec-fetch-site", "cross-site")
            .body(Body::empty())
            .unwrap();
        assert!(!browser_request_origin_allowed(
            cross_site_without_origin.method(),
            cross_site_without_origin.headers()
        ));

        let same_origin = Request::post("/api/auth/wol/targets/device-1/wake")
            .header(header::HOST, "auth.example.com")
            .header(header::ORIGIN, "https://auth.example.com")
            .header("x-forwarded-proto", "https")
            .header("sec-fetch-site", "same-origin")
            .body(Body::empty())
            .unwrap();
        assert!(browser_request_origin_allowed(
            same_origin.method(),
            same_origin.headers()
        ));

        let forged_forwarded_host = Request::post("/api/auth/wol/targets/device-1/wake")
            .header(header::HOST, "auth.example.com")
            .header(header::ORIGIN, "https://attacker.invalid")
            .header("x-forwarded-host", "attacker.invalid")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .unwrap();
        assert!(!browser_request_origin_allowed(
            forged_forwarded_host.method(),
            forged_forwarded_host.headers()
        ));

        let signed_loopback_proxy = Request::post("/api/auth/wol/targets/device-1/wake")
            .header(header::HOST, "127.0.0.1:7997")
            .header(header::ORIGIN, "https://auth.example.com")
            .header("x-forwarded-host", "auth.example.com")
            .header("x-forwarded-proto", "https")
            .header("x-timestamp", "1786274605858")
            .header("x-nonce", "0011223344556677")
            .header("x-signature", "validated-by-inner-middleware")
            .body(Body::empty())
            .unwrap();
        assert!(browser_request_origin_allowed(
            signed_loopback_proxy.method(),
            signed_loopback_proxy.headers()
        ));
    }

    #[tokio::test]
    async fn backend_router_rejects_cross_origin_mutations_and_sets_browser_headers() {
        let (_directory, state) = openwrt_test_state().await;
        let app = backend_router(state, false);

        let cross_origin = app
            .clone()
            .oneshot(
                Request::post("/api/admin/config")
                    .header(header::HOST, "admin.example.com")
                    .header(header::ORIGIN, "https://attacker.invalid")
                    .header("sec-fetch-site", "cross-site")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
        assert!(
            cross_origin
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("frame-ancestors 'none'")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::X_FRAME_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            cross_origin
                .headers()
                .get(header::REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );

        let same_origin = Request::post("/api/admin/config")
            .header(header::HOST, "admin.example.com")
            .header(header::ORIGIN, "https://admin.example.com")
            .header("x-forwarded-proto", "https")
            .header("sec-fetch-site", "same-origin")
            .body(Body::empty())
            .unwrap();
        assert!(browser_request_origin_allowed(
            same_origin.method(),
            same_origin.headers()
        ));

        let fnos_cgi_same_origin = Request::post("/api/admin/config")
            .header(header::HOST, "127.0.0.1:7998")
            .header(header::ORIGIN, "http://192.168.31.98:19122")
            .header("x-fn-knock-browser-origin", "http://192.168.31.98:19122")
            .body(Body::empty())
            .unwrap();
        assert!(browser_request_origin_allowed(
            fnos_cgi_same_origin.method(),
            fnos_cgi_same_origin.headers()
        ));

        let forged_cgi_origin = Request::post("/api/admin/config")
            .header(header::HOST, "127.0.0.1:7998")
            .header(header::ORIGIN, "https://attacker.invalid")
            .header("x-fn-knock-browser-origin", "https://admin.example.com")
            .body(Body::empty())
            .unwrap();
        assert!(!browser_request_origin_allowed(
            forged_cgi_origin.method(),
            forged_cgi_origin.headers()
        ));

        let cross_site_cgi_origin = Request::post("/api/admin/config")
            .header(header::HOST, "127.0.0.1:7998")
            .header(header::ORIGIN, "https://attacker.invalid")
            .header("x-fn-knock-browser-origin", "https://attacker.invalid")
            .header("sec-fetch-site", "cross-site")
            .body(Body::empty())
            .unwrap();
        assert!(!browser_request_origin_allowed(
            cross_site_cgi_origin.method(),
            cross_site_cgi_origin.headers()
        ));
    }

    #[tokio::test]
    async fn every_openapi_operation_matches_a_real_axum_route() {
        let (_directory, state) = openwrt_test_state().await;
        let mut capabilities = runtime_profile::get_runtime_capabilities(
            &runtime_profile::get_runtime_profile(&state),
        );
        capabilities.acme_available = true;
        capabilities.fnos_certificate_sync_available = true;
        capabilities.host_firewall_available = true;
        capabilities.fnos_connect_waf_available = true;
        capabilities.smart_connect_available = true;
        capabilities.terminal_available = true;
        capabilities.ssh_security_available = true;
        capabilities.cloudflared_available = true;
        capabilities.frpc_available = true;
        let app = backend_router_with_capabilities(state, false, capabilities);
        let document = crate::openapi_docs::build_openapi_document();
        let paths = document["paths"]
            .as_object()
            .expect("OpenAPI paths should be an object");
        let methods = ["get", "post", "put", "patch", "delete", "head", "options"];
        let mut checked = 0usize;

        for (contract_path, path_item) in paths {
            let operations = path_item
                .as_object()
                .expect("OpenAPI path item should be an object");
            for method in methods {
                if !operations.contains_key(method) {
                    continue;
                }
                let request_path = materialize_contract_path(contract_path);
                let request = Request::builder()
                    .method(Method::from_bytes(method.as_bytes()).expect("valid HTTP method"))
                    .uri(&request_path)
                    .header("x-fn-knock-route-contract-probe", "1")
                    .body(Body::empty())
                    .expect("route contract probe request");
                let response = app
                    .clone()
                    .oneshot(request)
                    .await
                    .expect("route contract probe response");
                let matched_path = response
                    .headers()
                    .get("x-fn-knock-matched-path")
                    .and_then(|value| value.to_str().ok());
                assert_eq!(
                    matched_path,
                    Some(contract_path.as_str()),
                    "{method} {contract_path} does not match a registered Axum route"
                );
                checked += 1;
            }
        }

        assert_eq!(checked, 416, "all OpenAPI operations should be probed");
    }

    #[tokio::test]
    async fn openwrt_does_not_register_firewall_smart_connect_ssh_or_terminal_routes() {
        let (_directory, state) = openwrt_test_state().await;
        let app = backend_router(state, false);
        let unsupported_routes = [
            (Method::POST, "/api/admin/firewall/clear"),
            (Method::GET, "/api/admin/config/firewall_additional_ports"),
            (Method::POST, "/api/admin/config/firewall_additional_ports"),
            (Method::GET, "/api/admin/config/fnos_connect_waf"),
            (Method::GET, "/api/admin/config/smart_connect/details"),
            (Method::GET, "/api/admin/system/dnsmasq/status"),
            (Method::GET, "/api/admin/ssh-security/config"),
            (Method::GET, "/api/admin/config/terminal_feature"),
            (Method::GET, "/api/admin/terminal/status"),
        ];

        for (method, path) in unsupported_routes {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .expect("OpenWrt unsupported route request"),
                )
                .await
                .expect("OpenWrt unsupported route response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn unprotected_backend_deep_monitor_does_not_require_panel_login() {
        let (_directory, state) = openwrt_test_state().await;
        let response = backend_router(state, false)
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/admin/deep-monitor/sessions")
                    .body(Body::empty())
                    .expect("deep monitor request"),
            )
            .await
            .expect("deep monitor response");

        // The unprotected backend is reached through platform-controlled
        // ingress such as the fnOS CGI. It must reach the Go backend instead
        // of requiring the Docker admin-panel cookie locally.
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            response.headers().get("x-fn-knock-admin-auth").is_none(),
            "deep monitor route unexpectedly required panel authentication"
        );
    }
}
