use std::sync::{Mutex, OnceLock};

use axum::{
    Router,
    routing::{get, post},
};
use serde_json::Value;

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
    clock_check, clock_status, clock_sync, cloudflared_cancel, cloudflared_delete,
    cloudflared_download, cloudflared_status, dnsmasq_install, dnsmasq_status, frp_cancel,
    frp_delete, frp_download, frp_status,
};

pub(crate) use dnsmasq::build_dnsmasq_status_with_translator;

#[cfg(test)]
use clock::{
    clock_sync_target_epoch_ms, format_beijing_time, format_drift, initial_clock_status,
    network_latency_compensation_ms, preserve_clock_sync_metadata_from,
};
#[cfg(test)]
use dnsmasq::{
    dnsmasq_bootstrap_config, dnsmasq_detected_message, dnsmasq_install_state_to_json,
    dnsmasq_ready_message, dnsmasq_state, normalize_dnsmasq_error, resolve_dnsmasq_install_state,
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

const FRP_VERSION: &str = "0.67.0";
const EXPECTED_TIME_ZONE: &str = "Asia/Shanghai";
const TIME_DRIFT_THRESHOLD_MS: i64 = 90_000;
const SMART_CONNECT_LOCAL_TTL_SECONDS: u16 = 30;
const SMART_CONNECT_MANAGED_CONF_PATH: &str = "/etc/dnsmasq.d/fn-knock-smart-connect.conf";
const CLOUDFLARED_MIRROR_BASE: &str = "https://cor.fnknock.cn/alldata/cloudflared";
const FRP_MIRROR_BASE: &str = "https://cor.fnknock.cn/alldata/frp";
const FRP_GITHUB_BASE: &str = "https://github.com/fatedier/frp/releases/download/v0.67.0";
const DOWNLOAD_CANCELLED_ERROR: &str = "Download cancelled";
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
    Router::new()
        .route("/api/admin/system/clock/status", get(clock_status))
        .route("/api/admin/system/clock/check", post(clock_check))
        .route("/api/admin/system/clock/sync", post(clock_sync))
        .route(
            "/api/admin/system/cloudflared/status",
            get(cloudflared_status),
        )
        .route(
            "/api/admin/system/cloudflared/download",
            post(cloudflared_download),
        )
        .route(
            "/api/admin/system/cloudflared/cancel",
            post(cloudflared_cancel),
        )
        .route(
            "/api/admin/system/cloudflared",
            axum::routing::delete(cloudflared_delete),
        )
        .route("/api/admin/system/frp/status", get(frp_status))
        .route("/api/admin/system/frp/download", post(frp_download))
        .route("/api/admin/system/frp/cancel", post(frp_cancel))
        .route("/api/admin/system/frp", axum::routing::delete(frp_delete))
        .route("/api/admin/system/dnsmasq/status", get(dnsmasq_status))
        .route("/api/admin/system/dnsmasq/install", post(dnsmasq_install))
}

pub fn start_system_clock_tasks(state: AppState) {
    tokio::spawn(async move {
        let translator = Translator::from_state(&state).await;
        let _ = refresh_clock_status(&state, &translator).await;
    });
}

#[cfg(test)]
mod tests;
