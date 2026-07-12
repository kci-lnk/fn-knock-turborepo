use axum::{
    Router,
    routing::{delete, get, patch, post},
};

use crate::state::AppState;

mod discovery;
mod handlers;
mod provider;
mod storage;
mod text;
mod tokens;
mod urls;

use handlers::{
    catalog, create_invitation, create_provider, delete_binding, delete_provider,
    list_bindings_by_totp, list_providers, test_provider, update_provider,
};

pub(crate) use discovery::resolve_discovery_with_translator;
pub(crate) use provider::oidc_provider_ready_with_translator;
#[allow(unused_imports)]
pub(crate) use storage::oidc_list_bindings;
pub(crate) use storage::{
    oidc_consume_invite, oidc_consume_login_error_notice, oidc_consume_state,
    oidc_delete_bindings_by_totp, oidc_get_binding_by_subject, oidc_get_provider,
    oidc_inspect_invite, oidc_public_providers, oidc_save_binding,
    oidc_save_binding_if_subject_available, oidc_save_login_error_notice, oidc_save_state,
};
pub(crate) use urls::callback_base_url;

#[cfg(test)]
use crate::i18n::Translator;
#[cfg(test)]
use axum::http::{HeaderMap, Uri};
#[cfg(test)]
use provider::{
    mask_provider, missing_required_provider_fields, normalize_connection_config, normalize_scopes,
    provider_catalog,
};
#[cfg(test)]
use serde_json::{Map, Value, json};
#[cfg(test)]
use text::oidc_text_params;
#[cfg(test)]
use urls::{callback_origin, public_auth_base_url};

const PROVIDERS_INDEX_KEY: &str = "fn_knock:oidc:providers:index";
const PROVIDERS_DATA_KEY_PREFIX: &str = "fn_knock:oidc:providers:data:";
const BINDINGS_INDEX_KEY: &str = "fn_knock:oidc:bindings:index";
const BINDINGS_DATA_KEY_PREFIX: &str = "fn_knock:oidc:bindings:data:";
const BINDINGS_SUBJECT_KEY_PREFIX: &str = "fn_knock:oidc:bindings:subject:";
const INVITE_KEY_PREFIX: &str = "fn_knock:oidc:invite:";
const STATE_KEY_PREFIX: &str = "fn_knock:oidc:state:";
const LOGIN_ERROR_KEY_PREFIX: &str = "fn_knock:oidc:login_error:";
const DEFAULT_INVITE_TTL_SECONDS: usize = 30 * 60;
pub(crate) const OIDC_HTTP_USER_AGENT: &str = "fn-knock-server-admin-rs/1.0";

pub fn oidc_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/auth/oidc/catalog", get(catalog))
        .route(
            "/api/admin/auth/oidc/providers",
            get(list_providers).post(create_provider),
        )
        .route(
            "/api/admin/auth/oidc/providers/{id}",
            patch(update_provider).delete(delete_provider),
        )
        .route(
            "/api/admin/auth/oidc/providers/{id}/test",
            post(test_provider),
        )
        .route(
            "/api/admin/auth/oidc/totp/{totp_id}/bindings",
            get(list_bindings_by_totp),
        )
        .route("/api/admin/auth/oidc/bindings/{id}", delete(delete_binding))
        .route("/api/admin/auth/oidc/invitations", post(create_invitation))
}

#[cfg(test)]
mod tests;
