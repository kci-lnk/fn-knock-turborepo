use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use totp_rs::Secret;

use crate::{
    auth::mode::AuthLoginMode,
    auth::verify_totp_token,
    auth_mobility,
    i18n::Translator,
    oidc_admin::oidc_delete_bindings_by_totp,
    response,
    state::AppState,
    store::{AuthAccount, AuthPasswordCredential, TotpCredential},
    system_events, time_utils, whitelist,
};

use super::{
    AuthCredentialSettingsBody, SessionCommentBody, TotpAccessScopesBody, TotpBindBody,
    TotpCommentBody, TotpImportBody, TotpSubdomainAccessBody,
    auth_mode::projected_auth_accounts,
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
    transfer::{
        CredentialImportPlan, PasswordCredentialImportPlan, build_credential_import_plan,
        build_password_export_payload, build_totp_export_payload, percent_encode,
    },
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
    match build_current_auth_credentials_export(&state).await {
        Ok((payload, filename)) => {
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
            tracing::warn!(%error, "failed to export auth credentials");
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
    let existing_totps = match state.store.get_totps().await {
        Ok(credentials) => credentials,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing TOTP credentials for import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    };
    let existing_accounts = match state.store.get_auth_accounts().await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing auth accounts for credential import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let existing_password_account_ids = match existing_auth_password_account_ids(
        &state,
        &existing_accounts,
    )
    .await
    {
        Ok(ids) => ids,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing auth account passwords for credential import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };

    let plan = match build_credential_import_plan(
        &existing_totps,
        &existing_accounts,
        &existing_password_account_ids,
        &body.payload,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return response::error(error.status, totp_import_error_message(&translator, &error));
        }
    };

    match plan {
        CredentialImportPlan::Totp(plan) => {
            if !plan.credentials.is_empty() {
                let mut next = existing_totps;
                next.extend(plan.credentials);
                if let Err(error) = state.store.set_totps(&next).await {
                    tracing::warn!(%error, "failed to import TOTP credentials");
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        admin_control_text(&translator, "totp.importFailed"),
                    );
                }
                if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                    tracing::warn!(%error, "failed to refresh auth gateway runtime after TOTP import");
                }
            }
            response::ok(plan.summary).into_response()
        }
        CredentialImportPlan::Password(plan) => {
            let summary = plan.summary.clone();
            if let Err(error) =
                apply_password_credential_import(&state, existing_accounts, existing_totps, plan)
                    .await
            {
                tracing::warn!(%error, "failed to import password auth credentials");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.importFailed"),
                );
            }
            response::ok(summary).into_response()
        }
    }
}

async fn build_current_auth_credentials_export(
    state: &AppState,
) -> anyhow::Result<(Value, String)> {
    let mode = state.store.get_auth_login_mode().await?;
    let exported_at = time_utils::now_iso();
    let timestamp = exported_at.replace([':', '.'], "-");
    if mode == AuthLoginMode::Password {
        let (_, accounts) = projected_auth_accounts(state).await?;
        let totps = state.store.get_totps().await?;
        let mut password_credentials = Vec::new();
        for account in &accounts {
            if let Some(record) = state
                .store
                .get_auth_password_credential(&account.id)
                .await?
            {
                password_credentials.push(record);
            }
        }
        let payload =
            build_password_export_payload(&accounts, &password_credentials, &totps, &exported_at);
        return Ok((
            payload,
            format!("fn-knock-password-credentials-{timestamp}.json"),
        ));
    }

    let credentials = state.store.get_totps().await?;
    let payload = build_totp_export_payload(&credentials, &exported_at);
    Ok((
        payload,
        format!("fn-knock-totp-credentials-{timestamp}.json"),
    ))
}

async fn existing_auth_password_account_ids(
    state: &AppState,
    accounts: &[AuthAccount],
) -> anyhow::Result<HashSet<String>> {
    let mut ids = HashSet::new();
    for account in accounts {
        if state
            .store
            .get_auth_password_credential(&account.id)
            .await?
            .is_some()
        {
            ids.insert(account.id.clone());
        }
    }
    Ok(ids)
}

async fn apply_password_credential_import(
    state: &AppState,
    existing_accounts: Vec<AuthAccount>,
    existing_totps: Vec<TotpCredential>,
    plan: PasswordCredentialImportPlan,
) -> anyhow::Result<()> {
    let password_snapshots =
        password_credential_snapshots(state, &plan.password_credentials).await?;
    let result = async {
        if !plan.totp_credentials.is_empty() {
            let mut next_totps = existing_totps.clone();
            next_totps.extend(plan.totp_credentials.clone());
            state.store.set_totps(&next_totps).await?;
        }
        if !plan.accounts.is_empty() {
            let mut next_accounts = existing_accounts.clone();
            next_accounts.extend(plan.accounts.clone());
            state.store.set_auth_accounts(&next_accounts).await?;
        }
        for credential in &plan.password_credentials {
            state.store.set_auth_password_credential(credential).await?;
        }
        anyhow::Ok(())
    }
    .await;

    if let Err(error) = result {
        rollback_password_credential_import(
            state,
            &existing_accounts,
            &existing_totps,
            &password_snapshots,
        )
        .await;
        return Err(error);
    }
    if let Err(error) = refresh_gateway_auth_runtime(state).await {
        tracing::warn!(%error, "failed to refresh auth gateway runtime after password credential import");
    }
    Ok(())
}

async fn password_credential_snapshots(
    state: &AppState,
    credentials: &[AuthPasswordCredential],
) -> anyhow::Result<Vec<(String, Option<AuthPasswordCredential>)>> {
    let mut snapshots = Vec::with_capacity(credentials.len());
    let mut seen = HashSet::new();
    for credential in credentials {
        if seen.insert(credential.account_id.clone()) {
            snapshots.push((
                credential.account_id.clone(),
                state
                    .store
                    .get_auth_password_credential(&credential.account_id)
                    .await?,
            ));
        }
    }
    Ok(snapshots)
}

async fn rollback_password_credential_import(
    state: &AppState,
    accounts: &[AuthAccount],
    totps: &[TotpCredential],
    password_snapshots: &[(String, Option<AuthPasswordCredential>)],
) {
    if let Err(error) = state.store.set_auth_accounts(accounts).await {
        tracing::warn!(%error, "failed to roll back auth accounts after credential import failure");
    }
    if let Err(error) = state.store.set_totps(totps).await {
        tracing::warn!(%error, "failed to roll back TOTP credentials after credential import failure");
    }
    for (account_id, snapshot) in password_snapshots {
        let result = if let Some(record) = snapshot {
            state.store.set_auth_password_credential(record).await
        } else {
            state
                .store
                .delete_auth_password_credential(account_id)
                .await
        };
        if let Err(error) = result {
            tracing::warn!(%error, %account_id, "failed to roll back auth password credential after import failure");
        }
    }
}

pub(super) async fn totp_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.delete_totp(&id).await {
        Ok(true) => {
            if let Err(error) = delete_auth_accounts_for_totp(&state, &id).await {
                tracing::warn!(%error, %id, "failed to delete auth accounts for deleted TOTP credential");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.deleteFailed"),
                );
            }
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

async fn delete_auth_accounts_for_totp(state: &AppState, totp_id: &str) -> anyhow::Result<()> {
    let mut accounts = state.store.get_auth_accounts().await?;
    let deleted_ids = auth_account_ids_for_deleted_totp(&accounts, totp_id);
    if deleted_ids.is_empty() {
        return Ok(());
    }
    accounts.retain(|account| {
        !deleted_ids
            .iter()
            .any(|deleted_id| deleted_id == account.id.as_str())
    });
    state.store.set_auth_accounts(&accounts).await?;
    for account_id in deleted_ids {
        state
            .store
            .delete_auth_password_credential(&account_id)
            .await?;
        auth_mobility::destroy_sessions_for_auth_credential(state, &account_id).await?;
    }
    Ok(())
}

fn auth_account_ids_for_deleted_totp(accounts: &[AuthAccount], totp_id: &str) -> Vec<String> {
    accounts
        .iter()
        .filter(|account| account.source_totp_id == totp_id)
        .map(|account| account.id.clone())
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_account(id: &str, source_totp_id: &str) -> AuthAccount {
        AuthAccount {
            id: id.to_string(),
            username: id.to_string(),
            display_name: id.to_string(),
            source_totp_id: source_totp_id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: Value::Array(Vec::new()),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        }
    }

    #[test]
    fn deleted_totp_selects_linked_accounts_for_removal() {
        let accounts = vec![
            auth_account("alice", "totp-a"),
            auth_account("bob", "totp-b"),
        ];

        let deleted_ids = auth_account_ids_for_deleted_totp(&accounts, "totp-a");

        assert_eq!(deleted_ids, vec!["alice"]);
    }

    #[test]
    fn deleted_totp_removal_is_idempotent_when_source_is_missing() {
        let accounts = vec![auth_account("alice", "")];

        let deleted_ids = auth_account_ids_for_deleted_totp(&accounts, "totp-a");

        assert!(deleted_ids.is_empty());
    }
}
