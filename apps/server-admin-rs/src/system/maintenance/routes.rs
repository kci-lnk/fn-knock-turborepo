use super::*;

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

pub(super) async fn clear_all_data(
    State(state): State<AppState>,
    Json(body): Json<ClearAllDataBody>,
) -> Response {
    let go_backend = state.go_backend.clone();
    clear_all_data_with_gateway_reset(state, body, move || async move {
        go_backend.reset_all_data().await
    })
    .await
}

pub(super) async fn clear_all_data_with_gateway_reset<F, Fut>(
    state: AppState,
    body: ClearAllDataBody,
    reset_gateway: F,
) -> Response
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let translator = Translator::from_state(&state).await;
    let _automatic_backup_guard = state.automatic_backup_lock.lock().await;
    if body.confirmation != maintenance_clear_text(&translator, "confirmPhrase") {
        return response::error(
            StatusCode::BAD_REQUEST,
            maintenance_clear_text(&translator, "confirmationMismatch"),
        );
    }

    if let Err(error) = reset_gateway().await {
        tracing::error!(%error, "failed to clear Go gateway data");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            maintenance_clear_text(&translator, "clearFailed"),
        );
    }

    match state.store.clear_all_keys().await {
        Ok(cleared_keys) => {
            state.automatic_backup_notify.notify_one();
            response::ok(json!({
                "cleared_keys": cleared_keys,
                "gateway_reset": true,
            }))
            .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "failed to clear all stored data");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_clear_text(&translator, "clearFailed"),
            )
        }
    }
}
