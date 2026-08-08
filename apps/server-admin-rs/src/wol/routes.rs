use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use fn_knock_wol_protocol::{AckStatus, Command, MacAddress};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use uuid::Uuid;

use crate::{crypto_utils, response, state::AppState, time_utils};

use super::{
    discovery::{
        DiscoveryJobError, cancel_discovery_job, get_discovery_job, local_broadcast_addresses,
        start_discovery_job,
    },
    dispatch::{DispatchError, dispatch},
    secrets::{local_relay_secret_id, secret_store},
    store::{
        LocalRelayConfig, RelayRecord, TargetRecord, delete_relay as delete_relay_record,
        delete_target as delete_target_record, list_relays, list_targets, load_local_relay_config,
        load_relay, load_target, save_local_relay_config, save_relay, save_target,
    },
};

const DEFAULT_RELAY_PORT: u16 = 40009;
const MAX_NAME_LENGTH: usize = 64;
const MAX_NOTE_LENGTH: usize = 256;
const MAX_BROADCAST_DESTINATIONS: usize = 16;
const MAX_ALLOWED_SOURCES: usize = 32;
const PAIRING_CODE_PREFIX: &str = "FNW1.";
const MAX_PAIRING_CODE_LENGTH: usize = 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayBody {
    name: String,
    address: String,
    #[serde(default = "default_relay_port")]
    port: u16,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetBody {
    name: String,
    mac: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    relay_id: Option<String>,
    #[serde(default)]
    broadcast_address: Option<String>,
    #[serde(default)]
    ip_address: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

struct ValidatedTargetBody {
    name: String,
    mac: String,
    note: String,
    relay_id: Option<String>,
    broadcast_address: Option<String>,
    ip_address: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryJobBody {
    #[serde(default)]
    target_cidrs: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoveryJobQuery {
    cursor: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalRelayBody {
    enabled: bool,
    relay_id: String,
    key_version: u32,
    listen_address: String,
    port: u16,
    broadcast_destinations: Vec<String>,
    #[serde(default)]
    allowed_sources: Vec<String>,
    #[serde(default)]
    psk: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalRelayPairBody {
    pairing_code: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingCodePayload {
    version: u8,
    relay_id: String,
    key_version: u32,
    psk: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRelayView {
    enabled: bool,
    relay_id: String,
    key_version: u32,
    listen_address: String,
    port: u16,
    broadcast_destinations: Vec<String>,
    allowed_sources: Vec<String>,
    psk_configured: bool,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayView {
    id: String,
    name: String,
    address: String,
    port: u16,
    enabled: bool,
    key_version: u32,
    psk_configured: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelaySummary {
    id: String,
    name: String,
    address: String,
    port: u16,
    enabled: bool,
    psk_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetView {
    id: String,
    name: String,
    mac: String,
    note: String,
    relay_id: Option<String>,
    broadcast_address: Option<String>,
    ip_address: Option<String>,
    delivery_mode: &'static str,
    enabled: bool,
    created_at: String,
    updated_at: String,
    relay: Option<RelaySummary>,
    status: super::status::TargetStatusView,
}

#[derive(Debug)]
struct WolHttpError {
    status: StatusCode,
    message: String,
}

impl WolHttpError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn not_found(entity: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, format!("{entity} was not found"))
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn into_response(self) -> Response {
        response::error(self.status, self.message)
    }
}

pub fn wol_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/wol/local-relay",
            get(get_local_relay).put(update_local_relay),
        )
        .route("/api/admin/wol/local-relay/pair", post(pair_local_relay))
        .route("/api/admin/wol/relays", get(get_relays).post(create_relay))
        .route(
            "/api/admin/wol/relays/{id}",
            get(get_relay).put(update_relay).delete(delete_relay),
        )
        .route(
            "/api/admin/wol/relays/{id}/rotate-psk",
            post(rotate_relay_psk),
        )
        .route("/api/admin/wol/relays/{id}/probe", post(probe_relay))
        .route(
            "/api/admin/wol/targets",
            get(get_targets).post(create_target),
        )
        .route("/api/admin/wol/discover/jobs", post(start_discovery))
        .route(
            "/api/admin/wol/discover/jobs/{id}",
            get(get_discovery).delete(cancel_discovery),
        )
        .route(
            "/api/admin/wol/targets/{id}",
            get(get_target).put(update_target).delete(delete_target),
        )
        .route("/api/admin/wol/targets/{id}/wake", post(wake_target))
        .route_layer(middleware::from_fn_with_state(state, require_wol_feature))
}

async fn require_wol_feature(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    match super::feature_enabled_for_state(&state).await {
        Ok(true) => next.run(req).await,
        Ok(false) => response::error(StatusCode::FORBIDDEN, "Wake-on-LAN is disabled"),
        Err(error) => {
            tracing::warn!(%error, "failed to load WoL feature config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load Wake-on-LAN feature config",
            )
        }
    }
}

async fn get_local_relay(State(state): State<AppState>) -> Response {
    match local_relay_response(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn update_local_relay(
    State(state): State<AppState>,
    body: Result<Json<LocalRelayBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => {
            return WolHttpError::bad_request("Local Relay request is invalid").into_response();
        }
    };
    match update_local_relay_inner(&state, body).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn pair_local_relay(
    State(state): State<AppState>,
    body: Result<Json<LocalRelayPairBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Pairing request is invalid").into_response(),
    };
    match pair_local_relay_inner(&state, &body.pairing_code).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn pair_local_relay_inner(
    state: &AppState,
    pairing_code: &str,
) -> Result<Value, WolHttpError> {
    let payload = decode_pairing_code(pairing_code)?;
    let broadcast_destinations = local_broadcast_addresses()
        .into_iter()
        .map(|address| format!("{address}:9"))
        .collect();
    update_local_relay_inner(
        state,
        LocalRelayBody {
            enabled: true,
            relay_id: payload.relay_id,
            key_version: payload.key_version,
            listen_address: "0.0.0.0".to_string(),
            port: DEFAULT_RELAY_PORT,
            broadcast_destinations,
            allowed_sources: Vec::new(),
            psk: Some(payload.psk),
        },
    )
    .await
}

async fn update_local_relay_inner(
    state: &AppState,
    body: LocalRelayBody,
) -> Result<Value, WolHttpError> {
    let (config, psk) = validate_local_relay_body(body)?;
    let _guard = state.wol_config_lock.lock().await;
    let previous_config = load_local_relay_config(state)
        .await
        .map_err(|error| internal_error("load local Relay configuration", error))?;
    let secret_id = local_relay_secret_id(&config.relay_id);
    let secrets = secret_store(state);
    let previous_secret = if previous_config.relay_id.is_empty() {
        None
    } else {
        secrets
            .read(
                &local_relay_secret_id(&previous_config.relay_id),
                previous_config.key_version,
            )
            .ok()
            .flatten()
    };
    if let Some(psk) = &psk {
        secrets
            .write(&secret_id, config.key_version, psk)
            .map_err(WolHttpError::internal)?;
    }
    let configured = secrets
        .read(&secret_id, config.key_version)
        .map_err(WolHttpError::internal)?
        .is_some();
    if config.enabled && !configured {
        return Err(WolHttpError::conflict(
            "Local Relay PSK must be supplied before enabling the listener",
        ));
    }
    if let Err(error) = save_local_relay_config(state, &config).await {
        if psk.is_some() {
            let _ = secrets.delete(&secret_id);
            if let Some(previous_secret) = previous_secret {
                let _ = secrets.write(
                    &local_relay_secret_id(&previous_config.relay_id),
                    previous_config.key_version,
                    &previous_secret,
                );
            }
        }
        return Err(internal_error("save local Relay configuration", error));
    }
    if !previous_config.relay_id.is_empty() && previous_config.relay_id != config.relay_id {
        let previous_secret_id = local_relay_secret_id(&previous_config.relay_id);
        if let Err(error) = secrets.delete(&previous_secret_id) {
            tracing::warn!(%error, "failed to remove superseded WoL pairing credential");
        }
    }
    state.wol_relay_reload.notify_one();
    local_relay_response(state).await
}

async fn local_relay_response(state: &AppState) -> Result<Value, WolHttpError> {
    let config = load_local_relay_config(state)
        .await
        .map_err(|error| internal_error("load local Relay configuration", error))?;
    let psk_configured = if config.relay_id.is_empty() {
        false
    } else {
        secret_store(state)
            .read(&local_relay_secret_id(&config.relay_id), config.key_version)
            .ok()
            .flatten()
            .is_some()
    };
    let view = LocalRelayView {
        enabled: config.enabled,
        relay_id: config.relay_id,
        key_version: config.key_version,
        listen_address: config.listen_address,
        port: config.port,
        broadcast_destinations: config.broadcast_destinations,
        allowed_sources: config.allowed_sources,
        psk_configured,
        updated_at: config.updated_at,
    };
    let runtime = state.wol_relay_status.read().await.clone();
    Ok(json!({ "config": view, "runtime": runtime }))
}

async fn get_relays(State(state): State<AppState>) -> Response {
    match relay_views(&state).await {
        Ok(items) => response::ok(json!({ "total": items.len(), "items": items })).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn get_relay(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match load_relay(&state, &id).await {
        Ok(Some(relay)) => response::ok(relay_view(&state, relay)).into_response(),
        Ok(None) => WolHttpError::not_found("Relay").into_response(),
        Err(error) => internal_error("load Relay", error).into_response(),
    }
}

async fn create_relay(
    State(state): State<AppState>,
    body: Result<Json<RelayBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Relay request is invalid").into_response(),
    };
    match create_relay_inner(&state, body).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn create_relay_inner(state: &AppState, body: RelayBody) -> Result<Value, WolHttpError> {
    let (name, address, port) = validate_relay_body(&body)?;
    let _guard = state.wol_config_lock.lock().await;
    let now = time_utils::now_iso();
    let relay = RelayRecord {
        id: Uuid::new_v4().to_string(),
        name,
        address,
        port,
        enabled: body.enabled,
        key_version: 1,
        created_at: now.clone(),
        updated_at: now,
    };
    let psk = crypto_utils::random_bytes::<32>();
    let secrets = secret_store(state);
    secrets
        .write(&relay.id, relay.key_version, &psk)
        .map_err(WolHttpError::internal)?;
    if let Err(error) = save_relay(state, &relay).await {
        let _ = secrets.delete(&relay.id);
        return Err(internal_error("save Relay", error));
    }
    Ok(json!({
        "relay": relay_view(state, relay.clone()),
        "bootstrap": {
            "pairingCode": encode_pairing_code(&relay, &psk)?,
        }
    }))
}

async fn update_relay(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Result<Json<RelayBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Relay request is invalid").into_response(),
    };
    match update_relay_inner(&state, &id, body).await {
        Ok(relay) => response::ok(relay).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn update_relay_inner(
    state: &AppState,
    id: &str,
    body: RelayBody,
) -> Result<RelayView, WolHttpError> {
    let (name, address, port) = validate_relay_body(&body)?;
    let _guard = state.wol_config_lock.lock().await;
    let mut relay = load_relay(state, id)
        .await
        .map_err(|error| internal_error("load Relay", error))?
        .ok_or_else(|| WolHttpError::not_found("Relay"))?;
    relay.name = name;
    relay.address = address;
    relay.port = port;
    relay.enabled = body.enabled;
    relay.updated_at = time_utils::now_iso();
    save_relay(state, &relay)
        .await
        .map_err(|error| internal_error("save Relay", error))?;
    Ok(relay_view(state, relay))
}

async fn delete_relay(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match delete_relay_inner(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => error.into_response(),
    }
}

async fn delete_relay_inner(state: &AppState, id: &str) -> Result<(), WolHttpError> {
    let _guard = state.wol_config_lock.lock().await;
    let relay = load_relay(state, id)
        .await
        .map_err(|error| internal_error("load Relay", error))?
        .ok_or_else(|| WolHttpError::not_found("Relay"))?;
    let targets = list_targets(state)
        .await
        .map_err(|error| internal_error("load Targets", error))?;
    if targets
        .iter()
        .any(|target| target.relay_id.as_deref() == Some(id))
    {
        return Err(WolHttpError::conflict(
            "Relay is still referenced by one or more Targets",
        ));
    }
    let secrets = secret_store(state);
    let previous = secrets
        .read(id, relay.key_version)
        .map_err(WolHttpError::internal)?;
    secrets.delete(id).map_err(WolHttpError::internal)?;
    if let Err(error) = delete_relay_record(state, id).await {
        if let Some(previous) = previous {
            let _ = secrets.write(id, relay.key_version, &previous);
        }
        return Err(internal_error("delete Relay", error));
    }
    Ok(())
}

async fn rotate_relay_psk(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match rotate_relay_psk_inner(&state, &id).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn rotate_relay_psk_inner(state: &AppState, id: &str) -> Result<Value, WolHttpError> {
    let _guard = state.wol_config_lock.lock().await;
    let mut relay = load_relay(state, id)
        .await
        .map_err(|error| internal_error("load Relay", error))?
        .ok_or_else(|| WolHttpError::not_found("Relay"))?;
    let secrets = secret_store(state);
    let previous = secrets
        .read(id, relay.key_version)
        .map_err(WolHttpError::internal)?;
    let previous_version = relay.key_version;
    relay.key_version = relay.key_version.saturating_add(1).max(1);
    relay.updated_at = time_utils::now_iso();
    let psk = crypto_utils::random_bytes::<32>();
    secrets
        .write(id, relay.key_version, &psk)
        .map_err(WolHttpError::internal)?;
    if let Err(error) = save_relay(state, &relay).await {
        if let Some(previous) = previous {
            let _ = secrets.write(id, previous_version, &previous);
        } else {
            let _ = secrets.delete(id);
        }
        return Err(internal_error("save rotated Relay key", error));
    }
    Ok(json!({
        "relay": relay_view(state, relay.clone()),
        "bootstrap": {
            "pairingCode": encode_pairing_code(&relay, &psk)?,
        }
    }))
}

async fn probe_relay(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match dispatch_for_relay(&state, &id, Command::Probe, None).await {
        Ok(result) => response::ok(result).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn start_discovery(body: Result<Json<DiscoveryJobBody>, JsonRejection>) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Discovery request is invalid").into_response(),
    };
    match start_discovery_job(body.target_cidrs).await {
        Ok(job) => response::ok(job).into_response(),
        Err(error) => discovery_job_error(error).into_response(),
    }
}

async fn get_discovery(Path(id): Path<String>, Query(query): Query<DiscoveryJobQuery>) -> Response {
    match get_discovery_job(&id, query.cursor.unwrap_or_default()) {
        Some(job) => response::ok(job).into_response(),
        None => WolHttpError::not_found("Discovery job").into_response(),
    }
}

async fn cancel_discovery(Path(id): Path<String>) -> Response {
    match cancel_discovery_job(&id) {
        Some(job) => response::ok(job).into_response(),
        None => WolHttpError::not_found("Discovery job").into_response(),
    }
}

async fn get_targets(State(state): State<AppState>) -> Response {
    match target_views(&state).await {
        Ok(items) => response::ok(json!({ "total": items.len(), "items": items })).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn get_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match load_target(&state, &id).await {
        Ok(Some(target)) => match target_view(&state, target).await {
            Ok(view) => response::ok(view).into_response(),
            Err(error) => error.into_response(),
        },
        Ok(None) => WolHttpError::not_found("Target").into_response(),
        Err(error) => internal_error("load Target", error).into_response(),
    }
}

async fn create_target(
    State(state): State<AppState>,
    body: Result<Json<TargetBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Target request is invalid").into_response(),
    };
    match create_target_inner(&state, body).await {
        Ok(target) => response::ok(target).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn create_target_inner(
    state: &AppState,
    body: TargetBody,
) -> Result<TargetView, WolHttpError> {
    let ValidatedTargetBody {
        name,
        mac,
        note,
        relay_id,
        broadcast_address,
        ip_address,
    } = validate_target_body(&body)?;
    let _guard = state.wol_config_lock.lock().await;
    if let Some(relay_id) = relay_id.as_deref() {
        require_relay(state, relay_id).await?;
    }
    ensure_unique_mac(state, relay_id.as_deref(), &mac, None).await?;
    let now = time_utils::now_iso();
    let target = TargetRecord {
        id: Uuid::new_v4().to_string(),
        name,
        mac,
        note,
        relay_id,
        broadcast_address,
        ip_address,
        enabled: body.enabled,
        created_at: now.clone(),
        updated_at: now,
    };
    save_target(state, &target)
        .await
        .map_err(|error| internal_error("save Target", error))?;
    target_view(state, target).await
}

async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Result<Json<TargetBody>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => return WolHttpError::bad_request("Target request is invalid").into_response(),
    };
    match update_target_inner(&state, &id, body).await {
        Ok(target) => response::ok(target).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn update_target_inner(
    state: &AppState,
    id: &str,
    body: TargetBody,
) -> Result<TargetView, WolHttpError> {
    let ValidatedTargetBody {
        name,
        mac,
        note,
        relay_id,
        broadcast_address,
        ip_address,
    } = validate_target_body(&body)?;
    let _guard = state.wol_config_lock.lock().await;
    if let Some(relay_id) = relay_id.as_deref() {
        require_relay(state, relay_id).await?;
    }
    ensure_unique_mac(state, relay_id.as_deref(), &mac, Some(id)).await?;
    let mut target = load_target(state, id)
        .await
        .map_err(|error| internal_error("load Target", error))?
        .ok_or_else(|| WolHttpError::not_found("Target"))?;
    let reset_status = target.mac != mac
        || target.ip_address != ip_address
        || target.relay_id != relay_id
        || target.enabled != body.enabled;
    target.name = name;
    target.mac = mac;
    target.note = note;
    target.relay_id = relay_id;
    target.broadcast_address = broadcast_address;
    target.ip_address = ip_address;
    target.enabled = body.enabled;
    target.updated_at = time_utils::now_iso();
    if reset_status {
        // Clear first while holding the CRUD/probe coordination lock. If the
        // subsequent target save fails, losing a cached observation is safe;
        // saving the edit and then failing to clear could expose stale online
        // state for the new identity.
        super::store::delete_target_status(state, id)
            .await
            .map_err(|error| internal_error("reset Target status", error))?;
    }
    save_target(state, &target)
        .await
        .map_err(|error| internal_error("save Target", error))?;
    target_view(state, target).await
}

async fn delete_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let _guard = state.wol_config_lock.lock().await;
    match load_target(&state, &id).await {
        Ok(Some(_)) => {
            if let Err(error) = super::store::delete_target_status(&state, &id).await {
                return internal_error("delete Target status", error).into_response();
            }
            match delete_target_record(&state, &id).await {
                Ok(()) => response::success_empty().into_response(),
                Err(error) => internal_error("delete Target", error).into_response(),
            }
        }
        Ok(None) => WolHttpError::not_found("Target").into_response(),
        Err(error) => internal_error("load Target", error).into_response(),
    }
}

async fn wake_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match wake_target_inner(&state, &id).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn wake_target_inner(state: &AppState, id: &str) -> Result<Value, WolHttpError> {
    super::service::wake_target(state, id)
        .await
        .map_err(|error| WolHttpError::new(error.status, error.message))
}

async fn dispatch_for_relay(
    state: &AppState,
    id: &str,
    command: Command,
    mac: Option<MacAddress>,
) -> Result<super::dispatch::DispatchResult, WolHttpError> {
    let relay = require_relay(state, id).await?;
    if !relay.enabled {
        return Err(WolHttpError::conflict("Relay is disabled"));
    }
    let psk = secret_store(state)
        .read(&relay.id, relay.key_version)
        .map_err(WolHttpError::internal)?
        .ok_or_else(|| WolHttpError::conflict("Relay PSK is not configured"))?;
    dispatch(&relay, &psk, command, mac)
        .await
        .map_err(dispatch_error)
}

async fn relay_views(state: &AppState) -> Result<Vec<RelayView>, WolHttpError> {
    list_relays(state)
        .await
        .map_err(|error| internal_error("load Relays", error))
        .map(|records| {
            records
                .into_iter()
                .map(|record| relay_view(state, record))
                .collect()
        })
}

async fn target_views(state: &AppState) -> Result<Vec<TargetView>, WolHttpError> {
    let targets = list_targets(state)
        .await
        .map_err(|error| internal_error("load Targets", error))?;
    let relays = list_relays(state)
        .await
        .map_err(|error| internal_error("load Relays", error))?;
    let secrets = secret_store(state);
    let mut views = Vec::with_capacity(targets.len());
    for target in targets {
        let relay = relays
            .iter()
            .find(|relay| target.relay_id.as_deref() == Some(relay.id.as_str()))
            .map(|relay| relay_summary(relay, secrets.configured(&relay.id)));
        let status = super::status::status_view(state, &target.id)
            .await
            .map_err(|error| internal_error("load Target status", error))?;
        views.push(target_view_with_relay(target, relay, status));
    }
    Ok(views)
}

async fn target_view(state: &AppState, target: TargetRecord) -> Result<TargetView, WolHttpError> {
    let relay = match target.relay_id.as_deref() {
        Some(relay_id) => load_relay(state, relay_id)
            .await
            .map_err(|error| internal_error("load Relay", error))?
            .map(|relay| {
                let configured = secret_store(state).configured(&relay.id);
                relay_summary(&relay, configured)
            }),
        None => None,
    };
    let status = super::status::status_view(state, &target.id)
        .await
        .map_err(|error| internal_error("load Target status", error))?;
    Ok(target_view_with_relay(target, relay, status))
}

fn relay_view(state: &AppState, relay: RelayRecord) -> RelayView {
    RelayView {
        psk_configured: secret_store(state).configured(&relay.id),
        id: relay.id,
        name: relay.name,
        address: relay.address,
        port: relay.port,
        enabled: relay.enabled,
        key_version: relay.key_version,
        created_at: relay.created_at,
        updated_at: relay.updated_at,
    }
}

fn relay_summary(relay: &RelayRecord, psk_configured: bool) -> RelaySummary {
    RelaySummary {
        id: relay.id.clone(),
        name: relay.name.clone(),
        address: relay.address.clone(),
        port: relay.port,
        enabled: relay.enabled,
        psk_configured,
    }
}

fn target_view_with_relay(
    target: TargetRecord,
    relay: Option<RelaySummary>,
    status: super::status::TargetStatusView,
) -> TargetView {
    let delivery_mode = if target.relay_id.is_some() {
        "relay"
    } else {
        "local"
    };
    TargetView {
        id: target.id,
        name: target.name,
        mac: target.mac,
        note: target.note,
        relay_id: target.relay_id,
        broadcast_address: target.broadcast_address,
        ip_address: target.ip_address,
        delivery_mode,
        enabled: target.enabled,
        created_at: target.created_at,
        updated_at: target.updated_at,
        relay,
        status,
    }
}

async fn require_relay(state: &AppState, id: &str) -> Result<RelayRecord, WolHttpError> {
    load_relay(state, id)
        .await
        .map_err(|error| internal_error("load Relay", error))?
        .ok_or_else(|| WolHttpError::not_found("Relay"))
}

async fn ensure_unique_mac(
    state: &AppState,
    relay_id: Option<&str>,
    mac: &str,
    current_id: Option<&str>,
) -> Result<(), WolHttpError> {
    let duplicate = list_targets(state)
        .await
        .map_err(|error| internal_error("load Targets", error))?
        .into_iter()
        .any(|target| {
            target.relay_id.as_deref() == relay_id
                && target.mac == mac
                && current_id != Some(target.id.as_str())
        });
    if duplicate {
        Err(WolHttpError::conflict(
            "A Target with this MAC already exists for this delivery path",
        ))
    } else {
        Ok(())
    }
}

fn validate_local_relay_body(
    body: LocalRelayBody,
) -> Result<(LocalRelayConfig, Option<Vec<u8>>), WolHttpError> {
    let relay_id = Uuid::parse_str(body.relay_id.trim())
        .map_err(|_| WolHttpError::bad_request("Local Relay ID must be a UUID"))?;
    if relay_id.is_nil() {
        return Err(WolHttpError::bad_request(
            "Local Relay ID must not be the nil UUID",
        ));
    }
    if body.key_version == 0 {
        return Err(WolHttpError::bad_request(
            "Local Relay key version must be greater than zero",
        ));
    }
    let listen_address = body
        .listen_address
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| WolHttpError::bad_request("Local Relay listen address is invalid"))?;
    if listen_address.is_multicast() {
        return Err(WolHttpError::bad_request(
            "Local Relay listen address must not be multicast",
        ));
    }
    if body.port == 0 {
        return Err(WolHttpError::bad_request(
            "Local Relay port must be between 1 and 65535",
        ));
    }
    if body.broadcast_destinations.is_empty()
        || body.broadcast_destinations.len() > MAX_BROADCAST_DESTINATIONS
    {
        return Err(WolHttpError::bad_request(format!(
            "Local Relay requires between 1 and {MAX_BROADCAST_DESTINATIONS} broadcast destinations"
        )));
    }
    let mut broadcast_destinations = Vec::with_capacity(body.broadcast_destinations.len());
    for value in body.broadcast_destinations {
        let endpoint = value.trim().parse::<SocketAddr>().map_err(|_| {
            WolHttpError::bad_request(format!("Broadcast destination is invalid: {value}"))
        })?;
        if !endpoint.is_ipv4() || endpoint.port() == 0 {
            return Err(WolHttpError::bad_request(format!(
                "Broadcast destination must be IPv4 with a port: {value}"
            )));
        }
        let normalized = endpoint.to_string();
        if !broadcast_destinations.contains(&normalized) {
            broadcast_destinations.push(normalized);
        }
    }
    if body.allowed_sources.len() > MAX_ALLOWED_SOURCES {
        return Err(WolHttpError::bad_request(format!(
            "Local Relay accepts at most {MAX_ALLOWED_SOURCES} source CIDRs"
        )));
    }
    let mut allowed_sources = Vec::with_capacity(body.allowed_sources.len());
    for value in body.allowed_sources {
        let network = value.trim().parse::<IpNet>().map_err(|_| {
            WolHttpError::bad_request(format!("Allowed source CIDR is invalid: {value}"))
        })?;
        let normalized = network.trunc().to_string();
        if !allowed_sources.contains(&normalized) {
            allowed_sources.push(normalized);
        }
    }
    let psk = match body
        .psk
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let decoded = URL_SAFE_NO_PAD
                .decode(value)
                .map_err(|_| WolHttpError::bad_request("Local Relay PSK is invalid"))?;
            if decoded.len() != 32 {
                return Err(WolHttpError::bad_request(
                    "Local Relay PSK must contain exactly 32 bytes",
                ));
            }
            Some(decoded)
        }
        None => None,
    };
    Ok((
        LocalRelayConfig {
            enabled: body.enabled,
            relay_id: relay_id.to_string(),
            key_version: body.key_version,
            listen_address: listen_address.to_string(),
            port: body.port,
            broadcast_destinations,
            allowed_sources,
            updated_at: time_utils::now_iso(),
        },
        psk,
    ))
}

fn encode_pairing_code(relay: &RelayRecord, psk: &[u8]) -> Result<String, WolHttpError> {
    let payload = PairingCodePayload {
        version: 1,
        relay_id: relay.id.clone(),
        key_version: relay.key_version,
        psk: URL_SAFE_NO_PAD.encode(psk),
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| internal_error("encode Relay pairing code", error))?;
    Ok(format!(
        "{PAIRING_CODE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn decode_pairing_code(value: &str) -> Result<PairingCodePayload, WolHttpError> {
    let value = value.trim();
    if value.len() > MAX_PAIRING_CODE_LENGTH {
        return Err(WolHttpError::bad_request("Pairing code is too long"));
    }
    let encoded = value
        .strip_prefix(PAIRING_CODE_PREFIX)
        .ok_or_else(|| WolHttpError::bad_request("Pairing code is invalid"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| WolHttpError::bad_request("Pairing code is invalid"))?;
    let payload = serde_json::from_slice::<PairingCodePayload>(&decoded)
        .map_err(|_| WolHttpError::bad_request("Pairing code is invalid"))?;
    if payload.version != 1 {
        return Err(WolHttpError::bad_request(
            "Pairing code version is not supported",
        ));
    }
    Ok(payload)
}

fn validate_relay_body(body: &RelayBody) -> Result<(String, String, u16), WolHttpError> {
    let name = validate_name(&body.name, "Relay")?;
    let address =
        body.address.trim().parse::<IpAddr>().map_err(|_| {
            WolHttpError::bad_request("Relay address must be an IPv4 or IPv6 literal")
        })?;
    let unusable = match address {
        IpAddr::V4(value) => value.is_unspecified() || value.is_multicast() || value.is_broadcast(),
        IpAddr::V6(value) => {
            value.is_unspecified() || value.is_multicast() || value.is_unicast_link_local()
        }
    };
    if unusable {
        return Err(WolHttpError::bad_request("Relay address must be unicast"));
    }
    if body.port == 0 {
        return Err(WolHttpError::bad_request(
            "Relay port must be between 1 and 65535",
        ));
    }
    Ok((name, address.to_string(), body.port))
}

fn validate_target_body(body: &TargetBody) -> Result<ValidatedTargetBody, WolHttpError> {
    let name = validate_name(&body.name, "Target")?;
    let mac = body
        .mac
        .parse::<MacAddress>()
        .map_err(|_| WolHttpError::bad_request("Target MAC address is invalid"))?
        .to_string();
    let note = body.note.trim();
    if note.chars().count() > MAX_NOTE_LENGTH {
        return Err(WolHttpError::bad_request(format!(
            "Target note must not exceed {MAX_NOTE_LENGTH} characters"
        )));
    }
    let relay_id = body
        .relay_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let relay_id = Uuid::parse_str(value)
                .map_err(|_| WolHttpError::bad_request("Relay ID must be a UUID"))?;
            if relay_id.is_nil() {
                return Err(WolHttpError::bad_request("Relay ID must not be nil"));
            }
            Ok(relay_id.to_string())
        })
        .transpose()?;
    let broadcast_address = if relay_id.is_none() {
        body.broadcast_address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                let address = value.parse::<Ipv4Addr>().map_err(|_| {
                    WolHttpError::bad_request("Broadcast address must be an IPv4 literal")
                })?;
                if address.is_unspecified() || address.is_multicast() {
                    return Err(WolHttpError::bad_request(
                        "Broadcast address must be a usable IPv4 broadcast destination",
                    ));
                }
                Ok(address.to_string())
            })
            .transpose()?
    } else {
        None
    };
    let ip_address = body
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let address = value
                .parse::<Ipv4Addr>()
                .map_err(|_| WolHttpError::bad_request("IP address must be an IPv4 literal"))?;
            if address.is_unspecified() || address.is_multicast() || address.is_broadcast() {
                return Err(WolHttpError::bad_request(
                    "IP address must be a usable unicast IPv4 address",
                ));
            }
            Ok(address.to_string())
        })
        .transpose()?;
    Ok(ValidatedTargetBody {
        name,
        mac,
        note: note.to_string(),
        relay_id,
        broadcast_address,
        ip_address,
    })
}

fn discovery_job_error(error: DiscoveryJobError) -> WolHttpError {
    match error {
        DiscoveryJobError::BadRequest(message) => WolHttpError::bad_request(message),
        DiscoveryJobError::Conflict(message) => WolHttpError::conflict(message),
        DiscoveryJobError::Internal(message) => WolHttpError::internal(message),
    }
}

fn validate_name(value: &str, entity: &str) -> Result<String, WolHttpError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_NAME_LENGTH {
        return Err(WolHttpError::bad_request(format!(
            "{entity} name must contain between 1 and {MAX_NAME_LENGTH} characters"
        )));
    }
    Ok(value.to_string())
}

fn dispatch_error(error: DispatchError) -> WolHttpError {
    match error {
        DispatchError::Network { message, .. } => WolHttpError::new(
            StatusCode::BAD_GATEWAY,
            format!("Failed to send WoL request: {message}"),
        ),
        DispatchError::Timeout { .. } => WolHttpError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "Relay acknowledgement timed out; broadcast status is unknown",
        ),
        DispatchError::Relay { status, .. } => WolHttpError::new(
            StatusCode::BAD_GATEWAY,
            match status {
                AckStatus::ClockSkew => {
                    "Relay rejected the request because its clock is out of sync"
                }
                AckStatus::InvalidTarget => "Relay rejected the target MAC address",
                AckStatus::BroadcastFailed => "Relay failed to send the local broadcast",
                AckStatus::InternalError => "Relay reported an internal error",
                AckStatus::Ok
                | AckStatus::TargetOnline
                | AckStatus::TargetOffline
                | AckStatus::TargetUnknown => "Relay returned an unexpected acknowledgement",
            },
        ),
    }
}

fn internal_error(action: &str, error: impl std::fmt::Display) -> WolHttpError {
    tracing::warn!(%error, action, "WoL operation failed");
    WolHttpError::internal(format!("Failed to {action}"))
}

fn default_relay_port() -> u16 {
    DEFAULT_RELAY_PORT
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
        settings.internal_rpc_token = "wol-test-token".to_string();
        settings.request_timeout = std::time::Duration::from_millis(100);
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    #[test]
    fn validates_and_normalizes_relay_and_target_inputs() {
        let relay = RelayBody {
            name: " Home ".to_string(),
            address: "127.0.0.1".to_string(),
            port: 40009,
            enabled: true,
        };
        assert_eq!(validate_relay_body(&relay).unwrap().0, "Home");
        let target = TargetBody {
            name: " Workstation ".to_string(),
            mac: "02-11-22-33-44-55".to_string(),
            note: " Study ".to_string(),
            relay_id: None,
            broadcast_address: Some("192.168.31.255".to_string()),
            ip_address: Some("192.168.31.20".to_string()),
            enabled: true,
        };
        assert_eq!(
            validate_target_body(&target).unwrap().mac,
            "02:11:22:33:44:55"
        );
        assert_eq!(validate_target_body(&target).unwrap().note, "Study");
        let mut invalid_note = target;
        invalid_note.note = "x".repeat(MAX_NOTE_LENGTH + 1);
        assert!(validate_target_body(&invalid_note).is_err());
    }

    #[tokio::test]
    async fn pairing_code_enables_receiver_with_safe_automatic_defaults() {
        let (_directory, state) = test_state().await;
        let created = create_relay_inner(
            &state,
            RelayBody {
                name: "Remote network".to_string(),
                address: "127.0.0.1".to_string(),
                port: 40123,
                enabled: true,
            },
        )
        .await
        .unwrap();
        let code = created["bootstrap"]["pairingCode"].as_str().unwrap();
        let response = pair_local_relay_inner(&state, code).await.unwrap();
        assert_eq!(response["config"]["enabled"], true);
        assert_eq!(response["config"]["listenAddress"], "0.0.0.0");
        assert_eq!(response["config"]["port"], DEFAULT_RELAY_PORT);
        assert_eq!(response["config"]["pskConfigured"], true);
        assert!(
            response["config"]["broadcastDestinations"]
                .as_array()
                .is_some_and(|values| !values.is_empty())
        );
        assert!(!response.to_string().contains(code));
    }

    #[tokio::test]
    async fn crud_enforces_normalization_uniqueness_references_and_secret_separation() {
        let (_directory, state) = test_state().await;
        let created = create_relay_inner(
            &state,
            RelayBody {
                name: " Home ".to_string(),
                address: "127.0.0.1".to_string(),
                port: 40009,
                enabled: true,
            },
        )
        .await
        .unwrap();
        let relay_id = created["relay"]["id"].as_str().unwrap().to_string();
        let pairing_code = created["bootstrap"]["pairingCode"]
            .as_str()
            .unwrap()
            .to_string();
        let psk = decode_pairing_code(&pairing_code).unwrap().psk;
        assert!(pairing_code.starts_with(PAIRING_CODE_PREFIX));
        assert_eq!(psk.len(), 43);
        assert!(created["relay"].get("psk").is_none());
        assert!(created["bootstrap"].get("psk").is_none());

        let target = create_target_inner(
            &state,
            TargetBody {
                name: " Workstation ".to_string(),
                mac: "02-11-22-33-44-55".to_string(),
                note: "Upstairs".to_string(),
                relay_id: Some(relay_id.clone()),
                broadcast_address: None,
                ip_address: Some("192.168.50.20".to_string()),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(target.mac, "02:11:22:33:44:55");
        assert_eq!(target.note, "Upstairs");
        let local_target = create_target_inner(
            &state,
            TargetBody {
                name: "Local workstation".to_string(),
                mac: "02:11:22:33:44:66".to_string(),
                note: String::new(),
                relay_id: None,
                broadcast_address: Some("192.168.31.255".to_string()),
                ip_address: Some("192.168.31.20".to_string()),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(local_target.delivery_mode, "local");
        assert_eq!(local_target.relay_id, None);
        assert_eq!(
            create_target_inner(
                &state,
                TargetBody {
                    name: "Duplicate".to_string(),
                    mac: "021122334455".to_string(),
                    note: String::new(),
                    relay_id: Some(relay_id.clone()),
                    broadcast_address: None,
                    ip_address: None,
                    enabled: true,
                },
            )
            .await
            .unwrap_err()
            .status,
            StatusCode::CONFLICT
        );
        assert_eq!(
            delete_relay_inner(&state, &relay_id)
                .await
                .unwrap_err()
                .status,
            StatusCode::CONFLICT
        );
        state
            .store
            .set_key_if_not_exists_with_ttl(
                &format!("fn_knock:wol:runtime:cooldown:{}", target.id),
                "1",
                super::super::service::WAKE_COOLDOWN_SECONDS,
            )
            .await
            .unwrap();
        assert_eq!(
            wake_target_inner(&state, &target.id)
                .await
                .unwrap_err()
                .status,
            StatusCode::TOO_MANY_REQUESTS
        );

        let stored = state
            .store
            .export_backup_entries_by_prefix_limited("fn_knock:wol:", 1024 * 1024, |_| true)
            .await
            .unwrap();
        assert!(!serde_json::to_string(&stored).unwrap().contains(&psk));

        let online_status = super::super::store::TargetStatusRecord {
            state: "online".to_string(),
            checked_at: Some(time_utils::now_iso()),
            last_online_at: Some(time_utils::now_iso()),
            observed_ip: Some("192.168.50.20".to_string()),
            last_error: None,
        };
        super::super::store::save_target_status(&state, &target.id, &online_status)
            .await
            .unwrap();
        update_target_inner(
            &state,
            &target.id,
            TargetBody {
                name: target.name.clone(),
                mac: target.mac.clone(),
                note: "Changed note only".to_string(),
                relay_id: target.relay_id.clone(),
                broadcast_address: None,
                ip_address: target.ip_address.clone(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            super::super::store::load_target_status(&state, &target.id)
                .await
                .unwrap()
                .state,
            "online"
        );
        update_target_inner(
            &state,
            &target.id,
            TargetBody {
                name: target.name.clone(),
                mac: target.mac.clone(),
                note: "Changed address".to_string(),
                relay_id: target.relay_id.clone(),
                broadcast_address: None,
                ip_address: Some("192.168.50.21".to_string()),
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            super::super::store::load_target_status(&state, &target.id)
                .await
                .unwrap()
                .state,
            "unknown"
        );
        super::super::store::save_target_status(&state, &target.id, &online_status)
            .await
            .unwrap();
        let deleted = delete_target(State(state.clone()), Path(target.id.clone())).await;
        assert_eq!(deleted.status(), StatusCode::OK);
        assert!(
            state
                .store
                .get_json_value(&format!("fn_knock:wol:target-status:{}", target.id))
                .await
                .unwrap()
                .is_none()
        );
        delete_target_record(&state, &local_target.id)
            .await
            .unwrap();
        delete_relay_inner(&state, &relay_id).await.unwrap();
        assert!(load_relay(&state, &relay_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn built_in_relay_handles_authenticated_probe_and_broadcast_end_to_end() {
        let (_directory, state) = test_state().await;
        let mut config = state.store.get_config().await.unwrap();
        config["wol_feature"]["enabled"] = json!(true);
        state.store.save_config(&config).await.unwrap();
        let port_reservation = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let relay_port = port_reservation.local_addr().unwrap().port();
        drop(port_reservation);
        let broadcast_receiver = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();

        let created = create_relay_inner(
            &state,
            RelayBody {
                name: "Built-in".to_string(),
                address: "127.0.0.1".to_string(),
                port: relay_port,
                enabled: true,
            },
        )
        .await
        .unwrap();
        let relay_id = created["relay"]["id"].as_str().unwrap().to_string();
        let pairing_code = created["bootstrap"]["pairingCode"].as_str().unwrap();
        let pairing = decode_pairing_code(pairing_code).unwrap();
        let psk = pairing.psk;
        let response = update_local_relay_inner(
            &state,
            LocalRelayBody {
                enabled: true,
                relay_id: relay_id.clone(),
                key_version: 1,
                listen_address: "127.0.0.1".to_string(),
                port: relay_port,
                broadcast_destinations: vec![broadcast_receiver.local_addr().unwrap().to_string()],
                allowed_sources: vec!["127.0.0.1/32".to_string()],
                psk: Some(psk.clone()),
            },
        )
        .await
        .unwrap();
        assert!(!response.to_string().contains(&psk));

        super::super::relay::start_wol_relay_tasks(state.clone());
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if state.wol_relay_status.read().await["active"] == true {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("built-in Relay should start");

        let probe = dispatch_for_relay(&state, &relay_id, Command::Probe, None)
            .await
            .unwrap();
        assert_eq!(probe.status, "ready");
        let mac = "02:11:22:33:44:55".parse::<MacAddress>().unwrap();
        let wake = dispatch_for_relay(&state, &relay_id, Command::Wake, Some(mac))
            .await
            .unwrap();
        assert_eq!(wake.status, "broadcasted");

        let mut magic = [0_u8; 103];
        let (length, _) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            broadcast_receiver.recv_from(&mut magic),
        )
        .await
        .expect("Magic Packet should arrive")
        .unwrap();
        assert_eq!(length, 102);
        assert_eq!(&magic[..6], &[0xff; 6]);

        let backup = state
            .store
            .export_backup_entries_by_prefix_limited("fn_knock:wol:", 1024 * 1024, |_| true)
            .await
            .unwrap();
        assert!(!serde_json::to_string(&backup).unwrap().contains(&psk));
        super::super::clear_secrets_after_backup_restore(&state)
            .await
            .unwrap();
        assert_eq!(
            local_relay_response(&state).await.unwrap()["config"]["pskConfigured"],
            false
        );
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if state.wol_relay_status.read().await["active"] == false {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("built-in Relay should stop after credentials are cleared");
        state.shutdown.cancel();
    }
}
