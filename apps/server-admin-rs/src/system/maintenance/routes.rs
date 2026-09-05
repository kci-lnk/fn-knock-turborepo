use super::*;
use axum::extract::FromRequestParts;
use std::sync::Arc;
use utoipa_axum::{
    router::{OpenApiRouter, UtoipaMethodRouterExt},
    routes,
};

#[derive(Clone)]
pub(super) struct BackupAdmission {
    _guard: Arc<tokio::sync::OwnedMutexGuard<()>>,
}

impl BackupAdmission {
    pub(super) fn try_acquire(state: &AppState) -> Result<Self, BackupImportError> {
        state
            .maintenance
            .backup_request_lock
            .clone()
            .try_lock_owned()
            .map(|guard| Self {
                _guard: Arc::new(guard),
            })
            .map_err(|_| {
                BackupImportError::new(StatusCode::SERVICE_UNAVAILABLE, "Backup operation is busy")
            })
    }
}

impl FromRequestParts<AppState> for BackupAdmission {
    type Rejection = Response;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match Self::try_acquire(state) {
            Ok(admission) => Ok(admission),
            Err(error) => {
                // This rejection runs before Json consumes any upload bytes.
                let translator = Translator::from_state(state).await;
                Err(backup_operation_error_response(error, &translator))
            }
        }
    }
}

pub(super) struct BackupExportAdmission(pub(super) BackupAdmission);

impl BackupExportAdmission {
    pub(super) async fn acquire(state: &AppState) -> Result<Self, BackupImportError> {
        Self::acquire_with_timeout(state, std::time::Duration::from_secs(5)).await
    }

    pub(super) async fn acquire_with_timeout(
        state: &AppState,
        wait_timeout: std::time::Duration,
    ) -> Result<Self, BackupImportError> {
        let shutting_down = || {
            BackupImportError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Backup service is shutting down",
            )
        };
        if state.shutdown.is_cancelled() {
            return Err(shutting_down());
        }
        if let Ok(admission) = BackupAdmission::try_acquire(state) {
            return Ok(Self(admission));
        }
        let busy =
            || BackupImportError::new(StatusCode::SERVICE_UNAVAILABLE, "Backup operation is busy");
        // Only GET downloads may wait. Reserve a small waiting slot before
        // joining the existing FIFO mutex; no storage or background work has
        // started. Cancellation/timeout drops both queue positions by RAII.
        let _waiting = state
            .maintenance
            .backup_export_waiters
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy())?;
        let guard = tokio::select! {
            biased;
            _ = state.shutdown.cancelled() => return Err(shutting_down()),
            result = tokio::time::timeout(
                wait_timeout,
                state.maintenance.backup_request_lock.clone().lock_owned(),
            ) => result.map_err(|_| busy())?,
        };
        Ok(Self(BackupAdmission {
            _guard: Arc::new(guard),
        }))
    }
}

impl FromRequestParts<AppState> for BackupExportAdmission {
    type Rejection = Response;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match Self::acquire(state).await {
            Ok(admission) => Ok(admission),
            Err(error) => {
                let translator = Translator::from_state(state).await;
                Err(backup_operation_error_response(error, &translator))
            }
        }
    }
}

fn backup_operation_error_response(error: BackupImportError, translator: &Translator) -> Response {
    let mut response = response::error(
        error.status,
        localize_backup_error_message(translator, &error.message),
    );
    if error.status == StatusCode::SERVICE_UNAVAILABLE {
        response.headers_mut().insert(
            header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("1"),
        );
    }
    response
}

pub(super) async fn run_backup_operation<T, F>(
    state: &AppState,
    admission: BackupAdmission,
    work: F,
) -> Result<T, BackupImportError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, BackupImportError>> + Send + 'static,
{
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let task = state.spawn_abortable_background("manual-backup-operation", async move {
        // An HTTP disconnect must not interrupt a restore between storage
        // replacement and its migration/rollback/runtime synchronization.
        let _admission = admission;
        let result = work.await;
        if let Err(error) = &result {
            tracing::warn!(error = %error.message, "manual backup operation failed");
        }
        let _ = result_tx.send(result);
    });
    if task.is_none() {
        return Err(BackupImportError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Backup service is shutting down",
        ));
    }
    result_rx
        .await
        .map_err(|_| BackupImportError::internal("Backup operation could not complete"))?
}

/// Backup endpoints use annotated handlers so the executable Axum router and
/// the generated OpenAPI contract cannot diverge. The import body limit stays
/// attached to its runtime route because archives may be substantially larger
/// than Axum's default JSON limit.
pub(crate) fn backup_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_automatic_backup_details))
        .routes(routes!(update_automatic_backup_config))
        .routes(routes!(list_automatic_backup_files))
        .routes(routes!(export_backup))
        .routes(routes!(list_backup_files))
        .routes(routes!(export_backup_to_directory))
        .routes(routes!(import_backup).layer(DefaultBodyLimit::max(MAX_BACKUP_IMPORT_BODY_SIZE)))
        .routes(routes!(import_backup_from_automatic_directory))
        .routes(routes!(import_backup_from_directory))
}

/// Destructive maintenance endpoints are kept separate from backup routes so
/// their confirmation contract remains visible and reviewable.
pub(crate) fn maintenance_data_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(clear_all_data))
}

#[utoipa::path(
    get,
    path = "/api/admin/maintenance/backup/automatic",
    tag = "maintenance",
    operation_id = "get_api_admin_maintenance_backup_automatic",
    responses((status = 200, description = "Automatic backup configuration and status"))
)]
pub(super) async fn get_automatic_backup_details(State(state): State<AppState>) -> Response {
    match automatic_backup_details(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load automatic backup settings");
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_backup_text(&translator, "automaticSettingsReadFailed"),
            )
        }
    }
}

#[utoipa::path(
    put,
    path = "/api/admin/maintenance/backup/automatic",
    tag = "maintenance",
    operation_id = "put_api_admin_maintenance_backup_automatic",
    request_body = UpdateAutomaticBackupBody,
    responses((status = 200, description = "Updated automatic backup configuration"))
)]
pub(super) async fn update_automatic_backup_config(
    State(state): State<AppState>,
    body: Result<Json<UpdateAutomaticBackupBody>, JsonRejection>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Json(body) = match body {
        Ok(body) => body,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                maintenance_backup_text(&translator, "automaticSettingsInvalidRequest"),
            );
        }
    };
    match save_automatic_backup_config(&state, body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let message = if error.status == StatusCode::INTERNAL_SERVER_ERROR {
                maintenance_backup_text(&translator, "automaticSettingsSaveFailed")
            } else {
                localize_backup_error_message(&translator, &error.message)
            };
            response::error(error.status, message)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/maintenance/backup/automatic/files",
    tag = "maintenance",
    operation_id = "get_api_admin_maintenance_backup_automatic_files",
    responses((status = 200, description = "Automatic backup archives"))
)]
pub(super) async fn list_automatic_backup_files(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match automatic_backup_files_payload(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list automatic backup files");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_backup_text(&translator, "automaticDirectoryReadFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/maintenance/backup/export",
    tag = "maintenance",
    operation_id = "get_api_admin_maintenance_backup_export",
    responses((status = 200, description = "Backup archive download"), (status = 503, description = "备份任务正在进行，请根据 Retry-After 稍后重试", body = Value))
)]
pub(super) async fn export_backup(
    State(state): State<AppState>,
    admission: BackupExportAdmission,
) -> Response {
    let BackupExportAdmission(admission) = admission;
    let translator = Translator::from_state(&state).await;
    let work_state = state.clone();
    match run_backup_operation(&state, admission.clone(), async move {
        export_backup_archive(&work_state)
            .await
            .map_err(|error| BackupImportError::internal(error.to_string()))
    })
    .await
    {
        Ok(archive) => binary_archive_response(archive, admission, &translator),
        Err(error) => backup_operation_error_response(error, &translator),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/maintenance/backup/files",
    tag = "maintenance",
    operation_id = "get_api_admin_maintenance_backup_files",
    responses((status = 200, description = "Shared-directory backup archives"))
)]
pub(super) async fn list_backup_files(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match list_backup_directory_files().await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list backup directory files");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_backup_text(&translator, "readFnosDirectoryFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/export/fnos",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_export_fnos",
    responses((status = 200, description = "Exported backup archive to shared directory"), (status = 503, description = "备份任务正在进行，请根据 Retry-After 稍后重试", body = Value))
)]
pub(super) async fn export_backup_to_directory(
    State(state): State<AppState>,
    admission: BackupAdmission,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let work_state = state.clone();
    match run_backup_operation(&state, admission, async move {
        export_backup_archive_to_directory(&work_state).await
    })
    .await
    {
        Ok(data) => Json(json!({
            "success": true,
            "data": data,
            "message": admin_backup_text(&translator, "exportFnosSuccess"),
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(error = %error.message, "failed to export backup archive to share directory");
            backup_operation_error_response(error, &translator)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import",
    request_body = ImportBackupBody,
    responses((status = 200, description = "Backup import result"), (status = 503, description = "备份任务正在进行，请根据 Retry-After 稍后重试", body = Value))
)]
pub(super) async fn import_backup(
    State(state): State<AppState>,
    admission: BackupAdmission,
    Json(body): Json<ImportBackupBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let work_state = state.clone();
    let work_translator = translator.clone();
    match run_backup_operation(&state, admission, async move {
        import_backup_archive(&work_state, body, &work_translator).await
    })
    .await
    {
        Ok(data) => import_success_response(data, false, &translator),
        Err(error) => backup_operation_error_response(error, &translator),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import/fnos",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import_fnos",
    request_body = ImportBackupFromDirectoryBody,
    responses((status = 200, description = "Shared-directory backup import result"), (status = 503, description = "备份任务正在进行，请根据 Retry-After 稍后重试", body = Value))
)]
pub(super) async fn import_backup_from_directory(
    State(state): State<AppState>,
    admission: BackupAdmission,
    Json(body): Json<ImportBackupFromDirectoryBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let work_state = state.clone();
    let work_translator = translator.clone();
    match run_backup_operation(&state, admission, async move {
        import_backup_archive_from_directory(&work_state, &body.path, &work_translator).await
    })
    .await
    {
        Ok(data) => import_success_response(data, true, &translator),
        Err(error) => backup_operation_error_response(error, &translator),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import/automatic",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import_automatic",
    request_body = ImportBackupFromDirectoryBody,
    responses((status = 200, description = "Automatic backup import result"), (status = 503, description = "备份任务正在进行，请根据 Retry-After 稍后重试", body = Value))
)]
pub(super) async fn import_backup_from_automatic_directory(
    State(state): State<AppState>,
    admission: BackupAdmission,
    Json(body): Json<ImportBackupFromDirectoryBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let work_state = state.clone();
    let work_translator = translator.clone();
    match run_backup_operation(&state, admission, async move {
        import_backup_archive_from_automatic_directory(&work_state, &body.path, &work_translator)
            .await
    })
    .await
    {
        Ok(data) => import_success_response(data, false, &translator),
        Err(error) => backup_operation_error_response(error, &translator),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/data/clear",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_data_clear",
    responses((status = 200, description = "Cleared maintenance data"))
)]
pub(super) async fn clear_all_data(
    State(state): State<AppState>,
    Json(body): Json<ClearAllDataBody>,
) -> Response {
    let go_backend = state.gateway.client.clone();
    let memory_state = state.clone();
    clear_all_data_with_gateway_reset(
        state,
        body,
        move || async move { go_backend.reset_all_data().await },
        move |settings| {
            let state = memory_state.clone();
            async move {
                gateway_settings::apply_gateway_memory_settings(&state, settings)
                    .await
                    .map(|_| ())
            }
        },
    )
    .await
}

pub(super) async fn clear_all_data_with_gateway_reset<F, Fut, M, MFut>(
    state: AppState,
    body: ClearAllDataBody,
    reset_gateway: F,
    mut apply_gateway_memory: M,
) -> Response
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
    M: FnMut(gateway_settings::GatewayMemorySettings) -> MFut,
    MFut: Future<Output = anyhow::Result<()>>,
{
    let translator = Translator::from_state(&state).await;
    let _automatic_backup_guard = state.maintenance.automatic_backup_lock.lock().await;
    if body.confirmation != maintenance_clear_text(&translator, "confirmPhrase") {
        return response::error(
            StatusCode::BAD_REQUEST,
            maintenance_clear_text(&translator, "confirmationMismatch"),
        );
    }

    if let Err(error) = cloudflared::cleanup_before_data_clear(&state).await {
        tracing::error!(%error, "failed to clean Cloudflare resources before clearing local data");
        return response::error(
            StatusCode::BAD_GATEWAY,
            format!("Failed to clean Cloudflare resources before clearing local data: {error}"),
        );
    }

    let _gateway_memory_guard = state.gateway.memory_update_lock.lock().await;
    let previous_memory_settings = match state.storage.store.get_config().await {
        Ok(config) => gateway_settings::gateway_memory_settings(&config),
        Err(error) => {
            tracing::error!(%error, "failed to load gateway memory settings before clearing data");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_clear_text(&translator, "clearFailed"),
            );
        }
    };
    if let Err(error) = reset_gateway().await {
        tracing::error!(%error, "failed to clear Go gateway data");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            maintenance_clear_text(&translator, "clearFailed"),
        );
    }

    match state.storage.store.clear_all_keys().await {
        Ok(cleared_keys) => {
            state.terminal.shutdown_all().await;
            let default_memory_settings = gateway_settings::GatewayMemorySettings {
                gc_percent: gateway_settings::DEFAULT_GATEWAY_GC_PERCENT,
                memory_limit_mib: None,
            };
            if let Err(error) = apply_gateway_memory(default_memory_settings).await {
                tracing::error!(%error, "failed to apply default gateway memory settings after clearing data");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    maintenance_clear_text(&translator, "clearFailed"),
                );
            }
            if let Err(error) = wol::clear_secrets_after_backup_restore(&state).await {
                tracing::error!(%error, "failed to clear WoL relay credentials after clearing data");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    maintenance_clear_text(&translator, "clearFailed"),
                );
            }
            if let Err(error) = terminal::clear_all_credentials(&state) {
                tracing::error!(%error, "failed to clear terminal credentials after clearing data");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    maintenance_clear_text(&translator, "clearFailed"),
                );
            }
            if let Err(error) = panel_sync::clear_all_credentials(&state) {
                tracing::error!(%error, "failed to clear panel sync credentials after clearing data");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    maintenance_clear_text(&translator, "clearFailed"),
                );
            }
            state.maintenance.automatic_backup_notify.notify_one();
            response::ok(json!({
                "cleared_keys": cleared_keys,
                "gateway_reset": true,
            }))
            .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to clear all stored data");
            if let Err(rollback_error) = apply_gateway_memory(previous_memory_settings).await {
                tracing::error!(
                    %rollback_error,
                    "failed to roll back gateway memory settings after storage clear failure"
                );
            }
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_clear_text(&translator, "clearFailed"),
            )
        }
    }
}
