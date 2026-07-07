use super::*;

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
