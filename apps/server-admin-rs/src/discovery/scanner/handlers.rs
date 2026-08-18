use super::*;
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_settings, update_settings))
        .routes(routes!(get_path_whitelist, update_path_whitelist))
        .routes(routes!(resolve_false_positive))
        .routes(routes!(list_blacklist, delete_blacklist))
        .routes(routes!(get_blacklist_record, delete_blacklist_record))
}

#[utoipa::path(
    get,
    path = "/api/admin/scanner/path-whitelist",
    tag = "scanner",
    operation_id = "get_api_admin_scanner_path_whitelist",
    responses((status = 200, description = "Scanner path whitelist"))
)]
pub(super) async fn get_path_whitelist(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match path_whitelist::load_scanner_path_whitelist(&state).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(error) => scanner_path_whitelist_error(&translator, error, "load"),
    }
}

#[utoipa::path(
    put,
    path = "/api/admin/scanner/path-whitelist",
    tag = "scanner",
    operation_id = "put_api_admin_scanner_path_whitelist",
    responses((status = 200, description = "Updated scanner path whitelist"))
)]
pub(super) async fn update_path_whitelist(
    State(state): State<AppState>,
    Json(body): Json<UpdateScannerPathWhitelistBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match path_whitelist::replace_scanner_path_whitelist(&state, body.paths).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(error) => scanner_path_whitelist_error(&translator, error, "update"),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/scanner/path-whitelist/false-positive",
    tag = "scanner",
    operation_id = "post_api_admin_scanner_path_whitelist_false_positive",
    responses((status = 200, description = "Allowed scanner false positive"))
)]
pub(super) async fn resolve_false_positive(
    State(state): State<AppState>,
    Json(body): Json<ScannerFalsePositiveBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match path_whitelist::resolve_scanner_false_positive(&state, &body.ip, &body.path).await {
        Ok(result) => response::ok(result).into_response(),
        Err(error) => scanner_path_whitelist_error(&translator, error, "false-positive"),
    }
}

fn scanner_path_whitelist_error(
    translator: &Translator,
    error: ScannerError,
    operation: &str,
) -> Response {
    match error {
        ScannerError::BadRequest(message) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(translator, &message),
        ),
        error => {
            tracing::warn!(%error, operation, "scanner path whitelist operation failed");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(translator, "pathWhitelistOperationFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/scanner/settings",
    tag = "scanner",
    operation_id = "get_api_admin_scanner_settings",
    responses((status = 200, description = "Scanner settings"))
)]
pub(super) async fn get_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_scanner_settings(&state).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load scanner settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "settingsLoadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/scanner/settings",
    tag = "scanner",
    operation_id = "post_api_admin_scanner_settings",
    responses((status = 200, description = "Updated scanner settings"))
)]
pub(super) async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<UpdateScannerSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match save_scanner_settings(&state, body).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(ScannerError::BadRequest(message)) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(&translator, &message),
        ),
        Err(ScannerError::Cidr(message)) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_cidr_error(&translator, &message),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to update scanner settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "settingsUpdateFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/scanner/blacklist",
    tag = "scanner",
    operation_id = "get_api_admin_scanner_blacklist",
    responses((status = 200, description = "Scanner blacklist page"))
)]
pub(super) async fn list_blacklist(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = parse_i64(query.page.as_deref(), 1);
    let limit = parse_i64(query.limit.as_deref(), 20);
    let search = query.search.as_deref().unwrap_or("");
    match state
        .storage
        .store
        .list_scanner_blacklist(page, limit, search)
        .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list scanner blacklist");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistLoadFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/scanner/blacklist/{ip}",
    tag = "scanner",
    operation_id = "get_api_admin_scanner_blacklist__ip_",
    responses((status = 200, description = "Scanner blacklist record"))
)]
pub(super) async fn get_blacklist_record(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_scanner_blacklist_record(&ip).await {
        Ok(Some(record)) => response::ok(record).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            scanner_text(&translator, "recordNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to load scanner blacklist record");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordLoadFailed"),
            )
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/scanner/blacklist/{ip}",
    tag = "scanner",
    operation_id = "delete_api_admin_scanner_blacklist__ip_",
    responses((status = 200, description = "Scanner blacklist record deleted"))
)]
pub(super) async fn delete_blacklist_record(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let ips = sanitize_scanner_ips([ip]);
    match state.storage.store.remove_scanner_blacklist(&ips).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete scanner blacklist record");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordDeleteFailed"),
            )
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/scanner/blacklist",
    tag = "scanner",
    operation_id = "delete_api_admin_scanner_blacklist",
    responses((status = 200, description = "Scanner blacklist records deleted"))
)]
pub(super) async fn delete_blacklist(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let ips = match parse_blacklist_delete_ips(&body) {
        Ok(ips) => ips,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_scanner_error(&translator, message),
            );
        }
    };
    if ips.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            scanner_text(&translator, "atLeastOneIpRequired"),
        );
    }
    match state.storage.store.remove_scanner_blacklist(&ips).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete scanner blacklist records");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordsDeleteFailed"),
            )
        }
    }
}
