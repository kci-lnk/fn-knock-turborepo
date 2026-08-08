use super::*;

pub(super) async fn targets(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = authorize(&state, &headers).await {
        return response;
    }
    match crate::wol::service::list_auth_targets(&state).await {
        Ok(items) => no_store(
            response::ok(json!({
                "total": items.len(),
                "items": items,
            }))
            .into_response(),
        ),
        Err(error) => no_store(response::error(error.status, error.message)),
    }
}

pub(super) async fn wake(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = authorize(&state, &headers).await {
        return response;
    }
    match crate::wol::service::wake_target(&state, &id).await {
        Ok(value) => no_store(response::ok(value).into_response()),
        Err(error) => no_store(response::error(error.status, error.message)),
    }
}

async fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let config = state.store.config_snapshot();
    let available = crate::wol::feature_enabled(config.as_ref())
        && config
            .get("gateway_portal")
            .and_then(|value| value.get("show_wol"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
    if !available {
        return Err(no_store(response::error(
            StatusCode::FORBIDDEN,
            "Wake-on-LAN portal is disabled",
        )));
    }

    let identity = inspect_auth_mobility_request(headers);
    let Some(session_id) = identity.session_id.as_deref() else {
        return Err(no_store(response::error(
            StatusCode::UNAUTHORIZED,
            "A valid login session is required",
        )));
    };
    let session = match state.store.get_session(session_id).await {
        Ok(Some(session)) if !login_session_has_expired(&session) => session,
        Ok(Some(session)) => {
            revoke_expired_presented_session(state, session_id, &session, config.as_ref()).await;
            return Err(no_store(response::error(
                StatusCode::UNAUTHORIZED,
                "The login session has expired",
            )));
        }
        Ok(None) => {
            return Err(no_store(response::error(
                StatusCode::UNAUTHORIZED,
                "The login session is no longer valid",
            )));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load login session for WoL portal API");
            return Err(no_store(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate the login session",
            )));
        }
    };
    let credential = match session_auth_credential(state, &session).await {
        Ok(Some(credential)) => credential,
        Ok(None) => {
            return Err(no_store(response::error(
                StatusCode::UNAUTHORIZED,
                "The login account is no longer available",
            )));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load login account for WoL portal API");
            return Err(no_store(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate the login account",
            )));
        }
    };
    if !is_host_allowed_by_totp_subdomain_access(
        &credential.subdomain_access,
        TOTP_SUBDOMAIN_ACCESS_WOL_PAGE,
    ) {
        return Err(no_store(response::error(
            StatusCode::FORBIDDEN,
            "This account cannot use Wake-on-LAN",
        )));
    }
    Ok(())
}

fn no_store(mut response: Response) -> Response {
    apply_no_store_headers(response.headers_mut());
    response
}
