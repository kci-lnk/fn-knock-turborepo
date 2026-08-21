use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use totp_rs::Secret;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    auth::mode::AuthLoginMode,
    auth::verify_totp_token,
    auth_mobility,
    i18n::Translator,
    ldap_auth::ldap_delete_bindings_by_totp,
    oidc_admin::oidc_delete_bindings_by_totp,
    response,
    state::AppState,
    store::{AuthAccount, AuthPasswordCredential, TotpCredential},
    time_utils, whitelist,
};

use super::{
    AuthCredentialSettingsBody, SessionCommentBody, TotpAccessScopesBody, TotpBindBody,
    TotpCommentBody, TotpImportBody, TotpSubdomainAccessBody,
    auth_mode::projected_auth_accounts,
    gateway::refresh_gateway_auth_runtime,
    sessions::{
        ensure_session_comment, hydrate_mobility_event_ip_locations,
        session_mobility_details_value, session_record_with_mobility,
        sync_session_whitelist_comments,
    },
    settings::{
        auth_credential_settings_from_config, ensure_object, is_allowed_auth_credential_setting,
        legacy_auto_add_whitelist_on_login, node_totp_bind_comment,
        normalize_auth_credential_settings, session_ip_mobility_settings_changed,
        stream_access_grant_settings_changed,
    },
    text::{admin_control_text, totp_import_error_message},
    transfer::{
        CredentialImportPlan, PasswordCredentialImportPlan, build_credential_import_plan,
        build_password_export_payload, build_totp_export_payload, percent_encode,
    },
};

/// Credential-settings endpoints are registered through their annotated
/// handlers so runtime and OpenAPI routes cannot drift.
pub(crate) fn auth_credential_settings_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_auth_credential_settings))
        .routes(routes!(update_auth_credential_settings))
}

/// Basic TOTP setup endpoints share their annotated runtime route definitions
/// with the OpenAPI document.
pub(crate) fn totp_bootstrap_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(totp_status))
        .routes(routes!(totp_setup))
        .routes(routes!(totp_bind))
}

/// TOTP lifecycle, transfer, and credential mutations use annotated routes so
/// their runtime behavior and OpenAPI operations have one source of truth.
pub(crate) fn totp_management_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(totp_delete))
        .routes(routes!(totp_update_access_scopes))
        .routes(routes!(totp_update_subdomain_access))
        .routes(routes!(totp_update_comment))
        .routes(routes!(passkey_delete))
        .routes(routes!(totp_export))
        .routes(routes!(totp_import))
        .routes(routes!(totp_passkeys))
}

/// Session-management routes are declared once for both the Axum runtime and
/// the generated OpenAPI document.
pub(crate) fn session_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(sessions_list))
        .routes(routes!(session_get))
        .routes(routes!(session_delete))
        .routes(routes!(session_update_comment))
        .routes(routes!(session_mobility_details))
}

#[utoipa::path(
    get,
    path = "/api/admin/config/auth_credential_settings",
    tag = "config",
    operation_id = "get_api_admin_config_auth_credential_settings",
    responses((status = 200, description = "Authentication credential settings"))
)]
pub(super) async fn get_auth_credential_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_config().await {
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

#[utoipa::path(
    post,
    path = "/api/admin/config/auth_credential_settings",
    tag = "config",
    operation_id = "post_api_admin_config_auth_credential_settings",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated authentication credential settings"))
)]
pub(super) async fn update_auth_credential_settings(
    State(state): State<AppState>,
    Json(body): Json<AuthCredentialSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.storage.store.get_config().await {
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
    let stream_access_grants_changed = stream_access_grant_settings_changed(&current, &normalized);
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
    if stream_access_grants_changed
        && let Err(error) =
            auth_mobility::reconcile_all_stream_access_grants(&state, &normalized).await
    {
        if session_ip_mobility_changed {
            let _ = auth_mobility::reconcile_session_ip_mobility_policy(
                &state,
                &normalized,
                &current,
                false,
            )
            .await;
        }
        tracing::warn!(%error, "failed to reconcile stream access grants before auth settings update");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authCredentialSettings.saveFailed"),
        );
    }
    ensure_object(&mut config).insert("auth_credential_settings".to_string(), normalized.clone());

    match state.storage.store.save_config(&config).await {
        Ok(()) => {
            state.request_auth_mobility_maintenance();
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
            if stream_access_grants_changed
                && let Err(rollback_error) =
                    auth_mobility::reconcile_all_stream_access_grants(&state, &current).await
            {
                tracing::warn!(
                    %rollback_error,
                    "failed to rollback stream access grants after auth settings save failure"
                );
            }
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.saveFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/totp/status",
    tag = "totp",
    operation_id = "get_api_admin_totp_status",
    responses((status = 200, description = "TOTP credential status"))
)]
pub(super) async fn totp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_totps().await {
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

#[utoipa::path(
    post,
    path = "/api/admin/totp/setup",
    tag = "totp",
    operation_id = "post_api_admin_totp_setup",
    responses((status = 200, description = "Generated TOTP setup secret"))
)]
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

#[utoipa::path(
    post,
    path = "/api/admin/totp/bind",
    tag = "totp",
    operation_id = "post_api_admin_totp_bind",
    request_body = TotpBindBody,
    responses((status = 200, description = "Bound TOTP credential"))
)]
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

    match state.storage.store.add_totp(credential).await {
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

#[utoipa::path(
    get,
    path = "/api/admin/totp/credentials/export",
    tag = "totp",
    operation_id = "get_api_admin_totp_credentials_export",
    responses((status = 200, description = "TOTP or password credential export"))
)]
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

#[utoipa::path(
    post,
    path = "/api/admin/totp/credentials/import",
    tag = "totp",
    operation_id = "post_api_admin_totp_credentials_import",
    responses((status = 200, description = "Credential import summary"))
)]
pub(super) async fn totp_import(
    State(state): State<AppState>,
    Json(body): Json<TotpImportBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let existing_totps = match state.storage.store.get_totps().await {
        Ok(credentials) => credentials,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing TOTP credentials for import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    };
    let existing_accounts = match state.storage.store.get_auth_accounts().await {
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
                if let Err(error) = state.storage.store.set_totps(&next).await {
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
    let mode = state.storage.store.get_auth_login_mode().await?;
    let exported_at = time_utils::now_iso();
    let timestamp = exported_at.replace([':', '.'], "-");
    if mode == AuthLoginMode::Password {
        let (_, accounts) = projected_auth_accounts(state).await?;
        let totps = state.storage.store.get_totps().await?;
        let mut password_credentials = Vec::new();
        for account in &accounts {
            if let Some(record) = state
                .storage
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

    let credentials = state.storage.store.get_totps().await?;
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
            .storage
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
            state.storage.store.set_totps(&next_totps).await?;
        }
        if !plan.accounts.is_empty() {
            let mut next_accounts = existing_accounts.clone();
            next_accounts.extend(plan.accounts.clone());
            state
                .storage
                .store
                .set_auth_accounts(&next_accounts)
                .await?;
        }
        for credential in &plan.password_credentials {
            state
                .storage
                .store
                .set_auth_password_credential(credential)
                .await?;
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
                    .storage
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
    if let Err(error) = state.storage.store.set_auth_accounts(accounts).await {
        tracing::warn!(%error, "failed to roll back auth accounts after credential import failure");
    }
    if let Err(error) = state.storage.store.set_totps(totps).await {
        tracing::warn!(%error, "failed to roll back TOTP credentials after credential import failure");
    }
    for (account_id, snapshot) in password_snapshots {
        let result = if let Some(record) = snapshot {
            state
                .storage
                .store
                .set_auth_password_credential(record)
                .await
        } else {
            state
                .storage
                .store
                .delete_auth_password_credential(account_id)
                .await
        };
        if let Err(error) = result {
            tracing::warn!(%error, %account_id, "failed to roll back auth password credential after import failure");
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/totp/{id}",
    tag = "totp",
    operation_id = "delete_api_admin_totp_by_id",
    params(("id" = String, Path, description = "TOTP credential identifier")),
    responses((status = 200, description = "Deleted TOTP credential"))
)]
pub(super) async fn totp_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.delete_totp(&id).await {
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
            if let Err(error) = ldap_delete_bindings_by_totp(&state, &id).await {
                tracing::warn!(%error, %id, "failed to delete LDAP bindings for deleted TOTP credential");
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
    let mut accounts = state.storage.store.get_auth_accounts().await?;
    let deleted_ids = auth_account_ids_for_deleted_totp(&accounts, totp_id);
    if deleted_ids.is_empty() {
        return Ok(());
    }
    accounts.retain(|account| {
        !deleted_ids
            .iter()
            .any(|deleted_id| deleted_id == account.id.as_str())
    });
    state.storage.store.set_auth_accounts(&accounts).await?;
    for account_id in deleted_ids {
        state
            .storage
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

#[utoipa::path(
    patch,
    path = "/api/admin/totp/{id}/access-scopes",
    tag = "totp",
    operation_id = "patch_api_admin_totp_by_id_access_scopes",
    params(("id" = String, Path, description = "TOTP credential identifier")),
    responses((status = 200, description = "Updated TOTP access scopes"))
)]
pub(super) async fn totp_update_access_scopes(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpAccessScopesBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .storage
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

#[utoipa::path(
    patch,
    path = "/api/admin/totp/{id}/subdomain-access",
    tag = "totp",
    operation_id = "patch_api_admin_totp_by_id_subdomain_access",
    params(("id" = String, Path, description = "TOTP credential identifier")),
    responses((status = 200, description = "Updated TOTP subdomain access"))
)]
pub(super) async fn totp_update_subdomain_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpSubdomainAccessBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous = match state.storage.store.get_totps().await {
        Ok(credentials) => credentials
            .into_iter()
            .find(|credential| credential.id == id),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load TOTP before subdomain access update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            );
        }
    };
    match state
        .storage
        .store
        .update_totp_subdomain_access(&id, body.subdomain_access)
        .await
    {
        Ok(Some(updated)) => {
            if updated.subdomain_access.get("mode").and_then(Value::as_str) == Some("custom")
                && let Err(error) =
                    auth_mobility::clear_auto_ip_grants_for_totp_credential(&state, &id).await
            {
                if let Some(previous) = previous.as_ref() {
                    rollback_totp_subdomain_access_update(&state, previous).await;
                }
                tracing::warn!(%error, %id, "failed to clear auto IP grants after TOTP subdomain access restriction");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.updateFailed"),
                );
            }
            if let Err(error) = auth_mobility::reconcile_stream_access_grants_for_totp_credential(
                &state,
                &id,
                &updated.subdomain_access,
            )
            .await
            {
                if let Some(previous) = previous.as_ref() {
                    rollback_totp_subdomain_access_update(&state, previous).await;
                }
                tracing::warn!(%error, %id, "failed to reconcile stream access grants after TOTP permission update");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.updateFailed"),
                );
            }
            if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                if let Some(previous) = previous.as_ref() {
                    rollback_totp_subdomain_access_update(&state, previous).await;
                    if let Err(rollback_error) = refresh_gateway_auth_runtime(&state).await {
                        tracing::warn!(%rollback_error, %id, "failed to refresh auth gateway runtime after TOTP permission rollback");
                    }
                }
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

async fn rollback_totp_subdomain_access_update(state: &AppState, previous: &TotpCredential) {
    if let Err(error) = state
        .storage
        .store
        .update_totp_subdomain_access(&previous.id, previous.subdomain_access.clone())
        .await
    {
        tracing::warn!(%error, id = %previous.id, "failed to roll back TOTP subdomain access");
        return;
    }
    if let Err(error) = auth_mobility::reconcile_stream_access_grants_for_totp_credential(
        state,
        &previous.id,
        &previous.subdomain_access,
    )
    .await
    {
        tracing::warn!(%error, id = %previous.id, "failed to roll back TOTP stream access grants");
    }
}

#[utoipa::path(
    patch,
    path = "/api/admin/totp/{id}/comment",
    tag = "totp",
    operation_id = "patch_api_admin_totp_by_id_comment",
    request_body = TotpCommentBody,
    params(("id" = String, Path, description = "TOTP credential identifier")),
    responses((status = 200, description = "Updated TOTP comment"))
)]
pub(super) async fn totp_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .storage
        .store
        .update_totp_comment(&id, body.comment)
        .await
    {
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

#[utoipa::path(
    get,
    path = "/api/admin/totp/{totp_id}/passkeys",
    tag = "totp",
    operation_id = "get_api_admin_totp_by_totp_id_passkeys",
    params(("totp_id" = String, Path, description = "TOTP credential identifier")),
    responses((status = 200, description = "Passkeys associated with the TOTP credential"))
)]
pub(super) async fn totp_passkeys(
    State(state): State<AppState>,
    Path(totp_id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_passkeys().await {
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

#[utoipa::path(
    delete,
    path = "/api/admin/passkeys/{id}",
    tag = "passkeys",
    operation_id = "delete_api_admin_passkeys_by_id",
    params(("id" = String, Path, description = "Passkey credential identifier")),
    responses((status = 200, description = "Deleted passkey credential"))
)]
pub(super) async fn passkey_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.delete_passkey(&id).await {
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

#[utoipa::path(
    get,
    path = "/api/admin/sessions",
    tag = "sessions",
    operation_id = "get_api_admin_sessions",
    responses((status = 200, description = "Authentication sessions"))
)]
pub(super) async fn sessions_list(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.list_session_values().await {
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

#[utoipa::path(
    get,
    path = "/api/admin/sessions/{id}",
    tag = "sessions",
    operation_id = "get_api_admin_sessions_by_id",
    params(("id" = String, Path, description = "Session identifier")),
    responses((status = 200, description = "Authentication session"))
)]
pub(super) async fn session_get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_session_value(&id).await {
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

#[utoipa::path(
    patch,
    path = "/api/admin/sessions/{id}/comment",
    tag = "sessions",
    operation_id = "patch_api_admin_sessions_by_id_comment",
    request_body = SessionCommentBody,
    params(("id" = String, Path, description = "Session identifier")),
    responses((status = 200, description = "Updated authentication session"))
)]
pub(super) async fn session_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SessionCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut updates = Map::new();
    let comment = body.comment;
    updates.insert("comment".to_string(), Value::String(comment.clone()));
    match state.storage.store.update_session_value(&id, updates).await {
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

#[utoipa::path(
    delete,
    path = "/api/admin/sessions/{id}",
    tag = "sessions",
    operation_id = "delete_api_admin_sessions_by_id",
    params(("id" = String, Path, description = "Session identifier")),
    responses((status = 200, description = "Deleted authentication session"))
)]
pub(super) async fn session_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let outcome =
        auth_mobility::revoke_login_session(&state, &id, None, "", "admin_session_delete").await;
    if outcome.complete {
        response::success_message(admin_control_text(&translator, "sessions.deleted"))
            .into_response()
    } else {
        response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            admin_control_text(&translator, "sessions.deleteFailed"),
        )
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/sessions/{id}/mobility",
    tag = "sessions",
    operation_id = "get_api_admin_sessions_by_id_mobility",
    params(("id" = String, Path, description = "Session identifier")),
    responses((status = 200, description = "Session mobility details"))
)]
pub(super) async fn session_mobility_details(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_session_value(&id).await {
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
            subdomain_access: json!({ "mode": "all", "hosts": [], "streams": [] }),
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

    #[tokio::test]
    async fn admin_session_delete_reports_unconfirmed_gateway_trust_revocation() {
        let directory = tempfile::tempdir().expect("temporary admin session database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "admin-session-delete-trust-test".to_string();
        let state = AppState::new(settings)
            .await
            .expect("admin session test state");
        state
            .storage
            .store
            .save_config(&json!({}))
            .await
            .expect("admin session test config");

        let session_id = "admin-delete-session";
        let session_ip = "203.0.113.45";
        state
            .storage
            .store
            .add_session(
                session_id,
                &crate::store::LoginSession {
                    totp_id: "totp-1".to_string(),
                    method: "TOTP".to_string(),
                    credential_id: "totp-1".to_string(),
                    credential_name: "TOTP".to_string(),
                    linked_totp_name: None,
                    access_scopes: None,
                    subdomain_access: None,
                    grant_type: Some("browser_session".to_string()),
                    post_login_ip_grant_mode: None,
                    post_login_ip_grant_record_id: None,
                    stream_access_expires_at: None,
                    comment: None,
                    ip: session_ip.to_string(),
                    user_agent: "test".to_string(),
                    login_time: time_utils::now_iso(),
                    expires_at: Some(time_utils::iso_after_seconds(3600)),
                    ip_location: None,
                },
                3600,
            )
            .await
            .expect("store admin-deleted session");
        whitelist::sync_reverse_proxy_trusted_ips(&state).await;

        let before = state
            .storage
            .store
            .get_json_value("fn_knock:gateway:trusted-client-ips:runtime")
            .await
            .expect("read trusted runtime before delete")
            .expect("trusted runtime before delete");
        assert!(
            before["items"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["ip"] == session_ip))
        );

        let response = session_delete(State(state.clone()), Path(session_id.to_string())).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let after = state
            .storage
            .store
            .get_json_value("fn_knock:gateway:trusted-client-ips:runtime")
            .await
            .expect("read trusted runtime after delete")
            .expect("trusted runtime after delete");
        assert!(
            after["items"]
                .as_array()
                .is_some_and(|items| items.iter().all(|item| item["ip"] != session_ip))
        );
    }
}
