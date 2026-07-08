use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jsonwebtoken::{
    DecodingKey, Validation, decode, decode_header,
    jwk::{Jwk, JwkSet},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq;
use url::Url;

use crate::{
    auth_mobility::{self, CreateLoginSessionInput},
    backoff::normalize_auth_failure_tracking_ip,
    cookies,
    http_utils::get_client_ip,
    i18n::{DEFAULT_LOCALE, Translator},
    oidc_admin::{
        OIDC_HTTP_USER_AGENT, oidc_consume_invite, oidc_consume_login_error_notice,
        oidc_consume_state, oidc_get_binding_by_subject, oidc_get_provider, oidc_inspect_invite,
        oidc_provider_ready_with_translator, oidc_save_binding,
        oidc_save_binding_if_subject_available, oidc_save_login_error_notice, oidc_save_state,
        resolve_discovery_with_translator,
    },
    response::{self, ApiEnvelope},
    state::AppState,
    system_events, time_utils,
};

const OIDC_STATE_TTL_SECONDS: usize = 10 * 60;
const LOGIN_ERROR_TTL_SECONDS: usize = 5 * 60;

#[derive(Deserialize)]
struct BindQuery {
    token: Option<String>,
    provider_id: Option<String>,
}

#[derive(Deserialize)]
struct StartBody {
    provider_id: String,
    mode: Option<String>,
    invite_token: Option<String>,
    redirect_uri: Option<String>,
    #[serde(default, rename = "rememberMe")]
    remember_me: bool,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}

struct AuthorizationBuild {
    authorization_url: String,
    flow_token: String,
    max_age: usize,
}

struct ExternalProfile {
    issuer: String,
    subject: String,
    display_name: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    avatar_url: Option<String>,
}

struct CallbackResolved {
    state: Value,
    provider: Value,
    binding: Value,
    profile: ExternalProfile,
}

pub fn oidc_runtime_routes() -> Router<AppState> {
    Router::new()
        .route("/oidc/bind", get(bind))
        .route("/oidc/start", post(start))
        .route("/oidc/callback/{provider_id}", get(callback))
}

fn oidc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.oidc.{key}"))
}

fn oidc_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.oidc.{key}"), params)
}

fn server_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.{key}"))
}

fn server_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.{key}"), params)
}

fn translator_from_config(config: &Value) -> Translator {
    Translator::new(locale_code(config))
}

mod authorization;
mod handlers;
mod helpers;
mod login_error;
mod providers;
mod session;

use authorization::*;
use handlers::{bind, callback, start};
use helpers::*;
pub(crate) use login_error::consume_login_error_for_bootstrap;
use login_error::{
    bind_html_response, bind_provider_selection_response, consume_callback_state_for_notice,
    is_oidc_operation_aborted_error, login_error_redirect_response,
    oidc_login_failed_retry_after_message, provider_error_message, redirect_response,
};
use providers::*;
use session::*;

#[cfg(test)]
mod tests;
