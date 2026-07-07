use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use url::Url;

use crate::{gateway_settings, i18n::Translator, response, runtime_config, ssl, state::AppState};

const DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE: &str = "text/plain; charset=utf-8";
const BASIC_AUTH_PROBE_USER_AGENT: &str = "fn-knock-server-admin-basic-auth-probe/1.0";
const METADATA_USER_AGENT: &str = "fn-knock-server-admin/1.0";
const MAX_METADATA_HTML_BYTES: usize = 256 * 1024;
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_ICONS_TO_TRY: usize = 4;
const MAX_HTML_FAVICON_CANDIDATES_TO_TRY: usize = 12;
const MAX_FAVICON_FETCH_ATTEMPTS: i32 = 8;
const FALLBACK_FAVICON_FETCH_RESERVE: i32 = 3;
const HEURISTIC_FAVICON_MIN_PRIORITY: i32 = 350;
const STRONG_HEURISTIC_FAVICON_MIN_PRIORITY: i32 = 520;
const MAX_FAVICON_BYTES: usize = 128 * 1024;
const ONE_PANEL_TITLE: &str = "1Panel";
const ONE_PANEL_LOADING_TITLE: &str = "loading...";
const ONE_PANEL_FAVICON_PATH: &str = "/public/favicon.png";
const OPENWRT_LUCI_PATH: &str = "/cgi-bin/luci/";
const OPENWRT_LUCI_LOGIN_REQUIRED_HEADER: &str = "x-luci-login-required";
const FALLBACK_FAVICON_PATHS: [&str; 3] =
    ["/favicon.ico", "/img/favicon.ico", ONE_PANEL_FAVICON_PATH];
const FAVICON_CANDIDATE_ATTRIBUTE_NAMES: [&str; 9] = [
    "href",
    "src",
    "content",
    "icon",
    "data-href",
    "data-src",
    "data-original",
    "data-icon",
    "data-favicon",
];
const GO_BACKEND_UNSUCCESSFUL_RESPONSE: &str = "Go backend returned an unsuccessful response";

fn admin_config_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

fn admin_config_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

fn load_config_failed(translator: &Translator) -> String {
    admin_config_text(translator, "gatewaySettingsRoutes.loadConfigFailed")
}

fn localize_runtime_sync_error(
    translator: &Translator,
    message: &str,
    fallback_key: &str,
) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() || trimmed == GO_BACKEND_UNSUCCESSFUL_RESPONSE {
        return translator.t(fallback_key);
    }
    let localized = localize_proxy_config_error(translator, trimmed);
    if localized == trimmed && trimmed == GO_BACKEND_UNSUCCESSFUL_RESPONSE {
        translator.t(fallback_key)
    } else {
        localized
    }
}

fn localize_proxy_config_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    match message {
        "Proxy mapping must be an object" => {
            return admin_config_text(translator, "proxyMappings.payloadObjectRequired");
        }
        "Proxy mapping target must be a supported HTTP/WebSocket URL" => {
            return admin_config_text(translator, "proxyMappings.targetInvalid");
        }
        "Host mapping must be an object" => {
            return admin_config_text(translator, "hostMappings.payloadObjectRequired");
        }
        "Host mapping host is required" => {
            return admin_config_text(translator, "hostMappings.hostRequired");
        }
        "Only one auth service host mapping is allowed" => {
            return admin_config_text(translator, "hostMappings.singleAuthPortMapping");
        }
        "Stream mapping must be an object" => {
            return admin_config_text(translator, "streamMappings.payloadObjectRequired");
        }
        "Stream mapping listen_port must be an integer" => {
            return admin_config_text(translator, "streamMappings.listenPortRequiredInteger");
        }
        "Passkey parent-domain RP ID is required" => {
            return admin_config_text(translator, "passkeyRp.parentDomainRequired");
        }
        "Only http/https targets are supported" => {
            return admin_config_text(translator, "hostMappings.onlyHttpTargetsSupported");
        }
        _ => {}
    }

    if let Some(host) = extract_between(
        message,
        "Host mapping ",
        " target must be a supported HTTP/WebSocket URL",
    )
    .filter(|host| !host.contains(" location "))
    {
        return admin_config_text_params(
            translator,
            "hostMappings.targetInvalid",
            &[("host", host.to_string())],
        );
    }
    if let Some(host) = extract_between(message, "Auth host mapping ", " must be public") {
        return admin_config_text_params(
            translator,
            "hostMappings.authMappingMustBePublic",
            &[("host", host.to_string())],
        );
    }
    if let Some(host) = extract_between(message, "Auth host mapping ", " cannot enable Basic Auth")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.authMappingBasicAuthForbidden",
            &[("host", host.to_string())],
        );
    }
    if let Some(host) =
        extract_between(message, "Host mapping ", " Basic Auth settings are invalid")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.basicAuthInvalid",
            &[("host", host.to_string())],
        );
    }
    if let Some(port) = extract_between(message, "Stream mapping listen_port ", " is out of range")
    {
        return admin_config_text_params(
            translator,
            "streamMappings.listenPortOutOfRange",
            &[("port", port.to_string())],
        );
    }
    if let Some(rest) = message.strip_prefix("Duplicate stream mapping for ")
        && let Some((protocol, port)) = rest.split_once(" port ")
    {
        return admin_config_text_params(
            translator,
            "streamMappings.duplicatePort",
            &[
                ("protocol", protocol.to_string()),
                ("port", port.to_string()),
            ],
        );
    }
    if let Some(target) = message.strip_prefix("Stream mapping target must be host:port: ") {
        return admin_config_text_params(
            translator,
            "streamMappings.targetMustBeHostPort",
            &[("target", target.to_string())],
        );
    }
    if let Some((auth_host, rp_id)) = message
        .strip_prefix("Passkey auth host ")
        .and_then(|rest| rest.split_once(" must match or belong to RP ID "))
    {
        return admin_config_text_params(
            translator,
            "passkeyRp.mustMatchAuthHost",
            &[
                ("authHost", auth_host.to_string()),
                ("rpId", rp_id.to_string()),
            ],
        );
    }
    if let Some(host) = extract_between(message, "Host mapping ", " location path is required") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationPathRequired",
            &[("host", host.to_string())],
        );
    }
    if let Some((host, path)) = extract_host_location_path(message, " must start with /") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationPathMustStartSlash",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some(host) = extract_between(message, "Host mapping ", " location path / is reserved") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationRootForbidden",
            &[("host", host.to_string())],
        );
    }
    if let Some((host, path)) = extract_host_location_path(message, " is reserved") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationReservedPath",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some(rest) = message.strip_prefix("Host mapping ")
        && let Some((host, path)) = rest.split_once(" has duplicate location ")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.locationDuplicate",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some((host, path)) = extract_host_location(message, " target is required") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationTargetRequired",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some((host, path)) =
        extract_host_location(message, " target must be a supported HTTP/WebSocket URL")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.locationTargetInvalid",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some((host, path)) = extract_host_location(message, " response status is invalid") {
        return admin_config_text_params(
            translator,
            "hostMappings.locationStatusInvalid",
            &[("host", host.to_string()), ("path", path.to_string())],
        );
    }
    if let Some((host, path, header)) =
        extract_host_location_header(message, " response header ", " is invalid")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.locationHeaderInvalid",
            &[
                ("host", host.to_string()),
                ("path", path.to_string()),
                ("header", header.to_string()),
            ],
        );
    }
    if let Some((host, path, header)) =
        extract_host_location_header(message, " response header ", " is forbidden")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.locationHeaderForbidden",
            &[
                ("host", host.to_string()),
                ("path", path.to_string()),
                ("header", header.to_string()),
            ],
        );
    }
    if let Some(status) = message.strip_prefix("Upstream responded with ") {
        return admin_config_text_params(
            translator,
            "hostMappings.metadataUpstreamStatus",
            &[("status", status.to_string())],
        );
    }

    message.to_string()
}

fn extract_between<'a>(message: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    message.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn extract_host_location_path<'a>(message: &'a str, suffix: &str) -> Option<(&'a str, &'a str)> {
    let rest = message.strip_prefix("Host mapping ")?;
    let (host, path_with_suffix) = rest.split_once(" location path ")?;
    Some((host, path_with_suffix.strip_suffix(suffix)?))
}

fn extract_host_location<'a>(message: &'a str, suffix: &str) -> Option<(&'a str, &'a str)> {
    let rest = message.strip_prefix("Host mapping ")?;
    let (host, path_with_suffix) = rest.split_once(" location ")?;
    Some((host, path_with_suffix.strip_suffix(suffix)?))
}

fn extract_host_location_header<'a>(
    message: &'a str,
    middle: &str,
    suffix: &str,
) -> Option<(&'a str, &'a str, &'a str)> {
    let rest = message.strip_prefix("Host mapping ")?;
    let (host, path_and_header) = rest.split_once(" location ")?;
    let (path, header_with_suffix) = path_and_header.split_once(middle)?;
    Some((host, path, header_with_suffix.strip_suffix(suffix)?))
}

#[derive(Deserialize)]
struct MappingsBody {
    mappings: Vec<Value>,
}

#[derive(Clone)]
struct HostMappingMetadataRefreshItem {
    mapping: Value,
    refresh_title: bool,
    refresh_favicon: bool,
}

#[derive(Default)]
struct HostMappingMetadataRefreshSummary {
    updated: i64,
    failed: i64,
    skipped: i64,
}

#[derive(Clone)]
struct MetadataBasicAuthContext {
    origin: String,
    username: String,
    password: String,
}

struct MetadataHtmlDocument {
    html: String,
    final_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FaviconCandidate {
    href: String,
    priority: i32,
    index: usize,
}

struct FaviconCandidateContext<'a> {
    tag_name: Option<&'a str>,
    attribute_name: Option<&'a str>,
    attributes: Option<&'a HashMap<String, String>>,
    surrounding_text: Option<&'a str>,
    source_priority: i32,
    min_priority: i32,
    force: bool,
}

struct FaviconFetchBudget {
    remaining: i32,
    seen: HashSet<String>,
}

pub fn proxy_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/proxy_mappings",
            post(update_proxy_mappings),
        )
        .route(
            "/api/admin/config/host_mappings",
            get(get_host_mappings).post(update_host_mappings),
        )
        .route(
            "/api/admin/config/host_mappings/basic_auth_probe",
            post(basic_auth_probe),
        )
        .route(
            "/api/admin/config/host_mappings/metadata",
            post(host_mapping_metadata),
        )
        .route(
            "/api/admin/config/host_mappings/refresh_titles",
            post(refresh_host_mapping_titles),
        )
        .route(
            "/api/admin/config/host_mappings/bookmarks/export",
            get(export_host_mapping_bookmarks),
        )
        .route(
            "/api/admin/config/stream_mappings",
            get(get_stream_mappings).post(update_stream_mappings),
        )
        .route(
            "/api/admin/config/subdomain_mode",
            get(get_subdomain_mode).post(update_subdomain_mode),
        )
}

async fn get_host_mappings(State(state): State<AppState>) -> Response {
    get_config_section(state, "host_mappings", Value::Array(Vec::new())).await
}

async fn basic_auth_probe(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let target = body.get("target").and_then(Value::as_str).unwrap_or("");
    response::ok(probe_basic_auth_target(target, &translator).await).into_response()
}

async fn host_mapping_metadata(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let target = body.get("target").and_then(Value::as_str).unwrap_or("");
    match fetch_host_mapping_metadata(target, body.get("basic_auth")).await {
        Ok(data) => response::ok(data).into_response(),
        Err(message) => response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, &message),
        ),
    }
}

async fn refresh_host_mapping_titles(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read host mappings before metadata refresh");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (next_mappings, summary) = refresh_host_mapping_metadata(mappings).await;
    ensure_object(&mut config).insert(
        "host_mappings".to_string(),
        Value::Array(next_mappings.clone()),
    );
    if let Err(error) = state.redis.save_config(&config).await {
        tracing::warn!(%error, "failed to save host mappings after metadata refresh");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "hostMappings.updateFailed"),
        );
    }
    if let Err(message) = sync_host_mappings_runtime(&state, &config, &next_mappings).await {
        tracing::warn!(%message, "failed to sync host mappings after metadata refresh");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.hostMappings.syncHostRulesFailed",
            ),
        );
    }
    response::ok(summary).into_response()
}

async fn export_host_mapping_bookmarks(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read host mappings for bookmarks export");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let document = build_bookmarks_document(&config, &translator);
    let filename = build_bookmark_filename(&config);
    let mut response = document.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=UTF-8"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn get_stream_mappings(State(state): State<AppState>) -> Response {
    get_config_section(state, "stream_mappings", Value::Array(Vec::new())).await
}

async fn get_subdomain_mode(State(state): State<AppState>) -> Response {
    get_config_section(state, "subdomain_mode", default_subdomain_mode()).await
}

async fn get_config_section(state: AppState, key: &str, fallback: Value) -> Response {
    match state.redis.get_config().await {
        Ok(config) => response::ok(config.get(key).cloned().unwrap_or(fallback)).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, %key, "failed to load config section");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            )
        }
    }
}

async fn update_proxy_mappings(
    State(state): State<AppState>,
    Json(body): Json<MappingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let normalized = match normalize_proxy_mappings(body.mappings) {
        Ok(value) => value,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_proxy_config_error(&translator, message),
            );
        }
    };

    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before proxy mappings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let mut updated_config = previous_config.clone();
    ensure_object(&mut updated_config).insert(
        "proxy_mappings".to_string(),
        Value::Array(normalized.clone()),
    );

    if let Err(error) = state.redis.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save proxy mappings");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "proxyMappings.updateFailed"),
        );
    }

    let rules = Value::Array(normalized.clone());
    if let Err(message) = sync_go_rules(&state, &rules).await {
        rollback_proxy_mappings(&state, &previous_config).await;
        tracing::warn!(%message, "failed to sync proxy mappings to Go backend");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.proxyMappings.syncRulesFailed",
            ),
        );
    }

    response::ok(Value::Array(normalized)).into_response()
}

async fn update_host_mappings(
    State(state): State<AppState>,
    Json(body): Json<MappingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before host mappings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };

    let normalized = match normalize_host_mappings_for_route(body.mappings, &previous_config) {
        Ok(value) => value,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_proxy_config_error(&translator, &message),
            );
        }
    };

    let mut updated_config = previous_config.clone();
    ensure_object(&mut updated_config).insert(
        "host_mappings".to_string(),
        Value::Array(normalized.clone()),
    );
    if let Err(message) = validate_passkey_rp_config(&updated_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, &message),
        );
    }

    if let Err(error) = state.redis.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save host mappings");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "hostMappings.updateFailed"),
        );
    }

    if let Err(message) = sync_host_mappings_runtime(&state, &updated_config, &normalized).await {
        rollback_host_mappings(&state, &previous_config).await;
        tracing::warn!(%message, "failed to sync host mappings runtime");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.hostMappings.syncHostRulesFailed",
            ),
        );
    }

    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    schedule_host_mappings_metadata_refresh(state.clone(), normalized.clone(), previous_mappings);
    runtime_config::schedule_smart_connect_sync_after_host_mappings_change(
        state.clone(),
        updated_config.clone(),
    );

    response::ok(Value::Array(normalized)).into_response()
}

async fn update_stream_mappings(
    State(state): State<AppState>,
    Json(body): Json<MappingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let normalized = match normalize_stream_mappings(body.mappings) {
        Ok(value) => value,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_proxy_config_error(&translator, &message),
            );
        }
    };

    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before stream mappings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let mut updated_config = previous_config.clone();
    ensure_object(&mut updated_config).insert(
        "stream_mappings".to_string(),
        Value::Array(normalized.clone()),
    );

    if let Err(error) = state.redis.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save stream mappings");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "streamMappings.saveFailed"),
        );
    }

    if let Err(message) = sync_stream_mappings_runtime(&state, &updated_config).await {
        rollback_stream_mappings(&state, &previous_config).await;
        tracing::warn!(%message, "failed to sync stream mappings runtime");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.streamMappings.syncFailed",
            ),
        );
    }

    response::success_empty().into_response()
}

async fn update_subdomain_mode(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(patch) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_config_text(&translator, "subdomainMode.payloadObjectRequired"),
        );
    };

    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before subdomain mode update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };

    let mut merged = previous_config
        .get("subdomain_mode")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    for (key, value) in patch {
        merged.insert(key.clone(), value.clone());
    }
    let next = normalize_subdomain_mode_config(&Value::Object(merged));

    let mut updated_config = previous_config.clone();
    ensure_object(&mut updated_config).insert("subdomain_mode".to_string(), next.clone());
    if let Err(message) = validate_host_mappings_section(&updated_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, &message),
        );
    }
    if let Err(message) = validate_passkey_rp_config(&updated_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, &message),
        );
    }

    if let Err(error) = state.redis.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save subdomain mode config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "subdomainMode.saveFailed"),
        );
    }

    if let Err(message) = sync_go_auth_config(&state, &updated_config).await {
        rollback_subdomain_mode(&state, &previous_config).await;
        tracing::warn!(%message, "failed to sync subdomain mode auth config");
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_sync_error(
                &translator,
                &message,
                "server.admin.hostMappings.syncAuthConfigFailed",
            ),
        );
    }

    let ssl_auto_selection =
        match ssl::auto_select_certificate_for_subdomain(&state, &translator).await {
            Ok(selection) => selection.unwrap_or(Value::Null),
            Err(error) => {
                tracing::warn!(%error, "failed to auto select SSL certificate for subdomain mode");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_config_text(&translator, "subdomainMode.sslAutoSelectionSyncFailed"),
                );
            }
        };

    let mut data = next.as_object().cloned().unwrap_or_else(Map::new);
    data.insert("ssl_auto_selection".to_string(), ssl_auto_selection);
    response::ok(Value::Object(data)).into_response()
}

async fn sync_go_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

async fn sync_go_host_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_host_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

async fn sync_stream_mappings_runtime(state: &AppState, config: &Value) -> Result<(), String> {
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    runtime_config::apply_run_type_config(state, config, run_type).await
}

async fn sync_go_auth_config(state: &AppState, config: &Value) -> Result<(), String> {
    let auth_config = build_gateway_auth_config(config);
    ensure_go_success(
        state
            .go_backend
            .set_auth_config(&auth_config)
            .await
            .map_err(|error| error.to_string())?,
    )
}

async fn sync_host_mappings_runtime(
    state: &AppState,
    config: &Value,
    mappings: &[Value],
) -> Result<(), String> {
    sync_go_host_rules(state, &build_host_rules_payload(mappings)).await?;
    sync_go_auth_config(state, config).await?;
    gateway_settings::sync_gateway_target_runtime_for_config(state, config, true).await
}

async fn probe_basic_auth_target(input_url: &str, translator: &Translator) -> Value {
    let Some(normalized_url) = normalize_http_probe_url(input_url) else {
        return json!({
            "requiresBasicAuth": false,
            "httpStatus": Value::Null,
            "error": admin_config_text(translator, "hostMappings.onlyHttpTargetsSupported"),
        });
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return json!({
                "requiresBasicAuth": false,
                "httpStatus": Value::Null,
                "error": error.to_string(),
            });
        }
    };

    match client
        .get(normalized_url)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,*/*;q=0.8",
        )
        .header(reqwest::header::USER_AGENT, BASIC_AUTH_PROBE_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .send()
        .await
    {
        Ok(response) => json!({
            "requiresBasicAuth": has_basic_auth_challenge(
                response
                    .headers()
                    .get(reqwest::header::WWW_AUTHENTICATE)
                    .and_then(|value| value.to_str().ok())
            ),
            "httpStatus": i64::from(response.status().as_u16()),
        }),
        Err(error) => json!({
            "requiresBasicAuth": false,
            "httpStatus": Value::Null,
            "error": error.to_string(),
        }),
    }
}

fn has_basic_auth_challenge(www_authenticate: Option<&str>) -> bool {
    www_authenticate
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .any(|value| {
            value.eq_ignore_ascii_case("basic") || value.to_ascii_lowercase().starts_with("basic ")
        })
}

fn normalize_http_probe_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parsed = Url::parse(trimmed).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return None;
    }
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

async fn fetch_host_mapping_metadata(
    target: &str,
    basic_auth: Option<&Value>,
) -> Result<Value, String> {
    let normalized_url = normalize_http_probe_url(target)
        .ok_or_else(|| "Only http/https targets are supported".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
        .map_err(|error| error.to_string())?;
    let basic_auth_context = create_basic_auth_context(basic_auth, &normalized_url);
    let response = send_metadata_get(
        &client,
        &normalized_url,
        "text/html,application/xhtml+xml,*/*;q=0.8",
        basic_auth_context.as_ref(),
    )
    .await
    .map_err(|error| error.to_string())?;
    let is_luci_login_required = is_openwrt_luci_login_required_response(&response);
    if !response.status().is_success() && !is_luci_login_required {
        return Err(format!(
            "Upstream responded with {}",
            response.status().as_u16()
        ));
    }

    let initial_document = read_metadata_document(response).await?;
    let document =
        fetch_openwrt_luci_document(&client, initial_document, basic_auth_context.as_ref())
            .await
            .unwrap_or_else(|document| document);
    let title = extract_html_title(&document.html);

    let one_panel_favicon = if is_one_panel_loading_title(&title) {
        if let Some(favicon_url) =
            resolve_origin_path_url(&document.final_url, ONE_PANEL_FAVICON_PATH)
        {
            fetch_favicon_as_data_url(&client, &favicon_url, basic_auth_context.as_ref())
                .await
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    if !one_panel_favicon.is_empty() {
        return Ok(json!({
            "title": ONE_PANEL_TITLE,
            "favicon": one_panel_favicon,
            "finalUrl": document.final_url,
        }));
    }

    let html_base_url = extract_html_base_url(&document.html, &document.final_url);
    let explicit_favicon_urls =
        extract_explicit_favicon_urls_from_html(&document.html, &html_base_url);
    let strong_heuristic_favicon_urls = extract_heuristic_favicon_urls_from_html(
        &document.html,
        &html_base_url,
        STRONG_HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    let weak_heuristic_favicon_urls = extract_heuristic_favicon_urls_from_html(
        &document.html,
        &html_base_url,
        HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    let manifest_url = extract_manifest_from_html(&document.html, &html_base_url);
    let mut favicon_budget = FaviconFetchBudget {
        remaining: MAX_FAVICON_FETCH_ATTEMPTS,
        seen: HashSet::new(),
    };
    let mut favicon = fetch_first_favicon_as_data_url(
        &client,
        &explicit_favicon_urls,
        basic_auth_context.as_ref(),
        &mut favicon_budget,
        FALLBACK_FAVICON_FETCH_RESERVE,
    )
    .await;
    if favicon.is_empty() {
        if let Some(manifest_url) = manifest_url {
            let manifest_icons =
                fetch_manifest_icon_urls(&client, &manifest_url, basic_auth_context.as_ref()).await;
            favicon = fetch_first_favicon_as_data_url(
                &client,
                &manifest_icons,
                basic_auth_context.as_ref(),
                &mut favicon_budget,
                FALLBACK_FAVICON_FETCH_RESERVE,
            )
            .await;
        }
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &strong_heuristic_favicon_urls,
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            FALLBACK_FAVICON_FETCH_RESERVE,
        )
        .await;
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &resolve_fallback_favicon_urls(&document.final_url),
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            0,
        )
        .await;
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &weak_heuristic_favicon_urls,
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            0,
        )
        .await;
    }

    Ok(json!({
        "title": title,
        "favicon": favicon,
        "finalUrl": document.final_url,
    }))
}

fn usable_basic_auth(value: Option<&Value>) -> Option<(String, String)> {
    let object = value?.as_object()?;
    if object.get("enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let password = object.get("password").and_then(Value::as_str).unwrap_or("");
    if username.is_empty() || password.is_empty() || username.contains(':') {
        return None;
    }
    Some((username.to_string(), password.to_string()))
}

fn create_basic_auth_context(
    value: Option<&Value>,
    target_url: &str,
) -> Option<MetadataBasicAuthContext> {
    let (username, password) = usable_basic_auth(value)?;
    Some(MetadataBasicAuthContext {
        origin: Url::parse(target_url).ok()?.origin().ascii_serialization(),
        username,
        password,
    })
}

fn has_same_origin(value: &str, origin: &str) -> bool {
    Url::parse(value)
        .map(|url| url.origin().ascii_serialization() == origin)
        .unwrap_or(false)
}

fn apply_basic_auth_context(
    request: reqwest::RequestBuilder,
    url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> reqwest::RequestBuilder {
    if let Some(context) = basic_auth
        && has_same_origin(url, &context.origin)
    {
        return request.basic_auth(context.username.clone(), Some(context.password.clone()));
    }
    request
}

async fn send_metadata_get(
    client: &reqwest::Client,
    url: &str,
    accept: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> reqwest::Result<reqwest::Response> {
    let request = client
        .get(url)
        .header(reqwest::header::ACCEPT, accept)
        .header(reqwest::header::USER_AGENT, METADATA_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close");
    apply_basic_auth_context(request, url, basic_auth)
        .send()
        .await
}

async fn read_metadata_document(
    response: reqwest::Response,
) -> Result<MetadataHtmlDocument, String> {
    let final_url = response.url().to_string();
    let html = read_response_text_limited(response, MAX_METADATA_HTML_BYTES).await?;
    Ok(MetadataHtmlDocument { html, final_url })
}

async fn read_response_text_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    let byte_len = bytes.len().min(max_bytes);
    Ok(String::from_utf8_lossy(&bytes[..byte_len])
        .trim_start_matches('\u{feff}')
        .to_string())
}

async fn fetch_favicon_as_data_url(
    client: &reqwest::Client,
    favicon_url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Option<String> {
    let trimmed_url = favicon_url.trim();
    if trimmed_url.to_ascii_lowercase().starts_with("data:image/") {
        return (trimmed_url.len() <= MAX_FAVICON_BYTES * 2).then(|| trimmed_url.to_string());
    }

    let normalized_url = normalize_http_probe_url(trimmed_url)?;
    let response = send_metadata_get(client, &normalized_url, "image/*,*/*;q=0.8", basic_auth)
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response_content_length_exceeds(response.headers(), MAX_FAVICON_BYTES) {
        return None;
    }
    let media_type = resolve_image_content_type(&normalized_url, response.headers())?;
    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_FAVICON_BYTES {
        return None;
    }
    Some(format!(
        "data:{media_type};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

fn response_content_length_exceeds(headers: &reqwest::header::HeaderMap, max_bytes: usize) -> bool {
    headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<usize>().ok())
        .is_some_and(|length| length > max_bytes)
}

fn resolve_image_content_type(value: &str, headers: &reqwest::header::HeaderMap) -> Option<String> {
    let header_value = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    match header_value.as_deref() {
        Some(
            "application/ico"
            | "application/x-ico"
            | "application/x-icon"
            | "application/vnd.microsoft.icon",
        ) => return Some("image/x-icon".to_string()),
        Some(value) if value.starts_with("image/") => return Some(value.to_string()),
        Some("application/octet-stream" | "binary/octet-stream") | None => {}
        Some(_) => return None,
    }

    let path = Url::parse(value).ok()?.path().to_ascii_lowercase();
    if path.ends_with(".svg") {
        Some("image/svg+xml".to_string())
    } else if path.ends_with(".png") {
        Some("image/png".to_string())
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        Some("image/jpeg".to_string())
    } else if path.ends_with(".gif") {
        Some("image/gif".to_string())
    } else if path.ends_with(".webp") {
        Some("image/webp".to_string())
    } else if path.ends_with(".ico") {
        Some("image/x-icon".to_string())
    } else {
        None
    }
}

fn extract_html_title(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<title") else {
        return String::new();
    };
    let Some(open_end) = lower[start..].find('>') else {
        return String::new();
    };
    let content_start = start + open_end + 1;
    let Some(close_start) = lower[content_start..].find("</title>") else {
        return String::new();
    };
    collapse_html_whitespace(&decode_html_entities(
        &html[content_start..content_start + close_start],
    ))
}

#[cfg(test)]
fn extract_favicon_url(html: &str, base_url: &str) -> Option<String> {
    let html_base_url = extract_html_base_url(html, base_url);
    extract_explicit_favicon_urls_from_html(html, &html_base_url)
        .into_iter()
        .next()
        .or_else(|| {
            extract_heuristic_favicon_urls_from_html(
                html,
                &html_base_url,
                HEURISTIC_FAVICON_MIN_PRIORITY,
            )
            .into_iter()
            .next()
        })
        .or_else(|| resolve_default_favicon_url(base_url))
}

fn resolve_url(base_url: &str, href: &str) -> Option<String> {
    if href.is_empty() {
        return None;
    }
    let base = Url::parse(base_url).ok()?;
    base.join(href).ok().map(|url| url.to_string())
}

#[cfg(test)]
fn resolve_default_favicon_url(final_url: &str) -> Option<String> {
    let mut parsed = Url::parse(final_url).ok()?;
    parsed.set_path("/favicon.ico");
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn resolve_origin_path_url(value: &str, pathname: &str) -> Option<String> {
    let mut parsed = Url::parse(value).ok()?;
    parsed.set_path(pathname);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn resolve_fallback_favicon_urls(value: &str) -> Vec<String> {
    FALLBACK_FAVICON_PATHS
        .iter()
        .filter_map(|pathname| resolve_origin_path_url(value, pathname))
        .collect()
}

fn normalize_favicon_url(value: &str, base_url: &str) -> Option<String> {
    let trimmed = decode_html_entities(value).trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.to_ascii_lowercase().starts_with("data:image/") {
        return Some(trimmed);
    }
    let resolved = resolve_url(base_url, &trimmed.replace("\\/", "/"))?;
    let parsed = Url::parse(&resolved).ok()?;
    matches!(parsed.scheme(), "http" | "https" | "data").then_some(parsed.to_string())
}

fn normalize_manifest_url(value: &str, base_url: &str) -> Option<String> {
    let trimmed = decode_html_entities(value).trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    let resolved = resolve_url(base_url, &trimmed)?;
    let parsed = Url::parse(&resolved).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(parsed.to_string())
}

fn extract_html_base_url(html: &str, base_url: &str) -> String {
    for tag in collect_html_tags(html, "base") {
        let attributes = parse_html_attributes(tag);
        if let Some(href) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        {
            return href;
        }
    }
    base_url.to_string()
}

fn collect_html_tags<'a>(html: &'a str, tag_name: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{}", tag_name.to_ascii_lowercase());
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = lower[cursor..].find(&needle) {
        let start = cursor + relative_start;
        let after_name = start + needle.len();
        if let Some(next) = lower.as_bytes().get(after_name)
            && !matches!(next, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>')
        {
            cursor = after_name;
            continue;
        }
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        if let Some(tag) = html.get(start..end) {
            tags.push(tag);
        }
        cursor = end;
    }
    tags
}

fn get_html_tag_name(tag: &str) -> String {
    let trimmed = tag.trim_start();
    let Some(rest) = trimmed.strip_prefix('<') else {
        return String::new();
    };
    rest.chars()
        .take_while(|ch| !ch.is_ascii_whitespace() && *ch != '/' && *ch != '>')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_html_attributes(tag: &str) -> HashMap<String, String> {
    let bytes = tag.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx] != b'<' {
        idx += 1;
    }
    if idx < bytes.len() {
        idx += 1;
    }
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    while idx < bytes.len()
        && !bytes[idx].is_ascii_whitespace()
        && bytes[idx] != b'/'
        && bytes[idx] != b'>'
    {
        idx += 1;
    }

    let mut attributes = HashMap::new();
    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() || bytes[idx] == b'>' {
            break;
        }
        if bytes[idx] == b'/' {
            idx += 1;
            continue;
        }

        let name_start = idx;
        while idx < bytes.len()
            && !bytes[idx].is_ascii_whitespace()
            && bytes[idx] != b'='
            && bytes[idx] != b'/'
            && bytes[idx] != b'>'
        {
            idx += 1;
        }
        if name_start == idx {
            idx += 1;
            continue;
        }
        let Some(raw_name) = tag.get(name_start..idx) else {
            continue;
        };
        let name = raw_name.to_ascii_lowercase();
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        let mut value = "";
        if idx < bytes.len() && bytes[idx] == b'=' {
            idx += 1;
            while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            if idx < bytes.len() {
                let quote = bytes[idx];
                let value_start;
                let value_end;
                if quote == b'"' || quote == b'\'' {
                    idx += 1;
                    value_start = idx;
                    while idx < bytes.len() && bytes[idx] != quote {
                        idx += 1;
                    }
                    value_end = idx;
                    if idx < bytes.len() {
                        idx += 1;
                    }
                } else {
                    value_start = idx;
                    while idx < bytes.len()
                        && !bytes[idx].is_ascii_whitespace()
                        && bytes[idx] != b'>'
                        && bytes[idx] != b'/'
                    {
                        idx += 1;
                    }
                    value_end = idx;
                }
                value = tag.get(value_start..value_end).unwrap_or("");
            }
        }
        attributes.insert(name, decode_html_entities(value).trim().to_string());
    }
    attributes
}

fn get_favicon_priority(rel: &str) -> i32 {
    let normalized = rel
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        0
    } else if normalized == "icon" {
        500
    } else if normalized == "shortcut icon" {
        450
    } else if normalized.contains("apple-touch-icon") {
        400
    } else if normalized.contains("mask-icon") {
        300
    } else if normalized.split_whitespace().any(|token| token == "icon") {
        350
    } else {
        0
    }
}

fn get_image_extension_priority(extension: &str) -> i32 {
    match extension {
        "ico" => 80,
        "png" => 60,
        "svg" => 50,
        "webp" => 40,
        "jpg" | "jpeg" => 30,
        "gif" => 20,
        _ => 0,
    }
}

fn get_favicon_path_priority(value: &str) -> i32 {
    if value.to_ascii_lowercase().starts_with("data:image/") {
        return 0;
    }

    let Ok(parsed) = Url::parse(value) else {
        return 0;
    };
    let pathname = parsed.path().to_ascii_lowercase();
    let file_name = pathname.rsplit('/').next().unwrap_or("");
    let extension = file_name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");

    let mut priority = if file_name == "favicon.ico" {
        700
    } else if file_name.starts_with("favicon")
        && file_name
            .as_bytes()
            .get("favicon".len())
            .is_none_or(|ch| matches!(ch, b'-' | b'_' | b'.'))
    {
        650
    } else if file_name.starts_with("apple-touch-icon") {
        600
    } else if file_name.starts_with("android-chrome") {
        560
    } else if file_name.starts_with("mstile") {
        520
    } else if file_name.contains("favicon") {
        500
    } else if pathname.contains("/favicon") {
        450
    } else if is_icon_like_file_name(file_name) {
        220
    } else if extension == "ico" {
        180
    } else if is_logo_like_file_name(file_name) {
        80
    } else {
        return 0;
    };

    priority += get_image_extension_priority(extension);
    if pathname.contains("/img/") {
        priority += 20;
    }
    if pathname.contains("/icons/") || pathname.contains("/icon/") {
        priority += 15;
    }
    if pathname.split('/').count() <= 3 {
        priority += 10;
    }
    priority
}

fn is_icon_like_file_name(file_name: &str) -> bool {
    let normalized = file_name.replace(['-', '_', '.'], " ");
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    tokens.iter().any(|token| {
        matches!(
            *token,
            "appicon" | "app" | "siteicon" | "site" | "touchicon" | "touch" | "icon"
        )
    }) && file_name.contains("icon")
}

fn is_logo_like_file_name(file_name: &str) -> bool {
    file_name
        .replace(['-', '_', '.'], " ")
        .split_whitespace()
        .any(|token| token == "logo")
}

fn get_favicon_type_priority(value: &str) -> i32 {
    let normalized = value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "image/x-icon"
        | "image/vnd.microsoft.icon"
        | "application/x-icon"
        | "application/vnd.microsoft.icon" => 850,
        "image/svg+xml" => 260,
        value if value.starts_with("image/") => 160,
        _ => 0,
    }
}

fn get_attribute_hint_priority(
    attribute_name: &str,
    attributes: Option<&HashMap<String, String>>,
) -> i32 {
    let mut priority = 0;
    let normalized_attribute_name = attribute_name.to_ascii_lowercase();
    if normalized_attribute_name.contains("favicon") {
        priority += 450;
    } else if normalized_attribute_name.contains("icon") {
        priority += 280;
    } else if normalized_attribute_name == "href" {
        priority += 60;
    } else if normalized_attribute_name == "src" {
        priority += 40;
    } else if normalized_attribute_name == "content" {
        priority += 30;
    }

    for key in ["name", "property", "itemprop", "id", "class"] {
        let normalized_value = attributes
            .and_then(|attributes| attributes.get(key))
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if normalized_value.is_empty() {
            continue;
        }
        if normalized_value.contains("favicon") || normalized_value.contains("shortcut icon") {
            priority += 520;
        } else if normalized_value.contains("apple-touch-icon") {
            priority += 480;
        } else if normalized_value.contains("msapplication-tileimage")
            || normalized_value.contains("tileimage")
        {
            priority += 440;
        } else if contains_word(&normalized_value, "icon") {
            priority += 260;
        }
    }
    priority
}

fn get_tag_priority(tag_name: &str) -> i32 {
    match tag_name {
        "link" => 120,
        "meta" => 60,
        "img" => 20,
        _ => 0,
    }
}

fn get_html_icon_size_priority(sizes: Option<&str>) -> i32 {
    let Some(sizes) = sizes else {
        return 0;
    };
    let mut best = 0_i32;
    for token in sizes.trim().to_ascii_lowercase().split_whitespace() {
        if token == "any" {
            best = best.max(1024);
            continue;
        }
        let Some((width, height)) = parse_icon_size(token) else {
            continue;
        };
        best = best.max(width.min(height));
    }

    if best >= 192 {
        160
    } else if best >= 64 {
        120
    } else if best >= 32 {
        80
    } else if best > 0 {
        30
    } else {
        0
    }
}

fn get_surrounding_favicon_priority(value: Option<&str>) -> i32 {
    let normalized = value.map(str::to_ascii_lowercase).unwrap_or_default();
    if normalized.is_empty() {
        0
    } else if normalized.contains("favicon") {
        520
    } else if normalized.contains("shortcut icon") {
        500
    } else if normalized.contains("apple-touch-icon") {
        480
    } else if normalized.contains("msapplication-tileimage") || normalized.contains("tileimage") {
        440
    } else if normalized.contains("fav-icon")
        || normalized.contains("fav_icon")
        || normalized.contains("fav icon")
        || normalized.contains("iconurl")
        || normalized.contains("iconuri")
        || normalized.contains("iconhref")
        || normalized.contains("iconsrc")
        || normalized.contains("iconpath")
        || normalized.contains("appicon")
        || normalized.contains("siteicon")
    {
        320
    } else if contains_word(&normalized, "icon") {
        140
    } else {
        0
    }
}

fn create_favicon_candidate(
    raw_value: &str,
    base_url: &str,
    index: usize,
    context: FaviconCandidateContext<'_>,
) -> Option<FaviconCandidate> {
    let href = normalize_favicon_url(raw_value, base_url)?;
    let attributes = context.attributes;
    let rel_priority = get_favicon_priority(
        attributes
            .and_then(|value| value.get("rel"))
            .map(String::as_str)
            .unwrap_or(""),
    );
    let path_priority = get_favicon_path_priority(&href);
    let type_priority = get_favicon_type_priority(
        attributes
            .and_then(|value| value.get("type"))
            .map(String::as_str)
            .unwrap_or(""),
    );
    let attribute_priority =
        get_attribute_hint_priority(context.attribute_name.unwrap_or(""), attributes);
    let surrounding_priority = get_surrounding_favicon_priority(context.surrounding_text);
    let size_priority = get_html_icon_size_priority(
        attributes
            .and_then(|attributes| attributes.get("sizes"))
            .map(String::as_str),
    );
    let priority = rel_priority * 1000
        + path_priority
        + type_priority
        + attribute_priority
        + surrounding_priority
        + get_tag_priority(context.tag_name.unwrap_or(""))
        + size_priority
        + context.source_priority;

    if !context.force && priority < context.min_priority {
        return None;
    }

    Some(FaviconCandidate {
        href,
        priority,
        index,
    })
}

fn sort_favicon_candidates(mut candidates: Vec<FaviconCandidate>) -> Vec<String> {
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            if seen.insert(candidate.href.clone()) {
                Some(candidate.href)
            } else {
                None
            }
        })
        .take(MAX_HTML_FAVICON_CANDIDATES_TO_TRY)
        .collect()
}

fn extract_explicit_favicon_urls_from_html(html: &str, base_url: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for (index, tag) in collect_html_tags(html, "link").into_iter().enumerate() {
        let attributes = parse_html_attributes(tag);
        if get_favicon_priority(attributes.get("rel").map(String::as_str).unwrap_or("")) <= 0 {
            continue;
        }
        if let Some(candidate) = create_favicon_candidate(
            attributes.get("href").map(String::as_str).unwrap_or(""),
            base_url,
            index,
            FaviconCandidateContext {
                tag_name: Some(&get_html_tag_name(tag)),
                attribute_name: Some("href"),
                attributes: Some(&attributes),
                surrounding_text: None,
                source_priority: 0,
                min_priority: HEURISTIC_FAVICON_MIN_PRIORITY,
                force: true,
            },
        ) {
            candidates.push(candidate);
        }
    }
    sort_favicon_candidates(candidates)
}

fn extract_heuristic_favicon_urls_from_html(
    html: &str,
    base_url: &str,
    min_priority: i32,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut index = 0_usize;
    for tag_name in ["link", "meta", "img", "source"] {
        for tag in collect_html_tags(html, tag_name) {
            let parsed_tag_name = get_html_tag_name(tag);
            let attributes = parse_html_attributes(tag);
            for attribute_name in FAVICON_CANDIDATE_ATTRIBUTE_NAMES {
                let Some(raw_value) = attributes.get(attribute_name) else {
                    continue;
                };
                if let Some(candidate) = create_favicon_candidate(
                    raw_value,
                    base_url,
                    index,
                    FaviconCandidateContext {
                        tag_name: Some(&parsed_tag_name),
                        attribute_name: Some(attribute_name),
                        attributes: Some(&attributes),
                        surrounding_text: None,
                        source_priority: 0,
                        min_priority,
                        force: false,
                    },
                ) {
                    candidates.push(candidate);
                }
                index += 1;
            }
        }
    }

    for (raw_value, match_index) in extract_image_resource_paths(html) {
        let start = match_index.saturating_sub(80);
        let end = (match_index + raw_value.len() + 80).min(html.len());
        let surrounding_text = html.get(start..end).unwrap_or("");
        if let Some(candidate) = create_favicon_candidate(
            &raw_value,
            base_url,
            index,
            FaviconCandidateContext {
                tag_name: None,
                attribute_name: None,
                attributes: None,
                surrounding_text: Some(surrounding_text),
                source_priority: 0,
                min_priority,
                force: false,
            },
        ) {
            candidates.push(candidate);
        }
        index += 1;
    }

    sort_favicon_candidates(candidates)
}

fn extract_image_resource_paths(html: &str) -> Vec<(String, usize)> {
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut results = Vec::new();
    let mut cursor = 0;
    while cursor < lower.len() {
        let next_match = [".ico", ".png", ".svg", ".jpg", ".jpeg", ".gif", ".webp"]
            .iter()
            .filter_map(|extension| {
                lower[cursor..]
                    .find(extension)
                    .map(|pos| (cursor + pos, *extension))
            })
            .min_by_key(|(pos, _)| *pos);
        let Some((extension_pos, extension)) = next_match else {
            break;
        };

        let mut start = extension_pos;
        while start > 0 && !is_image_resource_delimiter(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = extension_pos + extension.len();
        while end < bytes.len() && !is_image_resource_delimiter(bytes[end]) {
            end += 1;
        }

        if let Some(value) = html.get(start..end) {
            let trimmed = value.trim_matches(|ch| matches!(ch, '\'' | '"' | '(' | ')' | '\\'));
            if is_plausible_image_resource_path(trimmed) {
                results.push((trimmed.to_string(), start));
            }
        }
        cursor = end.max(extension_pos + extension.len());
    }
    results
}

fn is_image_resource_delimiter(value: u8) -> bool {
    value.is_ascii_whitespace() || matches!(value, b'"' | b'\'' | b'<' | b'>' | b'\\' | b')')
}

fn is_plausible_image_resource_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("//")
        || lower.starts_with('/')
        || lower.starts_with("./")
        || lower.starts_with("../")
        || (lower.contains('/') && !lower.contains('<') && !lower.contains('>'))
}

fn extract_manifest_from_html(html: &str, base_url: &str) -> Option<String> {
    for tag in collect_html_tags(html, "link") {
        let attributes = parse_html_attributes(tag);
        let has_manifest_rel = attributes
            .get("rel")
            .map(|rel| {
                rel.trim()
                    .to_ascii_lowercase()
                    .split_whitespace()
                    .any(|token| token == "manifest")
            })
            .unwrap_or(false);
        if !has_manifest_rel {
            continue;
        }
        if let Some(href) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        {
            return Some(href);
        }
    }
    None
}

async fn fetch_manifest_icon_urls(
    client: &reqwest::Client,
    manifest_url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Vec<String> {
    let Some(normalized_url) = normalize_http_probe_url(manifest_url) else {
        return Vec::new();
    };
    let Ok(response) = send_metadata_get(
        client,
        &normalized_url,
        "application/manifest+json,application/json,*/*;q=0.8",
        basic_auth,
    )
    .await
    else {
        return Vec::new();
    };
    if !response.status().is_success()
        || response_content_length_exceeds(response.headers(), MAX_MANIFEST_BYTES)
    {
        return Vec::new();
    }
    let manifest_url = response.url().to_string();
    let Ok(text) = read_response_text_limited(response, MAX_MANIFEST_BYTES).await else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    extract_manifest_icon_urls(&manifest, &manifest_url)
}

fn extract_manifest_icon_urls(manifest: &Value, manifest_url: &str) -> Vec<String> {
    let Some(icons) = manifest.get("icons").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for (index, raw_icon) in icons.iter().enumerate() {
        let Some(icon) = raw_icon.as_object() else {
            continue;
        };
        let Some(src) = icon.get("src").and_then(Value::as_str) else {
            continue;
        };
        let media_type = icon
            .get("type")
            .and_then(Value::as_str)
            .and_then(|value| value.split(';').next())
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if !media_type.is_empty() && !media_type.starts_with("image/") {
            continue;
        }
        let Some(href) = normalize_favicon_url(src, manifest_url) else {
            continue;
        };
        candidates.push(FaviconCandidate {
            href,
            priority: get_manifest_icon_priority(raw_icon),
            index,
        });
    }
    sort_manifest_icon_candidates(candidates)
}

fn sort_manifest_icon_candidates(mut candidates: Vec<FaviconCandidate>) -> Vec<String> {
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            if seen.insert(candidate.href.clone()) {
                Some(candidate.href)
            } else {
                None
            }
        })
        .take(MAX_MANIFEST_ICONS_TO_TRY)
        .collect()
}

fn get_manifest_icon_priority(icon: &Value) -> i32 {
    let purpose_tokens = icon
        .get("purpose")
        .and_then(Value::as_str)
        .map(|purpose| {
            purpose
                .trim()
                .to_ascii_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let media_type = icon
        .get("type")
        .and_then(Value::as_str)
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();

    let mut priority = icon
        .get("sizes")
        .and_then(Value::as_str)
        .map(get_manifest_icon_size_score)
        .unwrap_or(0);
    if purpose_tokens.is_empty() || purpose_tokens.iter().any(|token| token == "any") {
        priority += 2000;
    } else if purpose_tokens.iter().any(|token| token == "maskable") {
        priority += 1000;
    }
    priority += match media_type.as_str() {
        "image/png" => 80,
        "image/svg+xml" => 70,
        "image/webp" => 60,
        "image/jpeg" => 50,
        "image/x-icon" | "image/vnd.microsoft.icon" => 40,
        _ => 0,
    };
    priority
}

fn get_manifest_icon_size_score(sizes: &str) -> i32 {
    let mut best = 0_i32;
    for token in sizes.trim().to_ascii_lowercase().split_whitespace() {
        if token == "any" {
            best = best.max(1024);
            continue;
        }
        let Some((width, height)) = parse_icon_size(token) else {
            continue;
        };
        best = best.max(width.min(height));
    }
    best
}

fn parse_icon_size(token: &str) -> Option<(i32, i32)> {
    let (width, height) = token.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

async fn fetch_first_favicon_as_data_url(
    client: &reqwest::Client,
    favicon_urls: &[String],
    basic_auth: Option<&MetadataBasicAuthContext>,
    budget: &mut FaviconFetchBudget,
    reserve_attempts: i32,
) -> String {
    for favicon_url in favicon_urls {
        let normalized = favicon_url.trim();
        if normalized.is_empty() || budget.seen.contains(normalized) {
            continue;
        }

        let is_inline_image = normalized.to_ascii_lowercase().starts_with("data:image/");
        if !is_inline_image {
            if budget.remaining <= reserve_attempts {
                break;
            }
            budget.remaining -= 1;
        }
        budget.seen.insert(normalized.to_string());
        if let Some(favicon) = fetch_favicon_as_data_url(client, normalized, basic_auth).await {
            return favicon;
        }
    }
    String::new()
}

fn is_openwrt_luci_url(value: &str) -> bool {
    Url::parse(value)
        .map(|url| {
            let pathname = url.path().to_ascii_lowercase();
            pathname == "/cgi-bin/luci" || pathname.starts_with(OPENWRT_LUCI_PATH)
        })
        .unwrap_or(false)
}

fn is_same_origin_url(value: &str, base_url: &str) -> bool {
    let Ok(value) = Url::parse(value) else {
        return false;
    };
    let Ok(base) = Url::parse(base_url) else {
        return false;
    };
    value.origin() == base.origin()
}

fn strip_refresh_url_quotes(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).to_string()
}

fn extract_openwrt_luci_url_from_html(html: &str, base_url: &str) -> Option<String> {
    for tag in collect_html_tags(html, "meta") {
        let attributes = parse_html_attributes(tag);
        if attributes
            .get("http-equiv")
            .map(|value| value.trim().eq_ignore_ascii_case("refresh"))
            != Some(true)
        {
            continue;
        }
        let content =
            decode_html_entities(attributes.get("content").map(String::as_str).unwrap_or(""));
        let Some(refresh_url) = find_refresh_url(&content) else {
            continue;
        };
        let Some(resolved) =
            normalize_manifest_url(&strip_refresh_url_quotes(refresh_url), base_url)
        else {
            continue;
        };
        if is_openwrt_luci_url(&resolved) && is_same_origin_url(&resolved, base_url) {
            return Some(resolved);
        }
    }

    for tag in collect_html_tags(html, "a") {
        let attributes = parse_html_attributes(tag);
        let Some(resolved) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        else {
            continue;
        };
        if is_openwrt_luci_url(&resolved) && is_same_origin_url(&resolved, base_url) {
            return Some(resolved);
        }
    }

    Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(OPENWRT_LUCI_PATH).ok())
        .map(|url| url.to_string())
}

fn find_refresh_url(content: &str) -> Option<&str> {
    let lower = content.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let content_bytes = content.as_bytes();
    let mut cursor = 0;
    while let Some(relative_pos) = lower[cursor..].find("url") {
        let pos = cursor + relative_pos;
        let before_ok = pos == 0 || !lower_bytes[pos - 1].is_ascii_alphanumeric();
        if !before_ok {
            cursor = pos + 3;
            continue;
        }

        let mut idx = pos + 3;
        while idx < content_bytes.len() && content_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if content_bytes.get(idx) != Some(&b'=') {
            cursor = idx;
            continue;
        }
        idx += 1;
        while idx < content_bytes.len() && content_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let value = &content[idx..];
        return Some(value.split(';').next().unwrap_or(value).trim());
    }
    None
}

fn has_openwrt_luci_entrypoint_html(html: &str) -> bool {
    let normalized = html.to_ascii_lowercase();
    normalized.contains("cgi-bin/luci")
        && (normalized.contains("luci - lua configuration interface")
            || normalized.contains("http-equiv=\"refresh\"")
            || normalized.contains("http-equiv='refresh'")
            || normalized.contains("http-equiv=refresh"))
}

fn has_openwrt_luci_document_html(html: &str) -> bool {
    let title = extract_html_title(html).to_ascii_lowercase();
    let normalized = html.to_ascii_lowercase();
    title_has_luci_word(&title)
        && (normalized.contains("/luci-static/")
            || normalized.contains("application-name")
            || normalized.contains("apple-mobile-web-app-title"))
}

fn title_has_luci_word(title: &str) -> bool {
    let bytes = title.as_bytes();
    for (index, _) in title.match_indices("luci") {
        let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        let after = index + "luci".len();
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_openwrt_luci_login_required_response(response: &reqwest::Response) -> bool {
    response.status() == reqwest::StatusCode::FORBIDDEN
        && response
            .headers()
            .get(OPENWRT_LUCI_LOGIN_REQUIRED_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim().eq_ignore_ascii_case("yes"))
            == Some(true)
}

async fn fetch_openwrt_luci_document(
    client: &reqwest::Client,
    document: MetadataHtmlDocument,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Result<MetadataHtmlDocument, MetadataHtmlDocument> {
    if is_openwrt_luci_url(&document.final_url) || has_openwrt_luci_document_html(&document.html) {
        return Ok(document);
    }
    if !has_openwrt_luci_entrypoint_html(&document.html) {
        return Err(document);
    }

    let Some(luci_url) = extract_openwrt_luci_url_from_html(&document.html, &document.final_url)
    else {
        return Err(document);
    };
    let Ok(response) = send_metadata_get(
        client,
        &luci_url,
        "text/html,application/xhtml+xml,*/*;q=0.8",
        basic_auth,
    )
    .await
    else {
        return Err(document);
    };
    let is_luci_login_required = is_openwrt_luci_login_required_response(&response);
    if !response.status().is_success() && !is_luci_login_required {
        return Err(document);
    }
    let final_url = response.url().to_string();
    let Ok(html) = read_response_text_limited(response, MAX_METADATA_HTML_BYTES).await else {
        return Err(document);
    };
    if !has_openwrt_luci_document_html(&html) && !is_luci_login_required {
        return Err(document);
    }
    Ok(MetadataHtmlDocument { html, final_url })
}

fn is_one_panel_loading_title(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case(ONE_PANEL_LOADING_TITLE)
}

fn decode_html_entities(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            output.push(ch);
            continue;
        }
        let mut token = String::new();
        while let Some(next) = chars.peek().copied() {
            chars.next();
            if next == ';' {
                break;
            }
            token.push(next);
            if token.len() > 16 {
                output.push('&');
                output.push_str(&token);
                token.clear();
                break;
            }
        }
        if token.is_empty() {
            continue;
        }
        let replacement = match token.to_ascii_lowercase().as_str() {
            "amp" => Some("&".to_string()),
            "lt" => Some("<".to_string()),
            "gt" => Some(">".to_string()),
            "quot" => Some("\"".to_string()),
            "apos" => Some("'".to_string()),
            "nbsp" => Some(" ".to_string()),
            token if token.starts_with("#x") => u32::from_str_radix(&token[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|ch| ch.to_string()),
            token if token.starts_with('#') => token[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(|ch| ch.to_string()),
            _ => None,
        };
        if let Some(replacement) = replacement {
            output.push_str(&replacement);
        } else {
            output.push('&');
            output.push_str(&token);
            output.push(';');
        }
    }
    output
}

fn collapse_html_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_word(value: &str, word: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| token == word)
}

async fn refresh_host_mapping_metadata(mappings: Vec<Value>) -> (Vec<Value>, Value) {
    let mut updated = 0_i64;
    let mut failed = 0_i64;
    let mut skipped = 0_i64;
    let mut next_mappings = Vec::with_capacity(mappings.len());

    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            skipped += 1;
            next_mappings.push(mapping);
            continue;
        };
        let target = object.get("target").and_then(Value::as_str).unwrap_or("");
        if normalize_http_probe_url(target).is_none() {
            skipped += 1;
            next_mappings.push(Value::Object(object));
            continue;
        }
        match fetch_host_mapping_metadata(target, object.get("basic_auth")).await {
            Ok(metadata) => {
                object.insert(
                    "title".to_string(),
                    metadata
                        .get("title")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                );
                object.insert(
                    "favicon".to_string(),
                    metadata
                        .get("favicon")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                );
                updated += 1;
            }
            Err(error) => {
                tracing::debug!(%error, target, "failed to refresh host mapping metadata");
                failed += 1;
            }
        }
        next_mappings.push(Value::Object(object));
    }

    (
        next_mappings,
        json!({
            "updated": updated,
            "failed": failed,
            "skipped": skipped,
        }),
    )
}

fn schedule_host_mappings_metadata_refresh(
    state: AppState,
    mappings: Vec<Value>,
    previous_mappings: Vec<Value>,
) {
    tokio::spawn(async move {
        let (items, summary) =
            enrich_host_mapping_metadata_for_save(mappings, previous_mappings).await;
        tracing::debug!(
            updated = summary.updated,
            failed = summary.failed,
            skipped = summary.skipped,
            "host mappings metadata background refresh finished"
        );
        if summary.updated == 0 {
            return;
        }

        let current_config = match state.redis.get_config().await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "failed to load config before merging host mappings metadata refresh"
                );
                return;
            }
        };
        let current_mappings = current_config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let (next_mappings, changed) =
            merge_metadata_into_current_mappings(current_mappings, items);
        if !changed {
            return;
        }

        let mut next_config = current_config.clone();
        ensure_object(&mut next_config).insert(
            "host_mappings".to_string(),
            Value::Array(next_mappings.clone()),
        );
        if let Err(error) = state.redis.save_config(&next_config).await {
            tracing::warn!(
                %error,
                "failed to save host mappings after metadata background refresh"
            );
            return;
        }
        if let Err(message) =
            sync_gateway_portal_host_rules_if_title_mode(&state, &next_config, &next_mappings).await
        {
            tracing::warn!(
                %message,
                "failed to sync refreshed host mapping metadata to gateway"
            );
        }
    });
}

async fn enrich_host_mapping_metadata_for_save(
    mappings: Vec<Value>,
    previous_mappings: Vec<Value>,
) -> (
    Vec<HostMappingMetadataRefreshItem>,
    HostMappingMetadataRefreshSummary,
) {
    let previous_by_host = previous_mappings
        .into_iter()
        .map(|mapping| (host_mapping_key(&mapping), mapping))
        .collect::<HashMap<_, _>>();
    let mut summary = HostMappingMetadataRefreshSummary::default();
    let mut items = Vec::new();

    for mapping in mappings {
        let Some(object) = mapping.as_object() else {
            summary.skipped += 1;
            continue;
        };
        let (refresh_title, refresh_favicon) =
            resolve_metadata_refresh_decision(&mapping, &previous_by_host);
        if !refresh_title && !refresh_favicon {
            summary.skipped += 1;
            continue;
        }

        let target = object.get("target").and_then(Value::as_str).unwrap_or("");
        match fetch_host_mapping_metadata(target, object.get("basic_auth")).await {
            Ok(metadata) => {
                let mut refreshed = object.clone();
                if refresh_title {
                    refreshed.insert(
                        "title".to_string(),
                        Value::String(metadata_string(&metadata, "title")),
                    );
                }
                if refresh_favicon {
                    refreshed.insert(
                        "favicon".to_string(),
                        Value::String(metadata_string(&metadata, "favicon")),
                    );
                }
                items.push(HostMappingMetadataRefreshItem {
                    mapping: Value::Object(refreshed),
                    refresh_title,
                    refresh_favicon,
                });
                summary.updated += 1;
            }
            Err(error) => {
                tracing::debug!(%error, target, "failed to refresh host mapping metadata on save");
                summary.failed += 1;
            }
        }
    }

    (items, summary)
}

fn resolve_metadata_refresh_decision(
    mapping: &Value,
    previous_by_host: &HashMap<String, Value>,
) -> (bool, bool) {
    let target = mapping_target(mapping);
    if target.is_empty() || normalize_http_probe_url(&target).is_none() {
        return (false, false);
    }

    let previous = previous_by_host.get(&host_mapping_key(mapping));
    let target_changed = previous
        .map(|previous| mapping_target(previous) != target)
        .unwrap_or(true);
    let basic_auth_changed = host_mapping_has_usable_basic_auth(mapping)
        && previous
            .map(|previous| !host_mapping_basic_auth_matches(previous, mapping))
            .unwrap_or(true);
    let refresh_title =
        target_changed || basic_auth_changed || metadata_string(mapping, "title").is_empty();
    let refresh_favicon =
        target_changed || basic_auth_changed || metadata_string(mapping, "favicon").is_empty();

    (refresh_title, refresh_favicon)
}

fn merge_metadata_into_current_mappings(
    current_mappings: Vec<Value>,
    refreshed_items: Vec<HostMappingMetadataRefreshItem>,
) -> (Vec<Value>, bool) {
    let refreshed_by_host = refreshed_items
        .into_iter()
        .map(|item| (host_mapping_key(&item.mapping), item))
        .collect::<HashMap<_, _>>();
    let mut changed = false;
    let next_mappings = current_mappings
        .into_iter()
        .map(|mapping| {
            let Some(refreshed) = refreshed_by_host.get(&host_mapping_key(&mapping)) else {
                return mapping;
            };
            if mapping_target(&mapping) != mapping_target(&refreshed.mapping)
                || !host_mapping_basic_auth_matches(&mapping, &refreshed.mapping)
            {
                return mapping;
            }

            let Some(object) = mapping.as_object() else {
                return mapping;
            };
            let mut next = object.clone();
            let current_title = metadata_string(&mapping, "title");
            let current_favicon = metadata_string(&mapping, "favicon");
            let next_title = if refreshed.refresh_title {
                metadata_string(&refreshed.mapping, "title")
            } else {
                current_title.clone()
            };
            let next_favicon = if refreshed.refresh_favicon {
                metadata_string(&refreshed.mapping, "favicon")
            } else {
                current_favicon.clone()
            };

            if next_title == current_title && next_favicon == current_favicon {
                return mapping;
            }

            next.insert("title".to_string(), Value::String(next_title));
            next.insert("favicon".to_string(), Value::String(next_favicon));
            changed = true;
            Value::Object(next)
        })
        .collect();
    (next_mappings, changed)
}

async fn sync_gateway_portal_host_rules_if_title_mode(
    state: &AppState,
    config: &Value,
    mappings: &[Value],
) -> Result<bool, String> {
    if !is_gateway_portal_title_mode(config) || !is_any_subdomain_routing_mode(config) {
        return Ok(false);
    }
    sync_go_host_rules(state, &build_host_rules_payload(mappings)).await?;
    Ok(true)
}

fn is_gateway_portal_title_mode(config: &Value) -> bool {
    config
        .pointer("/gateway_portal/display_style")
        .and_then(Value::as_str)
        != Some("domain")
}

fn host_mapping_key(value: &Value) -> String {
    normalize_host_value(value.get("host").and_then(Value::as_str).unwrap_or(""))
}

fn mapping_target(value: &Value) -> String {
    value
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn metadata_string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

fn host_mapping_basic_auth_matches(left: &Value, right: &Value) -> bool {
    normalize_host_basic_auth(left.get("basic_auth"))
        == normalize_host_basic_auth(right.get("basic_auth"))
}

fn host_mapping_has_usable_basic_auth(value: &Value) -> bool {
    normalize_host_basic_auth(value.get("basic_auth"))
        .get("enabled")
        .and_then(Value::as_bool)
        == Some(true)
}

fn build_bookmarks_document(config: &Value, translator: &crate::i18n::Translator) -> String {
    let scheme = resolve_bookmark_scheme(config);
    let raw_public_base_url = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let resolved_public_port =
        resolve_public_port_for_scheme(config, scheme, raw_public_base_url, true, false);
    let access_entry_port = resolved_public_port
        .map(|port| port.to_string())
        .unwrap_or_else(|| crate::system_info::resolve_access_entry_port(config));
    let omit_access_entry_port =
        should_omit_public_access_entry_port(config) && resolved_public_port.is_none();
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let folder_title = if root_domain.is_empty() {
        translator.t("server.admin.hostMappings.bookmarkFolderDefault")
    } else {
        translator.t_params(
            "server.admin.hostMappings.bookmarkFolderForRoot",
            &[("root", root_domain.to_string())],
        )
    };
    let add_date = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut lines = vec![
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>".to_string(),
        "<!-- This is an automatically generated file.".to_string(),
        "     It will be read and overwritten.".to_string(),
        "     DO NOT EDIT! -->".to_string(),
        "<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">".to_string(),
        "<TITLE>Bookmarks</TITLE>".to_string(),
        "<H1>Bookmarks</H1>".to_string(),
        "<DL><p>".to_string(),
        format!(
            "  <DT><H3 ADD_DATE=\"{add_date}\" LAST_MODIFIED=\"{add_date}\">{}</H3>",
            escape_html(&folder_title)
        ),
        "  <DL><p>".to_string(),
    ];
    if let Some(mappings) = config.get("host_mappings").and_then(Value::as_array) {
        for mapping in mappings {
            let Some(object) = mapping.as_object() else {
                continue;
            };
            if object
                .get("target")
                .and_then(Value::as_str)
                .is_some_and(is_auth_service_target)
            {
                continue;
            }
            let host = object
                .get("host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
                .unwrap_or_default();
            if host.is_empty() {
                continue;
            }
            let href = build_bookmark_url(
                &host,
                scheme,
                Some(&access_entry_port),
                omit_access_entry_port,
            );
            let title = resolve_bookmark_title(object, &host);
            lines.push(format!(
                "    <DT><A HREF=\"{}\" ADD_DATE=\"{add_date}\">{}</A>",
                escape_html(&href),
                escape_html(&title)
            ));
        }
    }
    lines.push("  </DL><p>".to_string());
    lines.push("</DL><p>".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn resolve_bookmark_scheme(config: &Value) -> &'static str {
    let cert = config
        .pointer("/ssl/cert")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let key = config
        .pointer("/ssl/key")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !cert.is_empty() && !key.is_empty() {
        "https"
    } else {
        "http"
    }
}

fn build_bookmark_url(
    host: &str,
    scheme: &str,
    access_entry_port: Option<&str>,
    omit_access_entry_port: bool,
) -> String {
    if omit_access_entry_port {
        return format!("{scheme}://{host}/");
    }
    let port = resolve_bookmark_access_entry_port(access_entry_port);
    let parsed_port = parse_js_parse_int_radix_10(&port);
    let port_suffix = if port.is_empty()
        || parsed_port.is_some_and(|port| is_default_scheme_port(scheme, port))
    {
        String::new()
    } else {
        format!(":{port}")
    };
    format!("{scheme}://{host}{port_suffix}/")
}

fn resolve_bookmark_access_entry_port(access_entry_port: Option<&str>) -> String {
    let normalized = access_entry_port.unwrap_or("").trim();
    if normalized.is_empty() {
        "7999".to_string()
    } else {
        normalized.to_string()
    }
}

fn resolve_bookmark_title(object: &Map<String, Value>, host: &str) -> String {
    let title_override = object
        .get("title_override")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !title_override.is_empty() {
        return title_override.to_string();
    }
    object
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(host)
        .to_string()
}

fn build_bookmark_filename(config: &Value) -> String {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let normalized = normalize_bookmark_filename_part(root_domain);
    if normalized.is_empty() {
        "fn-knock-bookmarks.html".to_string()
    } else {
        format!("fn-knock-bookmarks-{normalized}.html")
    }
}

fn normalize_bookmark_filename_part(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
            output.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    output.trim_matches('-').to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn ensure_go_success(value: Value) -> Result<(), String> {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    Err(value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Go backend returned an unsuccessful response")
        .to_string())
}

async fn rollback_proxy_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback proxy mappings config");
        return;
    }
    let previous_rules = previous_config
        .get("proxy_mappings")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    if let Err(error) = sync_go_rules(state, &previous_rules).await {
        tracing::warn!(%error, "failed to rollback proxy mappings runtime");
    }
}

async fn rollback_host_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback host mappings config");
        return;
    }
    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Err(error) = sync_host_mappings_runtime(state, previous_config, &previous_mappings).await
    {
        tracing::warn!(%error, "failed to rollback host mappings runtime");
    }
}

async fn rollback_stream_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings config");
        return;
    }
    if let Err(error) = sync_stream_mappings_runtime(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings runtime");
    }
}

async fn rollback_subdomain_mode(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback subdomain mode config");
        return;
    }
    if let Err(error) = sync_go_auth_config(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback subdomain mode runtime");
    }
}

fn normalize_proxy_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, &'static str> {
    let mut normalized = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Proxy mapping must be an object");
        };
        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err("Proxy mapping target must be a supported HTTP/WebSocket URL");
        }
        object.insert("target".to_string(), Value::String(target));
        normalized.push(Value::Object(object));
    }
    Ok(normalized)
}

fn normalize_host_mappings_for_route(
    mappings: Vec<Value>,
    previous_config: &Value,
) -> Result<Vec<Value>, String> {
    let previous_by_host = previous_host_mappings_by_host(previous_config);
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut has_default_mapping = false;
    let mut auth_mapping_count = 0;

    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Host mapping must be an object".to_string());
        };
        let host = normalize_host_value(object.get("host").and_then(Value::as_str).unwrap_or(""));
        if host.is_empty() {
            return Err("Host mapping host is required".to_string());
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err(format!(
                "Host mapping {host} target must be a supported HTTP/WebSocket URL"
            ));
        }

        let service_role = if is_auth_service_target(&target) {
            "auth"
        } else {
            "app"
        };
        if service_role == "auth" {
            auth_mapping_count += 1;
            if auth_mapping_count > 1 {
                return Err("Only one auth service host mapping is allowed".to_string());
            }
            if object
                .get("use_auth")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                || object
                    .get("access_mode")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == "strict_whitelist")
            {
                return Err(format!("Auth host mapping {host} must be public"));
            }
            if host_basic_auth_enabled(object.get("basic_auth")) {
                return Err(format!("Auth host mapping {host} cannot enable Basic Auth"));
            }
        } else if host_basic_auth_invalid(object.get("basic_auth")) {
            return Err(format!(
                "Host mapping {host} Basic Auth settings are invalid"
            ));
        }

        let locations = if service_role == "auth" {
            Vec::new()
        } else {
            normalize_host_mapping_locations_for_route(&host, object.get("locations"))?
        };

        let previous = previous_by_host.get(&host);
        let can_reuse_previous_metadata = previous
            .and_then(|value| value.get("target"))
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim() == target);
        let normalized_basic_auth = if service_role == "auth" {
            disabled_host_basic_auth()
        } else {
            normalize_host_basic_auth(
                object
                    .get("basic_auth")
                    .or_else(|| previous.and_then(|value| value.get("basic_auth"))),
            )
        };
        let is_default = service_role != "auth"
            && object
                .get("is_default")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && !has_default_mapping;
        if is_default {
            has_default_mapping = true;
        }

        object.insert("host".to_string(), Value::String(host));
        object.insert("target".to_string(), Value::String(target));
        object.insert(
            "use_auth".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("use_auth")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
            ),
        );
        object.insert(
            "access_mode".to_string(),
            Value::String(if service_role == "auth" {
                "login_first".to_string()
            } else {
                normalize_access_mode(object.get("access_mode"))
            }),
        );
        object.insert(
            "suppress_toolbar".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("suppress_toolbar")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
            ),
        );
        object.insert(
            "preserve_host".to_string(),
            Value::Bool(
                object
                    .get("preserve_host")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
        );
        object.insert("is_default".to_string(), Value::Bool(is_default));
        object.insert("basic_auth".to_string(), normalized_basic_auth);
        object.insert("locations".to_string(), Value::Array(locations));
        object.insert(
            "service_role".to_string(),
            Value::String(service_role.to_string()),
        );
        object.insert(
            "title".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title"),
                previous,
                "title",
                can_reuse_previous_metadata,
            )),
        );
        object.insert(
            "title_override".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title_override"),
                previous,
                "title_override",
                true,
            )),
        );
        object.insert(
            "favicon".to_string(),
            Value::String(normalize_metadata_string(
                object.get("favicon"),
                previous,
                "favicon",
                can_reuse_previous_metadata,
            )),
        );
        normalized.push(Value::Object(object));
    }

    Ok(normalized)
}

fn normalize_stream_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, String> {
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut seen = HashSet::new();
    for mapping in mappings {
        let Some(object) = mapping.as_object() else {
            return Err("Stream mapping must be an object".to_string());
        };
        let protocol = if object
            .get("protocol")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "udp")
        {
            "udp"
        } else {
            "tcp"
        };
        let Some(listen_port) = object.get("listen_port").and_then(json_integer) else {
            return Err("Stream mapping listen_port must be an integer".to_string());
        };
        if listen_port <= 0 || listen_port > 65535 {
            return Err(format!(
                "Stream mapping listen_port {listen_port} is out of range"
            ));
        }
        let key = format!("{protocol}:{listen_port}");
        if !seen.insert(key) {
            return Err(format!(
                "Duplicate stream mapping for {} port {listen_port}",
                protocol.to_ascii_uppercase()
            ));
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_valid_host_port(&target) {
            return Err(format!("Stream mapping target must be host:port: {target}"));
        }

        normalized.push(json!({
            "protocol": protocol,
            "listen_port": listen_port,
            "target": target,
            "use_auth": object.get("use_auth").and_then(Value::as_bool).unwrap_or(true),
        }));
    }
    Ok(normalized)
}

fn validate_host_mappings_section(config: &Value) -> Result<(), String> {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    normalize_host_mappings_for_route(mappings, config).map(|_| ())
}

fn validate_passkey_rp_config(config: &Value) -> Result<(), String> {
    let Some(subdomain_mode) = config.get("subdomain_mode").and_then(Value::as_object) else {
        return Ok(());
    };
    let mode = subdomain_mode
        .get("passkey_rp_mode")
        .and_then(Value::as_str)
        .unwrap_or("auth_host");
    if mode != "parent_domain" {
        return Ok(());
    }

    let rp_id = normalize_host_value(
        subdomain_mode
            .get("passkey_rp_id")
            .and_then(Value::as_str)
            .or_else(|| subdomain_mode.get("root_domain").and_then(Value::as_str))
            .unwrap_or(""),
    );
    if rp_id.is_empty() {
        return Err("Passkey parent-domain RP ID is required".to_string());
    }

    let auth_host = get_auth_host_mapping(config)
        .and_then(|mapping| {
            mapping
                .get("host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .or_else(|| {
            subdomain_mode
                .get("auth_host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .unwrap_or_default();

    if !auth_host.is_empty() && auth_host != rp_id && !auth_host.ends_with(&format!(".{rp_id}")) {
        return Err(format!(
            "Passkey auth host {auth_host} must match or belong to RP ID {rp_id}"
        ));
    }
    Ok(())
}

fn normalize_host_mapping_locations_for_route(
    host: &str,
    value: Option<&Value>,
) -> Result<Vec<Value>, String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut normalized = Vec::with_capacity(items.len());
    let mut seen = HashSet::new();

    for item in items {
        let object = item.as_object().cloned().unwrap_or_else(Map::new);
        let raw_path = object
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if raw_path.is_empty() {
            return Err(format!("Host mapping {host} location path is required"));
        }
        if !raw_path.starts_with('/') {
            return Err(format!(
                "Host mapping {host} location path {raw_path} must start with /"
            ));
        }
        let path = clean_host_location_path(raw_path);
        if path == "/" {
            return Err(format!("Host mapping {host} location path / is reserved"));
        }
        if path.starts_with("/__") || path == "/s" || path == "/s/" {
            return Err(format!(
                "Host mapping {host} location path {path} is reserved"
            ));
        }
        let match_mode = if object
            .get("match")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "exact")
        {
            "exact"
        } else {
            "prefix"
        };
        let duplicate_key = format!("{match_mode}\0{path}");
        if !seen.insert(duplicate_key) {
            return Err(format!("Host mapping {host} has duplicate location {path}"));
        }

        let action = if object
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "response")
        {
            "response"
        } else {
            "proxy"
        };
        let target = if action == "proxy" {
            let target = object
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if target.is_empty() {
                return Err(format!(
                    "Host mapping {host} location {path} target is required"
                ));
            }
            if !is_supported_proxy_target_url(&target) {
                return Err(format!(
                    "Host mapping {host} location {path} target must be a supported HTTP/WebSocket URL"
                ));
            }
            target
        } else {
            String::new()
        };

        if action == "response" {
            validate_location_response(host, &path, object.get("response"))?;
        }

        normalized.push(json!({
            "path": path,
            "match": match_mode,
            "action": action,
            "target": target,
            "strip_path": action == "proxy" && object.get("strip_path").and_then(Value::as_bool).unwrap_or(true),
            "rewrite_html": action == "proxy" && object.get("rewrite_html").and_then(Value::as_bool).unwrap_or(true),
            "response": if action == "response" {
                normalize_location_response(object.get("response"))
            } else {
                normalize_location_response(None)
            },
        }));
    }

    Ok(normalized)
}

fn validate_location_response(host: &str, path: &str, value: Option<&Value>) -> Result<(), String> {
    let object = value.and_then(Value::as_object);
    let status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    if !(100..=599).contains(&status) {
        return Err(format!(
            "Host mapping {host} location {path} response status is invalid"
        ));
    }

    let headers = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object);
    if let Some(headers) = headers {
        for raw_name in headers.keys() {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {raw_name} is invalid"
                ));
            }
            if forbidden_response_header(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {name} is forbidden"
                ));
            }
        }
    }
    Ok(())
}

fn normalize_location_response(value: Option<&Value>) -> Value {
    let object = value.and_then(Value::as_object);
    let raw_status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    let status = if (100..=599).contains(&raw_status) {
        raw_status
    } else {
        200
    };
    let content_type = object
        .and_then(|map| map.get("content_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE);

    let mut headers = Map::new();
    if let Some(header_map) = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object)
    {
        for (raw_name, raw_value) in header_map {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) || forbidden_response_header(name) {
                continue;
            }
            headers.insert(
                name.to_string(),
                Value::String(
                    raw_value
                        .as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| raw_value.to_string()),
                ),
            );
        }
    }

    json!({
        "status": status,
        "content_type": content_type,
        "headers": headers,
        "body": object
            .and_then(|map| map.get("body"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    })
}

pub(crate) fn build_host_rules_payload(mappings: &[Value]) -> Value {
    Value::Array(
        mappings
            .iter()
            .filter_map(Value::as_object)
            .map(|object| {
                let title = resolve_host_rule_title(object);
                let favicon = object
                    .get("favicon")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| Value::String(value.to_string()))
                    .unwrap_or(Value::Null);
                json!({
                    "host": object.get("host").cloned().unwrap_or(Value::String(String::new())),
                    "target": object.get("target").cloned().unwrap_or(Value::String(String::new())),
                    "use_auth": object.get("use_auth").cloned().unwrap_or(Value::Bool(true)),
                    "access_mode": object.get("access_mode").cloned().unwrap_or(Value::String("login_first".to_string())),
                    "suppress_toolbar": object.get("suppress_toolbar").cloned().unwrap_or(Value::Bool(false)),
                    "preserve_host": object.get("preserve_host").cloned().unwrap_or(Value::Bool(true)),
                    "is_default": object.get("is_default").cloned().unwrap_or(Value::Bool(false)),
                    "title": title,
                    "favicon": favicon,
                    "basic_auth": object.get("basic_auth").cloned().unwrap_or_else(disabled_host_basic_auth),
                    "locations": object.get("locations").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                })
            })
            .collect(),
    )
}

fn resolve_host_rule_title(object: &Map<String, Value>) -> String {
    let override_title = object
        .get("title_override")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !override_title.is_empty() {
        return override_title.to_string();
    }
    object
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn build_gateway_auth_config(config: &Value) -> Value {
    let subdomain_mode = config
        .get("subdomain_mode")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let is_subdomain_mode_active = is_any_subdomain_routing_mode(config);
    let is_reverse_subdomain_mode = is_reverse_proxy_subdomain_mode(config);
    let default_auth_port = resolve_auth_service_port();
    let auth_mapping = get_auth_host_mapping(config);
    let explicit_public_auth_base_url = if is_subdomain_mode_active && !is_reverse_subdomain_mode {
        apply_public_port_to_base_url(
            subdomain_mode
                .get("public_auth_base_url")
                .and_then(Value::as_str)
                .unwrap_or(""),
            config,
        )
    } else {
        String::new()
    };
    let auth_target = auth_mapping
        .and_then(|mapping| mapping.get("target").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            subdomain_mode
                .get("auth_target")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("");
    let auth_port = parse_target_port(auth_target).unwrap_or(default_auth_port);
    let public_auth_base_url = if is_subdomain_mode_active {
        if explicit_public_auth_base_url.is_empty() {
            resolve_public_auth_base_url(config)
        } else {
            explicit_public_auth_base_url
        }
    } else {
        String::new()
    };
    let edge_client_ip_enabled = config.get("run_type").and_then(Value::as_i64) == Some(3)
        && subdomain_mode
            .get("edge_client_ip_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let tencent_edgeone_enabled = edge_client_ip_enabled
        && subdomain_mode
            .get("tencent_edgeone_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let aliyun_esa_enabled = edge_client_ip_enabled
        && !tencent_edgeone_enabled
        && subdomain_mode
            .get("aliyun_esa_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let public_http_port = if is_subdomain_mode_active {
        resolve_auth_public_port_for_scheme(config, "http", &public_auth_base_url, false)
            .unwrap_or(0)
    } else {
        0
    };
    let public_https_port = if is_subdomain_mode_active {
        resolve_auth_public_port_for_scheme(config, "https", &public_auth_base_url, true)
            .unwrap_or(0)
    } else {
        0
    };
    let auth_host = if is_subdomain_mode_active {
        auth_mapping
            .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                subdomain_mode
                    .get("auth_host")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    json!({
        "auth_port": auth_port,
        "auth_url": "/api/auth/verify",
        "login_url": "/login",
        "logout_url": "/api/auth/logout",
        "preflight_url": "/api/auth/preflight",
        "auth_cache_ttl_seconds": subdomain_mode
            .get("auth_cache_ttl_seconds")
            .and_then(json_number_floor)
            .unwrap_or(1),
        "auth_cache_unauthorized_ttl_seconds": subdomain_mode
            .get("auth_cache_unauthorized_ttl_seconds")
            .and_then(json_number_floor)
            .unwrap_or(1),
        "edge_client_ip_enabled": edge_client_ip_enabled && (aliyun_esa_enabled || tencent_edgeone_enabled),
        "aliyun_esa_enabled": aliyun_esa_enabled,
        "tencent_edgeone_enabled": tencent_edgeone_enabled,
        "public_auth_base_url": public_auth_base_url,
        "public_http_port": public_http_port,
        "public_https_port": public_https_port,
        "auth_host": auth_host,
        "trust_forwarded_proto": is_cloudflared_reverse_proxy_subdomain_mode(config),
    })
}

fn normalize_subdomain_mode_config(value: &Value) -> Value {
    let object = value.as_object().cloned().unwrap_or_default();
    let mut edge_client_ip_enabled = object
        .get("edge_client_ip_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut aliyun_esa_enabled = object
        .get("aliyun_esa_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut tencent_edgeone_enabled = object
        .get("tencent_edgeone_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !object.contains_key("edge_client_ip_enabled")
        && (aliyun_esa_enabled || tencent_edgeone_enabled)
    {
        edge_client_ip_enabled = true;
    }
    if !edge_client_ip_enabled {
        aliyun_esa_enabled = false;
        tencent_edgeone_enabled = false;
    }
    if tencent_edgeone_enabled && aliyun_esa_enabled {
        aliyun_esa_enabled = false;
    }

    json!({
        "root_domain": object.get("root_domain").and_then(Value::as_str).map(|value| value.trim().to_ascii_lowercase()).unwrap_or_default(),
        "auth_host": normalize_host_value(object.get("auth_host").and_then(Value::as_str).unwrap_or("")),
        "auth_target": object.get("auth_target").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string).unwrap_or_else(default_subdomain_auth_target),
        "cookie_domain": object.get("cookie_domain").and_then(Value::as_str).map(str::trim).unwrap_or("").to_string(),
        "edge_client_ip_enabled": edge_client_ip_enabled,
        "aliyun_esa_enabled": aliyun_esa_enabled,
        "tencent_edgeone_enabled": tencent_edgeone_enabled,
        "public_auth_base_url": object.get("public_auth_base_url").and_then(Value::as_str).map(|value| value.trim().trim_end_matches('/').to_string()).unwrap_or_default(),
        "public_http_port": normalize_public_port(object.get("public_http_port")),
        "public_https_port": normalize_public_port(object.get("public_https_port")),
        "auth_cache_ttl_seconds": normalize_cache_ttl(object.get("auth_cache_ttl_seconds"), 1),
        "auth_cache_unauthorized_ttl_seconds": normalize_cache_ttl(object.get("auth_cache_unauthorized_ttl_seconds"), 1),
        "default_access_mode": normalize_access_mode(object.get("default_access_mode")),
        "auto_add_whitelist_on_login": object.get("auto_add_whitelist_on_login").and_then(Value::as_bool).unwrap_or(true),
        "passkey_rp_mode": if object.get("passkey_rp_mode").and_then(Value::as_str) == Some("parent_domain") { "parent_domain" } else { "auth_host" },
        "passkey_rp_id": object.get("passkey_rp_id").and_then(Value::as_str).map(|value| value.trim().to_ascii_lowercase()).unwrap_or_default(),
    })
}

fn previous_host_mappings_by_host(config: &Value) -> HashMap<String, Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("host")
                        .and_then(Value::as_str)
                        .map(|host| (normalize_host_value(host), item.clone()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

fn get_auth_host_mapping(config: &Value) -> Option<&Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| {
            mapping
                .get("target")
                .and_then(Value::as_str)
                .is_some_and(is_auth_service_target)
        })
}

fn normalize_metadata_string(
    input: Option<&Value>,
    previous: Option<&Value>,
    previous_key: &str,
    can_reuse_previous: bool,
) -> String {
    if let Some(value) = input.and_then(Value::as_str) {
        return value.trim().to_string();
    }
    if can_reuse_previous {
        return previous
            .and_then(|value| value.get(previous_key))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
    }
    String::new()
}

fn normalize_host_basic_auth(value: Option<&Value>) -> Value {
    if host_basic_auth_invalid(value) || !host_basic_auth_enabled(value) {
        return disabled_host_basic_auth();
    }
    let object = value.and_then(Value::as_object).expect("basic auth object");
    json!({
        "enabled": true,
        "username": object.get("username").and_then(Value::as_str).unwrap_or("").trim(),
        "password": object.get("password").and_then(Value::as_str).unwrap_or(""),
    })
}

fn disabled_host_basic_auth() -> Value {
    json!({
        "enabled": false,
        "username": "",
        "password": "",
    })
}

fn host_basic_auth_enabled(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn host_basic_auth_invalid(value: Option<&Value>) -> bool {
    if !host_basic_auth_enabled(value) {
        return false;
    }
    let Some(object) = value.and_then(Value::as_object) else {
        return true;
    };
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let password = object.get("password").and_then(Value::as_str).unwrap_or("");
    username.is_empty() || password.is_empty() || username.contains(':')
}

fn is_supported_proxy_target_url(value: &str) -> bool {
    let target = value.trim();
    if target.is_empty() || has_explicit_empty_port(target) {
        return false;
    }
    let Ok(parsed) = Url::parse(target) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https" | "ws" | "wss")
        && parsed
            .host_str()
            .is_some_and(|host| !host.trim().is_empty())
}

fn has_explicit_empty_port(value: &str) -> bool {
    let Some((_, endpoint)) = value.trim().split_once("://") else {
        return false;
    };
    let boundary = endpoint.find(['/', '?', '#']).unwrap_or(endpoint.len());
    let authority_with_credentials = &endpoint[..boundary];
    let authority = authority_with_credentials
        .rsplit_once('@')
        .map(|(_, authority)| authority)
        .unwrap_or(authority_with_credentials);
    authority.ends_with(':')
}

fn is_valid_host_port(value: &str) -> bool {
    let target = value.trim();
    if target.is_empty()
        || target.contains("://")
        || target.contains('/')
        || target.chars().any(char::is_whitespace)
    {
        return false;
    }
    if let Some(rest) = target.strip_prefix('[') {
        let Some((host, port_part)) = rest.split_once("]:") else {
            return false;
        };
        return !host.trim().is_empty() && valid_port_string(port_part);
    }
    let Some((host, port_part)) = target.rsplit_once(':') else {
        return false;
    };
    !host.trim().is_empty() && !host.contains(':') && valid_port_string(port_part)
}

fn valid_port_string(value: &str) -> bool {
    let Ok(port) = value.parse::<u16>() else {
        return false;
    };
    port > 0
}

fn is_auth_service_target(target: &str) -> bool {
    is_supported_proxy_target_url(target)
        && parse_target_port(target).is_some_and(|port| port == resolve_auth_service_port())
}

fn parse_target_port(target: &str) -> Option<i64> {
    let normalized = target.trim();
    if normalized.is_empty() {
        return None;
    }
    if let Ok(parsed) = Url::parse(normalized) {
        if let Some(port) = parsed.port() {
            return Some(i64::from(port));
        }
        return match parsed.scheme() {
            "https" | "wss" => Some(443),
            "http" | "ws" => Some(80),
            _ => None,
        };
    }
    let (_, tail) = normalized.rsplit_once(':')?;
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits
        .parse::<i64>()
        .ok()
        .filter(|port| *port > 0 && *port <= 65535)
}

fn resolve_auth_service_port() -> i64 {
    parse_env_port_with_fallback("AUTH_PORT", 7997)
}

fn default_subdomain_auth_target() -> String {
    format!("http://localhost:{}", resolve_auth_service_port())
}

fn normalize_host_value(value: &str) -> String {
    let without_scheme = value
        .trim()
        .to_ascii_lowercase()
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| value.trim().to_ascii_lowercase());
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_string()
}

fn normalize_access_mode(value: Option<&Value>) -> String {
    if value.and_then(Value::as_str) == Some("strict_whitelist") {
        "strict_whitelist".to_string()
    } else {
        "login_first".to_string()
    }
}

fn clean_host_location_path(value: &str) -> String {
    let raw = value.trim();
    if !raw.starts_with('/') {
        return raw.to_string();
    }
    let mut segments = Vec::new();
    for segment in raw.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            segments.pop();
            continue;
        }
        segments.push(segment);
    }
    format!("/{}", segments.join("/"))
}

fn is_valid_http_header_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
                    | b'0'..=b'9'
                    | b'A'..=b'Z'
                    | b'a'..=b'z'
            )
        })
}

fn forbidden_response_header(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "content-type"
    )
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("value is object")
}

fn json_integer(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let number = value.as_f64()?;
    if number.is_finite() && number.fract() == 0.0 {
        Some(number as i64)
    } else {
        None
    }
}

fn json_number_floor(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let number = value.as_f64()?;
    if number.is_finite() {
        Some(number.floor() as i64)
    } else {
        None
    }
}

fn normalize_public_port(value: Option<&Value>) -> i64 {
    json_number_floor_value_or_parse(value)
        .filter(|port| *port > 0 && *port <= 65535)
        .unwrap_or(0)
}

fn normalize_cache_ttl(value: Option<&Value>, fallback: i64) -> i64 {
    json_number_floor_value_or_parse(value)
        .filter(|ttl| *ttl >= 0)
        .unwrap_or(fallback)
}

fn json_number_floor_value_or_parse(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(raw) => raw.trim().parse::<i64>().ok(),
        other => json_number_floor(other),
    }
}

fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || is_reverse_proxy_subdomain_mode(config)
}

fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
        && config
            .get("reverse_proxy_submode")
            .and_then(Value::as_str)
            .unwrap_or("path")
            == "subdomain"
}

fn is_cloudflared_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    is_reverse_proxy_subdomain_mode(config)
        && config
            .get("default_tunnel")
            .and_then(Value::as_str)
            .unwrap_or("frp")
            == "cloudflared"
}

fn should_omit_public_access_entry_port(config: &Value) -> bool {
    is_cloudflared_reverse_proxy_subdomain_mode(config)
        || (config.get("run_type").and_then(Value::as_i64) == Some(3)
            && config
                .pointer("/subdomain_mode/edge_client_ip_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && (config
                .pointer("/subdomain_mode/aliyun_esa_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || config
                    .pointer("/subdomain_mode/tencent_edgeone_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)))
}

fn resolve_public_gateway_port(config: &Value) -> Option<i64> {
    crate::system_info::resolve_public_gateway_port(config)
}

fn parse_env_port_with_fallback(name: &str, fallback: i64) -> i64 {
    parse_env_port_with_fallback_value(std::env::var(name).ok(), fallback)
}

fn parse_env_port_with_fallback_value(value: Option<String>, fallback: i64) -> i64 {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    parse_js_parse_int_radix_10(raw.trim_start())
        .filter(|port| *port > 0)
        .unwrap_or(fallback)
}

fn parse_js_parse_int_radix_10(value: &str) -> Option<i64> {
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }
    let mut end = 0;
    let mut has_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    has_digit
        .then(|| value[..end].parse::<i64>().ok())
        .flatten()
}

fn parse_explicit_url_port(raw_url: &str, scheme: &str) -> Option<i64> {
    let parsed = Url::parse(raw_url.trim()).ok()?;
    if parsed.scheme() != scheme {
        return None;
    }
    parsed.port().map(i64::from)
}

fn resolve_configured_public_port(
    config: &Value,
    scheme: &str,
    allow_reverse_proxy_configured_port: bool,
) -> Option<i64> {
    if is_reverse_proxy_subdomain_mode(config) && !allow_reverse_proxy_configured_port {
        return None;
    }
    let pointer = if scheme == "https" {
        "/subdomain_mode/public_https_port"
    } else {
        "/subdomain_mode/public_http_port"
    };
    config
        .pointer(pointer)
        .and_then(json_number_floor)
        .filter(|port| *port > 0)
}

fn resolve_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
    allow_reverse_proxy_configured_port: bool,
) -> Option<i64> {
    if let Some(port) = parse_explicit_url_port(raw_public_base_url, scheme) {
        return Some(port);
    }
    if let Some(port) =
        resolve_configured_public_port(config, scheme, allow_reverse_proxy_configured_port)
    {
        return Some(port);
    }
    if should_omit_public_access_entry_port(config) || !gateway_fallback {
        return None;
    }
    resolve_public_gateway_port(config)
}

fn resolve_auth_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
) -> Option<i64> {
    resolve_public_port_for_scheme(config, scheme, raw_public_base_url, gateway_fallback, true)
}

fn apply_public_port_to_base_url(raw_base_url: &str, config: &Value) -> String {
    let trimmed = raw_base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(mut parsed) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    let scheme = match parsed.scheme() {
        "http" => "http",
        "https" => "https",
        _ => return trimmed.to_string(),
    };
    if parsed.port().is_none()
        && let Some(port) = resolve_public_port_for_scheme(config, scheme, trimmed, true, false)
        && !is_default_scheme_port(scheme, port)
    {
        let _ = parsed.set_port(Some(port as u16));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if path.is_empty() { "/" } else { &path });
    parsed.to_string().trim_end_matches('/').to_string()
}

fn resolve_public_auth_base_url(config: &Value) -> String {
    let explicit = if is_reverse_proxy_subdomain_mode(config) {
        String::new()
    } else {
        apply_public_port_to_base_url(
            config
                .pointer("/subdomain_mode/public_auth_base_url")
                .and_then(Value::as_str)
                .unwrap_or(""),
            config,
        )
    };
    if !explicit.is_empty() {
        return explicit;
    }
    if let Some(host) = get_auth_host_mapping(config)
        .and_then(|mapping| mapping.get("host"))
        .and_then(Value::as_str)
        .filter(|host| !host.trim().is_empty())
    {
        return format_derived_public_auth_base_url(host, config, "https");
    }
    if let Some(host) = config
        .pointer("/subdomain_mode/auth_host")
        .and_then(Value::as_str)
        .filter(|host| !host.trim().is_empty())
    {
        return format_derived_public_auth_base_url(host, config, "https");
    }
    String::new()
}

fn format_derived_public_auth_base_url(host: &str, config: &Value, scheme: &str) -> String {
    let normalized_host = normalize_host_value(host);
    if normalized_host.is_empty() {
        return String::new();
    }
    let public_base = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let Some(port) = resolve_auth_public_port_for_scheme(config, scheme, public_base, true) else {
        return format!("{scheme}://{normalized_host}");
    };
    if is_default_scheme_port(scheme, port) {
        format!("{scheme}://{normalized_host}")
    } else {
        format!("{scheme}://{normalized_host}:{port}")
    }
}

fn is_default_scheme_port(scheme: &str, port: i64) -> bool {
    (scheme == "https" && port == 443) || (scheme == "http" && port == 80)
}

fn default_subdomain_mode() -> Value {
    json!({
        "root_domain": "",
        "auth_host": "",
        "auth_target": "http://127.0.0.1:7997",
        "cookie_domain": "",
        "edge_client_ip_enabled": false,
        "aliyun_esa_enabled": false,
        "tencent_edgeone_enabled": false,
        "public_auth_base_url": "",
        "public_http_port": 0,
        "public_https_port": 0,
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
        "default_access_mode": "login_first",
        "auto_add_whitelist_on_login": true,
        "passkey_rp_mode": "auth_host",
        "passkey_rp_id": ""
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn validates_supported_proxy_target_urls() {
        assert!(is_supported_proxy_target_url("http://127.0.0.1:8080"));
        assert!(is_supported_proxy_target_url("wss://example.com/socket"));
        assert!(!is_supported_proxy_target_url("ftp://example.com"));
        assert!(!is_supported_proxy_target_url("http://example.com:"));
        assert!(!is_supported_proxy_target_url("http://"));
    }

    #[test]
    fn normalizes_proxy_mapping_targets_without_touching_other_fields() {
        let mappings = normalize_proxy_mappings(vec![json!({
            "path": "/",
            "target": " http://127.0.0.1:8080 ",
            "rewrite_html": true,
            "use_auth": false,
            "use_root_mode": false,
            "strip_path": false
        })])
        .unwrap();
        assert_eq!(
            mappings[0].get("target").and_then(Value::as_str),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(mappings[0].get("rewrite_html"), Some(&Value::Bool(true)));
    }

    #[test]
    fn normalizes_host_mapping_route_shape() {
        let config = json!({
            "host_mappings": [{
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Old title",
                "favicon": "old.ico",
                "basic_auth": { "enabled": true, "username": "old", "password": "pw" }
            }]
        });
        let mappings = normalize_host_mappings_for_route(
            vec![json!({
                "host": "HTTPS://App.Example.Com/path",
                "target": " http://127.0.0.1:8080 ",
                "use_auth": true,
                "access_mode": "strict_whitelist",
                "locations": [{
                    "path": "/api/../health",
                    "match": "exact",
                    "action": "response",
                    "response": {
                        "status": 204,
                        "headers": { "X-Test": "ok" }
                    }
                }]
            })],
            &config,
        )
        .unwrap();
        let mapping_value = &mappings[0];
        let mapping = mapping_value.as_object().unwrap();
        assert_eq!(
            mapping.get("host").and_then(Value::as_str),
            Some("app.example.com")
        );
        assert_eq!(
            mapping.get("title").and_then(Value::as_str),
            Some("Old title")
        );
        assert_eq!(
            mapping_value.pointer("/basic_auth/enabled"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            mapping_value
                .pointer("/locations/0/path")
                .and_then(Value::as_str),
            Some("/health")
        );
        assert_eq!(
            mapping_value.pointer("/locations/0/response/headers/X-Test"),
            Some(&Value::String("ok".to_string()))
        );
    }

    #[test]
    fn extracts_host_mapping_metadata_helpers() {
        assert!(has_basic_auth_challenge(Some(
            "Bearer token, Basic realm=\"admin\""
        )));
        assert!(has_basic_auth_challenge(Some("basic")));
        assert!(!has_basic_auth_challenge(Some("Digest realm=\"admin\"")));
        assert_eq!(
            normalize_http_probe_url("https://example.com/app#fragment").as_deref(),
            Some("https://example.com/app")
        );
        assert_eq!(
            extract_html_title("<html><title> Fn &amp; Knock &#x4e2d; </title></html>"),
            "Fn & Knock 中"
        );
        assert_eq!(
            extract_favicon_url(
                r#"<link rel="shortcut icon" href="/assets/favicon.ico">"#,
                "https://example.com/ui/"
            )
            .as_deref(),
            Some("https://example.com/assets/favicon.ico")
        );
    }

    #[test]
    fn extracts_favicon_candidates_like_node_metadata() {
        let html = r#"
            <base href="https://static.example.com/app/">
            <link rel="apple-touch-icon" sizes="180x180" href="touch.png">
            <link rel="icon" type="image/svg+xml" sizes="any" href="favicon.svg">
        "#;
        assert_eq!(
            extract_favicon_url(html, "https://example.com/ui/").as_deref(),
            Some("https://static.example.com/app/favicon.svg")
        );

        let heuristic_html = r#"
            <meta name="msapplication-TileImage" content="/mstile-150x150.png">
            <img src="/logo.png">
            <img data-favicon="/assets/favicon-32.png">
        "#;
        let candidates = extract_heuristic_favicon_urls_from_html(
            heuristic_html,
            "https://example.com/admin/",
            HEURISTIC_FAVICON_MIN_PRIORITY,
        );
        assert_eq!(
            candidates.first().map(String::as_str),
            Some("https://example.com/assets/favicon-32.png")
        );
        assert!(
            candidates
                .iter()
                .any(|value| value == "https://example.com/mstile-150x150.png")
        );
    }

    #[test]
    fn extracts_manifest_icons_like_node_metadata() {
        let manifest_url = "https://example.com/app/manifest.webmanifest";
        let manifest = json!({
            "icons": [
                { "src": "/icon-maskable.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" },
                { "src": "icon-any.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
                { "src": "/not-image.txt", "sizes": "512x512", "type": "text/plain" },
                { "src": "icon-any.png", "sizes": "192x192", "type": "image/png" }
            ]
        });
        assert_eq!(
            extract_manifest_icon_urls(&manifest, manifest_url),
            vec![
                "https://example.com/app/icon-any.png".to_string(),
                "https://example.com/icon-maskable.png".to_string(),
            ]
        );
        assert_eq!(
            extract_manifest_from_html(
                r#"<link rel="manifest" href="/site.webmanifest">"#,
                "https://example.com/app/"
            )
            .as_deref(),
            Some("https://example.com/site.webmanifest")
        );
    }

    #[test]
    fn recognizes_openwrt_luci_and_fallback_favicon_paths() {
        let entrypoint = r#"
            <html><head>
              <meta http-equiv="refresh" content="0; url='/cgi-bin/luci/'">
            </head><body>LuCI - Lua Configuration Interface</body></html>
        "#;
        assert!(has_openwrt_luci_entrypoint_html(entrypoint));
        assert_eq!(
            extract_openwrt_luci_url_from_html(entrypoint, "https://router.example.com/")
                .as_deref(),
            Some("https://router.example.com/cgi-bin/luci/")
        );

        let document = r#"
            <html><head>
              <title>OpenWrt LuCI</title>
              <link rel="stylesheet" href="/luci-static/bootstrap/cascade.css">
            </head></html>
        "#;
        assert!(has_openwrt_luci_document_html(document));
        assert_eq!(
            resolve_fallback_favicon_urls("https://example.com/path/page"),
            vec![
                "https://example.com/favicon.ico".to_string(),
                "https://example.com/img/favicon.ico".to_string(),
                "https://example.com/public/favicon.png".to_string(),
            ]
        );
    }

    #[test]
    fn accepts_inline_and_same_origin_metadata_assets() {
        assert_eq!(
            normalize_favicon_url("data:image/png;base64,AA==", "https://example.com/").as_deref(),
            Some("data:image/png;base64,AA==")
        );

        let context = create_basic_auth_context(
            Some(&json!({
                "enabled": true,
                "username": "admin",
                "password": "pw"
            })),
            "https://example.com/app/",
        )
        .expect("basic auth context");
        assert!(has_same_origin(
            "https://example.com/assets/favicon.ico",
            &context.origin
        ));
        assert!(!has_same_origin(
            "https://cdn.example.com/assets/favicon.ico",
            &context.origin
        ));
    }

    #[tokio::test]
    async fn fetches_metadata_manifest_icon_as_data_url_like_node() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..3 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 2048];
                    let Ok(read_len) = socket.read(&mut buffer).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buffer[..read_len]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let (status, content_type, body): (&str, &str, Vec<u8>) = match path {
                        "/" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            br#"<!doctype html><title>Manifest App</title><link rel="manifest" href="/manifest.json">"#.to_vec(),
                        ),
                        "/manifest.json" => (
                            "200 OK",
                            "application/json",
                            br#"{"icons":[{"src":"/icon.png","sizes":"192x192","type":"image/png","purpose":"any"}]}"#.to_vec(),
                        ),
                        "/icon.png" => ("200 OK", "application/octet-stream", vec![1, 2, 3]),
                        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
                    };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });

        let metadata = fetch_host_mapping_metadata(&format!("http://{addr}/"), None)
            .await
            .unwrap();
        assert_eq!(
            metadata.get("title").and_then(Value::as_str),
            Some("Manifest App")
        );
        assert_eq!(
            metadata.get("favicon").and_then(Value::as_str),
            Some("data:image/png;base64,AQID")
        );
    }

    #[test]
    fn host_mapping_metadata_refresh_decision_matches_node_save_rules() {
        let previous_mappings = vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "Old",
            "favicon": "old.ico",
            "basic_auth": disabled_host_basic_auth()
        })];
        let previous_by_host = previous_mappings
            .into_iter()
            .map(|mapping| (host_mapping_key(&mapping), mapping))
            .collect::<HashMap<_, _>>();

        assert_eq!(
            resolve_metadata_refresh_decision(
                &json!({
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080",
                    "title": "Old",
                    "favicon": "old.ico",
                    "basic_auth": disabled_host_basic_auth()
                }),
                &previous_by_host
            ),
            (false, false)
        );
        assert_eq!(
            resolve_metadata_refresh_decision(
                &json!({
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080",
                    "title": "",
                    "favicon": "old.ico",
                    "basic_auth": disabled_host_basic_auth()
                }),
                &previous_by_host
            ),
            (true, false)
        );
        assert_eq!(
            resolve_metadata_refresh_decision(
                &json!({
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:9090",
                    "title": "Old",
                    "favicon": "old.ico",
                    "basic_auth": disabled_host_basic_auth()
                }),
                &previous_by_host
            ),
            (true, true)
        );
        assert_eq!(
            resolve_metadata_refresh_decision(
                &json!({
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080",
                    "title": "Old",
                    "favicon": "old.ico",
                    "basic_auth": { "enabled": true, "username": "admin", "password": "pw" }
                }),
                &previous_by_host
            ),
            (true, true)
        );
        assert_eq!(
            resolve_metadata_refresh_decision(
                &json!({
                    "host": "app.example.com",
                    "target": "tcp://127.0.0.1:8080",
                    "title": "",
                    "favicon": "",
                    "basic_auth": disabled_host_basic_auth()
                }),
                &previous_by_host
            ),
            (false, false)
        );
    }

    #[test]
    fn host_mapping_metadata_merge_preserves_user_changes() {
        let refreshed = HostMappingMetadataRefreshItem {
            mapping: json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Fetched",
                "favicon": "data:image/png;base64,AA==",
                "basic_auth": disabled_host_basic_auth()
            }),
            refresh_title: true,
            refresh_favicon: true,
        };

        let (changed_mappings, changed) = merge_metadata_into_current_mappings(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Current",
                "favicon": "current.ico",
                "basic_auth": disabled_host_basic_auth()
            })],
            vec![refreshed.clone()],
        );
        assert!(changed);
        assert_eq!(
            changed_mappings[0].get("title").and_then(Value::as_str),
            Some("Fetched")
        );
        assert_eq!(
            changed_mappings[0].get("favicon").and_then(Value::as_str),
            Some("data:image/png;base64,AA==")
        );

        let (stale_target_mappings, changed) = merge_metadata_into_current_mappings(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:9090",
                "title": "Current",
                "favicon": "current.ico",
                "basic_auth": disabled_host_basic_auth()
            })],
            vec![refreshed.clone()],
        );
        assert!(!changed);
        assert_eq!(
            stale_target_mappings[0]
                .get("title")
                .and_then(Value::as_str),
            Some("Current")
        );

        let (stale_auth_mappings, changed) = merge_metadata_into_current_mappings(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Current",
                "favicon": "current.ico",
                "basic_auth": { "enabled": true, "username": "admin", "password": "pw" }
            })],
            vec![refreshed],
        );
        assert!(!changed);
        assert_eq!(
            stale_auth_mappings[0]
                .get("favicon")
                .and_then(Value::as_str),
            Some("current.ico")
        );
    }

    #[test]
    fn gateway_portal_title_mode_defaults_like_node() {
        assert!(is_gateway_portal_title_mode(&json!({})));
        assert!(is_gateway_portal_title_mode(&json!({
            "gateway_portal": { "display_style": "title" }
        })));
        assert!(!is_gateway_portal_title_mode(&json!({
            "gateway_portal": { "display_style": "domain" }
        })));
    }

    #[test]
    fn builds_i18n_bookmarks_document_without_auth_mapping() {
        let config = json!({
            "run_type": 3,
            "ssl": {
                "cert": "-----BEGIN CERTIFICATE-----",
                "key": "-----BEGIN PRIVATE KEY-----"
            },
            "subdomain_mode": {
                "root_domain": "example.com",
                "public_https_port": 8443
            },
            "host_mappings": [
                {
                    "host": "app.example.com",
                    "target": "http://127.0.0.1:8080",
                    "title": "App",
                    "title_override": "Portal"
                },
                {
                    "host": "auth.example.com",
                    "target": "http://127.0.0.1:7997",
                    "title": "Auth"
                }
            ]
        });
        let document = build_bookmarks_document(&config, &crate::i18n::Translator::new("zh-CN"));

        assert!(document.contains("example.com 子域映射"));
        assert!(document.contains("https://app.example.com:8443/"));
        assert!(document.contains(">Portal</A>"));
        assert!(!document.contains("auth.example.com"));
        assert_eq!(
            build_bookmark_filename(&config),
            "fn-knock-bookmarks-example.com.html"
        );
    }

    #[test]
    fn bookmark_url_port_suffix_matches_node_string_rules() {
        assert_eq!(
            build_bookmark_url("app.example.com", "https", Some("abc"), false),
            "https://app.example.com:abc/"
        );
        assert_eq!(
            build_bookmark_url("app.example.com", "https", Some("443x"), false),
            "https://app.example.com/"
        );
        assert_eq!(
            build_bookmark_url("app.example.com", "http", Some("80x"), false),
            "http://app.example.com/"
        );
        assert_eq!(
            build_bookmark_url("app.example.com", "https", Some(""), false),
            "https://app.example.com:7999/"
        );
        assert_eq!(
            build_bookmark_url("app.example.com", "https", Some("abc"), true),
            "https://app.example.com/"
        );
    }

    #[test]
    fn auth_service_port_env_parser_matches_node_parse_int() {
        assert_eq!(parse_env_port_with_fallback_value(None, 7997), 7997);
        assert_eq!(
            parse_env_port_with_fallback_value(Some(String::new()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_with_fallback_value(Some(" 7997x ".to_string()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_with_fallback_value(Some("8000x".to_string()), 7997),
            8000
        );
        assert_eq!(
            parse_env_port_with_fallback_value(Some("0x10".to_string()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_with_fallback_value(Some("abc".to_string()), 7997),
            7997
        );
    }

    #[test]
    fn validates_stream_mapping_duplicates() {
        let error = normalize_stream_mappings(vec![
            json!({ "protocol": "tcp", "listen_port": 2222, "target": "127.0.0.1:22" }),
            json!({ "listen_port": 2222, "target": "example.com:22" }),
        ])
        .unwrap_err();
        assert!(error.contains("Duplicate stream mapping"));
        assert!(
            normalize_stream_mappings(vec![json!({
                "protocol": "udp",
                "listen_port": 5353,
                "target": "[::1]:53",
                "use_auth": false
            })])
            .is_ok()
        );
    }

    #[test]
    fn localizes_proxy_config_route_errors() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_proxy_config_error(
                &translator,
                "Host mapping app.example.com target must be a supported HTTP/WebSocket URL"
            ),
            "Host 映射 app.example.com 的目标必须以 http://、https://、ws:// 或 wss:// 开头并包含主机名"
        );
        assert_eq!(
            localize_proxy_config_error(
                &translator,
                "Host mapping app.example.com location /api target must be a supported HTTP/WebSocket URL"
            ),
            "Host 映射 app.example.com 的路径规则 /api 目标必须以 http://、https://、ws:// 或 wss:// 开头并包含主机名"
        );
        assert_eq!(
            localize_proxy_config_error(&translator, "Duplicate stream mapping for TCP port 2222"),
            "TCP 监听端口 2222 重复，请保持协议 + 端口唯一"
        );
        assert_eq!(
            localize_proxy_config_error(&translator, "Only http/https targets are supported"),
            "仅支持 http/https 目标地址"
        );
    }

    #[test]
    fn builds_gateway_auth_config_from_auth_mapping() {
        let config = json!({
            "run_type": 3,
            "reverse_proxy_submode": "host",
            "host_mappings": [{
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997"
            }],
            "subdomain_mode": {
                "auth_cache_ttl_seconds": 5,
                "auth_cache_unauthorized_ttl_seconds": 2,
                "edge_client_ip_enabled": true,
                "aliyun_esa_enabled": true,
                "tencent_edgeone_enabled": false,
                "public_auth_base_url": "",
                "public_http_port": 80,
                "public_https_port": 443
            }
        });
        let auth = build_gateway_auth_config(&config);
        assert_eq!(auth.get("auth_port").and_then(Value::as_i64), Some(7997));
        assert_eq!(
            auth.get("public_auth_base_url").and_then(Value::as_str),
            Some("https://auth.example.com")
        );
        assert_eq!(
            auth.get("auth_host").and_then(Value::as_str),
            Some("auth.example.com")
        );
        assert_eq!(
            auth.get("edge_client_ip_enabled").and_then(Value::as_bool),
            Some(true)
        );
    }
}
