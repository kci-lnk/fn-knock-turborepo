use axum::{
    Router,
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::state::AppState;

mod auth_mode;
mod gateway;
mod handlers;
mod sessions;
mod settings;
mod text;
mod transfer;

use auth_mode::{
    auth_account_create, auth_account_delete, auth_account_set_password, auth_account_setup,
    auth_account_totp_bind, auth_account_totp_setup, auth_account_update,
    auth_account_update_access_scopes, auth_account_update_subdomain_access, auth_accounts_list,
    auth_login_mode_preview, auth_login_mode_status, auth_login_mode_switch,
};
use handlers::{
    get_auth_credential_settings, passkey_delete, session_delete, session_get,
    session_mobility_details, session_update_comment, sessions_list, totp_bind, totp_delete,
    totp_export, totp_import, totp_passkeys, totp_setup, totp_status, totp_update_access_scopes,
    totp_update_comment, totp_update_subdomain_access, update_auth_credential_settings,
};

#[cfg(test)]
use crate::{
    auth_mobility,
    i18n::Translator,
    store::{AuthAccount, AuthPasswordCredential, LoginSession, TotpCredential},
    time_utils,
};
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use sessions::{
    apply_mobility_event_ip_locations, build_mobility_login_event, build_mobility_summary,
    normalize_auto_ip_grant_comment_value, session_attachment_from_binding,
};
#[cfg(test)]
use settings::{node_totp_bind_comment, normalize_auth_credential_settings};
#[cfg(test)]
use std::collections::{BTreeMap, HashSet};
#[cfg(test)]
use text::{admin_control_text, totp_import_error_message, totp_import_error_with_max};
#[cfg(test)]
use transfer::{
    CredentialImportPlan, build_credential_import_plan, build_password_export_payload,
    build_totp_export_payload, build_totp_import_plan,
};

const TOTP_TRANSFER_KIND: &str = "fn-knock.totp-credentials";
const TOTP_TRANSFER_VERSION: u64 = 1;
const PASSWORD_TRANSFER_KIND: &str = "fn-knock.password-credentials";
const PASSWORD_TRANSFER_VERSION: u64 = 1;
const MAX_TOTP_IMPORT_COUNT: usize = 200;
const MAX_AUTH_ACCOUNT_IMPORT_COUNT: usize = 200;
const AUTH_SESSION_TTL_SECONDS_DEFAULT: i64 = 24 * 3600;
const AUTH_REMEMBER_ME_TTL_SECONDS_DEFAULT: i64 = 365 * 24 * 3600;
const AUTH_POST_LOGIN_IP_GRANT_TTL_SECONDS_DEFAULT: i64 = 3600;
const AUTH_SESSION_IP_MOBILITY_WINDOW_SECONDS_DEFAULT: i64 = 20 * 60;
const AUTH_MAX_TTL_SECONDS: i64 = 5 * 365 * 24 * 3600;

#[derive(Debug)]
struct TotpImportRouteError {
    status: StatusCode,
    key: &'static str,
    max: Option<usize>,
}

#[derive(Deserialize)]
struct AuthCredentialSettingsBody {
    #[serde(flatten)]
    value: Map<String, Value>,
}

#[derive(Deserialize)]
struct AuthLoginModeBody {
    mode: String,
}

#[derive(Deserialize)]
struct AuthAccountPatchBody {
    username: Option<String>,
}

#[derive(Deserialize)]
struct AuthAccountCreateBody {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AuthAccountPasswordBody {
    password: String,
}

#[derive(Deserialize)]
struct AuthAccountSetupBody {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AuthAccountAccessScopesBody {
    access_scopes: Value,
}

#[derive(Deserialize)]
struct AuthAccountSubdomainAccessBody {
    subdomain_access: Value,
}

#[derive(Deserialize)]
struct TotpBindBody {
    secret: String,
    token: String,
    comment: Option<String>,
}

#[derive(Deserialize)]
struct TotpCommentBody {
    comment: String,
}

#[derive(Deserialize)]
struct TotpAccessScopesBody {
    access_scopes: Value,
}

#[derive(Deserialize)]
struct TotpSubdomainAccessBody {
    subdomain_access: Value,
}

#[derive(Deserialize)]
struct TotpImportBody {
    payload: Value,
}

#[derive(Deserialize)]
struct SessionCommentBody {
    comment: String,
}

pub fn admin_control_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/auth_credential_settings",
            get(get_auth_credential_settings).post(update_auth_credential_settings),
        )
        .route("/api/admin/totp/status", get(totp_status))
        .route("/api/admin/auth/mode", get(auth_login_mode_status))
        .route(
            "/api/admin/auth/mode/preview",
            post(auth_login_mode_preview),
        )
        .route("/api/admin/auth/mode/switch", post(auth_login_mode_switch))
        .route(
            "/api/admin/auth/accounts",
            get(auth_accounts_list).post(auth_account_create),
        )
        .route(
            "/api/admin/auth/accounts/{id}",
            patch(auth_account_update).delete(auth_account_delete),
        )
        .route(
            "/api/admin/auth/accounts/{id}/password",
            post(auth_account_set_password),
        )
        .route(
            "/api/admin/auth/accounts/{id}/setup",
            post(auth_account_setup),
        )
        .route(
            "/api/admin/auth/accounts/{id}/totp/setup",
            post(auth_account_totp_setup),
        )
        .route(
            "/api/admin/auth/accounts/{id}/totp/bind",
            post(auth_account_totp_bind),
        )
        .route(
            "/api/admin/auth/accounts/{id}/access-scopes",
            patch(auth_account_update_access_scopes),
        )
        .route(
            "/api/admin/auth/accounts/{id}/subdomain-access",
            patch(auth_account_update_subdomain_access),
        )
        .route("/api/admin/totp/setup", post(totp_setup))
        .route("/api/admin/totp/bind", post(totp_bind))
        .route("/api/admin/totp/credentials/export", get(totp_export))
        .route("/api/admin/totp/credentials/import", post(totp_import))
        .route("/api/admin/totp/{id}", delete(totp_delete))
        .route(
            "/api/admin/totp/{id}/access-scopes",
            patch(totp_update_access_scopes),
        )
        .route(
            "/api/admin/totp/{id}/subdomain-access",
            patch(totp_update_subdomain_access),
        )
        .route("/api/admin/totp/{id}/comment", patch(totp_update_comment))
        .route("/api/admin/totp/{totp_id}/passkeys", get(totp_passkeys))
        .route("/api/admin/passkeys/{id}", delete(passkey_delete))
        .route("/api/admin/sessions", get(sessions_list))
        .route(
            "/api/admin/sessions/{id}",
            get(session_get).delete(session_delete),
        )
        .route(
            "/api/admin/sessions/{id}/comment",
            patch(session_update_comment),
        )
        .route(
            "/api/admin/sessions/{id}/mobility",
            get(session_mobility_details),
        )
}

#[cfg(test)]
mod tests;
