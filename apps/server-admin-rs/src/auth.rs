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

async fn auth_api_not_found() -> Response {
    let translator = Translator::new(crate::i18n::DEFAULT_LOCALE);
    response::error(
        StatusCode::NOT_FOUND,
        auth_route_text(&translator, "pathNotFound"),
    )
}

async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<BootstrapQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let client_ip = client_ip_for_auth(&headers);
    enqueue_auth_ip_location(&state, &client_ip, "bootstrap");
    match build_auth_shell_data(&state, &headers, &uri, query.redirect_uri.as_deref(), true).await {
        Ok((mut data, access)) => {
            let mut clear_cookie = None;
            if let Ok(config) = state.redis.get_config().await
                && let Some((message, cookie)) =
                    consume_login_error_for_bootstrap(&state, &headers, &uri, &config).await
            {
                if let Some(oidc) = data.get_mut("oidc").and_then(Value::as_object_mut) {
                    oidc.insert("login_error".to_string(), Value::String(message));
                }
                clear_cookie = Some(cookie);
            }
            let mut response = with_auth_headers(response::ok(data).into_response());
            apply_auth_access_response_headers(response.headers_mut(), &access);
            if let Some(cookie) = clear_cookie
                && let Ok(value) = HeaderValue::from_str(&cookie)
            {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => {
            tracing::warn!(%error, "failed to build auth bootstrap data");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadBootstrapFailed"),
            ))
        }
    }
}

async fn session(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_auth_shell_data(&state, &headers, &uri, None, false).await {
        Ok((data, access)) => {
            if access.authenticated {
                let client_ip = client_ip_for_auth(&headers);
                enqueue_auth_ip_location(&state, &client_ip, "session");
            }
            let status = if access.authenticated {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            };
            let mut response = with_auth_headers(
                (
                    status,
                    Json(ApiEnvelope {
                        success: access.authenticated,
                        code: None,
                        message: if access.authenticated {
                            None
                        } else {
                            Some(auth_route_text(&translator, "authenticationRequired"))
                        },
                        data: if access.authenticated {
                            Some(data)
                        } else {
                            None
                        },
                    }),
                )
                    .into_response(),
            );
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load auth session");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadSessionFailed"),
            ))
        }
    }
}

async fn captcha_config(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match public_captcha_settings(&state).await {
        Ok(data) => with_auth_headers(response::ok(data).into_response()),
        Err(error) => {
            tracing::warn!(%error, "failed to load captcha config");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadCaptchaConfigFailed"),
            ))
        }
    }
}

async fn challenge(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load captcha config for challenge");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "createCaptchaChallengeFailed"),
            ));
        }
    };
    if config
        .pointer("/captcha/provider")
        .and_then(Value::as_str)
        .unwrap_or("pow")
        != "pow"
    {
        return with_auth_headers(response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            captcha_text(&translator, "powNotEnabled"),
        ));
    }
    let Some(key) = state
        .settings
        .altcha_hmac_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return with_auth_headers(response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            captcha_text(&translator, "powServerNotConfigured"),
        ));
    };

    let salt = hex::encode(random_bytes::<12>());
    let expires = time_utils::now_ms() / 1000 + 300;
    let salt_with_params = format!("{salt}?expires={expires}");
    let secret_number = pow_secret_number_from_random(rand::random::<u32>());
    let challenge = sha256_hex(format!("{salt_with_params}{secret_number}").as_bytes());
    let signature = match hmac_sha256_hex(key.as_bytes(), challenge.as_bytes()) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to sign captcha challenge");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "createCaptchaChallengeFailed"),
            ));
        }
    };

    with_auth_headers(
        Json(json!({
            "algorithm": "SHA-256",
            "challenge": challenge,
            "maxnumber": POW_MAX_NUMBER,
            "salt": salt_with_params,
            "signature": signature
        }))
        .into_response(),
    )
}

async fn ip(headers: HeaderMap) -> Response {
    with_auth_headers(response::ok(json!({ "ip": client_ip_for_auth(&headers) })).into_response())
}

async fn ip_location(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let ip = client_ip_for_auth(&headers);
    match ip_location::ensure_ip_location_enqueued(&state, &ip).await {
        Ok(snapshot) => {
            let mut data = json!({
                "ip": ip,
                "location": snapshot.get("location").cloned().unwrap_or_else(|| Value::String(String::new())),
                "status": snapshot.get("status").cloned().unwrap_or_else(|| Value::String("skipped".to_string())),
                "attempts": snapshot.get("attempts").cloned().unwrap_or_else(|| json!(0)),
                "maxAttempts": snapshot.get("maxAttempts").cloned().unwrap_or_else(|| json!(0))
            });
            if let Some(error) = snapshot.get("error") {
                data["error"] = error.clone();
            }
            with_auth_headers(response::ok(data).into_response())
        }
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to enqueue auth IP location lookup");
            let translator = Translator::from_state(&state).await;
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.ipLocationRoutes.enqueueFailed"),
            ))
        }
    }
}

async fn oidc_providers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match oidc_public_providers(&state).await {
        Ok(providers) => {
            with_auth_headers(response::ok(json!({ "providers": providers })).into_response())
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load public OIDC providers");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadOidcProvidersFailed"),
            ))
        }
    }
}

async fn oidc_invite(
    State(state): State<AppState>,
    Query(query): Query<OidcInviteQuery>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config for OIDC invite");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadOidcInviteFailed"),
            ));
        }
    };
    let translator = translator_from_config(&config);
    let locale = config
        .get("locale")
        .cloned()
        .unwrap_or_else(|| json!({ "default_locale": "zh-CN" }));
    let appearance = config
        .get("appearance")
        .cloned()
        .unwrap_or_else(|| json!({ "theme_color_preset": "default" }));
    let Some(token) = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return with_auth_headers(
            (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope {
                    success: false,
                    code: None,
                    message: Some(oidc_text(&translator, "inviteInvalid")),
                    data: Some(json!({ "locale": locale, "appearance": appearance })),
                }),
            )
                .into_response(),
        );
    };

    match oidc_inspect_invite(&state, token).await {
        Ok(Some(mut invite)) => {
            if let Some(object) = invite.as_object_mut() {
                object.insert("locale".to_string(), locale);
                object.insert("appearance".to_string(), appearance);
            }
            with_auth_headers(response::ok(invite).into_response())
        }
        Ok(None) => with_auth_headers(
            (
                StatusCode::NOT_FOUND,
                Json(ApiEnvelope {
                    success: false,
                    code: None,
                    message: Some(oidc_text(&translator, "inviteMissingExpiredUsed")),
                    data: Some(json!({ "locale": locale, "appearance": appearance })),
                }),
            )
                .into_response(),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to inspect OIDC invite");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "inspectOidcInviteFailed"),
            ))
        }
    }
}

#[axum::debug_handler(state = AppState)]
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    let client_ip = client_ip_for_auth(&headers);
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config during login");
            let translator = Translator::from_state(&state).await;
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadAuthConfigFailed"),
            ));
        }
    };
    let translator = translator_from_config(&config);

    match state.redis.get_login_backoff_status(&tracking_ip).await {
        Ok(status) if status.blocked => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            return with_auth_headers(backoff_login_response(
                &server_text_params(
                    &translator,
                    "tooManyAttemptsWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            ));
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to inspect auth login backoff");
        }
    }

    if let Err(message) =
        verify_captcha(&state, &config, &body.captcha, &client_ip, &translator).await
    {
        return with_auth_headers(response::error(StatusCode::BAD_REQUEST, message));
    }

    let totps = match state.redis.get_totps().await {
        Ok(totps) => totps,
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadLoginCredentialsFailed"),
            ));
        }
    };
    if totps.is_empty() {
        return with_auth_headers(response::error(
            StatusCode::BAD_REQUEST,
            server_text(&translator, "loginCredentialMissing"),
        ));
    }

    let Some(credential) = find_matching_totp(&totps, &body.token) else {
        match state
            .redis
            .register_login_backoff_failure(&tracking_ip)
            .await
        {
            Ok(status) => {
                let retry_after = status.retry_after.unwrap_or(1).max(1);
                if let Err(error) = system_events::publish_auth_login_failure_event(
                    &state,
                    json!({
                        "ip": tracking_ip.clone(),
                        "attempts": status.attempts,
                        "retry_after_seconds": retry_after,
                        "blocked_until": status.blocked_until.map(time_utils::iso_from_ms),
                        "method": "TOTP",
                        "credential_name": "! Unknown TOTP",
                        "user_agent": user_agent(&headers),
                    }),
                )
                .await
                {
                    tracing::warn!(%error, %tracking_ip, "failed to publish auth login failure event");
                }
                return with_auth_headers(backoff_login_response(
                    &server_text_params(
                        &translator,
                        "invalidOtpWithRetry",
                        &[("seconds", retry_after.to_string())],
                    ),
                    retry_after,
                    status.blocked_until,
                ));
            }
            Err(error) => {
                tracing::warn!(%error, %tracking_ip, "failed to register auth login failure");
                return with_auth_headers(response::error(
                    StatusCode::TOO_MANY_REQUESTS,
                    server_text_params(
                        &translator,
                        "invalidOtpWithRetry",
                        &[("seconds", "1".to_string())],
                    ),
                ));
            }
        }
    };

    let credential_name = credential_name(&credential, &translator);
    let passkey_info = if config
        .pointer("/auth_credential_settings/passkey_bind_prompt_enabled")
        .and_then(Value::as_bool)
        == Some(false)
    {
        None
    } else {
        match build_passkey_bind_info(&state, &credential.id).await {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(%error, totp_id = %credential.id, "failed to build passkey bind info");
                return with_auth_headers(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    auth_route_text(&translator, "createSessionFailed"),
                ));
            }
        }
    };
    let created = match auth_mobility::create_login_session(
        &state,
        &config,
        CreateLoginSessionInput {
            auth_method: "TOTP".to_string(),
            auth_provider_name: None,
            credential_id: credential.id.clone(),
            credential_name: credential_name.clone(),
            totp_id: credential.id.clone(),
            linked_totp_name: None,
            totp_credential: Some(credential.clone()),
            client_ip: client_ip.clone(),
            user_agent: user_agent(&headers),
            remember_me: body.remember_me,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to create auth session");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "createSessionFailed"),
            ));
        }
    };
    tracing::debug!(
        session_id = %created.session_id,
        grant_type = %created.grant_type,
        whitelist_record_id = ?created.whitelist_record_id,
        post_login_ip_grant_mode = ?created.post_login_ip_grant_mode,
        expires_at = %created.expires_at,
        session_comment = ?created.session_comment,
        "created auth session"
    );
    if created.ttl_seconds <= 0 {
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            auth_route_text(&translator, "createSessionFailed"),
        ));
    }
    if let Err(error) = state.redis.reset_login_backoff(&tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset auth login backoff after success");
    }

    let redirect_to = effective_login_redirect(
        &config,
        &headers,
        &created.grant_type,
        body.redirect_uri.as_deref(),
    );
    let cookie_domain = resolve_cookie_domain(&config, &headers);
    let cookie = cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        cookie_domain.as_deref(),
    );
    let mut data = json!({
        "run_type": config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        "grant_type": created.grant_type
    });
    if let Some(mut passkey_info) = passkey_info {
        if let Some(object) = passkey_info.as_object_mut() {
            object.remove("token");
        }
        data["passkey"] = passkey_info;
    }
    if let Some(redirect_to) = redirect_to {
        data["redirect_to"] = Value::String(redirect_to);
    }
    let mut response = (
        [(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
        )],
        Json(ApiEnvelope {
            success: true,
            code: None,
            message: Some(auth_route_text(&translator, "loginSuccessful")),
            data: Some(data),
        }),
    )
        .into_response();
    apply_no_store_headers(response.headers_mut());
    response
}

fn backoff_login_response(message: &str, retry_after: i64, blocked_until: Option<i64>) -> Response {
    let retry_after = retry_after.max(1);
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "success": false,
            "message": message,
            "retryAfter": retry_after,
            "blockedUntil": blocked_until
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&retry_after.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("1")),
    );
    apply_no_store_headers(response.headers_mut());
    response
}

async fn logout(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(%error, "failed to load config for logout");
            None
        }
    };
    let identity = inspect_auth_mobility_request(&headers);
    let session_id = identity.session_id;
    let mut session = None;
    let mut login_ip_from_session = None;
    if let Some(session_id) = session_id.as_deref() {
        session = state.redis.get_session(session_id).await.ok().flatten();
        login_ip_from_session = session.as_ref().map(|session| session.ip.clone());
        if let Err(error) = auth_mobility::destroy_session(&state, &session_id).await {
            tracing::warn!(%error, %session_id, "failed to cleanup auth mobility session on logout");
        }
        let _ = state.redis.delete_session(&session_id).await;
    }

    let client_ip = client_ip_for_auth(&headers);
    if session_id.is_none() {
        if let Err(error) =
            whitelist::remove_whitelist_records_by_ip(&state, &client_ip, Some("auto")).await
        {
            tracing::warn!(%error, %client_ip, "failed to remove auto whitelist records on logout without session");
        }
    } else if let Err(error) = revoke_custom_post_login_ip_grant(
        &state,
        session.as_ref(),
        config.as_ref(),
        login_ip_from_session.as_deref().unwrap_or(&client_ip),
    )
    .await
    {
        tracing::warn!(%error, "failed to revoke custom post-login IP grant on logout");
    }
    whitelist::sync_reverse_proxy_trusted_ips(&state).await;

    if let (Some(session_id), Some(session)) = (session_id.as_deref(), session.as_ref())
        && let Err(error) = system_events::publish_auth_logout_event(
            &state,
            json!({
                "session_id": session_id,
                "auth_method": session.method.clone(),
                "credential_id": session.credential_id.clone(),
                "credential_name": session.credential_name.clone(),
                "linked_totp_name": session.linked_totp_name.clone(),
                "session_comment": session.comment.clone(),
                "ip": session.ip.clone(),
                "ip_location": session.ip_location.clone(),
                "user_agent": session.user_agent.clone(),
                "login_time": session.login_time.clone(),
                "logout_source": "user_logout",
            }),
        )
        .await
    {
        tracing::warn!(%error, %session_id, "failed to publish auth logout event");
    }

    let cookie_domain = config
        .as_ref()
        .and_then(|config| resolve_cookie_domain(config, &headers));
    let mut response = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, post_logout_location(&headers, &uri))
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookies::session_clear_cookie(cookie_domain.as_deref()))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookies::fnos_share_clear_cookie(cookie_domain.as_deref()))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    response
}

async fn revoke_custom_post_login_ip_grant(
    state: &AppState,
    session: Option<&LoginSession>,
    config: Option<&Value>,
    fallback_ip: &str,
) -> anyhow::Result<bool> {
    let Some(config) = config else {
        return Ok(false);
    };
    if !should_revoke_custom_post_login_ip_grant(session, config) {
        return Ok(false);
    }
    if let Some(record_id) = session
        .and_then(|session| session.post_login_ip_grant_record_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return whitelist::remove_whitelist_record_by_id(state, record_id).await;
    }
    let ip = session
        .map(|session| session.ip.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_ip);
    whitelist::remove_whitelist_records_by_ip(state, ip, Some("auto")).await
}

fn should_revoke_custom_post_login_ip_grant(
    session: Option<&LoginSession>,
    config: &Value,
) -> bool {
    let Some(session) = session else {
        return false;
    };
    if session.grant_type.as_deref() == Some("login_ip_grant")
        && session.post_login_ip_grant_mode.as_deref() == Some("custom")
    {
        return true;
    }
    session
        .comment
        .as_deref()
        .is_some_and(auth_mobility::is_auto_ip_grant_comment)
        && config
            .pointer("/auth_credential_settings/post_login_ip_grant_mode")
            .and_then(Value::as_str)
            == Some("custom")
}

async fn preflight(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());

    if let Err(error) = apply_preflight_behavior(&state, &headers, &uri, &mut response).await {
        let client_ip = client_ip_for_auth(&headers);
        let forwarded_path = preflight_forwarded_path(&headers);
        tracing::warn!(%error, %client_ip, %forwarded_path, "auth preflight failed");
    }
    response
}

async fn apply_preflight_behavior(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    response: &mut Response,
) -> anyhow::Result<()> {
    let client_ip = client_ip_for_auth(headers);
    let forwarded_path = preflight_forwarded_path(headers);
    let access_mode = requested_access_mode(headers);
    let config = state.redis.get_config().await?;
    let mut share_decision_handled = false;

    let normal_access =
        resolve_preflight_normal_access(state, headers, uri, &config, &client_ip, access_mode)
            .await?;
    if normal_access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        insert_preflight_headers(response, &normal_access.response_headers);
        response.headers_mut().insert(
            REAUTH_ACCESS_DENIED_HEADER,
            HeaderValue::from_static(REAUTH_SCOPE_DENIED),
        );
    } else if access_mode == RequestedAccessMode::StrictWhitelist
        && !has_preflight_whitelist_access(state, &client_ip).await?
    {
        response
            .headers_mut()
            .insert("X-Option", HeaderValue::from_static("Deny"));
    } else if !normal_access.authorized {
        let decision = fnos_share_bypass::resolve_preflight(state, headers, uri, &config).await?;
        share_decision_handled = decision.handled;
        if let Some(location) = decision.redirect_location {
            insert_header_value(response, "X-Reauth-Redirect-Location", &location);
        }
    }

    if config.get("run_type").and_then(Value::as_i64).unwrap_or(0) != 0
        && !scanner::is_request_exempt_from_scan(headers, uri, &config)
    {
        if scanner::is_blacklisted_for_preflight(state, &client_ip).await? {
            response
                .headers_mut()
                .insert("X-Option", HeaderValue::from_static("Deny"));
        } else if !state
            .redis
            .is_recent_auth_ip_active(&client_ip, time_utils::now_ms() / 1000)
            .await?
            && !share_decision_handled
            && !forwarded_path.is_empty()
            && !scanner::is_common_path_for_preflight(state, &forwarded_path).await?
        {
            let _ = scanner::record_uncommon_path_for_preflight(state, &client_ip, &forwarded_path)
                .await?;
        }
    }

    Ok(())
}

fn insert_preflight_headers(response: &mut Response, values: &[(String, String)]) {
    for (key, value) in values {
        insert_header_value(response, key, value);
    }
}

fn insert_header_value(response: &mut Response, key: &str, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_bytes(key.as_bytes())
                .unwrap_or_else(|_| axum::http::HeaderName::from_static("x-ignored-invalid")),
            header_value,
        );
    }
}

fn apply_auth_access_response_headers(headers: &mut HeaderMap, access: &AuthAccess) {
    for cookie in &access.set_cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.append(header::SET_COOKIE, value);
        }
    }
    for (key, value) in &access.response_headers {
        if let Ok(header_value) = HeaderValue::from_str(value) {
            headers.insert(
                axum::http::HeaderName::from_bytes(key.as_bytes())
                    .unwrap_or_else(|_| axum::http::HeaderName::from_static("x-ignored-invalid")),
                header_value,
            );
        }
    }
}

fn preflight_forwarded_path(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-path")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedAccessMode {
    LoginFirst,
    StrictWhitelist,
}

fn requested_access_mode(headers: &HeaderMap) -> RequestedAccessMode {
    headers
        .get("x-reauth-access-mode")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.trim().eq_ignore_ascii_case("strict_whitelist"))
        .map(|_| RequestedAccessMode::StrictWhitelist)
        .unwrap_or(RequestedAccessMode::LoginFirst)
}

#[cfg(test)]
fn is_strict_whitelist_request(headers: &HeaderMap) -> bool {
    requested_access_mode(headers) == RequestedAccessMode::StrictWhitelist
}

#[derive(Debug, Default)]
struct PreflightNormalAccess {
    authorized: bool,
    grant_type: Option<String>,
    deny_reason: Option<String>,
    response_headers: Vec<(String, String)>,
}

#[derive(Debug)]
struct SessionSubdomainAccessDecision {
    protected_host: bool,
    allowed: bool,
    response_headers: Vec<(String, String)>,
}

#[derive(Debug, Default)]
struct AuthMobilityRequestIdentity {
    session_id: Option<String>,
    fnos_token: Option<String>,
    trim_media_token: Option<String>,
    app_binding: Option<&'static str>,
}

impl AuthMobilityRequestIdentity {
    fn has_app_mobility_signal(&self) -> bool {
        self.fnos_token.is_some() || self.trim_media_token.is_some() || self.app_binding.is_some()
    }
}

#[derive(Debug)]
struct MobilitySubdomainAccessDecision {
    protected_host: bool,
    has_owner_session: bool,
    allowed: bool,
    response_headers: Vec<(String, String)>,
}

async fn resolve_preflight_normal_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    client_ip: &str,
    access_mode: RequestedAccessMode,
) -> anyhow::Result<PreflightNormalAccess> {
    if http_utils::is_private_or_local_ip(client_ip) {
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("local_exempt".to_string()),
            ..Default::default()
        });
    }

    if has_preflight_whitelist_access_from_sources(state, client_ip, Some(&["manual"])).await? {
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("manual_whitelist".to_string()),
            ..Default::default()
        });
    }

    let identity = inspect_auth_mobility_request(headers);
    let mut session_scope_headers = Vec::new();
    let browser_session = if let Some(session_id) = identity.session_id.as_deref() {
        match state.redis.get_session(session_id).await? {
            Some(session) => {
                let scope =
                    resolve_session_subdomain_access(state, headers, uri, config, &session).await?;
                if !scope.allowed {
                    return Ok(PreflightNormalAccess {
                        authorized: false,
                        deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                        response_headers: scope.response_headers,
                        ..Default::default()
                    });
                }
                session_scope_headers = scope.response_headers;
                Some((session_id.to_string(), session))
            }
            None => None,
        }
    } else {
        None
    };

    if identity.has_app_mobility_signal() {
        let mobility =
            resolve_mobility_subdomain_access(state, headers, uri, config, client_ip, &identity)
                .await?;
        if mobility.protected_host && mobility.has_owner_session && !mobility.allowed {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
        session_scope_headers = mobility.response_headers;
    }

    let restored = auth_mobility::try_restore_access(
        state,
        client_ip,
        auth_mobility::AuthMobilityRestoreIdentity {
            session_id: identity.session_id.as_deref(),
            fnos_token: identity.fnos_token.as_deref(),
            trim_media_token: identity.trim_media_token.as_deref(),
            app_binding: identity.app_binding,
        },
    )
    .await?;
    if restored.success {
        let mobility =
            resolve_mobility_subdomain_access(state, headers, uri, config, client_ip, &identity)
                .await?;
        if mobility.protected_host && (!mobility.has_owner_session || !mobility.allowed) {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
        if access_mode != RequestedAccessMode::StrictWhitelist {
            return Ok(PreflightNormalAccess {
                authorized: true,
                grant_type: restored
                    .grant_type
                    .map(ToString::to_string)
                    .or_else(|| Some("browser_session".to_string())),
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
    }

    if let Some((session_id, _session)) = browser_session.as_ref() {
        if let Err(error) =
            auth_mobility::sync_browser_session_ip(state, session_id, client_ip, "browser-session")
                .await
        {
            tracing::warn!(%error, %session_id, "failed to sync browser session IP");
        }

        if access_mode != RequestedAccessMode::StrictWhitelist {
            return Ok(PreflightNormalAccess {
                authorized: true,
                grant_type: Some("browser_session".to_string()),
                response_headers: session_scope_headers,
                ..Default::default()
            });
        }
    }

    if has_preflight_whitelist_access_from_sources(state, client_ip, Some(&["auto"])).await? {
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("login_ip_grant".to_string()),
            ..Default::default()
        });
    }

    if access_mode != RequestedAccessMode::StrictWhitelist {
        if identity.has_app_mobility_signal() {
            let mobility = resolve_mobility_subdomain_access(
                state, headers, uri, config, client_ip, &identity,
            )
            .await?;
            if has_resolvable_auth_mobility_access(state, client_ip, &identity).await? {
                if mobility.protected_host && (!mobility.has_owner_session || !mobility.allowed) {
                    return Ok(PreflightNormalAccess {
                        authorized: false,
                        deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                        response_headers: mobility.response_headers,
                        ..Default::default()
                    });
                }
                return Ok(PreflightNormalAccess {
                    authorized: true,
                    grant_type: Some("fnos_fingerprint_session".to_string()),
                    response_headers: mobility.response_headers,
                    ..Default::default()
                });
            }
        }
    }

    Ok(PreflightNormalAccess {
        authorized: false,
        ..Default::default()
    })
}

async fn has_preflight_whitelist_access(state: &AppState, client_ip: &str) -> anyhow::Result<bool> {
    has_preflight_whitelist_access_from_sources(state, client_ip, None).await
}

async fn has_preflight_whitelist_access_from_sources(
    state: &AppState,
    client_ip: &str,
    sources: Option<&[&str]>,
) -> anyhow::Result<bool> {
    let normalized_ip = http_utils::normalize_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(false);
    }
    if http_utils::is_private_or_local_ip(&normalized_ip) {
        return Ok(true);
    }

    let client_ip = normalized_ip.parse::<IpAddr>()?;
    let targets = state.redis.list_whitelist_active_concrete_targets().await?;
    Ok(targets.iter().any(|target| {
        sources.is_none_or(|sources| sources.contains(&target.source.as_str()))
            && whitelist_target_matches_ip(&target.target, &target.target_type, client_ip)
    }))
}

fn whitelist_target_matches_ip(target: &str, target_type: &str, client_ip: IpAddr) -> bool {
    if target_type == "cidr" {
        return target
            .trim()
            .parse::<IpNet>()
            .is_ok_and(|network| network.contains(&client_ip));
    }

    http_utils::normalize_ip(target)
        .parse::<IpAddr>()
        .is_ok_and(|target_ip| target_ip == client_ip)
}

fn inspect_auth_mobility_request(headers: &HeaderMap) -> AuthMobilityRequestIdentity {
    let forwarded_pathname = normalize_forwarded_pathname(
        headers
            .get("x-forwarded-path")
            .and_then(|value| value.to_str().ok()),
    );
    let cookie_header = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let session_id = parse_auth_mobility_cookie_value(cookie_header, cookies::SESSION_COOKIE_NAME);
    let fnos_token = parse_auth_mobility_cookie_value(cookie_header, "fnos-token");
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let app_binding = resolve_auth_mobility_app_binding(
        user_agent,
        &forwarded_pathname,
        cookie_header,
        &fnos_token,
    );
    let trim_media_token = if app_binding == Some("trim-media-app") {
        ["authorization", "accesstoken", "access-token"]
            .iter()
            .filter_map(|name| headers.get(*name).and_then(|value| value.to_str().ok()))
            .find_map(parse_auth_mobility_header_token)
    } else {
        None
    };
    AuthMobilityRequestIdentity {
        session_id,
        fnos_token,
        trim_media_token,
        app_binding,
    }
}

fn parse_auth_mobility_cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    let mut last_value = None;
    for segment in cookie_header.split(';') {
        let (raw_key, raw_value) = match segment.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if raw_key.trim() != name {
            continue;
        }
        let value = raw_value.trim().trim_matches('"');
        if value.is_empty() {
            continue;
        }
        last_value = Some(cookies::percent_decode(value));
    }
    last_value
}

async fn has_resolvable_auth_mobility_access(
    state: &AppState,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<bool> {
    if let Some(token) = identity.fnos_token.as_deref()
        && let Some((_owner_id, owner_session)) =
            auth_mobility_binding_owner_session(state, "fnos-token", token).await?
        && auth_mobility_session_has_remaining_ttl(&owner_session)
    {
        return Ok(true);
    }
    if let Some(token) = identity.trim_media_token.as_deref()
        && let Some((_owner_id, owner_session)) =
            auth_mobility_binding_owner_session(state, "trim-media-token", token).await?
        && auth_mobility_session_has_remaining_ttl(&owner_session)
    {
        return Ok(true);
    }
    match identity.app_binding {
        Some("fnos-app") => Ok(list_auth_mobility_owner_sessions_by_ip(state, client_ip)
            .await?
            .len()
            == 1),
        Some("trim-media-app") => Ok(!list_auth_mobility_owner_sessions_by_ip(state, client_ip)
            .await?
            .is_empty()),
        _ => Ok(false),
    }
}

async fn resolve_mobility_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<MobilitySubdomainAccessDecision> {
    let host = normalize_subdomain_access_host(&resolve_request_subdomain_access_key(headers, uri));
    if host.is_empty() {
        return Ok(MobilitySubdomainAccessDecision {
            protected_host: false,
            has_owner_session: false,
            allowed: true,
            response_headers: Vec::new(),
        });
    }

    let owners = resolve_auth_mobility_owner_sessions(state, client_ip, identity).await?;
    if owners.is_empty() {
        let protected_host = is_protected_subdomain_auth_host(&host, config);
        return Ok(MobilitySubdomainAccessDecision {
            protected_host,
            has_owner_session: false,
            allowed: !protected_host,
            response_headers: Vec::new(),
        });
    }

    let mut protected_host = false;
    let mut denied_response_headers = Vec::new();
    for (_owner_session_id, owner_session) in owners {
        let decision =
            resolve_session_subdomain_access(state, headers, uri, config, &owner_session).await?;
        protected_host |= decision.protected_host;
        if decision.protected_host && decision.allowed {
            return Ok(MobilitySubdomainAccessDecision {
                protected_host: true,
                has_owner_session: true,
                allowed: true,
                response_headers: decision.response_headers,
            });
        }
        if decision.protected_host && !decision.allowed && denied_response_headers.is_empty() {
            denied_response_headers = decision.response_headers;
        }
    }

    Ok(MobilitySubdomainAccessDecision {
        protected_host,
        has_owner_session: true,
        allowed: !protected_host,
        response_headers: denied_response_headers,
    })
}

async fn resolve_auth_mobility_owner_sessions(
    state: &AppState,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let mut owners = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(session_id) = identity.session_id.as_deref()
        && let Some(session) = state.redis.get_session(session_id).await?
        && seen.insert(session_id.to_string())
    {
        owners.push((session_id.to_string(), session));
    }
    if let Some(token) = identity.fnos_token.as_deref()
        && let Some(owner) = auth_mobility_binding_owner_session(state, "fnos-token", token).await?
        && seen.insert(owner.0.clone())
    {
        owners.push(owner);
    }
    if let Some(token) = identity.trim_media_token.as_deref()
        && let Some(owner) =
            auth_mobility_binding_owner_session(state, "trim-media-token", token).await?
        && seen.insert(owner.0.clone())
    {
        owners.push(owner);
    }
    match identity.app_binding {
        Some("fnos-app") => {
            let sessions = list_auth_mobility_owner_sessions_by_ip(state, client_ip).await?;
            if sessions.len() == 1
                && let Some(owner) = sessions.into_iter().next()
                && seen.insert(owner.0.clone())
            {
                owners.push(owner);
            }
        }
        Some("trim-media-app") => {
            for owner in list_auth_mobility_owner_sessions_by_ip(state, client_ip).await? {
                if seen.insert(owner.0.clone()) {
                    owners.push(owner);
                }
            }
        }
        _ => {}
    }
    Ok(owners)
}

async fn auth_mobility_binding_owner_session(
    state: &AppState,
    subject_type: &str,
    subject_key: &str,
) -> anyhow::Result<Option<(String, LoginSession)>> {
    let Some(binding) = state
        .redis
        .get_auth_mobility_binding(subject_type, subject_key)
        .await?
    else {
        return Ok(None);
    };
    let owner_session_id = binding
        .get("ownerSessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(owner_session_id) = owner_session_id else {
        return Ok(None);
    };
    Ok(state
        .redis
        .get_session(owner_session_id)
        .await?
        .map(|session| (owner_session_id.to_string(), session)))
}

fn auth_mobility_session_has_remaining_ttl(session: &LoginSession) -> bool {
    let Some(expires_at) = session.expires_at.as_deref() else {
        return false;
    };
    let Some(expire_ms) = time_utils::parse_iso_ms(expires_at) else {
        return false;
    };
    expire_ms.div_euclid(1000) > time_utils::now_ms().div_euclid(1000)
}

async fn list_auth_mobility_owner_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let normalized_ip = http_utils::normalize_ip(client_ip);
    let target_ip = if normalized_ip.is_empty() {
        client_ip.trim().to_string()
    } else {
        normalized_ip
    };
    if target_ip.is_empty() {
        return Ok(Vec::new());
    }

    let config = state.redis.get_config().await?;
    let mut owners = Vec::new();
    for (session_id, session) in state.redis.list_login_sessions().await? {
        let ips =
            auth_mobility::effective_session_ips(state, &session_id, &session, &config).await?;
        if ips.iter().any(|ip| ip == &target_ip) {
            owners.push((session_id, session));
        }
    }
    Ok(owners)
}

fn normalize_forwarded_pathname(raw_path: Option<&str>) -> String {
    let value = raw_path.map(str::trim).unwrap_or("");
    if value.is_empty() {
        return String::new();
    }
    let base = url::Url::parse("http://localhost").ok();
    if let Some(base) = base
        && let Ok(parsed) = url::Url::options().base_url(Some(&base)).parse(value)
    {
        return parsed.path().to_string();
    }
    let pathname = value.split('?').next().unwrap_or("");
    if pathname.is_empty() {
        String::new()
    } else if pathname.starts_with('/') {
        pathname.to_string()
    } else {
        format!("/{pathname}")
    }
}

fn resolve_auth_mobility_app_binding(
    user_agent: &str,
    forwarded_pathname: &str,
    cookie_header: &str,
    fnos_token: &Option<String>,
) -> Option<&'static str> {
    let normalized_user_agent = user_agent.trim().to_ascii_lowercase();
    if normalized_user_agent.contains("com.trim.media") {
        return Some("trim-media-app");
    }

    let is_fnos_app_user_agent = normalized_user_agent.contains("com.trim.app")
        || normalized_user_agent.contains("dart:io")
        || normalized_user_agent.contains("flutter/");
    let is_fnos_app_path = forwarded_pathname == "/trimcon" || forwarded_pathname == "/websocket";
    let has_relay_cookie = cookie_header.to_ascii_lowercase().contains("mode=relay");
    (is_fnos_app_path && (is_fnos_app_user_agent || has_relay_cookie || fnos_token.is_some()))
        .then_some("fnos-app")
}

fn parse_auth_mobility_header_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("bearer ")
        .or_else(|| lower.strip_prefix("token "))
    {
        let start = trimmed.len() - rest.len();
        let token = trimmed[start..].trim();
        return (!token.is_empty()).then(|| token.to_string());
    }
    Some(trimmed.to_string())
}

async fn resolve_session_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    session: &LoginSession,
) -> anyhow::Result<SessionSubdomainAccessDecision> {
    let totps = state.redis.get_totps().await?;
    let credential = totps
        .iter()
        .find(|credential| credential.id == session.totp_id);
    let host = resolve_request_subdomain_access_key(headers, uri);
    let normalized_host = normalize_subdomain_access_host(&host);
    let protected_host = is_protected_subdomain_auth_host(&normalized_host, config);
    let allowed = if !protected_host {
        true
    } else {
        credential.is_some_and(|credential| {
            is_host_allowed_by_totp_subdomain_access(&credential.subdomain_access, &normalized_host)
        })
    };

    let mut response_headers = build_session_credential_response_headers(session);
    if let Some(credential) = credential {
        response_headers.extend(build_credential_subdomain_access_response_headers(
            credential,
        ));
    }

    Ok(SessionSubdomainAccessDecision {
        protected_host,
        allowed,
        response_headers,
    })
}

fn resolve_request_subdomain_access_key(headers: &HeaderMap, uri: &Uri) -> String {
    let pathname = resolve_forwarded_request_pathname(headers, uri);
    if pathname == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
    }
    if is_auth_service_request_pathname(&pathname) {
        return String::new();
    }
    resolve_request_hostname(headers, uri)
}

fn resolve_forwarded_request_pathname(headers: &HeaderMap, uri: &Uri) -> String {
    let raw_path = headers
        .get("x-forwarded-path")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uri.path().to_string());
    if raw_path.is_empty() {
        return String::new();
    }
    let base = url::Url::parse("https://fn-knock.internal").ok();
    if let Some(base) = base
        && let Ok(parsed) = url::Url::options().base_url(Some(&base)).parse(&raw_path)
    {
        return parsed.path().to_string();
    }
    raw_path.split(['?', '#']).next().unwrap_or("").to_string()
}

fn is_auth_service_request_pathname(pathname: &str) -> bool {
    ["/__auth__", "/auth", "/api/auth"]
        .iter()
        .any(|prefix| pathname == *prefix || pathname.starts_with(&format!("{prefix}/")))
}

fn resolve_request_hostname(headers: &HeaderMap, uri: &Uri) -> String {
    extract_hostname(
        parse_forwarded_header_host(headers)
            .or_else(|| first_header_value(headers, "x-forwarded-host"))
            .or_else(|| first_header_value(headers, "x-original-host"))
            .or_else(|| first_header_value(headers, "host"))
            .or_else(|| {
                uri.authority()
                    .map(|authority| authority.as_str().to_string())
            })
            .as_deref()
            .unwrap_or(""),
    )
}

fn parse_forwarded_header_host(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    for segment in first.split(';') {
        let Some((key, value)) = segment.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("host") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn first_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn extract_hostname(value: &str) -> String {
    normalize_subdomain_access_host(value)
}

fn normalize_subdomain_access_host(value: &str) -> String {
    let mut host = value.trim().to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE || host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
    }

    if let Ok(url) = if host.contains("://") {
        url::Url::parse(&host)
    } else {
        url::Url::parse(&format!("https://{host}"))
    } {
        host = url.host_str().unwrap_or("").to_string();
    } else {
        if let Some((_, rest)) = host.split_once("://") {
            host = rest.to_string();
        }
        if let Some((_, rest)) = host.rsplit_once('@') {
            host = rest.to_string();
        }
        host = host
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if host.starts_with('[') {
            if let Some(end) = host.find(']') {
                host = host[1..end].to_string();
            }
        } else if host.matches(':').count() == 1
            && let Some((without_port, _)) = host.rsplit_once(':')
        {
            host = without_port.to_string();
        }
    }

    host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.contains('*')
        || host
            .chars()
            .any(|value| value.is_whitespace() || value == ',')
    {
        return String::new();
    }
    host
}

fn is_protected_subdomain_auth_host(host: &str, config: &Value) -> bool {
    if host.is_empty() {
        return false;
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE {
        return true;
    }
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| {
            mapping.get("service_role").and_then(Value::as_str) != Some("auth")
                && mapping.get("use_auth").and_then(Value::as_bool) == Some(true)
                && mapping
                    .get("host")
                    .and_then(Value::as_str)
                    .map(normalize_subdomain_access_host)
                    == Some(host.to_string())
        })
}

fn is_host_allowed_by_totp_subdomain_access(access: &Value, host: &str) -> bool {
    let mode = access.get("mode").and_then(Value::as_str).unwrap_or("all");
    if mode != "custom" {
        return true;
    }
    !host.is_empty()
        && access
            .get("hosts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(normalize_subdomain_access_host)
            .any(|candidate| candidate == host)
}

fn build_credential_subdomain_access_response_headers(
    credential: &TotpCredential,
) -> Vec<(String, String)> {
    let mode = credential
        .subdomain_access
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("all");
    if mode != "custom" {
        return Vec::new();
    }
    let hosts = credential
        .subdomain_access
        .get("hosts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_subdomain_access_host)
        .filter(|host| !host.is_empty() && host != TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE)
        .collect::<Vec<_>>()
        .join(",");
    vec![
        (
            REAUTH_SUBDOMAIN_ACCESS_HEADER.to_string(),
            REAUTH_SUBDOMAIN_ACCESS_CUSTOM.to_string(),
        ),
        (REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER.to_string(), hosts),
    ]
}

fn build_session_credential_response_headers(session: &LoginSession) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_ID_HEADER,
        &session.credential_id,
    );
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_NAME_HEADER,
        if session.credential_name.trim().is_empty() {
            session.comment.as_deref().unwrap_or("")
        } else {
            &session.credential_name
        },
    );
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_METHOD_HEADER,
        &session.method,
    );
    push_credential_header(&mut headers, REAUTH_LINKED_TOTP_ID_HEADER, &session.totp_id);
    push_credential_header(
        &mut headers,
        REAUTH_LINKED_TOTP_NAME_HEADER,
        session.linked_totp_name.as_deref().unwrap_or(""),
    );
    headers
}

fn push_credential_header(headers: &mut Vec<(String, String)>, key: &str, value: &str) {
    let normalized = normalize_credential_header_value(value);
    if normalized.is_empty() {
        return;
    }
    headers.push((
        key.to_string(),
        format!(
            "{AUTH_IDENTITY_HEADER_ENCODING_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(normalized.as_bytes())
        ),
    ));
}

fn normalize_credential_header_value(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .trim()
        .chars()
        .take(AUTH_IDENTITY_HEADER_MAX_LENGTH)
        .collect()
}

async fn verify(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let translator = Translator::from_state(&state).await;
    match resolve_auth_access(&state, &headers, &uri, &translator).await {
        Ok(access) if access.authenticated => {
            let mut response = with_auth_headers(
                response::success_message(access.message.clone()).into_response(),
            );
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Ok(access) => {
            let status = auth_verify_denied_status(&access);
            let mut response = with_auth_headers(response::error(status, access.message.clone()));
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "auth verify failed");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "verifyFailed"),
            ))
        }
    }
}

fn auth_verify_denied_status(access: &AuthAccess) -> StatusCode {
    if access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::UNAUTHORIZED
    }
}

async fn build_auth_shell_data(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    redirect_uri: Option<&str>,
    include_redirect: bool,
) -> anyhow::Result<(Value, AuthAccess)> {
    let config = state.redis.get_config().await?;
    let locale = config
        .get("locale")
        .cloned()
        .unwrap_or_else(|| json!({ "default_locale": "zh-CN" }));
    let translator = translator_from_config(&config);
    let appearance = config
        .get("appearance")
        .cloned()
        .unwrap_or_else(|| json!({ "theme_color_preset": "default" }));
    let access = resolve_auth_access(state, headers, uri, &translator).await?;
    let client_ip = client_ip_for_auth(headers);
    let oidc_providers = oidc_public_providers(state).await.unwrap_or_default();
    let passkey = public_passkey_status(state, headers, &config).await;
    let mut data = json!({
        "locale": locale,
        "appearance": appearance,
        "auth": {
            "authenticated": access.authenticated,
            "message": access.message,
            "grant_type": access.grant_type
        },
        "client": { "ip": client_ip },
        "captcha": public_captcha_settings_from_config(state, &config, &translator),
        "passkey": passkey,
        "oidc": { "providers": oidc_providers }
    });

    if include_redirect {
        let redirect_to = if access.authenticated {
            effective_login_redirect(
                &config,
                headers,
                access.grant_type.as_deref().unwrap_or_default(),
                redirect_uri,
            )
        } else {
            resolve_shared_auth_login_redirect(&config, headers, redirect_uri)
        };
        if let Some(value) = redirect_to {
            data["redirect_to"] = Value::String(value);
        }
    }
    Ok((data, access))
}

#[derive(Debug)]
struct AuthAccess {
    authenticated: bool,
    message: String,
    grant_type: Option<String>,
    deny_reason: Option<String>,
    set_cookies: Vec<String>,
    response_headers: Vec<(String, String)>,
}

async fn resolve_auth_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
) -> anyhow::Result<AuthAccess> {
    let client_ip = client_ip_for_auth(headers);
    let config = state.redis.get_config().await?;
    let access_mode = requested_access_mode(headers);
    let normal_access =
        resolve_preflight_normal_access(state, headers, uri, &config, &client_ip, access_mode)
            .await?;
    if normal_access.authorized {
        let identity = inspect_auth_mobility_request(headers);
        if let Err(error) = auth_mobility::sync_trusted_request(
            state,
            &client_ip,
            auth_mobility::AuthMobilityRestoreIdentity {
                session_id: identity.session_id.as_deref(),
                fnos_token: identity.fnos_token.as_deref(),
                trim_media_token: identity.trim_media_token.as_deref(),
                app_binding: identity.app_binding,
            },
        )
        .await
        {
            tracing::warn!(%error, %client_ip, "failed to sync trusted auth mobility request");
        }
        if let Err(error) =
            common_auth_locations::record_recent_verified_ip(state, &client_ip).await
        {
            tracing::debug!(%error, %client_ip, "failed to record recent verified auth IP");
        }
        let grant_type = normal_access.grant_type.clone();
        let message = match grant_type.as_deref() {
            Some("local_exempt") => auth_route_text(translator, "localNetworkAccessAllowed"),
            _ => auth_route_text(translator, "authenticated"),
        };
        return Ok(AuthAccess {
            authenticated: true,
            message,
            grant_type,
            deny_reason: None,
            set_cookies: Vec::new(),
            response_headers: normal_access.response_headers,
        });
    }
    if normal_access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        let mut response_headers = normal_access.response_headers;
        if !response_headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(REAUTH_ACCESS_DENIED_HEADER))
        {
            response_headers.push((
                REAUTH_ACCESS_DENIED_HEADER.to_string(),
                REAUTH_SCOPE_DENIED.to_string(),
            ));
        }
        return Ok(AuthAccess {
            authenticated: false,
            message: "Access denied by credential scope".to_string(),
            grant_type: None,
            deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
            set_cookies: Vec::new(),
            response_headers,
        });
    }

    let share_access = fnos_share_bypass::authorize(state, headers, uri, &config).await?;
    if share_access.authorized {
        return Ok(AuthAccess {
            authenticated: true,
            message: auth_route_text(translator, "authenticated"),
            grant_type: Some("fnos_share".to_string()),
            deny_reason: None,
            set_cookies: share_access.set_cookies,
            response_headers: share_access.response_headers,
        });
    }
    if !share_access.set_cookies.is_empty() || !share_access.response_headers.is_empty() {
        return Ok(AuthAccess {
            authenticated: false,
            message: auth_route_text(translator, "authenticationRequired"),
            grant_type: None,
            deny_reason: None,
            set_cookies: share_access.set_cookies,
            response_headers: share_access.response_headers,
        });
    }

    Ok(AuthAccess {
        authenticated: false,
        message: auth_route_text(translator, "authenticationRequired"),
        grant_type: None,
        deny_reason: None,
        set_cookies: Vec::new(),
        response_headers: Vec::new(),
    })
}

async fn public_captcha_settings(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let translator = translator_from_config(&config);
    Ok(public_captcha_settings_from_config(
        state,
        &config,
        &translator,
    ))
}

fn public_captcha_settings_from_config(
    state: &AppState,
    config: &Value,
    translator: &Translator,
) -> Value {
    let captcha = config.get("captcha").cloned().unwrap_or_else(|| {
        json!({
            "provider": "pow",
            "widget_mode": "normal",
            "pow": {},
            "turnstile": { "site_key": "", "secret_key": "" }
        })
    });
    let provider = captcha
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("pow");
    let site_key = captcha
        .pointer("/turnstile/site_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    let turnstile_secret = captcha
        .pointer("/turnstile/secret_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (available, unavailable_reason) = match provider {
        "pow"
            if state
                .settings
                .altcha_hmac_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none() =>
        {
            (
                false,
                Some(captcha_text(translator, "powServerNotConfigured")),
            )
        }
        "turnstile" if site_key.trim().is_empty() || turnstile_secret.trim().is_empty() => (
            false,
            Some(captcha_text(translator, "turnstileNotConfigured")),
        ),
        "pow" | "turnstile" => (true, None),
        _ => (false, Some(captcha_text(translator, "providerUnavailable"))),
    };

    json!({
        "provider": provider,
        "widget_mode": "normal",
        "available": available,
        "unavailable_reason": unavailable_reason,
        "pow": {},
        "turnstile": { "site_key": site_key }
    })
}

async fn verify_captcha(
    state: &AppState,
    config: &Value,
    submission: &CaptchaSubmission,
    client_ip: &str,
    translator: &Translator,
) -> Result<(), String> {
    let settings = config
        .get("captcha")
        .cloned()
        .unwrap_or_else(|| json!({ "provider": "pow" }));
    let provider = settings
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("pow");
    let submitted_provider = captcha_submission_provider(submission);
    if provider != submitted_provider {
        return Err(captcha_text(translator, "providerConfigMismatch"));
    }

    match (provider, submission) {
        ("pow", CaptchaSubmission::Pow { proof }) => {
            verify_pow_captcha(state, proof, translator).await
        }
        ("turnstile", CaptchaSubmission::Turnstile { token }) => {
            verify_turnstile_captcha(state, &settings, token, client_ip, translator).await
        }
        _ => Err(captcha_text(translator, "providerUnavailable")),
    }
}

fn captcha_submission_provider(submission: &CaptchaSubmission) -> &'static str {
    match submission {
        CaptchaSubmission::Pow { .. } => "pow",
        CaptchaSubmission::Turnstile { .. } => "turnstile",
    }
}

async fn verify_pow_captcha(
    state: &AppState,
    proof: &str,
    translator: &Translator,
) -> Result<(), String> {
    let Some(key) = state
        .settings
        .altcha_hmac_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(captcha_text(translator, "powServerNotConfigured"));
    };
    let decoded = BASE64_STANDARD
        .decode(proof)
        .map_err(|_| auth_route_text(translator, "invalidCaptchaProof"))?;
    let data: PowProof = serde_json::from_slice(&decoded)
        .map_err(|_| auth_route_text(translator, "invalidCaptchaProof"))?;
    let validation = validate_pow_proof(data, key, time_utils::now_ms() / 1000, translator)?;
    match state
        .redis
        .set_nonce_if_not_exists(&validation.nonce, 86_400)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(auth_route_text(translator, "captchaChallengeAlreadyUsed")),
        Err(error) => {
            tracing::warn!(%error, "failed to store captcha nonce");
            Err(auth_route_text(translator, "captchaVerifyFailed"))
        }
    }
}

fn validate_pow_proof(
    data: PowProof,
    key: &str,
    now_seconds: i64,
    translator: &Translator,
) -> Result<PowValidation, String> {
    if data.algorithm.as_deref() != Some("SHA-256") {
        return Err(auth_route_text(translator, "invalidCaptchaAlgorithm"));
    }

    let raw_challenge = data.challenge.unwrap_or_default();
    let challenge = raw_challenge.to_ascii_lowercase();
    let number = pow_number_text(data.number.as_ref());
    let salt = data.salt.unwrap_or_default();
    let signature = data.signature.unwrap_or_default().to_ascii_lowercase();
    let expected_challenge = sha256_hex(format!("{salt}{number}").as_bytes());
    if expected_challenge
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(auth_route_text(translator, "invalidCaptchaChallenge"));
    }

    let expected_signature = hmac_sha256_hex(key.as_bytes(), raw_challenge.as_bytes())
        .map_err(|_| auth_route_text(translator, "invalidCaptchaSignature"))?;
    if expected_signature
        .as_bytes()
        .ct_eq(signature.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(auth_route_text(translator, "invalidCaptchaSignature"));
    }

    if let Some(expires) = parse_pow_expires(&salt) {
        if now_seconds > expires {
            return Err(auth_route_text(translator, "captchaChallengeExpired"));
        }
    }

    Ok(PowValidation {
        nonce: raw_challenge,
    })
}

async fn verify_turnstile_captcha(
    state: &AppState,
    settings: &Value,
    token: &str,
    client_ip: &str,
    translator: &Translator,
) -> Result<(), String> {
    let secret_key = settings
        .pointer("/turnstile/secret_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if secret_key.is_empty() {
        return Err(captcha_text(translator, "turnstileSecretMissing"));
    }
    if token.trim().is_empty() {
        return Err(captcha_text(translator, "turnstileTokenRequired"));
    }

    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("secret", &secret_key);
        serializer.append_pair("response", token.trim());
        if !client_ip.is_empty() {
            serializer.append_pair("remoteip", client_ip);
        }
        serializer.finish()
    };
    let response = state
        .fallback_client
        .post(TURNSTILE_VERIFY_URL)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|_| captcha_text(translator, "turnstileServiceUnavailable"))?;
    if !response.status().is_success() {
        return Err(captcha_text(translator, "turnstileServiceUnavailable"));
    }
    let result = response
        .json::<Value>()
        .await
        .map_err(|_| auth_route_text(translator, "turnstileResponseInvalid"))?;
    if result.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else if let Some(reason) = turnstile_error_reason(&result) {
        Err(captcha_text_params(
            translator,
            "turnstileVerifyFailedWithReason",
            &[("reason", reason)],
        ))
    } else {
        Err(captcha_text(translator, "turnstileVerifyFailed"))
    }
}

fn pow_secret_number_from_random(value: u32) -> u32 {
    value % POW_MAX_NUMBER
}

fn pow_number_text(value: Option<&Value>) -> String {
    let Some(Value::Number(number)) = value else {
        return String::new();
    };
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    let Some(value) = number.as_f64() else {
        return String::new();
    };
    if !value.is_finite() {
        return String::new();
    }
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

fn turnstile_error_reason(result: &Value) -> Option<String> {
    let reason = result
        .get("error-codes")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!reason.is_empty()).then_some(reason)
}

fn find_matching_totp(credentials: &[TotpCredential], token: &str) -> Option<TotpCredential> {
    credentials
        .iter()
        .find(|credential| verify_totp_token(&credential.secret, token).unwrap_or(false))
        .cloned()
}

pub(crate) fn verify_totp_token(secret: &str, token: &str) -> anyhow::Result<bool> {
    let secret = Secret::Encoded(secret.trim().replace(' ', ""));
    let bytes = secret.to_bytes()?;
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes)?;
    Ok(totp.check_current(token)?)
}

pub(crate) fn safe_redirect(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> Option<String> {
    let value = redirect_uri?.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        let base = url::Url::parse("http://127.0.0.1").ok()?;
        let target = base.join(value).ok()?;
        if is_post_logout_redirect(&target) {
            return Some(relative_url(&normalize_post_logout_redirect_target(
                &target,
            )));
        }
        return Some(value.to_string());
    }

    let mut target = url::Url::parse(value).ok()?;
    if !matches!(target.scheme(), "http" | "https") {
        return None;
    }
    if is_post_logout_redirect(&target) {
        target = normalize_post_logout_redirect_target(&target);
    }

    if let (Some(proto), Some(host)) = (
        Some(resolve_forwarded_proto(headers)),
        resolve_forwarded_host(headers),
    ) && let Ok(current_origin) = url::Url::parse(&format!("{proto}://{host}"))
        && same_origin(&target, &current_origin)
    {
        return Some(target.to_string());
    }

    let target_host = target.host_str().map(normalize_subdomain_access_host)?;
    if target_host.is_empty() {
        return None;
    }

    if let Some(root_domain) = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_subdomain_access_host)
        .filter(|value| !value.is_empty())
        && host_within_domain(&target_host, &root_domain)
    {
        return Some(target.to_string());
    }

    let configured = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_subdomain_access_host)
        .any(|host| !host.is_empty() && host == target_host);
    if configured {
        return Some(target.to_string());
    }

    if let Some(auth_base_url) = resolve_public_auth_base_url(config)
        && let Ok(auth_base_url) = url::Url::parse(&auth_base_url)
        && same_origin(&target, &auth_base_url)
    {
        return Some(target.to_string());
    }

    None
}

pub(crate) fn effective_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    grant_type: &str,
    redirect_uri: Option<&str>,
) -> Option<String> {
    let redirect_to = safe_redirect(config, headers, redirect_uri)?;
    if grant_type == "browser_session"
        && !can_browser_session_reach_redirect_uri(config, headers, Some(&redirect_to))
    {
        return None;
    }
    Some(redirect_to)
}

pub(crate) fn resolve_cookie_domain(config: &Value, headers: &HeaderMap) -> Option<String> {
    let request_host = resolve_request_hostname_from_headers(headers);
    resolve_cookie_domain_for_request_host(config, request_host.as_deref())
}

fn resolve_shared_auth_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> Option<String> {
    if !is_any_subdomain_routing_mode(config) {
        return None;
    }
    let shared_auth_base_url = resolve_public_auth_base_url(config)?;
    let shared_auth_url = url::Url::parse(&shared_auth_base_url).ok()?;
    let request_proto = resolve_forwarded_proto(headers);
    let request_host = resolve_forwarded_host(headers)?;
    let current_origin = format!("{request_proto}://{request_host}");
    if let Ok(current_origin_url) = url::Url::parse(&current_origin) {
        if same_origin(&shared_auth_url, &current_origin_url) {
            return None;
        }
    } else {
        return None;
    }

    let shared_auth_host = shared_auth_url.host_str()?;
    if !can_browser_session_reach_redirect_uri_for_host(
        config,
        Some(shared_auth_host),
        Some(&current_origin),
    ) {
        return None;
    }

    let safe_redirect_uri = safe_redirect(config, headers, redirect_uri);
    build_shared_auth_login_url(&shared_auth_base_url, safe_redirect_uri.as_deref())
}

fn build_shared_auth_login_url(auth_base_url: &str, redirect_uri: Option<&str>) -> Option<String> {
    let base = auth_base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let mut login_url = url::Url::parse(&format!("{base}/#/login")).ok()?;
    if let Some(redirect_uri) = redirect_uri.filter(|value| !value.trim().is_empty()) {
        login_url
            .query_pairs_mut()
            .append_pair("redirect_uri", redirect_uri);
    }
    Some(login_url.to_string())
}

fn can_browser_session_reach_redirect_uri(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> bool {
    can_browser_session_reach_redirect_uri_for_host(
        config,
        resolve_request_hostname_from_headers(headers).as_deref(),
        redirect_uri,
    )
}

fn can_browser_session_reach_redirect_uri_for_host(
    config: &Value,
    request_host: Option<&str>,
    redirect_uri: Option<&str>,
) -> bool {
    let raw = redirect_uri.map(str::trim).unwrap_or_default();
    if raw.is_empty() || raw.starts_with('/') {
        return true;
    }
    let Ok(target) = url::Url::parse(raw) else {
        return false;
    };
    let Some(target_host) = target.host_str().map(normalize_subdomain_access_host) else {
        return false;
    };
    if target_host.is_empty() {
        return false;
    }
    if let Some(cookie_domain) = resolve_cookie_domain_for_request_host(config, request_host) {
        return host_within_domain(&target_host, &cookie_domain);
    }
    request_host
        .map(normalize_subdomain_access_host)
        .filter(|host| !host.is_empty())
        .is_some_and(|host| host == target_host)
}

fn resolve_cookie_domain_for_request_host(
    config: &Value,
    request_host: Option<&str>,
) -> Option<String> {
    let request_host = request_host
        .map(normalize_subdomain_access_host)
        .unwrap_or_default();
    let can_use =
        |candidate: &str| request_host.is_empty() || host_within_domain(&request_host, candidate);
    if let Some(domain) = config
        .pointer("/subdomain_mode/cookie_domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && can_use(domain)
    {
        return Some(domain.to_string());
    }
    if let Ok(domain) = env::var("SESSION_COOKIE_DOMAIN") {
        let domain = domain.trim().to_string();
        if !domain.is_empty() && can_use(&domain) {
            return Some(domain);
        }
    }
    if is_any_subdomain_routing_mode(config)
        && let Some(root_domain) = config
            .pointer("/subdomain_mode/root_domain")
            .and_then(Value::as_str)
            .map(normalize_subdomain_access_host)
            .filter(|value| !value.is_empty())
        && !request_host.is_empty()
        && host_within_domain(&request_host, &root_domain)
    {
        return Some(root_domain);
    }
    None
}

pub(crate) fn resolve_public_auth_base_url(config: &Value) -> Option<String> {
    if !is_reverse_proxy_subdomain_mode(config)
        && let Some(explicit) = config
            .pointer("/subdomain_mode/public_auth_base_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| apply_public_port_to_base_url(value, config))
    {
        return Some(explicit);
    }
    let auth_host = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                if item
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(is_auth_service_target)
                {
                    item.get("host").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    format_derived_public_auth_base_url(auth_host, config)
}

fn is_auth_service_target(target: &str) -> bool {
    let Ok(parsed) = url::Url::parse(target.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https" | "ws" | "wss")
        || parsed.host_str().is_none_or(|host| host.trim().is_empty())
    {
        return false;
    }
    parsed.port_or_known_default() == Some(resolve_auth_service_port())
}

fn resolve_auth_service_port() -> u16 {
    env::var("AUTH_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(7997)
}

fn apply_public_port_to_base_url(raw_base_url: &str, config: &Value) -> Option<String> {
    let trimmed = raw_base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let Ok(mut parsed) = url::Url::parse(trimmed) else {
        return Some(trimmed.to_string());
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return Some(trimmed.to_string());
    }
    if parsed.port().is_none()
        && let Some(port) =
            resolve_public_port_for_scheme(config, parsed.scheme(), trimmed, true, false)
        && !is_default_scheme_port(parsed.scheme(), port)
    {
        let _ = parsed.set_port(Some(port));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if path.is_empty() { "/" } else { &path });
    Some(parsed.to_string().trim_end_matches('/').to_string())
}

fn format_derived_public_auth_base_url(host: &str, config: &Value) -> Option<String> {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    let scheme = "https";
    let public_base = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(port) = resolve_auth_public_port_for_scheme(config, scheme, public_base, true)
        && !is_default_scheme_port(scheme, port)
    {
        return Some(format!("{scheme}://{host}:{port}"));
    }
    Some(format!("{scheme}://{host}"))
}

fn parse_explicit_url_port(raw_url: &str, scheme: &str) -> Option<u16> {
    let parsed = url::Url::parse(raw_url.trim()).ok()?;
    if parsed.scheme() != scheme {
        return None;
    }
    parsed.port()
}

fn resolve_configured_public_port(
    config: &Value,
    scheme: &str,
    allow_reverse_proxy_configured_port: bool,
) -> Option<u16> {
    if is_reverse_proxy_subdomain_mode(config) && !allow_reverse_proxy_configured_port {
        return None;
    }
    let pointer = if scheme == "https" {
        "/subdomain_mode/public_https_port"
    } else {
        "/subdomain_mode/public_http_port"
    };
    config
        .pointer(pointer)
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(raw) => raw.trim().parse::<i64>().ok(),
            _ => None,
        })
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
        .map(|port| port as u16)
}

fn resolve_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
    allow_reverse_proxy_configured_port: bool,
) -> Option<u16> {
    if let Some(port) = parse_explicit_url_port(raw_public_base_url, scheme) {
        return Some(port);
    }
    if let Some(port) =
        resolve_configured_public_port(config, scheme, allow_reverse_proxy_configured_port)
    {
        return Some(port);
    }
    if should_omit_public_access_entry_port(config) || !gateway_fallback {
        return None;
    }
    resolve_public_gateway_port(config)
}

fn resolve_auth_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
) -> Option<u16> {
    resolve_public_port_for_scheme(config, scheme, raw_public_base_url, gateway_fallback, true)
}

fn resolve_public_gateway_port(config: &Value) -> Option<u16> {
    crate::system_info::resolve_public_gateway_port(config)
        .filter(|port| *port <= u16::MAX as i64)
        .map(|port| port as u16)
}

fn is_default_scheme_port(scheme: &str, port: u16) -> bool {
    (scheme == "https" && port == 443) || (scheme == "http" && port == 80)
}

fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || is_reverse_proxy_subdomain_mode(config)
}

fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
        && config
            .get("reverse_proxy_submode")
            .and_then(Value::as_str)
            .unwrap_or("path")
            == "subdomain"
}

fn is_cloudflared_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    is_reverse_proxy_subdomain_mode(config)
        && config
            .get("default_tunnel")
            .and_then(Value::as_str)
            .unwrap_or("frp")
            == "cloudflared"
}

fn should_omit_public_access_entry_port(config: &Value) -> bool {
    is_cloudflared_reverse_proxy_subdomain_mode(config)
        || (config.get("run_type").and_then(Value::as_i64) == Some(3)
            && config
                .pointer("/subdomain_mode/edge_client_ip_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && (config
                .pointer("/subdomain_mode/aliyun_esa_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || config
                    .pointer("/subdomain_mode/tencent_edgeone_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)))
}

fn resolve_forwarded_proto(headers: &HeaderMap) -> String {
    let proto = parse_forwarded_header_proto(headers)
        .or_else(|| first_header_value(headers, "x-forwarded-proto"))
        .or_else(|| first_header_value(headers, "x-forwarded-scheme"))
        .or_else(|| first_header_value(headers, "x-original-proto"))
        .or_else(|| first_header_value(headers, "x-original-scheme"))
        .unwrap_or_else(|| "http".to_string());
    let proto = proto.trim().trim_end_matches(':').to_ascii_lowercase();
    if matches!(proto.as_str(), "http" | "https") {
        proto
    } else {
        "https".to_string()
    }
}

fn resolve_forwarded_host(headers: &HeaderMap) -> Option<String> {
    parse_forwarded_header_host(headers)
        .or_else(|| first_header_value(headers, "x-forwarded-host"))
        .or_else(|| first_header_value(headers, "x-original-host"))
        .or_else(|| first_header_value(headers, "host"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_request_hostname_from_headers(headers: &HeaderMap) -> Option<String> {
    resolve_forwarded_host(headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty())
}

fn parse_forwarded_header_proto(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    for segment in first.split(';') {
        let Some((key, value)) = segment.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("proto") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn host_within_domain(host: &str, domain: &str) -> bool {
    let host = normalize_subdomain_access_host(host);
    let domain = normalize_subdomain_access_host(domain)
        .trim_start_matches('.')
        .to_string();
    !host.is_empty()
        && !domain.is_empty()
        && (host == domain || host.ends_with(&format!(".{domain}")))
}

fn same_origin(left: &url::Url, right: &url::Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str().map(normalize_subdomain_access_host)
            == right.host_str().map(normalize_subdomain_access_host)
        && left.port_or_known_default() == right.port_or_known_default()
}

fn is_post_logout_redirect(target: &url::Url) -> bool {
    target
        .query_pairs()
        .any(|(key, value)| key == "logged_out" && value == "1")
        && is_logged_out_login_path(target.path())
}

fn is_logged_out_login_path(pathname: &str) -> bool {
    let normalized = normalize_pathname(pathname);
    matches!(
        normalized.as_str(),
        "/login" | "/auth/login" | "/__auth__/login"
    )
}

fn normalize_post_logout_redirect_target(target: &url::Url) -> url::Url {
    let mut normalized = target.clone();
    normalized.set_path(match normalize_pathname(target.path()).as_str() {
        "/auth/login" => "/auth/",
        "/__auth__/login" => "/__auth__/",
        _ => "/",
    });
    let pairs = target
        .query_pairs()
        .filter(|(key, _)| key != "logged_out")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    normalized.set_query(None);
    if !pairs.is_empty() {
        let mut query = normalized.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
    }
    normalized.set_fragment(None);
    normalized
}

fn normalize_pathname(pathname: &str) -> String {
    let pathname = pathname.trim();
    if pathname.is_empty() {
        return "/".to_string();
    }
    let pathname = if pathname.starts_with('/') {
        pathname.to_string()
    } else {
        format!("/{pathname}")
    };
    let normalized = pathname.trim_end_matches('/').to_string();
    if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized
    }
}

fn relative_url(target: &url::Url) -> String {
    let mut value = target.path().to_string();
    if let Some(query) = target.query() {
        value.push('?');
        value.push_str(query);
    }
    if let Some(fragment) = target.fragment() {
        value.push('#');
        value.push_str(fragment);
    }
    value
}

fn enqueue_auth_ip_location(state: &AppState, ip: &str, context: &'static str) {
    if ip.trim().is_empty() {
        return;
    }
    let state = state.clone();
    let ip = ip.to_string();
    tokio::spawn(async move {
        if let Err(error) =
            ip_location::ensure_ip_locations_enqueued(&state, vec![ip.clone()]).await
        {
            tracing::warn!(%error, %ip, %context, "failed to enqueue auth IP location lookup");
        }
    });
}

pub(crate) fn client_ip_for_auth(headers: &HeaderMap) -> String {
    http_utils::get_client_ip(headers)
}

pub(crate) fn user_agent(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn credential_name(credential: &TotpCredential, translator: &Translator) -> String {
    let name = credential.comment.trim();
    if name.is_empty() {
        auth_route_text(translator, "unknownTotp")
    } else {
        name.to_string()
    }
}

fn post_logout_location(headers: &HeaderMap, uri: &Uri) -> String {
    let base = resolve_auth_ui_base_prefix(headers, uri);
    format!("{base}/login?logged_out=1")
}

pub(crate) fn resolve_auth_ui_base_prefix(headers: &HeaderMap, uri: &Uri) -> &'static str {
    for pathname in [
        Some(uri.path().to_string()),
        header_pathname(headers, "x-forwarded-path"),
        header_pathname(headers, header::REFERER.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        if pathname == "/__auth__" || pathname.starts_with("/__auth__/") {
            return "/__auth__";
        }
        if pathname == "/auth" || pathname.starts_with("/auth/") {
            return "/auth";
        }
    }
    ""
}

fn header_pathname(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    let base = url::Url::parse("http://127.0.0.1").ok()?;
    base.join(value).ok().map(|url| url.path().to_string())
}

pub(crate) fn with_auth_headers(mut response: Response) -> Response {
    apply_no_store_headers(response.headers_mut());
    response
}

pub(crate) fn apply_no_store_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, no-cache, max-age=0, must-revalidate"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
    headers.insert(
        "CDN-Cache-Control",
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert("Surrogate-Control", HeaderValue::from_static("no-store"));
}

fn parse_pow_expires(salt: &str) -> Option<i64> {
    let query = salt.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=')
            && key == "expires"
        {
            return value.parse::<i64>().ok();
        }
    }
    None
}

fn sha256_hex(input: &[u8]) -> String {
    hex::encode(Sha256::digest(input))
}

fn hmac_sha256_hex(key: &[u8], value: &[u8]) -> anyhow::Result<String> {
    let mut mac = HmacSha256::new_from_slice(key)?;
    mac.update(value);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn random_bytes<const N: usize>() -> [u8; N] {
    rand::random::<[u8; N]>()
}

#[allow(dead_code)]
fn _method_is_head(method: &Method) -> bool {
    method == Method::HEAD
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn safe_redirect_allows_relative_current_origin_and_configured_hosts() {
        let config = json!({
            "subdomain_mode": {
                "root_domain": "example.com",
                "public_auth_base_url": "https://login.example.net",
                "public_https_port": 443
            },
            "host_mappings": [
                { "host": "mapped.example.net" }
            ]
        });
        let headers = forwarded_headers("app.example.com");
        assert_eq!(
            safe_redirect(&config, &headers, Some("/app")).as_deref(),
            Some("/app")
        );
        assert_eq!(
            safe_redirect(&config, &headers, Some("https://app.example.com/app")).as_deref(),
            Some("https://app.example.com/app")
        );
        assert_eq!(
            safe_redirect(&config, &headers, Some("https://tools.example.com/app")).as_deref(),
            Some("https://tools.example.com/app")
        );
        assert_eq!(
            safe_redirect(&config, &headers, Some("https://mapped.example.net/app")).as_deref(),
            Some("https://mapped.example.net/app")
        );
        assert_eq!(
            safe_redirect(&config, &headers, Some("https://login.example.net/app")).as_deref(),
            Some("https://login.example.net/app")
        );
    }

    #[test]
    fn safe_redirect_matches_node_scheme_relative_and_unknown_scheme_rules() {
        let config = json!({});
        let headers = forwarded_headers("app.example.com");
        assert_eq!(
            safe_redirect(&config, &headers, Some("//example.com")).as_deref(),
            Some("//example.com")
        );
        assert!(safe_redirect(&config, &headers, Some("javascript:alert(1)")).is_none());
        assert!(safe_redirect(&config, &headers, Some("https://evil.example/app")).is_none());
    }

    #[test]
    fn browser_session_redirect_must_be_reachable_by_cookie_scope() {
        let config = json!({
            "host_mappings": [
                { "host": "app.example.net" }
            ]
        });
        let headers = forwarded_headers("auth.example.net");
        assert_eq!(
            safe_redirect(&config, &headers, Some("https://app.example.net/app")).as_deref(),
            Some("https://app.example.net/app")
        );
        assert!(
            effective_login_redirect(
                &config,
                &headers,
                "browser_session",
                Some("https://app.example.net/app")
            )
            .is_none()
        );

        let config = json!({
            "run_type": 3,
            "subdomain_mode": { "root_domain": "example.net" }
        });
        assert_eq!(
            effective_login_redirect(
                &config,
                &headers,
                "browser_session",
                Some("https://app.example.net/app")
            )
            .as_deref(),
            Some("https://app.example.net/app")
        );
    }

    #[test]
    fn shared_auth_redirect_targets_public_auth_origin() {
        let config = json!({
            "run_type": 3,
            "subdomain_mode": {
                "root_domain": "example.com",
                "auth_host": "auth.example.com",
                "public_https_port": 443
            }
        });
        let headers = forwarded_headers("app.example.com");
        let redirect = resolve_shared_auth_login_redirect(
            &config,
            &headers,
            Some("https://app.example.com/dashboard"),
        )
        .unwrap();
        assert!(redirect.starts_with("https://auth.example.com/?redirect_uri="));
        assert!(redirect.ends_with("#/login"));
        assert!(redirect.contains("https%3A%2F%2Fapp.example.com%2Fdashboard"));
    }

    #[test]
    fn public_auth_base_url_applies_configured_public_https_port() {
        let config = json!({
            "run_type": 3,
            "subdomain_mode": {
                "root_domain": "example.com",
                "auth_host": "auth.example.com",
                "public_https_port": 8443
            }
        });
        let headers = forwarded_headers("app.example.com");
        let redirect =
            resolve_shared_auth_login_redirect(&config, &headers, Some("/dashboard")).unwrap();
        assert!(redirect.starts_with("https://auth.example.com:8443/?redirect_uri="));
        assert!(redirect.contains("%2Fdashboard"));
    }

    #[test]
    fn forwarded_header_parsing_matches_node_fallbacks() {
        let config = json!({});
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("app.example.com"));
        assert_eq!(
            safe_redirect(&config, &headers, Some("http://app.example.com/app")).as_deref(),
            Some("http://app.example.com/app")
        );
        assert!(safe_redirect(&config, &headers, Some("https://app.example.com/app")).is_none());

        headers.insert(
            "forwarded",
            HeaderValue::from_static("for=192.0.2.1; bad; proto=https; host=auth.example.com"),
        );
        assert_eq!(resolve_forwarded_proto(&headers), "https");
        assert_eq!(
            resolve_forwarded_host(&headers).as_deref(),
            Some("auth.example.com")
        );
    }

    #[test]
    fn resolve_cookie_domain_matches_subdomain_mode_scope() {
        let config = json!({
            "run_type": 3,
            "subdomain_mode": { "root_domain": "example.com" }
        });
        let headers = forwarded_headers("auth.example.com");
        assert_eq!(
            resolve_cookie_domain(&config, &headers).as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn post_logout_location_matches_node_prefix_resolution() {
        let headers = HeaderMap::new();
        assert_eq!(
            post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
            "/login?logged_out=1"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-path",
            HeaderValue::from_static("/__auth__/api/auth/logout"),
        );
        assert_eq!(
            post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
            "/__auth__/login?logged_out=1"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::REFERER,
            HeaderValue::from_static("https://example.com/auth/settings"),
        );
        assert_eq!(
            post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
            "/auth/login?logged_out=1"
        );
    }

    fn forwarded_headers(host: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert("x-forwarded-host", HeaderValue::from_str(host).unwrap());
        headers
    }

    #[test]
    fn strict_whitelist_access_mode_matches_node_header_parsing() {
        let mut headers = HeaderMap::new();
        assert!(!is_strict_whitelist_request(&headers));

        headers.insert(
            "X-Reauth-Access-Mode",
            HeaderValue::from_static(" strict_whitelist "),
        );
        assert!(is_strict_whitelist_request(&headers));

        headers.insert(
            "X-Reauth-Access-Mode",
            HeaderValue::from_static("fnos-share"),
        );
        assert!(!is_strict_whitelist_request(&headers));
    }

    #[test]
    fn client_ip_for_auth_matches_node_header_extraction() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip_for_auth(&headers), "");

        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.20"));
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.10, 198.51.100.20"),
        );
        assert_eq!(client_ip_for_auth(&headers), "203.0.113.10");

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("[::1]:443"));
        assert_eq!(client_ip_for_auth(&headers), "127.0.0.1");
    }

    #[test]
    fn inspect_auth_mobility_request_matches_node_cookie_rules() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(
                "x-go-reauth-proxy-session-id=old; fnos-token=old%201; mode=relay; \
                 x-go-reauth-proxy-session-id=\"session%202\"; fnos-token=token%202",
            )
            .unwrap(),
        );
        headers.insert("x-forwarded-path", HeaderValue::from_static("trimcon?x=1"));
        headers.insert(header::USER_AGENT, HeaderValue::from_static("Dart:io"));

        let identity = inspect_auth_mobility_request(&headers);

        assert_eq!(identity.session_id.as_deref(), Some("session 2"));
        assert_eq!(identity.fnos_token.as_deref(), Some("token 2"));
        assert_eq!(identity.app_binding, Some("fnos-app"));
        assert_eq!(identity.trim_media_token, None);
    }

    #[test]
    fn inspect_auth_mobility_request_extracts_trim_media_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("com.trim.media"),
        );
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer media-token"),
        );

        let identity = inspect_auth_mobility_request(&headers);

        assert_eq!(identity.app_binding, Some("trim-media-app"));
        assert_eq!(identity.trim_media_token.as_deref(), Some("media-token"));
        assert_eq!(identity.fnos_token, None);
    }

    #[test]
    fn auth_mobility_resolvable_access_requires_live_owner_session_like_node() {
        let mut session = LoginSession {
            totp_id: "totp-1".to_string(),
            method: "TOTP".to_string(),
            credential_id: "totp-1".to_string(),
            credential_name: "TOTP".to_string(),
            linked_totp_name: None,
            grant_type: Some("browser_session".to_string()),
            post_login_ip_grant_mode: None,
            post_login_ip_grant_record_id: None,
            comment: None,
            ip: "203.0.113.10".to_string(),
            user_agent: "ua".to_string(),
            login_time: "2026-01-01T00:00:00Z".to_string(),
            expires_at: Some("2999-01-01T00:00:00Z".to_string()),
            ip_location: None,
        };

        assert!(auth_mobility_session_has_remaining_ttl(&session));

        session.expires_at = Some("2000-01-01T00:00:00Z".to_string());
        assert!(!auth_mobility_session_has_remaining_ttl(&session));

        session.expires_at = Some("not-a-date".to_string());
        assert!(!auth_mobility_session_has_remaining_ttl(&session));

        session.expires_at = None;
        assert!(!auth_mobility_session_has_remaining_ttl(&session));
    }

    #[test]
    fn whitelist_target_matching_supports_ip_and_cidr_targets() {
        assert!(whitelist_target_matches_ip(
            "192.0.2.10",
            "ip",
            "192.0.2.10".parse().unwrap()
        ));
        assert!(whitelist_target_matches_ip(
            "[2001:db8::10]",
            "ip",
            "2001:db8::10".parse().unwrap()
        ));
        assert!(whitelist_target_matches_ip(
            "2001:db8::/32",
            "cidr",
            "2001:db8:1::1".parse().unwrap()
        ));
        assert!(!whitelist_target_matches_ip(
            "2001:db8::/32",
            "cidr",
            "2001:db9::1".parse().unwrap()
        ));
    }

    #[test]
    fn parses_pow_expiry_from_salt() {
        assert_eq!(parse_pow_expires("abc?expires=123"), Some(123));
        assert_eq!(parse_pow_expires("abc?x=1&expires=456"), Some(456));
        assert_eq!(parse_pow_expires("abc?x&expires=789"), Some(789));
        assert_eq!(parse_pow_expires("abc"), None);
    }

    #[test]
    fn pow_challenge_generation_uses_node_exclusive_max_number() {
        assert_eq!(pow_secret_number_from_random(0), 0);
        assert_eq!(pow_secret_number_from_random(POW_MAX_NUMBER), 0);
        assert!(pow_secret_number_from_random(u32::MAX) < POW_MAX_NUMBER);
    }

    #[test]
    fn pow_number_text_matches_node_number_only_rule() {
        assert_eq!(pow_number_text(Some(&json!(42))), "42");
        assert_eq!(pow_number_text(Some(&json!(42.0))), "42");
        assert_eq!(pow_number_text(Some(&json!(42.5))), "42.5");
        assert_eq!(pow_number_text(Some(&json!("42"))), "");
        assert_eq!(pow_number_text(None), "");
    }

    #[test]
    fn pow_validation_uses_original_challenge_for_signature_and_nonce_like_node() {
        let translator = Translator::new("en");
        let key = "secret";
        let salt = "abc?expires=9999999999";
        let number = 7;
        let challenge = sha256_hex(format!("{salt}{number}").as_bytes()).to_ascii_uppercase();
        let signature = hmac_sha256_hex(key.as_bytes(), challenge.as_bytes()).unwrap();

        let validation = validate_pow_proof(
            PowProof {
                algorithm: Some("SHA-256".to_string()),
                challenge: Some(challenge.clone()),
                number: Some(json!(number)),
                salt: Some(salt.to_string()),
                signature: Some(signature),
            },
            key,
            1,
            &translator,
        )
        .unwrap();
        assert_eq!(validation.nonce, challenge);

        let rejected = validate_pow_proof(
            PowProof {
                algorithm: Some("SHA-256".to_string()),
                challenge: Some(challenge.clone()),
                number: Some(json!(number)),
                salt: Some(salt.to_string()),
                signature: Some(
                    hmac_sha256_hex(key.as_bytes(), challenge.to_ascii_lowercase().as_bytes())
                        .unwrap(),
                ),
            },
            key,
            1,
            &translator,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn turnstile_error_reason_matches_node_error_codes_join() {
        assert_eq!(
            turnstile_error_reason(&json!({ "error-codes": ["a", "", "b"] })),
            Some("a, b".to_string())
        );
        assert_eq!(turnstile_error_reason(&json!({ "error-codes": [] })), None);
        assert_eq!(turnstile_error_reason(&json!({})), None);
    }

    #[test]
    fn totp_verification_does_not_trim_token_like_node_otplib() {
        let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes).unwrap();
        let token = totp.generate_current().unwrap();

        assert!(verify_totp_token(secret, &token).unwrap());
        assert!(!verify_totp_token(secret, &format!(" {token}")).unwrap_or(false));
        assert!(!verify_totp_token(secret, &format!("{token} ")).unwrap_or(false));
    }

    #[test]
    fn verify_denied_status_matches_node_scope_boundary() {
        let scoped = AuthAccess {
            authenticated: false,
            message: "Access denied by credential scope".to_string(),
            grant_type: None,
            deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
            set_cookies: Vec::new(),
            response_headers: Vec::new(),
        };
        assert_eq!(auth_verify_denied_status(&scoped), StatusCode::FORBIDDEN);

        let ordinary = AuthAccess {
            authenticated: false,
            message: "Unauthorized".to_string(),
            grant_type: None,
            deny_reason: None,
            set_cookies: Vec::new(),
            response_headers: Vec::new(),
        };
        assert_eq!(
            auth_verify_denied_status(&ordinary),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn logout_custom_post_login_grant_revoke_predicate_matches_node() {
        let mut session = LoginSession {
            totp_id: "totp-1".to_string(),
            method: "TOTP".to_string(),
            credential_id: "totp-1".to_string(),
            credential_name: "TOTP".to_string(),
            linked_totp_name: None,
            grant_type: Some("login_ip_grant".to_string()),
            post_login_ip_grant_mode: Some("custom".to_string()),
            post_login_ip_grant_record_id: None,
            comment: None,
            ip: "203.0.113.10".to_string(),
            user_agent: "ua".to_string(),
            login_time: "2026-01-01T00:00:00Z".to_string(),
            expires_at: Some("2026-01-02T00:00:00Z".to_string()),
            ip_location: None,
        };
        assert!(should_revoke_custom_post_login_ip_grant(
            Some(&session),
            &json!({})
        ));

        session.post_login_ip_grant_mode = Some("follow_session".to_string());
        session.comment = Some("Automatically authorized after sign-in".to_string());
        assert!(should_revoke_custom_post_login_ip_grant(
            Some(&session),
            &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "custom"}})
        ));
        assert!(!should_revoke_custom_post_login_ip_grant(
            Some(&session),
            &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
        ));
    }

    #[test]
    fn localizes_auth_route_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            auth_route_text(&translator, "authenticationRequired"),
            "需要先完成认证"
        );
        let credential = TotpCredential {
            id: "totp-1".to_string(),
            secret: "secret".to_string(),
            comment: "".to_string(),
            created_at: String::new(),
            access_scopes: Value::Null,
            subdomain_access: Value::Null,
        };
        assert_eq!(credential_name(&credential, &translator), "未知 TOTP");
    }
}
