use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use totp_rs::Secret;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    auth::mode::{AuthLoginMode, AuthMethod},
    auth::password::{make_auth_password_credential, validate_auth_password},
    auth::verify_totp_token,
    auth_mobility,
    crypto_utils::random_bytes,
    http_utils::url_encode_component as percent_encode,
    i18n::Translator,
    ldap_auth::ldap_delete_bindings_by_totp,
    oidc_admin::oidc_delete_bindings_by_totp,
    response,
    state::AppState,
    store::{AuthAccount, AuthAccountMutationSnapshot, AuthPasswordCredential, TotpCredential},
    time_utils,
};

use super::{
    AuthAccountAccessScopesBody, AuthAccountCreateBody, AuthAccountPasswordBody,
    AuthAccountPatchBody, AuthAccountSetupBody, AuthAccountSubdomainAccessBody, AuthLoginModeBody,
    TotpBindBody, gateway::refresh_gateway_auth_runtime, text::admin_control_text,
};

const AUTH_MODE_SWITCH_BLOCKED: &str = "auth mode switch has blocking issues";

/// Authentication-mode operations are registered through the same annotated
/// handlers that contribute their OpenAPI operations.
pub(crate) fn auth_mode_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(auth_login_mode_status))
        .routes(routes!(auth_login_mode_preview))
        .routes(routes!(auth_login_mode_switch))
}

/// Authentication-account operations are registered by the same annotated
/// handlers that define their OpenAPI entries.
pub(crate) fn auth_account_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(auth_accounts_list))
        .routes(routes!(auth_account_create))
        .routes(routes!(auth_account_update))
        .routes(routes!(auth_account_delete))
        .routes(routes!(auth_account_set_password))
        .routes(routes!(auth_account_setup))
        .routes(routes!(auth_account_totp_setup))
        .routes(routes!(auth_account_totp_bind))
        .routes(routes!(auth_account_update_access_scopes))
        .routes(routes!(auth_account_update_subdomain_access))
}

#[derive(Clone)]
struct AuthAccountRollbackSnapshot {
    accounts: Vec<AuthAccount>,
    totps: Vec<TotpCredential>,
    password: Option<AuthPasswordCredential>,
}

#[utoipa::path(
    get,
    path = "/api/admin/auth/mode",
    tag = "auth",
    operation_id = "get_api_admin_auth_mode",
    responses((status = 200, description = "Authentication login mode status"))
)]
pub(super) async fn auth_login_mode_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_auth_mode_status(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load auth login mode status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authMode.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/mode/preview",
    tag = "auth",
    operation_id = "post_api_admin_auth_mode_preview",
    request_body = AuthLoginModeBody,
    responses((status = 200, description = "Authentication login mode switch preview"))
)]
pub(super) async fn auth_login_mode_preview(
    State(state): State<AppState>,
    Json(body): Json<AuthLoginModeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(target_mode) = AuthLoginMode::from_api(&body.mode) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, "authMode.invalidMode"),
        );
    };
    match build_auth_mode_preview(&state, target_mode).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, %target_mode, "failed to preview auth login mode switch");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authMode.previewFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/mode/switch",
    tag = "auth",
    operation_id = "post_api_admin_auth_mode_switch",
    request_body = AuthLoginModeBody,
    responses((status = 200, description = "Updated authentication login mode"))
)]
pub(super) async fn auth_login_mode_switch(
    State(state): State<AppState>,
    Json(body): Json<AuthLoginModeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(target_mode) = AuthLoginMode::from_api(&body.mode) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, "authMode.invalidMode"),
        );
    };

    match switch_auth_login_mode(&state, target_mode).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) if error.to_string() == AUTH_MODE_SWITCH_BLOCKED => response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, "authMode.blockingIssues"),
        ),
        Err(error) => {
            tracing::warn!(%error, %target_mode, "failed to switch auth login mode");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authMode.switchFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/auth/accounts",
    tag = "auth",
    operation_id = "get_api_admin_auth_accounts",
    responses((status = 200, description = "Authentication accounts"))
)]
pub(super) async fn auth_accounts_list(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_accounts_payload(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list auth accounts");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/admin/auth/accounts/{id}",
    tag = "auth",
    operation_id = "patch_api_admin_auth_accounts_by_id",
    request_body = AuthAccountPatchBody,
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Updated authentication account"))
)]
pub(super) async fn auth_account_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AuthAccountPatchBody>,
) -> Response {
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    let translator = Translator::from_state(&state).await;
    let (stored_accounts, mut accounts) = match projected_auth_accounts(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load auth accounts before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let Some(index) = accounts.iter().position(|account| account.id == id) else {
        return response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "authAccounts.notFound"),
        );
    };

    if let Some(username) = body.username.as_deref() {
        let normalized = match validate_username(username) {
            Ok(value) => value,
            Err(key) => {
                return response::error(
                    StatusCode::BAD_REQUEST,
                    admin_control_text(&translator, key),
                );
            }
        };
        if accounts
            .iter()
            .any(|account| account.id != id && account.username.eq_ignore_ascii_case(&normalized))
        {
            return response::error(
                StatusCode::CONFLICT,
                admin_control_text(&translator, "authAccounts.usernameExists"),
            );
        }
        accounts[index].username = normalized;
        accounts[index].display_name = accounts[index].username.clone();
    }
    accounts[index].updated_at = time_utils::now_iso();

    let updated = accounts[index].clone();
    let rollback = match capture_auth_account_rollback(&state, &stored_accounts, &id).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to capture auth account rollback before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    if let Err(error) = persist_auth_accounts_projection(&state, &stored_accounts, &accounts).await
    {
        tracing::warn!(%error, account_id = %id, "failed to save auth account update");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authAccounts.saveFailed"),
        );
    }
    if let Err(error) = sync_account_to_source_totp(&state, &updated).await {
        rollback_auth_account_mutation(&state, &rollback, &id, "auth account update").await;
        tracing::warn!(%error, account_id = %id, "failed to sync auth account update to TOTP");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authAccounts.syncFailed"),
        );
    }
    match account_payload(&state, &updated).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to build updated auth account payload");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/accounts",
    tag = "auth",
    operation_id = "post_api_admin_auth_accounts",
    request_body = AuthAccountCreateBody,
    responses((status = 200, description = "Created authentication account"))
)]
pub(super) async fn auth_account_create(
    State(state): State<AppState>,
    Json(body): Json<AuthAccountCreateBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let username = match validate_username(&body.username) {
        Ok(value) => value,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, key),
            );
        }
    };
    if let Err(key) = validate_auth_password(&body.password) {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, auth_account_password_error_key(key)),
        );
    }

    let (_, projected_accounts) = match projected_auth_accounts(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load projected auth accounts before create");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    if projected_accounts
        .iter()
        .any(|account| account.username.eq_ignore_ascii_case(username.as_str()))
    {
        return response::error(
            StatusCode::CONFLICT,
            admin_control_text(&translator, "authAccounts.usernameExists"),
        );
    }

    let now = time_utils::now_iso();
    let account = AuthAccount {
        id: unique_auth_account_id(&projected_accounts),
        username: username.clone(),
        display_name: username.clone(),
        source_totp_id: String::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    };
    let record = match make_auth_password_credential(&account.id, &body.password, None).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %account.id, "failed to hash created auth account password");
            return crate::auth::password::password_hash_error_response(
                &error,
                admin_control_text(&translator, "authAccounts.passwordSaveFailed"),
            );
        }
    };
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    // Hashing can wait in the shared pool. Rebuild the projection afterwards,
    // then compare and install the account plus its credential atomically.
    // Retry only storage conflicts; reuse the completed hash on every attempt.
    let mut installed = false;
    for _ in 0..3 {
        let (stored_accounts, mut projected_accounts) = match projected_auth_accounts(&state).await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "failed to reload auth accounts after password hashing");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "authAccounts.loadFailed"),
                );
            }
        };
        if projected_accounts
            .iter()
            .any(|existing| existing.username.eq_ignore_ascii_case(username.as_str()))
        {
            return response::error(
                StatusCode::CONFLICT,
                admin_control_text(&translator, "authAccounts.usernameExists"),
            );
        }
        if projected_accounts
            .iter()
            .any(|existing| existing.id == account.id)
        {
            return response::error(
                StatusCode::CONFLICT,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
        projected_accounts.push(account.clone());
        match state
            .storage
            .store
            .compare_and_set_auth_accounts_with_password(
                &stored_accounts,
                &projected_accounts,
                &record,
            )
            .await
        {
            Ok(true) => {
                installed = true;
                break;
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, account_id = %account.id, "failed to atomically save created auth account and password");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "authAccounts.saveFailed"),
                );
            }
        }
    }
    if !installed {
        return response::error(
            StatusCode::CONFLICT,
            admin_control_text(&translator, "authAccounts.saveFailed"),
        );
    }
    match account_payload(&state, &account).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, account_id = %account.id, "failed to build created auth account payload");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/auth/accounts/{id}",
    tag = "auth",
    operation_id = "delete_api_admin_auth_accounts_by_id",
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Deleted authentication account"))
)]
pub(super) async fn auth_account_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match delete_auth_account_and_linked_totp(&state, &id).await {
        Ok(true) => {
            response::success_message(admin_control_text(&translator, "authAccounts.deleted"))
                .into_response()
        }
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "authAccounts.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to delete auth account");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.deleteFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/accounts/{id}/totp/setup",
    tag = "auth",
    operation_id = "post_api_admin_auth_accounts_by_id_totp_setup",
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Generated account TOTP setup secret"))
)]
pub(super) async fn auth_account_totp_setup(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let (_, accounts) = match projected_auth_accounts(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to load projected auth accounts before TOTP setup");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let Some(account) = accounts.iter().find(|account| account.id == id) else {
        return response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "authAccounts.notFound"),
        );
    };
    match account_has_usable_totp(&state, account).await {
        Ok(true) => {
            return response::error(
                StatusCode::CONFLICT,
                admin_control_text(&translator, "authAccounts.totpAlreadyBound"),
            );
        }
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to inspect auth account TOTP before setup");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    }

    let secret = match Secret::generate_secret().to_encoded() {
        Secret::Encoded(value) => value,
        other => other.to_string(),
    };
    let label = percent_encode(&format!("fn-knock:{}", account.username));
    let issuer = percent_encode("fn-knock");
    response::ok(json!({
        "secret": secret,
        "uri": format!("otpauth://totp/{label}?secret={secret}&issuer={issuer}")
    }))
    .into_response()
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/accounts/{id}/totp/bind",
    tag = "auth",
    operation_id = "post_api_admin_auth_accounts_by_id_totp_bind",
    request_body = TotpBindBody,
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Bound account TOTP credential"))
)]
pub(super) async fn auth_account_totp_bind(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
            tracing::warn!(%error, account_id = %id, "failed to verify auth account TOTP bind token");
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, "totp.invalidSecretOrCode"),
            );
        }
    }

    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    let mut totps = match state.storage.store.get_totps().await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to load TOTP credentials before auth account TOTP bind");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    };
    let (stored_accounts, mut accounts) = match projected_auth_accounts(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to load projected auth accounts before TOTP bind");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let Some(index) = accounts.iter().position(|account| account.id == id) else {
        return response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "authAccounts.notFound"),
        );
    };
    if account_has_usable_totp_in(&totps, &accounts[index]) {
        return response::error(
            StatusCode::CONFLICT,
            admin_control_text(&translator, "authAccounts.totpAlreadyBound"),
        );
    }

    let totp_id = unique_totp_id(&totps);
    let now = time_utils::now_iso();
    accounts[index].source_totp_id = totp_id.clone();
    accounts[index].updated_at = now.clone();
    let updated = accounts[index].clone();
    totps.push(TotpCredential {
        id: totp_id.clone(),
        secret: body.secret,
        comment: updated.username.clone(),
        created_at: now,
        access_scopes: crate::store::normalize_totp_access_scopes(updated.access_scopes.clone()),
        subdomain_access: crate::store::normalize_totp_subdomain_access(
            updated.subdomain_access.clone(),
        ),
    });

    if let Err(error) = state.storage.store.set_totps(&totps).await {
        tracing::warn!(%error, account_id = %id, "failed to save auth account TOTP credential");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "totp.saveFailed"),
        );
    }
    if let Err(error) = persist_auth_accounts_projection(&state, &stored_accounts, &accounts).await
    {
        let _ = state.storage.store.delete_totp(&totp_id).await;
        tracing::warn!(%error, account_id = %id, "failed to save auth account after TOTP bind");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authAccounts.saveFailed"),
        );
    }
    if let Err(error) = refresh_gateway_auth_runtime(&state).await {
        tracing::warn!(%error, account_id = %id, "failed to refresh gateway runtime after auth account TOTP bind");
    }

    match account_payload(&state, &updated).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to build auth account after TOTP bind");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/accounts/{id}/password",
    tag = "auth",
    operation_id = "post_api_admin_auth_accounts_by_id_password",
    request_body = AuthAccountPasswordBody,
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Updated authentication account password"))
)]
pub(super) async fn auth_account_set_password(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AuthAccountPasswordBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(key) = validate_auth_password(&body.password) {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, auth_account_password_error_key(key)),
        );
    }
    update_auth_account_password(&state, &id, None, |created_at| {
        make_auth_password_credential(&id, &body.password, created_at)
    })
    .await
}

#[utoipa::path(
    post,
    path = "/api/admin/auth/accounts/{id}/setup",
    tag = "auth",
    operation_id = "post_api_admin_auth_accounts_by_id_setup",
    request_body = AuthAccountSetupBody,
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Updated authentication account setup"))
)]
pub(super) async fn auth_account_setup(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AuthAccountSetupBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let username = match validate_username(&body.username) {
        Ok(value) => value,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, key),
            );
        }
    };
    if let Err(key) = validate_auth_password(&body.password) {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_control_text(&translator, auth_account_password_error_key(key)),
        );
    }

    update_auth_account_password(&state, &id, Some(&username), |created_at| {
        make_auth_password_credential(&id, &body.password, created_at)
    })
    .await
}

/// Hash outside the account mutation lock. Re-read the latest projection and
/// require that the target's account, password and linked TOTP still match the
/// original request before applying its changes. Other accounts are preserved.
async fn update_auth_account_password<F, Fut>(
    state: &AppState,
    id: &str,
    username: Option<&str>,
    hash: F,
) -> Response
where
    F: FnOnce(Option<String>) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<AuthPasswordCredential>>,
{
    update_auth_account_password_with_commit_hook(state, id, username, hash, std::future::ready(()))
        .await
}

async fn update_auth_account_password_with_commit_hook<F, Fut, AfterCommit>(
    state: &AppState,
    id: &str,
    username: Option<&str>,
    hash: F,
    after_commit: AfterCommit,
) -> Response
where
    F: FnOnce(Option<String>) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<AuthPasswordCredential>>,
    AfterCommit: std::future::Future<Output = ()> + Send + 'static,
{
    let translator = Translator::from_state(state).await;
    let failure = |status, key| response::error(status, admin_control_text(&translator, key));
    let initial = {
        let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
        match state
            .storage
            .store
            .get_auth_account_mutation_snapshot(id, None)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(%error, account_id = %id, "failed to load account before password hashing");
                return failure(StatusCode::INTERNAL_SERVER_ERROR, "authAccounts.loadFailed");
            }
        }
    };
    let initial_projection = project_totps_to_accounts(&initial.totps, &initial.accounts);
    let Some(original) = initial_projection.iter().find(|account| account.id == id) else {
        return failure(StatusCode::NOT_FOUND, "authAccounts.notFound");
    };
    let created_at = initial
        .password
        .as_ref()
        .map(|credential| credential.created_at.clone());
    let record = match hash(created_at).await {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to hash auth account password");
            return crate::auth::password::password_hash_error_response(
                &error,
                admin_control_text(&translator, "authAccounts.passwordSaveFailed"),
            );
        }
    };
    // Admission happens before spawning, so at most one completion task owns
    // this Store's account mutation lock. HTTP cancellation drops only the
    // response receiver; the registry owns commit, revocation and rollback.
    let guard = state
        .storage
        .store
        .auth_account_mutation_lock
        .clone()
        .lock_owned()
        .await;
    let mutation = AuthAccountPasswordMutation {
        id: id.to_string(),
        username: username.map(str::to_string),
        original: original.clone(),
        initial,
        record,
    };
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let task_state = state.clone();
    if state
        .background_tasks
        .spawn_abortable("auth-account-password-mutation", async move {
            let response =
                finish_auth_account_password_mutation(&task_state, mutation, guard, after_commit)
                    .await;
            let _ = result_tx.send(response);
        })
        .is_none()
    {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "authAccounts.passwordSaveFailed",
        );
    }
    result_rx.await.unwrap_or_else(|_| {
        failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authAccounts.passwordSaveFailed",
        )
    })
}

struct AuthAccountPasswordMutation {
    id: String,
    username: Option<String>,
    original: AuthAccount,
    initial: AuthAccountMutationSnapshot,
    record: AuthPasswordCredential,
}

async fn finish_auth_account_password_mutation(
    state: &AppState,
    mutation: AuthAccountPasswordMutation,
    _guard: tokio::sync::OwnedMutexGuard<()>,
    after_commit: impl std::future::Future<Output = ()>,
) -> Response {
    let AuthAccountPasswordMutation {
        id,
        username,
        original,
        initial,
        record,
    } = mutation;
    let id = id.as_str();
    let username = username.as_deref();
    let translator = Translator::from_state(state).await;
    let failure = |status, key| response::error(status, admin_control_text(&translator, key));
    for _ in 0..3 {
        let current = match state
            .storage
            .store
            .get_auth_account_mutation_snapshot(id, Some(&initial))
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(%error, account_id = %id, "failed to reload account after password hashing");
                return failure(StatusCode::INTERNAL_SERVER_ERROR, "authAccounts.loadFailed");
            }
        };
        let mut projected = project_totps_to_accounts(&current.totps, &current.accounts);
        let Some(index) = projected.iter().position(|account| account.id == id) else {
            return failure(StatusCode::NOT_FOUND, "authAccounts.notFound");
        };
        if !auth_account_password_target_unchanged(&initial, &current, &original) {
            return failure(StatusCode::CONFLICT, "authAccounts.saveFailed");
        }
        if let Some(username) = username {
            if projected
                .iter()
                .any(|account| account.id != id && account.username.eq_ignore_ascii_case(username))
            {
                return failure(StatusCode::CONFLICT, "authAccounts.usernameExists");
            }
            projected[index].username = username.to_string();
            projected[index].display_name = username.to_string();
            projected[index].updated_at = time_utils::now_iso();
        }
        let updated = projected[index].clone();
        let mut replacement = current.clone();
        replacement.accounts = projected;
        replacement.password = Some(record.clone());
        if username.is_some()
            && let Some(totp) = replacement
                .totps
                .iter_mut()
                .find(|totp| totp.id == updated.source_totp_id)
        {
            sync_totp_metadata_from_account(totp, &updated);
        }
        match state
            .storage
            .store
            .compare_and_set_auth_account_mutation(id, &current, &replacement)
            .await
        {
            Ok(false) => continue,
            Err(error) => {
                tracing::warn!(%error, account_id = %id, "failed to atomically save account password/setup");
                return failure(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "authAccounts.passwordSaveFailed",
                );
            }
            Ok(true) => {}
        }
        after_commit.await;
        if let Err(error) = auth_mobility::destroy_sessions_for_auth_credential(state, id).await {
            // Only undo this exact write. A concurrent Store or cancelled HTTP
            // operation must never have its subsequent changes overwritten.
            match state
                .storage
                .store
                .compare_and_set_auth_account_mutation(id, &replacement, &current)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    tracing::warn!(account_id = %id, "account password rollback skipped after concurrent change")
                }
                Err(rollback_error) => {
                    tracing::warn!(%rollback_error, account_id = %id, "failed to roll back account password/setup")
                }
            }
            tracing::warn!(%error, account_id = %id, "failed to revoke sessions after account password/setup");
            return failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authAccounts.passwordSaveFailed",
            );
        }
        return match account_payload(state, &updated).await {
            Ok(value) => response::ok(value).into_response(),
            Err(error) => {
                tracing::warn!(%error, account_id = %id, "failed to build account after password/setup");
                failure(StatusCode::INTERNAL_SERVER_ERROR, "authAccounts.loadFailed")
            }
        };
    }
    failure(StatusCode::CONFLICT, "authAccounts.saveFailed")
}

fn auth_account_password_target_unchanged(
    initial: &AuthAccountMutationSnapshot,
    current: &AuthAccountMutationSnapshot,
    original: &AuthAccount,
) -> bool {
    let account = |snapshot: &AuthAccountMutationSnapshot| {
        serde_json::to_value(
            snapshot
                .accounts
                .iter()
                .find(|account| account.id == original.id),
        )
        .ok()
    };
    let totp = |snapshot: &AuthAccountMutationSnapshot| {
        serde_json::to_value(
            snapshot
                .totps
                .iter()
                .find(|totp| totp.id == original.source_totp_id),
        )
        .ok()
    };
    account(initial) == account(current)
        && totp(initial) == totp(current)
        && serde_json::to_value(&initial.password).ok()
            == serde_json::to_value(&current.password).ok()
}

#[utoipa::path(
    patch,
    path = "/api/admin/auth/accounts/{id}/access-scopes",
    tag = "auth",
    operation_id = "patch_api_admin_auth_accounts_by_id_access_scopes",
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Updated account access scopes"))
)]
pub(super) async fn auth_account_update_access_scopes(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AuthAccountAccessScopesBody>,
) -> Response {
    update_account_permissions(&state, &id, Some(body.access_scopes), None).await
}

#[utoipa::path(
    patch,
    path = "/api/admin/auth/accounts/{id}/subdomain-access",
    tag = "auth",
    operation_id = "patch_api_admin_auth_accounts_by_id_subdomain_access",
    params(("id" = String, Path, description = "Authentication account identifier")),
    responses((status = 200, description = "Updated account subdomain access"))
)]
pub(super) async fn auth_account_update_subdomain_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AuthAccountSubdomainAccessBody>,
) -> Response {
    update_account_permissions(&state, &id, None, Some(body.subdomain_access)).await
}

async fn update_account_permissions(
    state: &AppState,
    id: &str,
    access_scopes: Option<Value>,
    subdomain_access: Option<Value>,
) -> Response {
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    let translator = Translator::from_state(state).await;
    let mut account = match state.storage.store.get_auth_account(id).await {
        Ok(Some(account)) => account,
        Ok(None) => {
            return response::error(
                StatusCode::NOT_FOUND,
                admin_control_text(&translator, "authAccounts.notFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to load auth account before permission update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let stored_accounts = match state.storage.store.get_auth_accounts().await {
        Ok(accounts) => accounts,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to load auth accounts before permission update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    let rollback = match capture_auth_account_rollback(state, &stored_accounts, id).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to capture auth account rollback before permission update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            );
        }
    };
    if let Some(access_scopes) = access_scopes {
        account.access_scopes = crate::store::normalize_totp_access_scopes(access_scopes);
    }
    let mut subdomain_access_updated = false;
    if let Some(subdomain_access) = subdomain_access {
        account.subdomain_access = crate::store::normalize_totp_subdomain_access(subdomain_access);
        subdomain_access_updated = true;
    }
    account.updated_at = time_utils::now_iso();
    let saved = match state.storage.store.save_auth_account(account.clone()).await {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to save auth account permission update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
    };
    if let Err(error) = sync_account_to_source_totp(state, &saved).await {
        rollback_auth_account_permission_mutation(state, &rollback, id, subdomain_access_updated)
            .await;
        tracing::warn!(%error, account_id = %id, "failed to sync auth account permissions to TOTP");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authAccounts.syncFailed"),
        );
    }
    if should_clear_auto_ip_grants_for_subdomain_update(subdomain_access_updated, &saved) {
        if let Err(error) =
            auth_mobility::clear_auto_ip_grants_for_auth_credential(state, &saved.id).await
        {
            rollback_auth_account_permission_mutation(
                state,
                &rollback,
                id,
                subdomain_access_updated,
            )
            .await;
            tracing::warn!(%error, account_id = %id, "failed to clear auto IP grants after auth account subdomain access restriction");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
        let source_totp_id = saved.source_totp_id.trim();
        if !source_totp_id.is_empty()
            && let Err(error) =
                auth_mobility::clear_auto_ip_grants_for_totp_credential(state, source_totp_id).await
        {
            rollback_auth_account_permission_mutation(
                state,
                &rollback,
                id,
                subdomain_access_updated,
            )
            .await;
            tracing::warn!(%error, account_id = %id, totp_id = %source_totp_id, "failed to clear TOTP auto IP grants after auth account subdomain access restriction");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
    }
    if subdomain_access_updated {
        if let Err(error) = auth_mobility::reconcile_stream_access_grants_for_auth_credential(
            state,
            &saved.id,
            &saved.subdomain_access,
        )
        .await
        {
            rollback_auth_account_permission_mutation(
                state,
                &rollback,
                id,
                subdomain_access_updated,
            )
            .await;
            tracing::warn!(%error, account_id = %id, "failed to reconcile account stream access grants");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
        let source_totp_id = saved.source_totp_id.trim();
        if !source_totp_id.is_empty()
            && let Err(error) = auth_mobility::reconcile_stream_access_grants_for_totp_credential(
                state,
                source_totp_id,
                &saved.subdomain_access,
            )
            .await
        {
            rollback_auth_account_permission_mutation(
                state,
                &rollback,
                id,
                subdomain_access_updated,
            )
            .await;
            tracing::warn!(%error, account_id = %id, totp_id = %source_totp_id, "failed to reconcile linked TOTP stream access grants");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.saveFailed"),
            );
        }
    }
    if let Err(error) = refresh_gateway_auth_runtime(state).await {
        rollback_auth_account_permission_mutation(state, &rollback, id, subdomain_access_updated)
            .await;
        tracing::warn!(%error, account_id = %id, "failed to refresh gateway after auth account permission update");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "hostMappings.syncAuthConfigFailed"),
        );
    }
    match account_payload(state, &saved).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, account_id = %id, "failed to build auth account permission response");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authAccounts.loadFailed"),
            )
        }
    }
}

fn should_clear_auto_ip_grants_for_subdomain_update(
    subdomain_access_updated: bool,
    account: &AuthAccount,
) -> bool {
    subdomain_access_updated
        && account.subdomain_access.get("mode").and_then(Value::as_str) == Some("custom")
}

async fn build_auth_mode_status(state: &AppState) -> anyhow::Result<Value> {
    let mode = state.storage.store.get_auth_login_mode().await?;
    let totps = state.storage.store.get_totps().await?;
    let stored_accounts = state.storage.store.get_auth_accounts().await?;
    let accounts = project_totps_to_accounts(&totps, &stored_accounts);
    let password_configured = count_configured_passwords(state, &accounts).await?;
    Ok(json!({
        "mode": mode.as_str(),
        "totpCount": totps.len(),
        "accountCount": accounts.len(),
        "passwordConfiguredCount": password_configured,
        "passwordMissingCount": accounts.len().saturating_sub(password_configured)
    }))
}

async fn build_auth_mode_preview(
    state: &AppState,
    target_mode: AuthLoginMode,
) -> anyhow::Result<Value> {
    let current_mode = state.storage.store.get_auth_login_mode().await?;
    let totps = state.storage.store.get_totps().await?;
    let (_, accounts) = projected_auth_accounts(state).await?;
    if target_mode == AuthLoginMode::Password {
        let projected = project_totps_to_accounts(&totps, &accounts);
        let created = projected
            .iter()
            .filter(|account| !accounts.iter().any(|item| item.id == account.id))
            .count();
        let updated = projected
            .iter()
            .filter(|projected| {
                accounts
                    .iter()
                    .find(|account| account.id == projected.id)
                    .is_some_and(|account| account_projection_fields_changed(account, projected))
            })
            .count();
        let password_configured = count_configured_passwords(state, &projected).await?;
        let password_missing = projected.len().saturating_sub(password_configured);
        let password_required_before_switch =
            password_required_before_switch(projected.len(), password_configured);
        return Ok(json!({
            "currentMode": current_mode.as_str(),
            "targetMode": target_mode.as_str(),
            "totpCount": totps.len(),
            "accountCount": projected.len(),
            "createAccountCount": created,
            "updateAccountCount": updated,
            "passwordConfiguredCount": password_configured,
            "passwordMissingCount": password_missing,
            "blockingIssueCount": if password_required_before_switch { password_missing } else { 0 },
            "passwordRequiredBeforeSwitch": password_required_before_switch
        }));
    }

    let totp_ids = totps
        .iter()
        .map(|credential| credential.id.as_str())
        .collect::<HashSet<_>>();
    let missing_source_totp = accounts
        .iter()
        .filter(|account| !totp_ids.contains(account.source_totp_id.as_str()))
        .count();
    Ok(json!({
        "currentMode": current_mode.as_str(),
        "targetMode": target_mode.as_str(),
        "totpCount": totps.len(),
        "accountCount": accounts.len(),
        "createAccountCount": 0,
        "updateAccountCount": accounts.len().saturating_sub(missing_source_totp),
        "passwordConfiguredCount": count_configured_passwords(state, &accounts).await?,
        "passwordMissingCount": 0,
        "blockingIssueCount": missing_source_totp,
        "missingSourceTotpCount": missing_source_totp
    }))
}

async fn switch_auth_login_mode(
    state: &AppState,
    target_mode: AuthLoginMode,
) -> anyhow::Result<Value> {
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    if target_mode == AuthLoginMode::Password {
        let (accounts, projected) = projected_auth_accounts(state).await?;
        let password_configured = count_configured_passwords(state, &projected).await?;
        if password_required_before_switch(projected.len(), password_configured) {
            anyhow::bail!(AUTH_MODE_SWITCH_BLOCKED);
        }
        persist_auth_accounts_projection(state, &accounts, &projected).await?;
        destroy_sessions_for_disabled_auth_mode(state, target_mode).await?;
        state
            .storage
            .store
            .set_auth_login_mode(AuthLoginMode::Password)
            .await?;
        // Close the race with a login that passed its mode check immediately
        // before the first revocation sweep.
        destroy_sessions_for_disabled_auth_mode(state, target_mode).await?;
    } else {
        let (_, accounts) = projected_auth_accounts(state).await?;
        let totp_ids = state
            .storage
            .store
            .get_totps()
            .await?
            .iter()
            .map(|credential| credential.id.clone())
            .collect::<HashSet<_>>();
        if accounts
            .iter()
            .any(|account| !totp_ids.contains(account.source_totp_id.as_str()))
        {
            anyhow::bail!(AUTH_MODE_SWITCH_BLOCKED);
        }
        apply_accounts_to_totps(state, &accounts).await?;
        destroy_sessions_for_disabled_auth_mode(state, target_mode).await?;
        state
            .storage
            .store
            .set_auth_login_mode(AuthLoginMode::Totp)
            .await?;
        destroy_sessions_for_disabled_auth_mode(state, target_mode).await?;
    }
    if let Err(error) = refresh_gateway_auth_runtime(state).await {
        tracing::warn!(
            %error,
            %target_mode,
            "failed to refresh gateway runtime after auth login mode switch"
        );
    }
    build_auth_mode_status(state).await
}

async fn destroy_sessions_for_disabled_auth_mode(
    state: &AppState,
    target_mode: AuthLoginMode,
) -> anyhow::Result<usize> {
    let disabled_methods: &[AuthMethod] = if target_mode == AuthLoginMode::Password {
        &[
            AuthMethod::Totp,
            AuthMethod::Passkey,
            AuthMethod::Oidc,
            AuthMethod::Ldap,
        ]
    } else {
        &[AuthMethod::Password]
    };
    let mut destroyed = 0usize;
    for method in disabled_methods {
        destroyed +=
            auth_mobility::destroy_sessions_for_auth_method(state, method.as_session_str()).await?;
    }
    Ok(destroyed)
}

async fn build_accounts_payload(state: &AppState) -> anyhow::Result<Value> {
    let (_, accounts) = projected_auth_accounts(state).await?;
    let mut items = Vec::with_capacity(accounts.len());
    for account in &accounts {
        items.push(account_payload(state, account).await?);
    }
    Ok(json!({ "accounts": items }))
}

async fn delete_auth_account_and_linked_totp(state: &AppState, id: &str) -> anyhow::Result<bool> {
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    let (stored_accounts, projected_accounts) = projected_auth_accounts(state).await?;
    let Some(target) = projected_accounts
        .iter()
        .find(|account| account.id == id)
        .cloned()
    else {
        return Ok(false);
    };

    let linked_totp_id = target.source_totp_id.trim().to_string();
    let remove_ids = account_ids_removed_by_account_delete(&projected_accounts, &target);
    let next_accounts = projected_accounts
        .into_iter()
        .filter(|account| !remove_ids.contains(account.id.as_str()))
        .collect::<Vec<_>>();

    persist_auth_accounts_projection(state, &stored_accounts, &next_accounts).await?;
    for account_id in &remove_ids {
        state
            .storage
            .store
            .delete_auth_password_credential(account_id)
            .await?;
        auth_mobility::destroy_sessions_for_auth_credential(state, account_id).await?;
    }

    if !linked_totp_id.is_empty() {
        let _ = state.storage.store.delete_totp(&linked_totp_id).await?;
        auth_mobility::destroy_sessions_for_totp_credential(state, &linked_totp_id).await?;
        oidc_delete_bindings_by_totp(state, &linked_totp_id).await?;
        ldap_delete_bindings_by_totp(state, &linked_totp_id).await?;
    }

    if let Err(error) = refresh_gateway_auth_runtime(state).await {
        tracing::warn!(
            %error,
            account_id = %id,
            "failed to refresh gateway runtime after auth account delete"
        );
    }
    Ok(true)
}

fn account_ids_removed_by_account_delete(
    accounts: &[AuthAccount],
    target: &AuthAccount,
) -> HashSet<String> {
    let mut remove_ids = HashSet::from([target.id.clone()]);
    let linked_totp_id = target.source_totp_id.trim();
    if !linked_totp_id.is_empty() {
        for account in accounts
            .iter()
            .filter(|account| account.source_totp_id == linked_totp_id)
        {
            remove_ids.insert(account.id.clone());
        }
    }
    remove_ids
}

async fn account_payload(state: &AppState, account: &AuthAccount) -> anyhow::Result<Value> {
    let password_configured = state
        .storage
        .store
        .get_auth_password_credential(&account.id)
        .await?
        .is_some();
    let source_totp = state
        .storage
        .store
        .get_totps()
        .await?
        .into_iter()
        .find(|totp| totp.id == account.source_totp_id);
    let totp_configured = source_totp.is_some();
    let source_totp_name = source_totp.map(|totp| totp.comment).unwrap_or_default();
    Ok(json!({
        "id": account.id,
        "username": account.username,
        "displayName": account.username,
        "sourceTotpId": account.source_totp_id,
        "sourceTotpName": source_totp_name,
        "createdAt": account.created_at,
        "updatedAt": account.updated_at,
        "access_scopes": account.access_scopes,
        "subdomain_access": account.subdomain_access,
        "passwordConfigured": password_configured,
        "totpConfigured": totp_configured
    }))
}

async fn capture_auth_account_rollback(
    state: &AppState,
    accounts: &[AuthAccount],
    account_id: &str,
) -> anyhow::Result<AuthAccountRollbackSnapshot> {
    Ok(AuthAccountRollbackSnapshot {
        accounts: accounts.to_vec(),
        totps: state.storage.store.get_totps().await?,
        password: state
            .storage
            .store
            .get_auth_password_credential(account_id)
            .await?,
    })
}

async fn rollback_auth_account_permission_mutation(
    state: &AppState,
    snapshot: &AuthAccountRollbackSnapshot,
    account_id: &str,
    subdomain_access_updated: bool,
) {
    rollback_auth_account_mutation(
        state,
        snapshot,
        account_id,
        "auth account permission update",
    )
    .await;
    if !subdomain_access_updated {
        return;
    }
    let Some(account) = snapshot
        .accounts
        .iter()
        .find(|account| account.id == account_id)
    else {
        return;
    };
    if let Err(error) = auth_mobility::reconcile_stream_access_grants_for_auth_credential(
        state,
        account_id,
        &account.subdomain_access,
    )
    .await
    {
        tracing::warn!(%error, %account_id, "failed to roll back account stream access grants");
    }
    if let Some(totp) = snapshot
        .totps
        .iter()
        .find(|totp| totp.id == account.source_totp_id)
        && let Err(error) = auth_mobility::reconcile_stream_access_grants_for_totp_credential(
            state,
            &totp.id,
            &totp.subdomain_access,
        )
        .await
    {
        tracing::warn!(%error, %account_id, totp_id = %totp.id, "failed to roll back linked TOTP stream access grants");
    }
}

async fn rollback_auth_account_mutation(
    state: &AppState,
    snapshot: &AuthAccountRollbackSnapshot,
    account_id: &str,
    context: &'static str,
) {
    if let Err(error) = state
        .storage
        .store
        .set_auth_accounts(&snapshot.accounts)
        .await
    {
        tracing::warn!(%error, %account_id, %context, "failed to roll back auth accounts");
    }
    if let Err(error) = state.storage.store.set_totps(&snapshot.totps).await {
        tracing::warn!(%error, %account_id, %context, "failed to roll back TOTP credentials");
    }
    let password_result = match snapshot.password.as_ref() {
        Some(record) => {
            state
                .storage
                .store
                .set_auth_password_credential(record)
                .await
        }
        None => {
            state
                .storage
                .store
                .delete_auth_password_credential(account_id)
                .await
        }
    };
    if let Err(error) = password_result {
        tracing::warn!(%error, %account_id, %context, "failed to roll back auth account password");
    }
}

async fn count_configured_passwords(
    state: &AppState,
    accounts: &[AuthAccount],
) -> anyhow::Result<usize> {
    let mut count = 0usize;
    for account in accounts {
        if state
            .storage
            .store
            .get_auth_password_credential(&account.id)
            .await?
            .is_some()
        {
            count += 1;
        }
    }
    Ok(count)
}

fn password_required_before_switch(account_count: usize, password_configured_count: usize) -> bool {
    password_configured_count < account_count
}

pub(super) async fn projected_auth_accounts(
    state: &AppState,
) -> anyhow::Result<(Vec<AuthAccount>, Vec<AuthAccount>)> {
    let totps = state.storage.store.get_totps().await?;
    let accounts = state.storage.store.get_auth_accounts().await?;
    let projected = project_totps_to_accounts(&totps, &accounts);
    Ok((accounts, projected))
}

async fn persist_auth_accounts_projection(
    state: &AppState,
    current: &[AuthAccount],
    projected: &[AuthAccount],
) -> anyhow::Result<()> {
    if auth_accounts_need_store_update(current, projected) {
        state.storage.store.set_auth_accounts(projected).await?;
    }
    let projected_ids = projected
        .iter()
        .map(|account| account.id.as_str())
        .collect::<HashSet<_>>();
    for stale in current
        .iter()
        .filter(|account| !projected_ids.contains(account.id.as_str()))
    {
        state
            .storage
            .store
            .delete_auth_password_credential(&stale.id)
            .await?;
    }
    Ok(())
}

fn auth_accounts_need_store_update(current: &[AuthAccount], projected: &[AuthAccount]) -> bool {
    current.len() != projected.len()
        || current
            .iter()
            .zip(projected)
            .any(|(current, projected)| account_projection_fields_changed(current, projected))
}

fn account_projection_fields_changed(current: &AuthAccount, projected: &AuthAccount) -> bool {
    current.id != projected.id
        || current.username != projected.username
        || current.display_name != projected.display_name
        || current.source_totp_id != projected.source_totp_id
        || current.created_at != projected.created_at
        || current.updated_at != projected.updated_at
        || current.access_scopes != projected.access_scopes
        || current.subdomain_access != projected.subdomain_access
}

fn project_totps_to_accounts(
    totps: &[TotpCredential],
    existing: &[AuthAccount],
) -> Vec<AuthAccount> {
    let now = time_utils::now_iso();
    let totp_ids = totps
        .iter()
        .map(|totp| totp.id.as_str())
        .collect::<HashSet<_>>();
    let existing_by_totp = existing
        .iter()
        .filter(|account| !account.source_totp_id.trim().is_empty())
        .map(|account| (account.source_totp_id.clone(), account.clone()))
        .collect::<HashMap<_, _>>();
    let mut used_usernames = HashSet::<String>::new();
    let mut projected = Vec::with_capacity(totps.len());

    for totp in totps {
        let existing_account = existing_by_totp.get(&totp.id).cloned();
        let initial_timestamp = if totp.created_at.trim().is_empty() {
            now.clone()
        } else {
            totp.created_at.clone()
        };
        let mut account = existing_by_totp
            .get(&totp.id)
            .cloned()
            .unwrap_or_else(|| AuthAccount {
                id: deterministic_auth_account_id(&totp.id),
                username: String::new(),
                display_name: String::new(),
                source_totp_id: totp.id.clone(),
                created_at: initial_timestamp.clone(),
                updated_at: initial_timestamp.clone(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: json!({ "mode": "all", "hosts": [] }),
            });
        let original = account.clone();
        account.source_totp_id = totp.id.clone();
        account.access_scopes =
            crate::store::normalize_totp_access_scopes(totp.access_scopes.clone());
        account.subdomain_access =
            crate::store::normalize_totp_subdomain_access(totp.subdomain_access.clone());
        if account.created_at.trim().is_empty() {
            account.created_at = initial_timestamp.clone();
        }
        let preferred =
            validate_username(&account.username).unwrap_or_else(|_| username_base_from_totp(totp));
        account.username = unique_username(&preferred, &mut used_usernames);
        account.display_name = account.username.clone();
        account.updated_at = if existing_account.is_none() {
            initial_timestamp.clone()
        } else if original.username == account.username
            && original.display_name == account.display_name
            && original.source_totp_id == account.source_totp_id
            && original.access_scopes == account.access_scopes
            && original.subdomain_access == account.subdomain_access
            && !original.updated_at.trim().is_empty()
        {
            original.updated_at
        } else {
            now.clone()
        };
        projected.push(account);
    }

    for existing_account in existing.iter().filter(|account| {
        account.source_totp_id.trim().is_empty()
            || !totp_ids.contains(account.source_totp_id.as_str())
    }) {
        let mut account = existing_account.clone();
        let original = account.clone();
        let initial_timestamp = if account.created_at.trim().is_empty() {
            now.clone()
        } else {
            account.created_at.clone()
        };
        if account.created_at.trim().is_empty() {
            account.created_at = initial_timestamp.clone();
        }
        account.access_scopes =
            crate::store::normalize_totp_access_scopes(account.access_scopes.clone());
        account.subdomain_access =
            crate::store::normalize_totp_subdomain_access(account.subdomain_access.clone());
        let fallback_suffix = account
            .id
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .take(8)
            .collect::<String>();
        let fallback = if fallback_suffix.is_empty() {
            "user".to_string()
        } else {
            format!("user-{fallback_suffix}")
        };
        let preferred = validate_username(&account.username).unwrap_or(fallback);
        account.username = unique_username(&preferred, &mut used_usernames);
        account.display_name = account.username.clone();
        account.updated_at = if original.username == account.username
            && original.display_name == account.display_name
            && original.access_scopes == account.access_scopes
            && original.subdomain_access == account.subdomain_access
            && !original.updated_at.trim().is_empty()
        {
            original.updated_at
        } else {
            now.clone()
        };
        projected.push(account);
    }
    projected
}

fn deterministic_auth_account_id(totp_id: &str) -> String {
    let digest = crate::crypto_utils::sha256_hex_str(&format!("totp:{totp_id}"));
    format!("auth-account-{}", &digest[..16])
}

fn unique_auth_account_id(existing: &[AuthAccount]) -> String {
    let used = existing
        .iter()
        .map(|account| account.id.as_str())
        .collect::<HashSet<_>>();
    loop {
        let id = format!("auth-account-{}", hex::encode(random_bytes::<8>()));
        if !used.contains(id.as_str()) {
            return id;
        }
    }
}

async fn account_has_usable_totp(state: &AppState, account: &AuthAccount) -> anyhow::Result<bool> {
    let totps = state.storage.store.get_totps().await?;
    Ok(account_has_usable_totp_in(&totps, account))
}

fn account_has_usable_totp_in(totps: &[TotpCredential], account: &AuthAccount) -> bool {
    !account.source_totp_id.trim().is_empty()
        && totps
            .iter()
            .any(|credential| credential.id == account.source_totp_id)
}

fn unique_totp_id(existing: &[TotpCredential]) -> String {
    let used = existing
        .iter()
        .map(|credential| credential.id.as_str())
        .collect::<HashSet<_>>();
    loop {
        let id = hex::encode(random_bytes::<8>());
        if !used.contains(id.as_str()) {
            return id;
        }
    }
}

async fn apply_accounts_to_totps(state: &AppState, accounts: &[AuthAccount]) -> anyhow::Result<()> {
    let mut totps = state.storage.store.get_totps().await?;
    let mut updated = false;
    for account in accounts {
        let Some(totp) = totps
            .iter_mut()
            .find(|credential| credential.id == account.source_totp_id)
        else {
            anyhow::bail!("auth account {} has no source TOTP", account.id);
        };
        sync_totp_metadata_from_account(totp, account);
        updated = true;
    }
    if updated {
        state.storage.store.set_totps(&totps).await?;
    }
    Ok(())
}

async fn sync_account_to_source_totp(
    state: &AppState,
    account: &AuthAccount,
) -> anyhow::Result<()> {
    if account.source_totp_id.trim().is_empty() {
        return Ok(());
    }
    let mut totps = state.storage.store.get_totps().await?;
    let Some(totp) = totps
        .iter_mut()
        .find(|credential| credential.id == account.source_totp_id)
    else {
        return Ok(());
    };
    sync_totp_metadata_from_account(totp, account);
    state.storage.store.set_totps(&totps).await?;
    Ok(())
}

fn sync_totp_metadata_from_account(totp: &mut TotpCredential, account: &AuthAccount) {
    totp.comment = account.username.clone();
    totp.access_scopes = crate::store::normalize_totp_access_scopes(account.access_scopes.clone());
    totp.subdomain_access =
        crate::store::normalize_totp_subdomain_access(account.subdomain_access.clone());
}

fn validate_username(value: &str) -> Result<String, &'static str> {
    let username = value.trim().to_lowercase();
    if username.is_empty() {
        return Err("authAccounts.usernameTooShort");
    }
    if username.len() > 64 {
        return Err("authAccounts.usernameTooLong");
    }
    if username
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err("authAccounts.usernameInvalid");
    }
    if !username
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err("authAccounts.usernameInvalid");
    }
    Ok(username)
}

fn username_base_from_totp(totp: &TotpCredential) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in totp.comment.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_') {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.len() >= 3 {
        return slug.chars().take(48).collect();
    }
    format!("user-{}", totp.id.chars().take(8).collect::<String>())
}

fn unique_username(base: &str, used: &mut HashSet<String>) -> String {
    let base = validate_username(base).unwrap_or_else(|_| "user".to_string());
    if used.insert(base.clone()) {
        return base;
    }
    let mut suffix = 2usize;
    loop {
        let max_base_len = 64usize.saturating_sub(suffix.to_string().len() + 1);
        let candidate = format!(
            "{}-{suffix}",
            base.chars().take(max_base_len.max(1)).collect::<String>()
        );
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

fn auth_account_password_error_key(key: &'static str) -> &'static str {
    match key {
        "passwordTooShort" => "authAccounts.passwordTooShort",
        "passwordTooLong" => "authAccounts.passwordTooLong",
        "passwordWhitespace" => "authAccounts.passwordWhitespace",
        "passwordNeedsLettersAndNumbers" => "authAccounts.passwordNeedsLettersAndNumbers",
        _ => "authAccounts.passwordSaveFailed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn account_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary account database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "auth-account-create-test".to_string();
        let state = AppState::new(settings).await.expect("account test state");
        (directory, state)
    }

    #[tokio::test]
    async fn concurrent_auth_account_creation_preserves_accounts_and_passwords() {
        let (_directory, state) = account_test_state().await;
        let create = |username: &str, password: &str| {
            auth_account_create(
                State(state.clone()),
                Json(AuthAccountCreateBody {
                    username: username.to_string(),
                    password: password.to_string(),
                }),
            )
        };
        let (first, second) = tokio::join!(
            create("first", "first-password123"),
            create("second", "second-password456"),
        );
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        let accounts = state.storage.store.get_auth_accounts().await.unwrap();
        assert_eq!(accounts.len(), 2);
        for (username, password) in [
            ("first", "first-password123"),
            ("second", "second-password456"),
        ] {
            let account = accounts
                .iter()
                .find(|item| item.username == username)
                .unwrap();
            let credential = state
                .storage
                .store
                .get_auth_password_credential(&account.id)
                .await
                .unwrap()
                .expect("every successfully created account has its password");
            assert!(
                crate::auth::password::verify_auth_password(password, &credential)
                    .await
                    .unwrap()
            );
        }
    }

    #[tokio::test]
    async fn concurrent_duplicate_auth_account_creation_keeps_only_winner_password() {
        let (_directory, state) = account_test_state().await;
        let create = |username: &str, password: &str| {
            auth_account_create(
                State(state.clone()),
                Json(AuthAccountCreateBody {
                    username: username.to_string(),
                    password: password.to_string(),
                }),
            )
        };
        let (first, second) = tokio::join!(
            create("Shared-User", "first-password123"),
            create("shared-user", "second-password456"),
        );
        let (winner, winner_password, loser, loser_password) = if first.status() == StatusCode::OK {
            (first, "first-password123", second, "second-password456")
        } else {
            (second, "second-password456", first, "first-password123")
        };
        assert_eq!(winner.status(), StatusCode::OK);
        assert_eq!(loser.status(), StatusCode::CONFLICT);
        let accounts = state.storage.store.get_auth_accounts().await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].username, "shared-user");
        let credential = state
            .storage
            .store
            .get_auth_password_credential(&accounts[0].id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            crate::auth::password::verify_auth_password(winner_password, &credential)
                .await
                .unwrap()
        );
        assert!(
            !crate::auth::password::verify_auth_password(loser_password, &credential)
                .await
                .unwrap()
        );
    }

    fn fake_password(account_id: &str, hash: &str) -> AuthPasswordCredential {
        AuthPasswordCredential {
            account_id: account_id.to_string(),
            algorithm: "scrypt".to_string(),
            salt: "00".repeat(16),
            hash: hash.to_string(),
            n: 16_384,
            r: 8,
            p: 1,
            key_length: 64,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn auth_account_password_hash_wait_preserves_concurrent_create() {
        let (_directory, state) = account_test_state().await;
        for username in [None, Some("renamed-alice")] {
            let account = auth_account("original-id", "");
            state
                .storage
                .store
                .set_auth_accounts(&[account])
                .await
                .unwrap();
            state
                .storage
                .store
                .set_auth_password_credential(&fake_password("original-id", "old"))
                .await
                .unwrap();
            let hashing = tokio::sync::Notify::new();
            let resume = tokio::sync::Notify::new();
            // A controlled hash boundary proves create can complete while the
            // first request hashes, and its account survives the subsequent write.
            let update = update_auth_account_password(&state, "original-id", username, |_| async {
                hashing.notify_one();
                resume.notified().await;
                Ok(fake_password("original-id", "new"))
            });
            let create = async {
                hashing.notified().await;
                let result = auth_account_create(
                    State(state.clone()),
                    Json(AuthAccountCreateBody {
                        username: "concurrent".to_string(),
                        password: "concurrent-password123".to_string(),
                    }),
                )
                .await;
                resume.notify_one();
                result
            };
            let (updated, created) =
                tokio::time::timeout(std::time::Duration::from_secs(30), async {
                    tokio::join!(update, create)
                })
                .await
                .expect("hash wait must not hold the mutation lock");
            assert_eq!(created.status(), StatusCode::OK);
            assert_eq!(updated.status(), StatusCode::OK);
            let accounts = state.storage.store.get_auth_accounts().await.unwrap();
            assert_eq!(accounts.len(), 2);
            let original = accounts
                .iter()
                .find(|item| item.id == "original-id")
                .unwrap();
            assert_eq!(original.username, username.unwrap_or("alice"));
            assert!(accounts.iter().any(|item| item.username == "concurrent"));
            assert_eq!(
                state
                    .storage
                    .store
                    .get_auth_password_credential("original-id")
                    .await
                    .unwrap()
                    .unwrap()
                    .hash,
                "new"
            );
        }
    }

    #[tokio::test]
    async fn auth_account_password_hash_wait_rejects_target_changes_without_reviving_it() {
        let (_directory, state) = account_test_state().await;
        for changed in ["delete", "rename", "password", "totp"] {
            let account = auth_account("original-id", "source");
            state
                .storage
                .store
                .set_auth_accounts(&[account])
                .await
                .unwrap();
            state
                .storage
                .store
                .set_auth_password_credential(&fake_password("original-id", "old"))
                .await
                .unwrap();
            state
                .storage
                .store
                .set_totps(&[TotpCredential {
                    id: "source".to_string(),
                    secret: "SECRET".to_string(),
                    comment: "alice".to_string(),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                    access_scopes: json!([]),
                    subdomain_access: json!({"mode":"all", "hosts":[]}),
                }])
                .await
                .unwrap();
            let hashing = tokio::sync::Notify::new();
            let resume = tokio::sync::Notify::new();
            let update = update_auth_account_password(
                &state,
                "original-id",
                Some("requested-name"),
                |_| async {
                    hashing.notify_one();
                    resume.notified().await;
                    Ok(fake_password("original-id", "requested-password"))
                },
            );
            let concurrent = async {
                hashing.notified().await;
                {
                    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
                    match changed {
                        "delete" => {
                            state.storage.store.set_auth_accounts(&[]).await.unwrap();
                            state.storage.store.set_totps(&[]).await.unwrap();
                            state
                                .storage
                                .store
                                .delete_auth_password_credential("original-id")
                                .await
                                .unwrap();
                        }
                        "rename" => {
                            let mut accounts =
                                state.storage.store.get_auth_accounts().await.unwrap();
                            accounts[0].username = "concurrent-name".to_string();
                            state
                                .storage
                                .store
                                .set_auth_accounts(&accounts)
                                .await
                                .unwrap();
                        }
                        "password" => {
                            state
                                .storage
                                .store
                                .set_auth_password_credential(&fake_password(
                                    "original-id",
                                    "concurrent-password",
                                ))
                                .await
                                .unwrap();
                        }
                        "totp" => {
                            state.storage.store.set_totps(&[]).await.unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
                resume.notify_one();
            };
            let (response, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::join!(update, concurrent)
            })
            .await
            .unwrap();
            assert_eq!(
                response.status(),
                if changed == "delete" {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::CONFLICT
                }
            );
            let accounts = state.storage.store.get_auth_accounts().await.unwrap();
            if changed == "delete" {
                assert!(accounts.is_empty());
                assert!(
                    state
                        .storage
                        .store
                        .get_auth_password_credential("original-id")
                        .await
                        .unwrap()
                        .is_none()
                );
            } else {
                assert_ne!(accounts[0].username, "requested-name");
                assert_ne!(
                    state
                        .storage
                        .store
                        .get_auth_password_credential("original-id")
                        .await
                        .unwrap()
                        .unwrap()
                        .hash,
                    "requested-password"
                );
            }
        }
    }

    #[tokio::test]
    async fn cancelled_auth_account_password_request_finishes_revocation_and_rollback() {
        let (_directory, state) = account_test_state().await;
        for force_revoke_failure in [false, true] {
            state
                .storage
                .store
                .set_auth_accounts(&[auth_account("original-id", "")])
                .await
                .unwrap();
            state
                .storage
                .store
                .set_auth_password_credential(&fake_password("original-id", "old"))
                .await
                .unwrap();
            if force_revoke_failure {
                // Revocation removes this authoritative session, then the
                // deliberately unavailable gateway causes its required trust
                // sync to fail. The completion task must still roll back.
                let mut session =
                    crate::store::new_login_session("", "alice", "203.0.113.1", "test", 60);
                session.method = "PASSWORD".to_string();
                session.credential_id = "original-id".to_string();
                state
                    .storage
                    .store
                    .add_session("cancelled-update-session", &session, 60)
                    .await
                    .unwrap();
            }
            let committed = std::sync::Arc::new(tokio::sync::Notify::new());
            let release = std::sync::Arc::new(tokio::sync::Notify::new());
            let after_commit = {
                let committed = committed.clone();
                let release = release.clone();
                async move {
                    committed.notify_one();
                    release.notified().await;
                }
            };
            let mut request = Box::pin(update_auth_account_password_with_commit_hook(
                &state,
                "original-id",
                Some("changed-name"),
                |_| async { Ok(fake_password("original-id", "new")) },
                after_commit,
            ));
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                tokio::select! {
                    _ = committed.notified() => {}
                    _ = &mut request => panic!("request returned before the commit checkpoint was released"),
                }
            }).await.unwrap();
            assert_eq!(
                state
                    .storage
                    .store
                    .get_auth_password_credential("original-id")
                    .await
                    .unwrap()
                    .unwrap()
                    .hash,
                "new"
            );
            drop(request); // Simulate the HTTP client disappearing after commit.
            assert!(
                state
                    .storage
                    .store
                    .auth_account_mutation_lock
                    .try_lock()
                    .is_err()
            );
            release.notify_one();
            let finished = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                state.storage.store.auth_account_mutation_lock.lock(),
            )
            .await
            .expect("registry-owned mutation finishes after HTTP cancellation");
            assert!(
                state
                    .storage
                    .store
                    .get_session("cancelled-update-session")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                state
                    .storage
                    .store
                    .get_auth_password_credential("original-id")
                    .await
                    .unwrap()
                    .unwrap()
                    .hash,
                if force_revoke_failure { "old" } else { "new" }
            );
            assert_eq!(
                state
                    .storage
                    .store
                    .get_auth_account("original-id")
                    .await
                    .unwrap()
                    .unwrap()
                    .username,
                if force_revoke_failure {
                    "alice"
                } else {
                    "changed-name"
                }
            );
            drop(finished);
        }
        assert!(
            state
                .background_tasks
                .shutdown(std::time::Duration::from_secs(1))
                .await
                .is_empty()
        );
    }

    #[test]
    fn derives_unique_usernames_from_totps() {
        let totps = vec![
            TotpCredential {
                id: "a1".to_string(),
                secret: "SECRET".to_string(),
                comment: "Alice Admin".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: json!({ "mode": "all", "hosts": [] }),
            },
            TotpCredential {
                id: "b2".to_string(),
                secret: "SECRET".to_string(),
                comment: "Alice Admin".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: json!({ "mode": "all", "hosts": [] }),
            },
        ];
        let accounts = project_totps_to_accounts(&totps, &[]);
        assert_eq!(accounts[0].username, "alice-admin");
        assert_eq!(accounts[1].username, "alice-admin-2");
    }

    #[test]
    fn validates_username_shape() {
        assert_eq!(validate_username("Admin_01").unwrap(), "admin_01");
        assert_eq!(validate_username("a").unwrap(), "a");
        assert!(validate_username("").is_err());
        assert!(validate_username("a b").is_err());
        assert!(validate_username("中文").is_err());
    }

    #[test]
    fn password_switch_allows_empty_account_projection() {
        assert!(!password_required_before_switch(0, 0));
    }

    #[test]
    fn password_switch_blocks_accounts_without_any_password() {
        assert!(password_required_before_switch(2, 0));
    }

    #[test]
    fn password_switch_blocks_accounts_with_any_missing_password() {
        assert!(password_required_before_switch(2, 1));
    }

    #[test]
    fn password_switch_allows_accounts_when_all_passwords_are_configured() {
        assert!(!password_required_before_switch(2, 2));
    }

    fn auth_account(id: &str, source_totp_id: &str) -> AuthAccount {
        AuthAccount {
            id: id.to_string(),
            username: "alice".to_string(),
            display_name: "alice".to_string(),
            source_totp_id: source_totp_id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: json!(["docker_admin_panel", "unknown"]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": ["https://App.Example.com/path", "__builtin_select__"]
            }),
        }
    }

    #[test]
    fn totp_to_account_projection_is_idempotent() {
        let totps = vec![TotpCredential {
            id: "totp-a".to_string(),
            secret: "SECRET".to_string(),
            comment: "Alice Admin".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: json!(["docker_admin_panel"]),
            subdomain_access: json!({ "mode": "custom", "hosts": ["app.example.com"] }),
        }];
        let first = project_totps_to_accounts(&totps, &[]);
        assert_eq!(first[0].created_at, "2026-01-01T00:00:00Z");
        assert_eq!(first[0].updated_at, "2026-01-01T00:00:00Z");
        let orphan = AuthAccount {
            id: "orphan".to_string(),
            username: "orphan".to_string(),
            display_name: "Orphan".to_string(),
            source_totp_id: "missing".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: Value::Array(Vec::new()),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        };
        let mut existing = first.clone();
        existing.push(orphan);

        let second = project_totps_to_accounts(&totps, &existing);

        assert_eq!(second.len(), 2);
        assert_eq!(second[0].id, first[0].id);
        assert_eq!(second[0].username, first[0].username);
        assert_eq!(second[0].updated_at, first[0].updated_at);
        assert_eq!(second[0].source_totp_id, "totp-a");
        assert_eq!(second[0].access_scopes, json!(["docker_admin_panel"]));
        assert_eq!(second[0].subdomain_access["mode"], "custom");
        assert_eq!(second[1].id, "orphan");
        assert_eq!(second[1].username, "orphan");
        assert_eq!(second[1].source_totp_id, "missing");
    }

    #[test]
    fn usable_totp_check_requires_existing_source_totp() {
        let totps = vec![TotpCredential {
            id: "totp-a".to_string(),
            secret: "SECRET".to_string(),
            comment: "Alice".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: Value::Array(Vec::new()),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        }];

        assert!(account_has_usable_totp_in(
            &totps,
            &auth_account("account-a", "totp-a")
        ));
        assert!(!account_has_usable_totp_in(
            &totps,
            &auth_account("account-b", "")
        ));
        assert!(!account_has_usable_totp_in(
            &totps,
            &auth_account("account-c", "missing")
        ));
    }

    #[test]
    fn account_delete_removes_accounts_sharing_source_totp() {
        let accounts = vec![
            auth_account("account-a", "totp-a"),
            auth_account("account-b", "totp-a"),
            auth_account("account-c", "totp-c"),
        ];

        let removed = account_ids_removed_by_account_delete(&accounts, &accounts[0]);

        assert!(removed.contains("account-a"));
        assert!(removed.contains("account-b"));
        assert!(!removed.contains("account-c"));
    }

    #[test]
    fn account_delete_without_totp_removes_only_target_account() {
        let accounts = vec![auth_account("account-a", ""), auth_account("account-b", "")];

        let removed = account_ids_removed_by_account_delete(&accounts, &accounts[0]);

        assert!(removed.contains("account-a"));
        assert!(!removed.contains("account-b"));
    }

    #[test]
    fn subdomain_update_clear_decision_only_applies_to_custom_restrictions() {
        let mut account = auth_account("account-a", "totp-a");

        assert!(should_clear_auto_ip_grants_for_subdomain_update(
            true, &account
        ));
        assert!(!should_clear_auto_ip_grants_for_subdomain_update(
            false, &account
        ));

        account.subdomain_access = json!({ "mode": "all", "hosts": [] });

        assert!(!should_clear_auto_ip_grants_for_subdomain_update(
            true, &account
        ));
    }

    #[test]
    fn account_metadata_sync_normalizes_totp_permissions() {
        let account = auth_account("account-a", "totp-a");
        let mut totp = TotpCredential {
            id: "totp-a".to_string(),
            secret: "SECRET".to_string(),
            comment: "old".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: Value::Array(Vec::new()),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        };

        sync_totp_metadata_from_account(&mut totp, &account);

        assert_eq!(totp.comment, "alice");
        assert_eq!(totp.access_scopes, json!(["docker_admin_panel"]));
        assert_eq!(
            totp.subdomain_access,
            json!({
                "mode": "custom",
                "hosts": ["__builtin_select__", "app.example.com"],
                "streams": []
            })
        );
    }
}
