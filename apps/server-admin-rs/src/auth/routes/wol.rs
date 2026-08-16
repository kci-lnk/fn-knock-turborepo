use super::*;

pub(super) async fn targets(
    State(state): State<AppState>,
    verified: Option<Extension<crate::auth::hmac::VerifiedInternalRequest>>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = authorize(&state, &headers, verified.is_some()).await {
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
    verified: Option<Extension<crate::auth::hmac::VerifiedInternalRequest>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = authorize(&state, &headers, verified.is_some()).await {
        return response;
    }
    match crate::wol::service::wake_target(&state, &id, crate::wol::service::WakeSource::Portal)
        .await
    {
        Ok(value) => no_store(
            response::ok(json!({
                "targetId": id,
                "status": value.get("status").and_then(Value::as_str).unwrap_or("broadcasted"),
            }))
            .into_response(),
        ),
        Err(error) => {
            tracing::warn!(target_id = %id, status = %error.status, detail = %error.message, "WoL portal wake failed");
            let (status, message) = portal_wake_public_error(error.status);
            no_store(response::error(status, message))
        }
    }
}

pub(super) async fn shutdown(
    State(state): State<AppState>,
    verified: Option<Extension<crate::auth::hmac::VerifiedInternalRequest>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(response) = authorize(&state, &headers, verified.is_some()).await {
        return response;
    }
    match crate::wol::shutdown_target_for_portal(&state, &id).await {
        Ok(value) => no_store(
            response::ok(json!({
                "targetId": id,
                "status": value.get("status").and_then(Value::as_str).unwrap_or("accepted"),
            }))
            .into_response(),
        ),
        Err(error) => {
            tracing::warn!(target_id = %id, status = %error.status, "WoL portal shutdown failed");
            let (status, message) = portal_shutdown_public_error(error.status);
            no_store(response::error(status, message))
        }
    }
}

fn portal_wake_public_error(status: StatusCode) -> (StatusCode, &'static str) {
    match status {
        StatusCode::NOT_FOUND => (status, "Target was not found"),
        StatusCode::TOO_MANY_REQUESTS => {
            (status, "Target was woken recently; wait before retrying")
        }
        StatusCode::BAD_REQUEST => (status, "Target configuration is invalid"),
        StatusCode::CONFLICT => (status, "Target is unavailable"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to wake Target"),
    }
}

fn portal_shutdown_public_error(status: StatusCode) -> (StatusCode, &'static str) {
    match status {
        StatusCode::NOT_FOUND => (status, "Target was not found"),
        StatusCode::TOO_MANY_REQUESTS => (
            status,
            "Target shutdown was requested recently; wait before retrying",
        ),
        StatusCode::GATEWAY_TIMEOUT => (status, "Shutdown result is unknown"),
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT => {
            (StatusCode::CONFLICT, "Target is unavailable")
        }
        _ => (StatusCode::BAD_GATEWAY, "Failed to shut down Target"),
    }
}

async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    internal_request_verified: bool,
) -> Result<(), Response> {
    let config = state.storage.store.config_snapshot();
    let available = crate::wol::feature_enabled(config.as_ref())
        && config
            .get("gateway_portal")
            .and_then(|value| value.get("show_wol"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
    if !available {
        return Err(no_store(response::error(
            StatusCode::FORBIDDEN,
            "Wake-on-LAN portal is disabled",
        )));
    }

    let identity = inspect_auth_mobility_request(headers);
    let mut has_valid_session = false;
    if let Some(session_id) = identity.session_id.as_deref() {
        match state.storage.store.get_session(session_id).await {
            Ok(Some(session)) if !login_session_has_expired(&session) => {
                match session_can_use_wol(state, &session).await {
                    Ok(Some(true)) => return Ok(()),
                    Ok(Some(false)) => has_valid_session = true,
                    Ok(None) => {}
                    Err(response) => return Err(response),
                }
            }
            Ok(Some(session)) => {
                revoke_expired_presented_session(state, session_id, &session, config.as_ref())
                    .await;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to load login session for WoL portal API");
                return Err(no_store(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to validate the login session",
                )));
            }
        }
    }

    if internal_request_verified {
        let client_ip = client_ip_for_auth(headers);
        let sessions = match list_auth_mobility_owner_sessions_by_ip(state, &client_ip).await {
            Ok(sessions) => sessions,
            Err(error) => {
                tracing::warn!(%error, "failed to resolve WoL portal session by client IP");
                return Err(no_store(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to validate the login session",
                )));
            }
        };
        for (_, session) in sessions {
            if !auth_mobility_session_has_remaining_ttl(&session) {
                continue;
            }
            match session_can_use_wol(state, &session).await {
                Ok(Some(true)) => return Ok(()),
                Ok(Some(false)) => has_valid_session = true,
                Ok(None) => {}
                Err(response) => return Err(response),
            }
        }
    }

    if has_valid_session {
        return Err(no_store(response::error(
            StatusCode::FORBIDDEN,
            "No active session for this IP can use Wake-on-LAN",
        )));
    }

    Err(no_store(response::error(
        StatusCode::UNAUTHORIZED,
        "A valid login session or an authorized active session for this IP is required",
    )))
}

async fn session_can_use_wol(
    state: &AppState,
    session: &LoginSession,
) -> Result<Option<bool>, Response> {
    let credential = match session_auth_credential(state, session).await {
        Ok(Some(credential)) => credential,
        Ok(None) => return Ok(None),
        Err(error) => {
            tracing::warn!(%error, "failed to load login account for WoL portal API");
            return Err(no_store(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to validate the login account",
            )));
        }
    };
    Ok(Some(is_host_allowed_by_totp_subdomain_access(
        &credential.subdomain_access,
        TOTP_SUBDOMAIN_ACCESS_WOL_PAGE,
    )))
}

fn no_store(mut response: Response) -> Response {
    apply_no_store_headers(response.headers_mut());
    response
}

#[cfg(test)]
mod tests {
    use super::{portal_shutdown_public_error, portal_wake_public_error};
    use axum::http::StatusCode;

    #[test]
    fn portal_wake_errors_do_not_expose_relay_details() {
        assert_eq!(
            portal_wake_public_error(StatusCode::BAD_GATEWAY),
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to wake Target")
        );
        assert_eq!(
            portal_wake_public_error(StatusCode::GATEWAY_TIMEOUT),
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to wake Target")
        );
        assert!(
            !portal_wake_public_error(StatusCode::CONFLICT)
                .1
                .contains("Relay")
        );
    }

    #[test]
    fn portal_shutdown_errors_do_not_expose_ssh_details() {
        assert_eq!(
            portal_shutdown_public_error(StatusCode::GATEWAY_TIMEOUT),
            (StatusCode::GATEWAY_TIMEOUT, "Shutdown result is unknown")
        );
        assert_eq!(
            portal_shutdown_public_error(StatusCode::UNAUTHORIZED),
            (StatusCode::BAD_GATEWAY, "Failed to shut down Target")
        );
        assert!(
            !portal_shutdown_public_error(StatusCode::CONFLICT)
                .1
                .contains("SSH")
        );
    }
}
