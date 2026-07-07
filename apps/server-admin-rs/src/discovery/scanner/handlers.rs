use super::*;

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

pub(super) async fn list_blacklist(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = parse_i64(query.page.as_deref(), 1);
    let limit = parse_i64(query.limit.as_deref(), 20);
    let search = query.search.as_deref().unwrap_or("");
    match state
        .redis
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

pub(super) async fn get_blacklist_record(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_scanner_blacklist_record(&ip).await {
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

pub(super) async fn delete_blacklist_record(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let ips = sanitize_scanner_ips([ip]);
    match state.redis.remove_scanner_blacklist(&ips).await {
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
    match state.redis.remove_scanner_blacklist(&ips).await {
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
