use axum::{
    Extension, Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Request, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use scrypt::{Params as ScryptParams, scrypt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use subtle::ConstantTimeEq;

use crate::{
    cookies::{self, ADMIN_PANEL_SESSION_COOKIE_NAME, SESSION_COOKIE_NAME},
    http_utils,
    i18n::Translator,
    redis_store::{DockerAdminPasswordRecord, DockerAdminSessionRecord, LoginAttemptRecord},
    response::{self, ApiEnvelope},
    runtime_config,
    runtime_profile::{self, RuntimeProfile},
    state::AppState,
    time_utils,
};

const DEFAULT_SESSION_TTL_SECONDS: i64 = 12 * 60 * 60;
const MIN_SESSION_TTL_SECONDS: i64 = 15 * 60;
const MAX_SESSION_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const REMEMBER_ME_SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const LOGIN_BACKOFF_BASE_DELAY_MS: i64 = 2000;
const LOGIN_BACKOFF_MAX_DELAY_MS: i64 = 15 * 60 * 1000;
const SCRYPT_N: u32 = 16_384;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;
const SCRYPT_KEY_LENGTH: usize = 64;

fn admin_panel_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

fn admin_panel_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

fn admin_panel_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.adminPanelRoutes.{key}"))
}

fn docker_admin_panel_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.dockerAdminPanel.{key}"))
}

#[derive(Clone, Copy)]
struct PanelRuntime {
    enabled: bool,
}

#[derive(Deserialize)]
struct PasswordBody {
    password: String,
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
    #[serde(default, rename = "rememberMe")]
    remember_me: bool,
}

pub fn admin_routes(protected_admin_view: bool) -> Router<AppState> {
    Router::new()
        .route("/api/admin/panel/bootstrap", get(bootstrap))
        .route("/api/admin/panel/password", post(set_password))
        .route("/api/admin/panel/password/change", post(change_password))
        .route("/api/admin/panel/login", post(login))
        .route("/api/admin/panel/logout", post(logout))
        .route("/api/admin/config", get(config))
        .route("/api/admin/config/locale", get(locale).post(update_locale))
        .route(
            "/api/admin/config/appearance",
            get(appearance).post(update_appearance),
        )
        .layer(Extension(PanelRuntime {
            enabled: protected_admin_view,
        }))
}

pub async fn admin_auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if is_admin_public_path(path) {
        return next.run(req).await;
    }

    match resolve_panel_auth_context(&state, req.headers()).await {
        Ok(context)
            if context
                .get("authenticated")
                .and_then(Value::as_bool)
                .unwrap_or(false) =>
        {
            next.run(req).await
        }
        Ok(_) => {
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::UNAUTHORIZED,
                admin_panel_route_text(&translator, "signInRequired"),
            )
        }
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to resolve admin panel auth context");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "verifySessionFailed"),
            )
        }
    }
}

fn is_admin_public_path(path: &str) -> bool {
    matches!(
        path,
        "/api/admin/healthz"
            | "/api/admin/panel/bootstrap"
            | "/api/admin/panel/login"
            | "/api/admin/panel/password"
            | "/api/admin/panel/logout"
    )
}

async fn bootstrap(
    State(state): State<AppState>,
    Extension(runtime): Extension<PanelRuntime>,
    headers: HeaderMap,
) -> Response {
    match build_bootstrap_state(&state, &headers, runtime.enabled).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to build docker admin bootstrap state");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadStateFailed"),
            )
        }
    }
}

async fn config(State(state): State<AppState>) -> Response {
    match state.redis.get_config().await {
        Ok(mut config) => {
            enrich_gateway_logging_config(&state, &mut config).await;
            let protocol_mapping_feature =
                match runtime_config::load_protocol_mapping_feature(&state, Some(&config)).await {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(%error, "failed to load protocol mapping feature");
                        runtime_config::normalize_protocol_mapping_feature(
                            config.get("protocol_mapping_feature"),
                        )
                    }
                };
            response::ok(build_safe_app_config(
                config,
                runtime_profile::get_runtime_profile(&state),
                protocol_mapping_feature,
            ))
            .into_response()
        }
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadConfigFailed"),
            )
        }
    }
}

async fn enrich_gateway_logging_config(state: &AppState, config: &mut Value) {
    let current = config.get("gateway_logging");
    let enabled = current
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_days = current
        .and_then(|value| value.get("max_days"))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .unwrap_or(7);
    let logs_dir = match state.go_backend.get_logging_directory().await {
        Ok(value) if value.get("success").and_then(Value::as_bool) == Some(true) => value
            .pointer("/data/logs_dir")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        Ok(value) => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            tracing::warn!(%message, "Go backend rejected gateway logging directory request");
            String::new()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway logging directory");
            String::new()
        }
    };
    if let Some(object) = config.as_object_mut() {
        object.insert(
            "gateway_logging".to_string(),
            json!({
                "enabled": enabled,
                "max_days": max_days,
                "logs_dir": logs_dir,
            }),
        );
    }
}

pub(crate) fn build_safe_app_config(
    mut config: Value,
    profile: RuntimeProfile,
    protocol_mapping_feature: Value,
) -> Value {
    if !config.is_object() {
        config = crate::redis_store::default_config();
    }
    let capabilities = runtime_profile::get_runtime_capabilities(&profile);
    let ssl = safe_ssl_config(config.get("ssl"));
    let terminal_feature =
        runtime_config::normalize_terminal_feature(config.get("terminal_feature"));
    let fnos_share_bypass =
        runtime_config::normalize_fnos_share_bypass(config.get("fnos_share_bypass"));
    let fnos_port_icon_hijack =
        runtime_config::normalize_fnos_port_icon_hijack(config.get("fnos_port_icon_hijack"));
    let fnos_network_tuning =
        runtime_config::normalize_fnos_network_tuning(config.get("fnos_network_tuning"));
    let locale = normalize_locale_config(config.get("locale").unwrap_or(&Value::Null));
    let appearance = normalize_appearance_config(config.get("appearance").unwrap_or(&Value::Null));

    if let Some(object) = config.as_object_mut() {
        object.insert(
            "runtime_profile".to_string(),
            serde_json::to_value(profile).unwrap_or(Value::Null),
        );
        object.insert(
            "capabilities".to_string(),
            serde_json::to_value(capabilities).unwrap_or(Value::Null),
        );
        object.insert(
            "protocol_mapping_feature".to_string(),
            protocol_mapping_feature,
        );
        object.insert("ssl".to_string(), ssl);
        object.insert("terminal_feature".to_string(), terminal_feature);
        object.insert("fnos_share_bypass".to_string(), fnos_share_bypass);
        object.insert("fnos_port_icon_hijack".to_string(), fnos_port_icon_hijack);
        object.insert("fnos_network_tuning".to_string(), fnos_network_tuning);
        object.insert("locale".to_string(), locale);
        object.insert("appearance".to_string(), appearance);
    }
    config
}

fn safe_ssl_config(value: Option<&Value>) -> Value {
    let cert_present = value
        .and_then(|value| value.get("cert"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let key_present = value
        .and_then(|value| value.get("key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let deployment_mode = value
        .and_then(|value| value.get("deployment_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("single_active");
    let certificate_count = value
        .and_then(|value| value.get("certificates"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let mut object = serde_json::Map::new();
    object.insert(
        "enabled".to_string(),
        Value::Bool(cert_present && key_present),
    );
    if let Some(active_cert_id) = value
        .and_then(|value| value.get("active_cert_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(
            "active_cert_id".to_string(),
            Value::String(active_cert_id.to_string()),
        );
    }
    object.insert(
        "deployment_mode".to_string(),
        Value::String(deployment_mode.to_string()),
    );
    object.insert("certificate_count".to_string(), json!(certificate_count));
    Value::Object(object)
}

async fn locale(State(state): State<AppState>) -> Response {
    match state.redis.locale().await {
        Ok(locale) => response::ok(locale).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load locale config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadLocaleFailed"),
            )
        }
    }
}

async fn appearance(State(state): State<AppState>) -> Response {
    match state.redis.appearance().await {
        Ok(appearance) => response::ok(appearance).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load appearance config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadAppearanceFailed"),
            )
        }
    }
}

async fn update_locale(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let next = normalize_locale_config(&body);
    match save_config_section(&state, "locale", next.clone()).await {
        Ok(()) => {
            if let Err(error) = state.go_backend.set_locale_config(&next).await {
                tracing::warn!(%error, "failed to sync locale config to Go backend");
            }
            response::ok(next).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save locale config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "saveLocaleFailed"),
            )
        }
    }
}

async fn update_appearance(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let next = normalize_appearance_config(&body);
    match save_config_section(&state, "appearance", next.clone()).await {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save appearance config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "saveAppearanceFailed"),
            )
        }
    }
}

async fn set_password(
    State(state): State<AppState>,
    Extension(runtime): Extension<PanelRuntime>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<PasswordBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !runtime.enabled {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_panel_text(&translator, "dockerPanel.passwordNotNeeded"),
        );
    }

    if let Err(key) = validate_password(&body.password) {
        return response::error(
            StatusCode::BAD_REQUEST,
            docker_admin_panel_text(&translator, key),
        );
    }

    match state.redis.docker_admin_password().await {
        Ok(Some(_)) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                docker_admin_panel_text(&translator, "passwordAlreadyConfigured"),
            );
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to load docker admin password record");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadPasswordFailed"),
            );
        }
    }

    let record = match make_password_record(&body.password, None) {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!(%error, "failed to derive docker admin password hash");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_text(&translator, "dockerPanel.setPasswordFailed"),
            );
        }
    };
    if let Err(error) = state.redis.set_docker_admin_password(&record).await {
        tracing::warn!(%error, "failed to store docker admin password");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_panel_text(&translator, "dockerPanel.setPasswordFailed"),
        );
    }

    let session_ttl_seconds = session_ttl_seconds();
    let session = match create_panel_session(&state, &headers, session_ttl_seconds).await {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, "failed to create docker admin session");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "createSessionFailed"),
            );
        }
    };
    let _ = state
        .redis
        .reset_docker_admin_login_attempt(&client_ip_for_tracking(&headers))
        .await;

    panel_success_with_cookie(
        &state,
        &headers,
        runtime.enabled,
        Some(&session),
        cookies::admin_panel_cookie(
            &session.id,
            session_ttl_seconds,
            http_utils::is_secure_request(&headers, &uri),
        ),
    )
    .await
}

async fn change_password(
    State(state): State<AppState>,
    Extension(runtime): Extension<PanelRuntime>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<PasswordBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !runtime.enabled {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_panel_text(&translator, "dockerPanel.passwordChangeUnsupported"),
        );
    }
    if let Err(key) = validate_password(&body.password) {
        return response::error(
            StatusCode::BAD_REQUEST,
            docker_admin_panel_text(&translator, key),
        );
    }

    let existing = match state.redis.docker_admin_password().await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                docker_admin_panel_text(&translator, "passwordNotConfigured"),
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load docker admin password");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadPasswordFailed"),
            );
        }
    };

    match verify_password(&body.password, &existing) {
        Ok(true) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                docker_admin_panel_text(&translator, "newPasswordSameAsCurrent"),
            );
        }
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to verify docker admin password");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "verifyPasswordFailed"),
            );
        }
    }

    let record = match make_password_record(&body.password, Some(existing.created_at)) {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!(%error, "failed to derive docker admin password hash");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_text(&translator, "dockerPanel.changePasswordFailed"),
            );
        }
    };
    if let Err(error) = state.redis.set_docker_admin_password(&record).await {
        tracing::warn!(%error, "failed to store docker admin password");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_panel_text(&translator, "dockerPanel.changePasswordFailed"),
        );
    }

    if let Err(error) = state.redis.clear_docker_admin_sessions().await {
        tracing::warn!(%error, "failed to clear docker admin sessions after password change");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_panel_text(&translator, "dockerPanel.changePasswordFailed"),
        );
    }
    if let Err(error) = state.redis.clear_docker_admin_login_failures().await {
        tracing::warn!(%error, "failed to clear docker admin login failures after password change");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_panel_text(&translator, "dockerPanel.changePasswordFailed"),
        );
    }

    let session_ttl_seconds = session_ttl_seconds();
    let session = match create_panel_session(&state, &headers, session_ttl_seconds).await {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, "failed to create docker admin session after password change");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "createSessionFailed"),
            );
        }
    };

    panel_success_with_cookie(
        &state,
        &headers,
        runtime.enabled,
        Some(&session),
        cookies::admin_panel_cookie(
            &session.id,
            session_ttl_seconds,
            http_utils::is_secure_request(&headers, &uri),
        ),
    )
    .await
}

async fn login(
    State(state): State<AppState>,
    Extension(runtime): Extension<PanelRuntime>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<LoginBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !runtime.enabled {
        return match build_bootstrap_state(&state, &headers, false).await {
            Ok(data) => response::ok(data).into_response(),
            Err(_) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadStateFailed"),
            ),
        };
    }

    let client_ip = client_ip_for_tracking(&headers);
    match ensure_login_allowed(&state, &client_ip).await {
        Ok(Some((retry_after, blocked_until))) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(
                    header::RETRY_AFTER,
                    HeaderValue::from_str(&retry_after.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("1")),
                )],
                Json(json!({
                    "success": false,
                    "message": admin_panel_text_params(
                        &translator,
                        "dockerPanel.tooManyAttemptsWithRetry",
                        &[("seconds", retry_after.to_string())],
                    ),
                    "retryAfter": retry_after,
                    "blockedUntil": blocked_until
                })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to inspect docker admin login backoff");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "checkLoginRateLimitFailed"),
            );
        }
    }

    let Some(password_record) = (match state.redis.docker_admin_password().await {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!(%error, "failed to load docker admin password");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadPasswordFailed"),
            );
        }
    }) else {
        return response::error(
            StatusCode::CONFLICT,
            admin_panel_text(&translator, "dockerPanel.passwordSetupRequired"),
        );
    };

    match verify_password(&body.password, &password_record) {
        Ok(true) => {}
        Ok(false) => {
            let (retry_after, blocked_until) =
                match register_login_failure(&state, &client_ip).await {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(%error, "failed to register docker admin login failure");
                        (2, time_utils::now_ms() + 2000)
                    }
                };
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(
                    header::RETRY_AFTER,
                    HeaderValue::from_str(&retry_after.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("1")),
                )],
                Json(json!({
                    "success": false,
                    "message": admin_panel_text_params(
                        &translator,
                        "dockerPanel.passwordIncorrectWithRetry",
                        &[("seconds", retry_after.to_string())],
                    ),
                    "retryAfter": retry_after,
                    "blockedUntil": blocked_until
                })),
            )
                .into_response();
        }
        Err(error) => {
            tracing::warn!(%error, "failed to verify docker admin password");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "verifyPasswordFailed"),
            );
        }
    }

    let _ = state
        .redis
        .reset_docker_admin_login_attempt(&client_ip)
        .await;
    let ttl = if body.remember_me {
        REMEMBER_ME_SESSION_TTL_SECONDS
    } else {
        session_ttl_seconds()
    };
    let session = match create_panel_session(&state, &headers, ttl).await {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, "failed to create docker admin session");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "createSessionFailed"),
            );
        }
    };

    panel_success_with_cookie(
        &state,
        &headers,
        runtime.enabled,
        Some(&session),
        cookies::admin_panel_cookie(
            &session.id,
            ttl,
            http_utils::is_secure_request(&headers, &uri),
        ),
    )
    .await
}

async fn logout(
    State(state): State<AppState>,
    Extension(runtime): Extension<PanelRuntime>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    if let Some(session_id) = cookies::read_cookie(&headers, ADMIN_PANEL_SESSION_COOKIE_NAME) {
        let _ = state.redis.delete_docker_admin_session(&session_id).await;
    }
    panel_success_with_cookie(
        &state,
        &headers,
        runtime.enabled,
        None,
        cookies::admin_panel_clear_cookie(http_utils::is_secure_request(&headers, &uri)),
    )
    .await
}

async fn panel_success_with_cookie(
    state: &AppState,
    headers: &HeaderMap,
    runtime_enabled: bool,
    new_session: Option<&DockerAdminSessionRecord>,
    cookie: String,
) -> Response {
    let data = match build_bootstrap_state_with_session(
        state,
        headers,
        runtime_enabled,
        new_session,
    )
    .await
    {
        Ok(data) => data,
        Err(error) => {
            let translator = Translator::from_state(state).await;
            tracing::warn!(%error, "failed to build docker admin bootstrap state");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_panel_route_text(&translator, "loadStateFailed"),
            );
        }
    };
    (
        [(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
        )],
        Json(ApiEnvelope {
            success: true,
            code: None,
            message: None,
            data: Some(data),
        }),
    )
        .into_response()
}

async fn build_bootstrap_state(
    state: &AppState,
    headers: &HeaderMap,
    runtime_enabled: bool,
) -> anyhow::Result<Value> {
    build_bootstrap_state_with_session(state, headers, runtime_enabled, None).await
}

async fn build_bootstrap_state_with_session(
    state: &AppState,
    headers: &HeaderMap,
    runtime_enabled: bool,
    new_session: Option<&DockerAdminSessionRecord>,
) -> anyhow::Result<Value> {
    let locale = state.redis.locale().await?;
    let appearance = state.redis.appearance().await?;
    let deployment_target = runtime_profile::deployment_target(state);

    if !runtime_enabled {
        return Ok(json!({
            "deployment_target": deployment_target,
            "enabled": false,
            "password_configured": false,
            "authenticated": true,
            "auth_source": null,
            "session_expires_at": null,
            "locale": locale,
            "appearance": appearance
        }));
    }

    let password_configured = state.redis.docker_admin_password().await?.is_some();
    let auth_context = if let Some(session) = new_session {
        json!({
            "authenticated": true,
            "auth_source": "panel_session",
            "session_expires_at": session.expires_at
        })
    } else {
        resolve_panel_auth_context(state, headers).await?
    };

    Ok(json!({
        "deployment_target": deployment_target,
        "enabled": true,
        "password_configured": password_configured,
        "authenticated": auth_context.get("authenticated").and_then(Value::as_bool).unwrap_or(false),
        "auth_source": auth_context.get("auth_source").cloned().unwrap_or(Value::Null),
        "session_expires_at": auth_context.get("session_expires_at").cloned().unwrap_or(Value::Null),
        "locale": locale,
        "appearance": appearance
    }))
}

pub(crate) async fn resolve_panel_auth_context(
    state: &AppState,
    headers: &HeaderMap,
) -> anyhow::Result<Value> {
    if let Some(session_id) = cookies::read_cookie(headers, ADMIN_PANEL_SESSION_COOKIE_NAME) {
        if let Some(mut record) = state.redis.docker_admin_session(&session_id).await? {
            let now = time_utils::now_ms();
            if time_utils::parse_iso_ms(&record.expires_at).is_some_and(|expires| expires > now)
                && record.ip == client_ip_for_tracking(headers)
                && record.user_agent == user_agent_for_tracking(headers)
            {
                record.ttl_seconds = normalize_session_record_ttl(record.ttl_seconds);
                record.updated_at = time_utils::now_iso();
                record.expires_at = time_utils::iso_after_seconds(record.ttl_seconds);
                state.redis.set_docker_admin_session(&record).await?;
                return Ok(json!({
                    "authenticated": true,
                    "auth_source": "panel_session",
                    "session_expires_at": record.expires_at
                }));
            }
            let _ = state.redis.delete_docker_admin_session(&session_id).await;
        }
    }

    if let Some(session_id) = cookies::read_cookie(headers, SESSION_COOKIE_NAME) {
        if let Some(session) = state.redis.get_session(&session_id).await? {
            let totps = state.redis.get_totps().await?;
            if is_docker_admin_panel_reauth_session_allowed(&session, &totps) {
                return Ok(json!({
                    "authenticated": true,
                    "auth_source": "reauth_session",
                    "session_expires_at": session.expires_at
                }));
            }
        }
    }

    Ok(json!({
        "authenticated": false,
        "auth_source": null,
        "session_expires_at": null
    }))
}

fn is_docker_admin_panel_reauth_session_allowed(
    session: &crate::redis_store::LoginSession,
    totps: &[crate::redis_store::TotpCredential],
) -> bool {
    if session.totp_id.trim().is_empty() {
        return false;
    }
    if !session
        .expires_at
        .as_deref()
        .and_then(time_utils::parse_iso_ms)
        .is_some_and(|expires| expires > time_utils::now_ms())
    {
        return false;
    }
    totps
        .iter()
        .find(|credential| credential.id == session.totp_id)
        .is_some_and(|credential| has_docker_admin_panel_access_scope(&credential.access_scopes))
}

fn has_docker_admin_panel_access_scope(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .map(str::trim)
                .is_some_and(|scope| scope == "docker_admin_panel")
        })
    })
}

async fn create_panel_session(
    state: &AppState,
    headers: &HeaderMap,
    ttl_seconds: i64,
) -> anyhow::Result<DockerAdminSessionRecord> {
    let ttl_seconds = normalize_session_create_ttl(ttl_seconds);
    let now = time_utils::now_iso();
    let record = DockerAdminSessionRecord {
        id: hex::encode(random_bytes::<32>()),
        created_at: now.clone(),
        updated_at: now,
        expires_at: time_utils::iso_after_seconds(ttl_seconds),
        ttl_seconds,
        ip: client_ip_for_tracking(headers),
        user_agent: user_agent_for_tracking(headers),
    };
    state.redis.set_docker_admin_session(&record).await?;
    Ok(record)
}

fn session_ttl_seconds() -> i64 {
    env::var("DOCKER_ADMIN_SESSION_TTL_SECONDS")
        .ok()
        .and_then(|value| parse_int_prefix_like_node(&value))
        .map(|value| value.clamp(MIN_SESSION_TTL_SECONDS, MAX_SESSION_TTL_SECONDS))
        .unwrap_or(DEFAULT_SESSION_TTL_SECONDS)
}

fn normalize_session_create_ttl(ttl_seconds: i64) -> i64 {
    if ttl_seconds > 0 {
        ttl_seconds
    } else {
        session_ttl_seconds()
    }
}

fn normalize_session_record_ttl(ttl_seconds: i64) -> i64 {
    if ttl_seconds > 0 {
        ttl_seconds
    } else {
        session_ttl_seconds()
    }
}

fn parse_int_prefix_like_node(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix_trim_start(value)
}

fn make_password_record(
    password: &str,
    created_at: Option<String>,
) -> anyhow::Result<DockerAdminPasswordRecord> {
    let now = time_utils::now_iso();
    let salt = hex::encode(random_bytes::<16>());
    let hash = derive_password_hash(
        password,
        &salt,
        SCRYPT_N,
        SCRYPT_R,
        SCRYPT_P,
        SCRYPT_KEY_LENGTH,
    )?;
    Ok(DockerAdminPasswordRecord {
        algorithm: "scrypt".to_string(),
        salt,
        hash,
        n: SCRYPT_N,
        r: SCRYPT_R,
        p: SCRYPT_P,
        key_length: SCRYPT_KEY_LENGTH,
        created_at: created_at.unwrap_or_else(|| now.clone()),
        updated_at: now,
    })
}

fn verify_password(password: &str, record: &DockerAdminPasswordRecord) -> anyhow::Result<bool> {
    if record.algorithm != "scrypt" {
        return Ok(false);
    }
    let expected = derive_password_hash(
        password,
        &record.salt,
        record.n.max(2),
        record.r.max(1),
        record.p.max(1),
        record.key_length.max(1),
    )?;
    Ok(expected
        .as_bytes()
        .ct_eq(record.hash.as_bytes())
        .unwrap_u8()
        == 1)
}

fn derive_password_hash(
    password: &str,
    salt_hex: &str,
    n: u32,
    r: u32,
    p: u32,
    key_length: usize,
) -> anyhow::Result<String> {
    let salt = hex::decode(salt_hex)?;
    let log_n = n.ilog2() as u8;
    let params = ScryptParams::new(log_n, r, p)?;
    let mut output = vec![0u8; key_length];
    scrypt(password.as_bytes(), &salt, &params, &mut output)?;
    Ok(hex::encode(output))
}

fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 6 {
        return Err("passwordTooShort");
    }
    if password.len() > 128 {
        return Err("passwordTooLong");
    }
    if password.chars().any(char::is_whitespace) {
        return Err("passwordWhitespace");
    }
    if !password.chars().any(|value| value.is_ascii_alphabetic())
        || !password.chars().any(|value| value.is_ascii_digit())
    {
        return Err("passwordNeedsLettersAndNumbers");
    }
    Ok(())
}

async fn ensure_login_allowed(state: &AppState, ip: &str) -> anyhow::Result<Option<(i64, i64)>> {
    let Some(record) = state.redis.docker_admin_login_attempt(ip).await? else {
        return Ok(None);
    };
    let now = time_utils::now_ms();
    if record.blocked_until <= now {
        return Ok(None);
    }
    Ok(Some((
        ((record.blocked_until - now).max(1000) + 999) / 1000,
        record.blocked_until,
    )))
}

async fn register_login_failure(state: &AppState, ip: &str) -> anyhow::Result<(i64, i64)> {
    let previous_attempts = state
        .redis
        .docker_admin_login_attempt(ip)
        .await?
        .map(|record| record.attempts)
        .unwrap_or(0);
    let attempts = previous_attempts.saturating_add(1);
    let exponent = attempts.saturating_sub(1).min(30);
    let backoff_ms =
        (LOGIN_BACKOFF_BASE_DELAY_MS * 2_i64.pow(exponent)).min(LOGIN_BACKOFF_MAX_DELAY_MS);
    let blocked_until = time_utils::now_ms() + backoff_ms;
    let record = LoginAttemptRecord {
        ip: ip.to_string(),
        attempts,
        last_attempt_at: time_utils::now_iso(),
        blocked_until,
    };
    state.redis.set_docker_admin_login_attempt(&record).await?;
    Ok(((backoff_ms + 999) / 1000, blocked_until))
}

fn client_ip_for_tracking(headers: &HeaderMap) -> String {
    let ip = http_utils::get_client_ip(headers);
    if ip.is_empty() {
        "unknown".to_string()
    } else {
        ip
    }
}

fn user_agent_for_tracking(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn save_config_section(state: &AppState, key: &str, value: Value) -> redis::RedisResult<()> {
    let mut config = state.redis.get_config().await?;
    if !config.is_object() {
        config = crate::redis_store::default_config();
    }
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), value);
    }
    state.redis.save_config(&config).await
}

pub(crate) fn normalize_locale_config(value: &Value) -> Value {
    let locale = value
        .get("default_locale")
        .and_then(Value::as_str)
        .unwrap_or("zh-CN");
    let default_locale = match locale {
        "zh-CN" | "zh-Hant" | "en" | "ko-KR" | "ja-JP" => locale,
        _ => "zh-CN",
    };
    json!({ "default_locale": default_locale })
}

fn normalize_appearance_config(value: &Value) -> Value {
    let preset = value
        .get("theme_color_preset")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let theme_color_preset = match preset {
        "default" | "hermes_orange" | "prussian_blue" | "dynamic_white" => preset,
        _ => "default",
    };
    json!({ "theme_color_preset": theme_color_preset })
}

fn random_bytes<const N: usize>() -> [u8; N] {
    rand::random::<[u8; N]>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: impl AsRef<OsStr>) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            set_env(key, value);
        } else {
            remove_env(key);
        }
    }

    fn with_env_var<T>(key: &str, run: impl FnOnce() -> T) -> T {
        let previous = env::var_os(key);
        let result = run();
        restore_env(key, previous);
        result
    }

    #[test]
    fn validates_docker_admin_password_rules() {
        assert!(validate_password("abc123").is_ok());
        assert!(validate_password("abc12").is_err());
        assert!(validate_password("abcdef").is_err());
        assert!(validate_password("123456").is_err());
        assert!(validate_password("abc 123").is_err());
    }

    #[test]
    fn localizes_admin_panel_route_and_password_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            admin_panel_route_text(&zh, "loadStateFailed"),
            "加载管理面板状态失败"
        );
        assert_eq!(
            docker_admin_panel_text(&zh, validate_password("abc12").unwrap_err()),
            "管理面板密码至少需要 6 位"
        );
    }

    #[test]
    fn verifies_scrypt_password_record() {
        let record = make_password_record("abc123", None).expect("make record");
        assert!(verify_password("abc123", &record).expect("verify password"));
        assert!(!verify_password("wrong123", &record).expect("verify wrong password"));
        assert_eq!(record.n, 16_384);
        assert_eq!(record.r, 8);
        assert_eq!(record.p, 1);
        assert_eq!(record.key_length, 64);
    }

    #[test]
    fn docker_admin_session_ttl_matches_node_env_rules() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_env_var("DOCKER_ADMIN_SESSION_TTL_SECONDS", || {
            remove_env("DOCKER_ADMIN_SESSION_TTL_SECONDS");
            assert_eq!(session_ttl_seconds(), DEFAULT_SESSION_TTL_SECONDS);

            set_env("DOCKER_ADMIN_SESSION_TTL_SECONDS", "60s");
            assert_eq!(session_ttl_seconds(), MIN_SESSION_TTL_SECONDS);

            set_env("DOCKER_ADMIN_SESSION_TTL_SECONDS", "3600.9");
            assert_eq!(session_ttl_seconds(), 3600);

            set_env("DOCKER_ADMIN_SESSION_TTL_SECONDS", "999999999");
            assert_eq!(session_ttl_seconds(), MAX_SESSION_TTL_SECONDS);

            set_env("DOCKER_ADMIN_SESSION_TTL_SECONDS", "nope");
            assert_eq!(session_ttl_seconds(), DEFAULT_SESSION_TTL_SECONDS);
        });
    }

    #[test]
    fn docker_admin_session_record_ttl_falls_back_like_node() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_env_var("DOCKER_ADMIN_SESSION_TTL_SECONDS", || {
            set_env("DOCKER_ADMIN_SESSION_TTL_SECONDS", "7200");
            assert_eq!(normalize_session_create_ttl(0), 7200);
            assert_eq!(normalize_session_record_ttl(-1), 7200);
            assert_eq!(
                normalize_session_create_ttl(REMEMBER_ME_SESSION_TTL_SECONDS),
                REMEMBER_ME_SESSION_TTL_SECONDS
            );
            assert_eq!(normalize_session_record_ttl(42), 42);
        });
    }

    #[test]
    fn docker_admin_reauth_session_requires_access_scope() {
        let session = crate::redis_store::LoginSession {
            totp_id: "totp-1".to_string(),
            method: "totp".to_string(),
            credential_id: "totp-1".to_string(),
            credential_name: "Admin".to_string(),
            linked_totp_name: None,
            grant_type: Some("browser_session".to_string()),
            post_login_ip_grant_mode: None,
            post_login_ip_grant_record_id: None,
            comment: None,
            ip: "127.0.0.1".to_string(),
            user_agent: "test".to_string(),
            login_time: time_utils::now_iso(),
            expires_at: Some(time_utils::iso_after_seconds(60)),
            ip_location: None,
        };
        let allowed = crate::redis_store::TotpCredential {
            id: "totp-1".to_string(),
            secret: "secret".to_string(),
            comment: String::new(),
            created_at: String::new(),
            access_scopes: json!(["docker_admin_panel"]),
            subdomain_access: Value::Null,
        };
        let denied = crate::redis_store::TotpCredential {
            access_scopes: json!(["other"]),
            ..allowed.clone()
        };

        assert!(is_docker_admin_panel_reauth_session_allowed(
            &session,
            &[allowed]
        ));
        assert!(!is_docker_admin_panel_reauth_session_allowed(
            &session,
            &[denied]
        ));
        assert!(has_docker_admin_panel_access_scope(&json!([
            " docker_admin_panel "
        ])));
        assert!(!has_docker_admin_panel_access_scope(&json!(["other"])));
    }

    #[test]
    fn normalizes_locale_and_appearance_config() {
        assert_eq!(
            normalize_locale_config(&json!({ "default_locale": "en" })),
            json!({ "default_locale": "en" })
        );
        assert_eq!(
            normalize_locale_config(&json!({ "default_locale": "xx" })),
            json!({ "default_locale": "zh-CN" })
        );
        assert_eq!(
            normalize_appearance_config(&json!({ "theme_color_preset": "prussian_blue" })),
            json!({ "theme_color_preset": "prussian_blue" })
        );
    }

    #[test]
    fn safe_app_config_injects_runtime_capabilities_and_redacts_ssl() {
        let config = build_safe_app_config(
            json!({
                "ssl": {
                    "cert": "CERT",
                    "key": "KEY",
                    "active_cert_id": "cert-1",
                    "deployment_mode": "single_active",
                    "certificates": [{ "id": "cert-1" }]
                },
                "terminal_feature": { "enabled": true },
                "protocol_mapping_feature": { "enabled": false },
                "locale": { "default_locale": "en" },
                "appearance": { "theme_color_preset": "prussian_blue" }
            }),
            RuntimeProfile {
                deployment_target: "fpk".to_string(),
                is_docker: false,
                is_linux: true,
                is_root_process: true,
            },
            json!({ "enabled": true }),
        );
        assert_eq!(
            config.pointer("/runtime_profile/deployment_target"),
            Some(&json!("fpk"))
        );
        assert_eq!(
            config.pointer("/capabilities/host_firewall_available"),
            Some(&json!(true))
        );
        assert_eq!(
            config.pointer("/capabilities/terminal_available"),
            Some(&json!(true))
        );
        assert_eq!(config.pointer("/ssl/cert"), None);
        assert_eq!(config.pointer("/ssl/key"), None);
        assert_eq!(config.pointer("/ssl/enabled"), Some(&json!(true)));
        assert_eq!(config.pointer("/ssl/certificate_count"), Some(&json!(1)));
        assert_eq!(
            config.pointer("/terminal_feature/resume_backend"),
            Some(&json!("tmux"))
        );
        assert_eq!(
            config.pointer("/protocol_mapping_feature/enabled"),
            Some(&json!(true))
        );
    }
}
