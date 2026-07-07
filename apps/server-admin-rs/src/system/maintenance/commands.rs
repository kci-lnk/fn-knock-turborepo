use super::*;

pub(super) async fn ensure_archive_commands_ready() -> Result<(), BackupImportError> {
    if ARCHIVE_COMMANDS_READY.load(Ordering::SeqCst) {
        return Ok(());
    }

    install_archive_commands_if_needed().await?;
    ARCHIVE_COMMANDS_READY.store(true, Ordering::SeqCst);
    Ok(())
}

pub(super) async fn install_archive_commands_if_needed() -> Result<(), BackupImportError> {
    let missing = missing_archive_commands().await?;
    if missing.is_empty() {
        return Ok(());
    }

    let missing_names = missing.join(", ");
    let packages = missing.clone();
    let package_names = packages.join(", ");

    if command_available(OPENWRT_APK_COMMAND, &["--version"]).await? {
        let output = run_command(OPENWRT_APK_COMMAND, &["--update-cache", "add", "unzip"]).await?;
        if !output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    if command_available(OPENWRT_OPKG_COMMAND, &["--version"]).await? {
        let update_output = run_command(OPENWRT_OPKG_COMMAND, &["update"]).await?;
        if !update_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message("opkgUpdateFailed", &[]),
                &update_output,
            ));
        }

        let install_output = run_command(OPENWRT_OPKG_COMMAND, &["install", "unzip"]).await?;
        if !install_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &install_output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    if command_available(DEBIAN_APT_GET_PATH, &["--version"]).await? {
        let update_output = run_command(DEBIAN_APT_GET_PATH, &["update"]).await?;
        if !update_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message("aptUpdateFailed", &[]),
                &update_output,
            ));
        }

        let install_output = run_command(DEBIAN_APT_GET_PATH, &["install", "-y", "unzip"]).await?;
        if !install_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &install_output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    Err(BackupImportError::internal(backup_error_key_message(
        "commandsMissingNoPackageManager",
        &[("commands", missing_names)],
    )))
}

pub(super) async fn ensure_no_archive_commands_missing_after_install()
-> Result<(), BackupImportError> {
    let remaining = missing_archive_commands().await?;
    if remaining.is_empty() {
        return Ok(());
    }
    Err(BackupImportError::internal(backup_error_key_message(
        "commandsStillMissingAfterInstall",
        &[("commands", remaining.join(", "))],
    )))
}

pub(super) async fn missing_archive_commands() -> Result<Vec<String>, BackupImportError> {
    let mut missing = Vec::new();
    if !command_available("unzip", &["-v"]).await? {
        missing.push("unzip".to_string());
    }
    Ok(missing)
}

pub(super) async fn command_available(
    command: &str,
    args: &[&str],
) -> Result<bool, BackupImportError> {
    match Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(BackupImportError::internal(backup_error_key_message(
            "commandCheckFailed",
            &[("command", command.to_string())],
        ))),
    }
}

pub(super) async fn run_command(
    command: &str,
    args: &[&str],
) -> Result<std::process::Output, BackupImportError> {
    Command::new(command)
        .args(args)
        .output()
        .await
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                BackupImportError::internal(backup_error_key_message(
                    "commandMissing",
                    &[("command", command.to_string())],
                ))
            } else {
                BackupImportError::internal(backup_error_key_message(
                    "commandFailed",
                    &[("command", command.to_string())],
                ))
            }
        })
}
