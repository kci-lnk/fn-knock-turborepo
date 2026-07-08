use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};

use crate::{i18n::Translator, response, state::AppState, time_utils};

use super::{
    DEFAULT_INVITE_TTL_SECONDS,
    discovery::run_provider_test,
    provider::{
        build_new_provider, build_updated_provider, mask_provider,
        missing_required_provider_fields, normalize_string, provider_catalog,
    },
    storage::{
        oidc_delete_binding, oidc_delete_provider, oidc_get_provider, oidc_list_bindings,
        oidc_list_providers, oidc_save_provider,
    },
    text::oidc_text,
    tokens::{create_public_token, invite_key, sha256_hex},
    urls::{callback_base_url, invite_base_url},
};

pub(super) async fn catalog(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(json!({ "providers": provider_catalog(&translator) })).into_response()
}

pub(super) async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
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

pub(super) async fn create_provider(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
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

pub(super) async fn update_provider(
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

pub(super) async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
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

pub(super) async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
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

pub(super) async fn list_bindings_by_totp(
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

pub(super) async fn create_invitation(
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
            crate::http_utils::url_encode_component(&token)
        ),
        "expires_at": expires_at,
    }))
    .into_response()
}

pub(super) async fn delete_binding(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
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
