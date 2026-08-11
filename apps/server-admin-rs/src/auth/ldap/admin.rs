use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    auth::mode::AuthLoginMode, crypto_utils, i18n::Translator, oidc_admin::callback_base_url,
    response, state::AppState, time_utils,
};

use super::{
    DEFAULT_INVITE_TTL_SECONDS,
    client::{authenticate, test_connection},
    provider::{
        build_new_provider, build_updated_provider, catalog, mask_provider, provider_ready,
    },
    storage::{
        delete_binding, delete_provider, get_provider, list_bindings, list_providers, save_invite,
        save_provider,
    },
};

#[derive(Deserialize, Default, utoipa::ToSchema)]
struct TestBody {
    username: Option<String>,
    password: Option<String>,
}

fn text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.ldap.{key}"))
}

pub(crate) fn ldap_admin_routes() -> Router<AppState> {
    ldap_admin_openapi_routes().into()
}

pub(crate) fn ldap_admin_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(provider_catalog))
        .routes(routes!(providers))
        .routes(routes!(create_provider))
        .routes(routes!(update_provider))
        .routes(routes!(remove_provider))
        .routes(routes!(test_provider))
        .routes(routes!(bindings_by_totp))
        .routes(routes!(remove_binding))
        .routes(routes!(create_invitation))
}

#[utoipa::path(get, path = "/api/admin/auth/ldap/catalog", tag = "auth-ldap", operation_id = "get_api_admin_auth_ldap_catalog", responses((status = 200, description = "LDAP provider catalog")))]
async fn provider_catalog(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(json!({ "providers": catalog(&translator) })).into_response()
}

#[utoipa::path(get, path = "/api/admin/auth/ldap/providers", tag = "auth-ldap", operation_id = "get_api_admin_auth_ldap_providers", responses((status = 200, description = "LDAP providers")))]
async fn providers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match list_providers(&state).await {
        Ok(items) => response::ok(json!({
            "providers": items.into_iter().map(mask_provider).collect::<Vec<_>>()
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list LDAP providers");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "listProvidersFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/auth/ldap/providers", tag = "auth-ldap", operation_id = "post_api_admin_auth_ldap_providers", request_body = serde_json::Value, responses((status = 200, description = "Created LDAP provider")))]
async fn create_provider(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(input) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            text(&translator, "providerPayloadObject"),
        );
    };
    match build_new_provider(input) {
        Ok(provider) => match save_provider(&state, &provider).await {
            Ok(()) => response::ok(mask_provider(provider)).into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to save LDAP provider");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    text(&translator, "createProviderFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

#[utoipa::path(patch, path = "/api/admin/auth/ldap/providers/{id}", tag = "auth-ldap", operation_id = "patch_api_admin_auth_ldap_providers_by_id", request_body = serde_json::Value, params(("id" = String, Path, description = "LDAP provider identifier")), responses((status = 200, description = "Updated LDAP provider")))]
async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(input) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            text(&translator, "providerPayloadObject"),
        );
    };
    let existing = match get_provider(&state, &id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "providerNotFound"));
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load LDAP provider");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "loadProviderFailed"),
            );
        }
    };
    match build_updated_provider(existing, input) {
        Ok(provider) => match save_provider(&state, &provider).await {
            Ok(()) => response::ok(mask_provider(provider)).into_response(),
            Err(error) => {
                tracing::warn!(%error, %id, "failed to update LDAP provider");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    text(&translator, "updateProviderFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

#[utoipa::path(delete, path = "/api/admin/auth/ldap/providers/{id}", tag = "auth-ldap", operation_id = "delete_api_admin_auth_ldap_providers_by_id", params(("id" = String, Path, description = "LDAP provider identifier")), responses((status = 200, description = "Deleted LDAP provider")))]
async fn remove_provider(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_provider(&state, &id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "providerNotFound"));
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load LDAP provider before delete");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "loadProviderFailed"),
            );
        }
    }
    match delete_provider(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete LDAP provider");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "deleteProviderFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/auth/ldap/providers/{id}/test", tag = "auth-ldap", operation_id = "post_api_admin_auth_ldap_providers_by_id_test", request_body = Option<TestBody>, params(("id" = String, Path, description = "LDAP provider identifier")), responses((status = 200, description = "LDAP provider connection test result")))]
async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<TestBody>>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut provider = match get_provider(&state, &id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, text(&translator, "providerNotFound"));
        }
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load LDAP provider for test");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "loadProviderFailed"),
            );
        }
    };
    let credentials = body.map(|Json(body)| body).unwrap_or_default();
    let has_username = credentials
        .username
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_password = credentials
        .password
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    if has_username != has_password {
        return response::error(
            StatusCode::BAD_REQUEST,
            text(&translator, "testCredentialsRequired"),
        );
    }
    let result = if has_username {
        authenticate(
            &provider,
            credentials.username.as_deref().unwrap_or_default(),
            credentials.password.as_deref().unwrap_or_default(),
        )
        .await
        .map(|profile| profile.display_name.unwrap_or(profile.username))
    } else {
        test_connection(&provider).await
    };
    let (success, message) = match result {
        Ok(detail) => (
            true,
            format!("{}: {detail}", text(&translator, "connectionTestSuccess")),
        ),
        Err(error) => (false, error.to_string()),
    };
    if let Some(object) = provider.as_object_mut() {
        object.insert("last_test_at".into(), Value::String(time_utils::now_iso()));
        object.insert(
            "last_test_status".into(),
            Value::String(if success { "success" } else { "failed" }.into()),
        );
        object.insert(
            "last_error".into(),
            if success {
                Value::Null
            } else {
                Value::String(message.clone())
            },
        );
        object.insert("updated_at".into(), Value::String(time_utils::now_iso()));
    }
    if let Err(error) = save_provider(&state, &provider).await {
        tracing::warn!(%error, %id, "failed to persist LDAP test result");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            text(&translator, "testProviderFailed"),
        );
    }
    Json(json!({ "success": success, "message": message })).into_response()
}

#[utoipa::path(get, path = "/api/admin/auth/ldap/totp/{totp_id}/bindings", tag = "auth-ldap", operation_id = "get_api_admin_auth_ldap_totp_by_totp_id_bindings", params(("totp_id" = String, Path, description = "TOTP credential identifier")), responses((status = 200, description = "LDAP bindings for a TOTP credential")))]
async fn bindings_by_totp(State(state): State<AppState>, Path(totp_id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match list_bindings(&state).await {
        Ok(bindings) => {
            let providers = match list_providers(&state).await {
                Ok(providers) => providers,
                Err(error) => {
                    tracing::warn!(%error, %totp_id, "failed to list LDAP providers for bindings");
                    return response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        text(&translator, "listBindingsFailed"),
                    );
                }
            };
            let views = bindings
                .into_iter()
                .filter(|binding| binding.get("totp_id").and_then(Value::as_str) == Some(&totp_id))
                .map(|mut binding| {
                    let provider_name = binding
                        .get("provider_id")
                        .and_then(Value::as_str)
                        .and_then(|id| {
                            providers.iter().find(|provider| {
                                provider.get("id").and_then(Value::as_str) == Some(id)
                            })
                        })
                        .and_then(|provider| provider.get("name"))
                        .cloned();
                    if let (Some(object), Some(name)) = (binding.as_object_mut(), provider_name) {
                        object.insert("provider_name".into(), name);
                    }
                    binding
                })
                .collect::<Vec<_>>();
            response::ok(json!({ "bindings": views })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, %totp_id, "failed to list LDAP bindings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "listBindingsFailed"),
            )
        }
    }
}

#[utoipa::path(delete, path = "/api/admin/auth/ldap/bindings/{id}", tag = "auth-ldap", operation_id = "delete_api_admin_auth_ldap_bindings_by_id", params(("id" = String, Path, description = "LDAP binding identifier")), responses((status = 200, description = "Deleted LDAP binding")))]
async fn remove_binding(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match delete_binding(&state, &id).await {
        Ok(true) => response::success_empty().into_response(),
        Ok(false) => response::error(StatusCode::NOT_FOUND, text(&translator, "bindingNotFound")),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete LDAP binding");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "deleteBindingFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/auth/ldap/invitations", tag = "auth-ldap", operation_id = "post_api_admin_auth_ldap_invitations", request_body = serde_json::Value, responses((status = 200, description = "Created LDAP binding invitation")))]
async fn create_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_auth_login_mode().await {
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
                text(&translator, "createInviteFailed"),
            );
        }
    }
    let totp_id = body
        .get("totp_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let provider_id = body
        .get("provider_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if totp_id.is_empty() || provider_id.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            text(&translator, "invitationFieldsRequired"),
        );
    }
    let totps = match state.storage.store.get_totps().await {
        Ok(totps) => totps,
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials for LDAP invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "loadTotpFailed"),
            );
        }
    };
    if !totps.iter().any(|totp| totp.id == totp_id) {
        return response::error(StatusCode::NOT_FOUND, text(&translator, "totpMissing"));
    }
    let provider = match get_provider(&state, provider_id).await {
        Ok(Some(provider))
            if provider.get("id").and_then(Value::as_str) == Some(provider_id)
                && provider.get("enabled").and_then(Value::as_bool) == Some(true)
                && provider_ready(&provider) =>
        {
            provider
        }
        Ok(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                text(&translator, "providerUnavailable"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, %provider_id, "failed to load LDAP provider for invite");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "createInviteFailed"),
            );
        }
    };
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for LDAP invite URL");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                text(&translator, "loadConfigFailed"),
            );
        }
    };
    let Some(base_url) = callback_base_url(&headers, &uri, &config) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            text(&translator, "inviteUrlBuildFailed"),
        );
    };
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = crypto_utils::sha256_hex_str(&token);
    let expires_at = time_utils::iso_after_seconds(DEFAULT_INVITE_TTL_SECONDS as i64);
    let invite = json!({
        "token_hash": token_hash,
        "totp_id": totp_id,
        "provider_id": provider_id,
        "provider_name": provider.get("name").cloned().unwrap_or_default(),
        "created_at": time_utils::now_iso(),
        "expires_at": expires_at,
        "note": body.get("note").cloned().unwrap_or(Value::Null),
    });
    if let Err(error) = save_invite(&state, &token_hash, &invite).await {
        tracing::warn!(%error, "failed to save LDAP invitation");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            text(&translator, "createInviteFailed"),
        );
    }
    response::ok(json!({
        "invite_url": format!("{}/ldap/bind?token={}", base_url.trim_end_matches('/'), crate::http_utils::url_encode_component(&token)),
        "expires_at": expires_at,
    }))
    .into_response()
}
