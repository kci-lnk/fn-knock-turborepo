use std::sync::{Mutex, OnceLock};

use axum::Router;
use serde_json::Value;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{i18n::Translator, state::AppState};

mod clock;
mod dnsmasq;
mod downloads;
mod handlers;
mod process;
mod runtime;
mod text;

use clock::refresh_clock_status;
use handlers::{
    __path_clock_check, __path_clock_status, __path_clock_sync, __path_cloudflared_cancel,
    __path_cloudflared_delete, __path_cloudflared_download, __path_cloudflared_status,
    __path_dnsmasq_install, __path_dnsmasq_status, __path_frp_cancel, __path_frp_delete,
    __path_frp_download, __path_frp_status, clock_check, clock_status, clock_sync,
    cloudflared_cancel, cloudflared_delete, cloudflared_download, cloudflared_status,
    dnsmasq_install, dnsmasq_status, frp_cancel, frp_delete, frp_download, frp_status,
};

pub(crate) use dnsmasq::{
    activate_dnsmasq_service, build_dnsmasq_status_with_translator, deactivate_dnsmasq_service,
};

#[cfg(test)]
use clock::{
    clock_sync_target_epoch_ms, format_beijing_time, format_drift, initial_clock_status,
    network_latency_compensation_ms, preserve_clock_sync_metadata_from,
};
#[cfg(test)]
use dnsmasq::{
    DnsmasqServiceKind, dnsmasq_bootstrap_config, dnsmasq_detected_message,
    dnsmasq_install_state_to_json, dnsmasq_ready_message, dnsmasq_service_commands,
    dnsmasq_service_kind_for, dnsmasq_state, normalize_dnsmasq_error,
    resolve_dnsmasq_install_state, run_dnsmasq_service_commands_with,
};
#[cfg(test)]
use downloads::{detect_frp_platform, frp_binary_path, localize_asset_progress_error};
#[cfg(test)]
use handlers::cloudflared_delete_unsupported_message;
#[cfg(test)]
use process::summarize_process_output;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use text::{dnsmasq_text, tunnel_manager_text, tunnel_manager_text_params};

const EXPECTED_TIME_ZONE: &str = "Asia/Shanghai";
const TIME_DRIFT_THRESHOLD_MS: i64 = 90_000;
const SMART_CONNECT_LOCAL_TTL_SECONDS: u16 = 30;
const SMART_CONNECT_MANAGED_CONF_PATH: &str = "/etc/dnsmasq.d/fn-knock-smart-connect.conf";
const CLOUDFLARED_MIRROR_BASE: &str = "https://cor.fnknock.cn/alldata/cloudflared";
const FRP_MIRROR_BASE: &str = "https://cor.fnknock.cn/alldata/frp";
const DOWNLOAD_CANCELLED_ERROR: &str = "Download cancelled";
const DOWNLOAD_CONNECTION_FAILED_ERROR: &str = "Download connection failed";
const DOWNLOAD_CONNECTION_TIMED_OUT_PREFIX: &str = "Download connection timed out";
const DOWNLOAD_RESPONSE_TIMED_OUT_PREFIX: &str = "Download response timed out";
const DOWNLOAD_RESPONSE_BODY_UNREADABLE_ERROR: &str = "Download response body is unreadable";
const DOWNLOAD_TIMED_OUT_PREFIX: &str = "Download timed out";
const FRP_DOWNLOAD_FAILED_PREFIX: &str = "FRP download failed: ";
const UNKNOWN_DOWNLOAD_ERROR: &str = "Unknown error";
const NETWORK_TIME_SOURCES: [(&str, &str); 6] = [
    ("Baidu HTTPS", "https://www.baidu.com/"),
    ("QQ HTTPS", "https://www.qq.com/"),
    ("Aliyun HTTPS", "https://www.aliyun.com/"),
    ("Baidu HTTP", "http://www.baidu.com/"),
    ("QQ HTTP", "http://www.qq.com/"),
    ("Aliyun HTTP", "http://www.aliyun.com/"),
];

static ASSET_DOWNLOADS: OnceLock<Mutex<AssetDownloads>> = OnceLock::new();
static DNSMASQ_INSTALL: OnceLock<Mutex<DnsmasqInstallState>> = OnceLock::new();
static CLOCK_STATUS: OnceLock<Mutex<Option<Value>>> = OnceLock::new();

#[derive(Clone)]
struct DownloadProgress {
    status: String,
    percent: i64,
    error: Option<String>,
    cancel_requested: bool,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            percent: 0,
            error: None,
            cancel_requested: false,
        }
    }
}

#[derive(Default)]
struct AssetDownloads {
    cloudflared: DownloadProgress,
    frp: DownloadProgress,
}

#[derive(Clone)]
struct DnsmasqInstallState {
    status: String,
    progress: i64,
    message: String,
}

impl Default for DnsmasqInstallState {
    fn default() -> Self {
        Self {
            status: "uninstalled".to_string(),
            progress: 0,
            message: String::new(),
        }
    }
}

pub fn system_asset_routes() -> Router<AppState> {
    let system_clock_routes: Router<AppState> = system_clock_routes().into();
    let system_binary_asset_routes: Router<AppState> = system_binary_asset_routes().into();
    Router::new()
        .merge(system_clock_routes)
        .merge(system_binary_asset_routes)
}

pub(crate) fn system_binary_asset_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(cloudflared_status))
        .routes(routes!(cloudflared_download))
        .routes(routes!(cloudflared_cancel))
        .routes(routes!(cloudflared_delete))
        .routes(routes!(frp_status))
        .routes(routes!(frp_download))
        .routes(routes!(frp_cancel))
        .routes(routes!(frp_delete))
}

pub(crate) fn dnsmasq_asset_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(dnsmasq_status))
        .routes(routes!(dnsmasq_install))
}

/// Clock routes are registered once for both Axum and OpenAPI so their
/// availability and localized error behavior cannot drift from the contract.
pub(crate) fn system_clock_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(clock_status))
        .routes(routes!(clock_check))
        .routes(routes!(clock_sync))
}

pub fn smart_connect_asset_routes() -> Router<AppState> {
    dnsmasq_asset_routes().into()
}

pub fn start_system_clock_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("system-clock-sync", async move {
        let translator = Translator::from_state(&task_state).await;
        let _ = refresh_clock_status(&task_state, &translator).await;
    });
}

#[cfg(test)]
mod tests;
