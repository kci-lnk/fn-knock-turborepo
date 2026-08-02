use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    auth::{
        mode::{AuthLoginMode, AuthMethod},
        routes::{
            CaptchaSubmission, backoff_login_response, effective_login_redirect,
            resolve_cookie_domain, user_agent, verify_captcha,
        },
    },
    auth_mobility::{self, CreateLoginSessionInput},
    cookies, crypto_utils,
    i18n::Translator,
    response::{self, ApiEnvelope},
    state::AppState,
    system_events, time_utils,
};

use super::{
    client::{LdapProfile, authenticate},
    provider::provider_ready,
    storage::{
        claim_binding_and_consume_invite, delete_binding, get_binding_by_subject, get_provider,
        inspect_invite, public_providers, update_binding_if_owned,
    },
};

#[derive(Deserialize)]
struct InviteQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
struct BindBody {
    token: String,
    username: String,
    password: String,
    captcha: CaptchaSubmission,
    #[serde(default, rename = "rememberMe")]
    remember_me: bool,
    redirect_uri: Option<String>,
}

fn text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.ldap.{key}"))
}

fn text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.ldap.{key}"), params)
}

pub(crate) fn ldap_runtime_routes() -> Router<AppState> {
    Router::new()
        .route("/ldap/invite", get(invite))
        .route("/ldap/bind", post(bind_identity))
}

pub(crate) async fn ldap_public_providers(
    state: &AppState,
) -> crate::storage::StorageResult<Vec<Value>> {
    public_providers(state).await
}

async fn invite(State(state): State<AppState>, Query(query): Query<InviteQuery>) -> Response {
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for LDAP invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load LDAP invitation",
            );
        }
    };
    let translator = Translator::new(
        config
            .pointer("/locale/default_locale")
            .and_then(Value::as_str)
            .unwrap_or(crate::i18n::DEFAULT_LOCALE),
    );
    match state.store.get_auth_login_mode().await {
        Ok(AuthLoginMode::Totp) => {}
        Ok(AuthLoginMode::Password) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                text(&translator, "loginMethodUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load auth mode for LDAP invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    }
    let token = query.token.as_deref().map(str::trim).unwrap_or("");
    if token.is_empty() {
        return response::error(StatusCode::BAD_REQUEST, text(&translator, "inviteInvalid"));
    }
    let token_hash = crypto_utils::sha256_hex_str(token);
    let mut invitation = match inspect_invite(&state, &token_hash).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "inviteExpired"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to inspect LDAP invitation");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    };
    let provider_id = invitation
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let provider = match get_provider(&state, &provider_id).await {
        Ok(Some(provider))
            if provider.get("id").and_then(Value::as_str) == Some(provider_id.as_str())
                && provider.get("enabled").and_then(Value::as_bool) == Some(true)
                && provider_ready(&provider) =>
        {
            provider
        }
        Ok(_) => {
            return response::error(
                StatusCode::NOT_FOUND,
                text(&translator, "providerUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %provider_id, "failed to load LDAP provider for invitation");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    };
    let totp_id = invitation
        .get("totp_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let totp = match state.store.get_totps().await {
        Ok(items) => items.into_iter().find(|item| item.id == totp_id),
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP for LDAP invitation");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    };
    let Some(totp) = totp else {
        return response::error(StatusCode::NOT_FOUND, text(&translator, "totpMissing"));
    };
    if let Some(object) = invitation.as_object_mut() {
        object.remove("token_hash");
        object.insert(
            "totp".into(),
            json!({ "id": totp.id, "comment": totp.comment }),
        );
        object.insert(
            "provider".into(),
            json!({
                "id": provider_id,
                "name": provider.get("name").cloned().unwrap_or_default(),
                "protocol": "ldap",
            }),
        );
        object.insert(
            "locale".into(),
            config.get("locale").cloned().unwrap_or_default(),
        );
        object.insert(
            "appearance".into(),
            config.get("appearance").cloned().unwrap_or_default(),
        );
    }
    response::ok(invitation).into_response()
}

async fn bind_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<BindBody>,
) -> Response {
    let client_ip = super::super::routes::client_ip_for_auth(&headers);
    let tracking_ip = crate::backoff::normalize_auth_failure_tracking_ip(&client_ip);
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config during LDAP binding");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load authentication config",
            );
        }
    };
    let translator = Translator::new(
        config
            .pointer("/locale/default_locale")
            .and_then(Value::as_str)
            .unwrap_or(crate::i18n::DEFAULT_LOCALE),
    );
    match state.store.get_auth_login_mode().await {
        Ok(AuthLoginMode::Totp) => {}
        Ok(AuthLoginMode::Password) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                text(&translator, "loginMethodUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load auth mode during LDAP binding");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    }
    match state.store.get_login_backoff_status(&tracking_ip).await {
        Ok(status) if status.blocked => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            return backoff_login_response(
                &text_params(
                    &translator,
                    "invalidCredentialsWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to inspect LDAP binding backoff");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    }
    if let Err(message) = verify_captcha(&state, &body.captcha, &client_ip, &translator).await {
        return response::error(StatusCode::BAD_REQUEST, message);
    }
    let token_hash = crypto_utils::sha256_hex_str(body.token.trim());
    let invitation = match inspect_invite(&state, &token_hash).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "inviteExpired"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to inspect LDAP invitation during binding");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    };
    let provider_id = invitation
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let totp_id = invitation
        .get("totp_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    match state.store.get_totps().await {
        Ok(totps) if totps.iter().any(|totp| totp.id == totp_id) => {}
        Ok(_) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "totpMissing"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP during LDAP binding");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    }
    let provider = match enabled_provider(&state, provider_id).await {
        Ok(provider) => provider,
        Err(response) => return response,
    };
    let profile = match authenticate(&provider, &body.username, &body.password).await {
        Ok(profile) => profile,
        Err(error) if error.is_authentication_failure() => {
            return register_failure(&state, &headers, &translator, &tracking_ip, &provider).await;
        }
        Err(error) => {
            tracing::warn!(provider_id, %error, "LDAP provider failed during binding");
            return response::error(
                StatusCode::SERVICE_UNAVAILABLE,
                text(&translator, "serviceUnavailable"),
            );
        }
    };
    let now = time_utils::now_iso();
    let binding = json!({
        "id": format!("ldap_binding_{}", uuid::Uuid::new_v4().simple()),
        "provider_id": provider_id,
        "provider_type": provider.get("type").cloned().unwrap_or_default(),
        "totp_id": totp_id,
        "subject": profile.subject,
        "subject_key": profile.subject_key,
        "dn": profile.dn,
        "username": profile.username,
        "display_name": profile.display_name,
        "email": profile.email,
        "created_at": now,
        "updated_at": now,
        "last_used_at": now,
    });
    let resolved = match claim_binding_and_consume_invite(&state, &token_hash, &binding).await {
        Ok(Some(binding)) => binding,
        Ok(None) => {
            return response::error(StatusCode::CONFLICT, text(&translator, "bindingConflict"));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to claim LDAP binding");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "bindingFailed"),
            );
        }
    };
    create_session_response(
        &state,
        &headers,
        &config,
        &translator,
        &provider,
        &resolved,
        &profile,
        body.remember_me,
        body.redirect_uri.as_deref(),
        &client_ip,
        &tracking_ip,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn login(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
    provider_id: &str,
    username: &str,
    password: &str,
    remember_me: bool,
    redirect_uri: Option<&str>,
    client_ip: &str,
    tracking_ip: &str,
) -> Response {
    let provider = match enabled_provider(state, provider_id).await {
        Ok(provider) => provider,
        Err(response) => return response,
    };
    let profile = match authenticate(&provider, username, password).await {
        Ok(profile) => profile,
        Err(error) if error.is_authentication_failure() => {
            return register_failure(state, headers, translator, tracking_ip, &provider).await;
        }
        Err(error) => {
            tracing::warn!(provider_id, %error, "LDAP provider failed during login");
            return response::error(
                StatusCode::SERVICE_UNAVAILABLE,
                text(translator, "serviceUnavailable"),
            );
        }
    };
    let mut binding = match get_binding_by_subject(state, &profile.subject_key).await {
        Ok(Some(binding)) => binding,
        Ok(None) => {
            return register_failure(state, headers, translator, tracking_ip, &provider).await;
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load LDAP binding during login");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "bindingFailed"),
            );
        }
    };
    if !binding_matches_identity(&binding, provider_id, &profile.subject_key) {
        tracing::warn!(
            provider_id,
            binding_id = binding
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or(""),
            "rejected mismatched LDAP subject index"
        );
        return register_failure(state, headers, translator, tracking_ip, &provider).await;
    }
    if let Some(object) = binding.as_object_mut() {
        object.insert("dn".into(), Value::String(profile.dn.clone()));
        object.insert("username".into(), Value::String(profile.username.clone()));
        object.insert(
            "display_name".into(),
            profile
                .display_name
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        object.insert(
            "email".into(),
            profile
                .email
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        object.insert("updated_at".into(), Value::String(time_utils::now_iso()));
        object.insert("last_used_at".into(), Value::String(time_utils::now_iso()));
    }
    match update_binding_if_owned(state, &binding).await {
        Ok(true) => {}
        Ok(false) => {
            return register_failure(state, headers, translator, tracking_ip, &provider).await;
        }
        Err(error) => {
            tracing::warn!(%error, "failed to update LDAP binding profile");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "bindingFailed"),
            );
        }
    }
    create_session_response(
        state,
        headers,
        config,
        translator,
        &provider,
        &binding,
        &profile,
        remember_me,
        redirect_uri,
        client_ip,
        tracking_ip,
    )
    .await
}

async fn enabled_provider(state: &AppState, provider_id: &str) -> Result<Value, Response> {
    match get_provider(state, provider_id).await {
        Ok(Some(provider))
            if provider.get("id").and_then(Value::as_str) == Some(provider_id)
                && provider.get("enabled").and_then(Value::as_bool) == Some(true)
                && provider_ready(&provider) =>
        {
            Ok(provider)
        }
        Ok(_) => {
            let translator = Translator::from_state(state).await;
            Err(response::error(
                StatusCode::BAD_REQUEST,
                text(&translator, "providerUnavailable"),
            ))
        }
        Err(error) => {
            tracing::warn!(%error, %provider_id, "failed to load LDAP provider");
            let translator = Translator::from_state(state).await;
            Err(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "serviceUnavailable"),
            ))
        }
    }
}

fn binding_matches_identity(binding: &Value, provider_id: &str, subject_key: &str) -> bool {
    !provider_id.is_empty()
        && !subject_key.is_empty()
        && binding.get("provider_id").and_then(Value::as_str) == Some(provider_id)
        && binding.get("subject_key").and_then(Value::as_str) == Some(subject_key)
}

#[cfg(test)]
pub(super) fn binding_matches_identity_for_test(
    binding: &Value,
    provider_id: &str,
    subject_key: &str,
) -> bool {
    binding_matches_identity(binding, provider_id, subject_key)
}

async fn register_failure(
    state: &AppState,
    headers: &HeaderMap,
    translator: &Translator,
    tracking_ip: &str,
    provider: &Value,
) -> Response {
    let provider_id = provider.get("id").and_then(Value::as_str).unwrap_or("");
    let provider_name = provider
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("LDAP");
    match state
        .store
        .register_login_backoff_failure(tracking_ip)
        .await
    {
        Ok(status) => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            if let Err(error) = system_events::publish_auth_login_failure_event(
                state,
                json!({
                    "ip": tracking_ip,
                    "attempts": status.attempts,
                    "retry_after_seconds": retry_after,
                    "blocked_until": status.blocked_until.map(time_utils::iso_from_ms),
                    "method": AuthMethod::Ldap.as_session_str(),
                    "provider_id": provider_id,
                    "auth_provider_name": provider_name,
                    "credential_name": "! Unknown LDAP account",
                    "user_agent": user_agent(headers),
                }),
            )
            .await
            {
                tracing::warn!(%error, %tracking_ip, "failed to publish LDAP login failure event");
            }
            backoff_login_response(
                &text_params(
                    translator,
                    "invalidCredentialsWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            )
        }
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to register LDAP login failure");
            response::error(
                StatusCode::TOO_MANY_REQUESTS,
                text(translator, "invalidCredentials"),
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_session_response(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
    provider: &Value,
    binding: &Value,
    profile: &LdapProfile,
    remember_me: bool,
    redirect_uri: Option<&str>,
    client_ip: &str,
    tracking_ip: &str,
) -> Response {
    match state.store.get_auth_login_mode().await {
        Ok(AuthLoginMode::Totp) => {}
        Ok(AuthLoginMode::Password) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                text(translator, "loginMethodUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to confirm auth mode for LDAP session");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "createSessionFailed"),
            );
        }
    }
    let totp_id = binding.get("totp_id").and_then(Value::as_str).unwrap_or("");
    let totp = match state.store.get_totps().await {
        Ok(items) => items.into_iter().find(|item| item.id == totp_id),
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP for LDAP session");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "createSessionFailed"),
            );
        }
    };
    let credential_name = profile
        .display_name
        .as_deref()
        .filter(|value| !value.is_empty())
        .or_else(|| profile.username.split_once('@').map(|(name, _)| name))
        .filter(|value| !value.is_empty())
        .unwrap_or("Directory account");
    let Some(totp) = totp else {
        if let Some(binding_id) = binding.get("id").and_then(Value::as_str)
            && let Err(error) = delete_binding(state, binding_id).await
        {
            tracing::warn!(%error, %binding_id, "failed to remove orphaned LDAP binding");
        }
        return response::error(StatusCode::CONFLICT, text(translator, "totpMissing"));
    };
    let linked_totp_name = Some(totp.comment.trim().to_string()).filter(|value| !value.is_empty());
    let provider_name = provider
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("LDAP");
    let created = match auth_mobility::create_login_session(
        state,
        config,
        CreateLoginSessionInput {
            auth_method: AuthMethod::Ldap.as_session_str().into(),
            auth_provider_name: Some(provider_name.into()),
            credential_id: binding
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            credential_name: credential_name.into(),
            totp_id: totp_id.into(),
            linked_totp_name,
            totp_credential: Some(totp),
            client_ip: client_ip.into(),
            user_agent: user_agent(headers),
            remember_me,
        },
    )
    .await
    {
        Ok(created) => created,
        Err(error) => {
            tracing::warn!(%error, "failed to create LDAP auth session");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "createSessionFailed"),
            );
        }
    };
    match state.store.get_auth_login_mode().await {
        Ok(AuthLoginMode::Totp) => {}
        Ok(AuthLoginMode::Password) => {
            if let Err(error) = auth_mobility::destroy_session(state, &created.session_id).await {
                tracing::warn!(%error, "failed to revoke LDAP session after auth mode changed");
            }
            return response::error(
                StatusCode::BAD_REQUEST,
                text(translator, "loginMethodUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to recheck auth mode after LDAP session creation");
            if let Err(revoke_error) =
                auth_mobility::destroy_session(state, &created.session_id).await
            {
                tracing::warn!(%revoke_error, "failed to revoke LDAP session after auth mode check failed");
            }
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "createSessionFailed"),
            );
        }
    }
    if let Err(error) = state.store.reset_login_backoff(tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset LDAP login backoff");
    }
    let cookie = cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        resolve_cookie_domain(config, headers).as_deref(),
    );
    let cookie_header = match HeaderValue::from_str(&cookie) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to build LDAP session cookie");
            let _ = auth_mobility::destroy_session(state, &created.session_id).await;
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(translator, "createSessionFailed"),
            );
        }
    };
    let mut data = json!({
        "run_type": config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        "grant_type": created.grant_type,
    });
    if let Some(redirect) =
        effective_login_redirect(config, headers, &created.grant_type, redirect_uri)
    {
        data["redirect_to"] = Value::String(redirect);
    }
    let mut response = (
        [(header::SET_COOKIE, cookie_header)],
        Json(ApiEnvelope {
            success: true,
            code: None,
            message: Some(text(translator, "loginSuccessful")),
            data: Some(data),
        }),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
