mod acme;
mod admin_control;
mod admin_panel;
mod app_version;
mod auth;
mod auth_mobility;
mod auto_https;
mod backoff;
mod cloudflared;
mod common_auth_locations;
mod cookies;
mod dashboard;
mod ddns_status;
mod fnos_share_bypass;
mod frpc;
mod gateway_logs;
mod gateway_settings;
mod general_blacklist;
mod go_backend;
mod hmac_auth;
mod http_utils;
mod i18n;
mod ip_location;
mod ip_location_config;
mod maintenance;
mod notifications;
mod oidc_admin;
mod oidc_runtime;
mod openapi_docs;
mod passkey_runtime;
mod proxy_config;
mod redis_store;
mod response;
mod runtime_config;
mod runtime_profile;
mod scan_assets;
mod scanner;
mod security_overview;
mod settings;
mod ssh_security;
mod ssl;
mod state;
mod static_files;
mod system_assets;
mod system_events;
mod system_info;
mod system_monitor;
mod terminal;
mod terminal_paths;
mod time_utils;
mod update;
mod waf;
mod whitelist;

use std::{
    env, fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
};
use ipnet::IpNet;
use serde_json::json;
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    acme::{acme_routes, start_acme_tasks},
    admin_control::admin_control_routes,
    admin_panel::{
        admin_auth_middleware, admin_routes, normalize_locale_config, resolve_panel_auth_context,
    },
    auth::auth_api_routes,
    auth_mobility::start_auth_mobility_tasks,
    auto_https::sync_auto_https_on_boot,
    backoff::backoff_routes,
    cloudflared::{cloudflared_routes, start_cloudflared_tasks},
    common_auth_locations::start_common_auth_location_tasks,
    dashboard::{dashboard_routes, start_traffic_tasks},
    ddns_status::{ddns_status_routes, start_ddns_tasks},
    frpc::{frpc_routes, start_frpc_tasks},
    gateway_logs::gateway_logs_routes,
    gateway_settings::{gateway_settings_routes, sync_gateway_settings_on_boot},
    general_blacklist::general_blacklist_routes,
    hmac_auth::hmac_middleware,
    i18n::{DEFAULT_LOCALE, Translator},
    ip_location::{ip_location_routes, start_ip_location_worker},
    ip_location_config::ip_location_config_routes,
    maintenance::maintenance_routes,
    notifications::{notification_routes, start_notification_tasks},
    oidc_admin::oidc_admin_routes,
    openapi_docs::openapi_docs_routes,
    proxy_config::proxy_config_routes,
    redis_store::RedisStore,
    runtime_config::{runtime_config_routes, sync_runtime_config_on_boot},
    scan_assets::scan_asset_routes,
    scanner::{cidr_routes, scanner_routes},
    security_overview::security_overview_routes,
    settings::Settings,
    ssh_security::{ssh_security_routes, start_ssh_security_tasks},
    ssl::{ssl_routes, sync_ssl_deployment_to_gateway},
    state::AppState,
    static_files::{admin_static_routes, auth_static_routes},
    system_assets::{start_system_clock_tasks, system_asset_routes},
    system_events::{admin_event_routes, internal_system_event_routes},
    system_info::system_info_routes,
    system_monitor::start_system_monitor_tasks,
    terminal::{start_terminal_tasks, terminal_routes},
    update::{start_update_tasks, update_routes},
    waf::{start_waf_tasks, waf_routes},
    whitelist::{start_whitelist_tasks, whitelist_routes},
};

const CLEAN_SCRIPT_CONTENT: &str = r#"#!/bin/bash

CHAINS=("FN-KNOCK-FW" "FN-KNOCK-SSH")
PARENTS=("INPUT" "DOCKER-USER")
TABLES=("iptables" "ip6tables")

remove_parent_jumps() {
    local cmd="$1"
    local parent="$2"
    local chain="$3"

    if ! "$cmd" -L "$parent" -n >/dev/null 2>&1; then
        return
    fi

    while IFS= read -r line; do
        [[ "$line" == "-A $parent "* ]] || continue
        [[ "$line" == *" -j $chain"* ]] || continue

        local rule_args="${line#-A $parent }"
        # shellcheck disable=SC2086
        if "$cmd" -D "$parent" $rule_args 2>/dev/null; then
            echo "Removed jump rule from $parent -> $chain: $rule_args"
        fi
    done < <("$cmd" -S "$parent" 2>/dev/null || true)

    while "$cmd" -D "$parent" -j "$chain" 2>/dev/null; do
        echo "Removed legacy jump rule from $parent -> $chain"
    done
}

echo "Starting firewall cleanup for chains: ${CHAINS[*]}..."

for cmd in "${TABLES[@]}"; do
    if ! command -v "$cmd" &> /dev/null; then
        echo "$cmd is not installed or not in PATH, skipping..."
        continue
    fi

    echo "--- Processing $cmd ---"

    for chain in "${CHAINS[@]}"; do
        for parent in "${PARENTS[@]}"; do
            remove_parent_jumps "$cmd" "$parent" "$chain"
        done

        if "$cmd" -L "$chain" -n >/dev/null 2>&1; then
            "$cmd" -F "$chain"
            echo "Flushed all rules inside $chain"

            "$cmd" -X "$chain"
            echo "Deleted custom chain $chain"
        else
            echo "Chain $chain does not exist in $cmd (already clean)."
        fi
    done
done

echo "Cleanup complete!"
"#;
const DOCKER_ADMIN_PROXY_HEADER_NAME: &str = "x-fn-knock-admin-proxy";
const DOCKER_ADMIN_DISCOVER_IP_HEADER_NAME: &str = "x-fn-knock-docker-discover-ip";
const UPSTREAM_PRIVATE_IPV4_HEADER_NAME: &str = "x-reauth-upstream-private-ipv4";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("server_admin_rs=info,tower_http=info,axum=info")
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Some(command) = env::args().nth(1) {
        match command.as_str() {
            "reset-panel-password" | "reset-admin-panel-password" => {
                if let Err(error) = reset_panel_password_command().await {
                    let locale =
                        env::var("FN_KNOCK_LOCALE").unwrap_or_else(|_| DEFAULT_LOCALE.to_string());
                    let translator = Translator::new(locale);
                    eprintln!(
                        "{} {}",
                        translator.t("server.dockerAdminPanel.resetFailed"),
                        error
                    );
                    std::process::exit(1);
                }
                return Ok(());
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => anyhow::bail!("unknown command: {command}"),
        }
    }

    let settings = Settings::from_env();
    let state = AppState::new(settings.clone()).await?;
    sync_auto_https_on_boot(state.clone()).await;
    start_boot_sync_tasks(state.clone());
    start_traffic_tasks(state.clone());
    start_ip_location_worker(state.clone());
    start_notification_tasks(state.clone());
    start_update_tasks(state.clone());
    start_ddns_tasks(state.clone());
    start_acme_tasks(state.clone());
    start_system_monitor_tasks(state.clone());
    start_system_clock_tasks(state.clone());
    start_auth_mobility_tasks(state.clone());
    start_common_auth_location_tasks(state.clone());
    start_whitelist_tasks(state.clone());
    start_waf_tasks(state.clone());
    start_terminal_tasks(state.clone());
    start_ssh_security_tasks(state.clone());
    start_cloudflared_tasks(state.clone());
    start_frpc_tasks(state.clone());

    let backend_addr = settings.backend_addr()?;
    let auth_addr = settings.auth_addr()?;
    let protected_admin_runtime = runtime_profile::admin_panel_protected_runtime(&state);
    let admin_view_addr = if protected_admin_runtime {
        settings.admin_view_addr()?
    } else {
        None
    };

    let backend = serve(
        "backend",
        backend_addr,
        backend_router(state.clone(), false),
    );
    let auth = serve("auth", auth_addr, auth_router(state.clone()));

    if let Some(addr) = admin_view_addr {
        let admin_view = serve("admin-view", addr, backend_router(state, true));
        tokio::try_join!(backend, auth, admin_view)?;
    } else {
        tokio::try_join!(backend, auth)?;
    }

    Ok(())
}

fn print_help() {
    println!("server-admin-rs");
    println!();
    println!("Commands:");
    println!("  reset-panel-password    Clear admin panel password/session state");
}

fn start_boot_sync_tasks(state: AppState) {
    tokio::spawn(async move {
        if let Err(error) = cleanup_legacy_auth_log_storage(&state).await {
            tracing::warn!(%error, "failed to cleanup legacy auth log storage on boot");
        }
        sync_runtime_config_on_boot(state.clone()).await;
        sync_gateway_settings_on_boot(state.clone()).await;
        sync_locale_config_on_boot(&state).await;
        if let Err(error) = sync_ssl_deployment_to_gateway(&state, None).await {
            tracing::warn!(%error, "failed to sync SSL deployment on boot");
        }
        if let Err(error) = init_clean_script_on_boot(&state) {
            tracing::warn!(%error, "failed to initialize firewall cleanup script");
        }
    });
}

fn init_clean_script_on_boot(state: &AppState) -> anyhow::Result<()> {
    if !runtime_profile::host_firewall_available(state) {
        tracing::info!("skipped clean.sh generation: host firewall is unavailable");
        return Ok(());
    }
    fs::create_dir_all(&state.settings.data_dir)?;
    let script_path = state.settings.data_dir.join("clean.sh");
    fs::write(&script_path, CLEAN_SCRIPT_CONTENT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }
    tracing::info!(path = %script_path.display(), "initialized firewall cleanup script");
    Ok(())
}

pub(crate) async fn cleanup_legacy_auth_log_storage(state: &AppState) -> anyhow::Result<()> {
    const STATE_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1";
    const LOCK_KEY: &str = "fn_knock:cleanup:legacy-auth-logs:v1:lock";
    const INDEX_KEY: &str = "fn_knock:auth_logs:index";
    const DATA_PREFIX: &str = "fn_knock:auth_log_data:";
    const REF_PREFIX: &str = "fn_knock:ip_location:refs:";
    const LEGACY_REF_PREFIX: &str = "auth-log|";

    if state.redis.get_string_value(STATE_KEY).await?.as_deref() == Some("done") {
        return Ok(());
    }
    if !state
        .redis
        .set_key_if_not_exists_with_ttl(LOCK_KEY, &time_utils::now_ms().to_string(), 3600)
        .await?
    {
        return Ok(());
    }

    let cleanup_result = async {
        state
            .redis
            .set_string_value_with_optional_ttl(STATE_KEY, "running", Some(3600))
            .await?;
        let data_keys = state.redis.scan_keys(DATA_PREFIX, 200).await?;
        for chunk in data_keys.chunks(200) {
            state.redis.delete_keys(chunk).await?;
        }
        state.redis.delete_key(INDEX_KEY).await?;

        let ref_keys = state.redis.scan_keys(REF_PREFIX, 200).await?;
        for key in ref_keys {
            let members = state.redis.smembers_strings(&key).await?;
            let legacy_members = members
                .into_iter()
                .filter(|member| member.starts_with(LEGACY_REF_PREFIX))
                .collect::<Vec<_>>();
            state
                .redis
                .srem_string_members(&key, &legacy_members)
                .await?;
        }
        state.redis.set_string_value(STATE_KEY, "done").await
    }
    .await;

    let _ = state.redis.delete_key(LOCK_KEY).await;
    cleanup_result.map_err(Into::into)
}

async fn sync_locale_config_on_boot(state: &AppState) {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for locale boot sync");
            return;
        }
    };
    let locale = normalize_locale_config(config.get("locale").unwrap_or(&serde_json::Value::Null));
    match state.go_backend.set_locale_config(&locale).await {
        Ok((status, value)) if status == reqwest::StatusCode::NOT_FOUND => {
            tracing::debug!(?value, "gateway locale sync endpoint is unavailable");
        }
        Ok((status, value)) => {
            if !status.is_success()
                || value.get("success").and_then(serde_json::Value::as_bool) == Some(false)
            {
                tracing::warn!(%status, response = %value, "failed to sync locale config on boot");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to sync locale config on boot");
        }
    }
}

async fn reset_panel_password_command() -> anyhow::Result<()> {
    let args = env::args().skip(2).collect::<Vec<_>>();
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        let locale = env::var("FN_KNOCK_LOCALE").unwrap_or_else(|_| DEFAULT_LOCALE.to_string());
        let translator = Translator::new(locale);
        println!("{}", translator.t("server.dockerAdminPanel.resetHelp"));
        return Ok(());
    }
    if let Some(arg) = args.first() {
        anyhow::bail!("unknown argument for reset-panel-password: {arg}");
    }

    let settings = Settings::from_env();
    let redis = RedisStore::connect(&settings.redis_url)
        .await
        .context("connect Redis for admin panel password reset")?;
    let locale = redis
        .locale()
        .await
        .ok()
        .and_then(|value| {
            value
                .get("default_locale")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string());
    let translator = Translator::new(locale);

    let summary = redis.reset_docker_admin_password_state().await?;
    println!("{}", translator.t("server.dockerAdminPanel.resetCleared"));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passwordCleared": summary.password_cleared,
            "sessionsCleared": summary.sessions_cleared,
            "loginFailuresCleared": summary.login_failures_cleared,
        }))?
    );
    println!("{}", translator.t("server.dockerAdminPanel.resetNextVisit"));
    Ok(())
}

fn backend_router(state: AppState, protected_admin_view: bool) -> Router {
    let api = Router::new()
        .route("/api/admin/healthz", axum::routing::get(response::healthz))
        .merge(openapi_docs_routes())
        .merge(admin_routes(protected_admin_view))
        .merge(admin_control_routes())
        .merge(acme_routes())
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
        .merge(terminal_routes())
        .merge(system_asset_routes())
        .merge(ssh_security_routes())
        .merge(ssl_routes())
        .merge(cloudflared_routes())
        .merge(frpc_routes())
        .merge(general_blacklist_routes())
        .merge(gateway_logs_routes())
        .merge(gateway_settings_routes())
        .merge(ip_location_routes())
        .merge(ip_location_config_routes())
        .merge(maintenance_routes())
        .merge(notification_routes())
        .merge(oidc_admin_routes())
        .merge(update_routes())
        .merge(waf_routes())
        .fallback(api_not_found);
    let api = if protected_admin_view {
        api.layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_view_ingress_middleware,
        ))
    } else if runtime_profile::admin_panel_protected_runtime(&state)
        && state.settings.admin_view_port.is_some()
    {
        api.layer(middleware::from_fn_with_state(
            state.clone(),
            admin_backend_proxy_middleware,
        ))
    } else {
        api
    };

    Router::new()
        .merge(api)
        .merge(admin_static_routes())
        .fallback(static_files::admin_fallback)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn api_not_found(State(state): State<AppState>) -> axum::response::Response {
    let translator = Translator::from_state(&state).await;
    response::error(
        axum::http::StatusCode::NOT_FOUND,
        translator.t("server.apiPathNotFound"),
    )
}

async fn admin_backend_proxy_middleware(
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

async fn admin_view_ingress_middleware(
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

fn auth_router(state: AppState) -> Router {
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

async fn serve(name: &'static str, addr: SocketAddr, router: Router) -> anyhow::Result<()> {
    let mut listeners = Vec::new();
    for listen_addr in listen_addrs(addr) {
        let listener = TcpListener::bind(listen_addr)
            .await
            .with_context(|| format!("bind {name} listener on {listen_addr}"))?;
        tracing::info!(%name, addr = %listen_addr, "server listening");
        listeners.push((listen_addr, listener));
    }

    let mut tasks = tokio::task::JoinSet::new();
    for (listen_addr, listener) in listeners {
        let router = router.clone();
        tasks.spawn(async move { serve_listener(name, listen_addr, listener, router).await });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tasks.abort_all();
                return Err(error);
            }
            Err(error) => {
                tasks.abort_all();
                return Err(error.into());
            }
        }
    }

    Ok(())
}

fn listen_addrs(addr: SocketAddr) -> Vec<SocketAddr> {
    let mut addrs = vec![addr];
    if let Some(companion) = loopback_companion_addr(addr) {
        addrs.push(companion);
    }
    addrs
}

fn loopback_companion_addr(addr: SocketAddr) -> Option<SocketAddr> {
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_loopback() => Some(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            addr.port(),
        )),
        IpAddr::V6(ip) if ip.is_loopback() => Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            addr.port(),
        )),
        _ => None,
    }
}

async fn serve_listener(
    name: &'static str,
    addr: SocketAddr,
    listener: TcpListener,
    router: Router,
) -> anyhow::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .with_context(|| format!("{name} server failed on {addr}"))?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn loopback_listeners_include_ipv4_and_ipv6_without_wildcard() {
        let ipv4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997);
        assert_eq!(
            listen_addrs(ipv4),
            vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7997),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7997),
            ]
        );

        let ipv6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7998);
        assert_eq!(
            listen_addrs(ipv6),
            vec![
                SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 7998),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7998),
            ]
        );

        let wildcard = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 7997);
        assert_eq!(listen_addrs(wildcard), vec![wildcard]);

        let ipv6_wildcard = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 7997);
        assert_eq!(listen_addrs(ipv6_wildcard), vec![ipv6_wildcard]);
    }

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
