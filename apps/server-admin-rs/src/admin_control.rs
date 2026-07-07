use std::{
    collections::{BTreeMap, HashSet},
    net::IpAddr,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use totp_rs::Secret;

use crate::{
    app_version::APP_LOCAL_VERSION,
    auth::verify_totp_token,
    auth_mobility, http_utils,
    i18n::Translator,
    ip_location,
    oidc_admin::oidc_delete_bindings_by_totp,
    proxy_config::build_gateway_auth_config,
    redis_store::{
        LoginSession, TotpCredential, WhitelistRecord, normalize_totp_access_scopes,
        normalize_totp_subdomain_access,
    },
    response,
    state::AppState,
    system_events, time_utils, whitelist,
};

const TOTP_TRANSFER_KIND: &str = "fn-knock.totp-credentials";
const TOTP_TRANSFER_VERSION: u64 = 1;
const MAX_TOTP_IMPORT_COUNT: usize = 200;
const AUTH_SESSION_TTL_SECONDS_DEFAULT: i64 = 24 * 3600;
const AUTH_REMEMBER_ME_TTL_SECONDS_DEFAULT: i64 = 365 * 24 * 3600;
const AUTH_POST_LOGIN_IP_GRANT_TTL_SECONDS_DEFAULT: i64 = 3600;
const AUTH_SESSION_IP_MOBILITY_WINDOW_SECONDS_DEFAULT: i64 = 20 * 60;
const AUTH_MAX_TTL_SECONDS: i64 = 5 * 365 * 24 * 3600;

#[derive(Debug)]
struct TotpImportRouteError {
    status: StatusCode,
    key: &'static str,
    max: Option<usize>,
}

fn admin_control_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

fn admin_control_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

fn totp_import_error(status: StatusCode, key: &'static str) -> TotpImportRouteError {
    TotpImportRouteError {
        status,
        key,
        max: None,
    }
}

fn totp_import_error_with_max(
    status: StatusCode,
    key: &'static str,
    max: usize,
) -> TotpImportRouteError {
    TotpImportRouteError {
        status,
        key,
        max: Some(max),
    }
}

fn totp_import_error_message(translator: &Translator, error: &TotpImportRouteError) -> String {
    let key = format!("totpImport.{}", error.key);
    if let Some(max) = error.max {
        admin_control_text_params(translator, &key, &[("max", max.to_string())])
    } else {
        admin_control_text(translator, &key)
    }
}

#[derive(Deserialize)]
struct AuthCredentialSettingsBody {
    #[serde(flatten)]
    value: Map<String, Value>,
}

#[derive(Deserialize)]
struct TotpBindBody {
    secret: String,
    token: String,
    comment: Option<String>,
}

#[derive(Deserialize)]
struct TotpCommentBody {
    comment: String,
}

#[derive(Deserialize)]
struct TotpAccessScopesBody {
    access_scopes: Value,
}

#[derive(Deserialize)]
struct TotpSubdomainAccessBody {
    subdomain_access: Value,
}

#[derive(Deserialize)]
struct TotpImportBody {
    payload: Value,
}

#[derive(Deserialize)]
struct SessionCommentBody {
    comment: String,
}

pub fn admin_control_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/auth_credential_settings",
            get(get_auth_credential_settings).post(update_auth_credential_settings),
        )
        .route("/api/admin/totp/status", get(totp_status))
        .route("/api/admin/totp/setup", post(totp_setup))
        .route("/api/admin/totp/bind", post(totp_bind))
        .route("/api/admin/totp/credentials/export", get(totp_export))
        .route("/api/admin/totp/credentials/import", post(totp_import))
        .route("/api/admin/totp/{id}", delete(totp_delete))
        .route(
            "/api/admin/totp/{id}/access-scopes",
            patch(totp_update_access_scopes),
        )
        .route(
            "/api/admin/totp/{id}/subdomain-access",
            patch(totp_update_subdomain_access),
        )
        .route("/api/admin/totp/{id}/comment", patch(totp_update_comment))
        .route("/api/admin/totp/{totp_id}/passkeys", get(totp_passkeys))
        .route("/api/admin/passkeys/{id}", delete(passkey_delete))
        .route("/api/admin/sessions", get(sessions_list))
        .route(
            "/api/admin/sessions/{id}",
            get(session_get).delete(session_delete),
        )
        .route(
            "/api/admin/sessions/{id}/comment",
            patch(session_update_comment),
        )
        .route(
            "/api/admin/sessions/{id}/mobility",
            get(session_mobility_details),
        )
}

async fn get_auth_credential_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_config().await {
        Ok(config) => response::ok(auth_credential_settings_from_config(&config)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load auth credential settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.loadFailed"),
            )
        }
    }
}

async fn update_auth_credential_settings(
    State(state): State<AppState>,
    Json(body): Json<AuthCredentialSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before auth settings update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.loadConfigFailed"),
            );
        }
    };

    let current = auth_credential_settings_from_config(&config);
    let mut next = current.as_object().cloned().unwrap_or_default();
    for (key, value) in body.value {
        if is_allowed_auth_credential_setting(&key) {
            next.insert(key, value);
        }
    }
    let normalized = normalize_auth_credential_settings(
        Value::Object(next),
        legacy_auto_add_whitelist_on_login(&config),
    );
    let session_ip_mobility_changed = session_ip_mobility_settings_changed(&current, &normalized);
    if session_ip_mobility_changed
        && let Err(error) = auth_mobility::reconcile_session_ip_mobility_policy(
            &state,
            &current,
            &normalized,
            false,
        )
        .await
    {
        tracing::warn!(%error, "failed to reconcile session IP mobility before auth settings update");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_control_text(&translator, "authCredentialSettings.saveFailed"),
        );
    }
    ensure_object(&mut config).insert("auth_credential_settings".to_string(), normalized.clone());

    match state.redis.save_config(&config).await {
        Ok(()) => {
            if session_ip_mobility_changed {
                whitelist::sync_reverse_proxy_trusted_ips(&state).await;
            }
            response::ok(normalized).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save auth credential settings");
            if session_ip_mobility_changed {
                if let Err(rollback_error) = auth_mobility::reconcile_session_ip_mobility_policy(
                    &state,
                    &normalized,
                    &current,
                    false,
                )
                .await
                {
                    tracing::warn!(
                        %rollback_error,
                        "failed to rollback session IP mobility reconciliation after auth settings save failure"
                    );
                } else {
                    whitelist::sync_reverse_proxy_trusted_ips(&state).await;
                }
            }
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "authCredentialSettings.saveFailed"),
            )
        }
    }
}

async fn totp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_totps().await {
        Ok(credentials) => response::ok(json!({
            "bound": !credentials.is_empty(),
            "credentials": credentials
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            )
        }
    }
}

async fn totp_setup() -> Response {
    let secret = match Secret::generate_secret().to_encoded() {
        Secret::Encoded(value) => value,
        other => other.to_string(),
    };
    let label = percent_encode("fn-knock:admin");
    let issuer = percent_encode("fn-knock");
    response::ok(json!({
        "secret": secret,
        "uri": format!("otpauth://totp/{label}?secret={secret}&issuer={issuer}")
    }))
    .into_response()
}

async fn totp_bind(State(state): State<AppState>, Json(body): Json<TotpBindBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    match verify_totp_token(&body.secret, &body.token) {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, "totp.invalidCode"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to verify TOTP bind token");
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_control_text(&translator, "totp.invalidSecretOrCode"),
            );
        }
    }

    let credential = TotpCredential {
        id: hex::encode(rand::random::<[u8; 8]>()),
        secret: body.secret,
        comment: node_totp_bind_comment(body.comment),
        created_at: time_utils::now_iso(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    };

    match state.redis.add_totp(credential).await {
        Ok(()) => {
            response::success_message(admin_control_text(&translator, "totp.bound")).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save TOTP credential");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.saveFailed"),
            )
        }
    }
}

async fn totp_export(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_totps().await {
        Ok(credentials) => {
            let exported_at = time_utils::now_iso();
            let payload = build_totp_export_payload(&credentials, &exported_at);
            let filename = format!(
                "fn-knock-totp-credentials-{}.json",
                exported_at.replace([':', '.'], "-")
            );
            let body = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
            (
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/json; charset=utf-8"),
                    ),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
                    (
                        header::CONTENT_DISPOSITION,
                        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
                    ),
                ],
                body,
            )
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to export TOTP credentials");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.exportFailed"),
            )
        }
    }
}

async fn totp_import(State(state): State<AppState>, Json(body): Json<TotpImportBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    let existing = match state.redis.get_totps().await {
        Ok(credentials) => credentials,
        Err(error) => {
            tracing::warn!(%error, "failed to load existing TOTP credentials for import");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.loadFailed"),
            );
        }
    };

    let (credentials, summary) = match build_totp_import_plan(&existing, &body.payload) {
        Ok(plan) => plan,
        Err(error) => {
            return response::error(error.status, totp_import_error_message(&translator, &error));
        }
    };

    if !credentials.is_empty() {
        let mut next = existing;
        next.extend(credentials);
        if let Err(error) = state.redis.set_totps(&next).await {
            tracing::warn!(%error, "failed to import TOTP credentials");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.importFailed"),
            );
        }
    }

    response::ok(summary).into_response()
}

async fn totp_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.delete_totp(&id).await {
        Ok(true) => {
            if let Err(error) =
                auth_mobility::destroy_sessions_for_totp_credential(&state, &id).await
            {
                tracing::warn!(%error, %id, "failed to destroy sessions for deleted TOTP credential");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.deleteFailed"),
                );
            }
            if let Err(error) = oidc_delete_bindings_by_totp(&state, &id).await {
                tracing::warn!(%error, %id, "failed to delete OIDC bindings for deleted TOTP credential");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.deleteFailed"),
                );
            }
            if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                tracing::warn!(%error, %id, "failed to refresh auth gateway runtime after TOTP delete");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "hostMappings.syncAuthConfigFailed"),
                );
            }
            response::success_message(admin_control_text(&translator, "totp.deleted"))
                .into_response()
        }
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete TOTP credential");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.deleteFailed"),
            )
        }
    }
}

async fn totp_update_access_scopes(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpAccessScopesBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
        .update_totp_access_scopes(&id, body.access_scopes)
        .await
    {
        Ok(Some(updated)) => response::ok(updated).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP access scopes");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

async fn totp_update_subdomain_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpSubdomainAccessBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
        .update_totp_subdomain_access(&id, body.subdomain_access)
        .await
    {
        Ok(Some(updated)) => {
            if updated.subdomain_access.get("mode").and_then(Value::as_str) == Some("custom")
                && let Err(error) =
                    auth_mobility::clear_auto_ip_grants_for_totp_credential(&state, &id).await
            {
                tracing::warn!(%error, %id, "failed to clear auto IP grants after TOTP subdomain access restriction");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "totp.updateFailed"),
                );
            }
            if let Err(error) = refresh_gateway_auth_runtime(&state).await {
                tracing::warn!(%error, %id, "failed to refresh auth gateway runtime after TOTP subdomain access update");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_control_text(&translator, "hostMappings.syncAuthConfigFailed"),
                );
            }
            response::ok(updated).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP subdomain access");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

async fn totp_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TotpCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.update_totp_comment(&id, body.comment).await {
        Ok(Some(_)) => response::success_message(admin_control_text(&translator, "totp.updated"))
            .into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "totp.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update TOTP comment");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "totp.updateFailed"),
            )
        }
    }
}

async fn totp_passkeys(State(state): State<AppState>, Path(totp_id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_passkeys().await {
        Ok(passkeys) => {
            let filtered = passkeys
                .into_iter()
                .filter(|passkey| passkey.get("totpId").and_then(Value::as_str) == Some(&totp_id))
                .collect::<Vec<_>>();
            response::ok(filtered).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, %totp_id, "failed to list TOTP passkeys");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "passkeys.listFailed"),
            )
        }
    }
}

async fn passkey_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.delete_passkey(&id).await {
        Ok(true) => response::success_message(admin_control_text(&translator, "passkeys.deleted"))
            .into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "passkeys.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete passkey");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "passkeys.deleteFailed"),
            )
        }
    }
}

async fn sessions_list(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.list_session_values().await {
        Ok(sessions) => {
            let mut records = Vec::with_capacity(sessions.len());
            for (id, data) in sessions {
                let data = ensure_session_comment(&state, &id, data, &translator).await;
                records.push(session_record_with_mobility(&state, id, data).await);
            }
            response::ok(records).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list auth sessions");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.listFailed"),
            )
        }
    }
}

async fn session_get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_session_value(&id).await {
        Ok(Some(data)) => {
            let data = ensure_session_comment(&state, &id, data, &translator).await;
            response::ok(session_record_with_mobility(&state, id, data).await).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load auth session");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.loadFailed"),
            )
        }
    }
}

async fn session_update_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SessionCommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut updates = Map::new();
    let comment = body.comment;
    updates.insert("comment".to_string(), Value::String(comment.clone()));
    match state.redis.update_session_value(&id, updates).await {
        Ok(Some(data)) => {
            if let Err(error) = sync_session_whitelist_comments(&state, &id, &data, &comment).await
            {
                tracing::warn!(%error, %id, "failed to sync session whitelist comments");
            }
            response::ok(session_record_with_mobility(&state, id, data).await).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update auth session comment");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.updateFailed"),
            )
        }
    }
}

async fn session_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    let session = state.redis.get_session(&id).await.ok().flatten();
    let config = if session.is_some() {
        state.redis.get_config().await.ok()
    } else {
        None
    };
    if let Some(session) = session.as_ref()
        && let Err(error) = system_events::publish_auth_logout_event(
            &state,
            json!({
                "session_id": id.clone(),
                "auth_method": session.method.clone(),
                "credential_id": session.credential_id.clone(),
                "credential_name": session.credential_name.clone(),
                "linked_totp_name": session.linked_totp_name.clone(),
                "session_comment": session.comment.clone(),
                "ip": session.ip.clone(),
                "ip_location": session.ip_location.clone(),
                "user_agent": session.user_agent.clone(),
                "login_time": session.login_time.clone(),
                "logout_source": "admin_session_delete",
            }),
        )
        .await
    {
        tracing::warn!(%error, %id, "failed to publish admin session delete logout event");
    }
    if let Err(error) = auth_mobility::destroy_session(&state, &id).await {
        tracing::warn!(%error, %id, "failed to cleanup auth mobility session during admin delete");
    }
    if let Some(session) = session.as_ref()
        && let Some(config) = config.as_ref()
    {
        match revoke_custom_post_login_ip_grant_for_session(&state, session, config).await {
            Ok(true) | Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, %id, "failed to revoke custom post-login IP grant");
            }
        }
    }
    match state.redis.delete_session(&id).await {
        Ok(()) => response::success_message(admin_control_text(&translator, "sessions.deleted"))
            .into_response(),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete auth session");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.deleteFailed"),
            )
        }
    }
}

async fn session_mobility_details(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_session_value(&id).await {
        Ok(Some(session)) => {
            let mut details = session_mobility_details_value(&state, &id, Some(&session)).await;
            hydrate_mobility_event_ip_locations(&state, &id, &mut details).await;
            response::ok(details).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            admin_control_text(&translator, "sessions.notFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to load session mobility details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_control_text(&translator, "sessions.mobilityLoadFailed"),
            )
        }
    }
}

fn auth_credential_settings_from_config(config: &Value) -> Value {
    normalize_auth_credential_settings(
        config
            .get("auth_credential_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
        legacy_auto_add_whitelist_on_login(config),
    )
}

fn normalize_auth_credential_settings(
    value: Value,
    legacy_auto_add_whitelist_on_login: Option<bool>,
) -> Value {
    let session_ttl = bounded_int_like_node(
        &value,
        "session_ttl_seconds",
        AUTH_SESSION_TTL_SECONDS_DEFAULT,
        60,
        AUTH_MAX_TTL_SECONDS,
    );
    let remember_ttl = bounded_int_like_node(
        &value,
        "remember_me_ttl_seconds",
        AUTH_REMEMBER_ME_TTL_SECONDS_DEFAULT,
        session_ttl,
        AUTH_MAX_TTL_SECONDS,
    );
    let ip_grant_mode = match value
        .get("post_login_ip_grant_mode")
        .and_then(Value::as_str)
    {
        Some("disabled") => "disabled",
        Some("custom") => "custom",
        Some("follow_session") => "follow_session",
        _ if legacy_auto_add_whitelist_on_login == Some(false) => "disabled",
        _ => "follow_session",
    };
    let post_login_ip_grant_ttl_seconds = (ip_grant_mode == "custom").then(|| {
        bounded_int_like_node(
            &value,
            "post_login_ip_grant_ttl_seconds",
            AUTH_POST_LOGIN_IP_GRANT_TTL_SECONDS_DEFAULT,
            60,
            AUTH_MAX_TTL_SECONDS,
        )
    });
    json!({
        "session_ttl_seconds": session_ttl,
        "remember_me_ttl_seconds": remember_ttl,
        "post_login_ip_grant_mode": ip_grant_mode,
        "post_login_ip_grant_ttl_seconds": post_login_ip_grant_ttl_seconds,
        "session_ip_mobility_enabled": value
            .get("session_ip_mobility_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "session_ip_mobility_window_seconds": bounded_int_like_node(
            &value,
            "session_ip_mobility_window_seconds",
            AUTH_SESSION_IP_MOBILITY_WINDOW_SECONDS_DEFAULT,
            60,
            24 * 3600,
        ),
        "passkey_bind_prompt_enabled": value
            .get("passkey_bind_prompt_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    })
}

fn is_allowed_auth_credential_setting(key: &str) -> bool {
    matches!(
        key,
        "session_ttl_seconds"
            | "remember_me_ttl_seconds"
            | "post_login_ip_grant_mode"
            | "post_login_ip_grant_ttl_seconds"
            | "session_ip_mobility_enabled"
            | "session_ip_mobility_window_seconds"
            | "passkey_bind_prompt_enabled"
    )
}

fn legacy_auto_add_whitelist_on_login(config: &Value) -> Option<bool> {
    config
        .pointer("/subdomain_mode/auto_add_whitelist_on_login")
        .and_then(Value::as_bool)
}

fn bounded_int_like_node(value: &Value, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .get(key)
        .and_then(parse_int_like_node)
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn parse_int_like_node(value: &Value) -> Option<i64> {
    let raw = match value {
        Value::Null => return None,
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => return None,
    };
    parse_int_prefix(raw.trim_start())
}

fn parse_int_prefix(value: &str) -> Option<i64> {
    let mut chars = value.chars().peekable();
    let mut sign = 1_i64;
    if let Some(next) = chars.peek().copied() {
        if next == '-' {
            sign = -1;
            chars.next();
        } else if next == '+' {
            chars.next();
        }
    }

    let digits = chars
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i64>().ok().map(|value| value * sign)
}

fn session_ip_mobility_settings_changed(previous: &Value, next: &Value) -> bool {
    previous
        .get("session_ip_mobility_enabled")
        .and_then(Value::as_bool)
        != next
            .get("session_ip_mobility_enabled")
            .and_then(Value::as_bool)
        || previous
            .get("session_ip_mobility_window_seconds")
            .and_then(Value::as_i64)
            != next
                .get("session_ip_mobility_window_seconds")
                .and_then(Value::as_i64)
}

fn node_totp_bind_comment(comment: Option<String>) -> String {
    match comment {
        Some(value) if !value.is_empty() => value,
        _ => "New Token".to_string(),
    }
}

async fn refresh_gateway_auth_runtime(state: &AppState) -> anyhow::Result<()> {
    let config = state.redis.get_config().await?;
    let auth_config = build_gateway_auth_config(&config);
    ensure_go_success(state.go_backend.set_auth_config(&auth_config).await?)
}

fn ensure_go_success(value: Value) -> anyhow::Result<()> {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Go backend returned an unsuccessful response")
    )
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("value is object")
}

async fn ensure_session_comment(
    state: &AppState,
    session_id: &str,
    mut data: Value,
    translator: &Translator,
) -> Value {
    let Some(object) = data.as_object_mut() else {
        return data;
    };
    if object.contains_key("comment") {
        let comment = normalize_auto_ip_grant_comment_value(
            object.get("comment").and_then(Value::as_str),
            translator,
        );
        if object.get("comment").and_then(Value::as_str) != Some(comment.as_str()) {
            object.insert("comment".to_string(), Value::String(comment));
        }
        return data;
    }

    let comment = match resolve_session_default_comment(state, session_id, &data, translator).await
    {
        Ok(Some(comment)) => comment,
        Ok(None) => return data,
        Err(error) => {
            tracing::warn!(%error, %session_id, "failed to resolve session default comment");
            return data;
        }
    };

    let mut updates = Map::new();
    updates.insert("comment".to_string(), Value::String(comment.clone()));
    match state.redis.update_session_value(session_id, updates).await {
        Ok(Some(updated)) => updated,
        Ok(None) | Err(_) => {
            if let Some(object) = data.as_object_mut() {
                object.insert("comment".to_string(), Value::String(comment));
            }
            data
        }
    }
}

async fn resolve_session_default_comment(
    state: &AppState,
    session_id: &str,
    session: &Value,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
    if let Some(record_id) = session
        .get("postLoginIpGrantRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(record) = state.redis.get_whitelist_record(record_id).await?
        && record.status == "active"
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    if let Some(record_id) = auth_mobility::list_session_whitelist_record_ids(state, session_id)
        .await?
        .into_iter()
        .next()
        && let Some(record) = state.redis.get_whitelist_record(&record_id).await?
        && record.status == "active"
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    if let Some(ip) = session
        .get("ip")
        .and_then(Value::as_str)
        .map(http_utils::normalize_ip)
        .filter(|value| !value.is_empty())
        && let Some(record) = latest_active_whitelist_record_by_ip(state, &ip).await?
        && let Some(comment) = record.comment.as_deref()
    {
        return Ok(Some(normalize_auto_ip_grant_comment_value(
            Some(comment),
            translator,
        )));
    }

    Ok(None)
}

async fn latest_active_whitelist_record_by_ip(
    state: &AppState,
    ip: &str,
) -> anyhow::Result<Option<WhitelistRecord>> {
    let target_ip = ip.parse::<IpAddr>().ok();
    let now = time_utils::now_ms().div_euclid(1000);
    let mut records = state
        .redis
        .list_whitelist_records()
        .await?
        .into_iter()
        .filter(|record| record.status == "active")
        .filter(|record| record.expire_at.is_none_or(|expire_at| expire_at > now))
        .filter(|record| match record.target_type() {
            "ip" => record.ip == ip,
            "cidr" => target_ip.is_some_and(|target_ip| {
                record
                    .ip
                    .parse::<ipnet::IpNet>()
                    .is_ok_and(|network| network.contains(&target_ip))
            }),
            _ => false,
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(records.into_iter().next())
}

async fn sync_session_whitelist_comments(
    state: &AppState,
    session_id: &str,
    session: &Value,
    comment: &str,
) -> anyhow::Result<()> {
    let mut record_ids = HashSet::new();
    if let Some(record_id) = session
        .get("postLoginIpGrantRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record_ids.insert(record_id.to_string());
    }
    for record_id in auth_mobility::list_session_whitelist_record_ids(state, session_id).await? {
        record_ids.insert(record_id);
    }

    let mut changed = false;
    for record_id in record_ids {
        changed |= state
            .redis
            .update_whitelist_comment(&record_id, comment.to_string())
            .await?
            .is_some();
    }
    if changed {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(())
}

async fn revoke_custom_post_login_ip_grant_for_session(
    state: &AppState,
    session: &LoginSession,
    config: &Value,
) -> anyhow::Result<bool> {
    if !should_revoke_custom_post_login_ip_grant(session, config) {
        return Ok(false);
    }
    if let Some(record_id) = session
        .post_login_ip_grant_record_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return whitelist::remove_whitelist_record_by_id(state, record_id).await;
    }
    whitelist::remove_whitelist_records_by_ip(state, &session.ip, Some("auto")).await
}

fn should_revoke_custom_post_login_ip_grant(session: &LoginSession, config: &Value) -> bool {
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

fn normalize_auto_ip_grant_comment_value(value: Option<&str>, translator: &Translator) -> String {
    let trimmed = value.unwrap_or("").trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if auth_mobility::is_auto_ip_grant_comment(trimmed) {
        translator.t("auth.autoIpGrantComment")
    } else {
        trimmed.to_string()
    }
}

fn session_record(id: String, data: Value) -> Value {
    match data {
        Value::Object(mut object) => {
            object.insert("id".to_string(), Value::String(id));
            Value::Object(object)
        }
        other => json!({ "id": id, "data": other }),
    }
}

async fn session_record_with_mobility(state: &AppState, id: String, data: Value) -> Value {
    let mut record = session_record(id.clone(), data);
    let details = session_mobility_details_value(state, &id, Some(&record)).await;
    let fnos_attachments = list_session_attachments(state, &id, "fnos-token").await;
    let trim_media_attachments = list_session_attachments(state, &id, "trim-media-token").await;
    if let Some(object) = record.as_object_mut() {
        object.insert(
            "mobility".to_string(),
            details
                .get("summary")
                .cloned()
                .unwrap_or_else(default_mobility_summary),
        );
        object.insert(
            "fnosAttachments".to_string(),
            Value::Array(fnos_attachments),
        );
        object.insert(
            "trimMediaAttachments".to_string(),
            Value::Array(trim_media_attachments),
        );
    }
    hydrate_session_record_ip_location(state, &mut record).await;
    record
}

async fn hydrate_session_record_ip_location(state: &AppState, record: &mut Value) {
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let ip = record
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() || ip.is_empty() {
        return;
    }

    match ip_location::register_usage(state, &ip, vec![format!("session|{id}")]).await {
        Ok(location) if !location.trim().is_empty() => {
            if let Some(object) = record.as_object_mut() {
                object.insert("ipLocation".to_string(), Value::String(location));
            }
        }
        Ok(_) => {}
        Err(error) => {
            tracing::debug!(%error, %id, %ip, "failed to hydrate auth session IP location")
        }
    }
}

async fn list_session_attachments(
    state: &AppState,
    session_id: &str,
    subject_type: &str,
) -> Vec<Value> {
    match list_session_attachments_inner(state, session_id, subject_type).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, %session_id, %subject_type, "failed to list auth mobility session attachments");
            Vec::new()
        }
    }
}

async fn list_session_attachments_inner(
    state: &AppState,
    session_id: &str,
    subject_type: &str,
) -> anyhow::Result<Vec<Value>> {
    let binding_prefix = format!("fn_knock:auth_mobility:binding:{subject_type}:");
    let attachment_keys = state
        .redis
        .list_auth_mobility_session_binding_keys(session_id)
        .await?
        .into_iter()
        .filter(|key| key.starts_with(&binding_prefix))
        .collect::<Vec<_>>();
    if attachment_keys.is_empty() {
        return Ok(Vec::new());
    }

    let mut stale_keys = Vec::new();
    let mut attachments = Vec::new();
    for storage_key in attachment_keys {
        let Some(binding) = state.redis.get_json_value(&storage_key).await? else {
            stale_keys.push(storage_key);
            continue;
        };
        if let Some(attachment) =
            session_attachment_from_binding(&binding, session_id, subject_type)
        {
            attachments.push(attachment);
        } else {
            stale_keys.push(storage_key);
        }
    }
    if !stale_keys.is_empty() {
        state
            .redis
            .remove_auth_mobility_session_bindings(session_id, &stale_keys)
            .await?;
    }

    attachments.sort_by(|left, right| {
        let left_ms = left
            .get("lastSeenAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        let right_ms = right
            .get("lastSeenAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        right_ms.cmp(&left_ms)
    });
    Ok(attachments)
}

fn session_attachment_from_binding(
    binding: &Value,
    session_id: &str,
    subject_type: &str,
) -> Option<Value> {
    if binding.get("subjectType").and_then(Value::as_str) != Some(subject_type)
        || binding.get("ownerSessionId").and_then(Value::as_str) != Some(session_id)
    {
        return None;
    }

    let expire_at = binding
        .get("expireAt")
        .and_then(Value::as_i64)
        .map(|seconds| Value::String(time_utils::iso_from_ms(seconds.saturating_mul(1000))))
        .unwrap_or(Value::Null);
    Some(json!({
        "subjectHash": binding.get("subjectHash").and_then(Value::as_str).unwrap_or(""),
        "currentIp": binding.get("currentIp").and_then(Value::as_str).unwrap_or(""),
        "createdAt": binding.get("createdAt").and_then(Value::as_str).unwrap_or(""),
        "lastSeenAt": binding.get("lastSeenAt").and_then(Value::as_str).unwrap_or(""),
        "expiresAt": expire_at,
    }))
}

async fn session_mobility_details_value(
    state: &AppState,
    session_id: &str,
    fallback_session: Option<&Value>,
) -> Value {
    let mut events = state
        .redis
        .get_json_value(&auth_mobility_timeline_key(session_id))
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        let left_ms = left
            .get("happenedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        let right_ms = right
            .get("happenedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or(0);
        left_ms.cmp(&right_ms)
    });
    if events.is_empty()
        && let Some(session) = fallback_session
    {
        if let Some(login_event) = build_mobility_login_event(session) {
            events.push(login_event);
        }
    }

    let stored_summary = state
        .redis
        .get_json_value(&auth_mobility_summary_key(session_id))
        .await
        .ok()
        .flatten()
        .filter(valid_mobility_summary);
    let summary = stored_summary.unwrap_or_else(|| build_mobility_summary(&events));
    json!({
        "summary": summary,
        "events": events,
    })
}

async fn hydrate_mobility_event_ip_locations(
    state: &AppState,
    session_id: &str,
    details: &mut Value,
) {
    let Some(events) = details.get_mut("events").and_then(Value::as_array_mut) else {
        return;
    };
    if events.is_empty() {
        return;
    }

    let mut seen = HashSet::new();
    let mut ips = Vec::new();
    for event in events.iter() {
        for ip_key in ["toIp", "fromIp"] {
            let ip = event
                .get(ip_key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let normalized_ip = http_utils::normalize_ip(ip);
            if !normalized_ip.is_empty() && seen.insert(normalized_ip.clone()) {
                ips.push(normalized_ip);
            }
        }
    }

    let reference = format!("session-timeline|{session_id}");
    let mut locations = BTreeMap::new();
    for ip in ips {
        match ip_location::register_usage(state, &ip, vec![reference.clone()]).await {
            Ok(location) if !location.trim().is_empty() => {
                locations.insert(ip, location);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(%error, %session_id, %ip, "failed to hydrate auth mobility event IP location");
            }
        }
    }
    apply_mobility_event_ip_locations(events, &locations);
}

fn apply_mobility_event_ip_locations(events: &mut [Value], locations: &BTreeMap<String, String>) {
    if locations.is_empty() {
        return;
    }
    for event in events {
        let Some(object) = event.as_object_mut() else {
            continue;
        };
        for (ip_key, location_key) in [("toIp", "toIpLocation"), ("fromIp", "fromIpLocation")] {
            let ip = object.get(ip_key).and_then(Value::as_str).unwrap_or("");
            let normalized_ip = http_utils::normalize_ip(ip);
            if let Some(location) = locations.get(&normalized_ip) {
                object.insert(location_key.to_string(), Value::String(location.clone()));
            }
        }
    }
}

fn auth_mobility_timeline_key(session_id: &str) -> String {
    format!("fn_knock:auth_mobility:timeline:{session_id}")
}

fn auth_mobility_summary_key(session_id: &str) -> String {
    format!("fn_knock:auth_mobility:summary:{session_id}")
}

fn build_mobility_login_event(session: &Value) -> Option<Value> {
    let ip = session.get("ip").and_then(Value::as_str)?.trim();
    if ip.is_empty() {
        return None;
    }
    let mut event = Map::new();
    event.insert("version".to_string(), Value::Number(1.into()));
    event.insert("kind".to_string(), Value::String("login".to_string()));
    let happened_at = session
        .get("loginTime")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(time_utils::now_iso);
    event.insert("happenedAt".to_string(), Value::String(happened_at));
    event.insert("source".to_string(), Value::String("login".to_string()));
    event.insert("toIp".to_string(), Value::String(ip.to_string()));
    if let Some(location) = session
        .get("ipLocation")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        event.insert(
            "toIpLocation".to_string(),
            Value::String(location.to_string()),
        );
    }
    Some(Value::Object(event))
}

fn build_mobility_summary(events: &[Value]) -> Value {
    let drift_events = events
        .iter()
        .filter(|event| event.get("kind").and_then(Value::as_str) == Some("drift"))
        .collect::<Vec<_>>();
    let last_drift = drift_events.last().copied();
    json!({
        "hasHistory": !events.is_empty(),
        "driftCount": drift_events.len(),
        "lastDriftAt": last_drift
            .and_then(|event| event.get("happenedAt"))
            .and_then(Value::as_str),
        "lastDriftSource": last_drift
            .and_then(|event| event.get("source"))
            .and_then(Value::as_str),
    })
}

fn default_mobility_summary() -> Value {
    json!({
        "hasHistory": false,
        "driftCount": 0,
        "lastDriftAt": null,
        "lastDriftSource": null,
    })
}

fn valid_mobility_summary(value: &Value) -> bool {
    value.get("hasHistory").and_then(Value::as_bool).is_some()
        && value.get("driftCount").and_then(Value::as_i64).is_some()
}

fn build_totp_import_plan(
    existing: &[TotpCredential],
    payload: &Value,
) -> Result<(Vec<TotpCredential>, Value), TotpImportRouteError> {
    if !payload.is_object() {
        return Err(totp_import_error(StatusCode::BAD_REQUEST, "payloadObject"));
    }
    if payload.get("kind").and_then(Value::as_str) != Some(TOTP_TRANSFER_KIND) {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedKind",
        ));
    }
    if payload.get("version").and_then(Value::as_u64) != Some(TOTP_TRANSFER_VERSION) {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedVersion",
        ));
    }
    let Some(items) = payload.get("credentials").and_then(Value::as_array) else {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "credentialsArray",
        ));
    };
    if items.len() > MAX_TOTP_IMPORT_COUNT {
        return Err(totp_import_error_with_max(
            StatusCode::BAD_REQUEST,
            "countExceeded",
            MAX_TOTP_IMPORT_COUNT,
        ));
    }

    let mut summary = json!({
        "imported": 0,
        "skipped_existing_id": 0,
        "skipped_existing_secret": 0,
        "skipped_file_duplicate": 0,
        "invalid": 0,
        "total": items.len()
    });
    let mut existing_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut known_secrets = existing
        .iter()
        .map(|item| item.secret.clone())
        .collect::<HashSet<_>>();
    let mut file_ids = HashSet::new();
    let mut credentials = Vec::new();
    let imported_at = time_utils::now_iso();

    for item in items {
        let Some(object) = item.as_object() else {
            increment_summary(&mut summary, "invalid");
            continue;
        };
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let secret = object
            .get("secret")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() || secret.is_empty() {
            increment_summary(&mut summary, "invalid");
            continue;
        }
        if !file_ids.insert(id.clone()) {
            increment_summary(&mut summary, "skipped_file_duplicate");
            continue;
        }
        if existing_ids.contains(&id) {
            increment_summary(&mut summary, "skipped_existing_id");
            continue;
        }
        if known_secrets.contains(&secret) {
            increment_summary(&mut summary, "skipped_existing_secret");
            continue;
        }

        existing_ids.insert(id.clone());
        known_secrets.insert(secret.clone());
        increment_summary(&mut summary, "imported");
        credentials.push(TotpCredential {
            id,
            secret,
            comment: object
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
            created_at: normalize_totp_created_at(object.get("createdAt"), &imported_at),
            access_scopes: normalize_totp_access_scopes(
                object.get("access_scopes").cloned().unwrap_or(Value::Null),
            ),
            subdomain_access: normalize_totp_subdomain_access(
                object
                    .get("subdomain_access")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        });
    }
    Ok((credentials, summary))
}

fn build_totp_export_payload(credentials: &[TotpCredential], exported_at: &str) -> Value {
    let credentials = credentials
        .iter()
        .map(|credential| {
            json!({
                "id": credential.id.trim(),
                "secret": credential.secret.trim(),
                "comment": credential.comment.trim(),
                "createdAt": normalize_totp_created_at(
                    Some(&Value::String(credential.created_at.clone())),
                    exported_at,
                ),
                "access_scopes": normalize_totp_access_scopes(credential.access_scopes.clone()),
                "subdomain_access": normalize_totp_subdomain_access(
                    credential.subdomain_access.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    let mut payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "exported_at": exported_at,
        "credentials": credentials
    });
    if !APP_LOCAL_VERSION.trim().is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert(
            "app_version".to_string(),
            Value::String(APP_LOCAL_VERSION.to_string()),
        );
    }
    payload
}

fn increment_summary(summary: &mut Value, key: &str) {
    let next = summary.get(key).and_then(Value::as_i64).unwrap_or(0) + 1;
    if let Some(object) = summary.as_object_mut() {
        object.insert(key.to_string(), Value::from(next));
    }
}

fn normalize_totp_created_at(value: Option<&Value>, fallback: &str) -> String {
    let created_at = value.and_then(Value::as_str).unwrap_or("").trim();
    if !created_at.is_empty() && time_utils::parse_iso_ms(created_at).is_some() {
        created_at.to_string()
    } else {
        fallback.to_string()
    }
}

fn percent_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_auth_credential_settings_defaults() {
        let value = normalize_auth_credential_settings(json!({ "session_ttl_seconds": 60 }), None);
        assert_eq!(value["session_ttl_seconds"], 60);
        assert_eq!(value["post_login_ip_grant_mode"], "follow_session");
        assert!(value["post_login_ip_grant_ttl_seconds"].is_null());
        assert_eq!(value["passkey_bind_prompt_enabled"], true);
    }

    #[test]
    fn normalizes_auth_credential_settings_like_node_clamps_and_nulls() {
        let value = normalize_auth_credential_settings(
            json!({
                "session_ttl_seconds": "59.9",
                "remember_me_ttl_seconds": "61.7",
                "post_login_ip_grant_mode": "follow_session",
                "post_login_ip_grant_ttl_seconds": "7200",
                "session_ip_mobility_window_seconds": 90_000
            }),
            None,
        );
        assert_eq!(value["session_ttl_seconds"], 60);
        assert_eq!(value["remember_me_ttl_seconds"], 61);
        assert!(value["post_login_ip_grant_ttl_seconds"].is_null());
        assert_eq!(value["session_ip_mobility_window_seconds"], 86_400);

        let custom = normalize_auth_credential_settings(
            json!({
                "session_ttl_seconds": 120,
                "remember_me_ttl_seconds": 60,
                "post_login_ip_grant_mode": "custom",
                "post_login_ip_grant_ttl_seconds": "10"
            }),
            None,
        );
        assert_eq!(custom["remember_me_ttl_seconds"], 120);
        assert_eq!(custom["post_login_ip_grant_ttl_seconds"], 60);
    }

    #[test]
    fn normalizes_auth_credential_settings_legacy_auto_add_flag_like_node() {
        let value = normalize_auth_credential_settings(json!({}), Some(false));
        assert_eq!(value["post_login_ip_grant_mode"], "disabled");

        let explicit = normalize_auth_credential_settings(
            json!({ "post_login_ip_grant_mode": "follow_session" }),
            Some(false),
        );
        assert_eq!(explicit["post_login_ip_grant_mode"], "follow_session");
    }

    #[test]
    fn import_plan_skips_duplicate_totp_credentials() {
        let existing = vec![TotpCredential {
            id: "a".to_string(),
            secret: "AAAA".to_string(),
            comment: String::new(),
            created_at: time_utils::now_iso(),
            access_scopes: Value::Array(Vec::new()),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        }];
        let payload = json!({
            "kind": TOTP_TRANSFER_KIND,
            "version": TOTP_TRANSFER_VERSION,
            "credentials": [
                { "id": "a", "secret": "BBBB" },
                { "id": "b", "secret": "AAAA" },
                { "id": "b", "secret": "CCCC" },
                { "id": "c", "secret": "CCCC" }
            ]
        });
        let (credentials, summary) = build_totp_import_plan(&existing, &payload).unwrap();
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].id, "c");
        assert_eq!(summary["skipped_existing_id"], 1);
        assert_eq!(summary["skipped_existing_secret"], 1);
    }

    #[test]
    fn totp_bind_comment_matches_node_truthy_fallback() {
        assert_eq!(node_totp_bind_comment(None), "New Token");
        assert_eq!(node_totp_bind_comment(Some(String::new())), "New Token");
        assert_eq!(
            node_totp_bind_comment(Some("   ".to_string())),
            "   ".to_string()
        );
    }

    #[test]
    fn import_plan_normalizes_totp_metadata_like_node() {
        let payload = json!({
            "kind": TOTP_TRANSFER_KIND,
            "version": TOTP_TRANSFER_VERSION,
            "credentials": [
                {
                    "id": " imported ",
                    "secret": " SECRET ",
                    "comment": " Comment ",
                    "createdAt": "not-a-date",
                    "access_scopes": [" docker_admin_panel ", "other", "docker_admin_panel"],
                    "subdomain_access": {
                        "mode": "custom",
                        "hosts": ["https://Example.com:8443/path", "/__select__", "bad host"]
                    }
                }
            ]
        });

        let (credentials, summary) = build_totp_import_plan(&[], &payload).unwrap();
        assert_eq!(summary["imported"], 1);
        assert_eq!(credentials.len(), 1);
        let credential = &credentials[0];
        assert_eq!(credential.id, "imported");
        assert_eq!(credential.secret, "SECRET");
        assert_eq!(credential.comment, "Comment");
        assert!(time_utils::parse_iso_ms(&credential.created_at).is_some());
        assert_eq!(credential.access_scopes, json!(["docker_admin_panel"]));
        assert_eq!(
            credential.subdomain_access,
            json!({
                "mode": "custom",
                "hosts": ["__builtin_select__", "example.com"]
            })
        );
    }

    #[test]
    fn export_payload_normalizes_totp_metadata_like_node() {
        let payload = build_totp_export_payload(
            &[TotpCredential {
                id: " id ".to_string(),
                secret: " SECRET ".to_string(),
                comment: " comment ".to_string(),
                created_at: "not-a-date".to_string(),
                access_scopes: json!(["docker_admin_panel", "unknown", "docker_admin_panel"]),
                subdomain_access: json!({
                    "mode": "custom",
                    "hosts": [
                        " HTTPS://Example.COM:443/path ",
                        "*bad.example",
                        "__builtin_select__"
                    ]
                }),
            }],
            "2026-01-02T03:04:05.000Z",
        );
        assert_eq!(payload["kind"], TOTP_TRANSFER_KIND);
        assert_eq!(payload["version"], TOTP_TRANSFER_VERSION);
        assert_eq!(
            payload["credentials"][0],
            json!({
                "id": "id",
                "secret": "SECRET",
                "comment": "comment",
                "createdAt": "2026-01-02T03:04:05.000Z",
                "access_scopes": ["docker_admin_panel"],
                "subdomain_access": {
                    "mode": "custom",
                    "hosts": ["__builtin_select__", "example.com"]
                }
            })
        );
    }

    #[test]
    fn builds_mobility_login_event_from_session() {
        let event = build_mobility_login_event(&json!({
            "ip": "203.0.113.8",
            "ipLocation": "Test City",
            "loginTime": "2026-07-05T01:02:03Z"
        }))
        .unwrap();
        assert_eq!(event["kind"], "login");
        assert_eq!(event["source"], "login");
        assert_eq!(event["toIp"], "203.0.113.8");
        assert_eq!(event["toIpLocation"], "Test City");
        assert_eq!(event["happenedAt"], "2026-07-05T01:02:03Z");
    }

    #[test]
    fn applies_cached_mobility_event_locations_like_node() {
        let mut events = vec![json!({
            "kind": "drift",
            "toIp": " 203.0.113.8 ",
            "fromIp": "2001:db8::1",
            "toIpLocation": "old"
        })];
        let locations = BTreeMap::from([
            ("203.0.113.8".to_string(), "Tokyo".to_string()),
            ("2001:db8::1".to_string(), "Seoul".to_string()),
        ]);

        apply_mobility_event_ip_locations(&mut events, &locations);

        assert_eq!(events[0]["toIpLocation"], "Tokyo");
        assert_eq!(events[0]["fromIpLocation"], "Seoul");
    }

    #[test]
    fn builds_mobility_summary_from_drift_events() {
        let summary = build_mobility_summary(&[
            json!({ "kind": "login", "happenedAt": "2026-07-05T01:00:00Z" }),
            json!({ "kind": "drift", "source": "proxy-session", "happenedAt": "2026-07-05T01:10:00Z" }),
            json!({ "kind": "drift", "source": "session-refresh", "happenedAt": "2026-07-05T01:20:00Z" }),
        ]);
        assert_eq!(summary["hasHistory"], true);
        assert_eq!(summary["driftCount"], 2);
        assert_eq!(summary["lastDriftAt"], "2026-07-05T01:20:00Z");
        assert_eq!(summary["lastDriftSource"], "session-refresh");
    }

    #[test]
    fn builds_session_attachment_from_binding_like_node() {
        let attachment = session_attachment_from_binding(
            &json!({
                "subjectType": "fnos-token",
                "subjectHash": "hash-1",
                "currentIp": "203.0.113.8",
                "createdAt": "2026-07-05T01:00:00Z",
                "lastSeenAt": "2026-07-05T01:20:00Z",
                "expireAt": 1783213200,
                "ownerSessionId": "session-1"
            }),
            "session-1",
            "fnos-token",
        )
        .unwrap();

        assert_eq!(attachment["subjectHash"], "hash-1");
        assert_eq!(attachment["currentIp"], "203.0.113.8");
        assert_eq!(attachment["createdAt"], "2026-07-05T01:00:00Z");
        assert_eq!(attachment["lastSeenAt"], "2026-07-05T01:20:00Z");
        assert_eq!(attachment["expiresAt"], "2026-07-05T01:00:00Z");
    }

    #[test]
    fn rejects_stale_session_attachment_bindings_like_node() {
        assert!(
            session_attachment_from_binding(
                &json!({
                    "subjectType": "fnos-token",
                    "ownerSessionId": "other-session"
                }),
                "session-1",
                "fnos-token",
            )
            .is_none()
        );
        assert!(
            session_attachment_from_binding(
                &json!({
                    "subjectType": "trim-media-token",
                    "ownerSessionId": "session-1"
                }),
                "session-1",
                "fnos-token",
            )
            .is_none()
        );
    }

    #[test]
    fn normalizes_auto_ip_grant_comment_like_node() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            normalize_auto_ip_grant_comment_value(
                Some(" Automatically authorized after sign-in "),
                &translator,
            ),
            "登录后自动授权"
        );
        assert_eq!(
            normalize_auto_ip_grant_comment_value(Some(" custom note "), &translator),
            "custom note"
        );
        assert_eq!(
            normalize_auto_ip_grant_comment_value(Some("   "), &translator),
            ""
        );
    }

    #[test]
    fn custom_post_login_grant_revoke_condition_matches_node() {
        let mut session = LoginSession {
            totp_id: "totp".to_string(),
            method: "TOTP".to_string(),
            credential_id: "cred".to_string(),
            credential_name: "Credential".to_string(),
            linked_totp_name: None,
            grant_type: Some("login_ip_grant".to_string()),
            post_login_ip_grant_mode: Some("custom".to_string()),
            post_login_ip_grant_record_id: None,
            comment: None,
            ip: "203.0.113.8".to_string(),
            user_agent: "test".to_string(),
            login_time: "2026-07-05T01:00:00Z".to_string(),
            expires_at: None,
            ip_location: None,
        };
        assert!(should_revoke_custom_post_login_ip_grant(
            &session,
            &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
        ));

        session.grant_type = Some("session".to_string());
        session.post_login_ip_grant_mode = Some("follow_session".to_string());
        session.comment = Some("登录后自动授权".to_string());
        assert!(should_revoke_custom_post_login_ip_grant(
            &session,
            &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "custom"}})
        ));
        assert!(!should_revoke_custom_post_login_ip_grant(
            &session,
            &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
        ));
    }

    #[test]
    fn localizes_admin_control_route_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            admin_control_text(&translator, "authCredentialSettings.loadFailed"),
            "加载认证凭据配置失败"
        );
        assert_eq!(
            admin_control_text(&translator, "totp.notFound"),
            "TOTP 凭据不存在"
        );
        assert_eq!(
            admin_control_text(&translator, "passkeys.notFound"),
            "Passkey 不存在"
        );
        assert_eq!(
            admin_control_text(&translator, "sessions.notFound"),
            "会话不存在"
        );
        let error = totp_import_error_with_max(StatusCode::BAD_REQUEST, "countExceeded", 200);
        assert_eq!(
            totp_import_error_message(&translator, &error),
            "单次最多导入 200 个 TOTP 凭证"
        );
    }
}
