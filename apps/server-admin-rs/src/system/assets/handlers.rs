use std::fs;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{
    cloudflared,
    cloudflared_utils::{
        cloudflared_asset_name, cloudflared_binary_path, cloudflared_install_metadata_path,
    },
    i18n::Translator,
    response,
    state::AppState,
};

use super::{
    clock::{cached_clock_status, refresh_clock_status, sync_system_clock},
    dnsmasq::{
        build_dnsmasq_status_with_translator, dnsmasq_install_state_json,
        install_dnsmasq_background, set_dnsmasq_install_state,
    },
    downloads::{
        build_cloudflared_status, build_frp_status, detect_cloudflared_platform,
        detect_frp_platform, download_cloudflared, download_frp, frp_extracted_dir, request_cancel,
        reset_progress, start_download,
    },
    runtime::{
        smart_connect_available, smart_connect_unavailable_message, system_clock_sync_available,
        system_clock_unavailable_message,
    },
    text::{dnsmasq_text, tunnel_manager_text, tunnel_manager_text_params},
};

#[utoipa::path(
    get,
    path = "/api/admin/system/clock/status",
    tag = "system",
    operation_id = "get_api_admin_system_clock_status",
    responses((status = 200, description = "Cached system clock status"))
)]
pub(super) async fn clock_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(cached_clock_status(&state, &translator).await).into_response()
}

#[utoipa::path(
    post,
    path = "/api/admin/system/clock/check",
    tag = "system",
    operation_id = "post_api_admin_system_clock_check",
    responses((status = 200, description = "Refreshed system clock status"))
)]
pub(super) async fn clock_check(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(refresh_clock_status(&state, &translator).await).into_response()
}

#[utoipa::path(
    post,
    path = "/api/admin/system/clock/sync",
    tag = "system",
    operation_id = "post_api_admin_system_clock_sync",
    responses((status = 200, description = "System clock synchronization result"))
)]
pub(super) async fn clock_sync(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !system_clock_sync_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            system_clock_unavailable_message(&state, &translator),
        );
    }
    match sync_system_clock(&state, &translator).await {
        Ok((message, data)) => axum::Json(json!({
            "success": true,
            "message": message,
            "data": data
        }))
        .into_response(),
        Err(error) => response::error(StatusCode::BAD_REQUEST, error),
    }
}

#[utoipa::path(get, path = "/api/admin/system/cloudflared/status", tag = "system", operation_id = "get_api_admin_system_cloudflared_status", responses((status = 200, description = "Cloudflared binary status")))]
pub(super) async fn cloudflared_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_cloudflared_status(
        &state.settings.data_dir,
        &translator,
    ))
    .into_response()
}

#[utoipa::path(post, path = "/api/admin/system/cloudflared/download", tag = "system", operation_id = "post_api_admin_system_cloudflared_download", responses((status = 200, description = "Cloudflared download started")))]
pub(super) async fn cloudflared_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if start_download("cloudflared") {
        let task_state = state.clone();
        state.spawn_background("cloudflared-download", download_cloudflared(task_state));
    }
    response::success_message(tunnel_manager_text(
        &translator,
        "cloudflared",
        "downloadStarted",
    ))
    .into_response()
}

#[utoipa::path(post, path = "/api/admin/system/cloudflared/cancel", tag = "system", operation_id = "post_api_admin_system_cloudflared_cancel", responses((status = 200, description = "Cloudflared download cancelled")))]
pub(super) async fn cloudflared_cancel(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    request_cancel("cloudflared");
    response::success_message(tunnel_manager_text(
        &translator,
        "cloudflared",
        "downloadCancelled",
    ))
    .into_response()
}

#[utoipa::path(delete, path = "/api/admin/system/cloudflared", tag = "system", operation_id = "delete_api_admin_system_cloudflared", responses((status = 200, description = "Cloudflared binary removed")))]
pub(super) async fn cloudflared_delete(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let platform = detect_cloudflared_platform();
    if let Some(message) = cloudflared_delete_unsupported_message(&translator, platform) {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
    }
    let Some(path) = cloudflared_binary_path(&state.settings.data_dir, platform) else {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            tunnel_manager_text(&translator, "cloudflared", "platformUnsupported"),
        );
    };
    let _manage_guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let should_resume = match cloudflared::pause_cloudflared_for_asset_update(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to stop cloudflared before deleting its binary");
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, error);
        }
    };
    if path.exists()
        && let Err(error) = fs::remove_file(&path)
    {
        if let Err(resume_error) =
            cloudflared::resume_cloudflared_after_asset_update(&state, should_resume).await
        {
            tracing::error!(%resume_error, "failed to resume cloudflared after delete failure");
        }
        tracing::warn!(%error, path = %path.display(), "failed to delete cloudflared binary");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            tunnel_manager_text_params(
                &translator,
                "cloudflared",
                "deleteFailed",
                &[("detail", error.to_string())],
            ),
        );
    }
    let metadata_path = cloudflared_install_metadata_path(&state.settings.data_dir);
    if metadata_path.exists()
        && let Err(error) = fs::remove_file(&metadata_path)
    {
        tracing::warn!(%error, path = %metadata_path.display(), "failed to delete cloudflared install metadata");
    }
    reset_progress("cloudflared");
    response::success_message(tunnel_manager_text(
        &translator,
        "cloudflared",
        "deleteSuccess",
    ))
    .into_response()
}

pub(super) fn cloudflared_delete_unsupported_message(
    translator: &Translator,
    platform: &str,
) -> Option<String> {
    cloudflared_asset_name(platform)
        .is_none()
        .then(|| tunnel_manager_text(translator, "cloudflared", "platformUnsupported"))
}

#[utoipa::path(get, path = "/api/admin/system/frp/status", tag = "system", operation_id = "get_api_admin_system_frp_status", responses((status = 200, description = "FRP binary status")))]
pub(super) async fn frp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_frp_status(&state.settings.data_dir, &translator)).into_response()
}

#[utoipa::path(post, path = "/api/admin/system/frp/download", tag = "system", operation_id = "post_api_admin_system_frp_download", responses((status = 200, description = "FRP download started")))]
pub(super) async fn frp_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if start_download("frp") {
        let task_state = state.clone();
        state.spawn_background("frp-download", download_frp(task_state));
    }
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadStarted"))
        .into_response()
}

#[utoipa::path(post, path = "/api/admin/system/frp/cancel", tag = "system", operation_id = "post_api_admin_system_frp_cancel", responses((status = 200, description = "FRP download cancelled")))]
pub(super) async fn frp_cancel(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    request_cancel("frp");
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadCancelled"))
        .into_response()
}

#[utoipa::path(delete, path = "/api/admin/system/frp", tag = "system", operation_id = "delete_api_admin_system_frp", responses((status = 200, description = "FRP binary removed")))]
pub(super) async fn frp_delete(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let frp_dir = state.settings.data_dir.join("frp");
    let tar_path = frp_dir.join("frp.tar.gz");
    if tar_path.exists() {
        let _ = fs::remove_file(tar_path);
    }
    let platform = detect_frp_platform();
    if let Some(path) = frp_extracted_dir(&state.settings.data_dir, platform)
        && path.exists()
        && let Err(error) = fs::remove_dir_all(&path)
    {
        tracing::warn!(%error, path = %path.display(), "failed to delete frp directory");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            tunnel_manager_text_params(
                &translator,
                "frp",
                "deleteFailed",
                &[("detail", error.to_string())],
            ),
        );
    }
    reset_progress("frp");
    response::success_message(tunnel_manager_text(&translator, "frp", "deleteSuccess"))
        .into_response()
}

#[utoipa::path(get, path = "/api/admin/system/dnsmasq/status", tag = "system", operation_id = "get_api_admin_system_dnsmasq_status", responses((status = 200, description = "dnsmasq runtime status")))]
pub(super) async fn dnsmasq_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_dnsmasq_status_with_translator(&translator)).into_response()
}

#[utoipa::path(post, path = "/api/admin/system/dnsmasq/install", tag = "system", operation_id = "post_api_admin_system_dnsmasq_install", responses((status = 200, description = "dnsmasq installation state")))]
pub(super) async fn dnsmasq_install(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !smart_connect_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            smart_connect_unavailable_message(&state, &translator),
        );
    }
    let status = build_dnsmasq_status_with_translator(&translator);
    if status
        .pointer("/install_state/status")
        .and_then(Value::as_str)
        == Some("installing")
    {
        return response::ok(status["install_state"].clone()).into_response();
    }
    if status.get("installed").and_then(Value::as_bool) == Some(true)
        && status.get("service_active").and_then(Value::as_bool) == Some(true)
        && status.get("initialized").and_then(Value::as_bool) == Some(true)
    {
        return response::ok(status["install_state"].clone()).into_response();
    }

    set_dnsmasq_install_state(
        "installing",
        10,
        dnsmasq_text(&translator, "checkingEnvironment"),
    );
    let already_installed = status.get("installed").and_then(Value::as_bool) == Some(true);
    let install_translator = translator.clone();
    std::thread::spawn(move || install_dnsmasq_background(already_installed, install_translator));
    response::ok(dnsmasq_install_state_json(&translator)).into_response()
}
