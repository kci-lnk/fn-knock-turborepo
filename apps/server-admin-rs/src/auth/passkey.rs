use std::{collections::BTreeMap, str::FromStr};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::*;
use webauthn_rs_core::proto::{
    AttestationMetadata, AuthenticatorTransport, RegisteredExtensions, UserVerificationPolicy,
};

use crate::{
    auth::{
        client_ip_for_auth, effective_login_redirect,
        mode::{AuthLoginMode, AuthMethod},
        user_agent, with_auth_headers,
    },
    auth_mobility::{self, CreateLoginSessionInput},
    backoff::normalize_auth_failure_tracking_ip,
    cookies,
    i18n::Translator,
    response,
    state::AppState,
    system_events, time_utils,
};

const RP_NAME: &str = "fn-knock";
const PASSKEY_CHALLENGE_TTL_SECONDS: usize = 300;
const PASSKEY_BIND_TTL_SECONDS: usize = 600;
const PASSKEY_ADMIN_UUID: Uuid = Uuid::from_bytes(*b"fn-knock-admin!!");
const CA_HOSTS_KEY: &str = "fn_knock:ca:hosts";

#[derive(Deserialize)]
struct AuthVerifyBody {
    credential: Value,
    #[serde(default, rename = "rememberMe")]
    remember_me: bool,
    redirect_uri: Option<String>,
}

#[derive(Deserialize)]
struct RegisterOptionsBody {
    token: String,
}

#[derive(Deserialize)]
struct RegisterVerifyBody {
    token: String,
    #[serde(default, rename = "deviceName")]
    device_name: String,
    credential: Value,
}

pub fn passkey_routes() -> Router<AppState> {
    Router::new()
        .route("/passkey/status", get(status))
        .route("/passkey/auth/options", post(auth_options))
        .route("/passkey/auth/verify", post(auth_verify))
        .route("/passkey/bind-token", post(bind_token))
        .route("/passkey/register/options", post(register_options))
        .route("/passkey/register/verify", post(register_verify))
}

fn passkey_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.passkeyRoutes.{key}"))
}

fn passkey_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.passkeyRoutes.{key}"), params)
}

async fn ensure_passkey_login_mode(
    state: &AppState,
    translator: &Translator,
) -> Result<(), Response> {
    match state.store.get_auth_login_mode().await {
        Ok(AuthLoginMode::Totp) => Ok(()),
        Ok(_) => Err(with_auth_headers(response::error(
            StatusCode::BAD_REQUEST,
            passkey_text(translator, "loginMethodUnavailable"),
        ))),
        Err(error) => {
            tracing::warn!(%error, "failed to load auth login mode for passkey");
            Err(with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(translator, "loadStatusFailed"),
            )))
        }
    }
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for passkey status");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "loadStatusFailed"),
            ));
        }
    };
    let passkey_count = state
        .store
        .get_passkeys()
        .await
        .map(|items| items.len())
        .unwrap_or(0);
    let passkey_login_enabled = state
        .store
        .get_auth_login_mode()
        .await
        .map(AuthLoginMode::allows_totp_family)
        .unwrap_or(false);
    let rp = rp_info(&state, &config, &headers).await;
    with_auth_headers(
        response::ok(json!({
            "available": passkey_login_enabled && passkey_count > 0,
            "mode": rp.mode,
            "rp_id": rp.rp_id
        }))
        .into_response(),
    )
}

async fn auth_options(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(response) = ensure_passkey_login_mode(&state, &translator).await {
        return response;
    }
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for passkey auth options");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createOptionsFailed"),
            ));
        }
    };
    let passkeys = match state.store.get_passkeys().await {
        Ok(passkeys) => passkeys,
        Err(error) => {
            tracing::warn!(%error, "failed to load passkeys");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "loadPasskeysFailed"),
            ));
        }
    };
    if passkeys.is_empty() {
        return with_auth_headers(response::error(
            StatusCode::NOT_FOUND,
            passkey_text(&translator, "noPasskeyAvailable"),
        ));
    }

    let credentials = passkeys
        .iter()
        .filter_map(passkey_to_security_key)
        .collect::<Vec<_>>();
    if credentials.is_empty() {
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "noValidPasskeyAvailable"),
        ));
    }
    let rp_info = rp_info(&state, &config, &headers).await;
    let webauthn = match build_webauthn(&rp_info) {
        Ok(webauthn) => webauthn,
        Err(error) => {
            tracing::warn!(%error, rp_id = %rp_info.rp_id, origin = %rp_info.origin, "invalid passkey RP config");
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidRpConfig"),
            ));
        }
    };

    let (options, auth_state) = match webauthn.start_securitykey_authentication(&credentials) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to start passkey authentication");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createOptionsFailed"),
            ));
        }
    };
    let challenge = URL_SAFE_NO_PAD.encode(&options.public_key.challenge);
    if let Err(error) = state
        .store
        .set_passkey_challenge(&challenge, "auth", PASSKEY_CHALLENGE_TTL_SECONDS)
        .await
    {
        tracing::warn!(%error, "failed to store passkey challenge");
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "createOptionsFailed"),
        ));
    }
    let state_json = json!({
        "type": "auth",
        "state": auth_state,
        "rp_id": rp_info.rp_id,
        "origin": rp_info.origin,
        "mode": rp_info.mode
    });
    if let Err(error) = state
        .store
        .set_passkey_state(&challenge, &state_json, PASSKEY_CHALLENGE_TTL_SECONDS)
        .await
    {
        tracing::warn!(%error, "failed to store passkey auth state");
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "createOptionsFailed"),
        ));
    }

    with_auth_headers(response::ok(options.public_key).into_response())
}

async fn auth_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AuthVerifyBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(response) = ensure_passkey_login_mode(&state, &translator).await {
        return response;
    }
    let client_ip = client_ip_for_auth(&headers);
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    match state.store.get_login_backoff_status(&tracking_ip).await {
        Ok(status) if status.blocked => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            return with_auth_headers(passkey_backoff_response(
                &translator.t_params(
                    "server.tooManyAttemptsWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            ));
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to inspect passkey login backoff");
        }
    }

    let credential: PublicKeyCredential = match serde_json::from_value(body.credential.clone()) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to decode passkey auth credential");
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidResponse"),
            ));
        }
    };
    let challenge = match extract_challenge(&body.credential) {
        Some(value) => value,
        None => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidResponse"),
            ));
        }
    };
    match state
        .store
        .consume_passkey_challenge(&challenge, "auth")
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "challengeExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to consume passkey challenge");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "verifyFailed"),
            ));
        }
    }
    let mut state_json = match state.store.consume_passkey_state(&challenge).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "challengeExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to consume passkey auth state");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "verifyFailed"),
            ));
        }
    };
    let backup_flags = authenticator_backup_flags(&body.credential);
    if let Some(flags) = backup_flags {
        patch_authentication_state_backup_flags(&mut state_json, credential.id.as_str(), flags);
    }
    let auth_state: SecurityKeyAuthentication =
        match serde_json::from_value(state_json.get("state").cloned().unwrap_or(Value::Null)) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "failed to decode passkey auth state");
                return with_auth_headers(response::error(
                    StatusCode::BAD_REQUEST,
                    passkey_text(&translator, "challengeExpired"),
                ));
            }
        };

    let passkeys = match state.store.get_passkeys().await {
        Ok(passkeys) => passkeys,
        Err(error) => {
            tracing::warn!(%error, "failed to load passkeys for verification");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "verifyFailed"),
            ));
        }
    };
    let matched = passkeys
        .iter()
        .find(|passkey| passkey.get("id").and_then(Value::as_str) == Some(credential.id.as_str()));
    let Some(matched) = matched else {
        return register_passkey_failure(
            &state,
            &tracking_ip,
            &user_agent(&headers),
            "Unknown Passkey".to_string(),
            None,
            &translator,
            "notFoundWithRetry",
            "notFound",
            StatusCode::TOO_MANY_REQUESTS,
        )
        .await;
    };
    let rp_info = RpInfo {
        rp_id: state_json
            .get("rp_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        origin: state_json
            .get("origin")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        mode: state_json
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("auth_host")
            .to_string(),
    };
    let webauthn = match build_webauthn(&rp_info) {
        Ok(webauthn) => webauthn,
        Err(error) => {
            tracing::warn!(%error, "failed to rebuild passkey RP config");
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidRpConfig"),
            ));
        }
    };
    let unknown_device = passkey_text(&translator, "unknownDevice");
    let credential_name = string_field(matched, "deviceName")
        .unwrap_or(&unknown_device)
        .to_string();
    let totp_id = string_field(matched, "totpId").unwrap_or("").to_string();
    let linked_totp_name = linked_totp_name(&state, &totp_id).await;
    let auth_result = match webauthn.finish_securitykey_authentication(&credential, &auth_state) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "passkey verification failed");
            return register_passkey_failure(
                &state,
                &tracking_ip,
                &user_agent(&headers),
                credential_name,
                linked_totp_name,
                &translator,
                "verifyFailedWithRetry",
                "verifyFailed",
                StatusCode::TOO_MANY_REQUESTS,
            )
            .await;
        }
    };

    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config after passkey verification");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createSessionFailed"),
            ));
        }
    };
    let totp_credential = state
        .store
        .get_totps()
        .await
        .ok()
        .and_then(|totps| totps.into_iter().find(|totp| totp.id == totp_id));
    let created = match auth_mobility::create_login_session(
        &state,
        &config,
        CreateLoginSessionInput {
            auth_method: AuthMethod::Passkey.as_session_str().to_string(),
            auth_provider_name: None,
            credential_id: credential.id.clone(),
            credential_name: credential_name.to_string(),
            totp_id: totp_id.to_string(),
            linked_totp_name: linked_totp_name.clone(),
            totp_credential,
            client_ip: client_ip.clone(),
            user_agent: user_agent(&headers),
            remember_me: body.remember_me,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to store passkey session");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createSessionFailed"),
            ));
        }
    };
    if created.ttl_seconds <= 0 {
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "createSessionFailed"),
        ));
    }
    if let Err(error) = state
        .store
        .update_passkey_counter(
            &credential.id,
            auth_result.counter(),
            &time_utils::now_iso(),
            backup_flags.map(|flags| flags.backup_eligible),
            backup_flags.map(|flags| flags.backup_state),
        )
        .await
    {
        tracing::warn!(%error, id = %credential.id, "failed to update passkey counter");
    }
    if let Err(error) = state.store.reset_login_backoff(&tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset passkey login backoff");
    }

    let redirect_to = effective_login_redirect(
        &config,
        &headers,
        &created.grant_type,
        body.redirect_uri.as_deref(),
    );
    let cookie = cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        resolve_cookie_domain(&config, &headers).as_deref(),
    );
    let cookie_header = match HeaderValue::from_str(&cookie) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, session_id = %created.session_id, "failed to build passkey session cookie header");
            if let Err(error) = auth_mobility::destroy_session(&state, &created.session_id).await {
                tracing::warn!(%error, session_id = %created.session_id, "failed to destroy passkey session after cookie header failure");
            }
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createSessionFailed"),
            ));
        }
    };
    let mut data = json!({
        "run_type": config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        "grant_type": created.grant_type
    });
    if let Some(redirect_to) = redirect_to {
        data["redirect_to"] = Value::String(redirect_to);
    }
    let mut response = (
        [(header::SET_COOKIE, cookie_header)],
        Json(json!({
            "success": true,
            "message": passkey_text(&translator, "loginSuccessful"),
            "data": data
        })),
    )
        .into_response();
    response = with_auth_headers(response);
    response
}

async fn bind_token(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(response) = ensure_passkey_login_mode(&state, &translator).await {
        return response;
    }
    let Some(session_id) = parse_passkey_cookie_value(&headers, cookies::SESSION_COOKIE_NAME)
    else {
        return with_auth_headers(response::error(
            StatusCode::UNAUTHORIZED,
            passkey_text(&translator, "unauthorizedOrMissingTotp"),
        ));
    };
    let session = match state.store.get_session(&session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            return with_auth_headers(response::error(
                StatusCode::UNAUTHORIZED,
                passkey_text(&translator, "unauthorizedOrMissingTotp"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load passkey bind session");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createBindTokenFailed"),
            ));
        }
    };
    if session.totp_id.trim().is_empty() {
        return with_auth_headers(response::error(
            StatusCode::UNAUTHORIZED,
            passkey_text(&translator, "unauthorizedOrMissingTotp"),
        ));
    }
    match build_passkey_bind_info(&state, &session.totp_id).await {
        Ok(value) => with_auth_headers(response::ok(value).into_response()),
        Err(error) => {
            tracing::warn!(%error, "failed to create passkey bind token");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createBindTokenFailed"),
            ))
        }
    }
}

fn parse_passkey_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
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

fn passkey_device_name(value: String) -> String {
    if value.is_empty() {
        "Unknown Device".to_string()
    } else {
        value
    }
}

async fn register_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterOptionsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(response) = ensure_passkey_login_mode(&state, &translator).await {
        return response;
    }
    match state.store.is_passkey_bind_token_valid(&body.token).await {
        Ok(true) => {}
        Ok(false) => {
            return with_auth_headers(response::error(
                StatusCode::UNAUTHORIZED,
                passkey_text(&translator, "bindTokenExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to inspect passkey bind token");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createRegistrationOptionsFailed"),
            ));
        }
    }
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for passkey registration");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createRegistrationOptionsFailed"),
            ));
        }
    };
    let passkeys = match state.store.get_passkeys().await {
        Ok(passkeys) => passkeys,
        Err(error) => {
            tracing::warn!(%error, "failed to load passkeys for registration");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createRegistrationOptionsFailed"),
            ));
        }
    };
    let exclude = passkeys
        .iter()
        .filter_map(|passkey| {
            string_field(passkey, "id").and_then(|id| URL_SAFE_NO_PAD.decode(id).ok())
        })
        .collect::<Vec<_>>();
    let rp_info = rp_info(&state, &config, &headers).await;
    let webauthn = match build_webauthn(&rp_info) {
        Ok(webauthn) => webauthn,
        Err(error) => {
            tracing::warn!(%error, rp_id = %rp_info.rp_id, origin = %rp_info.origin, "invalid passkey RP config");
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidRpConfig"),
            ));
        }
    };
    let (mut options, registration_state) = match webauthn.start_securitykey_registration(
        PASSKEY_ADMIN_UUID,
        "admin",
        "admin",
        (!exclude.is_empty()).then_some(exclude),
        None,
        None,
    ) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to start passkey registration");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "createRegistrationOptionsFailed"),
            ));
        }
    };
    let registration_state =
        match require_registration_user_verification(&mut options, registration_state) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "failed to normalize passkey registration state");
                return with_auth_headers(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    passkey_text(&translator, "createRegistrationOptionsFailed"),
                ));
            }
        };
    let challenge = URL_SAFE_NO_PAD.encode(&options.public_key.challenge);
    if let Err(error) = state
        .store
        .set_passkey_challenge(&challenge, "register", PASSKEY_CHALLENGE_TTL_SECONDS)
        .await
    {
        tracing::warn!(%error, "failed to store passkey registration challenge");
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "createRegistrationOptionsFailed"),
        ));
    }
    let state_json = json!({
        "type": "register",
        "state": registration_state,
        "rp_id": rp_info.rp_id,
        "origin": rp_info.origin,
        "mode": rp_info.mode
    });
    if let Err(error) = state
        .store
        .set_passkey_state(&challenge, &state_json, PASSKEY_CHALLENGE_TTL_SECONDS)
        .await
    {
        tracing::warn!(%error, "failed to store passkey registration state");
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "createRegistrationOptionsFailed"),
        ));
    }

    with_auth_headers(response::ok(options.public_key).into_response())
}

fn require_registration_user_verification(
    options: &mut CreationChallengeResponse,
    registration_state: SecurityKeyRegistration,
) -> serde_json::Result<Value> {
    if let Some(selection) = options.public_key.authenticator_selection.as_mut() {
        selection.user_verification = UserVerificationPolicy::Required;
    }

    let mut state = serde_json::to_value(registration_state)?;
    if let Some(policy) = state.pointer_mut("/rs/policy") {
        *policy = Value::String("required".to_string());
    }
    Ok(state)
}

async fn register_verify(
    State(state): State<AppState>,
    Json(body): Json<RegisterVerifyBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Err(response) = ensure_passkey_login_mode(&state, &translator).await {
        return response;
    }
    let totp_id = match state.store.consume_passkey_bind_token(&body.token).await {
        Ok(Some(value)) if !value.trim().is_empty() => value,
        Ok(_) => {
            return with_auth_headers(response::error(
                StatusCode::UNAUTHORIZED,
                passkey_text(&translator, "bindTokenExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to consume passkey bind token");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "registerFailed"),
            ));
        }
    };
    let challenge = match extract_challenge(&body.credential) {
        Some(value) => value,
        None => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidResponse"),
            ));
        }
    };
    match state
        .store
        .consume_passkey_challenge(&challenge, "register")
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "challengeExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to consume passkey registration challenge");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "registerFailed"),
            ));
        }
    }
    let state_json = match state.store.consume_passkey_state(&challenge).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "challengeExpired"),
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to consume passkey registration state");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "registerFailed"),
            ));
        }
    };
    let registration_state: SecurityKeyRegistration =
        match serde_json::from_value(state_json.get("state").cloned().unwrap_or(Value::Null)) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "failed to decode passkey registration state");
                return with_auth_headers(response::error(
                    StatusCode::BAD_REQUEST,
                    passkey_text(&translator, "challengeExpired"),
                ));
            }
        };
    let credential: RegisterPublicKeyCredential =
        match serde_json::from_value(body.credential.clone()) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "failed to decode passkey registration credential");
                return with_auth_headers(response::error(
                    StatusCode::BAD_REQUEST,
                    passkey_text(&translator, "invalidResponse"),
                ));
            }
        };
    let rp_info = RpInfo {
        rp_id: state_json
            .get("rp_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        origin: state_json
            .get("origin")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        mode: state_json
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("auth_host")
            .to_string(),
    };
    let webauthn = match build_webauthn(&rp_info) {
        Ok(webauthn) => webauthn,
        Err(error) => {
            tracing::warn!(%error, "failed to rebuild passkey registration RP config");
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                passkey_text(&translator, "invalidRpConfig"),
            ));
        }
    };
    let security_key =
        match webauthn.finish_securitykey_registration(&credential, &registration_state) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, "passkey registration verification failed");
                return with_auth_headers(response::error(
                    StatusCode::BAD_REQUEST,
                    passkey_text(&translator, "registrationFailed"),
                ));
            }
        };
    let stored_credential: Credential = security_key.into();
    let id = URL_SAFE_NO_PAD.encode(&stored_credential.cred_id);
    let passkeys = match state.store.get_passkeys().await {
        Ok(passkeys) => passkeys,
        Err(error) => {
            tracing::warn!(%error, "failed to inspect passkeys after registration");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "registerFailed"),
            ));
        }
    };
    if passkeys
        .iter()
        .any(|passkey| passkey.get("id").and_then(Value::as_str) == Some(id.as_str()))
    {
        return with_auth_headers(response::error(
            StatusCode::CONFLICT,
            passkey_text(&translator, "alreadyRegistered"),
        ));
    }
    let public_key = match cose_key_to_base64url(&stored_credential.cred) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to encode registered passkey public key");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                passkey_text(&translator, "registerFailed"),
            ));
        }
    };
    let transports = credential
        .response
        .transports
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|transport| Value::String(transport.to_string()))
        .collect::<Vec<_>>();
    let device_name = passkey_device_name(body.device_name);
    let passkey = json!({
        "id": id,
        "totpId": totp_id,
        "publicKey": public_key,
        "counter": 0,
        "transports": transports,
        "deviceName": device_name,
        "createdAt": time_utils::now_iso(),
        "webauthnCredential": stored_credential
    });
    if let Err(error) = state.store.add_passkey(&passkey).await {
        tracing::warn!(%error, "failed to store registered passkey");
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            passkey_text(&translator, "registerFailed"),
        ));
    }
    with_auth_headers(response::success_empty().into_response())
}

pub(crate) async fn public_passkey_status(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
) -> Value {
    let passkey_count = state
        .store
        .get_passkeys()
        .await
        .map(|items| items.len())
        .unwrap_or(0);
    let passkey_login_enabled = state
        .store
        .get_auth_login_mode()
        .await
        .map(AuthLoginMode::allows_totp_family)
        .unwrap_or(false);
    let rp = rp_info(state, config, headers).await;
    let request_host = request_hostname(headers);
    let shared_auth_host = public_auth_base_host(config);
    let available_on_host = if rp.mode == "parent_domain" {
        !rp.rp_id.is_empty()
            && (request_host == rp.rp_id || request_host.ends_with(&format!(".{}", rp.rp_id)))
    } else {
        shared_auth_host.is_empty() || request_host == shared_auth_host
    };
    json!({
        "available": passkey_login_enabled && passkey_count > 0 && available_on_host,
        "mode": rp.mode,
        "rp_id": rp.rp_id
    })
}

pub(crate) async fn build_passkey_bind_info(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<Value> {
    let passkeys = state.store.get_passkeys().await?;
    let credential_ids = passkeys
        .iter()
        .filter(|passkey| passkey.get("totpId").and_then(Value::as_str) == Some(totp_id))
        .filter_map(|passkey| {
            passkey
                .get("id")
                .and_then(Value::as_str)
                .map(|value| Value::String(value.to_string()))
        })
        .collect::<Vec<_>>();
    let token = state
        .store
        .create_passkey_bind_token(totp_id, PASSKEY_BIND_TTL_SECONDS)
        .await?;
    Ok(json!({
        "available": !credential_ids.is_empty(),
        "can_bind": true,
        "bind_token": token,
        "token": token,
        "credential_ids": credential_ids
    }))
}

async fn linked_totp_name(state: &AppState, totp_id: &str) -> Option<String> {
    state
        .store
        .get_totps()
        .await
        .ok()?
        .into_iter()
        .find(|totp| totp.id == totp_id)
        .map(|totp| totp.comment)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn register_passkey_failure(
    state: &AppState,
    tracking_ip: &str,
    user_agent: &str,
    credential_name: String,
    linked_totp_name: Option<String>,
    translator: &Translator,
    retry_key: &str,
    fallback_key: &str,
    status: StatusCode,
) -> Response {
    match state
        .store
        .register_login_backoff_failure(tracking_ip)
        .await
    {
        Ok(failure) => {
            let retry_after = failure.retry_after.unwrap_or(1).max(1);
            if let Err(error) = system_events::publish_auth_login_failure_event(
                state,
                json!({
                    "ip": tracking_ip,
                    "attempts": failure.attempts,
                    "retry_after_seconds": retry_after,
                    "blocked_until": failure.blocked_until.map(time_utils::iso_from_ms),
                    "method": AuthMethod::Passkey.as_session_str(),
                    "credential_name": credential_name,
                    "linked_totp_name": linked_totp_name,
                    "user_agent": user_agent,
                }),
            )
            .await
            {
                tracing::warn!(%error, %tracking_ip, "failed to publish passkey login failure event");
            }
            let message = passkey_text_params(
                translator,
                retry_key,
                &[("seconds", retry_after.to_string())],
            );
            with_auth_headers(passkey_backoff_response(
                &message,
                retry_after,
                failure.blocked_until,
            ))
        }
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to register passkey auth failure");
            with_auth_headers(response::error(
                status,
                passkey_text(translator, fallback_key),
            ))
        }
    }
}

fn passkey_backoff_response(
    message: &str,
    retry_after: i64,
    blocked_until: Option<i64>,
) -> Response {
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
    response
}

fn passkey_to_security_key(value: &Value) -> Option<SecurityKey> {
    if let Some(credential) = value.get("webauthnCredential") {
        return match serde_json::from_value::<Credential>(credential.clone()) {
            Ok(credential) => Some(SecurityKey::from(credential)),
            Err(error) => {
                tracing::warn!(%error, "failed to decode stored passkey credential");
                None
            }
        };
    }

    let id = string_field(value, "id")?;
    let public_key = string_field(value, "publicKey")?;
    let cred_id = URL_SAFE_NO_PAD.decode(id).ok()?;
    let public_key_bytes = URL_SAFE_NO_PAD.decode(public_key).ok()?;
    let cose_value: serde_cbor_2::Value = serde_cbor_2::from_slice(&public_key_bytes).ok()?;
    let cose_key = COSEKey::try_from(&cose_value).ok()?;
    let transports = value
        .get("transports")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| AuthenticatorTransport::from_str(value).ok())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());
    let counter = value
        .get("counter")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let backup_eligible = bool_field(value, "backupEligible")
        .or_else(|| bool_field(value, "backup_eligible"))
        .unwrap_or(false);
    let backup_state = bool_field(value, "backupState")
        .or_else(|| bool_field(value, "backup_state"))
        .unwrap_or(false);
    let credential = Credential {
        cred_id,
        cred: cose_key,
        counter,
        transports,
        user_verified: false,
        backup_eligible,
        backup_state,
        registration_policy: UserVerificationPolicy::Preferred,
        extensions: RegisteredExtensions::none(),
        attestation: ParsedAttestation {
            data: ParsedAttestationData::None,
            metadata: AttestationMetadata::None,
        },
        attestation_format: AttestationFormat::None,
    };
    Some(SecurityKey::from(credential))
}

fn cose_key_to_base64url(key: &COSEKey) -> anyhow::Result<String> {
    let mut map = BTreeMap::<serde_cbor_2::Value, serde_cbor_2::Value>::new();
    map.insert(
        serde_cbor_2::Value::Integer(1),
        serde_cbor_2::Value::Integer(cose_key_type_id(&key.key)),
    );
    map.insert(
        serde_cbor_2::Value::Integer(3),
        serde_cbor_2::Value::Integer(cose_algorithm_id(key.type_)),
    );
    match &key.key {
        COSEKeyType::EC_EC2(ec) => {
            map.insert(
                serde_cbor_2::Value::Integer(-1),
                serde_cbor_2::Value::Integer(ecdsa_curve_id(&ec.curve)),
            );
            map.insert(
                serde_cbor_2::Value::Integer(-2),
                serde_cbor_2::Value::Bytes(ec.x.clone()),
            );
            map.insert(
                serde_cbor_2::Value::Integer(-3),
                serde_cbor_2::Value::Bytes(ec.y.clone()),
            );
        }
        COSEKeyType::RSA(rsa) => {
            map.insert(
                serde_cbor_2::Value::Integer(-1),
                serde_cbor_2::Value::Bytes(rsa.n.clone()),
            );
            map.insert(
                serde_cbor_2::Value::Integer(-2),
                serde_cbor_2::Value::Bytes(rsa.e.to_vec()),
            );
        }
        COSEKeyType::EC_OKP(okp) => {
            map.insert(
                serde_cbor_2::Value::Integer(-1),
                serde_cbor_2::Value::Integer(eddsa_curve_id(&okp.curve)),
            );
            map.insert(
                serde_cbor_2::Value::Integer(-2),
                serde_cbor_2::Value::Bytes(okp.x.clone()),
            );
        }
    }
    let cbor = serde_cbor_2::to_vec(&serde_cbor_2::Value::Map(map.into_iter().collect()))?;
    Ok(URL_SAFE_NO_PAD.encode(cbor))
}

fn cose_key_type_id(key: &COSEKeyType) -> i128 {
    match key {
        COSEKeyType::EC_OKP(_) => 1,
        COSEKeyType::EC_EC2(_) => 2,
        COSEKeyType::RSA(_) => 3,
    }
}

fn cose_algorithm_id(algorithm: COSEAlgorithm) -> i128 {
    match algorithm {
        COSEAlgorithm::ES256 => -7,
        COSEAlgorithm::ES384 => -35,
        COSEAlgorithm::ES521 => -36,
        COSEAlgorithm::RS256 => -257,
        COSEAlgorithm::RS384 => -258,
        COSEAlgorithm::RS512 => -259,
        COSEAlgorithm::PS256 => -37,
        COSEAlgorithm::PS384 => -38,
        COSEAlgorithm::PS512 => -39,
        COSEAlgorithm::EDDSA => -8,
        COSEAlgorithm::INSECURE_RS1 => -65535,
        COSEAlgorithm::PinUvProtocol => -25,
    }
}

fn ecdsa_curve_id(curve: &ECDSACurve) -> i128 {
    match curve {
        ECDSACurve::SECP256R1 => 1,
        ECDSACurve::SECP384R1 => 2,
        ECDSACurve::SECP521R1 => 3,
    }
}

fn eddsa_curve_id(curve: &EDDSACurve) -> i128 {
    match curve {
        EDDSACurve::ED25519 => 6,
        EDDSACurve::ED448 => 7,
    }
}

fn extract_challenge(credential: &Value) -> Option<String> {
    let encoded = credential
        .pointer("/response/clientDataJSON")
        .and_then(Value::as_str)?;
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    let data: Value = serde_json::from_slice(&bytes).ok()?;
    data.get("challenge")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthenticatorBackupFlags {
    backup_eligible: bool,
    backup_state: bool,
}

fn authenticator_backup_flags(credential: &Value) -> Option<AuthenticatorBackupFlags> {
    let encoded = credential
        .pointer("/response/authenticatorData")
        .and_then(Value::as_str)?;
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    if bytes.len() < 37 {
        return None;
    }
    let flags = bytes[32];
    Some(AuthenticatorBackupFlags {
        backup_eligible: flags & 0x08 != 0,
        backup_state: flags & 0x10 != 0,
    })
}

fn patch_authentication_state_backup_flags(
    state_json: &mut Value,
    credential_id: &str,
    flags: AuthenticatorBackupFlags,
) {
    let Some(credentials) = state_json
        .pointer_mut("/state/ast/credentials")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for credential in credentials {
        if !credential_id_matches(credential.get("cred_id"), credential_id) {
            continue;
        }
        if let Some(object) = credential.as_object_mut() {
            object.insert(
                "backup_eligible".to_string(),
                Value::Bool(flags.backup_eligible),
            );
            object.insert("backup_state".to_string(), Value::Bool(flags.backup_state));
        }
    }
}

fn credential_id_matches(value: Option<&Value>, credential_id: &str) -> bool {
    match value {
        Some(Value::String(value)) => value == credential_id,
        Some(Value::Array(items)) => {
            let Some(expected) = URL_SAFE_NO_PAD.decode(credential_id).ok() else {
                return false;
            };
            let actual = items
                .iter()
                .map(|item| item.as_u64().and_then(|value| u8::try_from(value).ok()))
                .collect::<Option<Vec<_>>>();
            actual.as_deref() == Some(expected.as_slice())
        }
        _ => false,
    }
}

fn build_webauthn(rp_info: &RpInfo) -> anyhow::Result<Webauthn> {
    let origin = Url::parse(&rp_info.origin)?;
    let builder = WebauthnBuilder::new(&rp_info.rp_id, &origin)?
        .rp_name(RP_NAME)
        .allow_subdomains(rp_info.mode == "parent_domain")
        .allow_any_port(true);
    Ok(builder.build()?)
}

#[derive(Clone)]
struct RpInfo {
    rp_id: String,
    origin: String,
    mode: String,
}

async fn rp_info(state: &AppState, config: &Value, headers: &HeaderMap) -> RpInfo {
    let configured_host = configured_rp_host(state).await;
    rp_info_with_configured_host(config, headers, configured_host.as_deref())
}

fn rp_info_with_configured_host(
    config: &Value,
    headers: &HeaderMap,
    configured_host: Option<&str>,
) -> RpInfo {
    let request_url = Url::parse("http://127.0.0.1").unwrap();
    let forwarded_proto = parse_forwarded_header_value(headers, "proto");
    let proto = forwarded_proto
        .or_else(|| first_header(headers, "x-forwarded-proto"))
        .or_else(|| first_header(headers, "x-forwarded-scheme"))
        .map(|value| value.trim().trim_end_matches(':').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| request_url.scheme().to_string());
    let forwarded_host = parse_forwarded_header_value(headers, "host")
        .or_else(|| first_header(headers, "x-forwarded-host"))
        .or_else(|| first_header(headers, "x-original-host"));
    let direct_host = first_header(headers, "host").unwrap_or_else(|| "127.0.0.1".to_string());
    let configured_auth_url = crate::auth::resolve_public_auth_base_url(config)
        .and_then(|value| parse_absolute_url(&value));
    let selected_url = pick_preferred_url(vec![
        first_header(headers, "origin").and_then(|value| parse_absolute_url(&value)),
        first_header(headers, "referer").and_then(|value| parse_absolute_url(&value)),
        build_absolute_url_from_host(forwarded_host.as_deref(), &proto),
        build_absolute_url_from_host(Some(&direct_host), &proto),
        configured_auth_url.clone(),
        Some(request_url.clone()),
    ]);
    let effective_origin_url = selected_url
        .clone()
        .or_else(|| configured_auth_url.clone())
        .or_else(|| build_absolute_url_from_host(Some(&direct_host), &proto))
        .unwrap_or(request_url.clone());
    let mode = config
        .pointer("/subdomain_mode/passkey_rp_mode")
        .and_then(Value::as_str)
        .filter(|value| *value == "parent_domain")
        .unwrap_or("auth_host")
        .to_string();
    let parent_rp_id = config
        .pointer("/subdomain_mode/passkey_rp_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            config
                .pointer("/subdomain_mode/root_domain")
                .and_then(Value::as_str)
        })
        .map(normalize_host_like)
        .unwrap_or_default();
    let rp_id = if mode == "parent_domain" && !parent_rp_id.is_empty() {
        parent_rp_id
    } else if let Some(selected_url) = selected_url.as_ref()
        && !is_loopback_hostname(selected_url.host_str().unwrap_or(""))
    {
        selected_url.host_str().unwrap_or("").to_string()
    } else if let Some(configured_host) = configured_host
        && let Some(configured_url) = build_absolute_url_from_host(Some(&configured_host), &proto)
    {
        configured_url.host_str().unwrap_or("").to_string()
    } else {
        request_url.host_str().unwrap_or("127.0.0.1").to_string()
    };
    RpInfo {
        rp_id: if rp_id.is_empty() {
            "localhost".to_string()
        } else {
            rp_id
        },
        origin: effective_origin_url.origin().ascii_serialization(),
        mode,
    }
}

fn public_auth_base_host(config: &Value) -> String {
    crate::auth::resolve_public_auth_base_url(config)
        .and_then(|value| Url::parse(&value).ok())
        .and_then(|url| url.host_str().map(normalize_host))
        .unwrap_or_default()
}

async fn configured_rp_host(state: &AppState) -> Option<String> {
    let hosts = state.store.get_json_value(CA_HOSTS_KEY).await.ok()??;
    hosts
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.contains('*'))
        .find_map(|host| {
            build_absolute_url_from_host(Some(host), "https").and_then(|url| {
                let hostname = url.host_str()?;
                (!is_loopback_hostname(hostname)).then(|| host.to_string())
            })
        })
}

fn parse_forwarded_header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    crate::http_utils::forwarded_header_value(headers, key)
}

fn parse_absolute_url(value: &str) -> Option<Url> {
    Url::parse(value).ok()
}

fn build_absolute_url_from_host(host: Option<&str>, proto: &str) -> Option<Url> {
    let host = host?.trim();
    if host.is_empty() {
        return None;
    }
    parse_absolute_url(&format!(
        "{}://{}",
        proto.trim().trim_end_matches(':'),
        host
    ))
}

fn pick_preferred_url(candidates: Vec<Option<Url>>) -> Option<Url> {
    let urls = candidates.into_iter().flatten().collect::<Vec<_>>();
    urls.iter()
        .find(|url| !is_loopback_hostname(url.host_str().unwrap_or("")))
        .cloned()
        .or_else(|| urls.into_iter().next())
}

fn is_loopback_hostname(hostname: &str) -> bool {
    let normalized = hostname.trim().to_ascii_lowercase();
    normalized == "localhost"
        || normalized == "::1"
        || normalized == "[::1]"
        || normalized == "0.0.0.0"
        || normalized == "[::]"
        || normalized.starts_with("127.")
}

fn normalize_host_like(value: &str) -> String {
    let mut value = value.trim().to_ascii_lowercase();
    if let Some((_, rest)) = value.split_once("://") {
        value = rest.to_string();
    }
    value = value.split('/').next().unwrap_or("").to_string();
    value.trim_end_matches('.').to_string()
}

use crate::auth::resolve_cookie_domain;

fn request_hostname(headers: &HeaderMap) -> String {
    parse_forwarded_header_value(headers, "host")
        .or_else(|| first_header(headers, "x-forwarded-host"))
        .or_else(|| first_header(headers, "x-original-host"))
        .or_else(|| first_header(headers, "host"))
        .map(|value| normalize_host(&value))
        .unwrap_or_default()
}

fn normalize_host(value: &str) -> String {
    let without_scheme = value
        .trim()
        .to_ascii_lowercase()
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| value.trim().to_ascii_lowercase());
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string();
    if host.is_empty() {
        return host;
    }
    if let Ok(url) = Url::parse(&format!("https://{host}"))
        && let Some(hostname) = url.host_str()
    {
        return hostname.trim_end_matches('.').to_string();
    }
    if let Some((hostname, port)) = host.rsplit_once(':')
        && !hostname.contains(':')
        && !hostname.is_empty()
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        return hostname.trim_end_matches('.').to_string();
    }
    host
}

use crate::http_utils::first_header_value as first_header;

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

#[cfg(test)]
mod tests;
