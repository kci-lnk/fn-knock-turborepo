use super::*;
use std::fs;

type OutputGuard = std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>;

pub(super) const OUTPUT_STATUS_PREFIX: &str = "fn_knock:acme:file-output:";

#[derive(Debug, thiserror::Error)]
enum OutputError {
    #[error("Invalid fileOutput configuration")]
    InvalidConfiguration,
    #[error("An absolute output directory without parent traversal is required")]
    InvalidDirectory,
    #[error("Certificate output path is already used by another application")]
    Conflict,
    #[error("Choose an output directory outside internal ACME certificate storage")]
    ManagedDirectory,
    #[error("Output target must be a regular file: {0}")]
    NotRegularFile(String),
}

pub(super) fn output_error_message(error: &anyhow::Error, t: &Translator) -> String {
    let key = match error.downcast_ref::<OutputError>() {
        Some(OutputError::InvalidConfiguration) => "invalidConfiguration",
        Some(OutputError::InvalidDirectory) => "invalidDirectory",
        Some(OutputError::Conflict) => "conflict",
        Some(OutputError::ManagedDirectory) => "managedDirectory",
        Some(OutputError::NotRegularFile(path)) => {
            return t.t_params(
                "server.acmeFileOutput.notRegularFile",
                &[("path", path.clone())],
            );
        }
        None => {
            return t.t_params(
                "server.acmeFileOutput.writeFailed",
                &[("message", error.to_string())],
            );
        }
    };
    t.t(&format!("server.acmeFileOutput.{key}"))
}

pub(super) fn validate_output_location(
    state: &AppState,
    application: &Value,
) -> anyhow::Result<()> {
    if application["fileOutput"]["enabled"] != true {
        return Ok(());
    }
    let path = output_directory(&application["fileOutput"])?;
    let path = canonical_output_directory(&path).unwrap_or(path);
    // Internal cleanup recursively removes these trees. External exports must
    // live elsewhere so deleting an application never removes exported files.
    for root in [
        state.settings.data_dir.join("ssl"),
        acme_home_dir(state),
        legacy_acme_home_dir(),
    ] {
        let root = std::path::absolute(root)?;
        let root = canonical_output_directory(&root).unwrap_or(root);
        anyhow::ensure!(!path.starts_with(root), OutputError::ManagedDirectory);
    }
    Ok(())
}

pub(super) fn normalize_file_output(value: Option<&Value>) -> Value {
    json!({
        "enabled": value.and_then(|v| v.get("enabled")).and_then(Value::as_bool).unwrap_or(false),
        "directory": value.and_then(|v| v.get("directory")).and_then(Value::as_str).unwrap_or("").trim(),
    })
}

fn output_directory(value: &Value) -> anyhow::Result<PathBuf> {
    let directory = value["directory"].as_str().unwrap_or("");
    let path = PathBuf::from(directory);
    anyhow::ensure!(
        path.is_absolute()
            && !directory.chars().any(char::is_control)
            && !path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
        OutputError::InvalidDirectory
    );
    Ok(path)
}

// Resolve existing ancestors too, so aliases of a not-yet-created directory
// cannot evade conflict detection. Missing paths are created only on sync.
fn canonical_output_directory(path: &Path) -> anyhow::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(error)?;
            Ok(canonical_output_directory(parent)?.join(
                path.file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid output directory"))?,
            ))
        }
        Err(error) => Err(error.into()),
    }
}

fn output_paths(application: &Value) -> (PathBuf, PathBuf) {
    let directory = PathBuf::from(
        application["fileOutput"]["directory"]
            .as_str()
            .unwrap_or(""),
    );
    let stem = acme_certificate_archive_stem(application["primaryDomain"].as_str().unwrap_or(""));
    (
        directory.join(format!("{stem}.cert.pem")),
        directory.join(format!("{stem}.key.pem")),
    )
}

pub(super) fn validate_file_output_input(value: &Value) -> anyhow::Result<Value> {
    anyhow::ensure!(
        value.is_object() && value["enabled"].is_boolean() && value["directory"].is_string(),
        OutputError::InvalidConfiguration
    );
    let normalized = normalize_file_output(Some(value));
    if normalized["enabled"] == true {
        output_directory(&normalized)?;
    }
    Ok(normalized)
}

fn path_identity(path: &Path) -> String {
    let identity = path.to_string_lossy().to_string();
    if cfg!(windows) {
        identity.to_lowercase()
    } else {
        identity
    }
}

pub(super) fn check_output_conflicts(
    application: &Value,
    applications: &[Value],
) -> anyhow::Result<()> {
    if application["fileOutput"]["enabled"] != true {
        return Ok(());
    }
    let raw_directory = output_directory(&application["fileOutput"])?;
    let directory = canonical_output_directory(&raw_directory).unwrap_or(raw_directory);
    let (cert_path, _) = output_paths(application);
    let identity = path_identity(&directory.join(cert_path.file_name().unwrap_or_default()));
    for other in applications {
        if other["id"] == application["id"] || other["fileOutput"]["enabled"] != true {
            continue;
        }
        let Ok(directory) =
            output_directory(&other["fileOutput"]).and_then(|p| canonical_output_directory(&p))
        else {
            continue;
        };
        let (other_path, _) = output_paths(other);
        anyhow::ensure!(
            identity != path_identity(&directory.join(other_path.file_name().unwrap_or_default())),
            OutputError::Conflict
        );
    }
    Ok(())
}

pub(super) async fn file_output_status(
    state: &AppState,
    application: &Value,
) -> crate::storage::StorageResult<Value> {
    let certificate = if application["fileOutput"]["enabled"] == true {
        read_issued_certificates(state)
            .await?
            .into_iter()
            .find(|c| {
                c["applicationId"] == application["id"]
                    && issued_certificate_compatible(application, c)
            })
    } else {
        None
    };
    file_output_status_for_certificate(state, application, certificate.as_ref()).await
}

pub(super) async fn file_output_status_for_certificate(
    state: &AppState,
    application: &Value,
    certificate: Option<&Value>,
) -> crate::storage::StorageResult<Value> {
    let id = application["id"].as_str().unwrap_or("");
    let saved = state
        .storage
        .store
        .get_json_value(&format!("{OUTPUT_STATUS_PREFIX}{id}"))
        .await?
        .unwrap_or(Value::Null);
    let (cert_path, key_path) = output_paths(application);
    let mut status = json!({
        "status": "disabled", "certificatePath": cert_path, "privateKeyPath": key_path,
        "lastSuccessAt": saved.get("lastSuccessAt"), "error": Value::Null,
    });
    if application["fileOutput"]["enabled"] != true {
        return Ok(status);
    }
    let Some(certificate) = certificate else {
        status["status"] = json!("waiting");
        return Ok(status);
    };
    status["status"] = json!("pending");
    if saved["certificatePath"] == status["certificatePath"]
        && saved["privateKeyPath"] == status["privateKeyPath"]
        && saved["fingerprint"] == certificate_fingerprint(certificate)
    {
        status["status"] = saved["status"].clone();
        status["error"] = saved["error"].clone();
    }
    Ok(status)
}

fn certificate_fingerprint(certificate: &Value) -> Value {
    let mut digest = Sha256::new();
    digest.update(certificate["cert"].as_str().unwrap_or(""));
    digest.update([0]);
    digest.update(certificate["key"].as_str().unwrap_or(""));
    json!(hex::encode(digest.finalize()))
}

pub(super) async fn sync_file_output(state: &AppState, id: &str) -> anyhow::Result<Value> {
    let guard = std::sync::Arc::new(state.gateway.acme_output_lock.clone().lock_owned().await);
    sync_file_output_locked(state, id, &guard).await
}

// The caller holds acme_output_lock through config/issued-certificate changes
// and disk writes. Always load the newest configuration and certificate here.
pub(super) async fn sync_file_output_locked(
    state: &AppState,
    id: &str,
    guard: &OutputGuard,
) -> anyhow::Result<Value> {
    sync_file_output_locked_with(state, id, guard, write_output_pair).await
}

async fn sync_file_output_locked_with(
    state: &AppState,
    id: &str,
    guard: &OutputGuard,
    write: impl FnOnce(&Path, &Path, &str, &str) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<Value> {
    let application = find_acme_application(state, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ACME application not found"))?;
    let mut status = file_output_status(state, &application).await?;
    if matches!(status["status"].as_str(), Some("disabled" | "waiting")) {
        return Ok(status);
    }
    let certificate = read_issued_certificates(state)
        .await?
        .into_iter()
        .find(|c| c["applicationId"] == id && issued_certificate_compatible(&application, c))
        .ok_or_else(|| anyhow::anyhow!("ACME certificate not found"))?;
    let applications = read_acme_applications(state).await?;
    // Persist pending before starting any filesystem work, so interruption or a
    // worker panic cannot leave an earlier successful save displayed as current.
    status["status"] = json!("pending");
    status["error"] = Value::Null;
    let fingerprint = certificate_fingerprint(&certificate);
    store_output_status(state, id, &status, &fingerprint).await?;

    // The worker owns both the lock and final status persistence. Dropping an
    // HTTP request must neither unlock an unfinished replacement nor lose its
    // outcome. block_on runs only on this blocking thread, never an async worker.
    let worker_guard = guard.clone();
    let worker_state = state.clone();
    let worker_id = id.to_string();
    let worker_status = status.clone();
    let worker_fingerprint = fingerprint.clone();
    let runtime = tokio::runtime::Handle::current();
    let worker = tokio::task::spawn_blocking(move || {
        let _guard = worker_guard;
        let result = (|| {
            validate_output_location(&worker_state, &application)?;
            check_output_conflicts(&application, &applications)?;
            let (cert_path, key_path) = output_paths(&application);
            write(
                &cert_path,
                &key_path,
                certificate["cert"].as_str().unwrap_or(""),
                certificate["key"].as_str().unwrap_or(""),
            )
        })();
        runtime.block_on(finish_output_status(
            &worker_state,
            &worker_id,
            worker_status,
            &worker_fingerprint,
            result,
        ))
    });
    match worker.await {
        Ok(result) => result,
        Err(error) => {
            finish_output_status(state, id, status, &fingerprint, Err(error.into())).await
        }
    }
}

async fn store_output_status(
    state: &AppState,
    id: &str,
    status: &Value,
    fingerprint: &Value,
) -> anyhow::Result<()> {
    let mut stored = status.clone();
    stored["fingerprint"] = fingerprint.clone();
    state
        .storage
        .store
        .set_json_value(&format!("{OUTPUT_STATUS_PREFIX}{id}"), &stored)
        .await?;
    Ok(())
}

async fn finish_output_status(
    state: &AppState,
    id: &str,
    mut status: Value,
    fingerprint: &Value,
    result: anyhow::Result<()>,
) -> anyhow::Result<Value> {
    match result {
        Ok(()) => {
            status["status"] = json!("saved");
            status["lastSuccessAt"] = json!(now_node_iso());
            status["error"] = Value::Null;
        }
        Err(error) => {
            status["status"] = json!("error");
            status["error"] = json!(output_error_message(
                &error,
                &Translator::from_state(state).await
            ));
        }
    }
    store_output_status(state, id, &status, fingerprint).await?;
    Ok(status)
}

fn check_output_file(path: &Path) -> anyhow::Result<Option<fs::Permissions>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.file_type().is_file(),
                OutputError::NotRegularFile(path.display().to_string())
            );
            Ok(Some(metadata.permissions()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn prepare_output_file(
    path: &Path,
    content: &[u8],
    permissions: Option<fs::Permissions>,
) -> anyhow::Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing output directory"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(windows)]
    crate::infra::private_permissions::secure_windows_path(file.path(), false)
        .map_err(anyhow::Error::msg)?;
    file.write_all(content)?;
    if let Some(permissions) = permissions {
        file.as_file().set_permissions(permissions)?;
    }
    file.as_file().sync_all()?;
    Ok(file)
}

fn replace_output_file(file: tempfile::NamedTempFile, path: &Path) -> anyhow::Result<()> {
    check_output_file(path)?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn restore_output_backup(backup: tempfile::NamedTempFile, path: &Path) -> anyhow::Result<()> {
    let backup = backup.into_temp_path();
    let result =
        check_output_file(path).and_then(|_| fs::rename(&backup, path).map_err(Into::into));
    if let Err(error) = result {
        // A failed rollback must not delete the only remaining old certificate.
        // Keep its recovery path in the error reported to the administrator.
        let recovery = backup.keep().map_err(|error| anyhow::anyhow!(error))?;
        anyhow::bail!(
            "{error}; previous certificate preserved at {}",
            recovery.display()
        );
    }
    Ok(())
}

fn write_output_pair(
    cert_path: &Path,
    key_path: &Path,
    cert: &str,
    key: &str,
) -> anyhow::Result<()> {
    write_output_pair_with(cert_path, key_path, cert, key, replace_output_file)
}

fn write_output_pair_with(
    cert_path: &Path,
    key_path: &Path,
    cert: &str,
    key: &str,
    mut replace: impl FnMut(tempfile::NamedTempFile, &Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cert.trim().is_empty() && !key.trim().is_empty(),
        "Empty certificate or private key"
    );
    fs::create_dir_all(
        cert_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Missing output directory"))?,
    )?;
    let cert_permissions = check_output_file(cert_path)?;
    check_output_file(key_path)?;
    let backup = if cert_permissions.is_some() {
        Some(prepare_output_file(
            cert_path,
            &fs::read(cert_path)?,
            cert_permissions.clone(),
        )?)
    } else {
        None
    };
    let cert_file = prepare_output_file(cert_path, cert.as_bytes(), cert_permissions)?;
    let key_file = prepare_output_file(key_path, key.as_bytes(), None)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        key_file
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    replace(cert_file, cert_path)?;
    if let Err(error) = replace(key_file, key_path) {
        let restore = match backup {
            Some(backup) => restore_output_backup(backup, cert_path),
            None => fs::remove_file(cert_path).map_err(Into::into),
        };
        if let Err(restore_error) = restore {
            anyhow::bail!("{error}; failed to restore previous certificate: {restore_error}");
        }
        return Err(error);
    }
    #[cfg(unix)]
    fs::File::open(cert_path.parent().unwrap_or(Path::new("/")))?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
