use super::*;

pub(super) async fn current_acme_install_state(state: &AppState, t: &Translator) -> Value {
    if let Some(raw) = state.acme_install_state.read().await.clone()
        && raw.get("status").and_then(Value::as_str) == Some("installing")
    {
        return localize_acme_install_state(raw, t);
    }

    if let Err(error) = migrate_legacy_acme_install_if_needed(state).await {
        set_acme_install_state(
            state,
            "error",
            0,
            "checkInstallFailed",
            &[("detail", error.to_string())],
        )
        .await;
        if let Some(raw) = state.acme_install_state.read().await.clone() {
            return localize_acme_install_state(raw, t);
        }
    }

    let executable_path = acme_executable_path(state);
    if executable_path.is_file() {
        json!({
            "status": "installed",
            "progress": 100,
            "message": t.t("server.acmeService.ready"),
            "messageKey": "ready",
            "executablePath": executable_path,
        })
    } else if let Some(raw) = state.acme_install_state.read().await.clone()
        && raw.get("status").and_then(Value::as_str) == Some("error")
    {
        localize_acme_install_state(raw, t)
    } else {
        acme_install_state_value(state, "uninstalled", 0, "notInstalled", &[], t)
    }
}

pub(super) fn acme_executable_path(state: &AppState) -> PathBuf {
    state.settings.data_dir.join(".acme.sh").join("acme.sh")
}

pub(super) async fn acme_install_is_installing(state: &AppState) -> bool {
    state
        .acme_install_state
        .read()
        .await
        .as_ref()
        .and_then(|value| value.get("status").and_then(Value::as_str))
        == Some("installing")
}

pub(super) async fn set_acme_install_state(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
) {
    let value = acme_install_state_value(
        state,
        status,
        progress,
        message_key,
        params,
        &Translator::new(DEFAULT_ACME_LOCALE),
    );
    *state.acme_install_state.write().await = Some(value);
}

pub(super) fn acme_install_state_value(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
    t: &Translator,
) -> Value {
    let full_key = format!("server.acmeService.{message_key}");
    let mut params_object = Map::new();
    for (key, value) in params {
        params_object.insert((*key).to_string(), json!(value));
    }
    json!({
        "status": status,
        "progress": progress.clamp(0, 100),
        "message": t.t_params(&full_key, params),
        "messageKey": message_key,
        "messageParams": params_object,
        "executablePath": acme_executable_path(state),
    })
}

pub(super) fn localize_acme_install_state(mut raw: Value, t: &Translator) -> Value {
    let Some(message_key) = raw
        .get("messageKey")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return raw;
    };
    let owned_params = raw
        .get("messageParams")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .as_str()
                        .map(|value| (key.to_string(), value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let params = owned_params
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect::<Vec<_>>();
    raw["message"] = json!(t.t_params(&format!("server.acmeService.{message_key}"), &params));
    raw
}

const DEFAULT_ACME_LOCALE: &str = "zh-CN";

pub(super) fn default_certificate_authority(state: &AppState) -> &'static str {
    let account_conf = state
        .settings
        .data_dir
        .join(".acme.sh")
        .join("account.conf");
    let content = std::fs::read_to_string(account_conf).unwrap_or_default();
    for line in content.lines() {
        let Some(raw) = line.strip_prefix("DEFAULT_ACME_SERVER=") else {
            continue;
        };
        let lower = raw.trim_matches(['"', '\'']).to_ascii_lowercase();
        if lower.contains("letsencrypt") {
            return "letsencrypt";
        }
        if lower.contains("zerossl") {
            return "zerossl";
        }
    }
    DEFAULT_ACME_CERTIFICATE_AUTHORITY
}

pub(super) fn normalize_certificate_authority(value: Option<&str>) -> String {
    if value == Some("letsencrypt") {
        "letsencrypt".to_string()
    } else {
        DEFAULT_ACME_CERTIFICATE_AUTHORITY.to_string()
    }
}

pub(super) async fn save_client_settings(
    state: &AppState,
    certificate_authority: &str,
) -> redis::RedisResult<Value> {
    let settings = json!({
        "certificateAuthority": normalize_certificate_authority(Some(certificate_authority)),
        "updatedAt": time_utils::now_iso(),
    });
    state
        .redis
        .set_json_value(ACME_CLIENT_SETTINGS_KEY, &settings)
        .await?;
    Ok(settings)
}

pub(super) async fn migrate_legacy_acme_install_if_needed(state: &AppState) -> anyhow::Result<()> {
    let acme_home = acme_home_dir(state);
    let legacy_home = legacy_acme_home_dir();
    if acme_home == legacy_home || !legacy_home.join("acme.sh").is_file() {
        return Ok(());
    }
    let acme_home_clone = acme_home.clone();
    tokio::task::spawn_blocking(move || {
        if acme_home_clone.exists() {
            std::fs::remove_dir_all(&acme_home_clone)?;
        }
        std::fs::create_dir_all(&acme_home_clone)?;
        copy_dir_recursive(&legacy_home, &acme_home_clone)?;
        chmod_executable(&acme_home_clone.join("acme.sh"));
        Ok::<(), anyhow::Error>(())
    })
    .await?
}

pub(super) async fn start_acme_install(state: AppState, certificate_authority: String) {
    if acme_install_is_installing(&state).await || acme_executable_path(&state).is_file() {
        return;
    }
    set_acme_install_state(&state, "installing", 10, "initializingBundled", &[]).await;

    let install_result = async {
        let install_state = state.clone();
        let executable_path =
            tokio::task::spawn_blocking(move || install_from_bundled_zip_blocking(&install_state))
                .await??;

        set_acme_install_state(&state, "installing", 90, "registeringAccount", &[]).await;
        let account_email = register_acme_account(
            &state,
            None,
            Some(&certificate_authority),
            &Translator::new(DEFAULT_ACME_LOCALE),
        )
        .await?;
        set_acme_install_state(&state, "installing", 95, "savingDefaultCa", &[]).await;
        set_default_certificate_authority(
            &state,
            &certificate_authority,
            &Translator::new(DEFAULT_ACME_LOCALE),
        )
        .await?;
        Ok::<(PathBuf, String), anyhow::Error>((executable_path, account_email))
    }
    .await;

    match install_result {
        Ok((_executable_path, account_email)) => {
            set_acme_install_state(
                &state,
                "installed",
                100,
                "installSuccess",
                &[("email", account_email)],
            )
            .await;
        }
        Err(error) => {
            set_acme_install_state(
                &state,
                "error",
                0,
                "installFailed",
                &[("detail", error.to_string())],
            )
            .await;
        }
    }
}

pub(super) fn install_from_bundled_zip_blocking(state: &AppState) -> anyhow::Result<PathBuf> {
    let bundle_zip_path = resolve_bundled_acme_zip_path().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.bundledZipMissing")
        )
    })?;
    let acme_home = acme_home_dir(state);
    let executable_path = acme_executable_path(state);
    let tmp_dir = acme_home
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".acme-extract-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));

    set_acme_install_state_blocking(state, "installing", 35, "extractingBundled", &[]);
    std::fs::create_dir_all(&tmp_dir)?;
    let result = (|| {
        extract_zip_file(&bundle_zip_path, &tmp_dir)?;
        let extracted_root = locate_extracted_root(&tmp_dir).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.extractedAcmeMissing")
            )
        })?;
        set_acme_install_state_blocking(state, "installing", 70, "writingDataDir", &[]);
        if acme_home.exists() {
            std::fs::remove_dir_all(&acme_home)?;
        }
        std::fs::create_dir_all(&acme_home)?;
        copy_dir_recursive(&extracted_root, &acme_home)?;
        if !executable_path.is_file() {
            anyhow::bail!(
                "{}",
                Translator::new(DEFAULT_ACME_LOCALE).t("server.acmeService.writtenAcmeMissing")
            );
        }
        chmod_executable(&executable_path);
        Ok(executable_path.clone())
    })();
    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

pub(super) fn set_acme_install_state_blocking(
    state: &AppState,
    status: &str,
    progress: i64,
    message_key: &str,
    params: &[(&str, String)],
) {
    if let Ok(mut guard) = state.acme_install_state.try_write() {
        *guard = Some(acme_install_state_value(
            state,
            status,
            progress,
            message_key,
            params,
            &Translator::new(DEFAULT_ACME_LOCALE),
        ));
    }
}

pub(super) fn resolve_bundled_acme_zip_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var("ACME_BUNDLE_ZIP") {
        if !value.trim().is_empty() {
            candidates.push(PathBuf::from(value.trim()));
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(meta_dir) = exe.parent()
    {
        candidates.extend([
            meta_dir.join("resources/acmesh.zip"),
            meta_dir.join("../resources/acmesh.zip"),
            meta_dir.join("../../resources/acmesh.zip"),
            meta_dir.join("../../../resources/acmesh.zip"),
        ]);
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.extend([
            cwd.join("resources/acmesh.zip"),
            cwd.join("apps/server-admin/resources/acmesh.zip"),
            cwd.join("server/server-admin/resources/acmesh.zip"),
        ]);
    }

    let mut seen = BTreeSet::new();
    candidates.into_iter().find(|path| {
        let normalized = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string();
        seen.insert(normalized) && path.is_file()
    })
}

pub(super) fn extract_zip_file(zip_path: &Path, output_dir: &Path) -> anyhow::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };
        let output_path = output_dir.join(enclosed);
        if entry.is_dir() {
            std::fs::create_dir_all(&output_path)?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&output_path)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

pub(super) fn locate_extracted_root(tmp_dir: &Path) -> Option<PathBuf> {
    for candidate in [
        tmp_dir.join("acmesh"),
        tmp_dir.join(".acme.sh"),
        tmp_dir.to_path_buf(),
    ] {
        if candidate.join("acme.sh").is_file() {
            return Some(candidate);
        }
    }
    let entries = std::fs::read_dir(tmp_dir).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.file_name().and_then(|value| value.to_str()) == Some("__MACOSX") {
            continue;
        }
        if candidate.is_dir() && candidate.join("acme.sh").is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn copy_dir_recursive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() || file_type.is_symlink() {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

pub(super) fn legacy_acme_home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".acme.sh")
}

#[cfg(unix)]
pub(super) fn chmod_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
pub(super) fn chmod_executable(_path: &Path) {}

pub(super) struct AcmeCommandResult {
    pub(super) exit_code: i32,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

pub(super) async fn switch_certificate_authority(
    state: &AppState,
    certificate_authority: &str,
    t: &Translator,
) -> anyhow::Result<String> {
    if !acme_executable_path(state).is_file() {
        anyhow::bail!(t.t("server.acmeService.installFirst"));
    }
    let account_email = register_acme_account(state, None, Some(certificate_authority), t).await?;
    set_default_certificate_authority(state, certificate_authority, t).await?;
    Ok(account_email)
}

pub(super) async fn register_acme_account(
    state: &AppState,
    email: Option<&str>,
    certificate_authority: Option<&str>,
    t: &Translator,
) -> anyhow::Result<String> {
    let account_email = resolve_account_email(state, email).await;
    let mut args = vec![
        "--register-account".to_string(),
        "-m".to_string(),
        account_email.clone(),
    ];
    args.extend(shared_acme_args(state, certificate_authority));
    args.push("--debug".to_string());
    let result = run_acme_command(state, args, None).await?;
    if result.exit_code == 0 {
        return Ok(account_email);
    }
    let merged = format!("{}\n{}", result.stdout, result.stderr).to_ascii_lowercase();
    if (merged.contains("already") && merged.contains("account"))
        || (merged.contains("already") && merged.contains("registered"))
    {
        return Ok(account_email);
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.registerAccountFailed",
        &[
            ("code", result.exit_code.to_string()),
            (
                "brief",
                command_output_brief(&result.stdout, &result.stderr)
            ),
        ],
    ))
}

pub(super) async fn set_default_certificate_authority(
    state: &AppState,
    certificate_authority: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let mut args = vec![
        "--set-default-ca".to_string(),
        "--server".to_string(),
        normalize_certificate_authority(Some(certificate_authority)),
    ];
    args.extend(shared_acme_args(state, None));
    args.push("--debug".to_string());
    let result = run_acme_command(state, args, None).await?;
    if result.exit_code == 0 {
        return Ok(());
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.setDefaultCaFailed",
        &[
            ("code", result.exit_code.to_string()),
            (
                "brief",
                command_output_brief(&result.stdout, &result.stderr)
            ),
        ],
    ))
}

pub(super) async fn run_acme_command(
    state: &AppState,
    args: Vec<String>,
    extra_env: Option<&Map<String, Value>>,
) -> anyhow::Result<AcmeCommandResult> {
    let mut command = Command::new(acme_executable_path(state));
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(extra_env) = extra_env {
        for (key, value) in extra_env {
            if let Some(value) = value.as_str() {
                command.env(key, value);
            }
        }
    }
    let output = command.output().await?;
    Ok(AcmeCommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub(super) fn shared_acme_args(
    state: &AppState,
    certificate_authority: Option<&str>,
) -> Vec<String> {
    let acme_home = acme_home_dir(state).to_string_lossy().to_string();
    let mut args = vec![
        "--home".to_string(),
        acme_home.clone(),
        "--config-home".to_string(),
        acme_home,
    ];
    if let Some(certificate_authority) = certificate_authority {
        args.push("--server".to_string());
        args.push(normalize_certificate_authority(Some(certificate_authority)));
    }
    args
}

pub(super) fn command_output_brief(stdout: &str, stderr: &str) -> String {
    let brief = format!("{stdout}\n{stderr}")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    if brief.is_empty() {
        String::new()
    } else {
        format!(": {brief}")
    }
}

pub(super) async fn resolve_account_email(state: &AppState, email: Option<&str>) -> String {
    if let Some(value) = email.map(str::trim).filter(|value| is_valid_email(value)) {
        return value.to_string();
    }
    if let Ok(value) = env::var("ACME_ACCOUNT_EMAIL")
        && is_valid_email(value.trim())
    {
        return value.trim().to_string();
    }
    if let Some(value) = get_existing_account_email(state).await {
        return value;
    }
    format!(
        "acme-{}-{}@fnknock.com",
        time_utils::now_ms(),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

pub(super) async fn get_existing_account_email(state: &AppState) -> Option<String> {
    let candidates = [
        acme_home_dir(state).join("account.conf"),
        acme_home_dir(state).join("ca/acme.zerossl.com/v2/DV90/account.conf"),
        acme_home_dir(state).join("ca/acme-v02.api.letsencrypt.org/directory/account.conf"),
    ];
    for path in candidates {
        let Ok(content) = tokio::fs::read_to_string(path).await else {
            continue;
        };
        for line in content.lines() {
            let Some(raw) = line.strip_prefix("ACCOUNT_EMAIL=") else {
                continue;
            };
            let value = raw.trim().trim_matches(['"', '\'']);
            if is_valid_email(value) {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(super) fn is_valid_email(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}
