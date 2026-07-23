use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::time::{self as tokio_time, MissedTickBehavior};
use url::Url;

use crate::{
    gateway_settings, i18n::Translator, response, runtime_config, ssl, state::AppState, waf,
};

mod advanced_auth;
mod auth_payload;
mod bookmarks;
mod metadata_fetch;
mod metadata_html;
mod metadata_refresh;
mod metadata_special;
mod normalize;
mod runtime;
mod subdomain;

use advanced_auth::*;
pub(crate) use auth_payload::*;
use bookmarks::*;
use metadata_fetch::*;
use metadata_html::*;
use metadata_refresh::*;
use metadata_special::*;
use normalize::*;
#[cfg(test)]
use runtime::ensure_go_host_protocol_modes_applied;
use runtime::{
    sync_go_auth_config, sync_go_rules, sync_host_mappings_runtime, sync_stream_mappings_runtime,
};
pub(crate) use runtime::{sync_go_host_rules_for_config_locked, sync_go_host_rules_locked};
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
pub(crate) const HOST_MAPPINGS_REVISION_HEADER: &str = "x-host-mappings-revision";
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
const HOST_MAPPINGS_TRANSACTION_LOCK_KEY: &str =
    "__fn_knock_internal:host-mappings-config-runtime-transaction";
const HOST_MAPPINGS_TRANSACTION_LOCK_TTL_SECONDS: usize = 120;
const HOST_MAPPINGS_TRANSACTION_LOCK_WAIT_SECONDS: u64 = 10;

pub(crate) struct HostMappingsTransactionLease {
    state: AppState,
    lock_id: String,
    valid: Arc<AtomicBool>,
    heartbeat: Option<tokio::task::JoinHandle<()>>,
    release_on_drop: bool,
}

pub(crate) async fn acquire_host_mappings_transaction_lease(
    state: &AppState,
) -> crate::storage::StorageResult<Option<HostMappingsTransactionLease>> {
    let lock_id = uuid::Uuid::new_v4().to_string();
    let deadline = tokio_time::Instant::now()
        + Duration::from_secs(HOST_MAPPINGS_TRANSACTION_LOCK_WAIT_SECONDS);
    loop {
        if state
            .store
            .set_json_value_nx_ex(
                HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
                &json!({
                    "lockId": lock_id,
                    "createdAt": crate::time_utils::now_iso(),
                }),
                HOST_MAPPINGS_TRANSACTION_LOCK_TTL_SECONDS,
            )
            .await?
        {
            let heartbeat_state = state.clone();
            let heartbeat_lock_id = lock_id.clone();
            let valid = Arc::new(AtomicBool::new(true));
            let heartbeat_valid = Arc::clone(&valid);
            let heartbeat = tokio::spawn(async move {
                let mut interval = tokio_time::interval(Duration::from_secs(
                    (HOST_MAPPINGS_TRANSACTION_LOCK_TTL_SECONDS as u64 / 3).max(1),
                ));
                interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                interval.tick().await;
                loop {
                    interval.tick().await;
                    let refreshed = heartbeat_state
                        .store
                        .set_json_lock_if_owned_ex(
                            HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
                            &heartbeat_lock_id,
                            &json!({
                                "lockId": heartbeat_lock_id,
                                "refreshedAt": crate::time_utils::now_iso(),
                            }),
                            HOST_MAPPINGS_TRANSACTION_LOCK_TTL_SECONDS,
                        )
                        .await;
                    if !matches!(refreshed, Ok(true)) {
                        heartbeat_valid.store(false, Ordering::Release);
                        break;
                    }
                }
            });
            return Ok(Some(HostMappingsTransactionLease {
                state: state.clone(),
                lock_id,
                valid,
                heartbeat: Some(heartbeat),
                release_on_drop: true,
            }));
        }
        if tokio_time::Instant::now() >= deadline {
            return Ok(None);
        }
        tokio_time::sleep(Duration::from_millis(10)).await;
    }
}

impl HostMappingsTransactionLease {
    async fn ensure_valid(&self) -> crate::storage::StorageResult<bool> {
        if !self.valid.load(Ordering::Acquire) {
            return Ok(false);
        }
        let refreshed = self
            .state
            .store
            .set_json_lock_if_owned_ex(
                HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
                &self.lock_id,
                &json!({
                    "lockId": self.lock_id,
                    "refreshedAt": crate::time_utils::now_iso(),
                }),
                HOST_MAPPINGS_TRANSACTION_LOCK_TTL_SECONDS,
            )
            .await?;
        if !refreshed {
            self.valid.store(false, Ordering::Release);
        }
        Ok(refreshed)
    }

    pub(crate) async fn ensure_owned(&self) -> crate::storage::StorageResult<()> {
        if self.ensure_valid().await? {
            return Ok(());
        }
        Err(crate::storage::storage_error(
            "host mappings transaction lease ownership was lost",
        ))
    }

    pub(crate) async fn release(mut self) -> crate::storage::StorageResult<bool> {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
        let result = self
            .state
            .store
            .delete_lock_if_owned(HOST_MAPPINGS_TRANSACTION_LOCK_KEY, &self.lock_id)
            .await;
        match result {
            Ok(true) => {
                self.release_on_drop = false;
                Ok(true)
            }
            Ok(false) => {
                self.valid.store(false, Ordering::Release);
                // The key is absent or belongs to a newer owner. Never let
                // Drop attempt to delete that owner's lease.
                self.release_on_drop = false;
                Err(crate::storage::storage_error(
                    "host mappings transaction lease ownership was lost before release",
                ))
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for HostMappingsTransactionLease {
    fn drop(&mut self) {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
        if !self.release_on_drop {
            return;
        }
        let state = self.state.clone();
        let lock_id = self.lock_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(error) = state
                    .store
                    .delete_lock_if_owned(HOST_MAPPINGS_TRANSACTION_LOCK_KEY, &lock_id)
                    .await
                {
                    tracing::warn!(%error, "failed to release host mappings transaction lease");
                }
            });
        }
    }
}

pub(crate) async fn with_host_mappings_runtime_transaction<Sync, SyncFuture>(
    state: &AppState,
    sync: Sync,
) -> Result<(), String>
where
    Sync: FnOnce(AppState) -> SyncFuture,
    SyncFuture: std::future::Future<Output = Result<(), String>>,
{
    let _update_guard = state.host_mappings_update_lock.lock().await;
    let lease = acquire_host_mappings_transaction_lease(state)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Host mappings transaction is busy".to_string())?;
    lease
        .ensure_owned()
        .await
        .map_err(|error| error.to_string())?;
    let sync_result = sync(state.clone()).await;
    let ownership_result = lease
        .ensure_owned()
        .await
        .map_err(|error| error.to_string());
    let release_result = lease.release().await.map_err(|error| error.to_string());
    match (sync_result, ownership_result, release_result) {
        (Err(error), _, _) => Err(error),
        (Ok(()), Err(error), _) => Err(error),
        (Ok(()), Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(()), Ok(true)) => Ok(()),
        (Ok(()), Ok(()), Ok(false)) => {
            Err("host mappings transaction lease ownership was lost before release".to_string())
        }
    }
}

pub(crate) async fn sync_current_go_host_rules(state: &AppState) -> Result<(), String> {
    with_host_mappings_runtime_transaction(state, |state| async move {
        let config = state
            .store
            .get_config()
            .await
            .map_err(|error| error.to_string())?;
        sync_go_host_rules_for_config_locked(&state, &config).await
    })
    .await
}

pub(crate) async fn sync_current_go_auth_config(state: &AppState) -> Result<(), String> {
    with_host_mappings_runtime_transaction(state, |state| async move {
        let config = state
            .store
            .get_config()
            .await
            .map_err(|error| error.to_string())?;
        sync_go_auth_config(&state, &config).await
    })
    .await
}

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
        "Subdomain root domain cannot contain wildcard" => {
            return admin_config_text(translator, "subdomainMode.rootDomainWildcardForbidden");
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
    if let Some(host) = message.strip_prefix("Duplicate host mapping ") {
        return admin_config_text_params(
            translator,
            "hostMappings.duplicateHost",
            &[("host", host.to_string())],
        );
    }
    if let Some(host) = extract_between(message, "Host mapping ", " cannot contain wildcard") {
        return admin_config_text_params(
            translator,
            "hostMappings.hostWildcardForbidden",
            &[("host", host.to_string())],
        );
    }
    if let Some(host) = extract_between(
        message,
        "Host mapping ",
        " HTTPS protocol mode must be auto, http1 or http2",
    ) {
        return admin_config_text_params(
            translator,
            "hostMappings.protocolModeInvalid",
            &[("host", host.to_string())],
        );
    }
    if let Some(rest) = message.strip_prefix("Go backend did not apply HTTPS protocol mode ")
        && let Some((mode, host_and_reported)) = rest.split_once(" for ")
        && let Some((host, _)) = host_and_reported.split_once(" (reported ")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.backendProtocolUnsupported",
            &[("host", host.to_string()), ("mode", mode.to_string())],
        );
    }
    if let Some(host) = extract_between(
        message,
        "Go backend did not apply host visibility for ",
        "; upgrade the gateway backend",
    ) {
        return admin_config_text_params(
            translator,
            "hostMappings.backendVisibilityUnsupported",
            &[("host", host.to_string())],
        );
    }
    if let Some(rest) = message.strip_prefix("Host mapping ")
        && let Some((host, detail)) = rest.split_once(" visibility: ")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.visibilityInvalid",
            &[("host", host.to_string()), ("message", detail.to_string())],
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
    if let Some(rest) = message.strip_prefix("Host mapping ")
        && let Some((host, _)) = rest.split_once(" custom icon")
    {
        return admin_config_text_params(
            translator,
            "hostMappings.customIconInvalid",
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
    #[serde(default)]
    revision: Option<String>,
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
            "/api/admin/config/host_mappings/{host}/advanced_auth",
            get(get_advanced_auth).put(update_advanced_auth),
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
    let translator = Translator::from_state(&state).await;
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load host mappings");
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
    host_mappings_response(mappings)
}

fn host_mappings_revision(mappings: &[Value]) -> String {
    let semantic = mappings
        .iter()
        .map(|mapping| {
            let Some(mut object) = mapping.as_object().cloned() else {
                return mapping.clone();
            };
            // These two fields are populated asynchronously from upstream
            // metadata and must not invalidate an in-progress user edit.
            object.remove("title");
            object.remove("favicon");
            Value::Object(object)
        })
        .collect::<Vec<_>>();
    crate::crypto_utils::sha256_hex_bytes(
        serde_json::to_vec(&semantic).unwrap_or_else(|_| b"[]".to_vec()),
    )
}

pub(crate) fn host_mappings_revision_from_config(config: &Value) -> String {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    host_mappings_revision(mappings)
}

fn host_mappings_response(mut mappings: Vec<Value>) -> Response {
    let revision = host_mappings_revision(&mappings);
    normalize_host_mapping_response_defaults(&mut mappings);
    let mut response = response::ok(Value::Array(mappings)).into_response();
    if let Ok(value) = HeaderValue::from_str(&revision) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(HOST_MAPPINGS_REVISION_HEADER),
            value,
        );
    }
    response
}

fn normalize_host_mapping_response_defaults(mappings: &mut [Value]) {
    for mapping in mappings {
        let Some(object) = mapping.as_object_mut() else {
            continue;
        };
        let is_auth = object.get("service_role").and_then(Value::as_str) == Some("auth")
            || object
                .get("target")
                .and_then(Value::as_str)
                .is_some_and(is_auth_service_target);
        let waf_enabled = is_auth
            || object
                .get("waf_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true);
        let visibility = normalize_host_mapping_visibility(None, object.get("visibility"), is_auth)
            .unwrap_or_else(|_| {
                json!({
                    "mode": "inherit",
                    "selections": [],
                    "custom_cidrs": [],
                    "cidrs": [],
                })
            });
        object.insert("waf_enabled".to_string(), Value::Bool(waf_enabled));
        object.insert("visibility".to_string(), visibility);
        if !object.contains_key("favicon_override") {
            object.insert("favicon_override".to_string(), Value::String(String::new()));
        }
    }
}

pub(crate) fn is_auth_host_mapping_target(target: &str) -> bool {
    is_auth_service_target(target)
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
    let _update_guard = state.host_mappings_update_lock.lock().await;
    let transaction_lease = match acquire_host_mappings_transaction_lease(&state).await {
        Ok(Some(lease)) => lease,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to acquire host mappings transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    };
    let previous_config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read host mappings before metadata refresh");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (next_mappings, summary) = refresh_host_mapping_metadata(mappings.clone()).await;
    match transaction_lease.ensure_valid().await {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to refresh host mappings transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    }
    match state
        .store
        .compare_and_set_host_mappings(&mappings, &next_mappings)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save host mappings after metadata refresh");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    }
    if let Err(error) = transaction_lease.ensure_owned().await {
        tracing::warn!(%error, "host mappings transaction lease was lost before runtime sync");
        let _ = state
            .store
            .compare_and_set_host_mappings(&next_mappings, &mappings)
            .await;
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }
    if let Err(message) = sync_host_mappings_runtime(&state, &previous_config, &next_mappings).await
    {
        rollback_host_mappings(&state, &previous_config, &next_mappings).await;
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
    if let Err(error) = transaction_lease.release().await {
        tracing::warn!(%error, "failed to release host mappings transaction lease");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "hostMappings.updateFailed"),
        );
    }
    response::ok(summary).into_response()
}

async fn export_host_mapping_bookmarks(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.store.get_config().await {
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
    match state.store.get_config().await {
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

    let previous_config = match state.store.get_config().await {
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

    if let Err(error) = state.store.save_config(&updated_config).await {
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
    // Keep persistence, runtime sync and any rollback in one transaction. The
    // mutex covers this AppState; the leased storage lock covers other states
    // and processes that share the same config database.
    let _update_guard = state.host_mappings_update_lock.lock().await;
    let transaction_lease = match acquire_host_mappings_transaction_lease(&state).await {
        Ok(Some(lease)) => lease,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to acquire host mappings transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    };
    let previous_config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before host mappings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                load_config_failed(&translator),
            );
        }
    };
    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(revision) = body.revision.as_deref().map(str::trim)
        && revision != host_mappings_revision(&previous_mappings)
    {
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }

    let normalized = match normalize_host_mappings_for_route(body.mappings, &previous_config) {
        Ok(value) => value,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_proxy_config_error(&translator, &message),
            );
        }
    };
    let normalized =
        match compile_host_mapping_visibilities(&state, normalized, &previous_config).await {
            Ok(value) => value,
            Err(message) => {
                return response::error(
                    StatusCode::BAD_REQUEST,
                    localize_proxy_config_error(&translator, &message),
                );
            }
        };

    let mut candidate_config = previous_config.clone();
    ensure_object(&mut candidate_config).insert(
        "host_mappings".to_string(),
        Value::Array(normalized.clone()),
    );
    if let Err(message) = validate_passkey_rp_config(&candidate_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, &message),
        );
    }
    match transaction_lease.ensure_valid().await {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to refresh host mappings transaction lease");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    }

    let updated_config = match state
        .store
        .compare_and_set_host_mappings(&previous_mappings, &normalized)
        .await
    {
        Ok(Some(config)) => config,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_config_text(&translator, "hostMappings.revisionConflict"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save host mappings");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_config_text(&translator, "hostMappings.updateFailed"),
            );
        }
    };
    if let Err(message) = validate_passkey_rp_config(&updated_config) {
        tracing::warn!(
            %message,
            "concurrent config update made the persisted host mappings invalid; rolling back"
        );
        match state
            .store
            .compare_and_set_host_mappings(&normalized, &previous_mappings)
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                let current_is_valid = state
                    .store
                    .get_config()
                    .await
                    .is_ok_and(|config| validate_passkey_rp_config(&config).is_ok());
                if current_is_valid {
                    return response::error(
                        StatusCode::CONFLICT,
                        admin_config_text(&translator, "hostMappings.revisionConflict"),
                    );
                }
                tracing::warn!(
                    "host mappings changed while rolling back an invalid config combination"
                );
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_config_text(&translator, "hostMappings.updateFailed"),
                );
            }
            Err(error) => {
                tracing::warn!(%error, "failed to rollback invalid host mappings combination");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_config_text(&translator, "hostMappings.updateFailed"),
                );
            }
        }
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }

    if let Err(error) = transaction_lease.ensure_owned().await {
        tracing::warn!(%error, "host mappings transaction lease was lost before runtime sync");
        let _ = state
            .store
            .compare_and_set_host_mappings(&normalized, &previous_mappings)
            .await;
        return response::error(
            StatusCode::CONFLICT,
            admin_config_text(&translator, "hostMappings.revisionConflict"),
        );
    }

    if let Err(message) = sync_host_mappings_runtime(&state, &previous_config, &normalized).await {
        rollback_host_mappings(&state, &previous_config, &normalized).await;
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

    if let Err(error) = transaction_lease.release().await {
        tracing::warn!(%error, "failed to release host mappings transaction lease");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "hostMappings.updateFailed"),
        );
    }

    schedule_host_mappings_metadata_refresh(state.clone(), normalized.clone(), previous_mappings);
    runtime_config::schedule_smart_connect_sync_after_host_mappings_change(
        state.clone(),
        updated_config.clone(),
    );

    host_mappings_response(normalized)
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

    let previous_config = match state.store.get_config().await {
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

    if let Err(error) = state.store.save_config(&updated_config).await {
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

    let previous_config = match state.store.get_config().await {
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
    if let Err(message) = validate_subdomain_root_domain(&next) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_proxy_config_error(&translator, message),
        );
    }

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

    if let Err(error) = state.store.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save subdomain mode config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_config_text(&translator, "subdomainMode.saveFailed"),
        );
    }

    if let Err(message) = sync_current_go_auth_config(&state).await {
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
