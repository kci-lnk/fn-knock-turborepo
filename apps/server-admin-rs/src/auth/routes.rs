use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, head, post},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use hmac::{Hmac, Mac};
use ipnet::IpNet;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, env, net::IpAddr};
use subtle::ConstantTimeEq;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::{
    auth_mobility::{self, CreateLoginSessionInput},
    backoff::normalize_auth_failure_tracking_ip,
    common_auth_locations, cookies, fnos_share_bypass, http_utils,
    i18n::Translator,
    ip_location,
    oidc_admin::{oidc_inspect_invite, oidc_public_providers},
    oidc_runtime::{consume_login_error_for_bootstrap, oidc_runtime_routes},
    passkey_runtime::{build_passkey_bind_info, passkey_routes, public_passkey_status},
    redis_store::{LoginSession, TotpCredential},
    response::{self, ApiEnvelope},
    scanner,
    state::AppState,
    system_events, time_utils, whitelist,
};

mod bridge;
mod captcha;
mod handlers;
mod preflight;
mod redirect;
mod utils;
mod verify;

pub(crate) use bridge::start_auth_bridge;
use captcha::*;
use handlers::*;
use preflight::*;
pub(crate) use redirect::*;
pub(crate) use utils::*;
use verify::*;

#[cfg(test)]
mod tests;

const TURNSTILE_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const POW_MAX_NUMBER: u32 = 100_000;
const REAUTH_ACCESS_DENIED_HEADER: &str = "X-Reauth-Access-Denied";
const REAUTH_SCOPE_DENIED: &str = "scope";
const REAUTH_SUBDOMAIN_ACCESS_HEADER: &str = "X-Reauth-Subdomain-Access";
const REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER: &str = "X-Reauth-Allowed-Subdomain-Hosts";
const REAUTH_CREDENTIAL_ID_HEADER: &str = "X-Reauth-Credential-Id";
const REAUTH_CREDENTIAL_NAME_HEADER: &str = "X-Reauth-Credential-Name";
const REAUTH_CREDENTIAL_METHOD_HEADER: &str = "X-Reauth-Credential-Method";
const REAUTH_LINKED_TOTP_ID_HEADER: &str = "X-Reauth-Linked-Totp-Id";
const REAUTH_LINKED_TOTP_NAME_HEADER: &str = "X-Reauth-Linked-Totp-Name";
const REAUTH_SUBDOMAIN_ACCESS_CUSTOM: &str = "custom";
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE: &str = "__builtin_select__";
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH: &str = "/__select__";
const AUTH_IDENTITY_HEADER_MAX_LENGTH: usize = 256;
const AUTH_IDENTITY_HEADER_ENCODING_PREFIX: &str = "b64:";

type HmacSha256 = Hmac<Sha256>;

#[derive(Deserialize)]
struct BootstrapQuery {
    redirect_uri: Option<String>,
}

#[derive(Deserialize)]
struct OidcInviteQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
struct LoginBody {
    token: String,
    captcha: CaptchaSubmission,
    #[serde(default, rename = "rememberMe")]
    remember_me: bool,
    redirect_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "provider")]
enum CaptchaSubmission {
    #[serde(rename = "pow")]
    Pow { proof: String },
    #[serde(rename = "turnstile")]
    Turnstile { token: String },
}

fn server_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.{key}"))
}

fn server_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.{key}"), params)
}

fn auth_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.authRoutes.{key}"))
}

fn captcha_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.captcha.{key}"))
}

fn captcha_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.captcha.{key}"), params)
}

fn oidc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.oidc.{key}"))
}

fn translator_from_config(config: &Value) -> Translator {
    let locale = config
        .get("locale")
        .and_then(|locale| locale.get("default_locale"))
        .and_then(Value::as_str)
        .unwrap_or(crate::i18n::DEFAULT_LOCALE);
    Translator::new(locale)
}

#[derive(Deserialize)]
struct PowProof {
    algorithm: Option<String>,
    challenge: Option<String>,
    number: Option<Value>,
    salt: Option<String>,
    signature: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct PowValidation {
    nonce: String,
}

pub fn auth_api_routes() -> Router<AppState> {
    Router::new()
        .route("/bootstrap", get(bootstrap))
        .route("/session", get(session))
        .route("/captcha/config", get(captcha_config))
        .route("/challenge", get(challenge))
        .route("/ip", get(ip))
        .route("/ip/location", get(ip_location))
        .route("/login", post(login))
        .route("/logout", get(logout))
        .route("/preflight", head(preflight))
        .route("/verify", get(verify))
        .route("/oidc/providers", get(oidc_providers))
        .route("/oidc/invite", get(oidc_invite))
        .merge(passkey_routes())
        .merge(oidc_runtime_routes())
        .fallback(auth_api_not_found)
}
