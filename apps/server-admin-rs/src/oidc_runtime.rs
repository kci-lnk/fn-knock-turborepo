use std::env;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{
    DecodingKey, Validation, decode, decode_header,
    jwk::{Jwk, JwkSet},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
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

pub(crate) async fn consume_login_error_for_bootstrap(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
) -> Option<(String, String)> {
    let token = cookies::read_cookie(headers, cookies::OIDC_LOGIN_ERROR_COOKIE_NAME)?;
    let notice = oidc_consume_login_error_notice(state, &hash_oidc_token(&token))
        .await
        .ok()
        .flatten()?;
    let message = notice
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let domain = resolve_cookie_domain(config, headers);
    let prefix = crate::auth::resolve_auth_ui_base_prefix(headers, uri);
    let path = if prefix.is_empty() { "/" } else { prefix };
    let clear_cookie = cookies::oidc_login_error_clear_cookie(domain.as_deref(), path);
    Some((message, clear_cookie))
}

async fn bind(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<BindQuery>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config for OIDC bind");
            return bind_html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &oidc_text(&translator, "bindFailedTitle"),
                &oidc_text(&translator, "loadConfigFailed"),
                DEFAULT_LOCALE,
                None,
            );
        }
    };
    let translator = translator_from_config(&config);
    let locale = locale_code(&config);
    let token = query.token.as_deref().map(str::trim).unwrap_or("");
    if token.is_empty() {
        return bind_html_response(
            StatusCode::BAD_REQUEST,
            &oidc_text(&translator, "inviteInvalid"),
            &oidc_text(&translator, "linkMissingToken"),
            &locale,
            None,
        );
    }

    let invite = match oidc_inspect_invite(&state, token).await {
        Ok(Some(invite)) => invite,
        Ok(None) => {
            return bind_html_response(
                StatusCode::NOT_FOUND,
                &oidc_text(&translator, "inviteExpired"),
                &oidc_text(&translator, "inviteMissingExpiredUsed"),
                &locale,
                None,
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to inspect OIDC invite before bind");
            return bind_html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &oidc_text(&translator, "bindFailedTitle"),
                &oidc_text(&translator, "bindStartFailed"),
                &locale,
                None,
            );
        }
    };
    let providers = invite
        .get("providers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if providers.is_empty() {
        return bind_html_response(
            StatusCode::NOT_FOUND,
            &oidc_text(&translator, "noProvidersTitle"),
            &oidc_text(&translator, "noProvidersBody"),
            &locale,
            None,
        );
    }
    let selected_provider = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| invite.get("provider_id").and_then(Value::as_str))
        .or_else(|| {
            (providers.len() == 1).then(|| providers[0].get("id").and_then(Value::as_str))?
        });
    let Some(provider_id) = selected_provider else {
        return bind_provider_selection_response(
            &uri,
            token,
            &invite,
            &providers,
            &translator,
            &locale,
        );
    };

    match build_authorization_url(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        provider_id,
        "bind",
        None,
        Some(token),
        false,
    )
    .await
    {
        Ok(result) => {
            let domain = resolve_cookie_domain(&config, &headers);
            let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
            redirect_response(
                &result.authorization_url,
                vec![cookies::oidc_flow_cookie(
                    &result.flow_token,
                    result.max_age as i64,
                    domain.as_deref(),
                    &path,
                )],
            )
        }
        Err(error) => bind_html_response(
            StatusCode::BAD_REQUEST,
            &oidc_text(&translator, "bindFailedTitle"),
            &error,
            &locale,
            None,
        ),
    }
}

async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<StartBody>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config before OIDC start");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let translator = translator_from_config(&config);
    let mode = match body.mode.as_deref().unwrap_or("login") {
        "login" | "bind" => body.mode.as_deref().unwrap_or("login"),
        _ => "login",
    };
    match build_authorization_url(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        &body.provider_id,
        mode,
        body.redirect_uri.as_deref(),
        body.invite_token.as_deref(),
        body.remember_me,
    )
    .await
    {
        Ok(result) => {
            let domain = resolve_cookie_domain(&config, &headers);
            let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
            let cookie = cookies::oidc_flow_cookie(
                &result.flow_token,
                result.max_age as i64,
                domain.as_deref(),
                &path,
            );
            let mut response = Json(ApiEnvelope {
                success: true,
                code: None,
                message: None,
                data: Some(json!({ "authorization_url": result.authorization_url })),
            })
            .into_response();
            apply_no_store_headers(response.headers_mut());
            append_set_cookie(response.headers_mut(), &cookie);
            response
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error),
    }
}

#[axum::debug_handler(state = AppState)]
async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(provider_id): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config before OIDC callback");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let translator = translator_from_config(&config);
    let code = query
        .code
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let state_token = query
        .state
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let flow_token = cookies::read_cookie(&headers, cookies::OIDC_FLOW_COOKIE_NAME);
    let clear_flow_cookie = || {
        state_token
            .filter(|state| oidc_flow_token_valid(state, flow_token.as_deref()))
            .map(|_| {
                let domain = resolve_cookie_domain(&config, &headers);
                let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
                cookies::oidc_flow_clear_cookie(domain.as_deref(), &path)
            })
    };

    if let Some(error) = query.error.as_deref() {
        let auth_state = consume_callback_state_for_notice(
            &state,
            &provider_id,
            state_token,
            flow_token.as_deref(),
        )
        .await;
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            provider_error_message(error, &translator),
            &translator,
            auth_state
                .as_ref()
                .and_then(|value| value.get("redirect_uri"))
                .and_then(Value::as_str),
            auth_state.is_some(),
            clear_flow_cookie(),
        )
        .await;
    }

    let Some(code) = code else {
        let auth_state = consume_callback_state_for_notice(
            &state,
            &provider_id,
            state_token,
            flow_token.as_deref(),
        )
        .await;
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            oidc_text(&translator, "callbackMissingParams"),
            &translator,
            auth_state
                .as_ref()
                .and_then(|value| value.get("redirect_uri"))
                .and_then(Value::as_str),
            auth_state.is_some(),
            clear_flow_cookie(),
        )
        .await;
    };
    let Some(state_token) = state_token else {
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            oidc_text(&translator, "callbackMissingParams"),
            &translator,
            None,
            false,
            None,
        )
        .await;
    };

    let client_ip = client_ip_for_headers(&headers);
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    match state.redis.get_login_backoff_status(&tracking_ip).await {
        Ok(status) if status.blocked => {
            let auth_state = consume_callback_state_for_notice(
                &state,
                &provider_id,
                Some(state_token),
                flow_token.as_deref(),
            )
            .await;
            let message = status
                .retry_after
                .map(|retry_after| {
                    server_text_params(
                        &translator,
                        "tooManyAttemptsWithRetry",
                        &[("seconds", retry_after.max(1).to_string())],
                    )
                })
                .unwrap_or_else(|| server_text(&translator, "tooManyAttempts"));
            return login_error_redirect_response(
                &state,
                &headers,
                &uri,
                &config,
                message,
                &translator,
                auth_state
                    .as_ref()
                    .and_then(|value| value.get("redirect_uri"))
                    .and_then(Value::as_str),
                auth_state.is_some(),
                clear_flow_cookie(),
            )
            .await;
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, %tracking_ip, "failed to inspect OIDC backoff"),
    }

    match resolve_callback(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        &provider_id,
        code,
        state_token,
        flow_token.as_deref(),
    )
    .await
    {
        Ok(resolved) => {
            let redirect_to = resolved
                .state
                .get("redirect_uri")
                .and_then(Value::as_str)
                .unwrap_or("/");
            match create_oidc_session_response(
                &state,
                &headers,
                &config,
                &resolved,
                &translator,
                redirect_to,
                clear_flow_cookie(),
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!(%error, "failed to create OIDC session");
                    login_error_redirect_response(
                        &state,
                        &headers,
                        &uri,
                        &config,
                        oidc_text(&translator, "loginFailedRetry"),
                        &translator,
                        Some(redirect_to),
                        true,
                        clear_flow_cookie(),
                    )
                    .await
                }
            }
        }
        Err(error) => {
            if error == oidc_text(&translator, "callbackStateExpired") {
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    error,
                    &translator,
                    None,
                    false,
                    clear_flow_cookie(),
                )
                .await
            } else if is_oidc_operation_aborted_error(&error) {
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    oidc_text(&translator, "operationAborted"),
                    &translator,
                    None,
                    true,
                    clear_flow_cookie(),
                )
                .await
            } else {
                let detail_message = error;
                let response_message = match state
                    .redis
                    .register_login_backoff_failure(&tracking_ip)
                    .await
                {
                    Ok(failure) => {
                        let retry_after = failure.retry_after.unwrap_or(1).max(1);
                        if let Err(event_error) = system_events::publish_auth_login_failure_event(
                            &state,
                            json!({
                                "ip": tracking_ip.clone(),
                                "attempts": failure.attempts,
                                "retry_after_seconds": retry_after,
                                "blocked_until": failure.blocked_until.map(time_utils::iso_from_ms),
                                "method": "OIDC",
                                "credential_name": provider_id.clone(),
                                "user_agent": user_agent(&headers),
                            }),
                        )
                        .await
                        {
                            tracing::warn!(%event_error, %tracking_ip, "failed to publish OIDC login failure event");
                        }
                        oidc_login_failed_retry_after_message(
                            &translator,
                            &detail_message,
                            retry_after,
                        )
                    }
                    Err(backoff_error) => {
                        tracing::warn!(%backoff_error, %tracking_ip, "failed to register OIDC login failure");
                        detail_message
                    }
                };
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    response_message,
                    &translator,
                    None,
                    true,
                    clear_flow_cookie(),
                )
                .await
            }
        }
    }
}

async fn build_authorization_url(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
    provider_id: &str,
    mode: &str,
    redirect_uri: Option<&str>,
    invite_token: Option<&str>,
    remember_me: bool,
) -> Result<AuthorizationBuild, String> {
    let provider = oidc_get_provider(state, provider_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "providerUnavailable"))?;
    if provider.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err(oidc_text(translator, "providerUnavailable"));
    }
    oidc_provider_ready_with_translator(&provider, translator)?;
    let mut invite_token_hash = None;
    if mode == "bind" {
        let token = invite_token
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| oidc_text(translator, "inviteInvalid"))?;
        let invite = oidc_inspect_invite(state, token)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| oidc_text(translator, "inviteExpired"))?;
        if let Some(invite_provider_id) = invite.get("provider_id").and_then(Value::as_str)
            && invite_provider_id != provider_id
        {
            return Err(oidc_text(translator, "inviteProviderNotAllowed"));
        }
        invite_token_hash = Some(hash_oidc_token(token));
    }

    let callback_url = build_callback_url(provider_id, headers, uri, config, translator)?;
    let state_token = create_public_token();
    let state_hash = hash_oidc_token(&state_token);
    let protocol = provider
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("oidc");
    let nonce = (protocol == "oidc").then(create_public_token);
    let code_verifier = (protocol == "oidc").then(create_pkce_verifier);
    let safe_redirect_uri = crate::auth::safe_redirect(config, headers, redirect_uri);
    let mut auth_state = Map::new();
    auth_state.insert("state_hash".to_string(), Value::String(state_hash.clone()));
    auth_state.insert("mode".to_string(), Value::String(mode.to_string()));
    auth_state.insert(
        "provider_id".to_string(),
        Value::String(provider_id.to_string()),
    );
    if let Some(redirect_uri) = safe_redirect_uri {
        auth_state.insert("redirect_uri".to_string(), Value::String(redirect_uri));
    }
    if let Some(invite_token_hash) = invite_token_hash {
        auth_state.insert(
            "invite_token_hash".to_string(),
            Value::String(invite_token_hash),
        );
    }
    if let Some(code_verifier) = code_verifier.as_deref() {
        auth_state.insert(
            "code_verifier".to_string(),
            Value::String(code_verifier.to_string()),
        );
    }
    if let Some(nonce) = nonce.as_deref() {
        auth_state.insert("nonce".to_string(), Value::String(nonce.to_string()));
    }
    auth_state.insert("remember_me".to_string(), Value::Bool(remember_me));
    let client_ip = client_ip_for_headers(headers);
    if !client_ip.is_empty() {
        auth_state.insert("client_ip".to_string(), Value::String(client_ip));
    }
    auth_state.insert(
        "created_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    auth_state.insert(
        "expires_at".to_string(),
        Value::String(time_utils::iso_after_seconds(OIDC_STATE_TTL_SECONDS as i64)),
    );
    oidc_save_state(state, &Value::Object(auth_state), OIDC_STATE_TTL_SECONDS)
        .await
        .map_err(|error| error.to_string())?;

    let authorization_url = if protocol == "oidc" {
        build_standard_oidc_authorization_url(
            &provider,
            &callback_url,
            &state_token,
            nonce.as_deref().unwrap_or(""),
            code_verifier.as_deref().unwrap_or(""),
            translator,
        )
        .await?
    } else {
        build_oauth_profile_authorization_url(&provider, &callback_url, &state_token, translator)?
    };
    Ok(AuthorizationBuild {
        authorization_url,
        flow_token: state_hash,
        max_age: OIDC_STATE_TTL_SECONDS,
    })
}

async fn resolve_callback(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
    provider_id: &str,
    code: &str,
    state_token: &str,
    flow_token: Option<&str>,
) -> Result<CallbackResolved, String> {
    if !oidc_flow_token_valid(state_token, flow_token) {
        return Err(oidc_text(translator, "callbackStateExpired"));
    }
    let state_hash = hash_oidc_token(state_token);
    let auth_state = oidc_consume_state(state, &state_hash)
        .await
        .map_err(|error| error.to_string())?
        .filter(|value| value.get("provider_id").and_then(Value::as_str) == Some(provider_id))
        .ok_or_else(|| oidc_text(translator, "callbackStateExpired"))?;
    let provider = oidc_get_provider(state, provider_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "providerUnavailable"))?;
    if provider.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err(oidc_text(translator, "providerUnavailable"));
    }
    let callback_url = build_callback_url(provider_id, headers, uri, config, translator)?;
    let profile = if provider.get("protocol").and_then(Value::as_str) == Some("oauth2_profile") {
        resolve_oauth_profile_callback(state, &provider, code, &callback_url, translator).await?
    } else {
        resolve_standard_oidc_callback(
            state,
            &provider,
            code,
            &callback_url,
            &auth_state,
            translator,
        )
        .await?
    };
    let subject_key = build_subject_key(provider_id, &profile.issuer, &profile.subject);
    if auth_state.get("mode").and_then(Value::as_str) == Some("bind") {
        return bind_profile_and_resolve_login(
            state,
            provider,
            profile,
            subject_key,
            auth_state,
            translator,
        )
        .await;
    }
    let Some(mut binding) = oidc_get_binding_by_subject(state, &subject_key)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Err(oidc_text(translator, "accountNotBoundCannotLogin"));
    };
    update_binding_profile_fields(&mut binding, &profile);
    if let Some(object) = binding.as_object_mut() {
        object.insert(
            "last_used_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
    }
    oidc_save_binding(state, &binding)
        .await
        .map_err(|error| error.to_string())?;
    Ok(CallbackResolved {
        state: auth_state,
        provider,
        binding,
        profile,
    })
}

async fn bind_profile_and_resolve_login(
    state: &AppState,
    provider: Value,
    profile: ExternalProfile,
    subject_key: String,
    auth_state: Value,
    translator: &Translator,
) -> Result<CallbackResolved, String> {
    let invite_hash = auth_state
        .get("invite_token_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "bindStateInvalid"))?;
    let invite = state
        .redis
        .get_json_value(&format!("fn_knock:oidc:invite:{invite_hash}"))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "inviteExpired"))?;
    if let Some(invite_provider) = invite.get("provider_id").and_then(Value::as_str)
        && invite_provider != provider.get("id").and_then(Value::as_str).unwrap_or("")
    {
        return Err(oidc_text(translator, "bindProviderMismatch"));
    }
    let totp_id = invite
        .get("totp_id")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "inviteTotpMissing"))?;
    let totps = state
        .redis
        .get_totps()
        .await
        .map_err(|error| error.to_string())?;
    if !totps.iter().any(|totp| totp.id == totp_id) {
        return Err(oidc_text(translator, "inviteTotpMissing"));
    }
    let existing = oidc_get_binding_by_subject(state, &subject_key)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(existing) = existing.as_ref()
        && existing.get("totp_id").and_then(Value::as_str) != Some(totp_id)
    {
        return Err(oidc_text(translator, "accountAlreadyBoundOtherTotp"));
    }
    oidc_consume_invite(state, invite_hash)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "inviteUsed"))?;
    if let Some(mut binding) = existing {
        update_binding_profile_fields(&mut binding, &profile);
        if let Some(object) = binding.as_object_mut() {
            object.insert(
                "last_used_at".to_string(),
                Value::String(time_utils::now_iso()),
            );
            object.insert(
                "updated_at".to_string(),
                Value::String(time_utils::now_iso()),
            );
        }
        oidc_save_binding(state, &binding)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(CallbackResolved {
            state: auth_state,
            provider,
            binding,
            profile,
        });
    }
    let now = time_utils::now_iso();
    let binding = json!({
        "id": create_oidc_id("oidc_binding"),
        "provider_id": provider.get("id").and_then(Value::as_str).unwrap_or(""),
        "provider_type": provider.get("type").and_then(Value::as_str).unwrap_or("custom_oidc"),
        "totp_id": totp_id,
        "issuer": profile.issuer.clone(),
        "subject": profile.subject.clone(),
        "subject_key": subject_key,
        "display_name": profile.display_name.clone(),
        "email": profile.email.clone(),
        "email_verified": profile.email_verified,
        "avatar_url": profile.avatar_url.clone(),
        "created_at": now,
        "updated_at": now,
        "last_used_at": now
    });
    let saved = oidc_save_binding_if_subject_available(state, &binding)
        .await
        .map_err(|error| error.to_string())?;
    if !saved {
        return Err(oidc_text(translator, "accountAlreadyBoundOtherTotp"));
    }
    Ok(CallbackResolved {
        state: auth_state,
        provider,
        binding,
        profile,
    })
}

async fn create_oidc_session_response(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    resolved: &CallbackResolved,
    _translator: &Translator,
    redirect_to: &str,
    flow_clear_cookie: Option<String>,
) -> anyhow::Result<Response> {
    let client_ip = client_ip_for_headers(headers);
    let provider_name = resolved
        .provider
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let credential_name = resolved
        .profile
        .display_name
        .as_deref()
        .or(resolved.profile.email.as_deref())
        .or(provider_name)
        .unwrap_or("External Account");
    let totp_id = resolved
        .binding
        .get("totp_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let totp_credentials = state.redis.get_totps().await?;
    let totp_credential = totp_credentials
        .iter()
        .find(|totp| totp.id == totp_id)
        .cloned();
    let linked_totp_name = totp_credential
        .as_ref()
        .map(|totp| totp.comment.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let credential_id = resolved
        .binding
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let remember_me = resolved
        .state
        .get("remember_me")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let created = auth_mobility::create_login_session(
        state,
        config,
        CreateLoginSessionInput {
            auth_method: "OIDC".to_string(),
            auth_provider_name: provider_name.map(str::to_string),
            credential_id,
            credential_name: credential_name.to_string(),
            totp_id: totp_id.to_string(),
            linked_totp_name,
            totp_credential,
            client_ip: client_ip.clone(),
            user_agent: user_agent(headers),
            remember_me,
        },
    )
    .await?;
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    if let Err(error) = state.redis.reset_login_backoff(&tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset OIDC login backoff");
    }
    let domain = resolve_cookie_domain(config, headers);
    let mut cookies = vec![cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        domain.as_deref(),
    )];
    if let Some(flow_clear_cookie) = flow_clear_cookie {
        cookies.push(flow_clear_cookie);
    }
    let final_redirect_to = crate::auth::effective_login_redirect(
        config,
        headers,
        &created.grant_type,
        Some(redirect_to),
    )
    .unwrap_or_else(|| "/".to_string());
    Ok(redirect_response(&final_redirect_to, cookies))
}

async fn build_standard_oidc_authorization_url(
    provider: &Value,
    callback_url: &str,
    state_token: &str,
    nonce: &str,
    code_verifier: &str,
    translator: &Translator,
) -> Result<String, String> {
    let discovery = resolve_discovery_with_translator(provider, translator).await?;
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id")
        .ok_or_else(|| oidc_text(translator, "clientIdMissing"))?;
    let mut url = Url::parse(
        discovery
            .get("authorization_endpoint")
            .and_then(Value::as_str)
            .ok_or_else(|| oidc_text(translator, "authorizationEndpointMissing"))?,
    )
    .map_err(|_| oidc_text(translator, "authorizationEndpointInvalid"))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("client_id", client_id);
        params.append_pair("response_type", "code");
        params.append_pair("redirect_uri", callback_url);
        params.append_pair(
            "scope",
            &scopes(config, &["openid", "profile", "email"]).join(" "),
        );
        params.append_pair("state", state_token);
        params.append_pair("nonce", nonce);
        params.append_pair("code_challenge", &create_pkce_challenge(code_verifier));
        params.append_pair("code_challenge_method", "S256");
        for (key, value) in extra_auth_params(config) {
            params.append_pair(&key, &value);
        }
    }
    Ok(url.to_string())
}

fn build_oauth_profile_authorization_url(
    provider: &Value,
    callback_url: &str,
    state_token: &str,
    translator: &Translator,
) -> Result<String, String> {
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id")
        .ok_or_else(|| oidc_text(translator, "clientIdMissing"))?;
    let endpoint = string_field(config, "authorization_endpoint")
        .ok_or_else(|| oidc_text(translator, "authorizationEndpointMissing"))?;
    let mut url =
        Url::parse(endpoint).map_err(|_| oidc_text(translator, "authorizationEndpointInvalid"))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("client_id", client_id);
        params.append_pair("response_type", "code");
        params.append_pair("redirect_uri", callback_url);
        params.append_pair("scope", &scopes(config, &[]).join(" "));
        params.append_pair("state", state_token);
        for (key, value) in extra_auth_params(config) {
            params.append_pair(&key, &value);
        }
    }
    Ok(url.to_string())
}

async fn resolve_standard_oidc_callback(
    state: &AppState,
    provider: &Value,
    code: &str,
    callback_url: &str,
    auth_state: &Value,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let discovery = resolve_discovery_with_translator(provider, translator).await?;
    let config = provider_config(provider, translator)?;
    let token_endpoint = discovery
        .get("token_endpoint")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "tokenEndpointMissing"))?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let client_secret = string_field(config, "client_secret").unwrap_or("");
    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
        ("code", code.to_string()),
        ("redirect_uri", callback_url.to_string()),
    ];
    if let Some(code_verifier) = auth_state.get("code_verifier").and_then(Value::as_str) {
        form.push(("code_verifier", code_verifier.to_string()));
    }
    let token_payload = exchange_form_token(state, token_endpoint, &form, None, translator).await?;
    verify_standard_oidc_profile(
        state,
        provider,
        &token_payload,
        &discovery,
        auth_state.get("nonce").and_then(Value::as_str),
        translator,
    )
    .await
}

async fn resolve_oauth_profile_callback(
    state: &AppState,
    provider: &Value,
    code: &str,
    callback_url: &str,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let config = provider_config(provider, translator)?;
    let token_endpoint = string_field(config, "token_endpoint")
        .ok_or_else(|| oidc_text(translator, "tokenEndpointMissing"))?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let client_secret = string_field(config, "client_secret").unwrap_or("");
    let form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", client_id.to_string()),
        ("client_secret", client_secret.to_string()),
        ("code", code.to_string()),
        ("redirect_uri", callback_url.to_string()),
    ];
    let headers = (provider.get("type").and_then(Value::as_str) == Some("github"))
        .then_some(vec![("Accept", "application/json")]);
    let token_payload =
        exchange_form_token(state, token_endpoint, &form, headers, translator).await?;
    let access_token = string_field_from_value(&token_payload, "access_token")
        .ok_or_else(|| oidc_text(translator, "accessTokenMissing"))?;
    if provider.get("type").and_then(Value::as_str) == Some("github") {
        return fetch_github_profile(state, provider, access_token, translator).await;
    }
    Err(oidc_text(translator, "providerUnsupported"))
}

async fn exchange_form_token(
    state: &AppState,
    endpoint: &str,
    fields: &[(&str, String)],
    extra_headers: Option<Vec<(&str, &str)>>,
    translator: &Translator,
) -> Result<Value, String> {
    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            serializer.append_pair(key, value);
        }
        serializer.finish()
    };
    let mut request = oidc_http_request(state.fallback_client.post(endpoint), "application/json")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body);
    for (key, value) in extra_headers.unwrap_or_default() {
        request = request.header(key, value);
    }
    let response = request.send().await.map_err(|error| {
        oidc_text_params(
            translator,
            "tokenRequestFailed",
            &[("detail", error.to_string())],
        )
    })?;
    parse_http_payload(response, translator).await
}

fn oidc_http_request(
    request: reqwest::RequestBuilder,
    accept: &'static str,
) -> reqwest::RequestBuilder {
    request
        .header(header::ACCEPT, accept)
        .header(header::USER_AGENT, OIDC_HTTP_USER_AGENT)
}

async fn parse_http_payload(
    response: reqwest::Response,
    translator: &Translator,
) -> Result<Value, String> {
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let text = response.text().await.map_err(|error| {
        oidc_text_params(
            translator,
            "readResponseFailed",
            &[("detail", error.to_string())],
        )
    })?;
    if !status.is_success() {
        return Err(oidc_text_params(
            translator,
            "httpResponseFailed",
            &[
                ("status", status.to_string()),
                ("detail", text.chars().take(160).collect::<String>()),
            ],
        ));
    }
    parse_json_or_form(&text, &content_type, translator)
}

fn parse_json_or_form(
    text: &str,
    content_type: &str,
    translator: &Translator,
) -> Result<Value, String> {
    let trimmed = text.trim();
    if content_type.contains("json") || trimmed.starts_with('{') {
        serde_json::from_str(trimmed).map_err(|error| {
            oidc_text_params(
                translator,
                "jsonResponseInvalid",
                &[("detail", error.to_string())],
            )
        })
    } else {
        let object = url::form_urlencoded::parse(trimmed.as_bytes())
            .map(|(key, value)| (key.into_owned(), Value::String(value.into_owned())))
            .collect::<Map<_, _>>();
        Ok(Value::Object(object))
    }
}

async fn verify_standard_oidc_profile(
    state: &AppState,
    provider: &Value,
    token_payload: &Value,
    discovery: &Value,
    expected_nonce: Option<&str>,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let id_token = token_payload
        .get("id_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| oidc_text(translator, "idTokenMissing"))?;
    let jwks_uri = discovery
        .get("jwks_uri")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "jwksUriMissing"))?;
    let jwks = oidc_http_request(state.fallback_client.get(jwks_uri), "application/json")
        .send()
        .await
        .map_err(|error| {
            oidc_text_params(
                translator,
                "jwksFetchFailed",
                &[("detail", error.to_string())],
            )
        })?
        .json::<JwkSet>()
        .await
        .map_err(|error| {
            oidc_text_params(translator, "jwksInvalid", &[("detail", error.to_string())])
        })?;
    let header = decode_header(id_token).map_err(|error| {
        oidc_text_params(
            translator,
            "tokenHeaderInvalid",
            &[("detail", error.to_string())],
        )
    })?;
    let jwk = select_jwk(&jwks, header.kid.as_deref())
        .ok_or_else(|| oidc_text(translator, "signingKeyUnavailable"))?;
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|error| {
        oidc_text_params(
            translator,
            "signingKeyInvalid",
            &[("detail", error.to_string())],
        )
    })?;
    let config = provider_config(provider, translator)?;
    let client_id = string_field(config, "client_id").unwrap_or("");
    let discovery_issuer = discovery
        .get("issuer")
        .and_then(Value::as_str)
        .unwrap_or("");
    let issuer_for_verify = (!discovery_issuer.contains("{tenantid}")).then_some(discovery_issuer);
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[client_id]);
    if let Some(issuer) = issuer_for_verify {
        validation.set_issuer(&[issuer]);
    }
    let token = decode::<Value>(id_token, &decoding_key, &validation).map_err(|error| {
        oidc_text_params(
            translator,
            "idTokenVerificationFailed",
            &[("detail", error.to_string())],
        )
    })?;
    let payload = token.claims;
    if let Some(expected_nonce) = expected_nonce
        && payload.get("nonce").and_then(Value::as_str) != Some(expected_nonce)
    {
        return Err(oidc_text(translator, "nonceCheckFailed"));
    }
    if issuer_for_verify.is_none() {
        let issuer = payload.get("iss").and_then(Value::as_str).unwrap_or("");
        if !issuer.starts_with("https://login.microsoftonline.com/") {
            return Err(oidc_text(translator, "issuerCheckFailed"));
        }
    }
    let subject = payload
        .get("sub")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| oidc_text(translator, "subjectEmpty"))?;
    let mut userinfo = Value::Object(Map::new());
    if let Some(endpoint) = discovery
        .get("userinfo_endpoint")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        && let Some(access_token) = token_payload.get("access_token").and_then(Value::as_str)
    {
        if let Ok(response) =
            oidc_http_request(state.fallback_client.get(endpoint), "application/json")
                .bearer_auth(access_token)
                .send()
                .await
        {
            if response.status().is_success()
                && let Ok(payload) = parse_http_payload(response, translator).await
            {
                userinfo = payload;
            }
        }
    }
    let pick = |key: &str| userinfo.get(key).or_else(|| payload.get(key));
    Ok(ExternalProfile {
        issuer: payload
            .get("iss")
            .and_then(Value::as_str)
            .unwrap_or(discovery_issuer)
            .to_string(),
        subject: subject.to_string(),
        display_name: optional_string(pick("name"))
            .or_else(|| optional_string(pick("preferred_username"))),
        email: optional_string(pick("email")),
        email_verified: Some(value_truthy(pick("email_verified"))),
        avatar_url: optional_string(pick("picture")),
    })
}

fn select_jwk<'a>(jwks: &'a JwkSet, kid: Option<&str>) -> Option<&'a Jwk> {
    if let Some(kid) = kid {
        if let Some(jwk) = jwks
            .keys
            .iter()
            .find(|jwk| jwk.common.key_id.as_deref() == Some(kid))
        {
            return Some(jwk);
        }
    }
    jwks.keys.first()
}

async fn fetch_github_profile(
    state: &AppState,
    provider: &Value,
    access_token: &str,
    translator: &Translator,
) -> Result<ExternalProfile, String> {
    let config = provider_config(provider, translator)?;
    let user_endpoint =
        string_field(config, "userinfo_endpoint").unwrap_or("https://api.github.com/user");
    let user = github_api_request(&state.fallback_client, user_endpoint, access_token)
        .send()
        .await
        .map_err(|error| {
            oidc_text_params(
                translator,
                "githubProfileRequestFailed",
                &[("detail", error.to_string())],
            )
        })?;
    let user = parse_http_payload(user, translator).await?;
    let subject = optional_string(user.get("id"))
        .or_else(|| {
            user.get("id")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| oidc_text(translator, "githubUserIdEmpty"))?;
    let mut email = optional_string(user.get("email"));
    let mut email_verified = false;
    if let Some(endpoint) = string_field(config, "emails_endpoint") {
        if let Ok(response) = github_api_request(&state.fallback_client, endpoint, access_token)
            .send()
            .await
        {
            if response.status().is_success()
                && let Ok(emails) = response.json::<Value>().await
                && let Some(items) = emails.as_array()
            {
                if let Some(primary) = items
                    .iter()
                    .find(|item| item.get("primary").and_then(Value::as_bool) == Some(true))
                    .or_else(|| items.first())
                {
                    email = optional_string(primary.get("email")).or(email);
                    email_verified = primary
                        .get("verified")
                        .and_then(Value::as_bool)
                        .unwrap_or(email.is_some());
                }
            }
        }
    }
    Ok(ExternalProfile {
        issuer: "github".to_string(),
        subject,
        display_name: optional_string(user.get("name"))
            .or_else(|| optional_string(user.get("login"))),
        email,
        email_verified: Some(email_verified),
        avatar_url: optional_string(user.get("avatar_url")),
    })
}

fn github_api_request(
    client: &reqwest::Client,
    endpoint: &str,
    access_token: &str,
) -> reqwest::RequestBuilder {
    oidc_http_request(client.get(endpoint), "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .bearer_auth(access_token)
}

async fn consume_callback_state_for_notice(
    state: &AppState,
    provider_id: &str,
    state_token: Option<&str>,
    flow_token: Option<&str>,
) -> Option<Value> {
    let state_token = state_token?;
    if !oidc_flow_token_valid(state_token, flow_token) {
        return None;
    }
    oidc_consume_state(state, &hash_oidc_token(state_token))
        .await
        .ok()
        .flatten()
        .filter(|value| value.get("provider_id").and_then(Value::as_str) == Some(provider_id))
}

async fn login_error_redirect_response(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    message: String,
    translator: &Translator,
    redirect_uri: Option<&str>,
    persist_notice: bool,
    flow_clear_cookie: Option<String>,
) -> Response {
    let mut cookies = Vec::new();
    if persist_notice {
        let token = create_public_token();
        let token_hash = hash_oidc_token(&token);
        let notice = json!({
            "token_hash": token_hash,
            "message": normalize_login_error_message(&message, translator),
            "created_at": time_utils::now_iso(),
            "expires_at": time_utils::iso_after_seconds(LOGIN_ERROR_TTL_SECONDS as i64)
        });
        if let Err(error) =
            oidc_save_login_error_notice(state, &notice, LOGIN_ERROR_TTL_SECONDS).await
        {
            tracing::warn!(%error, "failed to save OIDC login error notice");
        } else {
            let domain = resolve_cookie_domain(config, headers);
            let path = resolve_oidc_cookie_path(config, headers, uri.path());
            cookies.push(cookies::oidc_login_error_cookie(
                &token,
                LOGIN_ERROR_TTL_SECONDS as i64,
                domain.as_deref(),
                &path,
            ));
        }
    }
    if let Some(cookie) = flow_clear_cookie {
        cookies.push(cookie);
    }
    let location = build_login_redirect(config, headers, uri.path(), redirect_uri);
    redirect_response(&location, cookies)
}

fn provider_error_message(error: &str, translator: &Translator) -> String {
    match error.trim().to_ascii_lowercase().as_str() {
        "access_denied" => oidc_text(translator, "providerErrors.accessDenied"),
        "temporarily_unavailable" => oidc_text(translator, "providerErrors.temporarilyUnavailable"),
        "server_error" => oidc_text(translator, "providerErrors.serverError"),
        "invalid_scope" => oidc_text(translator, "providerErrors.invalidScope"),
        "invalid_request" | "unauthorized_client" | "unsupported_response_type" => {
            oidc_text(translator, "providerErrors.rejected")
        }
        _ => oidc_text(translator, "providerErrors.incomplete"),
    }
}

fn is_oidc_operation_aborted_error(error: &str) -> bool {
    let message = error.to_ascii_lowercase();
    message.contains("operation was aborted")
        || (message.contains("aborterror") && message.contains("aborted"))
}

fn oidc_login_failed_retry_after_message(
    translator: &Translator,
    message: &str,
    retry_after: i64,
) -> String {
    oidc_text_params(
        translator,
        "loginFailedRetryAfter",
        &[
            ("message", message.to_string()),
            ("seconds", retry_after.max(1).to_string()),
        ],
    )
}

fn redirect_response(location: &str, cookies: Vec<String>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());
    for cookie in cookies {
        append_set_cookie(response.headers_mut(), &cookie);
    }
    response
}

fn bind_provider_selection_response(
    uri: &Uri,
    token: &str,
    invite: &Value,
    providers: &[Value],
    translator: &Translator,
    locale: &str,
) -> Response {
    let totp_name = invite
        .pointer("/totp/comment")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("TOTP");
    let actions = providers
        .iter()
        .filter_map(|provider| {
            let id = provider.get("id").and_then(Value::as_str)?;
            let name = provider.get("name").and_then(Value::as_str).unwrap_or(id);
            let query = format!(
                "token={}&provider_id={}",
                encode_query(token),
                encode_query(id)
            );
            Some(format!(
                r#"<a href="{}?{}">{}</a>"#,
                html_escape(uri.path()),
                query,
                html_escape(&oidc_text_params(
                    translator,
                    "bindWithProvider",
                    &[("provider", name.to_string())],
                ))
            ))
        })
        .collect::<String>();
    bind_html_response(
        StatusCode::OK,
        &oidc_text(translator, "selectProviderTitle"),
        &oidc_text_params(translator, "bindToTotp", &[("totp", totp_name.to_string())]),
        locale,
        Some(format!(r#"<div class="actions">{actions}</div>"#)),
    )
}

fn bind_html_response(
    status: StatusCode,
    title: &str,
    body: &str,
    locale: &str,
    actions: Option<String>,
) -> Response {
    let html = format!(
        r#"<!doctype html>
<html lang="{locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <style>
      body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#f6f7f9;color:#111827;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}}
      main{{width:min(92vw,420px);box-sizing:border-box;border:1px solid #e5e7eb;border-radius:12px;background:#fff;padding:28px;box-shadow:0 18px 48px rgba(15,23,42,.08)}}
      h1{{margin:0 0 10px;font-size:22px;line-height:1.25}}
      p{{margin:0;color:#4b5563;line-height:1.7;font-size:14px}}
      .actions{{display:grid;gap:10px;margin-top:22px}}
      a{{display:flex;align-items:center;justify-content:center;height:40px;border-radius:8px;background:#111827;color:#fff;text-decoration:none;font-size:14px;font-weight:600}}
    </style>
  </head>
  <body>
    <main>
      <h1>{title}</h1>
      <p>{body}</p>
      {actions}
    </main>
  </body>
</html>"#,
        locale = html_escape(locale),
        title = html_escape(title),
        body = html_escape(body),
        actions = actions.unwrap_or_default(),
    );
    let mut response = (
        status,
        [
            ("content-type", "text/html; charset=utf-8"),
            ("x-content-type-options", "nosniff"),
            ("x-frame-options", "DENY"),
            ("referrer-policy", "no-referrer"),
        ],
        html,
    )
        .into_response();
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    apply_no_store_headers(response.headers_mut());
    response
}

fn update_binding_profile_fields(binding: &mut Value, profile: &ExternalProfile) {
    if let Some(object) = binding.as_object_mut() {
        if let Some(value) = profile
            .display_name
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            object.insert("display_name".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = profile.email.as_deref().filter(|value| !value.is_empty()) {
            object.insert("email".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = profile.email_verified {
            object.insert("email_verified".to_string(), Value::Bool(value));
        }
        if let Some(value) = profile
            .avatar_url
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            object.insert("avatar_url".to_string(), Value::String(value.to_string()));
        }
    }
}

fn extra_auth_params(config: &Map<String, Value>) -> Vec<(String, String)> {
    config
        .get("extra_auth_params")
        .and_then(Value::as_object)
        .map(|extra| {
            extra
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn provider_config<'a>(
    provider: &'a Value,
    translator: &Translator,
) -> Result<&'a Map<String, Value>, String> {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn string_field_from_value<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn scopes(config: &Map<String, Value>, fallback: &[&str]) -> Vec<String> {
    let values = config
        .get("scopes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if values.is_empty() {
        fallback.iter().map(|value| (*value).to_string()).collect()
    } else {
        values
    }
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|value| match value {
            Value::String(value) => Some(value.trim().to_string()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn value_truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(value)) => value.as_i64().unwrap_or_default() != 0,
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

fn build_callback_url(
    provider_id: &str,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
) -> Result<String, String> {
    if let Some(base) = public_auth_base_url(config) {
        return Ok(format!(
            "{}/api/auth/oidc/callback/{}",
            base.trim_end_matches('/'),
            encode_query(provider_id)
        ));
    }
    let origin = request_origin(headers, uri, translator)?;
    let prefix = auth_api_prefix(uri.path());
    Ok(format!(
        "{origin}{prefix}/api/auth/oidc/callback/{}",
        encode_query(provider_id)
    ))
}

fn build_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    path: &str,
    redirect_uri: Option<&str>,
) -> String {
    let prefix = configured_auth_view_prefix(config, headers, path);
    let mut location = format!("{prefix}/login");
    if let Some(redirect_uri) = redirect_uri.filter(|value| !value.trim().is_empty()) {
        location.push('?');
        location.push_str("redirect_uri=");
        location.push_str(&encode_query(redirect_uri));
    }
    location
}

fn resolve_oidc_cookie_path(config: &Value, headers: &HeaderMap, path: &str) -> String {
    configured_auth_view_prefix(config, headers, path)
        .trim_end_matches('/')
        .to_string()
        .if_empty("/")
}

fn configured_auth_view_prefix(config: &Value, _headers: &HeaderMap, path: &str) -> String {
    if let Some(prefix) = auth_view_prefix(path) {
        return prefix.to_string();
    }
    if let Some(base_url) = public_auth_base_url(config)
        && let Ok(url) = Url::parse(&base_url)
    {
        let path = url.path().trim_end_matches('/');
        if !path.is_empty() && path != "/" {
            return path.to_string();
        }
    }
    String::new()
}

fn auth_view_prefix(path: &str) -> Option<&'static str> {
    if path == "/__auth__" || path.starts_with("/__auth__/") {
        Some("/__auth__")
    } else if path == "/auth" || path.starts_with("/auth/") {
        Some("/auth")
    } else {
        None
    }
}

fn auth_api_prefix(path: &str) -> &'static str {
    if path.starts_with("/__auth__/api/auth/") {
        "/__auth__"
    } else if path.starts_with("/auth/api/auth/") {
        "/auth"
    } else {
        ""
    }
}

fn request_origin(
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
) -> Result<String, String> {
    let trust_forwarded = env_bool("OIDC_TRUST_FORWARDED_HEADERS", false)
        || env_bool("AUTH_TRUST_FORWARDED_HEADERS", false);
    let request_proto = uri.scheme_str().unwrap_or("http");
    let proto = if trust_forwarded {
        first_header(headers, "x-forwarded-proto")
    } else {
        None
    }
    .unwrap_or_else(|| request_proto.to_string())
    .trim()
    .trim_end_matches(':')
    .to_ascii_lowercase();
    let host = if trust_forwarded {
        first_header(headers, "x-forwarded-host")
    } else {
        None
    }
    .or_else(|| first_header(headers, "host"))
    .or_else(|| {
        uri.authority()
            .map(|authority| authority.as_str().to_string())
    })
    .ok_or_else(|| oidc_text(translator, "callbackUrlBuildFailed"))?;
    if (proto != "http" && proto != "https")
        || host
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, ',' | '/' | '?' | '#' | '\\' | '@'))
    {
        return Err(oidc_text(translator, "callbackUrlBuildFailed"));
    }
    Ok(format!("{proto}://{host}"))
}

fn public_auth_base_url(config: &Value) -> Option<String> {
    crate::auth::resolve_public_auth_base_url(config)
}

fn resolve_cookie_domain(config: &Value, headers: &HeaderMap) -> Option<String> {
    crate::auth::resolve_cookie_domain(config, headers)
}

fn first_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn client_ip_for_headers(headers: &HeaderMap) -> String {
    let ip = get_client_ip(headers);
    if ip.is_empty() {
        "127.0.0.1".to_string()
    } else {
        ip
    }
}

fn user_agent(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn locale_code(config: &Value) -> String {
    config
        .pointer("/locale/default_locale")
        .and_then(Value::as_str)
        .unwrap_or("zh-CN")
        .to_string()
}

fn oidc_flow_token_valid(state: &str, flow_token: Option<&str>) -> bool {
    let expected = hash_oidc_token(state);
    let Some(flow_token) = flow_token.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    expected.as_bytes().ct_eq(flow_token.as_bytes()).unwrap_u8() == 1
}

fn create_oidc_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

fn create_public_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

fn create_pkce_verifier() -> String {
    create_public_token()
}

fn create_pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn hash_oidc_token(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn build_subject_key(provider_id: &str, issuer: &str, subject: &str) -> String {
    hex::encode(Sha256::digest(format!(
        "{provider_id}\0{issuer}\0{subject}"
    )))
}

fn normalize_login_error_message(message: &str, translator: &Translator) -> String {
    let message = message.trim();
    if message.is_empty() {
        oidc_text(translator, "loginFailedRetry")
    } else {
        message.chars().take(500).collect()
    }
}

fn apply_no_store_headers(headers: &mut HeaderMap) {
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

fn append_set_cookie(headers: &mut HeaderMap, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        headers.append(header::SET_COOKIE, value);
    }
}

fn encode_query(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn env_bool(name: &str, fallback: bool) -> bool {
    match env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        _ => fallback,
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_oidc_flow_token_with_state_hash() {
        let state = "state-token";
        assert!(oidc_flow_token_valid(state, Some(&hash_oidc_token(state))));
        assert!(!oidc_flow_token_valid(state, Some("wrong")));
    }

    #[test]
    fn resolves_auth_cookie_path_from_prefixed_routes() {
        assert_eq!(auth_view_prefix("/auth/api/auth/oidc/start"), Some("/auth"));
        assert_eq!(
            auth_view_prefix("/__auth__/api/auth/oidc/start"),
            Some("/__auth__")
        );
        assert_eq!(auth_view_prefix("/api/auth/oidc/start"), None);
    }

    #[test]
    fn parses_json_and_form_payloads() {
        let translator = Translator::new(DEFAULT_LOCALE);
        assert_eq!(
            parse_json_or_form(r#"{"access_token":"abc"}"#, "application/json", &translator)
                .unwrap()["access_token"],
            json!("abc")
        );
        assert_eq!(
            parse_json_or_form(
                "access_token=abc&token_type=bearer",
                "text/plain",
                &translator
            )
            .unwrap()["token_type"],
            json!("bearer")
        );
    }

    #[test]
    fn detects_oidc_operation_aborted_errors_like_node() {
        assert!(is_oidc_operation_aborted_error(
            "The operation was aborted before completion"
        ));
        assert!(is_oidc_operation_aborted_error(
            "AbortError: request aborted"
        ));
        assert!(!is_oidc_operation_aborted_error("invalid_grant"));
    }

    #[test]
    fn oidc_outbound_requests_include_fetch_like_user_agent() {
        let client = reqwest::Client::new();
        let token_request = oidc_http_request(
            client.post("https://example.test/token"),
            "application/json",
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body("grant_type=authorization_code")
        .build()
        .unwrap();
        assert_eq!(
            token_request
                .headers()
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(OIDC_HTTP_USER_AGENT)
        );

        let github_request =
            github_api_request(&client, "https://api.github.com/user", "access-token")
                .build()
                .unwrap();
        let headers = github_request.headers();
        assert_eq!(
            headers
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(OIDC_HTTP_USER_AGENT)
        );
        assert_eq!(
            headers
                .get(header::ACCEPT)
                .and_then(|value| value.to_str().ok()),
            Some("application/vnd.github+json")
        );
        assert_eq!(
            headers
                .get("X-GitHub-Api-Version")
                .and_then(|value| value.to_str().ok()),
            Some("2022-11-28")
        );
        assert_eq!(
            headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-token")
        );
    }

    #[test]
    fn localizes_oidc_runtime_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            provider_error_message("access_denied", &translator),
            "你取消了外部登录授权，或授权请求被提供商拒绝。"
        );
        assert_eq!(
            normalize_login_error_message("   ", &translator),
            "外部登录失败，请重新发起登录。"
        );
        assert_eq!(
            oidc_login_failed_retry_after_message(&translator, "invalid_grant", 3),
            "invalid_grant，请在 3 秒后重试"
        );
        assert_eq!(
            request_origin(
                &HeaderMap::new(),
                &Uri::from_static("/api/auth/oidc/start"),
                &translator
            )
            .unwrap_err(),
            "无法生成外部登录回调地址，请配置 public_auth_base_url"
        );
        assert_eq!(
            request_origin(
                &HeaderMap::new(),
                &Uri::from_static("https://auth.example.com/api/auth/oidc/start"),
                &translator
            )
            .unwrap(),
            "https://auth.example.com"
        );
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("auth.example.com:7999"));
        assert_eq!(
            request_origin(
                &headers,
                &Uri::from_static("/api/auth/oidc/start"),
                &translator
            )
            .unwrap(),
            "http://auth.example.com:7999"
        );
        assert!(
            parse_json_or_form("{bad", "application/json", &translator)
                .unwrap_err()
                .starts_with("外部登录响应不是有效 JSON")
        );
    }
}
