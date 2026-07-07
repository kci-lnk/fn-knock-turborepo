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

mod auth_payload;
mod bookmarks;
mod metadata_fetch;
mod metadata_html;
mod metadata_refresh;
mod metadata_special;
mod normalize;
mod runtime;
mod subdomain;

pub(crate) use auth_payload::*;
use bookmarks::*;
use metadata_fetch::*;
use metadata_html::*;
use metadata_refresh::*;
use metadata_special::*;
use normalize::*;
use runtime::*;
use subdomain::*;

#[cfg(test)]
mod tests;

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
