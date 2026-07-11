use super::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

const LEGO_MANIFEST_URL: &str = "https://cdn.fnknock.cn/alldata/lego/windows/x86_64/stable.json";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegoResourceProgress {
    status: String,
    percent: u8,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LegoResourceManifest {
    schema_version: u32,
    resource: String,
    version: String,
    platform: String,
    architecture: String,
    file_name: String,
    url: String,
    sha256: String,
    size: u64,
    executable: String,
    license: String,
    source: String,
}

static LEGO_PROGRESS: OnceLock<Mutex<LegoResourceProgress>> = OnceLock::new();
static LEGO_CANCEL: AtomicBool = AtomicBool::new(false);

fn progress() -> &'static Mutex<LegoResourceProgress> {
    LEGO_PROGRESS.get_or_init(|| {
        Mutex::new(LegoResourceProgress {
            status: "idle".to_string(),
            percent: 0,
            error: None,
        })
    })
}

pub(super) fn set_progress(status: &str, percent: u8, error: Option<String>) {
    if let Ok(mut value) = progress().lock() {
        value.status = status.to_string();
        value.percent = percent.min(100);
        value.error = error;
    }
}

fn resource_root(state: &AppState) -> PathBuf {
    state.settings.data_dir.join("resources").join("lego")
}

fn current_metadata_path(state: &AppState) -> PathBuf {
    resource_root(state).join("current.json")
}

fn installed_manifest(state: &AppState) -> Option<LegoResourceManifest> {
    let bytes = std::fs::read(current_metadata_path(state)).ok()?;
    let manifest: LegoResourceManifest = serde_json::from_slice(&bytes).ok()?;
    let executable = resource_root(state)
        .join(&manifest.version)
        .join(&manifest.executable);
    executable.is_file().then_some(manifest)
}

pub(super) fn lego_executable_path(state: &AppState) -> Option<PathBuf> {
    let manifest = installed_manifest(state)?;
    Some(
        resource_root(state)
            .join(manifest.version)
            .join(manifest.executable),
    )
}

fn validate_manifest(manifest: &LegoResourceManifest) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.schema_version == 1,
        "unsupported Lego resource manifest schema"
    );
    anyhow::ensure!(
        manifest.resource == "lego",
        "resource manifest is not for Lego"
    );
    anyhow::ensure!(
        manifest.platform == "windows",
        "resource manifest platform is not Windows"
    );
    anyhow::ensure!(
        manifest.architecture == "x86_64",
        "resource manifest architecture is not x86_64"
    );
    anyhow::ensure!(
        manifest.version == "5.2.0",
        "unsupported Lego version {}",
        manifest.version
    );
    anyhow::ensure!(
        manifest.file_name == "lego_v5.2.0_windows_amd64.zip",
        "unexpected Lego archive name"
    );
    anyhow::ensure!(
        manifest.executable == "lego.exe",
        "unexpected Lego executable name"
    );
    anyhow::ensure!(manifest.license == "MIT", "unexpected Lego license");
    anyhow::ensure!(
        manifest.source == "https://github.com/go-acme/lego/releases/tag/v5.2.0",
        "unexpected Lego source"
    );
    anyhow::ensure!(
        manifest.url
            == "https://cdn.fnknock.cn/alldata/lego/v5.2.0/windows/x86_64/lego_v5.2.0_windows_amd64.zip",
        "unexpected Lego archive URL"
    );
    anyhow::ensure!(
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid Lego SHA-256"
    );
    anyhow::ensure!(
        manifest.size > 0 && manifest.size <= 256 * 1024 * 1024,
        "invalid Lego archive size"
    );
    Ok(())
}

fn ensure_not_cancelled() -> anyhow::Result<()> {
    anyhow::ensure!(
        !LEGO_CANCEL.load(Ordering::Relaxed),
        "Lego initialization was cancelled"
    );
    Ok(())
}

async fn download_bytes(
    state: &AppState,
    url: &str,
    expected_size: Option<u64>,
    progress_range: (u8, u8),
) -> anyhow::Result<Vec<u8>> {
    let mut response = state.asset_download_client.get(url).send().await?;
    anyhow::ensure!(
        response.status().is_success(),
        "download returned HTTP {}",
        response.status()
    );
    if let Some(expected_size) = expected_size {
        anyhow::ensure!(
            response.content_length() == Some(expected_size),
            "Lego archive Content-Length mismatch"
        );
    }
    let mut bytes = Vec::with_capacity(expected_size.unwrap_or_default() as usize);
    while let Some(chunk) = response.chunk().await? {
        ensure_not_cancelled()?;
        bytes.extend_from_slice(&chunk);
        if let Some(expected_size) = expected_size {
            anyhow::ensure!(
                bytes.len() as u64 <= expected_size,
                "Lego archive exceeds declared size"
            );
            let span = u64::from(progress_range.1.saturating_sub(progress_range.0));
            let percent =
                u64::from(progress_range.0) + (bytes.len() as u64 * span / expected_size.max(1));
            set_progress("downloading", percent.min(100) as u8, None);
        }
    }
    ensure_not_cancelled()?;
    Ok(bytes)
}

fn extract_lego(archive: &[u8], output: &Path) -> anyhow::Result<PathBuf> {
    let mut zip = ZipArchive::new(Cursor::new(archive))?;
    let mut found = None;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if name.file_name().and_then(|value| value.to_str()) != Some("lego.exe") {
            continue;
        }
        anyhow::ensure!(
            found.is_none(),
            "Lego archive contains multiple lego.exe files"
        );
        std::fs::create_dir_all(output)?;
        let target = output.join("lego.exe");
        let mut file = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut file)?;
        found = Some(target);
    }
    found.ok_or_else(|| anyhow::anyhow!("Lego archive does not contain lego.exe"))
}

#[cfg(windows)]
fn verify_lego_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new(path)
        .arg("--version")
        .creation_flags(0x0800_0000)
        .output()?;
    anyhow::ensure!(output.status.success(), "lego.exe --version failed");
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    anyhow::ensure!(
        version.contains("5.2.0"),
        "downloaded Lego version does not match 5.2.0"
    );
    Ok(())
}

#[cfg(not(windows))]
fn verify_lego_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn atomic_write_manifest(path: &Path, manifest: &LegoResourceManifest) -> anyhow::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("resource metadata path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".current-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> anyhow::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(manifest)?)?;
        file.sync_all()?;
        drop(file);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
            };
            let source = temp
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let target = path
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let moved = unsafe {
                MoveFileExW(
                    source.as_ptr(),
                    target.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            };
            anyhow::ensure!(moved != 0, std::io::Error::last_os_error());
        }
        #[cfg(not(windows))]
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if temp.exists() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

fn install_verified_archive(
    state: &AppState,
    manifest: &LegoResourceManifest,
    archive: &[u8],
) -> anyhow::Result<()> {
    let root = resource_root(state);
    let staging = root.join(format!(".staging-{}", uuid::Uuid::new_v4()));
    let result = (|| -> anyhow::Result<()> {
        let executable = extract_lego(archive, &staging)?;
        verify_lego_executable(&executable)?;
        ensure_not_cancelled()?;

        let target = root.join(&manifest.version);
        let backup = root.join(format!(".rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root)?;
        if target.exists() {
            std::fs::rename(&target, &backup)?;
        }
        if let Err(error) = std::fs::rename(&staging, &target) {
            if backup.exists() {
                let _ = std::fs::rename(&backup, &target);
            }
            return Err(error.into());
        }

        let commit_result = (|| -> anyhow::Result<()> {
            atomic_write_manifest(&current_metadata_path(state), manifest)?;
            Ok(())
        })();
        if let Err(error) = commit_result {
            let _ = std::fs::remove_dir_all(&target);
            if backup.exists() {
                let _ = std::fs::rename(&backup, &target);
            }
            return Err(error);
        }
        if backup.exists() {
            let _ = std::fs::remove_dir_all(backup);
        }
        Ok(())
    })();
    if staging.exists() {
        let _ = std::fs::remove_dir_all(staging);
    }
    result
}

pub(super) async fn initialize_resource_task(state: AppState) -> anyhow::Result<()> {
    LEGO_CANCEL.store(false, Ordering::Relaxed);
    set_progress("downloading", 5, None);
    let manifest_bytes = download_bytes(&state, LEGO_MANIFEST_URL, None, (5, 20)).await?;
    let manifest: LegoResourceManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_manifest(&manifest)?;
    ensure_not_cancelled()?;
    set_progress("downloading", 25, None);
    let archive = download_bytes(&state, &manifest.url, Some(manifest.size), (25, 65)).await?;
    anyhow::ensure!(
        archive.len() as u64 == manifest.size,
        "Lego archive size mismatch"
    );
    let digest = hex::encode(Sha256::digest(&archive));
    anyhow::ensure!(
        digest.eq_ignore_ascii_case(&manifest.sha256),
        "Lego archive SHA-256 mismatch"
    );
    ensure_not_cancelled()?;
    set_progress("verifying", 70, None);
    install_verified_archive(&state, &manifest, &archive)?;
    set_progress("completed", 100, None);
    Ok(())
}

pub(super) async fn resource_status(State(state): State<AppState>) -> Response {
    let installed = installed_manifest(&state);
    let progress = progress()
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    response::ok(json!({
        "supported": cfg!(windows),
        "initialized": installed.is_some(),
        "platform": if cfg!(windows) { "windows-x86_64" } else { "native-acme-sh" },
        "installedVersion": installed.as_ref().map(|value| value.version.as_str()),
        "availableVersion": "5.2.0",
        "progress": progress,
        "providerIds": lego_provider_ids(),
    }))
    .into_response()
}

pub(super) async fn initialize_resource(State(state): State<AppState>) -> Response {
    if !cfg!(windows) {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Lego resource is only required on Windows",
        );
    }
    if progress()
        .lock()
        .is_ok_and(|value| matches!(value.status.as_str(), "downloading" | "verifying"))
    {
        return response::error(
            StatusCode::CONFLICT,
            "Lego resource initialization is already running",
        );
    }
    set_progress("downloading", 0, None);
    tokio::spawn(async move {
        if let Err(error) = initialize_resource_task(state).await {
            let detail = error.to_string();
            if detail == "Lego initialization was cancelled" {
                set_progress("cancelled", 0, None);
            } else {
                set_progress("error", 0, Some(detail));
            }
        }
    });
    response::ok(json!({ "started": true })).into_response()
}

pub(super) async fn cancel_resource_initialization() -> Response {
    LEGO_CANCEL.store(true, Ordering::Relaxed);
    response::ok(json!({ "cancelRequested": true })).into_response()
}

pub(super) async fn delete_resource(State(state): State<AppState>) -> Response {
    if acme_install_is_installing(&state).await {
        return response::error(
            StatusCode::CONFLICT,
            "ACME task is active; stop it before deleting Lego",
        );
    }
    let _ = tokio::fs::remove_dir_all(resource_root(&state)).await;
    set_progress("idle", 0, None);
    response::ok(json!({ "deleted": true })).into_response()
}

pub(super) fn lego_provider_ids() -> &'static [&'static str] {
    &[
        "dns_cf",
        "dns_ali",
        "dns_dp",
        "dns_duckdns",
        "dns_gd",
        "dns_dgon",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn manifest() -> LegoResourceManifest {
        LegoResourceManifest {
            schema_version: 1,
            resource: "lego".into(),
            version: "5.2.0".into(),
            platform: "windows".into(),
            architecture: "x86_64".into(),
            file_name: "lego_v5.2.0_windows_amd64.zip".into(),
            url: "https://cdn.fnknock.cn/alldata/lego/v5.2.0/windows/x86_64/lego_v5.2.0_windows_amd64.zip".into(),
            sha256: "a".repeat(64),
            size: 123,
            executable: "lego.exe".into(),
            license: "MIT".into(),
            source: "https://github.com/go-acme/lego/releases/tag/v5.2.0".into(),
        }
    }

    #[test]
    fn manifest_contract_is_fixed_to_cdn_and_version() {
        assert!(validate_manifest(&manifest()).is_ok());
        let mut invalid = manifest();
        invalid.url = "https://example.com/lego.zip".into();
        assert!(validate_manifest(&invalid).is_err());
    }

    #[test]
    fn extraction_ignores_traversal_and_selects_lego_only() {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut writer = ZipWriter::new(cursor);
            writer
                .start_file("../escape.exe", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"bad").unwrap();
            writer
                .start_file("nested/lego.exe", SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"lego").unwrap();
            writer.finish().unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let executable = extract_lego(&archive, directory.path()).unwrap();
        assert_eq!(std::fs::read(executable).unwrap(), b"lego");
        assert!(
            !directory
                .path()
                .parent()
                .unwrap()
                .join("escape.exe")
                .exists()
        );
    }
}
