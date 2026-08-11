use axum::{Router, http::StatusCode};
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

pub(crate) use auth_mode::{auth_account_routes, auth_mode_routes};
pub(crate) use handlers::auth_credential_settings_routes;
pub(crate) use handlers::session_routes;
pub(crate) use handlers::totp_bootstrap_routes;
pub(crate) use handlers::totp_management_routes;

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

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct AuthLoginModeBody {
    mode: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct AuthAccountPatchBody {
    username: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct AuthAccountCreateBody {
    username: String,
    password: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct AuthAccountPasswordBody {
    password: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct AuthAccountSetupBody {
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

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct TotpBindBody {
    secret: String,
    token: String,
    comment: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct TotpCommentBody {
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

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct SessionCommentBody {
    comment: String,
}

pub fn admin_control_routes() -> Router<AppState> {
    let auth_mode_routes: Router<AppState> = auth_mode_routes().into();
    let auth_account_routes: Router<AppState> = auth_account_routes().into();
    let session_routes: Router<AppState> = session_routes().into();
    let auth_credential_settings_routes: Router<AppState> =
        auth_credential_settings_routes().into();
    let totp_bootstrap_routes: Router<AppState> = totp_bootstrap_routes().into();
    let totp_management_routes: Router<AppState> = totp_management_routes().into();
    Router::new()
        .merge(auth_credential_settings_routes)
        .merge(totp_management_routes)
        .merge(totp_bootstrap_routes)
        .merge(auth_account_routes)
        .merge(session_routes)
        .merge(auth_mode_routes)
}

#[cfg(test)]
mod tests;
