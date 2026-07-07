use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::time;

use crate::{
    app_version::{APP_GITHUB_URL, APP_LOCAL_VERSION},
    i18n::Translator,
    response, runtime_profile,
    state::AppState,
    system_events, time_utils,
};

const OTA_LATEST_URL: &str = "https://cor.fnknock.cn/latest.json";
const UPDATE_PENDING_KEY: &str = "fn_knock:update:pending";
const UPDATE_CONFIRM_KEY: &str = "fn_knock:update:confirm";
const UPDATE_PENDING_TTL_SECONDS: usize = 7 * 24 * 60 * 60;
const UPDATE_CONFIRM_TTL_SECONDS: usize = 7 * 24 * 60 * 60;
const UPDATE_CHECK_TIMEOUT_MS: u64 = 8_000;
const UPDATE_DOWNLOAD_TIMEOUT_MS: u64 = 300_000;

static UPDATE_MANAGER: OnceLock<UpdateManager> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OtaManifest {
    version: String,
    update_available: bool,
    force_update: bool,
    download_url: String,
    sha256: String,
    download_url_arm64: String,
    sha256_arm64: String,
    release_notes: String,
}

#[derive(Clone, Debug)]
struct ResolvedPackage {
    architecture: &'static str,
    download_url: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct DownloadState {
    status: String,
    percent: i64,
    #[serde(rename = "downloadedBytes")]
    downloaded_bytes: i64,
    #[serde(rename = "totalBytes")]
    total_bytes: Option<i64>,
    error: Option<String>,
    #[serde(rename = "targetVersion")]
    target_version: Option<String>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            percent: 0,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            target_version: None,
        }
    }
}

#[derive(Default)]
struct UpdateInner {
    latest_manifest: Option<OtaManifest>,
    update_enabled: bool,
    has_update: bool,
    force_update: bool,
    last_checked_at: Option<i64>,
    check_error: Option<String>,
    check_in_progress: bool,
    download_in_progress: bool,
    downloaded_path: Option<PathBuf>,
    downloaded_sha256: Option<String>,
    confirmed_pending_on_boot: bool,
    download: DownloadState,
}

struct UpdateManager {
    updates_dir: PathBuf,
    package_download_dir: PathBuf,
    install_log_path: PathBuf,
    install_env_path: PathBuf,
    inner: Mutex<UpdateInner>,
}

pub fn update_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/update/status", get(status))
        .route("/api/admin/update/check", post(check))
        .route("/api/admin/update/download", post(download))
        .route("/api/admin/update/install", post(install))
        .route(
            "/api/admin/update/check-and-download",
            post(check_and_download),
        )
        .route("/api/admin/update/confirm", get(confirm))
}

fn update_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.updateRoutes.{key}"))
}

fn update_manager_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.updateManager.{key}"))
}

fn update_manager_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.updateManager.{key}"), params)
}

pub fn start_update_tasks(state: AppState) {
    let update_manager = manager(&state);
    update_manager.ensure_dirs();
    tokio::spawn(async move {
        let update_manager = manager(&state);
        if let Err(error) = update_manager.ensure_confirm_by_pending(&state).await {
            tracing::warn!(%error, "failed to prepare update confirmation on boot");
        }
        if let Err(error) = update_manager.check_now(state.clone(), "startup").await {
            tracing::warn!(%error, "startup update check failed");
        }

        let mut interval = time::interval(update_check_interval());
        interval.tick().await;
        loop {
            interval.tick().await;
            match state
                .redis
                .set_lock_if_not_exists("ota-update-check", 600)
                .await
            {
                Ok(true) => {
                    if let Err(error) = manager(&state).check_now(state.clone(), "cron").await {
                        tracing::warn!(%error, "scheduled update check failed");
                    }
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "failed to acquire update check lock"),
            }
        }
    });
}

async fn status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    match manager.status(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load update status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                update_route_text(&translator, "loadStatusFailed"),
            )
        }
    }
}

async fn check(State(state): State<AppState>) -> Response {
    let manager = manager(&state);
    if let Err(error) = manager.check_now(state.clone(), "manual").await {
        tracing::warn!(%error, "manual update check failed");
    }
    status(State(state)).await
}

async fn download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !self_update_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            self_update_unavailable_message(&state, &translator),
        );
    }
    let manager = manager(&state);
    match manager.trigger_download(state.clone()).await {
        Ok(()) => match manager.status(&state).await {
            Ok(data) => Json(json!({
                "success": true,
                "message": update_route_text(&translator, "downloadStarted"),
                "data": data
            }))
            .into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to load status after update download start");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    update_route_text(&translator, "loadStatusFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

async fn install(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !self_update_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            self_update_unavailable_message(&state, &translator),
        );
    }
    let manager = manager(&state);
    match manager.trigger_install(&state).await {
        Ok(()) => response::success_message(update_route_text(&translator, "installStarted"))
            .into_response(),
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

async fn check_and_download(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !self_update_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            self_update_unavailable_message(&state, &translator),
        );
    }
    let manager = manager(&state);
    if let Err(error) = manager
        .check_now(state.clone(), "manual-check-and-download")
        .await
    {
        tracing::warn!(%error, "check-and-download update check failed");
    }
    match manager.trigger_download(state.clone()).await {
        Ok(()) => match manager.status(&state).await {
            Ok(data) => Json(json!({
                "success": true,
                "message": update_route_text(&translator, "checkAndDownloadStarted"),
                "data": data
            }))
            .into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to load status after check-and-download");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    update_route_text(&translator, "loadStatusFailed"),
                )
            }
        },
        Err(message) => response::error(StatusCode::BAD_REQUEST, message),
    }
}

async fn confirm(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let manager = manager(&state);
    match manager.consume_confirm_message(&state).await {
        Ok(data) => response::ok(data.unwrap_or(Value::Null)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to consume update confirm message");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                update_route_text(&translator, "loadConfirmationFailed"),
            )
        }
    }
}

impl UpdateManager {
    fn new(data_dir: &Path) -> Self {
        let updates_dir = data_dir.join("updates");
        let package_download_dir = PathBuf::from("/tmp/fn-knock-updates");
        let install_log_path = updates_dir.join("install.log");
        let install_env_path = updates_dir.join("install.env");
        Self {
            updates_dir,
            package_download_dir,
            install_log_path,
            install_env_path,
            inner: Mutex::new(UpdateInner::default()),
        }
    }

    fn ensure_dirs(&self) {
        let _ = fs::create_dir_all(&self.updates_dir);
        let _ = fs::create_dir_all(&self.package_download_dir);
    }

    async fn status(&self, state: &AppState) -> anyhow::Result<Value> {
        self.ensure_confirm_by_pending(state).await?;
        let inner = self.inner.lock().unwrap();
        Ok(json!({
            "githubUrl": APP_GITHUB_URL,
            "localVersion": APP_LOCAL_VERSION,
            "latest": inner.latest_manifest,
            "updateEnabled": inner.update_enabled,
            "hasUpdate": inner.has_update,
            "forceUpdate": inner.force_update,
            "check": {
                "lastCheckedAt": inner.last_checked_at,
                "error": inner.check_error,
            },
            "download": inner.download,
        }))
    }

    async fn check_now(&self, state: AppState, reason: &str) -> anyhow::Result<()> {
        let should_run = {
            let mut inner = self.inner.lock().unwrap();
            if inner.check_in_progress {
                false
            } else {
                inner.check_in_progress = true;
                true
            }
        };

        if !should_run {
            loop {
                if !self.inner.lock().unwrap().check_in_progress {
                    return Ok(());
                }
                time::sleep(Duration::from_millis(50)).await;
            }
        }

        let result = self.check_now_inner(&state, reason).await;
        let mut inner = self.inner.lock().unwrap();
        inner.check_in_progress = false;
        result
    }

    async fn check_now_inner(&self, state: &AppState, reason: &str) -> anyhow::Result<()> {
        let translator = Translator::from_state(state).await;
        match self.fetch_manifest(&translator).await {
            Ok(manifest) => {
                let previous = {
                    let inner = self.inner.lock().unwrap();
                    (inner.latest_manifest.clone(), inner.has_update)
                };
                let has_update = manifest.update_available
                    && compare_version(&manifest.version, APP_LOCAL_VERSION) > 0;
                if has_update {
                    self.resolve_manifest_package(&manifest, &translator)
                        .map_err(anyhow::Error::msg)?;
                }
                let force_update = has_update && manifest.force_update;
                let should_publish_update_event = {
                    let mut inner = self.inner.lock().unwrap();
                    let stale_download = inner
                        .download
                        .target_version
                        .as_ref()
                        .is_some_and(|version| version != &manifest.version)
                        && !matches!(inner.download.status.as_str(), "downloading" | "installing");
                    if stale_download {
                        inner.download = DownloadState::default();
                        inner.downloaded_path = None;
                        inner.downloaded_sha256 = None;
                    }
                    inner.latest_manifest = Some(manifest.clone());
                    inner.last_checked_at = Some(time_utils::now_ms());
                    inner.check_error = None;
                    inner.update_enabled = manifest.update_available;
                    inner.has_update = has_update;
                    inner.force_update = force_update;

                    has_update
                        && (!previous.1
                            || previous
                                .0
                                .as_ref()
                                .is_none_or(|old| old.version != manifest.version))
                };

                if should_publish_update_event {
                    tracing::info!(
                        version = manifest.version,
                        force_update = manifest.force_update,
                        reason,
                        "application update is available"
                    );
                    if let Err(error) = system_events::publish_app_update_available_event(
                        state,
                        APP_LOCAL_VERSION,
                        &manifest.version,
                        manifest.force_update,
                        &manifest.release_notes,
                        reason,
                    )
                    .await
                    {
                        tracing::warn!(
                            %error,
                            version = manifest.version,
                            "failed to publish app update available event"
                        );
                    }
                }
            }
            Err(error) => {
                let mut inner = self.inner.lock().unwrap();
                inner.check_error = Some(error.clone());
                inner.last_checked_at = Some(time_utils::now_ms());
                tracing::warn!(%error, reason, "update check failed");
            }
        }
        Ok(())
    }

    async fn trigger_download(&'static self, state: AppState) -> Result<(), String> {
        let translator = Translator::from_state(&state).await;
        {
            let inner = self.inner.lock().unwrap();
            if inner.download_in_progress
                || matches!(inner.download.status.as_str(), "downloading" | "verifying")
            {
                return Ok(());
            }
        }
        if self.inner.lock().unwrap().latest_manifest.is_none() {
            let _ = self.check_now(state.clone(), "download-bootstrap").await;
        }
        let (manifest, target_package, target_path) = {
            let inner = self.inner.lock().unwrap();
            let manifest = inner
                .latest_manifest
                .clone()
                .ok_or_else(|| update_manager_text(&translator, "noUpdateInfo"))?;
            if !inner.update_enabled {
                return Err(update_manager_text(&translator, "featureDisabled"));
            }
            if !inner.has_update {
                return Err(update_manager_text(&translator, "alreadyLatest"));
            }
            let target_package = self.resolve_manifest_package(&manifest, &translator)?;
            let target_path =
                self.build_package_path(&manifest.version, target_package.architecture);
            if inner.download.status == "downloaded"
                && inner.downloaded_path.as_ref() == Some(&target_path)
                && inner.downloaded_sha256.as_deref() == Some(target_package.sha256.as_str())
                && target_path.exists()
            {
                return Ok(());
            }
            (manifest, target_package, target_path)
        };
        {
            let mut inner = self.inner.lock().unwrap();
            inner.download_in_progress = true;
            inner.download = DownloadState {
                status: "downloading".to_string(),
                percent: 0,
                downloaded_bytes: 0,
                total_bytes: None,
                error: None,
                target_version: Some(manifest.version.clone()),
            };
        }
        tokio::spawn(async move {
            if let Err(error) = self
                .download_internal(translator, manifest, target_package, target_path)
                .await
            {
                self.set_download_error(&error);
                tracing::warn!(%error, "update download failed");
            }
            self.inner.lock().unwrap().download_in_progress = false;
        });
        Ok(())
    }

    async fn trigger_install(&self, state: &AppState) -> Result<(), String> {
        let translator = Translator::from_state(state).await;
        let (manifest, target_package, downloaded_path) = {
            let inner = self.inner.lock().unwrap();
            if inner.download.status == "installing" {
                return Ok(());
            }
            let manifest = inner
                .latest_manifest
                .clone()
                .ok_or_else(|| update_manager_text(&translator, "noInstallableUpdate"))?;
            if !inner.update_enabled || !inner.has_update {
                return Err(update_manager_text(&translator, "noInstallableUpdate"));
            }
            let downloaded_path = inner
                .downloaded_path
                .clone()
                .filter(|_| inner.download.status == "downloaded")
                .ok_or_else(|| update_manager_text(&translator, "downloadPackageFirst"))?;
            let target_package = self.resolve_manifest_package(&manifest, &translator)?;
            (manifest, target_package, downloaded_path)
        };
        if !downloaded_path.exists() {
            self.reset_download_state();
            return Err(update_manager_text(&translator, "packageMissing"));
        }
        let current_sha = compute_file_sha256(&downloaded_path)
            .map_err(|error| error.to_string())?
            .to_ascii_lowercase();
        if current_sha != target_package.sha256.to_ascii_lowercase() {
            self.reset_download_state();
            return Err(update_manager_text(&translator, "packageChecksumFailed"));
        }
        let pending = json!({
            "targetVersion": manifest.version,
            "requestedAt": time_utils::now_iso()
        });
        state
            .redis
            .set_json_value_ex(UPDATE_PENDING_KEY, &pending, UPDATE_PENDING_TTL_SECONDS)
            .await
            .map_err(|error| error.to_string())?;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.download.status = "installing".to_string();
            inner.download.error = None;
        }
        self.write_and_launch_install_script(&downloaded_path, &translator)
    }

    async fn consume_confirm_message(&self, state: &AppState) -> anyhow::Result<Option<Value>> {
        self.ensure_confirm_by_pending(state).await?;
        Ok(state
            .redis
            .consume_json_value(UPDATE_CONFIRM_KEY)
            .await?
            .filter(is_valid_confirm_payload))
    }

    async fn ensure_confirm_by_pending(&self, state: &AppState) -> anyhow::Result<()> {
        {
            let inner = self.inner.lock().unwrap();
            if inner.confirmed_pending_on_boot {
                return Ok(());
            }
        }
        let pending = state.redis.get_json_value(UPDATE_PENDING_KEY).await?;
        if let Some(pending) = pending {
            if let Some(target_version) = pending_confirmed_target_version(&pending) {
                let confirm = json!({
                    "version": target_version,
                    "completedAt": time_utils::now_iso()
                });
                state
                    .redis
                    .set_json_value_ex(UPDATE_CONFIRM_KEY, &confirm, UPDATE_CONFIRM_TTL_SECONDS)
                    .await?;
                state
                    .redis
                    .delete_keys(&[UPDATE_PENDING_KEY.to_string()])
                    .await?;
            }
        }
        self.inner.lock().unwrap().confirmed_pending_on_boot = true;
        Ok(())
    }

    async fn fetch_manifest(&self, translator: &Translator) -> Result<OtaManifest, String> {
        let mut url = url::Url::parse(OTA_LATEST_URL).map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .append_pair("t", &time_utils::now_ms().to_string());
        let client = update_http_client(UPDATE_CHECK_TIMEOUT_MS)?;
        let response = client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .header(reqwest::header::PRAGMA, "no-cache")
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(update_manager_text_params(
                translator,
                "checkHttpFailed",
                &[("status", status.as_u16().to_string())],
            ));
        }
        let payload = response.json::<Value>().await.unwrap_or(Value::Null);
        parse_manifest(&payload, translator)
    }

    async fn download_internal(
        &'static self,
        translator: Translator,
        manifest: OtaManifest,
        target_package: ResolvedPackage,
        target_path: PathBuf,
    ) -> Result<(), String> {
        self.ensure_dirs();
        let temp_path = target_path.with_extension("fpk.tmp");
        if temp_path.exists() {
            let _ = fs::remove_file(&temp_path);
        }
        let result = async {
            let client = update_http_client(UPDATE_DOWNLOAD_TIMEOUT_MS)?;
            let mut file = fs::File::create(&temp_path).map_err(|error| error.to_string())?;
            let mut response = client
                .get(&target_package.download_url)
                .header(reqwest::header::CACHE_CONTROL, "no-cache")
                .header(reqwest::header::PRAGMA, "no-cache")
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if !response.status().is_success() {
                return Err(update_manager_text_params(
                    &translator,
                    "downloadHttpFailed",
                    &[("status", response.status().as_u16().to_string())],
                ));
            }
            let total = response
                .content_length()
                .filter(|value| *value > 0)
                .map(|value| value as i64);
            let mut loaded = 0i64;
            while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
                file.write_all(&chunk).map_err(|error| error.to_string())?;
                loaded += chunk.len() as i64;
                let percent = download_percent(loaded, total);
                let mut inner = self.inner.lock().unwrap();
                inner.download = DownloadState {
                    status: "downloading".to_string(),
                    percent,
                    downloaded_bytes: loaded,
                    total_bytes: total,
                    error: None,
                    target_version: Some(manifest.version.clone()),
                };
            }
            file.flush().map_err(|error| error.to_string())?;
            {
                let mut inner = self.inner.lock().unwrap();
                inner.download.status = "verifying".to_string();
                inner.download.percent = 100;
            }
            let sha256 = compute_file_sha256(&temp_path)
                .map_err(|error| error.to_string())?
                .to_ascii_lowercase();
            if sha256 != target_package.sha256.to_ascii_lowercase() {
                return Err(update_manager_text_params(
                    &translator,
                    "checksumFailed",
                    &[
                        ("expected", target_package.sha256.clone()),
                        ("actual", sha256),
                    ],
                ));
            }
            fs::rename(&temp_path, &target_path).map_err(|error| error.to_string())?;
            let size = fs::metadata(&target_path)
                .map(|metadata| metadata.len() as i64)
                .unwrap_or(loaded);
            let mut inner = self.inner.lock().unwrap();
            inner.downloaded_path = Some(target_path);
            inner.downloaded_sha256 = Some(sha256);
            inner.download = DownloadState {
                status: "downloaded".to_string(),
                percent: 100,
                downloaded_bytes: size,
                total_bytes: Some(size),
                error: None,
                target_version: Some(manifest.version),
            };
            Ok(())
        }
        .await;
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn build_package_path(&self, version: &str, architecture: &str) -> PathBuf {
        self.package_download_dir
            .join(format!("fn-knock-{version}-{architecture}.fpk"))
    }

    fn resolve_manifest_package(
        &self,
        manifest: &OtaManifest,
        translator: &Translator,
    ) -> Result<ResolvedPackage, String> {
        let architecture = detect_architecture(translator)?;
        if architecture == "arm64" {
            if manifest.download_url_arm64.trim().is_empty() {
                return Err(update_manager_text(
                    translator,
                    "manifestMissingArm64DownloadUrl",
                ));
            }
            if manifest.sha256_arm64.trim().is_empty() {
                return Err(update_manager_text(
                    translator,
                    "manifestMissingArm64Checksum",
                ));
            }
            return Ok(ResolvedPackage {
                architecture,
                download_url: manifest.download_url_arm64.clone(),
                sha256: manifest.sha256_arm64.clone(),
            });
        }
        Ok(ResolvedPackage {
            architecture,
            download_url: manifest.download_url.clone(),
            sha256: manifest.sha256.clone(),
        })
    }

    fn reset_download_state(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.download = DownloadState::default();
        inner.downloaded_path = None;
        inner.downloaded_sha256 = None;
    }

    fn set_download_error(&self, message: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.download.status = "error".to_string();
        inner.download.error = Some(message.to_string());
    }

    fn write_and_launch_install_script(
        &self,
        downloaded_path: &Path,
        translator: &Translator,
    ) -> Result<(), String> {
        self.ensure_dirs();
        let script_path = self.updates_dir.join("apply-update.sh");
        let env_content = build_install_env_content();
        fs::write(&self.install_env_path, env_content).map_err(|error| error.to_string())?;
        let escaped_path = shell_escape_path(downloaded_path);
        let escaped_env_path = shell_escape_path(&self.install_env_path);
        let script = format!(
            r#"#!/bin/sh
set -eu
sleep 2

if [ "$(id -u)" -ne 0 ]; then
  if command -v sudo >/dev/null 2>&1; then
    exec sudo -n /bin/sh "$0" "$@"
  fi
  echo "root privileges are required for update installation" >&2
  exit 1
fi

resolve_install_volume() {{
  volume=""
  for dir in /vol[1-9]*; do
    [ -d "$dir/@appcenter" ] || continue
    candidate="$(basename "$dir" | sed 's/^vol//')"
    case "$candidate" in
      ''|*[!0-9]*)
        continue
        ;;
    esac
    volume="$candidate"
    break
  done
  echo "$volume"
}}

install_volume="$(resolve_install_volume)"
appcenter-cli stop fn-knock || true
appcenter-cli uninstall fn-knock || true
mkdir -p /tmp/appcenter
if [ -n "$install_volume" ]; then
  echo "Using appcenter volume: $install_volume"
  appcenter-cli install-fpk "{escaped_path}" --env "{escaped_env_path}" --volume "$install_volume"
else
  appcenter-cli install-fpk "{escaped_path}" --env "{escaped_env_path}"
fi
appcenter-cli start fn-knock
"#
        );
        fs::write(&script_path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        let escaped_script_path = shell_escape_path(&script_path);
        let escaped_log_path = shell_escape_path(&self.install_log_path);
        let launcher = format!(
            r#"if command -v setsid >/dev/null 2>&1; then
  nohup setsid /bin/sh "{escaped_script_path}" > "{escaped_log_path}" 2>&1 < /dev/null &
else
  nohup /bin/sh "{escaped_script_path}" > "{escaped_log_path}" 2>&1 < /dev/null &
fi"#
        );
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg(launcher)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            let message = update_manager_text(translator, "installStartFailed");
            self.set_download_error(&message);
            Err(message)
        }
    }
}

fn manager(state: &AppState) -> &'static UpdateManager {
    UPDATE_MANAGER.get_or_init(|| UpdateManager::new(&state.settings.data_dir))
}

fn update_http_client(timeout_ms: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|error| error.to_string())
}

fn pending_confirmed_target_version(pending: &Value) -> Option<String> {
    let target_version = pending.get("targetVersion").and_then(Value::as_str)?.trim();
    if target_version.is_empty() {
        return None;
    }
    (compare_version(target_version, APP_LOCAL_VERSION) == 0).then(|| target_version.to_string())
}

fn is_valid_confirm_payload(payload: &Value) -> bool {
    payload
        .get("version")
        .and_then(Value::as_str)
        .is_some_and(|version| !version.trim().is_empty())
}

fn parse_manifest(value: &Value, translator: &Translator) -> Result<OtaManifest, String> {
    let Some(object) = value.as_object() else {
        return Err(update_manager_text(translator, "manifestFormatInvalid"));
    };

    let version = manifest_string(object, "version");
    let update_available = object.get("update_available").and_then(Value::as_bool);
    let force_update = object.get("force_update").and_then(Value::as_bool);
    let download_url = manifest_string(object, "download_url");
    let sha256 = ensure_sha256(&manifest_string(object, "sha256"), "sha256", translator)?;
    let download_url_arm64 = manifest_string(object, "download_url_arm64");
    let sha256_arm64_raw = manifest_string(object, "sha256_arm64");
    let release_notes = manifest_raw_string(object, "release_notes");

    if version.is_empty() {
        return Err(update_manager_text(translator, "manifestMissingVersion"));
    }
    let update_available = update_available
        .ok_or_else(|| update_manager_text(translator, "manifestMissingUpdateAvailable"))?;
    let force_update = force_update
        .ok_or_else(|| update_manager_text(translator, "manifestMissingForceUpdate"))?;
    if download_url.is_empty() {
        return Err(update_manager_text(
            translator,
            "manifestMissingDownloadUrl",
        ));
    }
    if download_url_arm64.is_empty() != sha256_arm64_raw.is_empty() {
        return Err(update_manager_text(
            translator,
            "manifestArm64FieldsIncomplete",
        ));
    }
    let sha256_arm64 = if sha256_arm64_raw.is_empty() {
        String::new()
    } else {
        ensure_sha256(&sha256_arm64_raw, "sha256_arm64", translator)?
    };

    Ok(OtaManifest {
        version,
        update_available,
        force_update,
        download_url,
        sha256,
        download_url_arm64,
        sha256_arm64,
        release_notes,
    })
}

fn manifest_string(object: &serde_json::Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn manifest_raw_string(object: &serde_json::Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn ensure_sha256(value: &str, field: &str, translator: &Translator) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() == 64 && normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(normalized)
    } else {
        Err(update_manager_text_params(
            translator,
            "manifestFieldInvalid",
            &[("field", field.to_string())],
        ))
    }
}

fn compare_version(a: &str, b: &str) -> i32 {
    let left = normalize_version(a);
    let right = normalize_version(b);
    let max_len = left.len().max(right.len()).max(3);
    for index in 0..max_len {
        let l = *left.get(index).unwrap_or(&0);
        let r = *right.get(index).unwrap_or(&0);
        if l > r {
            return 1;
        }
        if l < r {
            return -1;
        }
    }
    0
}

fn normalize_version(value: &str) -> Vec<i64> {
    value
        .trim()
        .split('.')
        .map(|part| {
            let digits = part
                .chars()
                .skip_while(|ch| !ch.is_ascii_digit())
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<i64>().unwrap_or_default()
        })
        .collect()
}

fn detect_architecture(translator: &Translator) -> Result<&'static str, String> {
    match std::env::consts::ARCH {
        "x86_64" | "amd64" => Ok("amd64"),
        "aarch64" | "arm64" => Ok("arm64"),
        other => Err(update_manager_text_params(
            translator,
            "architectureUnsupported",
            &[("arch", other.to_string())],
        )),
    }
}

fn compute_file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn download_percent(loaded: i64, total: Option<i64>) -> i64 {
    total
        .filter(|value| *value > 0)
        .map(|value| (((loaded as f64 / value as f64) * 100.0).round() as i64).clamp(0, 100))
        .unwrap_or(0)
}

fn build_install_env_content() -> String {
    let backend_port = resolve_install_port(&["wizard_backend_port", "BACKEND_PORT"], "7998");
    let auth_port = resolve_install_port(&["wizard_auth_port", "AUTH_PORT"], "7997");
    let go_backend_port =
        resolve_install_port(&["wizard_go_backend_port", "GO_BACKEND_PORT"], "7996");
    let go_reproxy_port = resolve_install_port(
        &[
            "wizard_go_reproxy_port",
            "GO_REPROXY_PORT",
            "TRIM_SERVICE_PORT",
        ],
        "7999",
    );
    [
        format!("wizard_backend_port={backend_port}"),
        format!("wizard_auth_port={auth_port}"),
        format!("wizard_go_backend_port={go_backend_port}"),
        format!("wizard_go_reproxy_port={go_reproxy_port}"),
        format!("BACKEND_PORT={backend_port}"),
        format!("AUTH_PORT={auth_port}"),
        format!("GO_BACKEND_PORT={go_backend_port}"),
        format!("GO_REPROXY_PORT={go_reproxy_port}"),
        String::new(),
    ]
    .join("\n")
}

fn resolve_install_port(keys: &[&str], fallback: &str) -> String {
    for key in keys {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if let Some(port) = parse_node_decimal_port(trimmed) {
                return port.to_string();
            }
        }
    }
    fallback.to_string()
}

fn parse_node_decimal_port(value: &str) -> Option<u16> {
    let trimmed = value.trim_start();
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let parsed = digits.parse::<i64>().ok()?;
    let signed = if negative { -parsed } else { parsed };
    (1..=65_535).contains(&signed).then_some(signed as u16)
}

fn shell_escape_path(path: &Path) -> String {
    path.to_string_lossy().replace('"', "\\\"")
}

fn self_update_available(state: &AppState) -> bool {
    deployment_target(state) == "fpk"
}

fn self_update_unavailable_message(state: &AppState, translator: &Translator) -> String {
    let profile = runtime_profile::get_runtime_profile(state);
    runtime_profile::capability_unavailable_message("self_update_available", &profile, translator)
}

fn deployment_target(state: &AppState) -> String {
    runtime_profile::deployment_target(state)
}

fn update_check_interval() -> Duration {
    Duration::from_secs(2 * 60 * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_like_node() {
        assert_eq!(compare_version("1.8.7", "1.8.6"), 1);
        assert_eq!(compare_version("1.8.6", "1.8.6"), 0);
        assert_eq!(compare_version("1.8.6-beta", "1.8.7"), -1);
        assert_eq!(compare_version("v1.8.8", "1.8.8"), 0);
    }

    #[test]
    fn parses_valid_manifest() {
        let translator = Translator::new("zh-CN");
        let manifest = parse_manifest(
            &json!({
                "version": "1.8.7",
                "update_available": true,
                "force_update": false,
                "download_url": "https://example.com/app.fpk",
                "sha256": "a".repeat(64),
                "download_url_arm64": "",
                "sha256_arm64": "",
                "release_notes": "notes"
            }),
            &translator,
        )
        .unwrap();
        assert_eq!(manifest.version, "1.8.7");
        assert!(manifest.update_available);
    }

    #[test]
    fn manifest_release_notes_preserve_raw_string_like_node() {
        let translator = Translator::new("zh-CN");
        let manifest = parse_manifest(
            &json!({
                "version": "1.8.7",
                "update_available": true,
                "force_update": false,
                "download_url": "https://example.com/app.fpk",
                "sha256": "a".repeat(64),
                "release_notes": "  notes\n "
            }),
            &translator,
        )
        .unwrap();
        assert_eq!(manifest.release_notes, "  notes\n ");
    }

    #[test]
    fn rejects_invalid_manifest_checksum() {
        let translator = Translator::new("zh-CN");
        let error = parse_manifest(
            &json!({
                "version": "1.8.7",
                "update_available": true,
                "force_update": false,
                "download_url": "https://example.com/app.fpk",
                "sha256": "bad"
            }),
            &translator,
        )
        .unwrap_err();
        assert!(error.contains("sha256"));
    }

    #[test]
    fn rejects_invalid_manifest_json_like_node() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            parse_manifest(&Value::Null, &translator).unwrap_err(),
            update_manager_text(&translator, "manifestFormatInvalid")
        );
    }

    #[test]
    fn manifest_validates_main_checksum_before_required_fields_like_node() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            parse_manifest(&json!({}), &translator).unwrap_err(),
            update_manager_text_params(
                &translator,
                "manifestFieldInvalid",
                &[("field", "sha256".to_string())]
            )
        );
    }

    #[test]
    fn manifest_reports_arm64_incomplete_before_arm64_checksum_like_node() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            parse_manifest(
                &json!({
                    "version": "1.8.7",
                    "update_available": true,
                    "force_update": false,
                    "download_url": "https://example.com/app.fpk",
                    "sha256": "a".repeat(64),
                    "sha256_arm64": "bad"
                }),
                &translator
            )
            .unwrap_err(),
            update_manager_text(&translator, "manifestArm64FieldsIncomplete")
        );
    }

    #[test]
    fn manifest_validates_arm64_checksum_when_arm64_url_exists_like_node() {
        let translator = Translator::new("zh-CN");
        let error = parse_manifest(
            &json!({
                "version": "1.8.7",
                "update_available": true,
                "force_update": false,
                "download_url": "https://example.com/app.fpk",
                "sha256": "a".repeat(64),
                "download_url_arm64": "https://example.com/app-arm64.fpk",
                "sha256_arm64": "bad"
            }),
            &translator,
        )
        .unwrap_err();
        assert!(error.contains("sha256_arm64"));
    }

    #[test]
    fn pending_confirmation_uses_node_version_comparison() {
        assert_eq!(
            pending_confirmed_target_version(
                &json!({ "targetVersion": format!(" v{} ", APP_LOCAL_VERSION) })
            ),
            Some(format!("v{}", APP_LOCAL_VERSION))
        );
        assert_eq!(
            pending_confirmed_target_version(&json!({ "targetVersion": "0.0.1" })),
            None
        );
        assert_eq!(
            pending_confirmed_target_version(&json!({ "targetVersion": "   " })),
            None
        );
    }

    #[test]
    fn validates_confirm_payload_like_node() {
        assert!(is_valid_confirm_payload(&json!({
            "version": "1.8.8",
            "completedAt": "2026-07-07T00:00:00.000Z"
        })));
        assert!(!is_valid_confirm_payload(&json!({ "version": "" })));
        assert!(!is_valid_confirm_payload(&json!({ "completedAt": "now" })));
    }

    #[test]
    fn parses_install_ports_like_node_parse_int() {
        assert_eq!(parse_node_decimal_port("7999"), Some(7_999));
        assert_eq!(parse_node_decimal_port("7999abc"), Some(7_999));
        assert_eq!(parse_node_decimal_port("+7999"), Some(7_999));
        assert_eq!(parse_node_decimal_port("0x10"), None);
        assert_eq!(parse_node_decimal_port("0"), None);
        assert_eq!(parse_node_decimal_port("-1"), None);
        assert_eq!(parse_node_decimal_port("65536"), None);
        assert_eq!(parse_node_decimal_port("abc7999"), None);
    }

    #[test]
    fn download_progress_rounds_like_node() {
        assert_eq!(download_percent(1, Some(3)), 33);
        assert_eq!(download_percent(2, Some(3)), 67);
        assert_eq!(download_percent(5, Some(0)), 0);
        assert_eq!(download_percent(5, None), 0);
        assert_eq!(download_percent(120, Some(100)), 100);
    }

    #[test]
    fn localizes_update_route_and_manager_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            update_route_text(&translator, "downloadStarted"),
            "已开始下载更新包"
        );
        assert_eq!(
            parse_manifest(&json!({}), &translator).unwrap_err(),
            "更新信息 sha256 无效"
        );
        assert_eq!(
            ensure_sha256("bad", "sha256", &translator).unwrap_err(),
            "更新信息 sha256 无效"
        );
    }
}
