use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{i18n::Translator, response, state::AppState, time_utils};

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

fn oidc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.oidc.{key}"))
}

fn oidc_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.oidc.{key}"), params)
}

async fn catalog(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(json!({ "providers": provider_catalog(&translator) })).into_response()
}

async fn list_providers(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let translator = Translator::from_state(&state).await;
    let providers = match oidc_list_providers(&state).await {
        Ok(providers) => providers,
        Err(error) => {
            tracing::warn!(%error, "failed to list OIDC providers");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "listProvidersFailed"),
            );
        }
    };
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for OIDC provider callback URLs");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let callback_base_url = callback_base_url(&headers, &uri, &config);
    let views = providers
        .into_iter()
        .map(|provider| mask_provider(provider, callback_base_url.as_deref()))
        .collect::<Vec<_>>();
    response::ok(json!({ "providers": views })).into_response()
}

async fn create_provider(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(object) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "providerPayloadObject"),
        );
    };
    match build_new_provider(object, &translator) {
        Ok(provider) => match oidc_save_provider(&state, &provider).await {
            Ok(()) => response::ok(mask_provider(provider, None)).into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to save OIDC provider");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    oidc_text(&translator, "createProviderFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(object) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "providerPayloadObject"),
        );
    };
    let existing = match oidc_get_provider(&state, &id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                oidc_text(&translator, "providerNotFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load OIDC provider for update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadProviderFailed"),
            );
        }
    };
    match build_updated_provider(existing, object, &translator) {
        Ok(provider) => match oidc_save_provider(&state, &provider).await {
            Ok(()) => response::ok(mask_provider(provider, None)).into_response(),
            Err(error) => {
                tracing::warn!(%error, %id, "failed to save OIDC provider update");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    oidc_text(&translator, "updateProviderFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

async fn delete_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match oidc_get_provider(&state, &id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                oidc_text(&translator, "providerNotFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load OIDC provider for delete");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadProviderFailed"),
            );
        }
    }
    match oidc_delete_provider(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete OIDC provider");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "deleteProviderFailed"),
            )
        }
    }
}

async fn test_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    let provider = match oidc_get_provider(&state, &id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                oidc_text(&translator, "providerNotFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load OIDC provider for test");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadProviderFailed"),
            );
        }
    };

    let (success, message) = match run_provider_test(&provider, &translator).await {
        Ok(()) => (true, oidc_text(&translator, "connectionTestSuccess")),
        Err(message) => (false, message),
    };
    let mut updated = provider;
    if let Some(object) = updated.as_object_mut() {
        let now = time_utils::now_iso();
        object.insert("last_test_at".to_string(), Value::String(now.clone()));
        object.insert(
            "last_test_status".to_string(),
            Value::String(if success { "success" } else { "failed" }.to_string()),
        );
        object.insert(
            "last_error".to_string(),
            if success {
                Value::Null
            } else {
                Value::String(message.clone())
            },
        );
        object.insert("updated_at".to_string(), Value::String(now));
    }
    if let Err(error) = oidc_save_provider(&state, &updated).await {
        tracing::warn!(%error, %id, "failed to save OIDC provider test result");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            oidc_text(&translator, "testProviderFailed"),
        );
    }

    Json(json!({ "success": success, "message": message })).into_response()
}

async fn list_bindings_by_totp(
    State(state): State<AppState>,
    Path(totp_id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match oidc_list_bindings(&state).await {
        Ok(bindings) => {
            let providers = oidc_list_providers(&state).await.unwrap_or_default();
            let totps = state.redis.get_totps().await.unwrap_or_default();
            let views = bindings
                .into_iter()
                .filter(|binding| binding.get("totp_id").and_then(Value::as_str) == Some(&totp_id))
                .map(|mut binding| {
                    if let Some(object) = binding.as_object_mut() {
                        if let Some(provider_name) = providers
                            .iter()
                            .find(|provider| {
                                provider.get("id").and_then(Value::as_str)
                                    == object.get("provider_id").and_then(Value::as_str)
                            })
                            .and_then(|provider| provider.get("name"))
                            .and_then(Value::as_str)
                        {
                            object.insert(
                                "provider_name".to_string(),
                                Value::String(provider_name.to_string()),
                            );
                        }
                        if let Some(totp_name) = totps
                            .iter()
                            .find(|totp| {
                                Some(totp.id.as_str())
                                    == object.get("totp_id").and_then(Value::as_str)
                            })
                            .map(|totp| totp.comment.clone())
                        {
                            object.insert("totp_name".to_string(), Value::String(totp_name));
                        }
                    }
                    binding
                })
                .collect::<Vec<_>>();
            response::ok(json!({ "bindings": views })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, %totp_id, "failed to list OIDC bindings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "listBindingsFailed"),
            )
        }
    }
}

async fn create_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(object) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "invitationPayloadObject"),
        );
    };
    let Some(totp_id) = normalize_string(object.get("totp_id")) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "totpRequired"),
        );
    };
    let Some(provider_id) = normalize_string(object.get("provider_id")) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "selectProvider"),
        );
    };

    let totps = match state.redis.get_totps().await {
        Ok(totps) => totps,
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials for OIDC invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadTotpFailed"),
            );
        }
    };
    if !totps.iter().any(|totp| totp.id == totp_id) {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "totpMissing"),
        );
    }

    let provider = match oidc_get_provider(&state, &provider_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                oidc_text(&translator, "providerNotFound"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %provider_id, "failed to load OIDC provider for invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadProviderFailed"),
            );
        }
    };
    if provider.get("enabled").and_then(Value::as_bool) != Some(true)
        || !missing_required_provider_fields(&provider).is_empty()
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "providerUnavailable"),
        );
    }

    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for OIDC invite URL");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let Some(base_url) = invite_base_url(&headers, &uri, &config) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "inviteUrlBuildFailed"),
        );
    };
    let token = create_public_token();
    let token_hash = sha256_hex(&token);
    let created_at = time_utils::now_iso();
    let expires_at = time_utils::iso_after_seconds(DEFAULT_INVITE_TTL_SECONDS as i64);
    let mut invite = Map::new();
    invite.insert("token_hash".to_string(), Value::String(token_hash.clone()));
    invite.insert("totp_id".to_string(), Value::String(totp_id));
    invite.insert("provider_id".to_string(), Value::String(provider_id));
    invite.insert("created_at".to_string(), Value::String(created_at));
    invite.insert("expires_at".to_string(), Value::String(expires_at.clone()));
    if let Some(note) = normalize_string(object.get("note")) {
        invite.insert("note".to_string(), Value::String(note));
    }
    let invite_value = Value::Object(invite);
    if let Err(error) = state
        .redis
        .set_json_value_ex(
            &invite_key(&token_hash),
            &invite_value,
            DEFAULT_INVITE_TTL_SECONDS,
        )
        .await
    {
        tracing::warn!(%error, "failed to save OIDC invite");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            oidc_text(&translator, "createInviteFailed"),
        );
    }

    response::ok(json!({
        "invite_url": format!(
            "{}/api/auth/oidc/bind?token={}",
            base_url.trim_end_matches('/'),
            url::form_urlencoded::byte_serialize(token.as_bytes()).collect::<String>()
        ),
        "expires_at": expires_at,
    }))
    .into_response()
}

async fn delete_binding(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match oidc_delete_binding(&state, &id).await {
        Ok(true) => response::success_empty().into_response(),
        Ok(false) => response::error(
            StatusCode::BAD_REQUEST,
            oidc_text(&translator, "bindingNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete OIDC binding");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "deleteBindingFailed"),
            )
        }
    }
}

async fn oidc_list_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(PROVIDERS_INDEX_KEY).await?;
    let mut providers = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match oidc_get_provider(state, &id).await? {
            Some(provider) => providers.push(provider),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .redis
            .zrem_string_member(PROVIDERS_INDEX_KEY, &id)
            .await?;
    }
    Ok(providers)
}

pub(crate) async fn oidc_public_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    Ok(oidc_list_providers(state)
        .await?
        .into_iter()
        .filter(|provider| provider.get("enabled").and_then(Value::as_bool) == Some(true))
        .filter(|provider| missing_required_provider_fields(provider).is_empty())
        .map(|provider| {
            json!({
                "id": provider.get("id").cloned().unwrap_or(Value::String(String::new())),
                "type": provider.get("type").cloned().unwrap_or(Value::String(String::new())),
                "name": provider.get("name").cloned().unwrap_or(Value::String(String::new())),
                "protocol": provider.get("protocol").cloned().unwrap_or(Value::String(String::new())),
            })
        })
        .collect())
}

pub(crate) async fn oidc_inspect_invite(
    state: &AppState,
    token: &str,
) -> redis::RedisResult<Option<Value>> {
    let normalized_token = token.trim();
    if normalized_token.is_empty() {
        return Ok(None);
    }
    let token_hash = sha256_hex(normalized_token);
    let Some(invite) = state.redis.get_json_value(&invite_key(&token_hash)).await? else {
        return Ok(None);
    };
    let expires_at = invite
        .get("expires_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    if invite.get("used_at").is_some()
        || time_utils::parse_iso_ms(expires_at).unwrap_or(0) <= time_utils::now_ms()
    {
        return Ok(None);
    }
    let totp_id = invite.get("totp_id").and_then(Value::as_str).unwrap_or("");
    let Some(totp) = state
        .redis
        .get_totps()
        .await?
        .into_iter()
        .find(|credential| credential.id == totp_id)
    else {
        return Ok(None);
    };
    let invite_provider_id = invite.get("provider_id").and_then(Value::as_str);
    let providers = oidc_public_providers(state)
        .await?
        .into_iter()
        .filter(|provider| {
            invite_provider_id
                .map(|id| provider.get("id").and_then(Value::as_str) == Some(id))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    Ok(Some(json!({
        "totp": { "id": totp.id, "comment": totp.comment },
        "provider_id": invite_provider_id,
        "expires_at": expires_at,
        "note": invite.get("note").cloned().unwrap_or(Value::Null),
        "providers": providers,
    })))
}

pub(crate) async fn oidc_get_provider(
    state: &AppState,
    id: &str,
) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&provider_key(id)).await
}

async fn oidc_save_provider(state: &AppState, provider: &Value) -> redis::RedisResult<()> {
    let id = provider.get("id").and_then(Value::as_str).unwrap_or("");
    state
        .redis
        .set_json_value(&provider_key(id), provider)
        .await?;
    state
        .redis
        .zadd_string_member(
            PROVIDERS_INDEX_KEY,
            id,
            provider
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        )
        .await
}

async fn oidc_delete_provider(state: &AppState, id: &str) -> redis::RedisResult<()> {
    let bindings = oidc_list_bindings(state).await?;
    state.redis.delete_keys(&[provider_key(id)]).await?;
    state
        .redis
        .zrem_string_member(PROVIDERS_INDEX_KEY, id)
        .await?;
    for binding in bindings
        .iter()
        .filter(|binding| binding.get("provider_id").and_then(Value::as_str) == Some(id))
    {
        if let Some(binding_id) = binding.get("id").and_then(Value::as_str) {
            let _ = oidc_delete_binding(state, binding_id).await?;
        }
    }
    Ok(())
}

pub(crate) async fn oidc_list_bindings(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(BINDINGS_INDEX_KEY).await?;
    let mut bindings = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match state.redis.get_json_value(&binding_key(&id)).await? {
            Some(binding) => bindings.push(binding),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .redis
            .zrem_string_member(BINDINGS_INDEX_KEY, &id)
            .await?;
    }
    Ok(bindings)
}

pub(crate) async fn oidc_get_binding_by_subject(
    state: &AppState,
    subject_key: &str,
) -> redis::RedisResult<Option<Value>> {
    let Some(binding_id) = state
        .redis
        .get_string_value(&subject_binding_key(subject_key))
        .await?
    else {
        return Ok(None);
    };
    state.redis.get_json_value(&binding_key(&binding_id)).await
}

pub(crate) async fn oidc_save_binding(state: &AppState, binding: &Value) -> redis::RedisResult<()> {
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value(&binding_key(id), binding)
        .await?;
    if !subject_key.is_empty() {
        state
            .redis
            .set_string_value(&subject_binding_key(subject_key), id)
            .await?;
    }
    state
        .redis
        .zadd_string_member(
            BINDINGS_INDEX_KEY,
            id,
            binding
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        )
        .await
}

pub(crate) async fn oidc_save_binding_if_subject_available(
    state: &AppState,
    binding: &Value,
) -> redis::RedisResult<bool> {
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    if subject_key.is_empty() {
        return Ok(false);
    }
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    if let Some(existing_id) = state
        .redis
        .get_string_value(&subject_binding_key(subject_key))
        .await?
        && existing_id != id
    {
        return Ok(false);
    }
    oidc_save_binding(state, binding).await?;
    Ok(true)
}

async fn oidc_delete_binding(state: &AppState, id: &str) -> redis::RedisResult<bool> {
    let Some(binding) = state.redis.get_json_value(&binding_key(id)).await? else {
        return Ok(false);
    };
    let mut keys = vec![binding_key(id)];
    if let Some(subject_key) = binding.get("subject_key").and_then(Value::as_str) {
        keys.push(subject_binding_key(subject_key));
    }
    state.redis.delete_keys(&keys).await?;
    state
        .redis
        .zrem_string_member(BINDINGS_INDEX_KEY, id)
        .await?;
    Ok(true)
}

pub(crate) async fn oidc_delete_bindings_by_totp(
    state: &AppState,
    totp_id: &str,
) -> redis::RedisResult<usize> {
    let bindings = oidc_list_bindings(state).await?;
    let mut deleted = 0usize;
    for binding in bindings {
        if binding.get("totp_id").and_then(Value::as_str) != Some(totp_id) {
            continue;
        }
        let Some(binding_id) = binding.get("id").and_then(Value::as_str) else {
            continue;
        };
        if oidc_delete_binding(state, binding_id).await? {
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub(crate) async fn oidc_consume_invite(
    state: &AppState,
    token_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .consume_json_value(&invite_key(token_hash))
        .await
}

pub(crate) async fn oidc_save_state(
    state: &AppState,
    auth_state: &Value,
    ttl_seconds: usize,
) -> redis::RedisResult<()> {
    let state_hash = auth_state
        .get("state_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value_ex(&state_key(state_hash), auth_state, ttl_seconds)
        .await
}

pub(crate) async fn oidc_consume_state(
    state: &AppState,
    state_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state.redis.consume_json_value(&state_key(state_hash)).await
}

pub(crate) async fn oidc_save_login_error_notice(
    state: &AppState,
    notice: &Value,
    ttl_seconds: usize,
) -> redis::RedisResult<()> {
    let token_hash = notice
        .get("token_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value_ex(&login_error_key(token_hash), notice, ttl_seconds)
        .await
}

pub(crate) async fn oidc_consume_login_error_notice(
    state: &AppState,
    token_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .consume_json_value(&login_error_key(token_hash))
        .await
}

pub(crate) fn oidc_provider_ready_with_translator(
    provider: &Value,
    translator: &Translator,
) -> Result<(), String> {
    let missing = missing_required_provider_fields(provider);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(oidc_text_params(
            translator,
            "providerMissingRequiredFields",
            &[("fields", missing.join(", "))],
        ))
    }
}

async fn run_provider_test(provider: &Value, translator: &Translator) -> Result<(), String> {
    let missing = missing_required_provider_fields(provider);
    if !missing.is_empty() {
        return Err(oidc_text_params(
            translator,
            "providerMissingRequiredFields",
            &[("fields", missing.join(", "))],
        ));
    }
    if provider.get("protocol").and_then(Value::as_str) == Some("oidc") {
        resolve_discovery_with_translator(provider, translator).await?;
        return Ok(());
    }
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))?;
    for key in ["authorization_endpoint", "token_endpoint"] {
        if normalize_string(config.get(key)).is_none() {
            return Err(oidc_text_params(
                translator,
                "oauthEndpointIncompleteWithField",
                &[("field", key.to_string())],
            ));
        }
    }
    Ok(())
}

pub(crate) async fn resolve_discovery_with_translator(
    provider: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))?;
    let direct = [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
    ]
    .into_iter()
    .all(|key| normalize_string(config.get(key)).is_some());
    if direct {
        return Ok(json!({
            "issuer": normalize_string(config.get("issuer")).unwrap_or_default(),
            "authorization_endpoint": normalize_string(config.get("authorization_endpoint")).unwrap_or_default(),
            "token_endpoint": normalize_string(config.get("token_endpoint")).unwrap_or_default(),
            "userinfo_endpoint": normalize_string(config.get("userinfo_endpoint")),
            "jwks_uri": normalize_string(config.get("jwks_uri")).unwrap_or_default(),
        }));
    }

    let issuer = normalize_string(config.get("issuer"))
        .ok_or_else(|| oidc_text(translator, "issuerMissing"))?;
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(7))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(&discovery_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, OIDC_HTTP_USER_AGENT)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(oidc_text_params(
            translator,
            "discoveryHttpFailed",
            &[
                ("status", status.as_u16().to_string()),
                ("detail", text.chars().take(160).collect::<String>()),
            ],
        ));
    }
    let payload = serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?;
    let Some(object) = payload.as_object() else {
        return Err(oidc_text(translator, "discoveryInvalid"));
    };
    let missing = [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
    ]
    .into_iter()
    .filter(|key| normalize_string(object.get(*key)).is_none())
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(oidc_text_params(
            translator,
            "discoveryMissingFieldsWithList",
            &[("fields", missing.join(", "))],
        ));
    }
    Ok(payload)
}

fn build_new_provider(
    input: &Map<String, Value>,
    translator: &Translator,
) -> Result<Value, String> {
    let provider_type = normalize_string(input.get("type"))
        .ok_or_else(|| oidc_text(translator, "providerTypeRequired"))?;
    let definition = provider_definition(&provider_type)
        .ok_or_else(|| oidc_text(translator, "providerUnsupported"))?;
    let now = time_utils::now_iso();
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let connection_config = normalize_connection_config(
        &provider_type,
        input
            .get("connection_config")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        !enabled,
        translator,
    )?;
    Ok(json!({
        "id": create_oidc_id("oidc_provider"),
        "type": provider_type,
        "protocol": definition.protocol,
        "name": normalize_string(input.get("name")).unwrap_or_else(|| provider_default_name(&definition, translator)),
        "enabled": enabled,
        "connection_config": connection_config,
        "created_at": now,
        "updated_at": now,
        "last_test_status": "idle",
    }))
}

fn missing_required_provider_fields(provider: &Value) -> Vec<&'static str> {
    let Some(provider_type) = provider.get("type").and_then(Value::as_str) else {
        return vec!["type"];
    };
    let Some(definition) = provider_definition(provider_type) else {
        return vec!["type"];
    };
    let config = provider.get("connection_config").and_then(Value::as_object);
    definition
        .required_fields
        .iter()
        .filter(|field| !connection_value_present(config.and_then(|config| config.get(**field))))
        .copied()
        .collect()
}

fn build_updated_provider(
    mut provider: Value,
    input: &Map<String, Value>,
    translator: &Translator,
) -> Result<Value, String> {
    let Some(object) = provider.as_object_mut() else {
        return Err(oidc_text(translator, "storedProviderInvalid"));
    };
    let provider_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "storedProviderTypeInvalid"))?
        .to_string();
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            object
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        });
    let mut connection = object
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(patch) = input.get("connection_config").and_then(Value::as_object) {
        for (key, value) in patch {
            connection.insert(key.clone(), value.clone());
        }
    }
    let normalized_connection =
        normalize_connection_config(&provider_type, connection, !enabled, translator)?;
    if let Some(name) = normalize_string(input.get("name")) {
        object.insert("name".to_string(), Value::String(name));
    }
    object.insert("enabled".to_string(), Value::Bool(enabled));
    object.insert("connection_config".to_string(), normalized_connection);
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    Ok(provider)
}

fn normalize_connection_config(
    provider_type: &str,
    raw: Map<String, Value>,
    allow_incomplete: bool,
    translator: &Translator,
) -> Result<Value, String> {
    let definition = provider_definition(provider_type)
        .ok_or_else(|| oidc_text(translator, "providerUnsupported"))?;
    let defaults = default_connection_config(provider_type);
    let tenant = normalize_string(raw.get("tenant")).or_else(|| {
        defaults
            .get("tenant")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    let issuer = normalize_string(raw.get("issuer")).or_else(|| {
        if provider_type == "microsoft" {
            tenant
                .as_ref()
                .map(|tenant| format!("https://login.microsoftonline.com/{tenant}/v2.0"))
        } else {
            defaults
                .get("issuer")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }
    });
    let mut config = Map::new();
    insert_string(
        &mut config,
        "client_id",
        normalize_string(raw.get("client_id")).unwrap_or_default(),
    );
    insert_string(
        &mut config,
        "client_secret",
        normalize_string(raw.get("client_secret")).unwrap_or_default(),
    );
    insert_optional_string(&mut config, "issuer", issuer);
    insert_optional_string(&mut config, "tenant", tenant);
    for key in [
        "authorization_endpoint",
        "token_endpoint",
        "userinfo_endpoint",
        "jwks_uri",
        "emails_endpoint",
    ] {
        insert_optional_string(
            &mut config,
            key,
            normalize_string(raw.get(key)).or_else(|| {
                defaults
                    .get(key)
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            }),
        );
    }
    config.insert(
        "scopes".to_string(),
        Value::Array(
            normalize_scopes(raw.get("scopes"), definition.default_scopes)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    if let Some(extra) = normalize_extra_auth_params(raw.get("extra_auth_params"), translator)? {
        config.insert("extra_auth_params".to_string(), extra);
    }

    let missing = definition
        .required_fields
        .iter()
        .filter(|field| !connection_value_present(config.get(**field)))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() && !allow_incomplete {
        return Err(oidc_text_params(
            translator,
            "providerMissingRequiredConfig",
            &[
                ("provider", provider_label(&definition, translator)),
                ("fields", missing.join(", ")),
            ],
        ));
    }
    for key in [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "userinfo_endpoint",
        "jwks_uri",
        "emails_endpoint",
    ] {
        if let Some(value) = config
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            assert_http_url(value, key, translator)?;
        }
    }
    Ok(Value::Object(config))
}

fn mask_provider(provider: Value, callback_origin: Option<&str>) -> Value {
    let Some(object) = provider.as_object() else {
        return provider;
    };
    let mut masked_config = Map::new();
    if let Some(config) = object.get("connection_config").and_then(Value::as_object) {
        for (key, value) in config {
            masked_config.insert(
                key.clone(),
                if key == "client_secret" {
                    Value::String(mask_sensitive_value(value))
                } else {
                    value.clone()
                },
            );
        }
    }
    let mut view = Map::new();
    for key in [
        "id",
        "type",
        "protocol",
        "name",
        "enabled",
        "created_at",
        "updated_at",
        "last_test_at",
        "last_test_status",
        "last_error",
    ] {
        if let Some(value) = object.get(key) {
            view.insert(key.to_string(), value.clone());
        }
    }
    view.insert(
        "connection_config_masked".to_string(),
        Value::Object(masked_config),
    );
    if let (Some(origin), Some(id)) = (callback_origin, object.get("id").and_then(Value::as_str)) {
        view.insert(
            "callback_url".to_string(),
            Value::String(format!(
                "{}/api/auth/oidc/callback/{}",
                origin.trim_end_matches('/'),
                url::form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>()
            )),
        );
    }
    Value::Object(view)
}

fn provider_catalog(translator: &Translator) -> Vec<Value> {
    ["google", "microsoft", "github", "custom_oidc"]
        .into_iter()
        .filter_map(provider_definition)
        .map(|definition| {
            json!({
                "type": definition.provider_type,
                "protocol": definition.protocol,
                "label": provider_label(&definition, translator),
                "description": provider_description(&definition, translator),
                "default_name": provider_default_name(&definition, translator),
                "default_scopes": definition.default_scopes,
                "required_fields": definition.required_fields,
                "optional_fields": definition.optional_fields,
                "supports_pkce": definition.supports_pkce,
                "supports_discovery": definition.supports_discovery,
            })
        })
        .collect()
}

fn provider_label(definition: &ProviderDefinition, translator: &Translator) -> String {
    if definition.provider_type == "custom_oidc" {
        oidc_text(translator, "catalog.customLabel")
    } else {
        definition.label.to_string()
    }
}

fn provider_description(definition: &ProviderDefinition, translator: &Translator) -> String {
    let key = match definition.provider_type {
        "google" => "catalog.googleDescription",
        "microsoft" => "catalog.microsoftDescription",
        "github" => "catalog.githubDescription",
        "custom_oidc" => "catalog.customDescription",
        _ => return definition.description.to_string(),
    };
    oidc_text(translator, key)
}

fn provider_default_name(definition: &ProviderDefinition, translator: &Translator) -> String {
    if definition.provider_type == "custom_oidc" {
        oidc_text(translator, "catalog.customLabel")
    } else {
        definition.default_name.to_string()
    }
}

#[derive(Clone, Copy)]
struct ProviderDefinition {
    provider_type: &'static str,
    protocol: &'static str,
    label: &'static str,
    description: &'static str,
    default_name: &'static str,
    default_scopes: &'static [&'static str],
    required_fields: &'static [&'static str],
    optional_fields: &'static [&'static str],
    supports_pkce: bool,
    supports_discovery: bool,
}

fn provider_definition(provider_type: &str) -> Option<ProviderDefinition> {
    match provider_type {
        "google" => Some(ProviderDefinition {
            provider_type: "google",
            protocol: "oidc",
            label: "Google",
            description: "Sign in with Google",
            default_name: "Google",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["issuer", "scopes", "extra_auth_params"],
            supports_pkce: true,
            supports_discovery: true,
        }),
        "microsoft" => Some(ProviderDefinition {
            provider_type: "microsoft",
            protocol: "oidc",
            label: "Microsoft",
            description: "Sign in with Microsoft",
            default_name: "Microsoft",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["tenant", "issuer", "scopes", "extra_auth_params"],
            supports_pkce: true,
            supports_discovery: true,
        }),
        "github" => Some(ProviderDefinition {
            provider_type: "github",
            protocol: "oauth2_profile",
            label: "GitHub",
            description: "Sign in with GitHub",
            default_name: "GitHub",
            default_scopes: &["read:user", "user:email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["scopes", "extra_auth_params"],
            supports_pkce: false,
            supports_discovery: false,
        }),
        "custom_oidc" => Some(ProviderDefinition {
            provider_type: "custom_oidc",
            protocol: "oidc",
            label: "Custom OIDC",
            description: "Sign in with a custom OpenID Connect provider",
            default_name: "Custom OIDC",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret", "issuer"],
            optional_fields: &[
                "authorization_endpoint",
                "token_endpoint",
                "userinfo_endpoint",
                "jwks_uri",
                "scopes",
                "extra_auth_params",
            ],
            supports_pkce: true,
            supports_discovery: true,
        }),
        _ => None,
    }
}

fn default_connection_config(provider_type: &str) -> Map<String, Value> {
    match provider_type {
        "google" => map_from_pairs(&[("issuer", "https://accounts.google.com")]),
        "microsoft" => map_from_pairs(&[
            ("tenant", "common"),
            ("issuer", "https://login.microsoftonline.com/common/v2.0"),
        ]),
        "github" => map_from_pairs(&[
            (
                "authorization_endpoint",
                "https://github.com/login/oauth/authorize",
            ),
            (
                "token_endpoint",
                "https://github.com/login/oauth/access_token",
            ),
            ("userinfo_endpoint", "https://api.github.com/user"),
            ("emails_endpoint", "https://api.github.com/user/emails"),
        ]),
        _ => Map::new(),
    }
}

fn map_from_pairs(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
        .collect()
}

fn normalize_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_scopes(value: Option<&Value>, fallback: &[&str]) -> Vec<String> {
    let values = if let Some(items) = value.and_then(Value::as_array) {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    } else if let Some(raw) = value.and_then(Value::as_str) {
        raw.split(|ch: char| ch == ',' || ch.is_whitespace())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut seen = std::collections::HashSet::new();
    let deduped = values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>();
    if deduped.is_empty() {
        fallback.iter().map(|value| (*value).to_string()).collect()
    } else {
        deduped
    }
}

fn normalize_extra_auth_params(
    value: Option<&Value>,
    translator: &Translator,
) -> Result<Option<Value>, String> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Ok(None);
    };
    let mut normalized = Map::new();
    for (key, value) in object {
        let key = key.trim();
        let Some(value) = normalize_string(Some(value)) else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        if reserved_extra_auth_param_key(key) {
            return Err(oidc_text_params(
                translator,
                "reservedExtraAuthParam",
                &[("key", key.to_string())],
            ));
        }
        normalized.insert(key.to_string(), Value::String(value));
    }
    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(normalized)))
    }
}

fn reserved_extra_auth_param_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "client_id"
            | "client_secret"
            | "response_type"
            | "redirect_uri"
            | "scope"
            | "state"
            | "nonce"
            | "code_challenge"
            | "code_challenge_method"
            | "code_verifier"
            | "grant_type"
            | "code"
    )
}

fn connection_value_present(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

fn assert_http_url(value: &str, label: &str, translator: &Translator) -> Result<(), String> {
    let parsed = Url::parse(value)
        .map_err(|_| oidc_text_params(translator, "urlInvalid", &[("label", label.to_string())]))?;
    if parsed.scheme() != "https" && parsed.host_str() != Some("localhost") {
        return Err(oidc_text_params(
            translator,
            "urlMustUseHttps",
            &[("label", label.to_string())],
        ));
    }
    Ok(())
}

fn insert_string(object: &mut Map<String, Value>, key: &str, value: String) {
    object.insert(key.to_string(), Value::String(value));
}

fn insert_optional_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn mask_sensitive_value(value: &Value) -> String {
    let Some(value) = value.as_str() else {
        return "[configured]".to_string();
    };
    if value.is_empty() {
        String::new()
    } else if value.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}******", &value[..2])
    }
}

fn callback_base_url(headers: &HeaderMap, uri: &Uri, config: &Value) -> Option<String> {
    public_auth_base_url(config).or_else(|| callback_origin(headers, uri))
}

fn callback_origin(headers: &HeaderMap, uri: &Uri) -> Option<String> {
    let trust_forwarded = env_bool("OIDC_TRUST_FORWARDED_HEADERS", false)
        || env_bool("AUTH_TRUST_FORWARDED_HEADERS", false);
    let request_proto = uri.scheme_str().unwrap_or("http");
    let proto = if trust_forwarded {
        first_header(headers, "x-forwarded-proto")
    } else {
        None
    }
    .unwrap_or_else(|| request_proto.to_string());
    let proto = proto.trim().trim_end_matches(':').to_ascii_lowercase();
    if proto != "http" && proto != "https" {
        return None;
    }

    let host = if trust_forwarded {
        first_header(headers, "x-forwarded-host")
    } else {
        None
    }
    .or_else(|| first_header(headers, "host"))
    .or_else(|| {
        uri.authority()
            .map(|authority| authority.as_str().to_string())
    })?;
    if host
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, ',' | '/' | '?' | '#' | '\\' | '@'))
    {
        return None;
    }
    Some(format!("{proto}://{host}"))
}

fn invite_base_url(headers: &HeaderMap, uri: &Uri, config: &Value) -> Option<String> {
    callback_base_url(headers, uri, config)
}

fn public_auth_base_url(config: &Value) -> Option<String> {
    crate::auth::resolve_public_auth_base_url(config)
}

fn env_bool(name: &str, fallback: bool) -> bool {
    match std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("1" | "true" | "yes" | "on") => true,
        Some("0" | "false" | "no" | "off") => false,
        _ => fallback,
    }
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

fn create_oidc_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

fn create_public_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn provider_key(id: &str) -> String {
    format!("{PROVIDERS_DATA_KEY_PREFIX}{id}")
}

fn binding_key(id: &str) -> String {
    format!("{BINDINGS_DATA_KEY_PREFIX}{id}")
}

fn subject_binding_key(subject_key: &str) -> String {
    format!("{BINDINGS_SUBJECT_KEY_PREFIX}{subject_key}")
}

fn invite_key(token_hash: &str) -> String {
    format!("{INVITE_KEY_PREFIX}{token_hash}")
}

fn state_key(state_hash: &str) -> String {
    format!("{STATE_KEY_PREFIX}{state_hash}")
}

fn login_error_key(token_hash: &str) -> String {
    format!("{LOGIN_ERROR_KEY_PREFIX}{token_hash}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_google_provider_config_with_defaults() {
        let translator = Translator::new("zh-CN");
        let config = normalize_connection_config(
            "google",
            map_from_values(&[
                ("client_id", json!("client")),
                ("client_secret", json!("secret")),
            ]),
            false,
            &translator,
        )
        .unwrap();
        assert_eq!(config["issuer"], "https://accounts.google.com");
        assert_eq!(config["scopes"], json!(["openid", "profile", "email"]));
    }

    #[test]
    fn normalizes_oidc_scopes_like_node_array_vs_string_inputs() {
        assert_eq!(
            normalize_scopes(Some(&json!("openid profile,email")), &["fallback"]),
            vec!["openid", "profile", "email"]
        );
        assert_eq!(
            normalize_scopes(
                Some(&json!(["openid profile", "email", "email"])),
                &["fallback"]
            ),
            vec!["openid profile", "email"]
        );
    }

    #[test]
    fn rejects_reserved_extra_auth_param() {
        let translator = Translator::new("zh-CN");
        let error = normalize_connection_config(
            "google",
            map_from_values(&[
                ("client_id", json!("client")),
                ("client_secret", json!("secret")),
                ("extra_auth_params", json!({ "state": "bad" })),
            ]),
            false,
            &translator,
        )
        .unwrap_err();
        assert_eq!(error, "extra_auth_params 包含 OIDC 保留参数: state");
    }

    #[test]
    fn masks_provider_secret() {
        let provider = json!({
            "id": "oidc_provider_test",
            "type": "github",
            "protocol": "oauth2_profile",
            "name": "GitHub",
            "enabled": true,
            "created_at": "2026-07-05T00:00:00Z",
            "updated_at": "2026-07-05T00:00:00Z",
            "connection_config": {
                "client_id": "id",
                "client_secret": "verysecret"
            }
        });
        let view = mask_provider(provider, Some("https://auth.example.com"));
        assert_eq!(
            view.pointer("/connection_config_masked/client_secret"),
            Some(&Value::String("ve******".to_string()))
        );
        assert_eq!(
            view.get("callback_url").and_then(Value::as_str),
            Some("https://auth.example.com/api/auth/oidc/callback/oidc_provider_test")
        );
        assert!(view.get("connection_config").is_none());
    }

    #[test]
    fn detects_missing_required_provider_fields() {
        let provider = json!({
            "id": "oidc_provider_test",
            "type": "custom_oidc",
            "protocol": "oidc",
            "connection_config": {
                "client_id": "client",
                "client_secret": ""
            }
        });
        assert_eq!(
            missing_required_provider_fields(&provider),
            vec!["client_secret", "issuer"]
        );
    }

    #[test]
    fn localizes_oidc_catalog_and_validation_text() {
        let translator = Translator::new("zh-CN");
        let catalog = provider_catalog(&translator);
        let custom = catalog
            .iter()
            .find(|provider| provider.get("type").and_then(Value::as_str) == Some("custom_oidc"))
            .unwrap();
        assert_eq!(
            custom.get("label").and_then(Value::as_str),
            Some("自定义 OIDC")
        );
        assert_eq!(
            oidc_text_params(
                &translator,
                "providerMissingRequiredFields",
                &[("fields", "client_secret".to_string())]
            ),
            "外部登录提供商缺少必填配置 client_secret"
        );
    }

    #[test]
    fn builds_invite_base_url_from_public_auth_config_or_auth_host() {
        assert_eq!(
            public_auth_base_url(&json!({
                "subdomain_mode": {
                    "public_auth_base_url": "https://auth.example.com/auth/",
                    "public_https_port": 8443
                }
            })),
            Some("https://auth.example.com:8443/auth".to_string())
        );
        assert_eq!(
            public_auth_base_url(&json!({
                "host_mappings": [{
                    "host": "Auth.Example.Com",
                    "target": "http://127.0.0.1:7997"
                }]
            })),
            Some("https://auth.example.com:7999".to_string())
        );
    }

    #[test]
    fn builds_callback_base_url_from_public_auth_config_before_request_host() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "admin.example.com:7999".parse().unwrap());
        let uri = Uri::from_static("/api/admin/auth/oidc/providers");
        assert_eq!(
            callback_base_url(
                &headers,
                &uri,
                &json!({
                    "subdomain_mode": {
                        "public_auth_base_url": "https://auth.example.com/auth/"
                    }
                })
            ),
            Some("https://auth.example.com:7999/auth".to_string())
        );
    }

    #[test]
    fn callback_origin_uses_uri_or_host_like_node_fallback() {
        assert_eq!(
            callback_origin(
                &HeaderMap::new(),
                &Uri::from_static("https://auth.example.com/api/admin/auth/oidc/providers")
            ),
            Some("https://auth.example.com".to_string())
        );

        let mut headers = HeaderMap::new();
        headers.insert("host", "auth.example.com:7999".parse().unwrap());
        assert_eq!(
            callback_origin(
                &headers,
                &Uri::from_static("/api/admin/auth/oidc/providers")
            ),
            Some("http://auth.example.com:7999".to_string())
        );
    }

    fn map_from_values(values: &[(&str, Value)]) -> Map<String, Value> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }
}
