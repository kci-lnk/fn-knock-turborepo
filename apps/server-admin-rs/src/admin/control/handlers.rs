use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use totp_rs::Secret;

use crate::{
    auth::verify_totp_token, auth_mobility, i18n::Translator,
    oidc_admin::oidc_delete_bindings_by_totp, response, state::AppState, store::TotpCredential,
    system_events, time_utils, whitelist,
};

use super::{
    AuthCredentialSettingsBody, SessionCommentBody, TotpAccessScopesBody, TotpBindBody,
    TotpCommentBody, TotpImportBody, TotpSubdomainAccessBody,
    gateway::refresh_gateway_auth_runtime,
    sessions::{
        ensure_session_comment, hydrate_mobility_event_ip_locations,
        revoke_custom_post_login_ip_grant_for_session, session_mobility_details_value,
        session_record_with_mobility, sync_session_whitelist_comments,
    },
    settings::{
        auth_credential_settings_from_config, ensure_object, is_allowed_auth_credential_setting,
        legacy_auto_add_whitelist_on_login, node_totp_bind_comment,
        normalize_auth_credential_settings, session_ip_mobility_settings_changed,
    },
    text::{admin_control_text, totp_import_error_message},
    transfer::{build_totp_export_payload, build_totp_import_plan, percent_encode},
};

pub(super) async fn get_auth_credential_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_config().await {
        Ok(config) => response::ok(auth_credential_settings_from_config(&config)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load auth credential settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.loadFailed"),
            )
        }
    }
}

pub(super) async fn update_auth_credential_settings(
    State(state): State<AppState>,
    Json(body): Json<AuthCredentialSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before auth settings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.loadConfigFailed"),
            );
        }
    };

    let current = auth_credential_settings_from_config(&config);
    let mut next = current.as_object().cloned().unwrap_or_default();
    for (key, value) in body.value {
        if is_allowed_auth_credential_setting(&key) {
            next.insert(key, value);
        }
    }
    let normalized = normalize_auth_credential_settings(
        Value::Object(next),
        legacy_auto_add_whitelist_on_login(&config),
    );
    let session_ip_mobility_changed = session_ip_mobility_settings_changed(&current, &normalized);
    if session_ip_mobility_changed
        && let Err(error) = auth_mobility::reconcile_session_ip_mobility_policy(
            &state,
            &current,
            &normalized,
            false,
        )
        .await
    {
        tracing::warn!(%error, "failed to reconcile session IP mobility before auth settings update");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authCredentialSettings.saveFailed"),
        );
    }
    ensure_object(&mut config).insert("auth_credential_settings".to_string(), normalized.clone());

    match state.store.save_config(&config).await {
        Ok(()) => {
            if session_ip_mobility_changed {
                whitelist::sync_reverse_proxy_trusted_ips(&state).await;
            }
            response::ok(normalized).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save auth credential settings");
            if session_ip_mobility_changed {
                if let Err(rollback_error) = auth_mobility::reconcile_session_ip_mobility_policy(
                    &state,
                    &normalized,
                    &current,
                    false,
                )
                .await
                {
                    tracing::warn!(
                        %rollback_error,
                        "failed to rollback session IP mobility reconciliation after auth settings save failure"
                    );
                } else {
                    whitelist::sync_reverse_proxy_trusted_ips(&state).await;
                }
            }
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.saveFailed"),
            )
        }
    }
}

pub(super) async fn totp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_totps().await {
        Ok(credentials) => response::ok(json!({
            "bound": !credentials.is_empty(),
            "credentials": credentials
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            )
        }
    }
}

pub(super) async fn totp_setup() -> Response {
    let secret = match Secret::generate_secret().to_encoded() {
        Secret::Encoded(value) => value,
        other => other.to_string(),
    };
    let label = percent_encode("fn-knock:admin");
    let issuer = percent_encode("fn-knock");
    response::ok(json!({
        "secret": secret,
        "uri": format!("otpauth://totp/{label}?secret={secret}&issuer={issuer}")
    }))
    .into_response()
}

pub(super) async fn totp_bind(
    State(state): State<AppState>,
    Json(body): Json<TotpBindBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match verify_totp_token(&body.secret, &body.token) {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, "totp.invalidCode"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to verify TOTP bind token");
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, "totp.invalidSecretOrCode"),
            );
        }
    }

    let credential = TotpCredential {
        id: hex::encode(rand::random::<[u8; 8]>()),
        secret: body.secret,
        comment: node_totp_bind_comment(body.comment),
        created_at: time_utils::now_iso(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    };

    match state.store.add_totp(credential).await {
        Ok(()) => {
            response::success_message(admin_control_text(&translator, "totp.bound")).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save TOTP credential");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.saveFailed"),
            )
        }
    }
}

pub(super) async fn totp_export(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_totps().await {
        Ok(credentials) => {
            let exported_at = time_utils::now_iso();
            let payload = build_totp_export_payload(&credentials, &exported_at);
            let filename = format!(
                "fn-knock-totp-credentials-{}.json",
                exported_at.replace([':', '.'], "-")
            );
            let body = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
            (
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json; charset=utf-8"),
                    ),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
                    ),
                ],
                body,
            )
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to export TOTP credentials");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.exportFailed"),
            )
        }
    }
}

pub(super) async fn totp_import(
    State(state): State<AppState>,
    Json(body): Json<TotpImportBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let existing = match state.store.get_totps().await {
        Ok(credentials) => credentials,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing TOTP credentials for import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    };

    let (credentials, summary) = match build_totp_import_plan(&existing, &body.payload) {
        Ok(plan) => plan,
        Err(error) => {
            return response::error(error.status, totp_import_error_message(&translator, &error));
        }
    };

    if !credentials.is_empty() {
        let mut next = existing;
        next.extend(credentials);
        if let Err(error) = state.store.set_totps(&next).await {
            tracing::warn!(%error, "failed to import TOTP credentials");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.importFailed"),
            );
        }
    }

    response::ok(summary).into_response()
}

pub(super) async fn totp_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.delete_totp(&id).await {
        Ok(true) => {
            if let Err(error) =
                auth_mobility::destroy_sessions_for_totp_credential(&state, &id).await
            {
                tracing::warn!(%error, %id, "failed to destroy sessions for deleted TOTP credential");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.deleteFailed"),
                );
            }
            if let Err(error) = oidc_delete_bindings_by_totp(&state, &id).await {
                tracing::warn!(%error, %id, "failed to delete OIDC bindings for deleted TOTP credential");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.deleteFailed"),
                );
            }
            if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                tracing::warn!(%error, %id, "failed to refresh auth gateway runtime after TOTP delete");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "hostMappings.syncAuthConfigFailed"),
                );
            }
            response::success_message(admin_control_text(&translator, "totp.deleted"))
                .into_response()
        }
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete TOTP credential");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.deleteFailed"),
            )
        }
    }
}

pub(super) async fn totp_update_access_scopes(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpAccessScopesBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .store
        .update_totp_access_scopes(&id, body.access_scopes)
        .await
    {
        Ok(Some(updated)) => response::ok(updated).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP access scopes");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

pub(super) async fn totp_update_subdomain_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpSubdomainAccessBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .store
        .update_totp_subdomain_access(&id, body.subdomain_access)
        .await
    {
        Ok(Some(updated)) => {
            if updated.subdomain_access.get("mode").and_then(Value::as_str) == Some("custom")
                && let Err(error) =
                    auth_mobility::clear_auto_ip_grants_for_totp_credential(&state, &id).await
            {
                tracing::warn!(%error, %id, "failed to clear auto IP grants after TOTP subdomain access restriction");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.updateFailed"),
                );
            }
            if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                tracing::warn!(%error, %id, "failed to refresh auth gateway runtime after TOTP subdomain access update");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "hostMappings.syncAuthConfigFailed"),
                );
            }
            response::ok(updated).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP subdomain access");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

pub(super) async fn totp_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.update_totp_comment(&id, body.comment).await {
        Ok(Some(_)) => response::success_message(admin_control_text(&translator, "totp.updated"))
            .into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP comment");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

pub(super) async fn totp_passkeys(
    State(state): State<AppState>,
    Path(totp_id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_passkeys().await {
        Ok(passkeys) => {
            let filtered = passkeys
                .into_iter()
                .filter(|passkey| passkey.get("totpId").and_then(Value::as_str) == Some(&totp_id))
                .collect::<Vec<_>>();
            response::ok(filtered).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, %totp_id, "failed to list TOTP passkeys");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "passkeys.listFailed"),
            )
        }
    }
}

pub(super) async fn passkey_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.delete_passkey(&id).await {
        Ok(true) => response::success_message(admin_control_text(&translator, "passkeys.deleted"))
            .into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "passkeys.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete passkey");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "passkeys.deleteFailed"),
            )
        }
    }
}

pub(super) async fn sessions_list(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.list_session_values().await {
        Ok(sessions) => {
            let mut records = Vec::with_capacity(sessions.len());
            for (id, data) in sessions {
                let data = ensure_session_comment(&state, &id, data, &translator).await;
                records.push(session_record_with_mobility(&state, id, data).await);
            }
            response::ok(records).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list auth sessions");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.listFailed"),
            )
        }
    }
}

pub(super) async fn session_get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_session_value(&id).await {
        Ok(Some(data)) => {
            let data = ensure_session_comment(&state, &id, data, &translator).await;
            response::ok(session_record_with_mobility(&state, id, data).await).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load auth session");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.loadFailed"),
            )
        }
    }
}

pub(super) async fn session_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SessionCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut updates = Map::new();
    let comment = body.comment;
    updates.insert("comment".to_string(), Value::String(comment.clone()));
    match state.store.update_session_value(&id, updates).await {
        Ok(Some(data)) => {
            if let Err(error) = sync_session_whitelist_comments(&state, &id, &data, &comment).await
            {
                tracing::warn!(%error, %id, "failed to sync session whitelist comments");
            }
            response::ok(session_record_with_mobility(&state, id, data).await).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update auth session comment");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.updateFailed"),
            )
        }
    }
}

pub(super) async fn session_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let session = state.store.get_session(&id).await.ok().flatten();
    let config = if session.is_some() {
        state.store.get_config().await.ok()
    } else {
        None
    };
    if let Some(session) = session.as_ref()
        && let Err(error) = system_events::publish_auth_logout_event(
            &state,
            json!({
                "session_id": id.clone(),
                "auth_method": session.method.clone(),
                "credential_id": session.credential_id.clone(),
                "credential_name": session.credential_name.clone(),
                "linked_totp_name": session.linked_totp_name.clone(),
                "session_comment": session.comment.clone(),
                "ip": session.ip.clone(),
                "ip_location": session.ip_location.clone(),
                "user_agent": session.user_agent.clone(),
                "login_time": session.login_time.clone(),
                "logout_source": "admin_session_delete",
            }),
        )
        .await
    {
        tracing::warn!(%error, %id, "failed to publish admin session delete logout event");
    }
    if let Err(error) = auth_mobility::destroy_session(&state, &id).await {
        tracing::warn!(%error, %id, "failed to cleanup auth mobility session during admin delete");
    }
    if let Some(session) = session.as_ref()
        && let Some(config) = config.as_ref()
    {
        match revoke_custom_post_login_ip_grant_for_session(&state, session, config).await {
            Ok(true) | Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, %id, "failed to revoke custom post-login IP grant");
            }
        }
    }
    match state.store.delete_session(&id).await {
        Ok(()) => response::success_message(admin_control_text(&translator, "sessions.deleted"))
            .into_response(),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete auth session");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.deleteFailed"),
            )
        }
    }
}

pub(super) async fn session_mobility_details(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.get_session_value(&id).await {
        Ok(Some(session)) => {
            let mut details = session_mobility_details_value(&state, &id, Some(&session)).await;
            hydrate_mobility_event_ip_locations(&state, &id, &mut details).await;
            response::ok(details).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load session mobility details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.mobilityLoadFailed"),
            )
        }
    }
}
