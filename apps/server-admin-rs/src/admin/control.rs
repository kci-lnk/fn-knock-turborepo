use axum::{
    Router,
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::state::AppState;

mod gateway;
mod handlers;
mod sessions;
mod settings;
mod text;
mod transfer;

use handlers::{
    get_auth_credential_settings, passkey_delete, session_delete, session_get,
    session_mobility_details, session_update_comment, sessions_list, totp_bind, totp_delete,
    totp_export, totp_import, totp_passkeys, totp_setup, totp_status, totp_update_access_scopes,
    totp_update_comment, totp_update_subdomain_access, update_auth_credential_settings,
};

#[cfg(test)]
use crate::{
    i18n::Translator,
    store::{LoginSession, TotpCredential},
    time_utils,
};
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use sessions::{
    apply_mobility_event_ip_locations, build_mobility_login_event, build_mobility_summary,
    normalize_auto_ip_grant_comment_value, session_attachment_from_binding,
    should_revoke_custom_post_login_ip_grant,
};
#[cfg(test)]
use settings::{node_totp_bind_comment, normalize_auth_credential_settings};
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use text::{admin_control_text, totp_import_error_message, totp_import_error_with_max};
#[cfg(test)]
use transfer::{build_totp_export_payload, build_totp_import_plan};

const TOTP_TRANSFER_KIND: &str = "fn-knock.totp-credentials";
const TOTP_TRANSFER_VERSION: u64 = 1;
const MAX_TOTP_IMPORT_COUNT: usize = 200;
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
