use std::fs;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{i18n::Translator, response, state::AppState};

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
        host_runtime_available, smart_connect_unavailable_message, system_clock_unavailable_message,
    },
    text::{dnsmasq_text, tunnel_manager_text, tunnel_manager_text_params},
};

pub(super) async fn clock_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(cached_clock_status(&state, &translator).await).into_response()
}

pub(super) async fn clock_check(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(refresh_clock_status(&state, &translator).await).into_response()
}

pub(super) async fn clock_sync(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_runtime_available(&state) {
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

pub(super) async fn cloudflared_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_cloudflared_status(
        &state.settings.data_dir,
        &translator,
    ))
    .into_response()
}

pub(super) async fn cloudflared_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if start_download("cloudflared") {
        tokio::spawn(download_cloudflared(state));
    }
    response::success_message(tunnel_manager_text(
        &translator,
        "cloudflared",
        "downloadStarted",
    ))
    .into_response()
}

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

pub(super) async fn cloudflared_delete(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if let Some(message) =
        cloudflared_delete_unsupported_message(&translator, detect_cloudflared_platform())
    {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
    }
    let path = state
        .settings
        .data_dir
        .join("cloudflared")
        .join("cloudflared");
    if path.exists()
        && let Err(error) = fs::remove_file(&path)
    {
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
    (platform == "darwin")
        .then(|| tunnel_manager_text(translator, "cloudflared", "macManualRemove"))
}

pub(super) async fn frp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_frp_status(&state.settings.data_dir, &translator)).into_response()
}

pub(super) async fn frp_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if start_download("frp") {
        tokio::spawn(download_frp(state));
    }
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadStarted"))
        .into_response()
}

pub(super) async fn frp_cancel(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    request_cancel("frp");
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadCancelled"))
        .into_response()
}

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

pub(super) async fn dnsmasq_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_dnsmasq_status_with_translator(&translator)).into_response()
}

pub(super) async fn dnsmasq_install(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_runtime_available(&state) {
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
