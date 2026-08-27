use super::*;
use utoipa_axum::{
    router::{OpenApiRouter, UtoipaMethodRouterExt},
    routes,
};

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
    responses((status = 200, description = "Backup archive download"))
)]
pub(super) async fn export_backup(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match export_backup_archive(&state).await {
        Ok(archive) => binary_archive_response(archive, &translator),
        Err(error) => {
            tracing::warn!(%error, "failed to export backup archive");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                localize_backup_error_message(&translator, &error.to_string()),
            )
        }
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
    responses((status = 200, description = "Exported backup archive to shared directory"))
)]
pub(super) async fn export_backup_to_directory(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match export_backup_archive_to_directory(&state).await {
        Ok(data) => Json(json!({
            "success": true,
            "data": data,
            "message": admin_backup_text(&translator, "exportFnosSuccess"),
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(error = %error.message, "failed to export backup archive to share directory");
            response::error(
                error.status,
                localize_backup_error_message(&translator, &error.message),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import",
    request_body = ImportBackupBody,
    responses((status = 200, description = "Backup import result"))
)]
pub(super) async fn import_backup(
    State(state): State<AppState>,
    Json(body): Json<ImportBackupBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match import_backup_archive(&state, body, &translator).await {
        Ok(data) => import_success_response(data, false, &translator),
        Err(error) => response::error(
            error.status,
            localize_backup_error_message(&translator, &error.message),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import/fnos",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import_fnos",
    request_body = ImportBackupFromDirectoryBody,
    responses((status = 200, description = "Shared-directory backup import result"))
)]
pub(super) async fn import_backup_from_directory(
    State(state): State<AppState>,
    Json(body): Json<ImportBackupFromDirectoryBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match import_backup_archive_from_directory(&state, &body.path, &translator).await {
        Ok(data) => import_success_response(data, true, &translator),
        Err(error) => response::error(
            error.status,
            localize_backup_error_message(&translator, &error.message),
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/backup/import/automatic",
    tag = "maintenance",
    operation_id = "post_api_admin_maintenance_backup_import_automatic",
    request_body = ImportBackupFromDirectoryBody,
    responses((status = 200, description = "Automatic backup import result"))
)]
pub(super) async fn import_backup_from_automatic_directory(
    State(state): State<AppState>,
    Json(body): Json<ImportBackupFromDirectoryBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match import_backup_archive_from_automatic_directory(&state, &body.path, &translator).await {
        Ok(data) => import_success_response(data, false, &translator),
        Err(error) => response::error(
            error.status,
            localize_backup_error_message(&translator, &error.message),
        ),
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
