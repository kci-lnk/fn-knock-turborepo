use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use ::time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc2822};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use tokio::time::{self, MissedTickBehavior};

use crate::{i18n::Translator, response, runtime_profile, state::AppState, time_utils};

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
        let mut ticker = time::interval(std::time::Duration::from_secs(10 * 60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let translator = Translator::from_state(&state).await;
            let _ = refresh_clock_status(&state, &translator).await;
        }
    });
}

async fn clock_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(cached_clock_status(&state, &translator).await).into_response()
}

async fn clock_check(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(refresh_clock_status(&state, &translator).await).into_response()
}

async fn clock_sync(State(state): State<AppState>) -> Response {
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

async fn cloudflared_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_cloudflared_status(
        &state.settings.data_dir,
        &translator,
    ))
    .into_response()
}

async fn cloudflared_download(State(state): State<AppState>) -> Response {
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

async fn cloudflared_cancel(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    request_cancel("cloudflared");
    response::success_message(tunnel_manager_text(
        &translator,
        "cloudflared",
        "downloadCancelled",
    ))
    .into_response()
}

async fn cloudflared_delete(State(state): State<AppState>) -> Response {
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

fn cloudflared_delete_unsupported_message(
    translator: &Translator,
    platform: &str,
) -> Option<String> {
    (platform == "darwin")
        .then(|| tunnel_manager_text(translator, "cloudflared", "macManualRemove"))
}

async fn frp_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_frp_status(&state.settings.data_dir, &translator)).into_response()
}

async fn frp_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if start_download("frp") {
        tokio::spawn(download_frp(state));
    }
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadStarted"))
        .into_response()
}

async fn frp_cancel(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    request_cancel("frp");
    response::success_message(tunnel_manager_text(&translator, "frp", "downloadCancelled"))
        .into_response()
}

async fn frp_delete(State(state): State<AppState>) -> Response {
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

async fn dnsmasq_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(build_dnsmasq_status_with_translator(&translator)).into_response()
}

async fn dnsmasq_install(State(state): State<AppState>) -> Response {
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

async fn cached_clock_status(state: &AppState, translator: &Translator) -> Value {
    if let Some(status) = clock_status_lock().lock().unwrap().clone() {
        return localize_clock_status(status, translator);
    }
    build_clock_status(state, false, translator).await
}

async fn refresh_clock_status(state: &AppState, translator: &Translator) -> Value {
    let status = build_clock_status(state, true, translator).await;
    *clock_status_lock().lock().unwrap() = Some(status.clone());
    status
}

async fn build_clock_status(state: &AppState, checked: bool, translator: &Translator) -> Value {
    let system_time_ms = time_utils::now_ms();
    let system_time_zone = detect_system_timezone();
    let timezone_mismatch = system_time_zone.as_deref() != Some(EXPECTED_TIME_ZONE);
    let remote = if checked {
        fetch_network_time(state, translator).await
    } else {
        Ok(None)
    };
    let (network_source, remote_time_ms, last_check_error) = match remote {
        Ok(Some(remote)) => (Some(remote.source), Some(remote.epoch_ms), None),
        Ok(None) => (None, None, None),
        Err(error) => (None, None, Some(error)),
    };
    let drift_ms = remote_time_ms.map(|remote| system_time_ms - remote);
    let time_mismatch = drift_ms.is_some_and(|value| value.abs() > TIME_DRIFT_THRESHOLD_MS);
    let mut status = json!({
        "expectedTimeZone": EXPECTED_TIME_ZONE,
        "systemTimeZone": system_time_zone,
        "checkedAt": if checked { Value::String(time_utils::now_iso()) } else { Value::Null },
        "networkSource": network_source,
        "hasRemoteTime": remote_time_ms.is_some(),
        "lastCheckError": last_check_error,
        "systemTimeMs": system_time_ms,
        "remoteTimeMs": remote_time_ms,
        "systemBeijingTime": format_beijing_time(system_time_ms, translator.locale()),
        "remoteBeijingTime": remote_time_ms.and_then(|value| {
            format_beijing_time(value, translator.locale())
        }),
        "driftMs": drift_ms,
        "driftThresholdMs": TIME_DRIFT_THRESHOLD_MS,
        "timeMismatch": time_mismatch,
        "timezoneMismatch": timezone_mismatch,
        "needsAttention": timezone_mismatch || time_mismatch,
        "issues": [],
        "checking": false,
        "syncInProgress": false,
        "lastSyncAt": Value::Null,
        "lastSyncError": Value::Null,
        "syncSummary": Value::Null
    });
    preserve_clock_sync_metadata(&mut status);
    localize_clock_status(status, translator)
}

fn localize_clock_status(mut status: Value, translator: &Translator) -> Value {
    let timezone_mismatch = status
        .get("timezoneMismatch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let time_mismatch = status
        .get("timeMismatch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let system_time_zone = status
        .get("systemTimeZone")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| translator.t("server.systemClock.unknown"));
    let drift_ms = status.get("driftMs").and_then(Value::as_i64);

    let mut issues = Vec::new();
    if timezone_mismatch {
        issues.push(json!({
            "code": "timezone_mismatch",
            "title": translator.t("server.systemClock.issues.timezone.title"),
            "message": translator.t_params(
                "server.systemClock.issues.timezone.message",
                &[
                    ("timezone", system_time_zone),
                    ("expected", EXPECTED_TIME_ZONE.to_string())
                ]
            )
        }));
    }
    if time_mismatch && let Some(drift_ms) = drift_ms {
        issues.push(json!({
            "code": "time_mismatch",
            "title": translator.t("server.systemClock.issues.timeMismatch.title"),
            "message": translator.t_params(
                "server.systemClock.issues.timeMismatch.message",
                &[("drift", format_drift(drift_ms, translator))]
            )
        }));
    }

    if let Some(object) = status.as_object_mut() {
        object.insert("issues".to_string(), Value::Array(issues));
    }
    status
}

fn format_drift(drift_ms: i64, translator: &Translator) -> String {
    let total_seconds = ((drift_ms.abs() + 500) / 1000).max(1);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes <= 0 {
        return translator.t_params(
            "server.systemClock.duration.seconds",
            &[("seconds", seconds.to_string())],
        );
    }
    if seconds == 0 {
        return translator.t_params(
            "server.systemClock.duration.minutes",
            &[("minutes", minutes.to_string())],
        );
    }
    translator.t_params(
        "server.systemClock.duration.minutesSeconds",
        &[
            ("minutes", minutes.to_string()),
            ("seconds", seconds.to_string()),
        ],
    )
}

fn build_cloudflared_status(data_dir: &Path, translator: &Translator) -> Value {
    let platform = detect_cloudflared_platform();
    let bin_path = data_dir.join("cloudflared").join("cloudflared");
    let downloaded = if platform == "darwin" {
        command_succeeds("which", &["cloudflared"])
    } else {
        bin_path.exists()
    };
    json!({
        "supported": platform != "unsupported",
        "platform": platform,
        "downloaded": downloaded,
        "progress": progress_json("cloudflared", translator)
    })
}

fn build_frp_status(data_dir: &Path, translator: &Translator) -> Value {
    let platform = detect_frp_platform();
    let downloaded = frp_binary_path(data_dir, &platform, "frpc").is_some_and(|path| path.exists())
        || frp_binary_path(data_dir, &platform, "frps").is_some_and(|path| path.exists());
    json!({
        "supported": platform != "unsupported",
        "platform": platform,
        "downloaded": downloaded,
        "progress": progress_json("frp", translator)
    })
}

async fn download_cloudflared(state: AppState) {
    let result = async {
        let platform = detect_cloudflared_platform();
        if platform == "darwin" {
            return Err("Cloudflared auto download is not supported on macOS".to_string());
        }
        let url = match platform {
            "linux-amd64" => format!("{CLOUDFLARED_MIRROR_BASE}/cloudflared-linux-amd64"),
            "linux-arm64" => format!("{CLOUDFLARED_MIRROR_BASE}/cloudflared-linux-arm64"),
            "linux-arm" => format!("{CLOUDFLARED_MIRROR_BASE}/cloudflared-linux-arm"),
            _ => return Err("Cloudflared platform is unsupported".to_string()),
        };
        let dir = state.settings.data_dir.join("cloudflared");
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let target = dir.join("cloudflared");
        let temp = dir.join("cloudflared.tmp");
        download_to_file(&state, "cloudflared", &url, &temp).await?;
        if is_cancel_requested("cloudflared") {
            let _ = fs::remove_file(&temp);
            return Err(DOWNLOAD_CANCELLED_ERROR.to_string());
        }
        fs::rename(&temp, &target).map_err(|error| error.to_string())?;
        chmod_executable(&target);
        Ok(())
    }
    .await;
    finish_download("cloudflared", result);
}

async fn download_frp(state: AppState) {
    let result = async {
        let platform = detect_frp_platform();
        let archive = frp_archive_name(platform).ok_or("FRP platform is unsupported")?;
        let candidates = [
            format!("{FRP_MIRROR_BASE}/{archive}.tar.gz"),
            format!("{FRP_GITHUB_BASE}/{archive}.tar.gz"),
        ];
        let frp_dir = state.settings.data_dir.join("frp");
        fs::create_dir_all(&frp_dir).map_err(|error| error.to_string())?;
        let target = frp_dir.join("frp.tar.gz");
        let temp = frp_dir.join("frp.tar.gz.tmp");
        let mut last_error: Option<String> = None;
        let mut succeeded = false;
        for url in candidates {
            reset_download_file(&temp);
            match download_to_file(&state, "frp", &url, &temp).await {
                Ok(()) => {
                    succeeded = true;
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    if is_cancel_requested("frp") {
                        break;
                    }
                }
            }
        }
        if !succeeded {
            let _ = fs::remove_file(&temp);
            let detail = last_error.unwrap_or_else(|| UNKNOWN_DOWNLOAD_ERROR.to_string());
            return Err(format!("{FRP_DOWNLOAD_FAILED_PREFIX}{detail}"));
        }
        if is_cancel_requested("frp") {
            let _ = fs::remove_file(&temp);
            return Err(DOWNLOAD_CANCELLED_ERROR.to_string());
        }
        fs::rename(&temp, &target).map_err(|error| error.to_string())?;
        if let Some(extracted) = frp_extracted_dir(&state.settings.data_dir, platform)
            && extracted.exists()
        {
            let _ = fs::remove_dir_all(extracted);
        }
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&target)
            .arg("-C")
            .arg(&frp_dir)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!(
                "FRP package extraction failed with code {}",
                status.code().unwrap_or_default()
            ));
        }
        if let Some(frpc) = frp_binary_path(&state.settings.data_dir, platform, "frpc") {
            chmod_executable(&frpc);
        }
        if let Some(frps) = frp_binary_path(&state.settings.data_dir, platform, "frps") {
            chmod_executable(&frps);
        }
        Ok(())
    }
    .await;
    finish_download("frp", result);
}

async fn download_to_file(
    state: &AppState,
    asset: &str,
    url: &str,
    path: &Path,
) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
    let mut response = state
        .fallback_client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut loaded = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if is_cancel_requested(asset) {
            return Err(DOWNLOAD_CANCELLED_ERROR.to_string());
        }
        file.write_all(&chunk).map_err(|error| error.to_string())?;
        loaded += chunk.len() as u64;
        if total > 0 {
            let percent = ((loaded * 100) / total).min(100) as i64;
            set_progress(asset, "downloading", percent, None);
        }
    }
    file.flush().map_err(|error| error.to_string())
}

struct NetworkTimeResult {
    epoch_ms: i64,
    source: String,
}

async fn fetch_network_time(
    state: &AppState,
    translator: &Translator,
) -> Result<Option<NetworkTimeResult>, String> {
    let mut last_error = translator.t("server.systemClock.networkTimeUnavailable");
    for (label, url) in NETWORK_TIME_SOURCES {
        match fetch_network_time_from_source(state, translator, label, url).await {
            Ok(result) => return Ok(Some(result)),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

async fn fetch_network_time_from_source(
    state: &AppState,
    translator: &Translator,
    label: &str,
    url: &str,
) -> Result<NetworkTimeResult, String> {
    let started = Instant::now();
    let mut date_header = state
        .fallback_client
        .head(url)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::PRAGMA, "no-cache")
        .send()
        .await
        .ok()
        .and_then(|response| {
            response
                .headers()
                .get(header::DATE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        });

    if date_header.is_none() {
        date_header = state
            .fallback_client
            .get(url)
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::PRAGMA, "no-cache")
            .send()
            .await
            .map_err(|_| {
                translator.t_params(
                    "server.systemClock.sourceFetchFailed",
                    &[("source", label.to_string())],
                )
            })?
            .headers()
            .get(header::DATE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
    }

    let date_header = date_header.ok_or_else(|| {
        translator.t_params(
            "server.systemClock.missingDateHeader",
            &[("source", label.to_string())],
        )
    })?;
    let parsed = OffsetDateTime::parse(&date_header, &Rfc2822).map_err(|_| {
        translator.t_params(
            "server.systemClock.invalidDateHeader",
            &[("source", label.to_string())],
        )
    })?;
    let latency_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    Ok(NetworkTimeResult {
        epoch_ms: parsed.unix_timestamp() * 1000 + network_latency_compensation_ms(latency_ms),
        source: label.to_string(),
    })
}

fn network_latency_compensation_ms(latency_ms: i64) -> i64 {
    latency_ms.max(0).saturating_add(1) / 2
}

async fn sync_system_clock(
    state: &AppState,
    translator: &Translator,
) -> Result<(String, Value), String> {
    set_clock_sync_in_progress();
    match sync_system_clock_inner(state, translator).await {
        Ok(result) => Ok(result),
        Err(error) => {
            set_clock_sync_error(error.clone());
            Err(error)
        }
    }
}

async fn sync_system_clock_inner(
    state: &AppState,
    translator: &Translator,
) -> Result<(String, Value), String> {
    let before = build_clock_status(state, true, translator).await;
    let mut actions = Vec::new();

    if before.get("systemTimeZone").and_then(Value::as_str) != Some(EXPECTED_TIME_ZONE) {
        actions.push(set_system_timezone(translator)?);
    }

    if let Some(remote_time_ms) = before.get("remoteTimeMs").and_then(Value::as_i64) {
        let checked_at_ms = before
            .get("checkedAt")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or_else(time_utils::now_ms);
        actions.push(set_system_clock(
            clock_sync_target_epoch_ms(remote_time_ms, checked_at_ms, time_utils::now_ms()),
            translator,
        )?);
    }

    if let Some(message) = enable_network_time_sync(translator) {
        actions.push(message);
    }

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let mut next = build_clock_status(state, true, translator).await;
    let message = if actions.is_empty() {
        translator.t("server.systemClock.statusRefreshed")
    } else {
        actions.join(&translator.t("server.systemClock.actionSeparator"))
    };
    next["syncInProgress"] = Value::Bool(false);
    next["lastSyncAt"] = json!(time_utils::now_iso());
    next["lastSyncError"] = Value::Null;
    next["syncSummary"] = json!(message.clone());
    *clock_status_lock().lock().unwrap() = Some(next.clone());
    Ok((message, next))
}

fn preserve_clock_sync_metadata(status: &mut Value) {
    let previous = clock_status_lock().lock().unwrap().clone();
    preserve_clock_sync_metadata_from(status, previous.as_ref());
}

fn preserve_clock_sync_metadata_from(status: &mut Value, previous: Option<&Value>) {
    let Some(previous) = previous else {
        return;
    };
    let Some(object) = status.as_object_mut() else {
        return;
    };
    for key in [
        "syncInProgress",
        "lastSyncAt",
        "lastSyncError",
        "syncSummary",
    ] {
        if let Some(value) = previous.get(key) {
            object.insert(key.to_string(), value.clone());
        }
    }
}

fn set_clock_sync_in_progress() {
    update_cached_clock_sync_metadata(|status| {
        status["syncInProgress"] = Value::Bool(true);
        status["lastSyncError"] = Value::Null;
    });
}

fn set_clock_sync_error(message: String) {
    update_cached_clock_sync_metadata(|status| {
        status["syncInProgress"] = Value::Bool(false);
        status["lastSyncAt"] = json!(time_utils::now_iso());
        status["lastSyncError"] = json!(message);
        status["syncSummary"] = Value::Null;
    });
}

fn update_cached_clock_sync_metadata(update: impl FnOnce(&mut Value)) {
    let mut guard = clock_status_lock().lock().unwrap();
    let mut status = guard.take().unwrap_or_else(initial_clock_status);
    update(&mut status);
    *guard = Some(status);
}

fn initial_clock_status() -> Value {
    json!({
        "expectedTimeZone": EXPECTED_TIME_ZONE,
        "systemTimeZone": Value::Null,
        "checkedAt": Value::Null,
        "networkSource": Value::Null,
        "hasRemoteTime": false,
        "lastCheckError": Value::Null,
        "systemTimeMs": Value::Null,
        "remoteTimeMs": Value::Null,
        "systemBeijingTime": Value::Null,
        "remoteBeijingTime": Value::Null,
        "driftMs": Value::Null,
        "driftThresholdMs": TIME_DRIFT_THRESHOLD_MS,
        "timeMismatch": false,
        "timezoneMismatch": false,
        "needsAttention": false,
        "issues": [],
        "checking": false,
        "syncInProgress": false,
        "lastSyncAt": Value::Null,
        "lastSyncError": Value::Null,
        "syncSummary": Value::Null
    })
}

fn clock_sync_target_epoch_ms(remote_time_ms: i64, checked_at_ms: i64, now_ms: i64) -> i64 {
    remote_time_ms + (now_ms - checked_at_ms).max(0)
}

fn set_system_timezone(translator: &Translator) -> Result<String, String> {
    if run_process_success("timedatectl", &["set-timezone", EXPECTED_TIME_ZONE]).is_ok() {
        return Ok(translator.t_params(
            "server.systemClock.timezoneSet",
            &[("timezone", EXPECTED_TIME_ZONE.to_string())],
        ));
    }

    let zoneinfo_path = format!("/usr/share/zoneinfo/{EXPECTED_TIME_ZONE}");
    if !Path::new(&zoneinfo_path).exists() {
        return Err(translator.t_params(
            "server.systemClock.missingZoneinfoFile",
            &[("path", zoneinfo_path)],
        ));
    }
    let _ = fs::remove_file("/etc/localtime");
    match std::os::unix::fs::symlink(&zoneinfo_path, "/etc/localtime") {
        Ok(()) => {}
        Err(_) => fs::copy(&zoneinfo_path, "/etc/localtime")
            .map(|_| ())
            .map_err(|error| error.to_string())?,
    }
    fs::write("/etc/timezone", format!("{EXPECTED_TIME_ZONE}\n"))
        .map_err(|error| error.to_string())?;
    Ok(translator.t_params(
        "server.systemClock.timezoneWritten",
        &[("timezone", EXPECTED_TIME_ZONE.to_string())],
    ))
}

fn set_system_clock(target_epoch_ms: i64, translator: &Translator) -> Result<String, String> {
    let target_seconds = target_epoch_ms / 1000;
    run_process_success("date", &["-u", "-s", &format!("@{target_seconds}")])?;
    let _ = run_process_success("hwclock", &["--systohc"]);
    Ok(translator.t("server.systemClock.clockAdjusted"))
}

fn enable_network_time_sync(translator: &Translator) -> Option<String> {
    let mut actions = Vec::new();
    if run_process_success("timedatectl", &["set-ntp", "true"]).is_ok() {
        actions.push(translator.t("server.systemClock.ntpEnabled"));
    }
    for service in ["systemd-timesyncd", "chrony", "chronyd", "ntp"] {
        if run_process_success("systemctl", &["restart", service]).is_ok() {
            actions.push(translator.t_params(
                "server.systemClock.serviceRestarted",
                &[("service", service.to_string())],
            ));
            break;
        }
    }
    (!actions.is_empty()).then(|| actions.join(&translator.t("server.systemClock.listSeparator")))
}

pub(crate) fn build_dnsmasq_status_with_translator(translator: &Translator) -> Value {
    let current = dnsmasq_install_state();
    let executable = detect_dnsmasq_executable();
    let raw_service_active = dnsmasq_service_active();
    let service_active = if executable.is_none() && current.status != "installing" {
        false
    } else {
        raw_service_active
    };
    let initialized = current.status != "installing"
        && executable
            .as_ref()
            .is_some_and(|(path, _)| dnsmasq_can_initialize(path));
    let has_service_definition =
        current.status != "installing" && executable.is_some() && has_service_definition();
    let version = executable.as_ref().map(|(_, version)| version.as_str());
    let install_state = resolve_dnsmasq_install_state(
        translator,
        version,
        service_active,
        initialized,
        has_service_definition,
        current,
    );
    json!({
        "installed": executable.is_some(),
        "service_active": service_active,
        "initialized": initialized,
        "version": executable.map(|(_, version)| version).unwrap_or_default(),
        "install_state": dnsmasq_install_state_to_json(&install_state, translator)
    })
}

fn dnsmasq_state(status: &str, progress: i64, message: String) -> DnsmasqInstallState {
    DnsmasqInstallState {
        status: status.to_string(),
        progress,
        message,
    }
}

fn dnsmasq_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.dnsmasq.{key}"))
}

fn dnsmasq_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.dnsmasq.{key}"), params)
}

fn tunnel_manager_text(translator: &Translator, manager: &str, key: &str) -> String {
    translator.t(&format!("server.tunnelManagers.{manager}.{key}"))
}

fn tunnel_manager_text_params(
    translator: &Translator,
    manager: &str,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.tunnelManagers.{manager}.{key}"), params)
}

fn dnsmasq_ready_message(translator: &Translator, version: &str) -> String {
    if version.trim().is_empty() {
        dnsmasq_text(translator, "ready")
    } else {
        dnsmasq_text_params(
            translator,
            "readyWithVersion",
            &[("version", version.to_string())],
        )
    }
}

fn dnsmasq_detected_message(
    translator: &Translator,
    version: &str,
    has_service_definition: bool,
) -> String {
    if !has_service_definition {
        dnsmasq_text(translator, "missingServiceAutoComplete")
    } else if version.trim().is_empty() {
        dnsmasq_text(translator, "detected")
    } else {
        dnsmasq_text_params(
            translator,
            "detectedWithVersion",
            &[("version", version.to_string())],
        )
    }
}

fn resolve_dnsmasq_install_state(
    translator: &Translator,
    executable_version: Option<&str>,
    service_active: bool,
    initialized: bool,
    has_service_definition: bool,
    current: DnsmasqInstallState,
) -> DnsmasqInstallState {
    if current.status == "installing" {
        return current;
    }
    let Some(version) = executable_version else {
        return if current.status == "error" {
            current
        } else {
            dnsmasq_state(
                "uninstalled",
                0,
                dnsmasq_text(translator, "notDetectedInstallFirst"),
            )
        };
    };
    if service_active && initialized {
        return dnsmasq_state("installed", 100, dnsmasq_ready_message(translator, version));
    }
    if current.status == "error" {
        return current;
    }
    dnsmasq_state(
        "installed",
        100,
        dnsmasq_detected_message(translator, version, has_service_definition),
    )
}

fn format_beijing_time(epoch_ms: i64, locale: &str) -> Option<String> {
    let seconds = epoch_ms.div_euclid(1000);
    let offset = UtcOffset::from_hms(8, 0, 0).ok()?;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .map(|value| value.to_offset(offset))
        .map(|value| {
            let year = value.year();
            let month = u8::from(value.month());
            let day = value.day();
            let hour = value.hour();
            let minute = value.minute();
            let second = value.second();
            match locale {
                "en" => {
                    format!("{month:02}/{day:02}/{year:04}, {hour:02}:{minute:02}:{second:02}")
                }
                "ko-KR" => {
                    format!("{year:04}. {month:02}. {day:02}. {hour:02}:{minute:02}:{second:02}")
                }
                "zh-Hant" => {
                    format!(
                        "{year:04}/{month:02}/{day:02}\u{2009}{hour:02}:{minute:02}:{second:02}"
                    )
                }
                _ => format!("{year:04}/{month:02}/{day:02} {hour:02}:{minute:02}:{second:02}"),
            }
        })
}

fn detect_system_timezone() -> Option<String> {
    if let Ok(value) = std::env::var("TZ")
        && !value.trim().is_empty()
    {
        return Some(value.trim().to_string());
    }
    if let Ok(value) = fs::read_to_string("/etc/timezone") {
        let timezone = value.trim();
        if !timezone.is_empty() {
            return Some(timezone.to_string());
        }
    }
    if let Ok(target) = fs::read_link("/etc/localtime")
        && let Some(text) = target.to_str()
        && let Some((_, zone)) = text.split_once("zoneinfo/")
        && !zone.trim().is_empty()
    {
        return Some(zone.trim().to_string());
    }
    None
}

fn host_runtime_available(state: &AppState) -> bool {
    deployment_target(state) != "docker" && std::env::consts::OS == "linux" && is_running_as_root()
}

fn system_clock_unavailable_message(state: &AppState, translator: &Translator) -> String {
    if deployment_target(state) == "docker" {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.docker")
    } else if std::env::consts::OS != "linux" {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.platform")
    } else {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.permission")
    }
}

fn smart_connect_unavailable_message(state: &AppState, translator: &Translator) -> String {
    if deployment_target(state) == "docker" {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.docker")
    } else if std::env::consts::OS != "linux" {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.platform")
    } else {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.permission")
    }
}

fn clock_status_lock() -> &'static Mutex<Option<Value>> {
    CLOCK_STATUS.get_or_init(|| Mutex::new(None))
}

fn deployment_target(state: &AppState) -> String {
    runtime_profile::deployment_target(state)
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() == 0 }
}

#[cfg(not(unix))]
fn is_running_as_root() -> bool {
    false
}

fn detect_cloudflared_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => "darwin",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

fn detect_frp_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

fn frp_binary_path(data_dir: &Path, platform: &str, binary: &str) -> Option<PathBuf> {
    frp_archive_name(platform)
        .map(|archive_name| data_dir.join("frp").join(archive_name).join(binary))
}

fn detect_dnsmasq_executable() -> Option<(String, String)> {
    for candidate in ["dnsmasq", "/usr/sbin/dnsmasq", "/usr/bin/dnsmasq"] {
        let Ok(output) = Command::new(candidate).arg("--version").output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("dnsmasq")
            .to_string();
        return Some((candidate.to_string(), version));
    }
    None
}

fn dnsmasq_service_active() -> bool {
    if has_systemd_unit()
        && run_process_success("systemctl", &["is-active", "--quiet", "dnsmasq"]).is_ok()
    {
        return true;
    }
    has_init_script() && run_process_success("service", &["dnsmasq", "status"]).is_ok()
}

fn dnsmasq_can_initialize(executable_path: &str) -> bool {
    if fs::create_dir_all(
        Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
            .parent()
            .unwrap_or_else(|| Path::new("/etc/dnsmasq.d")),
    )
    .is_err()
    {
        return false;
    }
    let test_path = Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
        .parent()
        .unwrap_or_else(|| Path::new("/etc/dnsmasq.d"))
        .join(format!(".fn-knock-write-test-{}", time_utils::now_ms()));
    if fs::write(&test_path, "").is_err() {
        return false;
    }
    let _ = fs::remove_file(test_path);
    validate_dnsmasq_config(executable_path, &dnsmasq_bootstrap_config()).is_ok()
}

fn install_dnsmasq_background(already_installed: bool, translator: Translator) {
    let result = if already_installed {
        initialize_dnsmasq(&translator)
    } else {
        install_dnsmasq_package(&translator)
    };
    if let Err(error) = result {
        set_dnsmasq_install_state("error", 0, error);
    }
}

fn install_dnsmasq_package(translator: &Translator) -> Result<(), String> {
    set_dnsmasq_install_state("installing", 15, dnsmasq_text(translator, "refreshingApt"));
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["update"],
        "aptUpdateFailed",
    )?;

    set_dnsmasq_install_state("installing", 55, dnsmasq_text(translator, "installing"));
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["install", "-y", "dnsmasq"],
        "aptInstallFailed",
    )?;

    initialize_dnsmasq(translator)
}

fn initialize_dnsmasq(translator: &Translator) -> Result<(), String> {
    set_dnsmasq_install_state(
        "installing",
        20,
        dnsmasq_text(translator, "checkingEnvironment"),
    );
    let executable = detect_dnsmasq_executable()
        .ok_or_else(|| dnsmasq_text(translator, "notDetectedInstallFirst"))?;

    set_dnsmasq_install_state(
        "installing",
        45,
        dnsmasq_text(translator, "validatingConfig"),
    );
    ensure_dnsmasq_service_package_installed(translator)?;
    fs::create_dir_all(
        Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
            .parent()
            .unwrap_or_else(|| Path::new("/etc/dnsmasq.d")),
    )
    .map_err(|error| error.to_string())?;
    validate_dnsmasq_config(&executable.0, &dnsmasq_bootstrap_config())
        .map_err(|error| normalize_dnsmasq_error(translator, &error, "configTestFailed"))?;

    set_dnsmasq_install_state(
        "installing",
        72,
        dnsmasq_text(translator, "enablingService"),
    );
    enable_dnsmasq_on_boot();

    set_dnsmasq_install_state(
        "installing",
        90,
        dnsmasq_text(translator, "startingService"),
    );
    restart_dnsmasq_service(translator)?;

    set_dnsmasq_install_state(
        "installed",
        100,
        dnsmasq_ready_message(translator, &executable.1),
    );
    Ok(())
}

fn ensure_dnsmasq_service_package_installed(translator: &Translator) -> Result<(), String> {
    if has_service_definition() {
        return Ok(());
    }
    if !Path::new("/usr/bin/apt-get").exists() {
        return Err(dnsmasq_text(translator, "servicePackageMissing"));
    }
    set_dnsmasq_install_state(
        "installing",
        58,
        dnsmasq_text(translator, "completingService"),
    );
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["install", "-y", "dnsmasq"],
        "completeServiceFailed",
    )?;
    if !has_service_definition() {
        return Err(dnsmasq_text(
            translator,
            "serviceDefinitionMissingAfterInstall",
        ));
    }
    Ok(())
}

fn dnsmasq_bootstrap_config() -> String {
    [
        format!("local-ttl={SMART_CONNECT_LOCAL_TTL_SECONDS}"),
        "listen-address=127.0.0.1".to_string(),
        "bind-interfaces".to_string(),
        String::new(),
    ]
    .join("\n")
}

fn validate_dnsmasq_config(executable_path: &str, content: &str) -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-dnsmasq-{}", time_utils::now_ms()));
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    let temp_conf_path = temp_dir.join("dnsmasq.conf");
    let result = (|| {
        fs::write(&temp_conf_path, content).map_err(|error| error.to_string())?;
        run_process_success(
            executable_path,
            &[
                "--test",
                &format!("--conf-file={}", temp_conf_path.display()),
            ],
        )
    })();
    let _ = fs::remove_dir_all(temp_dir);
    result.map(|_| ())
}

fn restart_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    let mut errors = Vec::new();
    if has_systemd_unit() {
        match run_dnsmasq_process_success(
            translator,
            "systemctl",
            &["restart", "dnsmasq"],
            "restartFailed",
        ) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    if has_init_script() {
        match run_dnsmasq_process_success(
            translator,
            "service",
            &["dnsmasq", "restart"],
            "restartFailed",
        ) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Err(dnsmasq_text(translator, "serviceDefinitionMissing"))
    } else {
        Err(errors.join(" | "))
    }
}

fn enable_dnsmasq_on_boot() {
    if has_systemd_unit() {
        let _ = run_process_success("systemctl", &["enable", "dnsmasq"]);
        return;
    }
    if has_init_script() {
        let _ = run_process_success("update-rc.d", &["dnsmasq", "defaults"]);
    }
}

fn has_service_definition() -> bool {
    has_systemd_unit() || has_init_script()
}

fn has_systemd_unit() -> bool {
    [
        "/etc/systemd/system/dnsmasq.service",
        "/lib/systemd/system/dnsmasq.service",
        "/usr/lib/systemd/system/dnsmasq.service",
    ]
    .iter()
    .any(|path| Path::new(path).exists())
}

fn has_init_script() -> bool {
    Path::new("/etc/init.d/dnsmasq").exists()
}

fn dnsmasq_install_state() -> DnsmasqInstallState {
    dnsmasq_install_state_lock()
        .lock()
        .expect("dnsmasq install mutex poisoned")
        .clone()
}

fn dnsmasq_install_state_json(translator: &Translator) -> Value {
    dnsmasq_install_state_to_json(&dnsmasq_install_state(), translator)
}

fn dnsmasq_install_state_to_json(state: &DnsmasqInstallState, translator: &Translator) -> Value {
    json!({
        "status": state.status,
        "progress": state.progress,
        "message": localize_dnsmasq_install_message(state, translator)
    })
}

fn localize_dnsmasq_install_message(
    state: &DnsmasqInstallState,
    translator: &Translator,
) -> String {
    let message = state.message.trim();
    if state.status == "uninstalled"
        && (message.is_empty()
            || message == "dnsmasq is not detected"
            || message == "dnsmasq was not detected. Install it first.")
    {
        return dnsmasq_text(translator, "notDetectedInstallFirst");
    }
    state.message.clone()
}

fn set_dnsmasq_install_state(status: impl Into<String>, progress: i64, message: impl Into<String>) {
    let mut guard = dnsmasq_install_state_lock()
        .lock()
        .expect("dnsmasq install mutex poisoned");
    guard.status = status.into();
    guard.progress = progress.clamp(0, 100);
    guard.message = message.into();
}

fn dnsmasq_install_state_lock() -> &'static Mutex<DnsmasqInstallState> {
    DNSMASQ_INSTALL.get_or_init(|| Mutex::new(DnsmasqInstallState::default()))
}

fn run_dnsmasq_process_success(
    translator: &Translator,
    command: &str,
    args: &[&str],
    fallback_key: &str,
) -> Result<(), String> {
    run_process_success(command, args)
        .map_err(|error| normalize_dnsmasq_error(translator, &error, fallback_key))
}

fn normalize_dnsmasq_error(translator: &Translator, message: &str, fallback_key: &str) -> String {
    let detail = message.trim();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("failed to create listening socket")
        || lower.contains("failed to bind listening socket")
        || lower.contains("permission denied")
    {
        return if detail.is_empty() {
            dnsmasq_text(translator, "dnsPortUnavailable")
        } else {
            dnsmasq_text_params(
                translator,
                "dnsPortUnavailableWithDetail",
                &[("detail", detail.to_string())],
            )
        };
    }
    if detail.is_empty() {
        dnsmasq_text(translator, fallback_key)
    } else {
        detail.to_string()
    }
}

fn run_process_success(command: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let detail = summarize_process_output(&output.stdout, &output.stderr);
    Err(if detail.is_empty() {
        format!("{command} failed")
    } else {
        detail
    })
}

fn summarize_process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let detail = format!(
        "{}\n{}",
        String::from_utf8_lossy(stderr),
        String::from_utf8_lossy(stdout)
    );
    detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(500)
        .collect()
}

fn command_succeeds(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn downloads() -> &'static Mutex<AssetDownloads> {
    ASSET_DOWNLOADS.get_or_init(|| Mutex::new(AssetDownloads::default()))
}

fn asset_progress_mut<'a>(
    downloads: &'a mut AssetDownloads,
    asset: &str,
) -> &'a mut DownloadProgress {
    match asset {
        "frp" => &mut downloads.frp,
        _ => &mut downloads.cloudflared,
    }
}

fn progress_json(asset: &str, translator: &Translator) -> Value {
    let guard = downloads().lock().expect("asset downloads mutex poisoned");
    let progress = match asset {
        "frp" => &guard.frp,
        _ => &guard.cloudflared,
    };
    json!({
        "status": progress.status,
        "percent": progress.percent,
        "error": progress.error.as_deref().map(|error| {
            localize_asset_progress_error(translator, asset, error)
        })
    })
}

fn localize_asset_progress_error(translator: &Translator, asset: &str, error: &str) -> String {
    match (asset, error) {
        (_, DOWNLOAD_CANCELLED_ERROR) => {
            tunnel_manager_text(translator, asset, "downloadCancelled")
        }
        ("cloudflared", "Cloudflared auto download is not supported on macOS") => {
            tunnel_manager_text(translator, "cloudflared", "macAutoDownloadUnsupported")
        }
        ("cloudflared", "Cloudflared platform is unsupported")
        | ("frp", "FRP platform is unsupported") => {
            tunnel_manager_text(translator, asset, "platformUnsupported")
        }
        _ => {
            if asset == "frp"
                && let Some(detail) = error.strip_prefix(FRP_DOWNLOAD_FAILED_PREFIX)
            {
                let detail = if detail == UNKNOWN_DOWNLOAD_ERROR || detail == "Download failed" {
                    tunnel_manager_text(translator, "frp", "unknownError")
                } else {
                    detail.to_string()
                };
                return tunnel_manager_text_params(
                    translator,
                    "frp",
                    "downloadFailed",
                    &[("detail", detail)],
                );
            }
            if asset == "frp" && error == "Download failed" {
                return tunnel_manager_text_params(
                    translator,
                    "frp",
                    "downloadFailed",
                    &[(
                        "detail",
                        tunnel_manager_text(translator, "frp", "unknownError"),
                    )],
                );
            }
            if let Some(code) = error.strip_prefix("FRP package extraction failed with code ") {
                return tunnel_manager_text_params(
                    translator,
                    "frp",
                    "extractFailed",
                    &[("code", code.to_string())],
                );
            }
            error.to_string()
        }
    }
}

fn start_download(asset: &str) -> bool {
    let mut guard = downloads().lock().expect("asset downloads mutex poisoned");
    let progress = asset_progress_mut(&mut guard, asset);
    if progress.status == "downloading" {
        return false;
    }
    progress.status = "downloading".to_string();
    progress.percent = 0;
    progress.error = None;
    progress.cancel_requested = false;
    true
}

fn request_cancel(asset: &str) {
    let mut guard = downloads().lock().expect("asset downloads mutex poisoned");
    let progress = asset_progress_mut(&mut guard, asset);
    if progress.status == "downloading" {
        progress.cancel_requested = true;
        progress.error = None;
    } else {
        progress.status = "idle".to_string();
        progress.percent = 0;
        progress.error = None;
        progress.cancel_requested = false;
    }
}

fn reset_progress(asset: &str) {
    let mut guard = downloads().lock().expect("asset downloads mutex poisoned");
    *asset_progress_mut(&mut guard, asset) = DownloadProgress::default();
}

fn set_progress(asset: &str, status: &str, percent: i64, error: Option<String>) {
    let mut guard = downloads().lock().expect("asset downloads mutex poisoned");
    let progress = asset_progress_mut(&mut guard, asset);
    progress.status = status.to_string();
    progress.percent = percent.clamp(0, 100);
    progress.error = error;
}

fn finish_download(asset: &str, result: Result<(), String>) {
    let mut guard = downloads().lock().expect("asset downloads mutex poisoned");
    let progress = asset_progress_mut(&mut guard, asset);
    match result {
        Ok(()) => {
            progress.status = "completed".to_string();
            progress.percent = 100;
            progress.error = None;
            progress.cancel_requested = false;
        }
        Err(error) if error.to_ascii_lowercase().contains("cancel") => {
            progress.status = "idle".to_string();
            progress.percent = 0;
            progress.error = Some(DOWNLOAD_CANCELLED_ERROR.to_string());
            progress.cancel_requested = false;
        }
        Err(error) => {
            progress.status = "error".to_string();
            progress.percent = 0;
            progress.error = Some(error);
            progress.cancel_requested = false;
        }
    }
}

fn is_cancel_requested(asset: &str) -> bool {
    let guard = downloads().lock().expect("asset downloads mutex poisoned");
    match asset {
        "frp" => guard.frp.cancel_requested,
        _ => guard.cloudflared.cancel_requested,
    }
}

fn reset_download_file(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(unix)]
fn chmod_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn chmod_executable(_path: &Path) {}

fn frp_archive_name(platform: &str) -> Option<String> {
    match platform {
        "linux-amd64" => Some(format!("frp_{FRP_VERSION}_linux_amd64")),
        "linux-arm64" => Some(format!("frp_{FRP_VERSION}_linux_arm64")),
        "linux-arm" => Some(format!("frp_{FRP_VERSION}_linux_arm")),
        "darwin-arm64" => Some(format!("frp_{FRP_VERSION}_darwin_arm64")),
        _ => None,
    }
}

fn frp_extracted_dir(data_dir: &Path, platform: &str) -> Option<PathBuf> {
    frp_archive_name(platform).map(|archive_name| data_dir.join("frp").join(archive_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_frp_platform_names_like_node() {
        let platform = detect_frp_platform();
        assert!(
            [
                "darwin-arm64",
                "linux-amd64",
                "linux-arm64",
                "linux-arm",
                "unsupported"
            ]
            .contains(&platform)
        );
    }

    #[test]
    fn builds_frp_binary_path_for_supported_platforms() {
        let path = frp_binary_path(Path::new("/tmp/data"), "linux-amd64", "frpc").unwrap();
        assert!(path.ends_with("frp/frp_0.67.0_linux_amd64/frpc"));
        assert!(frp_binary_path(Path::new("/tmp/data"), "unsupported", "frpc").is_none());
    }

    #[test]
    fn builds_dnsmasq_bootstrap_config_like_node() {
        let config = dnsmasq_bootstrap_config();
        assert!(config.contains("local-ttl=30"));
        assert!(config.contains("listen-address=127.0.0.1"));
        assert!(config.contains("bind-interfaces"));
        assert!(config.ends_with('\n'));
    }

    #[test]
    fn localizes_system_asset_and_dnsmasq_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            tunnel_manager_text(&zh, "cloudflared", "downloadStarted"),
            "已开始下载 Cloudflared"
        );
        assert_eq!(
            cloudflared_delete_unsupported_message(&zh, "darwin").as_deref(),
            Some("MAC 平台请手动移除 cloudflared")
        );
        assert_eq!(
            cloudflared_delete_unsupported_message(&zh, "linux-amd64"),
            None
        );
        assert_eq!(
            tunnel_manager_text_params(
                &zh,
                "frp",
                "deleteFailed",
                &[("detail", "权限不足".to_string())]
            ),
            "删除 FRP 失败：权限不足"
        );
        assert_eq!(dnsmasq_ready_message(&zh, "2.90"), "dnsmasq 已就绪：2.90");
        assert_eq!(
            dnsmasq_detected_message(&zh, "2.90", true),
            "dnsmasq 已检测到：2.90，等待初始化或启动服务"
        );
        assert_eq!(
            dnsmasq_detected_message(&zh, "2.90", false),
            "缺少系统服务，初始化时会自动补全"
        );
        assert_eq!(
            dnsmasq_install_state_to_json(&DnsmasqInstallState::default(), &zh)["message"],
            "未检测到 dnsmasq，请先完成安装"
        );
        assert_eq!(
            normalize_dnsmasq_error(
                &zh,
                "failed to create listening socket for port 53: Address already in use",
                "restartFailed",
            ),
            "DNS 53 端口不可用，请先释放端口后重试：failed to create listening socket for port 53: Address already in use"
        );

        let en = Translator::new("en");
        assert_eq!(
            tunnel_manager_text(&en, "frp", "deleteSuccess"),
            "FRP deleted"
        );
        assert_eq!(
            dnsmasq_text(&en, "checkingEnvironment"),
            "Checking dnsmasq environment..."
        );
    }

    #[test]
    fn resolves_dnsmasq_install_state_like_node() {
        let zh = Translator::new("zh-CN");

        let installing = resolve_dnsmasq_install_state(
            &zh,
            Some("2.90"),
            true,
            true,
            true,
            dnsmasq_state("installing", 42, "installing now".to_string()),
        );
        assert_eq!(installing.status, "installing");
        assert_eq!(installing.progress, 42);
        assert_eq!(installing.message, "installing now");

        let previous_error = dnsmasq_state("error", 0, "bind failed".to_string());
        let preserved_error = resolve_dnsmasq_install_state(
            &zh,
            Some("2.90"),
            false,
            false,
            true,
            previous_error.clone(),
        );
        assert_eq!(preserved_error.status, "error");
        assert_eq!(preserved_error.message, "bind failed");

        let ready_overrides_error =
            resolve_dnsmasq_install_state(&zh, Some("2.90"), true, true, true, previous_error);
        assert_eq!(ready_overrides_error.status, "installed");
        assert_eq!(ready_overrides_error.message, "dnsmasq 已就绪：2.90");

        let missing_service = resolve_dnsmasq_install_state(
            &zh,
            Some("2.90"),
            false,
            false,
            false,
            DnsmasqInstallState::default(),
        );
        assert_eq!(missing_service.status, "installed");
        assert_eq!(missing_service.message, "缺少系统服务，初始化时会自动补全");
    }

    #[test]
    fn localizes_asset_progress_errors() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            localize_asset_progress_error(&zh, "frp", "Download cancelled"),
            "下载已取消"
        );
        assert_eq!(
            localize_asset_progress_error(
                &zh,
                "cloudflared",
                "Cloudflared platform is unsupported"
            ),
            "当前平台不受支持"
        );
        assert_eq!(
            localize_asset_progress_error(&zh, "frp", "FRP package extraction failed with code 2"),
            "解压失败，退出码 2"
        );
        assert_eq!(
            localize_asset_progress_error(&zh, "frp", "FRP download failed: HTTP 503"),
            "下载失败：HTTP 503"
        );
        assert_eq!(
            localize_asset_progress_error(&zh, "frp", "Download failed"),
            "下载失败：未知错误"
        );
    }

    #[test]
    fn preserves_clock_sync_metadata_across_status_refresh() {
        let previous = json!({
            "syncInProgress": true,
            "lastSyncAt": "2026-07-07T01:02:03Z",
            "lastSyncError": "boom",
            "syncSummary": "done"
        });
        let mut status = initial_clock_status();

        preserve_clock_sync_metadata_from(&mut status, Some(&previous));

        assert_eq!(status["syncInProgress"], true);
        assert_eq!(status["lastSyncAt"], "2026-07-07T01:02:03Z");
        assert_eq!(status["lastSyncError"], "boom");
        assert_eq!(status["syncSummary"], "done");
    }

    #[test]
    fn calculates_clock_sync_target_like_node() {
        assert_eq!(clock_sync_target_epoch_ms(10_000, 1_000, 3_500), 12_500);
        assert_eq!(clock_sync_target_epoch_ms(10_000, 3_500, 1_000), 10_000);
    }

    #[test]
    fn rounds_network_latency_compensation_like_node() {
        assert_eq!(network_latency_compensation_ms(0), 0);
        assert_eq!(network_latency_compensation_ms(1), 1);
        assert_eq!(network_latency_compensation_ms(2), 1);
        assert_eq!(network_latency_compensation_ms(3), 2);
    }

    #[test]
    fn formats_drift_with_node_rounding() {
        let zh = Translator::new("zh-CN");
        assert_eq!(format_drift(90_100, &zh), "1 分 30 秒");
        assert_eq!(format_drift(90_500, &zh), "1 分 31 秒");
    }

    #[test]
    fn summarizes_process_output_tail_like_node() {
        let summary = summarize_process_output(
            b"stdout-1\nstdout-2\n",
            b"stderr-1\nstderr-2\nstderr-3\nstderr-4\nstderr-5\nstderr-6\nstderr-7\nstderr-8\nstderr-9\n",
        );
        assert!(!summary.contains("stderr-1"));
        assert!(summary.contains("stderr-9"));
        assert!(summary.contains("stdout-2"));
    }

    #[test]
    fn formats_epoch_as_localized_beijing_time_like_node() {
        assert_eq!(
            format_beijing_time(0, "zh-CN").as_deref(),
            Some("1970/01/01 08:00:00")
        );
        assert_eq!(
            format_beijing_time(0, "en").as_deref(),
            Some("01/01/1970, 08:00:00")
        );
        assert_eq!(
            format_beijing_time(0, "zh-Hant").as_deref(),
            Some("1970/01/01\u{2009}08:00:00")
        );
        assert_eq!(
            format_beijing_time(0, "ko-KR").as_deref(),
            Some("1970. 01. 01. 08:00:00")
        );
    }
}
