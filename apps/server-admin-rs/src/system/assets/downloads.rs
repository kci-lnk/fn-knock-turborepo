use std::{
    fs,
    io::Write,
    path::Path,
    process::Command,
    sync::{Mutex, MutexGuard},
};

const MAX_ASSET_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

use serde_json::{Value, json};

use crate::{frp_utils, fs_utils::chmod_executable, i18n::Translator, state::AppState};

pub(super) use crate::frp_utils::{
    detect_frp_platform, frp_archive_name, frp_binary_path, frp_extracted_dir,
};

use super::{
    ASSET_DOWNLOADS, AssetDownloads, CLOUDFLARED_MIRROR_BASE, DOWNLOAD_CANCELLED_ERROR,
    DOWNLOAD_CONNECTION_FAILED_ERROR, DOWNLOAD_CONNECTION_TIMED_OUT_PREFIX,
    DOWNLOAD_RESPONSE_BODY_UNREADABLE_ERROR, DOWNLOAD_RESPONSE_TIMED_OUT_PREFIX,
    DOWNLOAD_TIMED_OUT_PREFIX, DownloadProgress, FRP_DOWNLOAD_FAILED_PREFIX, FRP_MIRROR_BASE,
    UNKNOWN_DOWNLOAD_ERROR,
    text::{tunnel_manager_text, tunnel_manager_text_params},
};

pub(super) fn build_cloudflared_status(data_dir: &Path, translator: &Translator) -> Value {
    let platform = detect_cloudflared_platform();
    let bin_path = data_dir.join("cloudflared").join("cloudflared");
    let downloaded = bin_path.exists();
    json!({
        "supported": platform != "unsupported",
        "platform": platform,
        "downloaded": downloaded,
        "progress": progress_json("cloudflared", translator)
    })
}

pub(super) fn build_frp_status(data_dir: &Path, translator: &Translator) -> Value {
    let platform = detect_frp_platform();
    let downloaded = frp_binary_path(data_dir, platform, "frpc").is_some_and(|path| path.exists())
        || frp_binary_path(data_dir, platform, "frps").is_some_and(|path| path.exists());
    json!({
        "supported": platform != "unsupported",
        "platform": platform,
        "downloaded": downloaded,
        "progress": progress_json("frp", translator)
    })
}

pub(super) async fn download_cloudflared(state: AppState) {
    let result = async {
        let platform = detect_cloudflared_platform();
        let url = match platform {
            "darwin-amd64" => format!("{CLOUDFLARED_MIRROR_BASE}/cloudflared-darwin-amd64"),
            "darwin-arm64" => format!("{CLOUDFLARED_MIRROR_BASE}/cloudflared-darwin-arm64"),
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
        if let Err(error) = validate_downloaded_architecture(&temp, platform, "Cloudflared") {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        fs::rename(&temp, &target).map_err(|error| error.to_string())?;
        chmod_executable(&target);
        Ok(())
    }
    .await;
    finish_download("cloudflared", result);
}

pub(super) async fn download_frp(state: AppState) {
    let result = async {
        let platform = detect_frp_platform();
        let archive = frp_archive_name(platform).ok_or("FRP platform is unsupported")?;
        let candidates = [
            format!("{FRP_MIRROR_BASE}/{archive}.tar.gz"),
            frp_utils::frp_github_archive_url(&archive),
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
                Ok(()) => match validate_frp_archive(&temp, &archive) {
                    Ok(()) => {
                        succeeded = true;
                        break;
                    }
                    Err(error) => last_error = Some(error),
                },
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
        install_frp_archive_transactionally(&temp, &target, &frp_dir, platform, &archive)?;
        Ok(())
    }
    .await;
    finish_download("frp", result);
}

pub(super) async fn download_to_file(
    state: &AppState,
    asset: &str,
    url: &str,
    path: &Path,
) -> Result<(), String> {
    let result = tokio::time::timeout(
        state.settings.asset_download_total_timeout,
        download_to_file_inner(state, asset, url, path),
    )
    .await
    .unwrap_or_else(|_| {
        Err(format_download_total_timeout_error(
            state.settings.asset_download_total_timeout,
        ))
    });
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

async fn download_to_file_inner(
    state: &AppState,
    asset: &str,
    url: &str,
    path: &Path,
) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
    let mut response = state
        .asset_download_client
        .get(url)
        .send()
        .await
        .map_err(|error| format_download_request_error(error, &state.settings))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    if total > MAX_ASSET_DOWNLOAD_BYTES {
        return Err(format!(
            "Download exceeds {} byte safety limit",
            MAX_ASSET_DOWNLOAD_BYTES
        ));
    }
    let mut loaded = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format_download_request_error(error, &state.settings))?
    {
        if is_cancel_requested(asset) {
            return Err(DOWNLOAD_CANCELLED_ERROR.to_string());
        }
        loaded += chunk.len() as u64;
        if loaded > MAX_ASSET_DOWNLOAD_BYTES {
            return Err(format!(
                "Download exceeds {} byte safety limit",
                MAX_ASSET_DOWNLOAD_BYTES
            ));
        }
        file.write_all(&chunk).map_err(|error| error.to_string())?;
        if let Some(percent) = loaded
            .checked_mul(100)
            .and_then(|value| value.checked_div(total))
        {
            let percent = percent.min(100) as i64;
            set_progress(asset, "downloading", percent, None);
        }
    }
    file.flush().map_err(|error| error.to_string())
}

pub(super) fn format_download_request_error(
    error: reqwest::Error,
    settings: &crate::settings::Settings,
) -> String {
    if error.is_connect() {
        if error.is_timeout() {
            return format!(
                "{DOWNLOAD_CONNECTION_TIMED_OUT_PREFIX} after {}s",
                timeout_seconds(settings.asset_download_connect_timeout)
            );
        }
        return DOWNLOAD_CONNECTION_FAILED_ERROR.to_string();
    }
    if error.is_timeout() {
        return format!(
            "{DOWNLOAD_RESPONSE_TIMED_OUT_PREFIX} after {}s without receiving data",
            timeout_seconds(settings.asset_download_read_timeout)
        );
    }
    if error.is_body() || error.is_decode() {
        return DOWNLOAD_RESPONSE_BODY_UNREADABLE_ERROR.to_string();
    }
    error.without_url().to_string()
}

fn format_download_total_timeout_error(total_timeout: std::time::Duration) -> String {
    format!(
        "{DOWNLOAD_TIMED_OUT_PREFIX} after {}s total",
        timeout_seconds(total_timeout)
    )
}

fn timeout_seconds(duration: std::time::Duration) -> u64 {
    duration.as_secs().max(1)
}

pub(super) fn detect_cloudflared_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => "darwin-amd64",
        ("macos", "aarch64") => "darwin-arm64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

fn validate_downloaded_architecture(
    path: &Path,
    platform: &str,
    label: &str,
) -> Result<(), String> {
    if !platform.starts_with("darwin-") {
        return Ok(());
    }
    let file_command = if cfg!(target_os = "macos") {
        "/usr/bin/file"
    } else {
        "file"
    };
    let output = Command::new(file_command)
        .arg("-b")
        .arg(path)
        .output()
        .map_err(|error| format!("{label} architecture validation failed: {error}"))?;
    if !output.status.success() {
        return Err(format!("{label} architecture validation failed"));
    }
    let description = String::from_utf8_lossy(&output.stdout);
    if downloaded_architecture_matches(platform, &description) {
        Ok(())
    } else {
        Err(format!(
            "{label} architecture mismatch for {platform}: {}",
            description.trim()
        ))
    }
}

pub(super) fn downloaded_architecture_matches(platform: &str, description: &str) -> bool {
    let description = description.trim();
    match platform {
        "darwin-amd64" => description.starts_with("Mach-O 64-bit executable x86_64"),
        "darwin-arm64" => description.starts_with("Mach-O 64-bit executable arm64"),
        _ => false,
    }
}

pub(super) fn frp_archive_entry_path_is_safe(expected_root: &str, entry: &str) -> bool {
    let entry = entry.strip_suffix('/').unwrap_or(entry);
    if entry.is_empty()
        || entry.starts_with('/')
        || entry.chars().any(|character| character.is_control())
    {
        return false;
    }
    let mut parts = entry.split('/');
    if parts.next() != Some(expected_root) {
        return false;
    }
    parts.all(|part| !part.is_empty() && part != "." && part != "..")
}

pub(super) fn frp_archive_entry_type_is_safe(verbose_line: &str) -> bool {
    matches!(verbose_line.as_bytes().first(), Some(b'-' | b'd'))
}

fn validate_frp_archive(path: &Path, expected_root: &str) -> Result<(), String> {
    let list = Command::new("tar")
        .arg("-tzf")
        .arg(path)
        .output()
        .map_err(|error| format!("FRP package validation failed: {error}"))?;
    if !list.status.success() {
        return Err(format!(
            "FRP package validation failed with code {}",
            list.status.code().unwrap_or_default()
        ));
    }
    let entries = String::from_utf8(list.stdout)
        .map_err(|_| "FRP package contains non-UTF-8 paths".to_string())?;
    let mut found_frpc = false;
    let mut found_frps = false;
    for entry in entries.lines() {
        if !frp_archive_entry_path_is_safe(expected_root, entry) {
            return Err(format!("FRP package contains unsafe path: {entry}"));
        }
        let normalized = entry.strip_suffix('/').unwrap_or(entry);
        found_frpc |= normalized == format!("{expected_root}/frpc");
        found_frps |= normalized == format!("{expected_root}/frps");
    }
    if !found_frpc || !found_frps {
        return Err("FRP package is missing frpc or frps".to_string());
    }

    let verbose = Command::new("tar")
        .arg("-tvzf")
        .arg(path)
        .output()
        .map_err(|error| format!("FRP package validation failed: {error}"))?;
    if !verbose.status.success() {
        return Err(format!(
            "FRP package validation failed with code {}",
            verbose.status.code().unwrap_or_default()
        ));
    }
    let verbose = String::from_utf8(verbose.stdout)
        .map_err(|_| "FRP package contains invalid metadata".to_string())?;
    if verbose
        .lines()
        .any(|line| !frp_archive_entry_type_is_safe(line))
    {
        return Err("FRP package contains links or special files".to_string());
    }
    Ok(())
}

fn install_frp_archive_transactionally(
    downloaded_archive: &Path,
    archive_target: &Path,
    frp_dir: &Path,
    platform: &str,
    archive_name: &str,
) -> Result<(), String> {
    validate_frp_archive(downloaded_archive, archive_name)?;
    let staging_dir = frp_dir.join(".extract.tmp");
    let staged_root = staging_dir.join(archive_name);
    let final_root = frp_dir.join(archive_name);
    let backup_root = frp_dir.join(".previous.tmp");
    reset_directory(&staging_dir)?;

    let result = (|| {
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(downloaded_archive)
            .arg("-C")
            .arg(&staging_dir)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!(
                "FRP package extraction failed with code {}",
                status.code().unwrap_or_default()
            ));
        }

        for binary_name in ["frpc", "frps"] {
            let binary = staged_root.join(binary_name);
            if !binary.is_file() {
                return Err(format!("FRP package is missing {binary_name}"));
            }
            chmod_executable(&binary);
            validate_downloaded_architecture(&binary, platform, binary_name)?;
        }

        fs::rename(downloaded_archive, archive_target).map_err(|error| error.to_string())?;
        reset_path(&backup_root)?;
        let had_previous = final_root.exists();
        if had_previous {
            fs::rename(&final_root, &backup_root).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&staged_root, &final_root) {
            if had_previous {
                fs::rename(&backup_root, &final_root).map_err(|restore_error| {
                    format!(
                        "FRP install failed ({error}); previous version restore failed: {restore_error}"
                    )
                })?;
            }
            return Err(error.to_string());
        }
        reset_path(&backup_root)?;
        Ok(())
    })();

    let _ = fs::remove_dir_all(&staging_dir);
    result
}

fn reset_directory(path: &Path) -> Result<(), String> {
    reset_path(path)?;
    fs::create_dir_all(path).map_err(|error| error.to_string())
}

fn reset_path(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    } else if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn downloads() -> &'static Mutex<AssetDownloads> {
    ASSET_DOWNLOADS.get_or_init(|| Mutex::new(AssetDownloads::default()))
}

fn downloads_guard() -> MutexGuard<'static, AssetDownloads> {
    downloads()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

pub(super) fn asset_progress_mut<'a>(
    downloads: &'a mut AssetDownloads,
    asset: &str,
) -> &'a mut DownloadProgress {
    match asset {
        "frp" => &mut downloads.frp,
        _ => &mut downloads.cloudflared,
    }
}

pub(super) fn progress_json(asset: &str, translator: &Translator) -> Value {
    let guard = downloads_guard();
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

pub(super) fn localize_asset_progress_error(
    translator: &Translator,
    asset: &str,
    error: &str,
) -> String {
    match (asset, error) {
        (_, DOWNLOAD_CANCELLED_ERROR) => {
            tunnel_manager_text(translator, asset, "downloadCancelled")
        }
        ("cloudflared", "Cloudflared platform is unsupported")
        | ("frp", "FRP platform is unsupported") => {
            tunnel_manager_text(translator, asset, "platformUnsupported")
        }
        _ => {
            if asset == "frp"
                && let Some(detail) = error.strip_prefix(FRP_DOWNLOAD_FAILED_PREFIX)
            {
                let detail = localize_download_error_detail(translator, asset, detail);
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
                        localize_download_error_detail(translator, asset, error),
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
            localize_download_error_detail(translator, asset, error)
        }
    }
}

fn localize_download_error_detail(translator: &Translator, asset: &str, detail: &str) -> String {
    if detail == UNKNOWN_DOWNLOAD_ERROR || detail == "Download failed" {
        return tunnel_manager_text(translator, asset, "unknownError");
    }
    if detail == DOWNLOAD_CONNECTION_FAILED_ERROR {
        return translator.t("admin.connectionTest.failed");
    }
    if detail == DOWNLOAD_RESPONSE_BODY_UNREADABLE_ERROR {
        return tunnel_manager_text(translator, asset, "responseBodyUnreadable");
    }
    if detail.starts_with(DOWNLOAD_CONNECTION_TIMED_OUT_PREFIX)
        || detail.starts_with(DOWNLOAD_RESPONSE_TIMED_OUT_PREFIX)
        || detail.starts_with(DOWNLOAD_TIMED_OUT_PREFIX)
    {
        return translator.t("admin.connectionTest.timeout");
    }
    detail.to_string()
}

pub(super) fn start_download(asset: &str) -> bool {
    let mut guard = downloads_guard();
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

pub(super) fn request_cancel(asset: &str) {
    let mut guard = downloads_guard();
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

pub(super) fn reset_progress(asset: &str) {
    let mut guard = downloads_guard();
    *asset_progress_mut(&mut guard, asset) = DownloadProgress::default();
}

pub(super) fn set_progress(asset: &str, status: &str, percent: i64, error: Option<String>) {
    let mut guard = downloads_guard();
    let progress = asset_progress_mut(&mut guard, asset);
    progress.status = status.to_string();
    progress.percent = percent.clamp(0, 100);
    progress.error = error;
}

pub(super) fn finish_download(asset: &str, result: Result<(), String>) {
    let mut guard = downloads_guard();
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

pub(super) fn is_cancel_requested(asset: &str) -> bool {
    let guard = downloads_guard();
    match asset {
        "frp" => guard.frp.cancel_requested,
        _ => guard.cloudflared.cancel_requested,
    }
}

pub(super) fn reset_download_file(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
