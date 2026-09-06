use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    sync::RwLock,
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{fs as tokio_fs, process::Command, sync::Mutex};
use utoipa_axum::{router::OpenApiRouter, routes};

mod cloudflare_api;
mod connectivity;
mod managed;
mod optimization;
mod secrets;

use connectivity::{CloudflaredConnectivity, CloudflaredSignal, parse_cloudflared_signal};
use secrets::{CloudflaredSecretStore, SecretKind, atomic_private_write};

use crate::{
    cloudflared_utils::{
        cloudflared_asset_name, cloudflared_binary_path, cloudflared_install_is_current,
        detect_cloudflared_platform,
    },
    i18n::Translator,
    response,
    state::AppState,
    system_events, time_utils,
    tunnels::{
        TUNNEL_RUNTIME_KEY,
        connectivity::{ConnectedEventAction, TunnelDisconnectEvent},
        supervisor::{
            OutputStream, ProcessLaunch, SupervisorFailure, SupervisorHandle, SupervisorSnapshot,
            TunnelProcessAdapter,
        },
    },
};

const LOG_KEY: &str = "fn_knock:cloudflared:logs";
const LOG_TTL_SECONDS: usize = 24 * 3600;
const LOG_MAX_LEN: usize = 1000;
const CLOUDFLARED_RUNTIME_KEY: &str = "fn_knock:cloudflared:runtime:v2";
const CLOUDFLARED_SUPERVISOR_KEY: &str = "cloudflared";

fn cloudflared_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.cloudflared.{key}"))
}

fn tunnel_manager_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.tunnelManagers.cloudflared.{key}"))
}

fn localize_cloudflared_error(translator: &Translator, message: &str) -> String {
    match message.trim() {
        "Cloudflared token is required" => cloudflared_text(translator, "missingToken"),
        "Cloudflared is not initialized" => cloudflared_text(translator, "notInitialized"),
        "Cloudflared platform is unsupported" => {
            tunnel_manager_text(translator, "platformUnsupported")
        }
        value => value.to_string(),
    }
}

struct CloudflaredManager {
    dir: PathBuf,
    config_path: PathBuf,
    bin_path: PathBuf,
    pid_path: PathBuf,
    runtime_token_path: PathBuf,
}

#[derive(Deserialize)]
struct LogsQuery {
    limit: Option<String>,
    cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct CloudflaredPidRecord {
    pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    creation_time: Option<u64>,
}

#[derive(Clone)]
struct CloudflaredConfig {
    token: String,
    protocol: String,
}

pub fn cloudflared_routes() -> Router<AppState> {
    cloudflared_openapi_routes().into()
}

pub(crate) fn cloudflared_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(config))
        .routes(routes!(save_config))
        .routes(routes!(start))
        .routes(routes!(stop))
        .routes(routes!(logs))
        .routes(routes!(clear_logs))
        .routes(routes!(poll))
        .merge(managed::openapi_routes())
        .merge(optimization::openapi_routes())
}

pub(crate) async fn cleanup_before_data_clear(state: &AppState) -> Result<(), String> {
    managed::cleanup_before_data_clear(state)
        .await
        .map_err(|error| error.to_string())
}

pub fn start_cloudflared_tasks(state: AppState) {
    manager(&state).ensure_dir();
    optimization::start_tasks(state.clone());
    let task_state = state.clone();
    state.spawn_background("cloudflared-restore", async move {
        match should_resume_tunnel(&task_state).await {
            Ok(true) => {
                let translator = Translator::from_state(&task_state).await;
                if let Err(error) = append_logs(
                    &task_state,
                    vec![cloudflared_text(&translator, "resumeOnBoot")],
                )
                .await
                {
                    tracing::warn!(%error, "failed to append cloudflared resume log");
                }
                if let Err(error) = ensure_cloudflared_supervisor(&task_state).await {
                    let _ = append_logs(&task_state, vec![format!("resume error: {error}")]).await;
                }
            }
            Ok(false) => {
                if let Err(error) = ensure_cloudflared_supervisor(&task_state).await {
                    tracing::warn!(%error, "failed to initialize cloudflared supervisor");
                }
            }
            Err(error) => tracing::warn!(%error, "failed to load cloudflared resume state"),
        }
    });
}

pub(crate) fn schedule_managed_reconcile_after_host_mappings_change(state: AppState) {
    optimization::schedule_after_host_mappings_change(state);
}

pub(crate) async fn clear_credentials_after_backup_restore(state: &AppState) -> Result<(), String> {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let manager = manager(state);
    // Reading first performs the legacy plaintext migration and rewrites the
    // non-secret config before both encrypted credentials are removed.
    let previous_config = manager.read_config()?;
    let secrets = manager.secret_store();
    let previous_api_token = secrets.read(SecretKind::ApiToken)?;
    let previous_runtime_token = match tokio_fs::read(&manager.runtime_token_path).await {
        Ok(value) => Some(value),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let handle = ensure_cloudflared_supervisor(state).await?;
    let previous_snapshot = handle.snapshot();
    let should_resume = previous_snapshot.desired_running || previous_snapshot.running;
    if let Err(error) = handle.stop().await {
        if should_resume && let Err(rollback_error) = handle.start().await {
            return Err(format!("{error}; tunnel rollback failed: {rollback_error}"));
        }
        return Err(error);
    }

    let cleanup_result = async {
        secrets.delete(SecretKind::ApiToken)?;
        secrets.delete(SecretKind::TunnelToken)?;
        match tokio_fs::remove_file(&manager.runtime_token_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    .await;
    if let Err(error) = cleanup_result {
        let rollback = restore_credentials_after_failed_backup_restore(
            &manager,
            previous_api_token.as_deref(),
            &previous_config,
            previous_runtime_token.as_deref(),
            &handle,
            should_resume,
        )
        .await;
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!(
                "{error}; credential rollback failed: {rollback_error}"
            )),
        };
    }
    Ok(())
}

async fn restore_credentials_after_failed_backup_restore(
    manager: &CloudflaredManager,
    api_token: Option<&str>,
    config: &CloudflaredConfig,
    runtime_token: Option<&[u8]>,
    handle: &SupervisorHandle,
    should_resume: bool,
) -> Result<(), String> {
    let secrets = manager.secret_store();
    match api_token {
        Some(token) => secrets.write(SecretKind::ApiToken, token)?,
        None => secrets.delete(SecretKind::ApiToken)?,
    }
    if config.token.is_empty() {
        secrets.delete(SecretKind::TunnelToken)?;
    } else {
        secrets.write(SecretKind::TunnelToken, &config.token)?;
    }
    manager.write_config(config)?;
    match runtime_token {
        Some(token) => atomic_private_write(&manager.runtime_token_path, token)?,
        None => match fs::remove_file(&manager.runtime_token_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        },
    }
    if should_resume {
        handle.start().await?;
    }
    Ok(())
}

#[utoipa::path(get, path = "/api/admin/cloudflared/status", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_status", responses((status = 200, description = "Cloudflared status")))]
async fn status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    let asset = manager.asset_status();
    let snapshot = match ensure_cloudflared_supervisor(&state).await {
        Ok(handle) => handle.snapshot(),
        Err(error) => {
            tracing::warn!(%error, "failed to load cloudflared supervisor status");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "statusLoadFailed"),
            );
        }
    };
    response::ok(json!({
        "initialized": asset.get("downloaded").and_then(Value::as_bool).unwrap_or(false),
        "platform": asset.get("platform").cloned().unwrap_or_else(|| json!("unsupported")),
        "running": snapshot.running,
        "pid": snapshot.pid,
        "desiredRunning": snapshot.desired_running,
        "supervisor": snapshot,
    }))
    .into_response()
}

#[utoipa::path(get, path = "/api/admin/cloudflared/config", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_config", responses((status = 200, description = "Cloudflared configuration")))]
async fn config(State(state): State<AppState>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let translator = Translator::from_state(&state).await;
    match manager(&state).read_config() {
        Ok(config) => response::ok(managed::public_config_state(&state, &config.protocol).await)
            .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read cloudflared config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "configReadFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/cloudflared/config", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_config", responses((status = 200, description = "Updated Cloudflared configuration")))]
async fn save_config(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    let previous_config = match manager.read_config() {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read cloudflared config before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "configReadFailed"),
            );
        }
    };
    let previous_managed = managed::load_managed_config(&state).await;
    let token_input = body.get("token").and_then(Value::as_str).map(str::trim);
    let clear_token = body.get("clearToken").and_then(Value::as_bool) == Some(true);
    let replacement_token = (!clear_token)
        .then_some(token_input)
        .flatten()
        .filter(|value| !value.is_empty());
    let credential_changed = clear_token || replacement_token.is_some();
    let credential_result = if clear_token {
        manager.secret_store().delete(SecretKind::TunnelToken)
    } else if let Some(token) = replacement_token {
        manager.secret_store().write(SecretKind::TunnelToken, token)
    } else {
        Ok(())
    };
    if let Err(error) = credential_result {
        tracing::warn!(%error, "failed to update cloudflared Tunnel Token");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            cloudflared_text(&translator, "configWriteFailed"),
        );
    }
    if credential_changed && let Err(error) = managed::mark_manual_mode(&state).await {
        let rollback =
            restore_manual_config(&state, &manager, &previous_config, &previous_managed).await;
        tracing::warn!(%error, ?rollback, "failed to mark cloudflared manual mode");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            cloudflared_text(&translator, "configWriteFailed"),
        );
    }
    let config = CloudflaredConfig {
        token: replacement_token.map(str::to_string).unwrap_or_else(|| {
            if clear_token {
                String::new()
            } else {
                previous_config.token.clone()
            }
        }),
        protocol: normalize_protocol(body.get("protocol").and_then(Value::as_str)),
    };
    match manager.write_config(&config) {
        Ok(()) => {
            let handle = match ensure_cloudflared_supervisor(&state).await {
                Ok(handle) => handle,
                Err(error) => {
                    let rollback = restore_manual_config(
                        &state,
                        &manager,
                        &previous_config,
                        &previous_managed,
                    )
                    .await;
                    if let Err(rollback_error) = rollback {
                        tracing::error!(%rollback_error, "failed to roll back cloudflared config update");
                    }
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        localize_cloudflared_error(&translator, &error),
                    );
                }
            };
            let previously_desired = handle.snapshot().desired_running;
            let restart = if clear_token {
                handle.stop().await
            } else if previously_desired {
                handle.restart().await.map(drop)
            } else {
                Ok(())
            };
            match restart {
                Ok(()) => response::success_empty().into_response(),
                Err(error) => {
                    let rollback = restore_manual_config(
                        &state,
                        &manager,
                        &previous_config,
                        &previous_managed,
                    )
                    .await;
                    if let Err(rollback_error) = rollback {
                        tracing::error!(%rollback_error, "failed to roll back cloudflared config update");
                    } else {
                        let recovery = if previously_desired {
                            handle.start().await.map(drop)
                        } else {
                            handle.stop().await
                        };
                        if let Err(recovery_error) = recovery {
                            tracing::error!(%recovery_error, "failed to restore cloudflared runtime after rollback");
                        }
                    }
                    tracing::warn!(%error, "failed to restart cloudflared after config update");
                    response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        localize_cloudflared_error(&translator, &error),
                    )
                }
            }
        }
        Err(error) => {
            let rollback =
                restore_manual_config(&state, &manager, &previous_config, &previous_managed).await;
            if let Err(rollback_error) = rollback {
                tracing::error!(%rollback_error, "failed to roll back cloudflared config update");
            }
            tracing::warn!(%error, "failed to write cloudflared config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "configWriteFailed"),
            )
        }
    }
}

async fn restore_manual_config(
    state: &AppState,
    manager: &CloudflaredManager,
    previous_config: &CloudflaredConfig,
    previous_managed: &Value,
) -> Result<(), String> {
    if previous_config.token.is_empty() {
        manager.secret_store().delete(SecretKind::TunnelToken)?;
    } else {
        manager
            .secret_store()
            .write(SecretKind::TunnelToken, &previous_config.token)?;
    }
    managed::save_managed_config(state, previous_managed)
        .await
        .map_err(|error| error.to_string())?;
    manager.write_config(previous_config)
}

#[utoipa::path(post, path = "/api/admin/cloudflared/start", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_start", responses((status = 200, description = "Started Cloudflared")))]
async fn start(State(state): State<AppState>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    if !manager.downloaded() {
        return response::error(
            StatusCode::BAD_REQUEST,
            cloudflared_text(&translator, "notInitialized"),
        );
    }
    let handle = match ensure_cloudflared_supervisor(&state).await {
        Ok(handle) => handle,
        Err(error) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                localize_cloudflared_error(&translator, &error),
            );
        }
    };
    match handle.start().await {
        Ok(pid) => response::ok(json!({ "pid": pid })).into_response(),
        Err(error) => {
            let _ = append_logs(&state, vec![format!("start error: {error}")]).await;
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                localize_cloudflared_error(&translator, &error),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/cloudflared/stop", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_stop", responses((status = 200, description = "Stopped Cloudflared")))]
async fn stop(State(state): State<AppState>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let translator = Translator::from_state(&state).await;
    let result = match ensure_cloudflared_supervisor(&state).await {
        Ok(handle) => handle.stop().await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to stop cloudflared");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "stopFailed"),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/cloudflared/logs", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_logs", responses((status = 200, description = "Cloudflared logs")))]
async fn logs(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let limit = parse_log_limit(query.limit.as_deref(), 200, LOG_MAX_LEN);
    match state
        .storage
        .store
        .list_log_buffer(LOG_KEY, limit, LOG_MAX_LEN)
        .await
    {
        Ok(logs) => response::ok(logs).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsListFailed"),
            )
        }
    }
}

#[utoipa::path(delete, path = "/api/admin/cloudflared/logs", tag = "cloudflared", operation_id = "delete_api_admin_cloudflared_logs", responses((status = 200, description = "Cleared Cloudflared logs")))]
async fn clear_logs(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.clear_log_buffer(LOG_KEY).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to clear cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsClearFailed"),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/cloudflared/poll", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_poll", responses((status = 200, description = "Cloudflared poll result")))]
async fn poll(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .storage
        .store
        .poll_log_buffer(LOG_KEY, query.cursor.as_deref())
        .await
    {
        Ok(mut result) => {
            let snapshot = match ensure_cloudflared_supervisor(&state).await {
                Ok(handle) => handle.snapshot(),
                Err(error) => {
                    tracing::warn!(%error, "failed to poll cloudflared supervisor");
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        cloudflared_text(&translator, "statusLoadFailed"),
                    );
                }
            };
            let cursor = result.get("cursor").cloned().unwrap_or_else(|| json!(0));
            let reset = result.get("reset").cloned().unwrap_or(Value::Bool(false));
            let logs = result
                .as_object_mut()
                .and_then(|object| object.remove("items"))
                .unwrap_or_else(|| json!([]));
            response::ok(json!({
                "cursor": cursor,
                "reset": reset,
                "logs": logs,
                "status": {
                    "running": snapshot.running,
                    "pid": snapshot.pid,
                    "desiredRunning": snapshot.desired_running,
                    "supervisor": snapshot,
                }
            }))
            .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to poll cloudflared logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cloudflared_text(&translator, "logsPollFailed"),
            )
        }
    }
}

impl CloudflaredManager {
    fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("cloudflared");
        let bin_path = cloudflared_binary_path(data_dir, detect_cloudflared_platform())
            .unwrap_or_else(|| {
                dir.join(if cfg!(windows) {
                    "cloudflared.exe"
                } else {
                    "cloudflared"
                })
            });
        Self {
            config_path: dir.join("cloudflared.json"),
            bin_path,
            pid_path: dir.join("cloudflared.pid"),
            runtime_token_path: dir.join("tunnel-token.runtime"),
            dir,
        }
    }

    fn ensure_dir(&self) {
        let _ = fs::create_dir_all(&self.dir);
    }

    fn asset_status(&self) -> Value {
        let platform = detect_cloudflared_platform();
        let supported = cloudflared_asset_name(platform).is_some();
        let downloaded = supported
            && cloudflared_install_is_current(self.dir.parent().unwrap_or(&self.dir), platform);
        json!({
            "supported": supported,
            "platform": platform,
            "downloaded": downloaded,
            "progress": { "status": "idle", "percent": 0 }
        })
    }

    fn downloaded(&self) -> bool {
        self.asset_status()
            .get("downloaded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn executable(&self) -> Result<String, String> {
        if cloudflared_asset_name(detect_cloudflared_platform()).is_none() {
            return Err("Cloudflared platform is unsupported".to_string());
        }
        let data_dir = self.dir.parent().unwrap_or(&self.dir);
        let platform = detect_cloudflared_platform();
        if cloudflared_install_is_current(data_dir, platform) {
            Ok(self.bin_path.to_string_lossy().to_string())
        } else {
            Err("Cloudflared is not initialized".to_string())
        }
    }

    fn read_config(&self) -> Result<CloudflaredConfig, String> {
        self.ensure_dir();
        if !self.config_path.exists() {
            let config = CloudflaredConfig {
                token: self
                    .secret_store()
                    .read(SecretKind::TunnelToken)?
                    .unwrap_or_default(),
                protocol: "auto".to_string(),
            };
            self.write_config(&config)?;
            return Ok(config);
        }
        let raw = fs::read_to_string(&self.config_path).map_err(|error| error.to_string())?;
        let mut value = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}));
        let legacy_token = value
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !legacy_token.is_empty() {
            self.secret_store()
                .write(SecretKind::TunnelToken, &legacy_token)?;
            if let Some(object) = value.as_object_mut() {
                object.remove("token");
                object.insert("credential_migrated".to_string(), json!(true));
            }
            self.write_non_secret_value(&value)?;
        }
        Ok(CloudflaredConfig {
            token: self
                .secret_store()
                .read(SecretKind::TunnelToken)?
                .unwrap_or_default(),
            protocol: normalize_protocol(value.get("protocol").and_then(Value::as_str)),
        })
    }

    fn write_config(&self, config: &CloudflaredConfig) -> Result<(), String> {
        self.ensure_dir();
        let value = json!({
            "protocol": normalize_protocol(Some(&config.protocol)),
            "credential_migrated": true,
        });
        self.write_non_secret_value(&value)
    }

    fn write_non_secret_value(&self, value: &Value) -> Result<(), String> {
        atomic_private_write(
            &self.config_path,
            serde_json::to_string_pretty(value)
                .unwrap_or_else(|_| "{}".to_string())
                .as_bytes(),
        )
        .map_err(|error| error.to_string())
    }

    fn secret_store(&self) -> CloudflaredSecretStore {
        CloudflaredSecretStore::new(self.dir.clone())
    }

    fn write_runtime_token(&self, token: &str) -> Result<(), String> {
        atomic_private_write(&self.runtime_token_path, token.as_bytes())
    }
}

fn manager(state: &AppState) -> CloudflaredManager {
    CloudflaredManager::new(&state.settings.data_dir)
}

pub(crate) async fn ensure_cloudflared_supervisor(
    state: &AppState,
) -> Result<SupervisorHandle, String> {
    if let Some(handle) = state
        .tunnel
        .supervisors
        .get(CLOUDFLARED_SUPERVISOR_KEY)
        .await
    {
        return Ok(handle);
    }
    let desired_running = should_resume_tunnel(state)
        .await
        .map_err(|error| error.to_string())?;
    let mut initial = state
        .storage
        .store
        .get_json_value(CLOUDFLARED_RUNTIME_KEY)
        .await
        .map_err(|error| error.to_string())?
        .and_then(|value| serde_json::from_value::<SupervisorSnapshot>(value).ok())
        .unwrap_or_default();
    // The dedicated snapshot is the authoritative v2 record. The aggregate
    // flag remains an old-storage compatibility source, so either record can
    // restore an enabled tunnel after a partially completed legacy write.
    initial.desired_running |= desired_running;
    let adapter = Arc::new(CloudflaredProcessAdapter {
        state: state.clone(),
        manager: manager(state),
        connection: Arc::new(Mutex::new(CloudflaredConnectivity::default())),
        secret: RwLock::new(
            manager(state)
                .read_config()
                .map(|config| config.token)
                .unwrap_or_default(),
        ),
    });
    Ok(state
        .tunnel
        .supervisors
        .ensure(adapter, initial, state.shutdown.clone())
        .await)
}

pub(crate) async fn pause_cloudflared_for_asset_update(state: &AppState) -> Result<bool, String> {
    let handle = ensure_cloudflared_supervisor(state).await?;
    let snapshot = handle.snapshot();
    let should_resume =
        should_resume_cloudflared_after_asset_update(snapshot.desired_running, snapshot.running);
    if should_resume {
        handle.pause_for_restart().await?;
    }
    Ok(should_resume)
}

fn should_resume_cloudflared_after_asset_update(desired_running: bool, running: bool) -> bool {
    desired_running || running
}

pub(crate) async fn resume_cloudflared_after_asset_update(
    state: &AppState,
    should_resume: bool,
) -> Result<(), String> {
    if should_resume {
        ensure_cloudflared_supervisor(state).await?.start().await?;
    }
    Ok(())
}

struct CloudflaredProcessAdapter {
    state: AppState,
    manager: CloudflaredManager,
    connection: Arc<Mutex<CloudflaredConnectivity>>,
    secret: RwLock<String>,
}

#[async_trait]
impl TunnelProcessAdapter for CloudflaredProcessAdapter {
    fn key(&self) -> String {
        CLOUDFLARED_SUPERVISOR_KEY.to_string()
    }

    fn label(&self) -> String {
        "cloudflared".to_string()
    }

    async fn prepare_launch(&self) -> Result<ProcessLaunch, String> {
        let config = self.manager.read_config()?;
        if !cloudflared_token_configured(&config.token) {
            return Err("Cloudflared token is required".to_string());
        }
        let executable = self.manager.executable()?;
        ensure_token_file_supported(&executable).await?;
        *self
            .secret
            .write()
            .unwrap_or_else(|error| error.into_inner()) = config.token.clone();
        self.manager.write_runtime_token(&config.token)?;
        Ok(ProcessLaunch {
            executable: executable.into(),
            args: build_args(&config, &self.manager.runtime_token_path)
                .into_iter()
                .map(Into::into)
                .collect(),
            current_dir: self.manager.dir.clone(),
        })
    }

    async fn find_existing_pid(&self) -> Option<u32> {
        let pid_record = read_pid_record(&self.manager.pid_path).await;
        #[cfg(windows)]
        {
            let record = pid_record?;
            if is_owned_cloudflared_pid(
                record.pid,
                &self.current_secret(),
                &self.manager.bin_path,
                &self.manager.runtime_token_path,
                record.creation_time,
            )
            .await
            {
                return Some(record.pid);
            }
            return None;
        }
        #[cfg(not(windows))]
        {
            let runtime_pid = self
                .state
                .storage
                .store
                .get_json_value(CLOUDFLARED_RUNTIME_KEY)
                .await
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_value::<SupervisorSnapshot>(value).ok())
                .and_then(|snapshot| snapshot.pid);
            for pid in [runtime_pid, pid_record.map(|record| record.pid)]
                .into_iter()
                .flatten()
            {
                if is_owned_cloudflared_pid(
                    pid,
                    &self.current_secret(),
                    &self.manager.bin_path,
                    &self.manager.runtime_token_path,
                    None,
                )
                .await
                {
                    return Some(pid);
                }
            }
            None
        }
    }

    async fn owns_live_pid(&self, pid: u32) -> bool {
        let creation_time = read_pid_record(&self.manager.pid_path)
            .await
            .filter(|record| record.pid == pid)
            .and_then(|record| record.creation_time);
        is_owned_cloudflared_pid(
            pid,
            &self.current_secret(),
            &self.manager.bin_path,
            &self.manager.runtime_token_path,
            creation_time,
        )
        .await
    }

    async fn persist_snapshot(&self, snapshot: &SupervisorSnapshot) -> Result<(), String> {
        let previous = self
            .state
            .storage
            .store
            .get_json_value(CLOUDFLARED_RUNTIME_KEY)
            .await
            .map_err(|error| error.to_string())?;
        self.state
            .storage
            .store
            .set_json_value(
                CLOUDFLARED_RUNTIME_KEY,
                &serde_json::to_value(snapshot).map_err(|error| error.to_string())?,
            )
            .await
            .map_err(|error| error.to_string())?;
        let aggregate_result = if snapshot.desired_running {
            mark_tunnel_running(&self.state).await
        } else {
            mark_tunnel_stopped(&self.state).await
        };
        if let Err(error) = aggregate_result {
            let rollback = match previous {
                Some(value) => {
                    self.state
                        .storage
                        .store
                        .set_json_value(CLOUDFLARED_RUNTIME_KEY, &value)
                        .await
                }
                None => {
                    self.state
                        .storage
                        .store
                        .delete_key(CLOUDFLARED_RUNTIME_KEY)
                        .await
                }
            };
            return Err(match rollback {
                Ok(()) => error,
                Err(rollback_error) => {
                    format!("{error}; failed to roll back cloudflared runtime: {rollback_error}")
                }
            });
        }
        Ok(())
    }

    fn sanitize_output(&self, line: &str) -> String {
        let token = self.current_secret();
        redact_cloudflared_line(line, &token)
    }

    async fn append_output(&self, stream: OutputStream, line: String) {
        let source = match stream {
            OutputStream::Stdout => "stdout",
            OutputStream::Stderr => "stderr",
        };
        let logged = format!("[{source}] {line}");
        let _ = append_logs(&self.state, vec![logged]).await;
        handle_cloudflared_runtime_signal(&self.state, &self.connection, &line).await;
    }

    async fn append_supervisor_log(&self, line: String) {
        let _ = append_logs(&self.state, vec![line]).await;
    }

    async fn set_expected_stop(&self, expected: bool) {
        let mut connection = self.connection.lock().await;
        connection.set_expected_stop(expected);
    }

    async fn finish_expected_stop(&self, stopped: bool) {
        self.connection.lock().await.finish_expected_stop(stopped);
        if !stopped {
            emit_cloudflared_connectivity_with_state(
                &self.state,
                &self.connection,
                CloudflaredSignal::Reconcile,
                Some("cloudflared termination failed; resumed connection monitoring"),
                None,
            )
            .await;
        }
    }

    async fn on_unexpected_exit(&self, pid: Option<u32>, failure: &SupervisorFailure) {
        let mut details = vec![
            "cloudflared stopped unexpectedly".to_string(),
            format!(
                "pid={}",
                pid.map_or_else(|| "-".to_string(), |pid| pid.to_string())
            ),
            format!("startedAt={}", failure.started_at.as_deref().unwrap_or("-")),
            format!("exitedAt={}", failure.at),
            format!("reason={}", failure.reason),
            format!("uptimeMs={}", failure.uptime_ms),
        ];
        if let Some(signal) = failure.signal {
            details.push(format!("signal={signal}"));
        }
        if let Some(code) = failure.exit_code {
            details.push(format!("exitCode={code}"));
        }
        if let Some(diagnosis) = failure.diagnosis.as_deref() {
            details.push(format!("diagnosis={diagnosis}"));
        }
        let mut lines = vec![details.join(" ")];
        lines.extend(
            failure
                .recent_stdout
                .iter()
                .map(|line| format!("[LAST stdout] {line}")),
        );
        lines.extend(
            failure
                .recent_stderr
                .iter()
                .map(|line| format!("[LAST stderr] {line}")),
        );
        let _ = append_logs(&self.state, lines).await;
        emit_cloudflared_connectivity_with_state(
            &self.state,
            &self.connection,
            CloudflaredSignal::ProcessExited,
            Some(&failure.reason),
            pid,
        )
        .await;
    }

    async fn remove_pid_file(&self) {
        let _ = tokio_fs::remove_file(&self.manager.pid_path).await;
        let _ = tokio_fs::remove_file(&self.manager.runtime_token_path).await;
    }

    async fn write_pid_file(&self, pid: u32) {
        let _ = tokio_fs::create_dir_all(&self.manager.dir).await;
        #[cfg(windows)]
        let content = serde_json::to_vec(&CloudflaredPidRecord {
            pid,
            creation_time: windows_process_creation_time(pid),
        })
        .unwrap_or_else(|_| format!(r#"{{"pid":{pid}}}"#).into_bytes());
        #[cfg(not(windows))]
        let content = format!("{pid}\n").into_bytes();
        let _ = tokio_fs::write(&self.manager.pid_path, content).await;
    }
}

async fn ensure_token_file_supported(executable: &str) -> Result<(), String> {
    let mut command = Command::new(executable);
    command.arg("--version");
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let output = command
        .output()
        .await
        .map_err(|error| format!("Failed to check cloudflared version: {error}"))?;
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if cloudflared_version_supports_token_file(&text) {
        Ok(())
    } else {
        Err("Cloudflared 2025.4.0 or later is required for secure token-file startup".to_string())
    }
}

fn cloudflared_version_supports_token_file(output: &str) -> bool {
    output
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .find_map(|candidate| {
            let mut parts = candidate.split('.');
            let year = parts.next()?.parse::<u32>().ok()?;
            let month = parts.next()?.parse::<u32>().ok()?;
            let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
            Some((year, month, patch) >= (2025, 4, 0))
        })
        .unwrap_or(false)
}

impl CloudflaredProcessAdapter {
    fn current_secret(&self) -> String {
        self.secret
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

async fn append_logs(state: &AppState, lines: Vec<String>) -> crate::storage::StorageResult<()> {
    let normalized = lines
        .into_iter()
        .map(|line| crate::tunnels::supervisor::bounded_log_line(line.trim_end()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Ok(());
    }
    state
        .storage
        .store
        .append_log_buffer(LOG_KEY, &normalized, LOG_TTL_SECONDS, LOG_MAX_LEN)
        .await
}

async fn handle_cloudflared_runtime_signal(
    state: &AppState,
    connection: &Arc<Mutex<CloudflaredConnectivity>>,
    line: &str,
) {
    // Parse the original line: truncating the displayed feedback can remove connIndex.
    let Some(signal) = parse_cloudflared_signal(line) else {
        return;
    };
    let message = normalize_tunnel_event_message(line);
    emit_cloudflared_connectivity_with_state(state, connection, signal, message.as_deref(), None)
        .await;
}

async fn emit_cloudflared_connectivity_with_state(
    state: &AppState,
    connection: &Arc<Mutex<CloudflaredConnectivity>>,
    signal: CloudflaredSignal,
    message: Option<&str>,
    pid: Option<u32>,
) {
    let mut connection_guard = connection.lock().await;
    let Some(connected) = connection_guard.observe_signal(signal) else {
        return;
    };
    if connected {
        match connection_guard
            .gate
            .observe_connected(tokio::time::Instant::now())
        {
            ConnectedEventAction::Ignore => return,
            ConnectedEventAction::PublishConnected => {}
            ConnectedEventAction::PublishDisconnectThenConnected(disconnected) => {
                publish_cloudflared_connectivity_event(
                    state,
                    false,
                    disconnected.pid,
                    disconnected.message.as_deref(),
                    Some(&disconnected.happened_at),
                )
                .await;
            }
        }
        let event_pid = cloudflared_event_pid(state, pid).await;
        publish_cloudflared_connectivity_event(state, true, event_pid, message, None).await;
        return;
    }

    let observed_at = tokio::time::Instant::now();
    let disconnected = TunnelDisconnectEvent {
        happened_at: time_utils::now_iso(),
        message: message.map(str::to_string),
        pid: cloudflared_event_pid(state, pid).await,
    };
    let Some(timer) = connection_guard
        .gate
        .observe_disconnected(observed_at, disconnected)
    else {
        return;
    };
    drop(connection_guard);
    let state = state.clone();
    let connection = Arc::clone(connection);
    let shutdown = state.shutdown.clone();
    let task_state = state.clone();
    state.spawn_background("cloudflared-disconnect-confirmation", async move {
        tokio::select! {
            _ = tokio::time::sleep_until(timer.deadline) => {}
            _ = timer.cancelled() => return,
            _ = shutdown.cancelled() => return,
        }
        let mut connection = connection.lock().await;
        let Some(disconnected) = connection
            .gate
            .confirm_disconnect(&timer, tokio::time::Instant::now())
        else {
            return;
        };
        publish_cloudflared_connectivity_event(
            &task_state,
            false,
            disconnected.pid,
            disconnected.message.as_deref(),
            Some(&disconnected.happened_at),
        )
        .await;
    });
}

async fn publish_cloudflared_connectivity_event(
    state: &AppState,
    connected: bool,
    pid: Option<u32>,
    message: Option<&str>,
    happened_at: Option<&str>,
) {
    if let Err(error) = system_events::publish_tunnel_connectivity_event(
        state,
        system_events::TunnelConnectivityEvent {
            tunnel: "cloudflared",
            connected,
            pid,
            message,
            instance_id: None,
            instance_name: None,
            is_primary: None,
            happened_at,
        },
    )
    .await
    {
        tracing::warn!(%error, "failed to publish cloudflared connectivity event");
    }
}

async fn cloudflared_event_pid(state: &AppState, pid: Option<u32>) -> Option<u32> {
    if pid.is_some() {
        return pid;
    }
    state
        .tunnel
        .supervisors
        .get(CLOUDFLARED_SUPERVISOR_KEY)
        .await
        .and_then(|handle| handle.snapshot().pid)
}

async fn read_pid_record(path: &Path) -> Option<CloudflaredPidRecord> {
    let raw = tokio_fs::read_to_string(path).await.ok()?;
    if let Ok(record) = serde_json::from_str::<CloudflaredPidRecord>(&raw) {
        return (record.pid > 0).then_some(record);
    }
    raw.trim()
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid > 0)
        .map(|pid| CloudflaredPidRecord {
            pid,
            creation_time: None,
        })
}

async fn is_owned_cloudflared_pid(
    pid: u32,
    token: &str,
    executable: &Path,
    token_file: &Path,
    expected_creation_time: Option<u64>,
) -> bool {
    if pid == std::process::id() || !i32::try_from(pid).is_ok_and(crate::unix::process_exists) {
        return false;
    }
    #[cfg(windows)]
    {
        let _ = (token, token_file);
        let Some(expected_creation_time) = expected_creation_time else {
            return false;
        };
        return windows_process_creation_time(pid) == Some(expected_creation_time)
            && windows_process_executable(pid)
                .is_some_and(|actual| windows_paths_match(&actual, executable));
    }
    #[cfg(not(windows))]
    {
        let _ = expected_creation_time;
        let args = read_process_args(pid).await;
        args.as_deref()
            .is_some_and(|args| is_cloudflared_process_args(args, token, executable, token_file))
    }
}

#[cfg(windows)]
fn windows_process_creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };

    // SAFETY: the process handle is checked, all FILETIME pointers are valid,
    // and the handle is closed before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0;
        CloseHandle(handle);
        ok.then_some((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }
}

#[cfg(windows)]
fn windows_process_executable(pid: u32) -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    };

    // Windows permits paths longer than MAX_PATH. Start generously and retry if
    // the process image path does not fit in the first buffer.
    for capacity in [512usize, 32_768] {
        // SAFETY: the process handle and output buffer are checked, and the
        // handle is closed before every return from this iteration.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buffer = vec![0u16; capacity];
            let mut size = u32::try_from(buffer.len()).ok()?;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                buffer.as_mut_ptr(),
                &mut size,
            ) != 0;
            CloseHandle(handle);
            if ok {
                buffer.truncate(usize::try_from(size).ok()?);
                return Some(PathBuf::from(OsString::from_wide(&buffer)));
            }
        }
    }
    None
}

#[cfg(windows)]
fn windows_paths_match(actual: &Path, expected: &Path) -> bool {
    let actual = fs::canonicalize(actual).unwrap_or_else(|_| actual.to_path_buf());
    let expected = fs::canonicalize(expected).unwrap_or_else(|_| expected.to_path_buf());
    actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
}

#[cfg(not(windows))]
async fn read_process_args(pid: u32) -> Option<Vec<String>> {
    #[cfg(target_os = "linux")]
    if let Ok(bytes) = tokio_fs::read(format!("/proc/{pid}/cmdline")).await
        && !bytes.is_empty()
    {
        let args = bytes
            .split(|byte| *byte == 0)
            .filter_map(|part| {
                let value = String::from_utf8_lossy(part).trim().to_string();
                (!value.is_empty()).then_some(value)
            })
            .collect::<Vec<_>>();
        if !args.is_empty() {
            return Some(args);
        }
    }
    #[cfg(target_os = "macos")]
    {
        read_macos_process_args(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("ps")
            .args(["-ww", "-p", &pid.to_string(), "-o", "args="])
            .output()
            .await
            .ok()?;
        output.status.success().then(|| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
    }
}

#[cfg(target_os = "macos")]
fn read_macos_process_args(pid: u32) -> Option<Vec<String>> {
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROCARGS2,
        i32::try_from(pid).ok()?,
    ];
    let mut size = 0usize;
    // SAFETY: mib and the size out-pointer are valid. A null output buffer is
    // the documented sizing query for sysctl.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || size < std::mem::size_of::<i32>()
    {
        return None;
    }
    let mut bytes = vec![0u8; size];
    // SAFETY: bytes has `size` writable bytes and sysctl updates size to the
    // number of bytes actually written.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    bytes.truncate(size);
    parse_macos_procargs2(&bytes)
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_procargs2(bytes: &[u8]) -> Option<Vec<String>> {
    let argc_bytes: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    let argc = usize::try_from(i32::from_ne_bytes(argc_bytes)).ok()?;
    if argc == 0 {
        return None;
    }
    let mut cursor = 4 + bytes.get(4..)?.iter().position(|byte| *byte == 0)?;
    while bytes.get(cursor) == Some(&0) {
        cursor += 1;
    }
    let mut args = Vec::with_capacity(argc);
    while args.len() < argc && cursor < bytes.len() {
        let remainder = bytes.get(cursor..)?;
        let end = remainder.iter().position(|byte| *byte == 0)?;
        args.push(String::from_utf8_lossy(&remainder[..end]).into_owned());
        cursor += end + 1;
        while bytes.get(cursor) == Some(&0) {
            cursor += 1;
        }
    }
    (args.len() == argc).then_some(args)
}

#[cfg(any(not(windows), test))]
fn is_cloudflared_process_args(
    args: &[String],
    token: &str,
    executable: &Path,
    token_file: &Path,
) -> bool {
    args.first().is_some_and(|value| {
        let actual = fs::canonicalize(value).unwrap_or_else(|_| PathBuf::from(value));
        let expected = fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
        actual == expected
    }) && !token.is_empty()
        && args.iter().any(|value| value == "tunnel")
        && args.iter().any(|value| value == "run")
        && (args
            .windows(2)
            .any(|pair| pair[0] == "--token" && pair[1] == token)
            || args
                .windows(2)
                .any(|pair| pair[0] == "--token-file" && Path::new(&pair[1]) == token_file))
}

fn redact_cloudflared_line(line: &str, token: &str) -> String {
    if token.is_empty() {
        return line.to_string();
    }
    line.replace(token, "[REDACTED]")
}

fn normalize_tunnel_event_message(line: &str) -> Option<String> {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= 240 {
        return Some(normalized);
    }
    let truncated = normalized.chars().take(240).collect::<String>();
    Some(format!("{}...", truncated.trim()))
}

fn parse_log_limit(value: Option<&str>, fallback: usize, max: usize) -> usize {
    let parsed = value
        .and_then(parse_node_parse_int)
        .unwrap_or(fallback as i64);
    parsed.clamp(1, max as i64) as usize
}

use crate::node_compat::parse_i64_prefix_trim_start as parse_node_parse_int;

async fn should_resume_tunnel(state: &AppState) -> crate::storage::StorageResult<bool> {
    let runtime = tunnel_runtime_state(state).await?;
    Ok(runtime
        .get("cloudflared_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

async fn mark_tunnel_running(state: &AppState) -> Result<(), String> {
    let _guard = state.tunnel.runtime_update_lock.lock().await;
    let mut runtime = tunnel_runtime_state(state)
        .await
        .map_err(|error| error.to_string())?;
    let object = runtime
        .as_object_mut()
        .ok_or_else(|| "invalid tunnel runtime state".to_string())?;
    object.insert("cloudflared_enabled".to_string(), Value::Bool(true));
    object.insert(
        "last_tunnel".to_string(),
        Value::String("cloudflared".to_string()),
    );
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    state
        .storage
        .store
        .set_json_value(TUNNEL_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())
}

async fn mark_tunnel_stopped(state: &AppState) -> Result<(), String> {
    let _guard = state.tunnel.runtime_update_lock.lock().await;
    let mut runtime = tunnel_runtime_state(state)
        .await
        .map_err(|error| error.to_string())?;
    let object = runtime
        .as_object_mut()
        .ok_or_else(|| "invalid tunnel runtime state".to_string())?;
    object.insert("cloudflared_enabled".to_string(), Value::Bool(false));
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    state
        .storage
        .store
        .set_json_value(TUNNEL_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())
}

async fn tunnel_runtime_state(state: &AppState) -> crate::storage::StorageResult<Value> {
    let raw = state
        .storage
        .store
        .get_json_value(TUNNEL_RUNTIME_KEY)
        .await?;
    Ok(normalize_tunnel_runtime_state(raw))
}

fn normalize_tunnel_runtime_state(value: Option<Value>) -> Value {
    let Some(value) = value else {
        return default_tunnel_runtime_state();
    };
    let Some(raw) = value.as_object().cloned() else {
        return default_tunnel_runtime_state();
    };
    let updated_at = raw
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(time_utils::now_iso);
    if !raw.contains_key("frp_enabled")
        && !raw.contains_key("cloudflared_enabled")
        && raw.contains_key("tunnel")
        && raw.contains_key("enabled")
    {
        let tunnel = raw.get("tunnel").and_then(Value::as_str).unwrap_or("frp");
        let enabled = raw.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        return json!({
            "frp_enabled": tunnel == "frp" && enabled,
            "cloudflared_enabled": tunnel == "cloudflared" && enabled,
            "last_tunnel": if tunnel == "cloudflared" { "cloudflared" } else { "frp" },
            "updated_at": updated_at
        });
    }
    json!({
        "frp_enabled": raw.get("frp_enabled").and_then(Value::as_bool).unwrap_or(false),
        "cloudflared_enabled": raw.get("cloudflared_enabled").and_then(Value::as_bool).unwrap_or(false),
        "last_tunnel": if raw.get("last_tunnel").and_then(Value::as_str) == Some("cloudflared") { "cloudflared" } else { "frp" },
        "updated_at": raw.get("updated_at").and_then(Value::as_str).map(str::to_string).unwrap_or_else(time_utils::now_iso)
    })
}

fn default_tunnel_runtime_state() -> Value {
    json!({
        "frp_enabled": false,
        "cloudflared_enabled": false,
        "last_tunnel": "frp",
        "updated_at": "1970-01-01T00:00:00.000Z"
    })
}

fn normalize_protocol(value: Option<&str>) -> String {
    match value.unwrap_or("auto") {
        "http2" => "http2".to_string(),
        "quic" => "quic".to_string(),
        _ => "auto".to_string(),
    }
}

fn build_args(config: &CloudflaredConfig, token_file: &Path) -> Vec<String> {
    let mut args = vec!["tunnel".to_string(), "--no-autoupdate".to_string()];
    if config.protocol != "auto" {
        args.push("--protocol".to_string());
        args.push(config.protocol.clone());
    }
    args.push("run".to_string());
    args.push("--token-file".to_string());
    args.push(token_file.to_string_lossy().to_string());
    args
}

fn cloudflared_token_configured(token: &str) -> bool {
    !token.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cloudflared_protocol() {
        assert_eq!(normalize_protocol(Some("http2")), "http2");
        assert_eq!(normalize_protocol(Some("bad")), "auto");
    }

    #[test]
    fn builds_cloudflared_args() {
        let args = build_args(
            &CloudflaredConfig {
                token: "tok".to_string(),
                protocol: "quic".to_string(),
            },
            Path::new("/run/fn-knock/tunnel-token"),
        );
        assert_eq!(
            args,
            vec![
                "tunnel",
                "--no-autoupdate",
                "--protocol",
                "quic",
                "run",
                "--token-file",
                "/run/fn-knock/tunnel-token"
            ]
        );
    }

    #[test]
    fn requires_cloudflared_version_with_token_file_support() {
        assert!(cloudflared_version_supports_token_file(
            "cloudflared version 2025.4.0 (built 2025-04-01)"
        ));
        assert!(cloudflared_version_supports_token_file(
            "cloudflared version 2026.7.1"
        ));
        assert!(!cloudflared_version_supports_token_file(
            "cloudflared version 2025.3.2"
        ));
        assert!(!cloudflared_version_supports_token_file("unknown"));
    }

    #[test]
    fn preserves_the_pre_update_running_intent() {
        assert!(!should_resume_cloudflared_after_asset_update(false, false));
        assert!(should_resume_cloudflared_after_asset_update(false, true));
        assert!(should_resume_cloudflared_after_asset_update(true, false));
        assert!(should_resume_cloudflared_after_asset_update(true, true));
    }

    #[test]
    fn token_presence_matches_node_truthiness() {
        assert!(!cloudflared_token_configured(""));
        assert!(cloudflared_token_configured("   "));
    }

    #[test]
    fn normalizes_legacy_tunnel_runtime_state() {
        let state = normalize_tunnel_runtime_state(Some(json!({
            "tunnel": "cloudflared",
            "enabled": true,
            "updated_at": "2026-01-01T00:00:00Z"
        })));
        assert_eq!(state["cloudflared_enabled"], true);
        assert_eq!(state["last_tunnel"], "cloudflared");
    }

    #[test]
    fn tunnel_runtime_state_matches_node_legacy_boundaries() {
        let state = normalize_tunnel_runtime_state(None);
        assert_eq!(state["updated_at"], "1970-01-01T00:00:00.000Z");

        let partial_new_state = normalize_tunnel_runtime_state(Some(json!({
            "cloudflared_enabled": true,
            "tunnel": "frp",
            "enabled": true
        })));
        assert_eq!(partial_new_state["frp_enabled"], false);
        assert_eq!(partial_new_state["cloudflared_enabled"], true);
        assert_ne!(partial_new_state["updated_at"], "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn log_limit_parser_matches_node_parse_int_prefixes() {
        assert_eq!(parse_log_limit(None, 200, LOG_MAX_LEN), 200);
        assert_eq!(parse_log_limit(Some(""), 200, LOG_MAX_LEN), 200);
        assert_eq!(parse_log_limit(Some("10x"), 200, LOG_MAX_LEN), 10);
        assert_eq!(parse_log_limit(Some("0x10"), 200, LOG_MAX_LEN), 1);
        assert_eq!(parse_log_limit(Some("-5"), 200, LOG_MAX_LEN), 1);
        assert_eq!(parse_log_limit(Some("5000"), 200, LOG_MAX_LEN), 1000);
        assert_eq!(parse_log_limit(Some("abc"), 200, LOG_MAX_LEN), 200);
    }

    #[test]
    fn localizes_cloudflared_route_errors() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_cloudflared_error(&translator, "Cloudflared token is required"),
            "请先配置 Cloudflare Token"
        );
        assert_eq!(
            localize_cloudflared_error(&translator, "Cloudflared is not initialized"),
            "Cloudflared 未初始化"
        );
        assert_eq!(
            cloudflared_text(&translator, "logsPollFailed"),
            "轮询 Cloudflared 日志失败"
        );
    }

    #[test]
    fn validates_process_shape_without_exposing_the_token() {
        assert!(is_cloudflared_process_args(
            &[
                "/opt/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token".to_string(),
                "secret".to_string(),
            ],
            "secret",
            Path::new("/opt/cloudflared"),
            Path::new("/tmp/token")
        ));
        assert!(is_cloudflared_process_args(
            &[
                "/opt/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token-file".to_string(),
                "/tmp/token".to_string(),
            ],
            "secret",
            Path::new("/opt/cloudflared"),
            Path::new("/tmp/token")
        ));
        assert!(!is_cloudflared_process_args(
            &[
                "/opt/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token".to_string(),
                "other-secret".to_string(),
            ],
            "secret",
            Path::new("/opt/cloudflared"),
            Path::new("/tmp/token")
        ));
        assert!(!is_cloudflared_process_args(
            &[
                "/other/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token-file".to_string(),
                "/tmp/token".to_string(),
            ],
            "secret",
            Path::new("/opt/cloudflared"),
            Path::new("/tmp/token")
        ));
        assert!(!is_cloudflared_process_args(
            &[
                "/opt/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token-file".to_string(),
                "/tmp/token".to_string(),
            ],
            "",
            Path::new("/opt/cloudflared"),
            Path::new("/tmp/token")
        ));
        assert!(is_cloudflared_process_args(
            &[
                "/Library/Application Support/FnKnock/cloudflared".to_string(),
                "tunnel".to_string(),
                "run".to_string(),
                "--token-file".to_string(),
                "/Library/Application Support/FnKnock/token".to_string(),
            ],
            "secret",
            Path::new("/Library/Application Support/FnKnock/cloudflared"),
            Path::new("/Library/Application Support/FnKnock/token")
        ));
        assert_eq!(
            redact_cloudflared_line("failed token=secret", "secret"),
            "failed token=[REDACTED]"
        );
    }

    #[test]
    fn parses_macos_procargs_without_splitting_spaces() {
        let mut bytes = 4i32.to_ne_bytes().to_vec();
        bytes.extend_from_slice(b"/Library/Application Support/FnKnock/cloudflared\0\0");
        bytes.extend_from_slice(
            b"/Library/Application Support/FnKnock/cloudflared\0tunnel\0run\0--token-file\0",
        );
        assert_eq!(
            parse_macos_procargs2(&bytes).unwrap(),
            vec![
                "/Library/Application Support/FnKnock/cloudflared",
                "tunnel",
                "run",
                "--token-file",
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_live_macos_process_arguments_without_ps() {
        let args = read_macos_process_args(std::process::id()).unwrap();
        assert!(!args.is_empty());
    }

    #[test]
    fn deserializes_windows_pid_identity_records() {
        let json =
            serde_json::from_str::<CloudflaredPidRecord>(r#"{"pid":42,"creation_time":123456}"#)
                .unwrap();
        assert_eq!(json.pid, 42);
        assert_eq!(json.creation_time, Some(123456));
    }
}
